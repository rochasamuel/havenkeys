# havenkeys-client Extraction (Android Step 0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the desktop's account, session and sync logic out of the Tauri shell into a new `crates/havenkeys-client`, so the Android app can reuse it, with the desktop behaving exactly as before.

**Architecture:** `HavenClient` owns the vault handle, the device record (ID + Secret Key), the server session and the HTTP client, and implements activation, sign-in, unlock, sync, writes, the master password change, devices and device removal. Platform differences go through two traits: `KeyStore` (where the Secret Key lives — already a trait today) and `ClientEvents` (what the shell is told: unlocked, locked, online/offline, synced…). The desktop's `AppState` holds an `Arc<HavenClient>` plus its desktop-only parts (bridge, auto-lock, clipboard, QR scan slot), and its Tauri commands become thin wrappers.

**Tech Stack:** Rust 1.88, tokio (rt, time), havenkeys-core, havenkeys-sync-client (reqwest + rustls), Tauri 2 (desktop only).

**Spec:** `docs/superpowers/specs/2026-10-01-android-app-design.md` §4.1 (Step 0).

## Global Constraints

- `havenkeys-client` has no Tauri dependency and no `tauri::` import anywhere.
- `havenkeys-core` stays free of any network dependency.
- Error `code` strings and messages seen by the desktop UI do not change (the React app matches on them).
- Desktop event names and payloads (`vault://locked`, `vault://connectivity`, `vault://signed-out`, `vault://synced`, `vault://items-changed`, `vault://removed`) do not change.
- `device.json` keeps its name, location and format; the keychain service name stays `app.havenkeys`.
- No secret in any log, error, `Debug` output or panic message (CLAUDE.md §39–40).
- No `MutexGuard` is held across an `.await`.
- Lock ordering: the vault mutex before the device mutex and before the connectivity mutex.
- `#![forbid(unsafe_code)]` and `#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]` in the new crate.
- Code style: clear names; comments only where the reason is not obvious; keep the existing comments that explain *why* when moving code.
- Commits carry no Co-Authored-By trailer.

## Review Focus

1. **A lock that lands while `connect` is signing in** — expected: the device stays offline and locked. Today `connect` sets the session after `login` returns without checking the lock. Pinned by Task 4 Step 1 (`connect_on_a_locked_vault_stays_offline`) and fixed in Task 4.
2. **The UI's error codes after the move** — expected: every code the React app knows (`sign_in_failed`, `keychain_unavailable`, `signed_out`, `password_change_unknown`, `password_changed_elsewhere`, `rate_limited`, `invalid_server_url`, `sync_failed`, `file`, `internal`) maps exactly as before. Pinned by Task 1's error table test.
3. **An existing desktop install starting on the new build** — expected: its `device.json` and keychain entry are found and unlock works without the Emergency Kit. Pinned by Task 2's `an_existing_device_json_is_read_unchanged`.
4. **`ClientEvents::unlocked` is called under the vault guard** — expected: no deadlock, because the desktop implementation only touches the lock manager and the bridge. Pinned by Task 7's round trip (unlock completes while a `RecordingEvents` probes the vault with `try_lock`).
5. **Removing a device while offline** — expected: the vault file is set aside, the Secret Key forgotten, the shell told `removed`, and the call succeeds even though the server revoke could not be sent. Pinned by Task 6's `removal_without_a_session_still_completes`.

---

## File Structure

```text
crates/havenkeys-client/
  Cargo.toml
  src/
    lib.rs        re-exports, now_ms()
    error.rs      ClientError (was the desktop's CmdError), ClientResult
    events.rs     ClientEvents trait
    key_store.rs  KeyStore, TimedKeyStore, Storage, StoreError, test fakes (from secret_store.rs)
    device.rs     Device: device ID + Secret Key record (moved as-is)
    client.rs     HavenClient: vault handle, session, server client cache, lock
    sync.rs       connect, sync_now, push, push_batches, ensure_identity
    account.rs    status, activate, sign_in, unlock, change_master_password, devices, Account item
    removal.rs    remove_device and set_aside
  tests/
    round_trip.rs two clients against a real server (Postgres)

apps/desktop/src-tauri/src/
  state.rs        AppState now wraps Arc<HavenClient>; CmdError/CmdResult become aliases
  events.rs       NEW: DesktopEvents implements ClientEvents with app.emit
  secret_store.rs keeps only OsKeyStore (implements havenkeys_client::key_store::KeyStore)
  sync.rs         keeps only event names and bridge_error
  account.rs      thin Tauri commands over HavenClient
  removal.rs      thin Tauri command + Removed payload
  commands.rs     unlock/sync/change-password commands become wrappers
  lib.rs          builds HavenClient in setup
  device.rs       DELETED (moved)
```

---

### Task 1: Crate skeleton and `ClientError`

