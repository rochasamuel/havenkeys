//! SQLite persistence. Stores only the plaintext header needed to unlock and
//! opaque encrypted blobs. Knows nothing about keys.

use crate::account::{AccountRef, NormalizedEmail};
use crate::crypto::kdf::KdfParams;
use crate::error::{Error, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

pub const SCHEMA_VERSION: i64 = 3;

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

/// Schema 1 → 2: key scheme and header revision in the header (Secret Key,
/// sync), and tombstones so deletions reach other devices.
const MIGRATE_1_TO_2: &str = "
ALTER TABLE vault_header ADD COLUMN key_scheme INTEGER NOT NULL DEFAULT 1;
ALTER TABLE vault_header ADD COLUMN header_revision INTEGER NOT NULL DEFAULT 0;
CREATE TABLE tombstones (
    id         TEXT PRIMARY KEY NOT NULL,
    deleted_at INTEGER NOT NULL
);
";

/// Schema 2 → 3: server sync. `dirty` marks rows changed locally since the
/// last confirmed push (existing rows start dirty, so a vault joining an
/// account uploads itself once), and `account` holds the identity, the
/// server cursor and the header-rollback guard.
const MIGRATE_2_TO_3: &str = "
ALTER TABLE items ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tombstones ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
CREATE TABLE account (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    account_id     TEXT    NOT NULL,
    email          TEXT    NOT NULL,
    server_url     TEXT    NOT NULL,
    server_cursor  INTEGER NOT NULL DEFAULT 0,
    max_header_rev INTEGER NOT NULL DEFAULT 0,
    last_synced_at INTEGER
);
";

/// How the key-encryption key is derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyScheme {
    /// Argon2id(master password) only. Vaults created before the Secret Key.
    PasswordOnly,
    /// Argon2id(master password) combined with the device's Secret Key.
    PasswordAndSecretKey,
    /// Argon2id(master password) combined with the device's Secret Key,
    /// bound to the account (email + account ID). Vaults that sync to a
    /// server.
    AccountBound,
}

impl KeyScheme {
    fn to_db(self) -> i64 {
        match self {
            KeyScheme::PasswordOnly => 1,
            KeyScheme::PasswordAndSecretKey => 2,
            KeyScheme::AccountBound => 3,
        }
    }

    fn from_db(v: i64) -> Result<Self> {
        match v {
            1 => Ok(KeyScheme::PasswordOnly),
            2 => Ok(KeyScheme::PasswordAndSecretKey),
            3 => Ok(KeyScheme::AccountBound),
            _ => Err(Error::UnsupportedVersion),
        }
    }
}

/// Plaintext vault header.
#[derive(Clone, Debug)]
pub struct HeaderRecord {
    pub format_version: u32,
    pub vault_id: Uuid,
    pub kdf: KdfParams,
    pub wrapped_vault_key: Vec<u8>,
    pub created_at: i64,
    pub key_scheme: KeyScheme,
    /// Bumped on every re-wrap (master password change, Secret Key set-up),
    /// so devices sharing a sync folder can tell which header is newest.
    pub revision: u64,
}

/// The account this vault belongs to, and where its sync stands.
#[derive(Clone, Debug)]
pub struct AccountRecord {
    pub account_id: Uuid,
    /// As the user typed it, for display. Normalize on use, never on store.
    pub email: String,
    pub server_url: String,
    /// Highest server revision this device has pulled.
    pub server_cursor: i64,
    /// Highest `header_revision` ever accepted. Never goes down: that is the
    /// rollback guard from the design doc §8.4.
    pub max_header_rev: i64,
    pub last_synced_at: Option<i64>,
}

impl AccountRecord {
    /// The identity key derivation binds to. The stored email is normalized
    /// here rather than on write, so a row written by an older build still
    /// derives the same key.
    pub fn to_ref(&self) -> Result<AccountRef> {
        Ok(AccountRef::new(
            self.account_id,
            NormalizedEmail::parse(&self.email)?,
        ))
    }
}

