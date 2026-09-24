//! SQLite persistence. Stores only the plaintext header needed to unlock and
//! opaque encrypted blobs. Knows nothing about keys.

use crate::account::{AccountRef, NormalizedEmail};
use crate::crypto::kdf::KdfParams;
use crate::error::{Error, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

pub const SCHEMA_VERSION: i64 = 5;

const SCHEMA: &str = "
CREATE TABLE vault_header (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    format_version    INTEGER NOT NULL,
    vault_id          TEXT    NOT NULL,
    kdf               TEXT    NOT NULL,
    wrapped_vault_key BLOB    NOT NULL,
    created_at        INTEGER NOT NULL,
    key_scheme        INTEGER NOT NULL,
    header_revision   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE items (
    id       TEXT PRIMARY KEY NOT NULL,
    overview BLOB NOT NULL,
    details  BLOB NOT NULL,
    revision INTEGER NOT NULL
);
CREATE TABLE account (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    account_id     TEXT    NOT NULL,
    email          TEXT    NOT NULL,
    server_url     TEXT    NOT NULL,
    server_cursor  INTEGER NOT NULL DEFAULT 0,
    max_header_rev INTEGER NOT NULL DEFAULT 0,
    last_synced_at INTEGER
);
CREATE TABLE settings (
    id   INTEGER PRIMARY KEY CHECK (id = 1),
    blob BLOB NOT NULL
);
CREATE TABLE unreadable_items (
    id       TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL
);
";

/// How the key-encryption key is derived. One scheme: the master password,
/// the device's Secret Key and the account (docs/crypto.md). Kept as an enum,
/// rather than a unit struct, so the header's serde form stays forward-
/// compatible if a new scheme is ever added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyScheme {
    /// Argon2id(master password) combined with the device's Secret Key,
    /// bound to the account (email + account ID). Vaults that sync to a
    /// server.
    AccountBound,
}

impl KeyScheme {
    /// Kept as a method so call sites do not match on the variant; every
    /// scheme this build understands needs the Secret Key.
    pub fn uses_secret_key(self) -> bool {
        true
    }

    fn to_db(self) -> i64 {
        match self {
            KeyScheme::AccountBound => 3,
        }
    }

    fn from_db(v: i64) -> Result<Self> {
        match v {
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
    /// so devices sharing an account can tell, via the server, which header
    /// is newest.
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
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            SCHEMA_VERSION => {}
            // Vaults from before accounts are not migrated (spec §8.2).
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
    /// (master password change, Secret Key set-up, or a newer header adopted
    /// from the account's server).
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

    pub fn upsert_item(
        &mut self,
        id: &Uuid,
        overview: &[u8],
        details: &[u8],
        revision: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items (id, overview, details, revision) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                           details  = excluded.details,
                                           revision = excluded.revision",
            params![id.to_string(), overview, details, revision],
        )?;
        Ok(())
    }

    /// Everything one pull changes, in one transaction: upserts, deletions,
    /// the unreadable-item bookkeeping and (for a pull, not a refetch) the
    /// cursor. A crash can no longer leave the cursor past rows that were
    /// never written. Returns how many rows were deleted.
    pub fn apply_pull(
        &mut self,
        rows: &[(Uuid, Vec<u8>, Vec<u8>, i64)],
        deletions: &[Uuid],
        unreadable: &[(Uuid, i64)],
        cursor: Option<(i64, i64)>,
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut deleted = 0;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO items (id, overview, details, revision) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                               details  = excluded.details,
                                               revision = excluded.revision",
            )?;
            let mut clear = tx.prepare("DELETE FROM unreadable_items WHERE id = ?1")?;
            let mut delete = tx.prepare("DELETE FROM items WHERE id = ?1")?;
            let mut record = tx.prepare(
                "INSERT INTO unreadable_items (id, revision) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET revision = excluded.revision",
            )?;
            for (id, overview, details, revision) in rows {
                upsert.execute(params![id.to_string(), overview, details, revision])?;
                clear.execute(params![id.to_string()])?;
            }
            for id in deletions {
                deleted += delete.execute(params![id.to_string()])?;
                clear.execute(params![id.to_string()])?;
            }
            for (id, revision) in unreadable {
                record.execute(params![id.to_string(), revision])?;
            }
        }
        if let Some((cursor, synced_at)) = cursor {
            tx.execute(
                "UPDATE account SET server_cursor = ?1, last_synced_at = ?2 WHERE id = 1",
                params![cursor, synced_at],
            )?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// IDs of items pulled from the server that did not decrypt under this
    /// vault's data key, ordered for a stable, deterministic retry batch.
    pub fn unreadable_ids(&self) -> Result<Vec<Uuid>> {
        Ok(self
            .unreadable_revisions()?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    /// `(id, revision)` for every item pulled from the server that did not
    /// decrypt, ordered by id: the revision is what a retry must not regress
    /// behind (`apply_refetched`). Rows with a malformed ID are skipped
    /// rather than surfaced as an error: this table only ever holds IDs this
    /// device wrote itself.
    pub fn unreadable_revisions(&self) -> Result<Vec<(Uuid, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, revision FROM unreadable_items ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, revision) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, revision));
            }
        }
        Ok(out)
    }

    pub fn unreadable_count(&self) -> Result<usize> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM unreadable_items", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Ask for the whole vault again: cursor back to zero, and forget what
    /// was unreadable so far — the next full pull re-derives that list from
    /// scratch. One transaction.
    pub fn reset_cursor(&mut self, synced_at: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE account SET server_cursor = 0, last_synced_at = ?1 WHERE id = 1",
            params![synced_at],
        )?;
        tx.execute("DELETE FROM unreadable_items", [])?;
        tx.commit()?;
        Ok(())
    }

    /// The server revision this device last stored for an item.
    pub fn item_revision(&self, id: &Uuid) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT revision FROM items WHERE id = ?1",
                params![id.to_string()],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Remove an item. The server holds the tombstone; this device does not.
    pub fn delete_item(&mut self, id: &Uuid) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM items WHERE id = ?1", params![id.to_string()])?;
        Ok(n == 1)
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

    /// Link this vault to an account, or refresh the email and server
    /// address of the account it already belongs to.
    ///
    /// Re-pointing a vault at a *different* account is refused. The clause
    /// below deliberately keeps `server_cursor` and `max_header_rev` — right
    /// for signing in again to the same account, ruinous for a different
    /// one: the stale cursor would make the first pull skip everything
    /// before it, and the stale floor would fail every header the new
    /// account serves. A future "re-point" flow needs its own path that
    /// resets both, not this one.
    pub fn set_account(&mut self, rec: &AccountRecord) -> Result<()> {
        if let Some(current) = self.account()? {
            if current.account_id != rec.account_id {
                return Err(Error::InvalidInput(
                    "this vault is linked to a different account",
                ));
            }
        }
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
        s.upsert_item(&id, b"o1", b"d1", 1).unwrap();
        s.upsert_item(&id, b"o2", b"d2", 2).unwrap();
        assert_eq!(s.item_details(&id).unwrap().unwrap(), b"d2");
        assert_eq!(s.item_revision(&id).unwrap(), Some(2));
        let all = s.item_overviews().unwrap();
        assert_eq!(all.len(), 1);
        assert!(s.delete_item(&id).unwrap());
        assert!(!s.delete_item(&id).unwrap());
        assert!(s.item_details(&id).unwrap().is_none());
        // No tombstone survives the delete: the server holds those.
        assert_eq!(s.item_revision(&id).unwrap(), None);
    }

    #[test]
    fn a_database_from_an_older_build_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.sqlite3");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 3i64).unwrap();
        }
        let err = Store::open(&path).err().unwrap();
        assert_eq!(err.code(), "unsupported_version");
    }
}