**Files:**
- Create: `crates/havenkeys-client/Cargo.toml`
- Create: `crates/havenkeys-client/src/lib.rs`
- Create: `crates/havenkeys-client/src/error.rs`
- Modify: `Cargo.toml` (workspace members)
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/state.rs:145-216` (CmdError block)
- Modify: `apps/desktop/src-tauri/src/sync.rs:516-542` (From<SyncError>)

**Interfaces:**
- Produces: `havenkeys_client::{ClientError, ClientResult, now_ms}`. `ClientError { pub code: &'static str, pub message: String }`, `Serialize`, `Clone`, `Debug`; constructors `internal()`, `file()`, `sign_in_failed()`, `vault_unreadable(&Path)`, `clipboard()`, `open_website()`, `keychain_unavailable()`, `password_change_unknown()`, `password_changed_elsewhere()`; `From<havenkeys_core::Error>`, `From<SyncError>`.

- [ ] **Step 1: Create the crate manifest**

`crates/havenkeys-client/Cargo.toml`:

```toml
[package]
name = "havenkeys-client"
version = "0.1.0"
description = "HavenKeys account, session and sync logic shared by the desktop and mobile apps"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
publish = false

[features]
# Exposes the in-memory and failing key stores to other crates' tests.
testing = []

[dependencies]
havenkeys-core = { path = "../havenkeys-core" }
havenkeys-sync-client = { path = "../havenkeys-sync-client" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt", "time"] }
uuid = { version = "1.20", features = ["v4", "serde"] }
zeroize = "1.8"

[dev-dependencies]
tempfile = "3"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
havenkeys-server = { path = "../havenkeys-server" }
axum = { version = "0.8", default-features = false, features = ["http1", "tokio"] }
tokio-postgres = { version = "0.7", features = ["with-uuid-1"] }
deadpool-postgres = "0.14"
```

Add `"crates/havenkeys-client",` to both `members` and `default-members` in the root `Cargo.toml`, after `"crates/havenkeys-sync-client",`.

In `apps/desktop/src-tauri/Cargo.toml`, add under `[dependencies]` after `havenkeys-sync-client`:

```toml
havenkeys-client = { path = "../../../crates/havenkeys-client" }
```

and under `[dev-dependencies]`:

```toml
havenkeys-client = { path = "../../../crates/havenkeys-client", features = ["testing"] }
```

- [ ] **Step 2: Write the failing error-table test**

`crates/havenkeys-client/src/error.rs` (test module only for now; the type comes in Step 4):

```rust
#[cfg(test)]
mod tests {
    use super::ClientError;
    use havenkeys_sync_client::{Conflict, SyncError};

    #[test]
    fn sync_errors_keep_the_codes_the_desktop_ui_knows() {
        let cases = [
            (SyncError::Conflict(Conflict::default()), "item_changed_elsewhere"),
            (SyncError::Unavailable, "offline"),
            (SyncError::Unauthorized, "signed_out"),
            (SyncError::RateLimited, "rate_limited"),
            (SyncError::InvalidServerUrl, "invalid_server_url"),
            (SyncError::TooLarge, "sync_failed"),
        ];
        for (err, code) in cases {
            assert_eq!(ClientError::from(err).code, code);
        }
    }

    #[test]
    fn fixed_errors_keep_their_codes() {
        let cases = [
            (ClientError::internal(), "internal"),
            (ClientError::file(), "file"),
            (ClientError::sign_in_failed(), "sign_in_failed"),
            (ClientError::clipboard(), "clipboard"),
            (ClientError::open_website(), "open_website"),
            (ClientError::keychain_unavailable(), "keychain_unavailable"),
            (ClientError::password_change_unknown(), "password_change_unknown"),
            (ClientError::password_changed_elsewhere(), "password_changed_elsewhere"),
        ];
        for (err, code) in cases {
            assert_eq!(err.code, code);
        }
    }

    #[test]
    fn the_servers_own_words_are_never_shown() {
        let err = ClientError::from(SyncError::Refused("<script>pwned</script>".into()));
        assert_eq!(err.code, "sync_failed");
        assert!(!err.message.contains("pwned"));
    }
}
```

Before writing it, check the constructors of `Conflict` and `SyncError::Refused` in `crates/havenkeys-sync-client/src/error.rs`. If `Conflict` has no `Default`, build it the way that file's own tests do, and pass `Refused` whatever payload type it takes (a `String` or a status).

`crates/havenkeys-client/src/lib.rs`:

```rust
//! Account, session and sync logic shared by the HavenKeys apps.
//!
//! Everything a device does with its account lives here: activation,
//! signing in, unlocking, the server session, sync and writes. The shells
//! (Tauri on desktop, UniFFI on mobile) only translate calls and events.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

mod error;

pub use error::{ClientError, ClientResult};

/// Wall-clock time in Unix milliseconds.
pub fn now_ms() -> i64 {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p havenkeys-client`
Expected: FAIL to compile — `cannot find type ClientError`.

- [ ] **Step 4: Move `CmdError` into `error.rs` as `ClientError`**

Put this above the test module in `crates/havenkeys-client/src/error.rs`. It is the desktop's `CmdError` (`state.rs:145-216`) plus the `From<SyncError>` from `sync.rs:516-542`, plus three constructors that were free functions in `commands.rs` (`keychain_unavailable`, `password_change_unknown`, and the message inside `credential_change_conflict`):

```rust
//! The one error type the shells see: a stable code and a fixed message.
//! Never carries a secret, and never the server's own words.

use havenkeys_sync_client::SyncError;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ClientError {
    pub code: &'static str,
    pub message: String,
}

pub type ClientResult<T> = Result<T, ClientError>;

impl From<havenkeys_core::Error> for ClientError {
    fn from(e: havenkeys_core::Error) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

impl From<SyncError> for ClientError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Conflict(_) => havenkeys_core::Error::ItemChangedElsewhere.into(),
            SyncError::Unavailable => havenkeys_core::Error::Offline.into(),
            SyncError::Unauthorized => Self::fixed(
                "signed_out",
                "HavenKeys is signed out of this account. Unlock again to reconnect.",
            ),
            SyncError::RateLimited => {
                Self::fixed("rate_limited", "Too many attempts. Try again in a few minutes.")
            }
            SyncError::InvalidServerUrl => Self::fixed(
                "invalid_server_url",
                "That server address cannot be used. It must start with https://.",
            ),
            // The server's own words are never shown: they are text an
            // attacker could choose.
            SyncError::Refused(_) | SyncError::Protocol(_) | SyncError::TooLarge => {
                Self::fixed("sync_failed", "The server did not accept that request.")
            }
        }
    }
}

impl ClientError {
    fn fixed(code: &'static str, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn internal() -> Self {
        Self::fixed("internal", "Internal error.")
    }

    pub fn file() -> Self {
        Self::fixed("file", "Could not read or delete the file.")
    }

    /// One message for every way signing in can fail. The device never says
    /// whether it was the address, the password or the Secret Key.
    pub fn sign_in_failed() -> Self {
        Self::fixed(
            "sign_in_failed",
            "Email, master password or Secret Key is incorrect.",
        )
    }

    /// The vault file exists but this build cannot open it. Carries the
    /// folder so the message can tell the user where their file is; a path
    /// is not a secret, and without it the advice is unfollowable.
    pub fn vault_unreadable(dir: &std::path::Path) -> Self {
        Self {
            code: "vault_unreadable",
            message: format!(
                "This vault was created by an older version of HavenKeys and cannot be \
                 opened by this one. Your file is in {}. Move vault.sqlite3 and \
                 device.json somewhere safe — do not delete them — and start HavenKeys \
                 again to set this computer up with an invite.",
                dir.display()
            ),
        }
    }

    pub fn clipboard() -> Self {
        Self::fixed("clipboard", "Could not access the clipboard.")
    }

    pub fn open_website() -> Self {
        Self::fixed("open_website", "Could not open the website.")
    }

    /// The keychain did not give a definite answer at unlock.
    pub fn keychain_unavailable() -> Self {
        Self::fixed(
            "keychain_unavailable",
            "Your system keychain did not answer. Approve its prompt if one is showing, then try again.",
        )
    }

    pub fn password_change_unknown() -> Self {
        Self::fixed(
            "password_change_unknown",
            "HavenKeys could not confirm whether the server applied the new master password. If your current password stops working, use the new one.",
        )
    }

    pub fn password_changed_elsewhere() -> Self {
        Self::fixed(
            "password_changed_elsewhere",
            "Your master password was changed on another device. Lock and unlock with the new password.",
        )
    }
}
```

Add `pub mod error;`-style wiring is already in `lib.rs` (`mod error;` + `pub use`).

- [ ] **Step 5: Make the desktop use it**

In `apps/desktop/src-tauri/src/state.rs`, delete the `CmdError` struct, its two `impl` blocks and `pub type CmdResult<T>` (lines 145–216) and add near the top:

```rust
pub use havenkeys_client::{ClientError as CmdError, ClientResult as CmdResult};
```

In `apps/desktop/src-tauri/src/sync.rs`, delete `impl From<SyncError> for CmdError` (lines 516–542).

In `apps/desktop/src-tauri/src/commands.rs`, replace the bodies of `keychain_unavailable()` and `password_change_unknown()` with `CmdError::keychain_unavailable()` / `CmdError::password_change_unknown()` call sites and delete the two functions; in `credential_change_conflict`, build the error with `CmdError::password_changed_elsewhere()`. Search `commands.rs` for the other literal `"password_changed_elsewhere"` constructor (around line 810) and replace it with `CmdError::password_changed_elsewhere()` too.

- [ ] **Step 6: Run all tests**

Run: `cargo test -p havenkeys-client && cargo test -p havenkeys-desktop`
Expected: PASS (the desktop's existing tests, including `other_credential_change_failures_keep_their_usual_mapping`, still pass).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/havenkeys-client apps/desktop/src-tauri
git commit -m "refactor(client): add havenkeys-client with the shared error type"
```

---

### Task 2: Move the key store and `Device`

**Files:**
- Create: `crates/havenkeys-client/src/key_store.rs` (from `apps/desktop/src-tauri/src/secret_store.rs`)
- Create: `crates/havenkeys-client/src/device.rs` (moved from `apps/desktop/src-tauri/src/device.rs`)
- Modify: `apps/desktop/src-tauri/src/secret_store.rs`
- Modify: `crates/havenkeys-client/src/lib.rs`
- Modify: every desktop `use crate::device` / `use crate::secret_store::{MemoryKeyStore,…}` site

**Interfaces:**
- Produces: `havenkeys_client::key_store::{KeyStore, TimedKeyStore, Storage, StoreError}`; with `cfg(any(test, feature = "testing"))`: `MemoryKeyStore`, `FailingKeyStore`, `SlowKeyStore`, `SlowOnceKeyStore`. `havenkeys_client::device::{Device, KeyStatus, Forgotten}` with unchanged methods (`load`, `key_status`, `secret_key`, `secret_key_text`, `secret_key_lookup`, `set_secret_key`, `migrate`, `forget`, `pub id: Uuid`).

- [ ] **Step 1: Write the failing compatibility test**

Add to the `tests` module that will live in `crates/havenkeys-client/src/device.rs` (create the file with just this module first):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_store::{FailingKeyStore, MemoryKeyStore};

    #[test]
    fn an_existing_device_json_is_read_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let key = SecretKey::generate().unwrap();
        let id = Uuid::from_u128(42);
        let written = format!(
            "{{\n  \"deviceId\": \"{id}\",\n  \"secretKey\": \"{}\"\n}}",
            key.to_text().expose()
        );
        std::fs::write(dir.path().join("device.json"), written).unwrap();

        let mut device = Device::load(dir.path(), Box::new(FailingKeyStore));
        assert_eq!(device.id, id);
        assert_eq!(
            device.secret_key_text(Uuid::from_u128(7)).unwrap().expose(),
            key.to_text().expose()
        );
        let _ = MemoryKeyStore::default();
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p havenkeys-client device`
Expected: FAIL to compile — `Device` and `key_store` not found.

- [ ] **Step 3: Move the code**

1. `crates/havenkeys-client/src/key_store.rs`: copy from `apps/desktop/src-tauri/src/secret_store.rs`:
   - the module doc, adjusted to: `//! Where a device keeps its Secret Key: a platform store behind the KeyStore trait. Nothing here returns or logs the value in an error. Every call runs on its own thread with a timeout (TimedKeyStore), because a platform store can wait a long time and a fallback is better than a frozen unlock screen.`
   - `use` lines, `TIMEOUT`, `Storage`, `StoreError` (+ its `Debug`), `KeyStore`, `NoAnswer`, `run_timed`, `TimedKeyStore` and `impl KeyStore for TimedKeyStore` (lines 12–165, minus the `SERVICE` constant);
   - the test fakes section (from `// --- test fakes` to the line before `#[cfg(test)] mod tests`), replacing each `#[cfg(test)]` on a fake with `#[cfg(any(test, feature = "testing"))]`;
   - the `tests` module minus `no_platform_keychain_is_a_definite_answer` and `a_keychain_that_failed_or_is_still_installing_is_not_a_definite_answer`.
   - `TimedKeyStore::with_timeout` stays `pub(crate)`.
2. `git mv apps/desktop/src-tauri/src/device.rs crates/havenkeys-client/src/device.rs`, then merge the Step 1 test into its `tests` module. Replace `use crate::secret_store::` with `use crate::key_store::` (top and in tests, including the `crate::secret_store::StoreError` paths in `FailsFirstGet`).
3. In `apps/desktop/src-tauri/src/secret_store.rs` delete everything moved; keep `SERVICE`, `OsKeyStore` and its `impl`s, `platform_store` and the two `OsKeyStore` tests. Add at the top:

```rust
use havenkeys_client::key_store::{KeyStore, StoreError};
```

   and add `pub use havenkeys_client::key_store::Storage;` so `crate::secret_store::Storage` keeps working where the desktop serializes it.
4. In `crates/havenkeys-client/src/lib.rs` add `pub mod device;` and `pub mod key_store;`.
5. Desktop: remove `mod device;` from `lib.rs`; replace `crate::device::Device` with `havenkeys_client::device::Device` and test imports of `crate::secret_store::MemoryKeyStore` with `havenkeys_client::key_store::MemoryKeyStore` (`account.rs` tests). `grep -rn "crate::device\|secret_store::{\|MemoryKeyStore" apps/desktop/src-tauri/src` must show no stale path.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-client && cargo test -p havenkeys-desktop`
Expected: PASS — 14 device tests and the timed-store tests now run in `havenkeys-client`; the two `OsKeyStore` tests still run in the desktop.

- [ ] **Step 5: Commit**

```bash
git add -A crates/havenkeys-client apps/desktop/src-tauri
git commit -m "refactor(client): move the Secret Key store and device record into havenkeys-client"
```

---

### Task 3: `HavenClient`, `ClientEvents` and the lock

**Files:**
- Create: `crates/havenkeys-client/src/events.rs`
- Create: `crates/havenkeys-client/src/client.rs`
- Create: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (setup, `migrate_secret_key_in_background`, `start_auto_lock`, `browser_bridge`)
- Modify: lock call sites in `lib.rs`, `tray.rs`, `commands.rs`, `account.rs`, `removal.rs`, `updater.rs`
- Modify: `.device` users in `commands.rs`, `emergency_kit.rs`, `account.rs`, `removal.rs`

**Interfaces:**
- Consumes: `ClientError`, `ClientResult`, `Device` (Tasks 1–2).
- Produces:

```rust
pub trait ClientEvents: Send + Sync {
    fn unlocked(&self, auto_lock_minutes: u32);
    fn locked(&self, reason: &'static str, was_open: bool);
    fn connectivity(&self, online: bool);
    fn signed_out(&self);
    fn synced(&self, report: SyncReport);
    fn items_changed(&self);
    fn removed(&self, keychain_cleared: bool);
}

pub struct ClientConfig { pub device_name: &'static str, pub vault_path: PathBuf }
pub type ServerClient = Arc<SyncClient<HttpTransport>>;

impl HavenClient {
    pub fn new(vault: Arc<Mutex<VaultService>>, device: Device, storage_error: Option<ClientError>,
               events: Arc<dyn ClientEvents>, config: ClientConfig) -> Arc<Self>;
    pub fn vault(&self) -> ClientResult<MutexGuard<'_, VaultService>>;
    pub fn device(&self) -> ClientResult<MutexGuard<'_, Device>>;
    pub fn device_id(&self) -> ClientResult<Uuid>;
    pub fn is_online(&self) -> bool;
    pub fn session(&self) -> ClientResult<Session>;
    pub fn go_offline(&self) -> bool;
    pub fn server(&self) -> ClientResult<ServerClient>;
    pub fn server_for(&self, url: &str) -> ClientResult<ServerClient>;
    pub fn forget_server(&self);
    pub fn sync_due(&self, interval: Duration) -> bool;
    pub fn require_unlocked(&self) -> ClientResult<()>;
    pub fn require_online(&self) -> ClientResult<()>;
    pub fn lock(&self, reason: &'static str);
    pub(crate) fn set_online(&self, session: Session);
    pub(crate) fn mark_sync_attempt(&self);
}
```

- [ ] **Step 1: Write the failing tests**

At the bottom of `crates/havenkeys-client/src/client.rs`:

```rust
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::key_store::MemoryKeyStore;
    use havenkeys_core::store::Store;
    use havenkeys_core::sync::SyncReport;

    #[derive(Default)]
    pub(crate) struct RecordingEvents(pub Mutex<Vec<String>>);

    impl RecordingEvents {
        pub fn seen(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
        fn push(&self, s: String) {
            self.0.lock().unwrap().push(s);
        }
    }

    impl ClientEvents for RecordingEvents {
        fn unlocked(&self, minutes: u32) {
            self.push(format!("unlocked:{minutes}"));
        }
        fn locked(&self, reason: &'static str, was_open: bool) {
            self.push(format!("locked:{reason}:{was_open}"));
        }
        fn connectivity(&self, online: bool) {
            self.push(format!("online:{online}"));
        }
        fn signed_out(&self) {
            self.push("signed_out".into());
        }
        fn synced(&self, _: SyncReport) {
            self.push("synced".into());
        }
        fn items_changed(&self) {
            self.push("items_changed".into());
        }
        fn removed(&self, keychain_cleared: bool) {
            self.push(format!("removed:{keychain_cleared}"));
        }
    }

    pub(crate) fn client_in(
        dir: &std::path::Path,
    ) -> (Arc<HavenClient>, Arc<RecordingEvents>) {
        let vault = Arc::new(Mutex::new(VaultService::new(
            Store::open_in_memory().unwrap(),
        )));
        let device = Device::load(dir, Box::new(MemoryKeyStore::default()));
        let events = Arc::new(RecordingEvents::default());
        let client = HavenClient::new(
            vault,
            device,
            None,
            events.clone(),
            ClientConfig {
                device_name: "Test",
                vault_path: dir.join("vault.sqlite3"),
            },
        );
        (client, events)
    }

    #[test]
    fn a_new_client_is_offline_and_refuses_writes() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        assert!(!client.is_online());
        assert_eq!(client.require_online().unwrap_err().code, "offline");
        assert_eq!(client.session().unwrap_err().code, "offline");
    }

    #[test]
    fn a_storage_error_is_returned_by_every_vault_access() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(VaultService::new(
            Store::open_in_memory().unwrap(),
        )));
        let client = HavenClient::new(
            vault,
            Device::load(dir.path(), Box::new(MemoryKeyStore::default())),
            Some(ClientError::vault_unreadable(dir.path())),
            Arc::new(RecordingEvents::default()),
            ClientConfig {
                device_name: "Test",
                vault_path: dir.path().join("vault.sqlite3"),
            },
        );
        assert_eq!(client.vault().err().unwrap().code, "vault_unreadable");
    }

    #[test]
    fn locking_a_locked_vault_tells_the_shell_it_was_not_open() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        client.lock("user");
        assert_eq!(events.seen(), vec!["locked:user:false"]);
    }

    #[test]
    fn going_offline_is_announced_only_when_a_session_was_held() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        assert!(!client.go_offline());
        assert!(events.seen().is_empty());
    }

    #[test]
    fn the_server_client_is_rebuilt_for_another_url() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        let a = client.server_for("https://a.example.com").unwrap();
        let again = client.server_for("https://a.example.com").unwrap();
        let b = client.server_for("https://b.example.com").unwrap();
        assert!(Arc::ptr_eq(&a, &again));
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn a_pull_is_due_until_one_is_attempted() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        assert!(client.sync_due(Duration::from_secs(60)));
        client.mark_sync_attempt();
        assert!(!client.sync_due(Duration::from_secs(60)));
        client.lock("user");
        assert!(client.sync_due(Duration::from_secs(60)));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p havenkeys-client client`
Expected: FAIL to compile — `HavenClient`, `ClientEvents` not found.

- [ ] **Step 3: Write `events.rs` and `client.rs`**

`crates/havenkeys-client/src/events.rs`:

```rust
use havenkeys_core::sync::SyncReport;

/// What the shell is told. Implementations must return quickly and must not
/// take the vault: `unlocked` and `locked` run while it is held, so that a
/// concurrent lock can never be overtaken by an unlock announcement.
pub trait ClientEvents: Send + Sync {
    /// The vault opened; start the auto-lock clock.
    fn unlocked(&self, auto_lock_minutes: u32);
    /// The vault locked (or a lock was requested on a locked vault:
    /// `was_open` is false). Clear anything that held secrets.
    fn locked(&self, reason: &'static str, was_open: bool);
    fn connectivity(&self, online: bool);
    /// The server refused this device's session.
    fn signed_out(&self);
    fn synced(&self, report: SyncReport);
    fn items_changed(&self);
    /// This device left its account; `keychain_cleared` is false when the
    /// Secret Key may still be in the platform store.
    fn removed(&self, keychain_cleared: bool);
}
```

`crates/havenkeys-client/src/client.rs`:

```rust
//! The client's state: the vault handle, the device record, the server
//! session and the HTTP client.

use crate::device::Device;
use crate::error::{ClientError, ClientResult};
use crate::events::ClientEvents;
use havenkeys_core::vault::VaultService;
use havenkeys_sync_client::{HttpTransport, Session, SyncClient};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub type ServerClient = Arc<SyncClient<HttpTransport>>;

/// Whether this device has a server session. A locked vault is never online;
/// an unlocked one may be offline (spec 2026-09-20 §8.6).
enum Connectivity {
    Offline,
    /// The token lives here and nowhere else — never on disk — and is
    /// dropped (and zeroized) when the vault locks or the device goes
    /// offline.
    Online(Session),
}

pub struct ClientConfig {
    /// The label this device reports. Deliberately not the hostname, which
    /// is metadata the server has no use for.
    pub device_name: &'static str,
    /// The vault database, which removing the device sets aside.
    pub vault_path: PathBuf,
}

pub struct HavenClient {
    vault: Arc<Mutex<VaultService>>,
    device: Mutex<Device>,
    connectivity: Mutex<Connectivity>,
    /// Kept because building one sets up a TLS stack. Keyed by URL so a
    /// re-pointed vault cannot keep talking to the old server.
    server: Mutex<Option<(String, ServerClient)>>,
    /// When a pull was last attempted, so the periodic one keeps its spacing
    /// whether or not the attempt worked.
    last_sync_attempt: Mutex<Option<Duration>>,
    /// Set when the vault file could not be opened: every vault access
    /// refuses with it.
    storage_error: Option<ClientError>,
    pub(crate) events: Arc<dyn ClientEvents>,
    pub(crate) config: ClientConfig,
    origin: Instant,
}

impl HavenClient {
    pub fn new(
        vault: Arc<Mutex<VaultService>>,
        device: Device,
        storage_error: Option<ClientError>,
        events: Arc<dyn ClientEvents>,
        config: ClientConfig,
    ) -> Arc<Self> {
        Arc::new(Self {
            vault,
            device: Mutex::new(device),
            connectivity: Mutex::new(Connectivity::Offline),
            server: Mutex::new(None),
            last_sync_attempt: Mutex::new(None),
            storage_error,
            events,
            config,
            origin: Instant::now(),
        })
    }

    pub fn vault(&self) -> ClientResult<MutexGuard<'_, VaultService>> {
        if let Some(err) = &self.storage_error {
            return Err(err.clone());
        }
        self.vault.lock().map_err(|_| ClientError::internal())
    }

    pub fn device(&self) -> ClientResult<MutexGuard<'_, Device>> {
        self.device.lock().map_err(|_| ClientError::internal())
    }

    pub fn device_id(&self) -> ClientResult<Uuid> {
        Ok(self.device()?.id)
    }

    pub fn is_online(&self) -> bool {
        matches!(
            self.connectivity.lock().as_deref(),
            Ok(Connectivity::Online(_))
        )
    }

    pub fn session(&self) -> ClientResult<Session> {
        match self.connectivity.lock().as_deref() {
            Ok(Connectivity::Online(session)) => Ok(session.clone()),
            _ => Err(havenkeys_core::Error::Offline.into()),
        }
    }

    pub(crate) fn set_online(&self, session: Session) {
        if let Ok(mut c) = self.connectivity.lock() {
            *c = Connectivity::Online(session);
        }
    }

    /// Drop the session without telling the shell. Returns whether one was held.
    fn drop_session(&self) -> bool {
        match self.connectivity.lock() {
            Ok(mut c) => {
                let was_online = matches!(*c, Connectivity::Online(_));
                *c = Connectivity::Offline;
                was_online
            }
            Err(_) => false,
        }
    }

    /// Drop the session and tell the shell, when one was held.
    pub fn go_offline(&self) -> bool {
        let dropped = self.drop_session();
        if dropped {
            self.events.connectivity(false);
        }
        dropped
    }

    /// The HTTP client for this vault's server.
    pub fn server(&self) -> ClientResult<ServerClient> {
        let url = self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?
            .server_url;
        self.server_for(&url)
    }

    pub fn server_for(&self, url: &str) -> ClientResult<ServerClient> {
        let mut cached = self.server.lock().map_err(|_| ClientError::internal())?;
        if let Some((cached_url, client)) = cached.as_ref() {
            if cached_url == url {
                return Ok(client.clone());
            }
        }
        let client = Arc::new(SyncClient::new(HttpTransport::new(url)?));
        *cached = Some((url.to_string(), client.clone()));
        Ok(client)
    }

    /// So a stale client for an old server is never reused after this
    /// device leaves its account.
    pub fn forget_server(&self) {
        if let Ok(mut cached) = self.server.lock() {
            *cached = None;
        }
    }

    pub(crate) fn mark_sync_attempt(&self) {
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = Some(self.origin.elapsed());
        }
    }

    /// Is a periodic pull due? Also true when none has been attempted yet.
    pub fn sync_due(&self, interval: Duration) -> bool {
        match self.last_sync_attempt.lock() {
            Ok(last) => last.is_none_or(|at| self.origin.elapsed().saturating_sub(at) >= interval),
            Err(_) => false,
        }
    }

    pub fn require_unlocked(&self) -> ClientResult<()> {
        if self.vault()?.is_unlocked() {
            Ok(())
        } else {
            Err(havenkeys_core::Error::Locked.into())
        }
    }

    pub fn require_online(&self) -> ClientResult<()> {
        if self.is_online() {
            Ok(())
        } else {
            Err(havenkeys_core::Error::Offline.into())
        }
    }

    /// Lock the vault and drop the session: a locked vault cannot reach the
    /// server at all (spec 2026-09-20 §6).
    pub fn lock(&self, reason: &'static str) {
        let was_open = match self.vault.lock() {
            Ok(mut v) => v.lock(),
            // A poisoned mutex means a panic mid-operation; the session is
            // still dropped.
            Err(poisoned) => poisoned.into_inner().lock(),
        };
        self.drop_session();
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = None;
        }
        self.events.locked(reason, was_open);
    }
}
```

`HttpTransport::new(url)?` relies on `From<SyncError> for ClientError` (Task 1).

In `crates/havenkeys-client/src/lib.rs` add:

```rust
mod client;
mod events;

