//! Taking this device off its account ("remove this device"): the vault
//! file is set aside, the device revoked on the server, and the Secret Key
//! forgotten, leaving the app at first run.

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use havenkeys_core::account::NormalizedEmail;
use havenkeys_core::store::Store;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

impl HavenClient {
    /// The vault stays on the server. The local file is renamed, not
    /// deleted: if the old server is gone, it is the only copy left, and it
    /// still opens with the master password and the Emergency Kit.
    ///
    /// Only while unlocked: the shell is not trusted to gate this, and
    /// locked would let a caller delete the Secret Key for an account
    /// nobody has proven they can open.
    pub async fn remove_device(self: &Arc<Self>, confirmation: String) -> ClientResult<()> {
        let account = {
            let vault = self.vault()?;
            if !vault.is_unlocked() {
                return Err(havenkeys_core::Error::Locked.into());
            }
            vault.account()?.ok_or(havenkeys_core::Error::NoVault)?
        };
        if !confirms(&confirmation, &account.email) {
            return Err(havenkeys_core::Error::InvalidInput(
                "type this account's email to confirm",
            )
            .into());
        }
        // Captured now, because locking drops the session. The revocation
        // itself waits until the file is set aside: revoking first and then
        // failing the rename would leave an intact vault on a device the
        // server refuses forever.
        let revoke = match (self.session(), self.server(), self.device_id()) {
            (Ok(session), Ok(server), Ok(device_id)) => Some((session, server, device_id)),
            _ => None,
        };
        let was_online = self.is_online();
        self.lock("user");
        self.forget_server();
        if was_online {
            self.events.connectivity(false);
        }

        let path = self.config.vault_path.clone();
        let stamp = chrono_free_utc_stamp();
        {
            let mut vault = self.vault()?;
            let old = vault.replace_store(Store::open_in_memory()?);
            drop(old); // closes the connection before the rename
            if set_aside(&path, &stamp).is_err() {
                // Nothing to undo below: the rename never happened (or was
                // rolled back), so `path` is still the original file. Reopen
                // it and report failure: the account is still on this device.
                let _ = vault.replace_store(Store::open(&path)?);
                return Err(ClientError::file());
            }
        }

        // Best effort: the server may be gone, which may be why this is
        // happening. No vault guard is held here.
        if let Some((session, server, device_id)) = revoke {
            let _ = server.revoke_device(&session, device_id).await;
        }

        // Past this point the file is already renamed aside: this device must
        // end up fully removed even if a step below fails, rather than left
        // locked with a session, keychain entry and device.json that still
        // name an account whose file is gone. So every remaining step runs
        // regardless of earlier failures, and the first error (if any) is
        // what's reported, after `forget` ran and the shell was told.
        let mut first_error: Option<ClientError> = None;

        // Neither a reopen failure nor a poisoned vault mutex here should
        // skip `forget` or the event below, so this collects its error
        // rather than using `?`. Either way the vault stays on the in-memory
        // store from above, which reads as "no vault", i.e. first run.
        let reopened = Store::open(&path)
            .map_err(ClientError::from)
            .and_then(|store| self.vault().map(|mut v| v.replace_store(store)));
        if let Err(e) = reopened {
            first_error = Some(e);
        }

        // Off the async threads: `forget` deletes from the platform store,
        // which can wait on a prompt.
        let client = Arc::clone(self);
        let account_id = account.account_id;
        let forgot = tokio::task::spawn_blocking(move || {
            let forgotten = client.device()?.forget(account_id);
            Ok::<_, ClientError>((
                forgotten.keychain_cleared,
                forgotten.saved.map_err(|_| ClientError::file()),
            ))
        })
        .await
        .map_err(|_| ClientError::internal())
        .and_then(|r| r);
        // Spec §6.3: the Secret Key must not stay behind. When the keychain
        // would not confirm the deletion, the rest of the removal still
        // stands and the user is told to delete the entry by hand.
        let keychain_cleared = match forgot {
            Ok((cleared, saved)) => {
                if let Err(e) = saved {
                    first_error.get_or_insert(e);
                }
                cleared
            }
            Err(e) => {
                first_error.get_or_insert(e);
                false
            }
        };

        self.events.removed(keychain_cleared);
        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// Normalized comparison, so case and surrounding spaces do not matter.
pub(crate) fn confirms(typed: &str, email: &str) -> bool {
    match (NormalizedEmail::parse(typed), NormalizedEmail::parse(email)) {
        (Ok(a), Ok(b)) => a.as_str() == b.as_str(),
        _ => false,
    }
}

/// Rename the vault file (and SQLite's journal files, if any) aside. Never
/// overwrites: an earlier removal's file is somebody's last copy too.
///
/// The journal files move first and the main file last, so any failure —
/// including the main file's own rename — leaves the vault still openable
/// at `path`: anything already moved is put back (best effort; the store
/// does not normally use WAL, so this path is rarely exercised).
fn set_aside(path: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    let target = PathBuf::from(format!("{}.removed-{stamp}", path.display()));
    if target.exists() {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    }
    let mut moved: Vec<&str> = Vec::new();
    for suffix in ["-wal", "-shm", "-journal"] {
        let from = PathBuf::from(format!("{}{suffix}", path.display()));
        if !from.exists() {
            continue;
        }
        let to = format!("{}{suffix}", target.display());
        if let Err(e) = std::fs::rename(&from, &to) {
            restore_journal_files(path, &target, &moved);
            return Err(e);
        }
        moved.push(suffix);
    }
    if let Err(e) = std::fs::rename(path, &target) {
        restore_journal_files(path, &target, &moved);
        return Err(e);
    }
    Ok(target)
}

/// Move journal files already renamed to `target` back next to `path`,
/// after a later step in `set_aside` failed. Best effort: a failure here
/// just leaves a `.removed-<stamp>-wal`/`-shm`/`-journal` orphan, which is
/// harmless (the main file itself is still at `path` either way).
fn restore_journal_files(path: &Path, target: &Path, moved: &[&str]) {
    for suffix in moved {
        let from = format!("{}{suffix}", target.display());
        let to = PathBuf::from(format!("{}{suffix}", path.display()));
        let _ = std::fs::rename(from, to);
    }
}

/// `SystemTime::now()` as `YYYYMMDDTHHMMSSZ`, without pulling in `time` or
/// `chrono` for one timestamp. Civil-date arithmetic is Howard Hinnant's
/// `civil_from_days`: http://howardhinnant.github.io/date_algorithms.html
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

fn stamp_for_secs(secs: i64) -> String {
    let (days, secs_of_day) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

fn chrono_free_utc_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    stamp_for_secs(secs)
}

#[cfg(test)]
mod tests {
    #[test]
    fn set_aside_renames_the_vault_and_its_journal_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        for suffix in ["", "-wal", "-shm"] {
            std::fs::write(format!("{}{suffix}", path.display()), b"x").unwrap();
        }
        let moved = super::set_aside(&path, "20260923T120000Z").unwrap();
        assert!(!path.exists());
        assert!(moved.ends_with("vault.sqlite3.removed-20260923T120000Z"));
        assert!(moved.exists());
        assert!(dir
            .path()
            .join("vault.sqlite3.removed-20260923T120000Z-wal")
            .exists());
    }

    #[test]
    fn set_aside_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        std::fs::write(&path, b"x").unwrap();
        std::fs::write(dir.path().join("vault.sqlite3.removed-S"), b"older").unwrap();
        assert!(super::set_aside(&path, "S").is_err());
        assert!(path.exists());
    }