/// One full `items` row, as stored (encrypted).
pub type ItemRow = (Uuid, Vec<u8>, Vec<u8>);

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
                tx.execute_batch(MIGRATE_1_TO_2)?;
                tx.execute_batch(MIGRATE_2_TO_3)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            1 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(MIGRATE_1_TO_2)?;
                tx.execute_batch(MIGRATE_2_TO_3)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            2 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(MIGRATE_2_TO_3)?;
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
                "SELECT format_version, vault_id, kdf, wrapped_vault_key, created_at,
                        key_scheme, header_revision
                 FROM vault_header WHERE id = 1",
                [],
                |r| {
                    Ok((
                        (
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ),
                        (
                            r.get::<_, Vec<u8>>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, i64>(5)?,
                            r.get::<_, i64>(6)?,
                        ),
                    ))
                },
            )
            .optional()?;
        let Some((
            (format_version, vault_id, kdf),
            (wrapped_vault_key, created_at, key_scheme, revision),
        )) = row
        else {
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
            key_scheme: KeyScheme::from_db(key_scheme)?,
            revision: u64::try_from(revision).map_err(|_| Error::Corrupted)?,
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
            "INSERT INTO vault_header (id, format_version, vault_id, kdf, wrapped_vault_key,
                                       created_at, key_scheme, header_revision)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                header.format_version,
                header.vault_id.to_string(),
                kdf,
                header.wrapped_vault_key,
                header.created_at,
                header.key_scheme.to_db(),
                i64::try_from(header.revision).map_err(|_| Error::Storage)?,
            ],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO settings (id, blob) VALUES (1, ?1)",
            params![settings_blob],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Replace the KDF descriptor, wrapped vault key, key scheme and revision
    /// (master password change, Secret Key set-up, or a newer header from
    /// the sync folder).
    pub fn update_key_wrap(
        &mut self,
        kdf: &KdfParams,
        wrapped_vault_key: &[u8],
        key_scheme: KeyScheme,
        revision: u64,
    ) -> Result<()> {
        let kdf = serde_json::to_string(kdf).map_err(|_| Error::Storage)?;
        let n = self.conn.execute(
            "UPDATE vault_header SET kdf = ?1, wrapped_vault_key = ?2, key_scheme = ?3,
                                     header_revision = ?4
             WHERE id = 1",
            params![
                kdf,
                wrapped_vault_key,
                key_scheme.to_db(),
                i64::try_from(revision).map_err(|_| Error::Storage)?
            ],
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
            "INSERT INTO items (id, overview, details, dirty) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                           details = excluded.details,
                                           dirty = 1",
            params![id.to_string(), overview, details],
        )?;
        Ok(())
    }

    /// Insert many items atomically: either all rows are written or none.
    pub fn insert_items(&mut self, rows: &[(Uuid, Vec<u8>, Vec<u8>)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO items (id, overview, details, dirty) VALUES (?1, ?2, ?3, 1)",
            )?;
            for (id, overview, details) in rows {
                stmt.execute(params![id.to_string(), overview, details])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Delete an item and record a tombstone, atomically.
    pub fn delete_item(&mut self, id: &Uuid, deleted_at: i64) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let n = tx.execute("DELETE FROM items WHERE id = ?1", params![id.to_string()])?;
        tx.execute(
            "INSERT INTO tombstones (id, deleted_at, dirty) VALUES (?1, ?2, 1)
             ON CONFLICT(id) DO UPDATE SET deleted_at = max(deleted_at, excluded.deleted_at),
                                           dirty = 1",
            params![id.to_string(), deleted_at],
        )?;
        tx.commit()?;
        Ok(n == 1)
    }

    /// Every item row, for a sync snapshot. Malformed IDs are skipped.
    pub fn item_rows(&self) -> Result<Vec<ItemRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, overview, details FROM items")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, ov, det) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, ov, det));
            }
        }
        Ok(out)
    }

    /// `(id, deleted_at)` of every deleted item.
    pub fn tombstones(&self) -> Result<Vec<(Uuid, i64)>> {
        let mut stmt = self.conn.prepare("SELECT id, deleted_at FROM tombstones")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, at) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, at));
            }
        }
        Ok(out)
    }

    pub fn account(&self) -> Result<Option<AccountRecord>> {
        self.conn
            .query_row(
                "SELECT account_id, email, server_url, server_cursor, max_header_rev, last_synced_at
                 FROM account WHERE id = 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .optional()?
            .map(|(id, email, server_url, server_cursor, max_header_rev, last_synced_at)| {
                Ok(AccountRecord {
                    account_id: Uuid::parse_str(&id).map_err(|_| Error::Corrupted)?,
                    email,
                    server_url,
                    server_cursor,
                    max_header_rev,
                    last_synced_at,
                })
            })
            .transpose()
    }

    pub fn set_account(&mut self, rec: &AccountRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO account
               (id, account_id, email, server_url, server_cursor, max_header_rev, last_synced_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id,
               email = excluded.email,
               server_url = excluded.server_url",
            params![
                rec.account_id.to_string(),
                rec.email,
                rec.server_url,
                rec.server_cursor,
                rec.max_header_rev,
                rec.last_synced_at,
            ],
        )?;
        Ok(())
    }

    pub fn set_cursor(&mut self, cursor: i64, synced_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE account SET server_cursor = ?1, last_synced_at = ?2 WHERE id = 1",
            params![cursor, synced_at],
        )?;
        Ok(())
    }

    /// Raise the rollback guard. An older header never lowers it.
    pub fn raise_max_header_rev(&mut self, rev: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE account SET max_header_rev = max(max_header_rev, ?1) WHERE id = 1",
            params![rev],
        )?;
        Ok(())
    }

    /// Items changed locally since the last confirmed push.
    pub fn dirty_rows(&self) -> Result<Vec<ItemRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, overview, details FROM items WHERE dirty = 1")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, ov, det) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, ov, det));
            }
        }
        Ok(out)
    }

    /// Deletions not yet pushed.
    pub fn dirty_tombstones(&self) -> Result<Vec<(Uuid, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, deleted_at FROM tombstones WHERE dirty = 1")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, at) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, at));
            }
        }
        Ok(out)
    }

    /// Mark pushed rows as clean, in one transaction — but only the exact
    /// versions that were pushed. Matching on content as well as ID means a
    /// row edited or deleted again after it was read for the push (and
    /// before the server's ack arrived) keeps its dirty flag: the clear
    /// simply misses it, and the newer version stays pending for the next
    /// push. Clearing by ID alone would silently drop that newer version,
    /// since a deletion reuses the same ID and would otherwise be cleared by
    /// an ack meant for the row it replaced.
    pub fn clear_dirty(&mut self, items: &[ItemRow], tombstones: &[(Uuid, i64)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (id, overview, details) in items {
            tx.execute(
                "UPDATE items SET dirty = 0 WHERE id = ?1 AND overview = ?2 AND details = ?3",
                params![id.to_string(), overview, details],
            )?;
        }
        for (id, deleted_at) in tombstones {
            tx.execute(
                "UPDATE tombstones SET dirty = 0 WHERE id = ?1 AND deleted_at = ?2",
                params![id.to_string(), deleted_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Apply a sync merge in one transaction: upsert rows, delete items that
    /// were deleted elsewhere, and set tombstones.
    pub fn apply_merge(
        &mut self,
        upserts: &[ItemRow],
        tombstones: &[(Uuid, i64)],
        resurrected: &[Uuid],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (id, ov, det) in upserts {
            tx.execute(
                "INSERT INTO items (id, overview, details, dirty) VALUES (?1, ?2, ?3, 0)
                 ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                               details = excluded.details,
                                               dirty = 0",
                params![id.to_string(), ov, det],
            )?;
        }
        for id in resurrected {
            tx.execute(
                "DELETE FROM tombstones WHERE id = ?1",
                params![id.to_string()],
            )?;
        }
        for (id, at) in tombstones {
            tx.execute("DELETE FROM items WHERE id = ?1", params![id.to_string()])?;
            tx.execute(
                "INSERT INTO tombstones (id, deleted_at, dirty) VALUES (?1, ?2, 0)
                 ON CONFLICT(id) DO UPDATE SET deleted_at = max(deleted_at, excluded.deleted_at),
                                               dirty = 0",
                params![id.to_string(), at],
            )?;
        }
        tx.commit()?;
        Ok(())
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
        let mut s = Store::open_in_memory().unwrap();
        let id = Uuid::new_v4();
        s.upsert_item(&id, b"o1", b"d1").unwrap();
        s.upsert_item(&id, b"o2", b"d2").unwrap();
        assert_eq!(s.item_details(&id).unwrap().unwrap(), b"d2");
        let all = s.item_overviews().unwrap();
        assert_eq!(all.len(), 1);
        assert!(s.delete_item(&id, 5).unwrap());
        assert!(!s.delete_item(&id, 3).unwrap());
        assert!(s.item_details(&id).unwrap().is_none());
        // The tombstone keeps the latest deletion time.
        assert_eq!(s.tombstones().unwrap(), vec![(id, 5)]);
    }

    /// A vault created before the Secret Key (schema 1) opens, migrates, and
    /// reads as a password-only vault.
    #[test]
    fn migrates_schema_1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        let c = Connection::open(&path).unwrap();
        c.execute_batch(SCHEMA).unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        let kdf = serde_json::to_string(&KdfParams::generate().unwrap()).unwrap();
        c.execute(
            "INSERT INTO vault_header (id, format_version, vault_id, kdf, wrapped_vault_key, created_at)
             VALUES (1, 1, ?1, ?2, x'00', 7)",
            params![Uuid::nil().to_string(), kdf],
        )
        .unwrap();
        c.execute(
            "INSERT INTO items (id, overview, details) VALUES (?1, x'01', x'02')",
            params![Uuid::nil().to_string()],
        )
        .unwrap();
        drop(c);

        let s = Store::open(&path).unwrap();
        let h = s.header().unwrap().unwrap();
        assert_eq!(h.key_scheme, KeyScheme::PasswordOnly);
        assert_eq!(h.revision, 0);
        assert_eq!(h.created_at, 7);
        assert_eq!(s.item_rows().unwrap().len(), 1);
        assert!(s.tombstones().unwrap().is_empty());
        drop(s);
        // Opening again is a no-op.
        assert!(Store::open(&path).is_ok());
        let c = Connection::open(&path).unwrap();
        let v: i64 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }
}