pub use client::{ClientConfig, HavenClient, ServerClient};
pub use events::ClientEvents;
```

- [ ] **Step 4: Run the client tests**

Run: `cargo test -p havenkeys-client`
Expected: PASS.

- [ ] **Step 5: Wire the desktop to `HavenClient`**

`apps/desktop/src-tauri/src/events.rs`:

```rust
//! The desktop's answer to `ClientEvents`: Tauri events for the window,
//! the auto-lock clock and the browser bridge.

use crate::state::{AppState, ITEMS_CHANGED_EVENT, LOCKED_EVENT};
use crate::sync::{CONNECTIVITY_EVENT, SIGNED_OUT_EVENT, SYNCED_EVENT};
use havenkeys_client::ClientEvents;
use havenkeys_core::sync::SyncReport;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const REMOVED_EVENT: &str = "vault://removed";

const KEYCHAIN_NOT_CLEARED: &str = "This computer was removed, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry \u{201c}app.havenkeys\u{201d} yourself.";

#[derive(Clone, Serialize)]
struct LockedPayload {
    reason: &'static str,
}

/// The `vault://removed` payload.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Removed {
    /// Set when the Secret Key may still be in the system keychain.
    keychain_warning: Option<&'static str>,
}

pub struct DesktopEvents {
    pub app: AppHandle,
}