    /// The new order (journal files, then the main file) means the guard
    /// against overwriting an earlier removal must still fire before
    /// anything moves — including the journal files — not just the main
    /// file. `set_aside_never_overwrites` above already covers the case
    /// with no journal files; this is the same failure with `-wal`/`-shm`
    /// present, confirming they are left exactly where they were.
    #[test]
    fn a_pre_existing_target_leaves_the_journal_files_untouched_too() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        std::fs::write(&path, b"x").unwrap();
        std::fs::write(format!("{}-wal", path.display()), b"wal").unwrap();
        std::fs::write(format!("{}-shm", path.display()), b"shm").unwrap();
        std::fs::write(dir.path().join("vault.sqlite3.removed-S"), b"older").unwrap();

        assert!(super::set_aside(&path, "S").is_err());

        assert!(path.exists());
        assert!(dir.path().join("vault.sqlite3-wal").exists());
        assert!(dir.path().join("vault.sqlite3-shm").exists());
        assert!(!dir.path().join("vault.sqlite3.removed-S-wal").exists());
        assert!(!dir.path().join("vault.sqlite3.removed-S-shm").exists());
    }

    /// A failure partway through the journal-file renames must put back
    /// anything already moved, so a later failure at the main file's own
    /// rename (the case this ordering exists for) never has to reconcile a
    /// half-moved journal on top of it. The "-shm" rename is made to fail
    /// by blocking its destination with a non-empty directory, which a
    /// plain-file rename cannot replace.
    #[test]
    fn a_failed_journal_rename_puts_earlier_ones_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        std::fs::write(&path, b"x").unwrap();
        std::fs::write(format!("{}-wal", path.display()), b"wal").unwrap();
        std::fs::write(format!("{}-shm", path.display()), b"shm").unwrap();
        let blocked = dir.path().join("vault.sqlite3.removed-S-shm");
        std::fs::create_dir(&blocked).unwrap();
        std::fs::write(blocked.join("keep"), b"y").unwrap();

        assert!(super::set_aside(&path, "S").is_err());

        // The vault itself was never touched (the main rename is last), and
        // the "-wal" file that did move is back where it started.
        assert!(path.exists());
        assert!(dir.path().join("vault.sqlite3-wal").exists());
        assert!(!dir.path().join("vault.sqlite3.removed-S-wal").exists());
        // The "-shm" source is untouched too: its own rename never
        // completed.
        assert!(dir.path().join("vault.sqlite3-shm").exists());
    }

    #[test]
    fn the_confirmation_must_be_the_account_email() {
        assert!(super::confirms("User@Example.com ", "user@example.com"));
        assert!(!super::confirms("someone@example.com", "user@example.com"));
        assert!(!super::confirms("", "user@example.com"));
    }

    #[test]
    fn civil_from_days_matches_known_dates() {
        // day 0 is the epoch itself; day 20354 is 2025-09-23 (verified by
        // hand against Howard Hinnant's algorithm).
        assert_eq!(super::civil_from_days(0), (1970, 1, 1));
        assert_eq!(super::civil_from_days(20354), (2025, 9, 23));
    }

    #[test]
    fn known_timestamps_format_as_expected() {
        // 0 is the epoch; 1_758_628_800 is 2025-09-23T12:00:00Z (checked
        // against `date -u -d @1758628800`).
        assert_eq!(super::stamp_for_secs(0), "19700101T000000Z");
        assert_eq!(super::stamp_for_secs(1_758_628_800), "20250923T120000Z");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn removal_without_a_session_still_completes() {
        use crate::client::tests::client_in;
        use havenkeys_core::account::{AccountRef, NormalizedEmail};
        use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
        use havenkeys_core::store::{AccountRecord, Store};
        use havenkeys_core::vault::prepare_new_account_vault;
        use havenkeys_core::SecretString;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        let (client, events) = client_in(dir.path());
        client
            .vault()
            .unwrap()
            .replace_store(Store::open(&path).unwrap());
        let account = AccountRef::new(
            uuid::Uuid::from_u128(3),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let made = prepare_new_account_vault(
            &SecretString::from("correct horse battery staple"),
            &account,
            KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap(),
            1_700_000_000_000,
        )
        .unwrap();
        client
            .vault()
            .unwrap()
            .create_account_vault(
                made.prepared,
                &AccountRecord {
                    account_id: account.id,
                    email: "user@example.com".into(),
                    server_url: "http://127.0.0.1:9".into(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        client
            .device()
            .unwrap()
            .set_secret_key(account.id, &made.secret_key)
            .unwrap();
        let old_id = client.device_id().unwrap();

        client
            .remove_device(" User@Example.com ".into())
            .await
            .unwrap();

        assert!(client.vault().unwrap().account().unwrap().is_none());
        assert!(client.device().unwrap().secret_key(account.id).is_none());
        assert_ne!(client.device_id().unwrap(), old_id);
        assert!(events.seen().contains(&"removed:true".to_string()));
        let set_aside = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("vault.sqlite3.removed-")
            });
        assert!(set_aside);
    }

    #[tokio::test]
    async fn a_locked_vault_cannot_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = crate::client::tests::client_in(dir.path());
        let err = client
            .remove_device("user@example.com".into())
            .await
            .unwrap_err();
        assert_eq!(err.code, "locked");
    }
}
