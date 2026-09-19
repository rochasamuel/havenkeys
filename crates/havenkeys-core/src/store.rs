//! SQLite persistence. Stores only the plaintext header needed to unlock and
//! opaque encrypted blobs. Knows nothing about keys.

use crate::crypto::kdf::KdfParams;
use crate::error::{Error, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE vault_header (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    format_version    INTEGER NOT NULL,
    vault_id          TEXT    NOT NULL,
    kdf               TEXT    NOT NULL,
    wrapped_vault_key BLOB    NOT NULL,
    created_at        INTEGER NOT NULL
);
CREATE TABLE items (
    id       TEXT PRIMARY KEY NOT NULL,
    overview BLOB NOT NULL,
    details  BLOB NOT NULL
);
CREATE TABLE settings (
    id   INTEGER PRIMARY KEY CHECK (id = 1),
    blob BLOB NOT NULL
);
";

/// Plaintext vault header.
#[derive(Clone, Debug)]
pub struct HeaderRecord {
    pub format_version: u32,
    pub vault_id: Uuid,
    pub kdf: KdfParams,
    pub wrapped_vault_key: Vec<u8>,
    pub created_at: i64,
}

/// One `items` row: `(id, overview blob)`, or an error if the row is malformed.
pub type OverviewRow = Result<(Uuid, Vec<u8>)>;

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open or create the vault database. On Unix a new file is created with
    /// mode 0600 before SQLite touches it.
    pub fn open(path: &Path) -> Result<Self> {
        create_private_file(path)?;
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // Overwrite freed pages so deleted ciphertexts do not linger.
        conn.pragma_update(None, "secure_delete", "ON")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match version {
            0 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(SCHEMA)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            SCHEMA_VERSION => {}
            _ => return Err(Error::UnsupportedVersion),
        }
        Ok(Self { conn })
    }

    pub fn header(&self) -> Result<Option<HeaderRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT format_version, vault_id, kdf, wrapped_vault_key, created_at
                 FROM vault_header WHERE id = 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((format_version, vault_id, kdf, wrapped_vault_key, created_at)) = row else {
            return Ok(None);
        };
        let format_version = u32::try_from(format_version).map_err(|_| Error::Corrupted)?;
        let vault_id = Uuid::parse_str(&vault_id).map_err(|_| Error::Corrupted)?;
        let kdf: KdfParams = serde_json::from_str(&kdf).map_err(|_| Error::Corrupted)?;
        Ok(Some(HeaderRecord {
            format_version,
            vault_id,
            kdf,
            wrapped_vault_key,
            created_at,
        }))
    }

    /// Write the header and initial settings atomically. Fails if a header exists.
    pub fn insert_header(&mut self, header: &HeaderRecord, settings_blob: &[u8]) -> Result<()> {
        let kdf = serde_json::to_string(&header.kdf).map_err(|_| Error::Storage)?;
        let tx = self.conn.transaction()?;
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM vault_header)", [], |r| {
            r.get(0)
        })?;
        if exists {
            return Err(Error::VaultExists);
        }
        tx.execute(
            "INSERT INTO vault_header (id, format_version, vault_id, kdf, wrapped_vault_key, created_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)",
            params![
                header.format_version,
                header.vault_id.to_string(),
                kdf,
                header.wrapped_vault_key,
                header.created_at
            ],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO settings (id, blob) VALUES (1, ?1)",
            params![settings_blob],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Replace the KDF descriptor and wrapped vault key (master password change).
    pub fn update_key_wrap(&mut self, kdf: &KdfParams, wrapped_vault_key: &[u8]) -> Result<()> {
        let kdf = serde_json::to_string(kdf).map_err(|_| Error::Storage)?;
        let n = self.conn.execute(
            "UPDATE vault_header SET kdf = ?1, wrapped_vault_key = ?2 WHERE id = 1",
            params![kdf, wrapped_vault_key],
        )?;
        if n == 1 {
            Ok(())
        } else {
            Err(Error::NoVault)
        }
    }

    /// All (id, overview blob) pairs. Rows with malformed IDs are reported as
    /// `Err` entries so one bad row cannot hide the rest.
    pub fn item_overviews(&self) -> Result<Vec<OverviewRow>> {
        let mut stmt = self.conn.prepare("SELECT id, overview FROM items")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(Error::from).and_then(|(id, blob)| {
                Uuid::parse_str(&id)
                    .map(|id| (id, blob))
                    .map_err(|_| Error::Corrupted)
            }));
        }
        Ok(out)
    }

    pub fn item_details(&self, id: &Uuid) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row(
                "SELECT details FROM items WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn upsert_item(&self, id: &Uuid, overview: &[u8], details: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items (id, overview, details) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET overview = excluded.overview, details = excluded.details",
            params![id.to_string(), overview, details],
        )?;
        Ok(())
    }

    /// Insert many items atomically: either all rows are written or none.
    pub fn insert_items(&mut self, rows: &[(Uuid, Vec<u8>, Vec<u8>)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt =
                tx.prepare("INSERT INTO items (id, overview, details) VALUES (?1, ?2, ?3)")?;
            for (id, overview, details) in rows {
                stmt.execute(params![id.to_string(), overview, details])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_item(&self, id: &Uuid) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM items WHERE id = ?1", params![id.to_string()])?
            == 1)
    }

    pub fn settings_blob(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row("SELECT blob FROM settings WHERE id = 1", [], |r| r.get(0))
            .optional()?)
    }

    pub fn write_settings(&self, blob: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (id, blob) VALUES (1, ?1)",
            params![blob],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> Result<()> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Tighten permissions on an existing file that is too open.
            let meta = std::fs::metadata(path).map_err(|_| Error::Storage)?;
            if meta.permissions().mode() & 0o077 != 0 {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|_| Error::Storage)?;
            }
            Ok(())
        }
        Err(_) => Err(Error::Storage),
    }
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> Result<()> {
    // On Windows the per-user AppData directory ACL provides isolation.
    use std::fs::OpenOptions;
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(Error::Storage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_set() {
        let s = Store::open_in_memory().unwrap();
        let v: i64 = s
            .conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        assert!(s.header().unwrap().is_none());
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.db");
        drop(Store::open(&path).unwrap());
        let c = Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
        drop(c);
        assert_eq!(Store::open(&path).err(), Some(Error::UnsupportedVersion));
    }

    #[test]
    fn non_sqlite_file_fails_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("garbage.db");
        std::fs::write(&path, vec![0x42u8; 8192]).unwrap();
        assert!(Store::open(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.db");
        drop(Store::open(&path).unwrap());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        drop(Store::open(&path).unwrap());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn item_crud() {
        let s = Store::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        s.upsert_item(&id, b"o1", b"d1").unwrap();
        s.upsert_item(&id, b"o2", b"d2").unwrap();
        assert_eq!(s.item_details(&id).unwrap().unwrap(), b"d2");
        let all = s.item_overviews().unwrap();
        assert_eq!(all.len(), 1);
        assert!(s.delete_item(&id).unwrap());
        assert!(!s.delete_item(&id).unwrap());
        assert!(s.item_details(&id).unwrap().is_none());
    }
}