impl ClientEvents for DesktopEvents {
    fn unlocked(&self, auto_lock_minutes: u32) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.arm_auto_lock(auto_lock_minutes);
            state.notify_unlocked();
        }
    }

    fn locked(&self, reason: &'static str, was_open: bool) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.clear_after_lock();
            if was_open {
                state.notify_locked();
                let _ = self.app.emit(LOCKED_EVENT, LockedPayload { reason });
            }
        }
    }

    fn connectivity(&self, online: bool) {
        let _ = self.app.emit(CONNECTIVITY_EVENT, online);
    }

    fn signed_out(&self) {
        let _ = self.app.emit(SIGNED_OUT_EVENT, ());
    }

    fn synced(&self, report: SyncReport) {
        let _ = self.app.emit(SYNCED_EVENT, report);
    }

    fn items_changed(&self) {
        let _ = self.app.emit(ITEMS_CHANGED_EVENT, ());
    }

    fn removed(&self, keychain_cleared: bool) {
        let payload = Removed {
            keychain_warning: (!keychain_cleared).then_some(KEYCHAIN_NOT_CLEARED),
        };
        let _ = self.app.emit(REMOVED_EVENT, payload);
    }
}
```

`apps/desktop/src-tauri/src/state.rs` — the struct and the changed methods:

```rust
pub struct AppState {
    client: Arc<HavenClient>,
    /// The same vault the client holds, for the auto-lock tick, which must
    /// keep working on a poisoned mutex.
    vault: Arc<Mutex<VaultService>>,
    bridge: Bridge,
    lock_manager: Mutex<LockManager>,
    pub clipboard: ClipboardGuard,
    /// The export file the user picked for the last import, so the UI can
    /// offer to delete it without ever supplying a path itself.
    pub last_import: Mutex<Option<PathBuf>>,
    /// The last QR scan's codes, waiting to be saved (scan_slot.rs).
    totp_scan: Mutex<ScanSlot>,
    origin: Instant,
    /// The app's data folder: where `vault.sqlite3` and `device.json` live.
    data_dir: PathBuf,
}

impl AppState {
    pub fn new(
        client: Arc<HavenClient>,
        vault: Arc<Mutex<VaultService>>,
        bridge: Bridge,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            client,
            vault,
            bridge,
            lock_manager: Mutex::new(LockManager::new(None)),
            clipboard: ClipboardGuard::default(),
            last_import: Mutex::new(None),
            totp_scan: Mutex::new(ScanSlot::default()),
            origin: Instant::now(),
            data_dir,
        }
    }

    pub fn client(&self) -> &Arc<HavenClient> {
        &self.client
    }

    pub fn vault(&self) -> CmdResult<MutexGuard<'_, VaultService>> {
        self.client.vault()
    }

    pub fn is_online(&self) -> bool {
        self.client.is_online()
    }

    pub fn session(&self) -> CmdResult<Session> {
        self.client.session()
    }

    pub fn device_id(&self) -> CmdResult<uuid::Uuid> {
        self.client.device_id()
    }

    pub fn require_unlocked(&self) -> CmdResult<()> {
        self.client.require_unlocked()
    }

    pub fn require_online(&self) -> CmdResult<()> {
        self.client.require_online()
    }

    pub fn now_ms() -> i64 {
        havenkeys_client::now_ms()
    }

    pub fn notify_locked(&self) {
        self.bridge.notify(Event::Locked {});
    }

    /// What locking clears on the desktop side. Called by `DesktopEvents`
    /// on every lock, whether or not the vault was open.
    pub fn clear_after_lock(&self) {
        self.clipboard.clear_if_owned(None);
        // The path of the last imported `.1pux` is only there so the UI can
        // offer to delete it after an import. A locked vault has no import in
        // progress, so the offer — and the ability to act on it — goes away.
        if let Ok(mut last) = self.last_import.lock() {
            *last = None;
        }
        // Scanned TOTP codes waiting to be saved are vault secrets too.
        self.clear_totp_scan();
    }

    pub fn lock(&self, reason: &'static str) {
        self.client.lock(reason);
    }

    /// Called periodically by the auto-lock thread.
    pub fn auto_lock_tick(&self) {
        let reason = {
            // Treat a poisoned mutex like a normal one: auto-lock must keep working.
            let vault = self.vault.lock().unwrap_or_else(|p| p.into_inner());
            if !vault.is_unlocked() {
                return;
            }
            let mut lm = self.lock_manager.lock().unwrap_or_else(|p| p.into_inner());
            lm.tick(self.mono(), Self::wall())
        };
        if let Some(reason) = reason {
            self.lock(reason.as_str());
        }
    }
}
```

Keep `data_dir`, `store_totp_scan`, `resolve_item_input`, `clear_totp_scan`, `mono`, `wall`, `unix_seconds`, `touch`, `notify_unlocked`, `arm_auto_lock` as they are. Delete `Connectivity`, `device`, `connectivity`, `sync_client`, `last_sync_attempt`, `storage_error`, `forget_sync_client`, `set_online`, `go_offline`, `sync_client()`, `mark_sync_attempt`, `sync_due`, `LockedPayload` and the old `lock`. Check that `reason.as_str()` returns `&'static str`; if it returns `&str`, make `ClientEvents::locked` and `HavenClient::lock` take `&str` and change `LockedPayload.reason` to `String`.

`apps/desktop/src-tauri/src/lib.rs` setup — replace the `device`/`bridge`/`app.manage(AppState::new(...))` lines with:

```rust
let device = havenkeys_client::device::Device::load(
    &dir,
    Box::new(secret_store::OsKeyStore::install()),
);
let events = Arc::new(events::DesktopEvents {
    app: app.handle().clone(),
});
let client = havenkeys_client::HavenClient::new(
    vault.clone(),
    device,
    storage_error,
    events,
    havenkeys_client::ClientConfig {
        device_name: "Desktop",
        vault_path: path.clone(),
    },
);
let bridge = browser_bridge(app.handle(), vault.clone());
app.manage(AppState::new(client, vault, bridge.clone(), dir.clone()));
```

and add `mod events;`. In `migrate_secret_key_in_background` replace `state.device.lock()` with `state.client().device()`. In `start_auto_lock` replace `state.lock(&handle, "screen_lock")` with `state.lock("screen_lock")`, `state.auto_lock_tick(&handle)` with `state.auto_lock_tick()`, and `state.sync_due(PULL_INTERVAL)` with `state.client().sync_due(PULL_INTERVAL)`.

Lock call sites: `grep -rn '\.lock(&\?[a-z_]*, "' apps/desktop/src-tauri/src` lists them (lib.rs ×4, tray.rs ×2, commands.rs ×2, account.rs, removal.rs, updater.rs). Remove the handle argument from each: `state.lock(&app, "user")` → `state.lock("user")`.

Device users: `grep -rn '\.device$\|\.device\.lock()' apps/desktop/src-tauri/src` — replace `state.device.lock().map_err(|_| CmdError::internal())?` (also when split over lines) with `state.client().device()?`, and `if let Ok(mut d) = state.device.lock()` with `if let Ok(mut d) = state.client().device()`.

Calls of the deleted `AppState` methods (`state.set_online`, `state.go_offline`, `state.sync_client`, `state.mark_sync_attempt`, `state.forget_sync_client`) are all inside `sync.rs`, `account.rs`, `commands.rs` and `removal.rs`, which Tasks 4–6 rewrite. To keep this task compiling, route them through the client for now: `state.client().set_online(..)` is `pub(crate)`, so temporarily make `set_online` and `mark_sync_attempt` `pub` with a `// pub until Task 6` note, and switch the desktop's `client_for` to `state.client().server_for(url)`. Task 6 Step 4 makes both `pub(crate)` again.

- [ ] **Step 6: Run everything**

Run: `cargo test -p havenkeys-client && cargo test -p havenkeys-desktop && cargo clippy -p havenkeys-client -p havenkeys-desktop -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 7: Commit**

```bash
git add -A crates/havenkeys-client apps/desktop/src-tauri
git commit -m "refactor(client): HavenClient owns the session and the lock; desktop listens through ClientEvents"
```

---

### Task 4: Sync and writes

**Files:**
- Create: `crates/havenkeys-client/src/sync.rs`
- Modify: `apps/desktop/src-tauri/src/sync.rs` (keep only event names and `bridge_error`)
- Modify: desktop callers of `sync::connect`, `sync::sync_now`, `sync::push`, `sync::push_batches`, `sync::failed`, `sync::go_offline`, `sync::client`, `sync::client_for`, `sync::DEVICE_NAME`, `sync::MAX_BATCH`, `sync::PULL_INTERVAL_SECS`

**Interfaces:**
- Consumes: `HavenClient` (Task 3).
- Produces:

```rust
pub const PULL_INTERVAL: Duration;   // 60 s
pub const MAX_BATCH: usize;          // 500
impl HavenClient {
    pub async fn connect(&self, auth_key: AuthKey) -> ClientResult<()>;
    pub async fn sync_now(&self) -> ClientResult<SyncReport>;
    pub async fn push(&self, staged: StagedWrite) -> ClientResult<Option<ItemOverview>>;
    pub async fn push_batches(&self, staged: Vec<StagedWrite>) -> ClientResult<usize>;
    pub(crate) fn failed(&self, err: SyncError) -> ClientError;
}
```

- [ ] **Step 1: Write the failing test**

In `crates/havenkeys-client/src/sync.rs` tests (needs a vault that is activated but locked — no server is involved, because the check must come before any request):

```rust
#[cfg(test)]
mod tests {
    use crate::client::tests::client_in;
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::store::AccountRecord;
    use havenkeys_core::vault::prepare_new_account_vault;
    use havenkeys_core::SecretString;
    use uuid::Uuid;

