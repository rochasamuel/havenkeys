//! Taking this computer off its account ("remove this device"): the vault
//! file is set aside, the device revoked on the server, and the Secret Key
//! forgotten, leaving the app at first run.

use crate::account::off_main_thread;
use crate::state::{AppState, CmdError, CmdResult};
use crate::sync;
use havenkeys_core::account::NormalizedEmail;
use havenkeys_core::store::Store;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use crate::events::{Removed, KEYCHAIN_NOT_CLEARED, REMOVED_EVENT};

/// Normalized comparison, so case and surrounding spaces do not matter.
fn confirms(typed: &str, email: &str) -> bool {
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

/// Take this computer off the account and return it to first run.
///
/// The vault stays on the server. The local file is renamed, not deleted:
/// if the old server is gone, it is the only copy left, and it still opens
/// with the master password and the Emergency Kit.
///
/// Only while unlocked: the renderer is not trusted to gate this (CLAUDE.md
/// §4), and locked would still let a page-less caller delete the Secret Key
/// for an account nobody has proven they can open.
#[tauri::command]
pub async fn remove_device(app: AppHandle, confirmation: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let account = {
        let vault = state.vault()?;
        if !vault.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        vault.account()?.ok_or(havenkeys_core::Error::NoVault)?
    };
    if !confirms(&confirmation, &account.email) {
        return Err(
            havenkeys_core::Error::InvalidInput("type this account's email to confirm").into(),
        );
    }
    // Captured now, because locking drops the session. The revocation itself
    // waits until the file is set aside: revoking first and then failing the
    // rename would leave an intact vault on a device the server refuses
    // forever.
    let revoke = match (state.session(), sync::client(&state), state.device_id()) {
        (Ok(session), Ok(client), Ok(device_id)) => Some((session, client, device_id)),
        _ => None,
    };
    state.lock("user");
    state.client().forget_server();
    let _ = app.emit(sync::CONNECTIVITY_EVENT, false);

    let path = state.data_dir().join(crate::VAULT_FILE);
    let stamp = chrono_free_utc_stamp();
    {
        let mut vault = state.vault()?;
        let old = vault.replace_store(Store::open_in_memory().map_err(CmdError::from)?);
        drop(old); // closes the connection before the rename
        if set_aside(&path, &stamp).is_err() {
            // Nothing to undo below: the rename never happened (or was
            // rolled back), so `path` is still the original file. Reopen it
            // and report failure — the account is still on this device.
            let _ = vault.replace_store(Store::open(&path).map_err(CmdError::from)?);
            return Err(CmdError::file());
        }
    }

    // Best effort: the server may be gone, which may be why this is
    // happening. No vault guard is held here.
    if let Some((session, client, device_id)) = revoke {
        let _ = client.revoke_device(&session, device_id).await;
    }

    // Past this point the file is already renamed aside: this device must
    // end up fully removed even if a step below fails, rather than left
    // locked with a session, keychain entry and device.json that still name
    // an account whose file is gone. So every remaining step runs
    // regardless of earlier failures here, and the first error (if any) is
    // what's reported — after `forget` ran and the event fired.
    let mut first_error: Option<CmdError> = None;

    // Neither a reopen failure nor a poisoned vault mutex here should skip
    // `forget` or the event below, so this collects its error rather than
    // using `?`. Either way the vault stays on the in-memory store from
    // above, which the renderer reads as "no vault", i.e. first run —
    // acceptable per the brief for this rare case.
    let reopened = Store::open(&path)
        .map_err(CmdError::from)
        .and_then(|store| state.vault().map(|mut v| v.replace_store(store)));
    if let Err(e) = reopened {
        first_error = Some(e);
    }

    // Off the main thread: `forget` deletes from the OS keychain, which can
    // wait on D-Bus or a keyring prompt.
    let account_id = account.account_id;
    let forgot = off_main_thread(app.clone(), move |state| {
        let forgotten = state.client().device()?.forget(account_id);
        Ok((
            forgotten.keychain_cleared,
            forgotten.saved.map_err(|_| CmdError::file()),
        ))
    })
    .await;
    // Spec §6.3: the Secret Key must not stay behind. When the keychain
    // would not confirm the deletion, the rest of the removal still stands
    // and the user is told to delete the entry by hand.
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

    let _ = app.emit(
        REMOVED_EVENT,
        Removed {
            keychain_warning: (!keychain_cleared).then_some(KEYCHAIN_NOT_CLEARED),
        },
    );
    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
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
}