    #[tokio::test]
    async fn connect_on_a_locked_vault_stays_offline() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        let account = AccountRef::new(
            Uuid::from_u128(1),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
        let made = prepare_new_account_vault(
            &SecretString::from("correct horse battery staple"),
            &account,
            kdf,
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
                    // Nothing listens here: a request would fail as offline,
                    // not as locked.
                    server_url: "http://127.0.0.1:9".into(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        client.lock("user");

        let err = client.connect(made.auth_key).await.unwrap_err();
        assert_eq!(err.code, "locked");
        assert!(!client.is_online());
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p havenkeys-client connect_on_a_locked_vault`
Expected: FAIL to compile — no `connect` on `HavenClient`.

- [ ] **Step 3: Write `sync.rs`**

The desktop `sync.rs` functions as methods, with `app.state::<AppState>()` → `self`, `failed(app, e)` → `self.failed(e)`, `client(&state)` → `self.server()`, `AppState::now_ms()` → `now_ms()`, and every `app.emit(..)` → the matching `self.events` call. `connect` gets two changes: it refuses a locked vault before any request, and it sets the session under the vault guard only if the vault is still unlocked.

```rust
//! The server session, and every operation that needs one.
//!
//! The vault is a replica: reads come from SQLite and work offline, writes go
//! to the server first and are recorded locally only once it has accepted
//! them (spec 2026-09-20 §8.4).
//!
//! Nothing here holds the vault lock across an `await`: the guard is taken,
//! used and dropped before every network call, so a lock is never delayed by
//! a slow server.

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::now_ms;
use havenkeys_core::crypto::keys::AuthKey;
use havenkeys_core::model::ItemOverview;
use havenkeys_core::sync::SyncReport;
use havenkeys_core::vault::StagedWrite;
use havenkeys_sync_client::SyncError;
use std::time::Duration;
use uuid::Uuid;

/// How often an unlocked, online device pulls.
pub const PULL_INTERVAL: Duration = Duration::from_secs(60);

/// The most changes the server takes in one request (spec 2026-09-20 §7.3).
pub const MAX_BATCH: usize = 500;

impl HavenClient {
    /// Map a failure, and drop the session when the server says it is gone,
    /// so the device falls back to read-only instead of retrying with a dead
    /// token. A refused session is announced as signed out, not just offline.
    pub(crate) fn failed(&self, err: SyncError) -> ClientError {
        if matches!(err, SyncError::Unauthorized | SyncError::Unavailable) {
            let dropped = self.go_offline();
            if dropped && err == SyncError::Unauthorized {
                self.events.signed_out();
            }
        }
        err.into()
    }

    /// Sign in to the account with a freshly derived auth key, then catch up.
    ///
    /// A failure here is not an unlock failure: the vault stays open and
    /// readable, offline.
    pub async fn connect(&self, auth_key: AuthKey) -> ClientResult<()> {
        let (email, account_id, device_id, server) = {
            let vault = self.vault()?;
            if !vault.is_unlocked() {
                return Err(havenkeys_core::Error::Locked.into());
            }
            let account = vault.account()?.ok_or(havenkeys_core::Error::NoVault)?;
            drop(vault);
            let server = self.server_for(&account.server_url)?;
            (account.email, account.account_id, self.device_id()?, server)
        };

        let session = server
            .login(&email, &auth_key, account_id, device_id, self.config.device_name)
            .await
            .map_err(|e| {
                // No session is held here as a rule, so `failed` would not
                // announce this one.
                if e == SyncError::Unauthorized {
                    self.events.signed_out();
                }
                self.failed(e)
            })?;
        drop(auth_key);

        // A lock that landed while signing in wins: the session is dropped
        // unused rather than left on a locked vault.
        {
            let vault = self.vault()?;
            if !vault.is_unlocked() {
                return Err(havenkeys_core::Error::Locked.into());
            }
            self.set_online(session);
        }
        self.events.connectivity(true);
        self.sync_now().await?;
        self.ensure_identity().await;
        Ok(())
    }

    /// Create the account's Identity if the replica does not have it (spec
    /// 2026-09-29-identity-item §5.2). Runs after the pull, so "missing" means
    /// missing on the server too, unless another device is creating it right
    /// now: then the server refuses this write and a pull brings theirs. Never
    /// fails the caller; the next connect tries again.
    async fn ensure_identity(&self) {
        let staged = {
            let Ok(vault) = self.vault() else { return };
            let Ok(Some(account)) = vault.account() else {
                return;
            };
            match vault.stage_identity_if_missing(&account.email, now_ms()) {
                Ok(Some(staged)) => staged,
                _ => return,
            }
        };
        match self.push(staged).await {
            Ok(_) => self.events.items_changed(),
            Err(e) if e.code == "item_changed_elsewhere" => {
                if self.sync_now().await.is_ok() {
                    self.events.items_changed();
                }
            }
            Err(_) => {}
        }
    }

    /// Reconcile the header, then pull until the replica has caught up.
    pub async fn sync_now(&self) -> ClientResult<SyncReport> {
        // Recorded up front, so a failing server is retried on the same
        // spacing rather than on every tick.
        self.mark_sync_attempt();
        let (session, server) = (self.session()?, self.server()?);
        let mut report = SyncReport::default();

        // The header first: a master password changed on another device must
        // be adopted before anything else. Nothing is ever published from
        // here. The core checks the attestation and refuses a revision that
        // goes backwards.
        let remote = server.header(&session).await.map_err(|e| self.failed(e))?;
        {
            let revision = self.vault()?.header_revision()?.unwrap_or(0);
            if remote.revision as u64 > revision {
                report.header_adopted = self.vault()?.adopt_account_header(&remote.bytes)?;
            } else if revision > remote.revision as u64 {
                // Ahead of the server means something is wrong locally; send
                // nothing.
                return Err(ClientError::internal());
            }
        }

        let mut cursor = self.vault()?.account()?.map(|a| a.server_cursor).unwrap_or(0);
        loop {
            let pulled = server
                .pull(&session, cursor)
                .await
                .map_err(|e| self.failed(e))?;
            let has_more = pulled.has_more;
            let next = pulled.cursor;
            let page = self
                .vault()?
                .apply_remote_changes(next, pulled.changes, now_ms())?;
            report.added += page.added;
            report.updated += page.updated;
            report.deleted += page.deleted;
            report.skipped_items += page.skipped_items;

            // A server claiming there is more while handing out the same
            // cursor would spin this loop forever.
            if !has_more || next <= cursor {
                break;
            }
            cursor = next;
        }

        // Items that did not open earlier are asked for again, by id, until
        // they open, are deleted, or a Re-download clears them. Best effort:
        // a failure stops the retry for this sync and nothing more. Only a
        // refused session is acted on.
        let pending = self.vault()?.unreadable_item_ids()?;
        for chunk in pending.chunks(MAX_BATCH) {
            let changes = match server.fetch_items(&session, chunk).await {
                Ok(changes) => changes,
                Err(SyncError::Unauthorized) => {
                    let _ = self.failed(SyncError::Unauthorized);
                    break;
                }
                Err(_) => break,
            };
            let applied = self
                .vault()
                .and_then(|mut v| Ok(v.apply_refetched(chunk, changes, now_ms())?));
            let Ok(page) = applied else { break };
            report.added += page.added;
            report.updated += page.updated;
            report.deleted += page.deleted;
        }

        self.mark_sync_attempt();
        self.events.synced(report);
        Ok(report)
    }

    /// Send one staged write, then record what the server accepted. Nothing
    /// is written locally until the server has assigned the revision, so the
    /// replica can never be ahead of the authority.
    pub async fn push(&self, staged: StagedWrite) -> ClientResult<Option<ItemOverview>> {
        let (session, server) = (self.session()?, self.server()?);
        let item_id = staged.item_id;
        let ack = server
            .write(&session, std::slice::from_ref(&staged))
            .await
            .map_err(|e| self.failed(e))?;
        let revision = revision_for(&ack.applied, item_id)?;
        Ok(self.vault()?.commit_write(staged, revision)?)
    }

    /// Send many staged writes, in batches the server accepts, committing
    /// each batch before the next is sent. Returns how many were recorded: a
    /// refused batch stops the run, and earlier batches are already on the
    /// server.
    pub async fn push_batches(&self, staged: Vec<StagedWrite>) -> ClientResult<usize> {
        let (session, server) = (self.session()?, self.server()?);
        let mut committed = 0usize;
        let mut queue = staged;
        while !queue.is_empty() {
            let rest = queue.split_off(queue.len().min(MAX_BATCH));
            let batch = std::mem::replace(&mut queue, rest);
            let ack = server
                .write(&session, &batch)
                .await
                .map_err(|e| self.failed(e))?;
            for write in batch {
                let revision = revision_for(&ack.applied, write.item_id)?;
                self.vault()?.commit_write(write, revision)?;
                committed += 1;
            }
        }
        Ok(committed)
    }
}

fn revision_for(applied: &[(Uuid, i64)], item_id: Uuid) -> ClientResult<i64> {
    applied
        .iter()
        .find(|(id, _)| *id == item_id)
        .map(|(_, revision)| *revision)
        .ok_or_else(|| ClientError {
            code: "sync_failed",
            message: "The server did not acknowledge that item.".into(),
        })
}
```

In `crates/havenkeys-client/src/lib.rs` add `mod sync;` and `pub use sync::{MAX_BATCH, PULL_INTERVAL};`.

- [ ] **Step 4: Run the client tests**

Run: `cargo test -p havenkeys-client`
Expected: PASS.

- [ ] **Step 5: Make the desktop call the client**

`apps/desktop/src-tauri/src/sync.rs` becomes the module doc line `//! Event names for the server session, and what the browser extension is told when a save fails.`, the three event constants (`SYNCED_EVENT`, `CONNECTIVITY_EVENT`, `SIGNED_OUT_EVENT`) and `bridge_error` (unchanged). Everything else is deleted.

Callers — capture the client as an `Arc` before awaiting, so no `State` borrow crosses an await:

```rust
let client = app.state::<AppState>().client().clone();
client.sync_now().await
```

- `commands.rs`: `sync::sync_now(&app)` → as above; `sync::push(&app, staged)` / `sync::push(app, staged)` → `client.push(staged)`; `sync::failed(&app, e)` → `state.client().failed(e)` is `pub(crate)`, so `change_master_password` (moved in Task 5) keeps calling the old path until then: make `failed` `pub` with `// pub until Task 6` and restore `pub(crate)` in Task 6 Step 4.
- `import.rs`: `sync::push_batches(&app, staged)` → `client.push_batches(staged)`.
- `lib.rs`: `PULL_INTERVAL` → `havenkeys_client::PULL_INTERVAL` (delete the local const); in `start_auto_lock` the pull becomes `let client = state.client().clone(); tauri::async_runtime::spawn(async move { let _ = client.sync_now().await; });`; in `browser_bridge` the writer becomes:

```rust
move |staged| {
    let Some(state) = save_handle.try_state::<AppState>() else {
        return Err(havenkeys_protocol::ErrorCode::Internal);
    };
    let client = state.client().clone();
    tauri::async_runtime::block_on(async move { client.push(staged).await.map(|_| ()) })
        .map_err(sync::bridge_error)
}
```

- `account.rs` and the unlock code in `commands.rs` still call `sync::connect`, `sync::client_for`, `sync::client`, `sync::go_offline`, `sync::DEVICE_NAME`: replace with `client.connect(auth_key)`, `state.client().server_for(&url)`, `state.client().server()`, `state.client().go_offline()` and the literal `"Desktop"` until Task 5 deletes those bodies.

`grep -rn 'sync::' apps/desktop/src-tauri/src` must show only the three event constants and `bridge_error`.

- [ ] **Step 6: Run everything**

Run: `cargo test -p havenkeys-client && cargo test -p havenkeys-desktop && cargo clippy -p havenkeys-client -p havenkeys-desktop -- -D warnings`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A crates/havenkeys-client apps/desktop/src-tauri
git commit -m "refactor(client): sync and writes move to HavenClient; connect refuses a locked vault"
```

---

### Task 5: Account operations

**Files:**
- Create: `crates/havenkeys-client/src/account.rs`
- Modify: `apps/desktop/src-tauri/src/account.rs` (thin commands)
- Modify: `apps/desktop/src-tauri/src/commands.rs` (`unlock_vault`, `unlock_from_server`, `remember_typed_secret_key`, `vault_account`, `change_master_password`, `ChangeOutcome`, `change_outcome`, `credential_change_conflict`, their 4 tests, `FALLBACK_PARAMS_TIMEOUT`)

**Interfaces:**
- Consumes: Tasks 3–4.
- Produces:

```rust
pub struct DeviceStatus { /* serde camelCase: keyScheme, needsSecretKey, online, secretKeyStorage */ }
pub struct AccountStatus { /* email, serverUrl, accountId, online, lastSyncedAt */ }
pub struct DeviceEntry { /* id, name, createdAt, lastSeenAt, current */ }
pub enum AccountField { Email, Server, AccountId, SecretKey }   // serde snake_case, Deserialize
impl HavenClient {
    pub fn device_status(&self) -> ClientResult<DeviceStatus>;          // blocking: may wait on the platform store
    pub fn account_status(&self) -> ClientResult<Option<AccountStatus>>;
    pub async fn activate(self: &Arc<Self>, invite: String, password: SecretString) -> ClientResult<VaultStatus>;
    pub async fn sign_in(self: &Arc<Self>, server_url: String, email: String, password: SecretString,
                         secret_key: Option<SecretString>) -> ClientResult<VaultStatus>;
    pub async fn unlock(self: &Arc<Self>, password: SecretString, secret_key: Option<SecretString>) -> ClientResult<VaultStatus>;
    pub async fn change_master_password(&self, current: SecretString, new: SecretString) -> ClientResult<()>;
    pub async fn list_devices(&self) -> ClientResult<Vec<DeviceEntry>>;
    pub async fn revoke_device(&self, id: Uuid) -> ClientResult<()>;
    pub async fn sign_out(&self) -> ClientResult<()>;
    pub fn account_value(&self, field: AccountField) -> ClientResult<(SecretString, u32)>;   // blocking
}
```

The methods that start background work (`activate`, `sign_in`, `unlock`) take `self: &Arc<Self>` and use `tokio::spawn`; they must be called from inside a tokio runtime (Tauri's async commands are).

- [ ] **Step 1: Move the tests first**

Create `crates/havenkeys-client/src/account.rs` containing only a test module with:
- the `account_item` tests from `apps/desktop/src-tauri/src/account.rs:461-582`, with `use crate::device::Device;` and `use crate::key_store::MemoryKeyStore;` and the paths `super::super::{account_field, unlocked_account, AccountField}` kept;
- the four tests from `apps/desktop/src-tauri/src/commands.rs` (`a_lost_credential_change_is_judged_by_the_servers_kdf`, `an_unconfirmed_password_change_says_so`, `a_conflict_on_a_credential_change_means_changed_elsewhere`, `other_credential_change_failures_keep_their_usual_mapping`) in a `mod credential_change` with `use super::super::*;`. Where they compare against `password_change_unknown()` use `ClientError::password_change_unknown()`.

Delete those tests from the desktop files.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p havenkeys-client account`
Expected: FAIL to compile — `account_field`, `change_outcome`, … not found.

- [ ] **Step 3: Write `account.rs`**

The bodies below are the desktop's, translated with the same substitutions as Task 4 plus: `state.device.lock()…` → `self.device()?`; `tauri::async_runtime::spawn_blocking` → `tokio::task::spawn_blocking`; `tauri::async_runtime::spawn` → `tokio::spawn` on an `Arc` clone; `sync::DEVICE_NAME` → `self.config.device_name`; `state.arm_auto_lock(m); state.notify_unlocked();` → `self.events.unlocked(m);`; `app.emit(CONNECTIVITY_EVENT, true)` → `self.events.connectivity(true)`.

```rust
//! The account: activation, signing in on a second device, unlocking, the
//! master password change, devices and the Account item.
//!
//! Every vault belongs to an account, and activation is the only way one is
//! created (spec 2026-09-20 §5). The cryptography all happens here on the
//! device; the server never sees the master password, the Secret Key, the
//! KEK or the vault key.

use crate::client::HavenClient;
use crate::device::Device;
use crate::error::{ClientError, ClientResult};
use crate::key_store::Storage;
use crate::now_ms;
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::{AccountRecord, KeyScheme};
use havenkeys_core::sync::{encode_header_for, prepare_sign_in};
use havenkeys_core::vault::{
    self, prepare_new_account_vault, PreparedVault, VaultService, VaultStatus,
};
use havenkeys_core::SecretString;
use havenkeys_sync_client::{invite as invite_parser, Activation, CredentialChange, SyncError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;
use uuid::Uuid;

/// How long the unlock fallback waits for the server's KDF parameters.
const FALLBACK_PARAMS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    /// "account_bound"; null without a vault.
    pub key_scheme: Option<KeyScheme>,
    /// The vault needs a Secret Key and this device does not have it.
    pub needs_secret_key: bool,
    pub online: bool,
    /// Where the Secret Key is kept: "keychain", "file" or "none".
    pub secret_key_storage: Storage,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub email: String,
    pub server_url: String,
    pub account_id: Uuid,
    pub online: bool,
    /// Unix ms of the last successful pull, or null if none yet.
    pub last_synced_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEntry {
    pub id: Uuid,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub current: bool,
}

/// The values of the HavenKeys Account item (spec 2026-09-29-account-item).
/// The item is virtual: built from the account record and this device's
/// Secret Key when shown, never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountField {
    Email,
    Server,
    AccountId,
    SecretKey,
}

impl HavenClient {
    /// Safe while locked: reveals no secrets. May wait on the platform
    /// store, so shells call it off their UI thread.
    pub fn device_status(&self) -> ClientResult<DeviceStatus> {
        // Vault before device, as everywhere.
        let (key_scheme, account) = {
            let v = self.vault()?;
            (v.key_scheme()?, v.account()?)
        };
        let uses_secret_key = key_scheme.is_some_and(KeyScheme::uses_secret_key);
        let (needs_secret_key, secret_key_storage) = match account {
            Some(a) => {
                let status = self.device()?.key_status(a.account_id);
                (uses_secret_key && status.missing, status.storage)
            }
            None => (false, Storage::None),
        };
        Ok(DeviceStatus {
            key_scheme,
            needs_secret_key,
            online: self.is_online(),
            secret_key_storage,
        })
    }

    /// No secrets; safe while locked.
    pub fn account_status(&self) -> ClientResult<Option<AccountStatus>> {
        let Some(account) = self.vault()?.account()? else {
            return Ok(None);
        };
        Ok(Some(AccountStatus {
            email: account.email,
            server_url: account.server_url,
            account_id: account.account_id,
            online: self.is_online(),
            last_synced_at: account.last_synced_at,
        }))
    }

    /// First run: the invite and a new master password.
    ///
    /// Keys are derived and the header is built in memory, the server is
    /// asked to accept them, and only then is anything written to disk — so
    /// a refused activation leaves this device with no vault at all.
    pub async fn activate(
        self: &Arc<Self>,
        invite: String,
        password: SecretString,
    ) -> ClientResult<VaultStatus> {
        if self.vault()?.key_scheme()?.is_some() {
            return Err(havenkeys_core::Error::VaultExists.into());
        }
        vault::check_new_master_password(&password)?;

        // The server decodes and burns the invite itself, so the original
        // string is what travels; the fields read here only tell this device
        // which account and server to derive against.
        let raw_invite = invite.trim().to_string();
        let invite = invite_parser::decode(&raw_invite)?;
        let email = NormalizedEmail::parse(&invite.email)?;
        let account = AccountRef::new(invite.account, email);
        let server_url = invite.server.clone();
        let server = self.server_for(&server_url)?;

        let kdf = KdfParams::generate()?;
        let for_derivation = account.clone();
        let made = spawn_blocking(move || {
            prepare_new_account_vault(&password, &for_derivation, kdf, now_ms())
        })
        .await
        .map_err(|_| ClientError::internal())??;

        let header = encode_header_for(&made.prepared)?;
        let vault_id = made.prepared.vault_id();
        let kdf = made.prepared.kdf().clone();

        // Written before the server is asked, not after. If activation
        // succeeds and anything below it fails, the account exists, its
        // invite is spent, and the only copy of the Secret Key would
        // otherwise be gone with it. A key stored for an activation that
        // never completed is harmless.
        self.device()?
            .set_secret_key(account.id, &made.secret_key)
            .map_err(|_| ClientError::file())?;

        server
            .activate(Activation {
                email: account.email.as_str(),
                invite: &raw_invite,
                kdf: &kdf,
                auth_key: &made.auth_key,
                vault_id,
                header: &header,
            })
            .await?;

        let record = new_account_record(&account, server_url, 0);
        self.create_vault(made.prepared, &record)?;
        let status = self.vault()?.status()?;

        // Sign in so the vault is immediately writable. A failure here leaves
        // a perfectly good offline vault.
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let _ = client.connect(made.auth_key).await;
        });
        Ok(status)
    }

    /// A second device: server, email, master password and the Secret Key
    /// from the Emergency Kit. Nothing is written to disk until the header
    /// the server serves has been opened with the keys derived here — which
    /// is what proves all four inputs are right.
    pub async fn sign_in(
        self: &Arc<Self>,
        server_url: String,
        email: String,
        password: SecretString,
        secret_key: Option<SecretString>,
    ) -> ClientResult<VaultStatus> {
        if self.vault()?.key_scheme()?.is_some() {
            return Err(havenkeys_core::Error::VaultExists.into());
        }
        let email = NormalizedEmail::parse(&email)?;
        let typed = match secret_key {
            Some(typed) if !typed.is_empty() => Some(SecretKey::parse(typed.expose())?),
            _ => None,
        };
        let server_url = server_url.trim().trim_end_matches('/').to_string();
        let server = self.server_for(&server_url)?;
        let device_id = self.device_id()?;

        // Public by design: a device needs them before it can derive
        // anything, and the server answers the same way for an unknown
        // address.
        let params = server.auth_params(email.as_str()).await?;
        let account = AccountRef::new(params.account_id, email);

        // A key already on this device for this account is used when none is
        // typed: that is what makes an activation interrupted after the
        // server accepted it recoverable.
        let secret_key = match typed {
            Some(k) => k,
            None => self
                .device()?
                .secret_key(account.id)
                .ok_or(havenkeys_core::Error::SecretKeyRequired)?,
        };

        let for_auth = account.clone();
        let password_for_auth = password.clone();
        let kdf = params.kdf.clone();
        let (auth_key, secret_key) = spawn_blocking(move || {
            let derived = vault::derive_auth_key(&password_for_auth, &secret_key, &kdf, &for_auth);
            (derived, secret_key)
        })
        .await
        .map_err(|_| ClientError::internal())?;
        let auth_key = auth_key?;

        let session = server
            .login(
                account.email.as_str(),
                &auth_key,
                account.id,
                device_id,
                self.config.device_name,
            )
            .await
            .map_err(|_| ClientError::sign_in_failed())?;
        let header = server
            .header(&session)
            .await
            .map_err(|_| ClientError::sign_in_failed())?;

        let for_unwrap = account.clone();
        let (prepared, secret_key) = spawn_blocking(move || {
            let prepared = prepare_sign_in(&header.bytes, &password, &secret_key, &for_unwrap);
            (prepared, secret_key)
        })
        .await
        .map_err(|_| ClientError::internal())?;
        // Wrong password, wrong Secret Key, a header that does not open: one
        // message. The device never says which.
        let (prepared, _) = prepared.map_err(|_| ClientError::sign_in_failed())?;
        let header_revision = prepared.header_revision() as i64;

        let record = new_account_record(&account, server_url, header_revision);
        self.create_vault(prepared, &record)?;
        self.device()?
            .set_secret_key(account.id, &secret_key)
            .map_err(|_| ClientError::file())?;
        self.set_online(session);
        let status = self.vault()?.status()?;
        self.events.connectivity(true);

        // Catch up in the background: a large vault should not hold the
        // sign-in screen open.
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let _ = client.sync_now().await;
        });
        Ok(status)
    }

    /// Store the new vault, which opens unlocked, and start its auto-lock.
    fn create_vault(&self, prepared: PreparedVault, record: &AccountRecord) -> ClientResult<()> {
        let mut vault = self.vault()?;
        vault.create_account_vault(prepared, record)?;
        let minutes = vault.settings()?.auto_lock_minutes;
        self.events.unlocked(minutes);
        Ok(())
    }

    /// The account this vault belongs to, from the local store (never from
    /// the caller).
    fn vault_account(&self) -> ClientResult<AccountRef> {
        Ok(self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?
            .to_ref()?)
    }
```

Continue the same `impl HavenClient` block with `unlock`, `remember_typed_secret_key`, `unlock_from_server`, `change_master_password`, `list_devices`, `revoke_device`, `sign_out` and `account_value`. Each is the desktop function of the same name (`commands.rs:48-286, 324-411`, `account.rs:346-387, 431-439`), translated with the substitutions listed at the top of this step and these specific ones:

| Desktop | Client |
|---|---|
| `let state = app.state::<AppState>();` | (delete; use `self`) |
| `vault_account(&state)?` | `self.vault_account()?` |
| `state.lock(&app, "error")` | `self.lock("error")` |
| `state.arm_auto_lock(minutes); state.notify_unlocked();` | `self.events.unlocked(minutes);` |
| `unlock_from_server(&app, pw, sk, epoch)` | `self.unlock_from_server(pw, sk, epoch)` |
| `remember_typed_secret_key(&state, id, text)` | `self.remember_typed_secret_key(id, text)` |
| `return Err(keychain_unavailable())` | `return Err(ClientError::keychain_unavailable())` |
| `tauri::async_runtime::spawn(async move { let _ = sync::connect(&handle, auth_key).await; })` | `let client = Arc::clone(self); tokio::spawn(async move { let _ = client.connect(auth_key).await; });` |
| `tauri::async_runtime::spawn(async move { let _ = sync::sync_now(&handle).await; })` | `let client = Arc::clone(self); tokio::spawn(async move { let _ = client.sync_now().await; });` |
| `sync::sync_now(&app).await?` | `self.sync_now().await?` |
| `sync::failed(&app, e)` | `self.failed(e)` |
| `client.` (the `sync::client(&state)?` value) | `server.` from `let server = self.server()?;` |
| `state.touch();` | (delete; the desktop wrapper touches) |
| `if id == state.device_id()? { sync::go_offline(&app); }` | `if id == self.device_id()? { self.go_offline(); }` |
| `sign_out`: `state.lock(&app, "user"); let _ = app.emit(sync::CONNECTIVITY_EVENT, false);` | `self.lock("user"); self.events.connectivity(false);` |
| `account_value`: `state.touch(); … state.device.lock()…` | no touch; `let mut device = self.device()?;` |

`unlock` signature: `pub async fn unlock(self: &Arc<Self>, password: SecretString, secret_key: Option<SecretString>) -> ClientResult<VaultStatus>`; `unlock_from_server(&self, password: SecretString, secret_key_text: SecretString, epoch: u64) -> ClientResult<VaultStatus>` needs to spawn too, so it takes `self: &Arc<Self>`. `tokio::time::timeout` replaces nothing — it is already tokio. `credential_change_conflict(err)` returns `matches!(err, SyncError::Conflict(_)).then(ClientError::password_changed_elsewhere)`.

After the `impl` block, the free functions `new_account_record`, `unlocked_account`, `account_field`, the `ChangeOutcome` enum and `change_outcome`, unchanged from the desktop.

In `crates/havenkeys-client/src/lib.rs` add `mod account;` and `pub use account::{AccountField, AccountStatus, DeviceEntry, DeviceStatus};`.

- [ ] **Step 4: Run the client tests**

Run: `cargo test -p havenkeys-client`
Expected: PASS (5 account-item tests, 4 credential-change tests and the earlier ones).

- [ ] **Step 5: Reduce the desktop to wrappers**

`apps/desktop/src-tauri/src/account.rs`:

```rust
//! Account commands: thin wrappers over `havenkeys_client`.

use crate::commands::{copy_to_clipboard, CopyResult};
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_client::{AccountField, AccountStatus, DeviceEntry, DeviceStatus};
use havenkeys_core::vault::VaultStatus;
use havenkeys_core::SecretString;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

/// Run `f` on a blocking thread. For commands that touch the keychain: a
/// sync command runs on the main thread, where a keychain waiting on D-Bus
/// or a prompt would freeze the window.
pub(crate) async fn off_main_thread<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|_| CmdError::internal())?
}

#[tauri::command]
pub async fn device_status(app: AppHandle) -> CmdResult<DeviceStatus> {
    off_main_thread(app, |state| state.client().device_status()).await
}

#[tauri::command]
pub fn account_status(state: State<'_, AppState>) -> CmdResult<Option<AccountStatus>> {
    state.client().account_status()
}

#[tauri::command]
pub async fn activate_account(
    app: AppHandle,
    invite: String,
    password: SecretString,
) -> CmdResult<VaultStatus> {
    let client = app.state::<AppState>().client().clone();
    client.activate(invite, password).await
}

#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    server_url: String,
    email: String,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let client = app.state::<AppState>().client().clone();
    client.sign_in(server_url, email, password, secret_key).await
}

#[tauri::command]
pub async fn list_devices(app: AppHandle) -> CmdResult<Vec<DeviceEntry>> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.list_devices().await
}

/// Cut a device off. Revoking this one signs it out immediately.
#[tauri::command]
pub async fn revoke_device(app: AppHandle, id: Uuid) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.revoke_device(id).await
}

/// End this device's session and lock the vault.
#[tauri::command]
pub async fn sign_out(app: AppHandle) -> CmdResult<()> {
    let client = app.state::<AppState>().client().clone();
    client.sign_out().await
}

/// The Secret Key, on an explicit reveal from the Account item.
#[tauri::command]
pub async fn reveal_account_secret_key(app: AppHandle) -> CmdResult<SecretString> {
    off_main_thread(app, |state| {
        state.touch();
        state
            .client()
            .account_value(AccountField::SecretKey)
            .map(|(value, _)| value)
    })
    .await
}

/// Copy one of the Account item's values. It goes from here to the
/// clipboard, cleared after the usual delay; the renderer never holds it.
#[tauri::command]
pub async fn copy_account_field(app: AppHandle, field: AccountField) -> CmdResult<CopyResult> {
    off_main_thread(app, move |state| {
        state.touch();
        let (value, seconds) = state.client().account_value(field)?;
        copy_to_clipboard(state, &value, seconds)
    })
    .await
}
```

In `commands.rs`:

```rust
#[tauri::command]
pub async fn unlock_vault(
    app: AppHandle,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let client = app.state::<AppState>().client().clone();
    client.unlock(password, secret_key).await
}

#[tauri::command]
pub async fn change_master_password(
    app: AppHandle,
    current: SecretString,
    new: SecretString,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.change_master_password(current, new).await
}
```

and `sync_now` / `resync_vault` call `client.sync_now()` (after `touch`, and for `resync_vault` after `require_online` and `reset_sync_cursor`, as today). Delete `vault_account`, `keychain_unavailable`, `remember_typed_secret_key`, `unlock_from_server`, `ChangeOutcome`, `change_outcome`, `password_change_unknown`, `credential_change_conflict`, `FALLBACK_PARAMS_TIMEOUT`, and the imports that become unused (`cargo clippy` lists them).

`emergency_kit.rs`: `state.client().device()?` replaces the device lock (done in Task 3); nothing else changes.

- [ ] **Step 6: Run everything**

Run: `cargo test -p havenkeys-client && cargo test -p havenkeys-desktop && cargo clippy -p havenkeys-client -p havenkeys-desktop -- -D warnings`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A crates/havenkeys-client apps/desktop/src-tauri
git commit -m "refactor(client): activation, sign-in, unlock and devices move to HavenClient"
```

---

### Task 6: Device removal

**Files:**
- Create: `crates/havenkeys-client/src/removal.rs`
- Modify: `apps/desktop/src-tauri/src/removal.rs` (thin command only)
- Modify: `crates/havenkeys-client/src/client.rs`, `sync.rs` (restore `pub(crate)`)

**Interfaces:**
- Consumes: Tasks 3–5.
- Produces: `impl HavenClient { pub async fn remove_device(self: &Arc<Self>, confirmation: String) -> ClientResult<()> }` — on success and on partial failure after the file is set aside, `events.removed(keychain_cleared)` has been called.

- [ ] **Step 1: Move the tests and add the offline-removal test**

Create `crates/havenkeys-client/src/removal.rs` with the desktop's `#[cfg(test)] mod tests` (`removal.rs:233-339`, unchanged) plus:

```rust
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
            .any(|e| e.file_name().to_string_lossy().starts_with("vault.sqlite3.removed-"));
        assert!(set_aside);
    }

    #[tokio::test]
    async fn a_locked_vault_cannot_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = crate::client::tests::client_in(dir.path());
        let err = client.remove_device("user@example.com".into()).await.unwrap_err();
        assert_eq!(err.code, "locked");
    }
```

Check `Store::open` and `VaultService::replace_store` signatures in `crates/havenkeys-core/src/store.rs` and `vault.rs` before running; adjust the setup lines to match them.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p havenkeys-client removal`
Expected: FAIL to compile — no `remove_device`.

- [ ] **Step 3: Write `removal.rs`**

Above the tests: the desktop's `confirms`, `set_aside`, `restore_journal_files`, `civil_from_days`, `stamp_for_secs`, `chrono_free_utc_stamp` unchanged, and:

```rust
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
    /// deleted: if the old server is gone, it is the only copy left.
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
        // waits until the file is set aside: revoking first and then failing
        // the rename would leave an intact vault on a device the server
        // refuses forever.
        let revoke = match (self.session(), self.server(), self.device_id()) {
            (Ok(session), Ok(server), Ok(device_id)) => Some((session, server, device_id)),
            _ => None,
        };
        self.lock("user");
        self.forget_server();
        self.events.connectivity(false);

        let path = self.config.vault_path.clone();
        let stamp = chrono_free_utc_stamp();
        {
            let mut vault = self.vault()?;
            let old = vault.replace_store(Store::open_in_memory()?);
            drop(old); // closes the connection before the rename
            if set_aside(&path, &stamp).is_err() {
                // The rename never happened (or was rolled back): reopen the
                // original and report failure.
                let _ = vault.replace_store(Store::open(&path)?);
                return Err(ClientError::file());
            }
        }

        // Best effort: the server may be gone, which may be why this is
        // happening. No vault guard is held here.
        if let Some((session, server, device_id)) = revoke {
            let _ = server.revoke_device(&session, device_id).await;
        }

        // Past this point the file is renamed: every remaining step runs
        // regardless of earlier failures, and the first error is reported
        // after `forget` ran and the shell was told.
        let mut first_error: Option<ClientError> = None;
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
```

Keep the desktop's comments on `set_aside` and the date helpers as they are. Add `mod removal;` to `lib.rs`. If `vault.replace_store` returns a value other than the old store, match the desktop's usage exactly.

- [ ] **Step 4: Desktop wrapper and visibility cleanup**

`apps/desktop/src-tauri/src/removal.rs` becomes:

```rust
//! Remove this device: a thin wrapper over `havenkeys_client`. The
//! `vault://removed` event is sent by `DesktopEvents`.

use crate::state::{AppState, CmdResult};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn remove_device(app: AppHandle, confirmation: String) -> CmdResult<()> {
    let client = app.state::<AppState>().client().clone();
    client.remove_device(confirmation).await
}
```

Delete `pub(crate) const VAULT_FILE` usage from `removal.rs` (it stays in `lib.rs` for `vault_path`). In `crates/havenkeys-client/src/client.rs` make `set_online` and `mark_sync_attempt` `pub(crate)` again and in `sync.rs` make `failed` `pub(crate)`; delete the three `// pub until Task 6` notes. `grep -rn "pub until Task" crates apps` must print nothing.

- [ ] **Step 5: Run everything**

Run: `cargo test -p havenkeys-client && cargo test -p havenkeys-desktop && cargo clippy -p havenkeys-client -p havenkeys-desktop -- -D warnings`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A crates/havenkeys-client apps/desktop/src-tauri
git commit -m "refactor(client): device removal moves to HavenClient"
```

---

### Task 7: Round trip against the real server

**Files:**
- Create: `crates/havenkeys-client/tests/round_trip.rs`
- Modify: `scripts/test-server.sh` (add `-p havenkeys-client`)

**Interfaces:**
- Consumes: the whole public `HavenClient` API.

- [ ] **Step 1: Write the test**

`crates/havenkeys-client/tests/round_trip.rs` — copy the `Server` helper (`start`, `invite`, `cleanup`) and `exec` from `crates/havenkeys-sync-client/tests/round_trip.rs` verbatim, then:

```rust
//! Two devices through HavenClient, against the real server and a real
//! Postgres. Needs Postgres: `scripts/test-server.sh` starts one.

use havenkeys_client::device::Device;
use havenkeys_client::key_store::MemoryKeyStore;
use havenkeys_client::{ClientConfig, ClientEvents, HavenClient};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::store::Store;
use havenkeys_core::sync::SyncReport;
use havenkeys_core::vault::VaultService;
use havenkeys_core::SecretString;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PASSWORD: &str = "correct horse battery staple";

/// Records events, and checks on `unlocked` that the vault is held — the
/// contract `ClientEvents` documents.
struct Probe {
    vault: Arc<Mutex<VaultService>>,
    seen: Mutex<Vec<String>>,
}

impl ClientEvents for Probe {
    fn unlocked(&self, _: u32) {
        let held = self.vault.try_lock().is_err();
        self.seen.lock().unwrap().push(format!("unlocked:held={held}"));
    }
    fn locked(&self, reason: &'static str, was_open: bool) {
        self.seen.lock().unwrap().push(format!("locked:{reason}:{was_open}"));
    }
    fn connectivity(&self, online: bool) {
        self.seen.lock().unwrap().push(format!("online:{online}"));
    }
    fn signed_out(&self) {}
    fn synced(&self, _: SyncReport) {}
    fn items_changed(&self) {}
    fn removed(&self, _: bool) {}
}

fn device(dir: &std::path::Path) -> (Arc<HavenClient>, Arc<Probe>) {
    let vault = Arc::new(Mutex::new(VaultService::new(Store::open_in_memory().unwrap())));
    let probe = Arc::new(Probe {
        vault: vault.clone(),
        seen: Mutex::new(Vec::new()),
    });
    let client = HavenClient::new(
        vault,
        Device::load(dir, Box::new(MemoryKeyStore::default())),
        None,
        probe.clone(),
        ClientConfig {
            device_name: "Test",
            vault_path: dir.join("vault.sqlite3"),
        },
    );
    (client, probe)
}

async fn until(what: impl Fn() -> bool) {
    for _ in 0..200 {
        if what() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not reached in 5 s");
}

fn login(title: &str) -> ItemInput {
    ItemInput {
        item_type: ItemType::Login,
        title: title.into(),
        username: Some("me@example.com".into()),
        urls: vec![UrlRule {
            url: "github.com".into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(SecretString::from("hunter2-but-longer")),
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: None,
        sections: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn two_devices_share_one_vault_through_the_client() {
    let server = Server::start().await;
    let invite = server.invite("user@example.com").await;

    let dir_a = tempfile::tempdir().unwrap();
    let (a, probe_a) = device(dir_a.path());
    a.activate(invite, SecretString::from(PASSWORD)).await.unwrap();
    until(|| a.is_online()).await;
    assert!(probe_a
        .seen
        .lock()
        .unwrap()
        .contains(&"unlocked:held=true".to_string()));

    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("GitHub"), havenkeys_client::now_ms())
        .unwrap();
    a.push(staged).await.unwrap();

    let secret_key = a
        .device()
        .unwrap()
        .secret_key_text(a.account_status().unwrap().unwrap().account_id)
        .unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, _) = device(dir_b.path());
    b.sign_in(
        server.base.clone(),
        "user@example.com".into(),
        SecretString::from(PASSWORD),
        Some(secret_key),
    )
    .await
    .unwrap();
    b.sync_now().await.unwrap();
    let titles: Vec<String> = b
        .vault()
        .unwrap()
        .list_items()
        .unwrap()
        .into_iter()
        .map(|o| o.title)
        .collect();
    assert!(titles.contains(&"GitHub".to_string()));

    b.lock("user");
    assert!(!b.is_online());
    assert!(b
        .vault()
        .unwrap()
        .stage_create(login("Nope"), havenkeys_client::now_ms())
        .is_err());

    b.unlock(SecretString::from(PASSWORD), None).await.unwrap();
    until(|| b.is_online()).await;

    let wrong = b.unlock(SecretString::from("not the password"), None).await;
    assert!(wrong.is_err());

    server.cleanup().await;
}
```

Check `stage_create` and `list_items` names against `crates/havenkeys-core/src/vault.rs` (`grep -n "pub fn stage_create\|pub fn list_items\|pub fn list" crates/havenkeys-core/src/vault.rs`) and use the real names and return types. The second `unlock` call is made while unlocked: if the core refuses unlocking an unlocked vault with a specific error, assert that error instead of a bare `is_err()`.

- [ ] **Step 2: Add the crate to the server test script**

In `scripts/test-server.sh` change the last line to:

```bash
cargo test -p havenkeys-server -p havenkeys-sync-client -p havenkeys-client "$@"
```

- [ ] **Step 3: Run it**

Run: `scripts/test-server.sh --test round_trip`
Expected: PASS for `havenkeys-client`'s and `havenkeys-sync-client`'s round trips. If Docker is unavailable, say so in the task report rather than skipping silently.

- [ ] **Step 4: Commit**

```bash
git add crates/havenkeys-client/tests scripts/test-server.sh
git commit -m "test(client): two devices round trip through HavenClient"
```

---

### Task 8: Docs and full verification

**Files:**
- Modify: `docs/architecture.md` (Components tree and "The server and the local replica" diagram)
- Modify: `docs/server-sync.md` (where it names `apps/desktop/src-tauri/src/sync.rs` or `account.rs`)
- Modify: `docs/development.md` (test commands)

- [ ] **Step 1: Update the docs**

In `docs/architecture.md`, add to the crates tree after `havenkeys-sync-client/`:

```text
│   ├── havenkeys-client/      account, session and sync for every app: activation, unlock,
│   │                          sync, writes, devices, removal (no Tauri, no UI)
```

and in the replica diagram replace `Tauri commands ──── AppState::require_online()` with `Tauri commands ──── HavenClient (havenkeys-client)`. Replace every mention of `apps/desktop/src-tauri/src/sync.rs`, `account.rs`, `device.rs`, `removal.rs` as the home of that logic with the `crates/havenkeys-client/src/` file (`grep -rn "src-tauri/src/\(sync\|account\|device\|removal\|secret_store\)" docs README.md`). In `docs/development.md`, list `cargo test -p havenkeys-client` and note that `scripts/test-server.sh` now runs its round trip.

- [ ] **Step 2: Full verification**

Run, in order:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/test-server.sh
cargo deny check
grep -rn "tauri" crates/havenkeys-client && echo "FAIL: tauri in client" || echo "ok: no tauri"
```

Expected: all pass; the last line prints `ok: no tauri`.

- [ ] **Step 3: Run the desktop app**

Run: `pnpm --filter desktop tauri dev` (check `docs/development.md` for the exact command). With an existing vault: unlock (password only — the Secret Key must come from the keychain), see the online indicator, add a login, lock, unlock, change auto-lock, open Account → Devices. Expected: identical behaviour to before. Report what was exercised.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: havenkeys-client in the architecture and development docs"
```
