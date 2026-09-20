# Core: server-authoritative vault — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `havenkeys-core` into the client half of a server-authoritative
vault: one key scheme, a schema-4 read-only replica with per-item revisions, a
staged write path that a server acknowledges, and a pull applier with no merge
logic — deleting folder sync, key schemes 1 and 2, and the whole conflict
machinery on the way.

**Architecture:** The server becomes the single writer; the local SQLite
becomes an encrypted replica that follows it. Writes are staged under the
vault lock (`stage_write`), sent by a later crate, and committed with the
revision the server assigned (`commit_write`). Pulls are applied in cursor
order as plain upserts and deletions — no `decide_merge`, no `dirty` flags, no
tombstones on the client.

**Tech Stack:** Rust 2021 (MSRV 1.88), `rusqlite`, `uuid`, `zeroize`; the
existing `havenkeys-core` crypto stack unchanged. No new dependencies. No
network in this crate.

**Spec:** `docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`

## Known end state of this step

This step deliberately leaves the application unable to create a vault or save
an item, because both now require the server, which does not exist until steps
2 and 3 of the spec's §13. The desktop compiles, opens an existing schema-4
replica, and is permanently in the `Offline` state that Task 7 introduces —
which is the *final* behaviour for a disconnected device, not a stand-in. Vault
creation is exercised by core tests only until step 3 lands.

Do not invent a temporary local activation path to work around this. It would
produce vaults bound to an account no server knows.

## Global Constraints

Copied verbatim from `CLAUDE.md` and the spec; every task's requirements
include these.

- **Never invent cryptography.** No new construction; this plan changes no
  derivation. `derive_kek_v3` and `derive_auth_key_from_master` stay exactly
  as they are.
- **Never log secrets.** No `println!`, `eprintln!`, `dbg!` or logging of
  passwords, keys, blobs, item content or emails. `tests/no_logging.rs` must
  keep passing.
- **Never stringify objects containing sensitive fields.** New types holding
  blobs or secrets get a manual `Debug` that redacts, following
  `FillCredentials` in `vault.rs`.
- **Errors must not leak secrets.** New `Error` variants carry `&'static str`
  messages only, like every existing variant.
- **Validate in the core.** Nothing trusts the renderer, the extension or the
  server.
- **`havenkeys-core` has no network dependency.** Adding one is a plan
  violation.
- **MSRV 1.88**, edition 2021, `cargo fmt` clean, `cargo clippy --all-targets
  -- -D warnings` clean.
- **Every task ends green:** `cargo test` (workspace default members),
  `cargo clippy -p havenkeys-core -p havenkeys-desktop --all-targets -- -D
  warnings`, and `pnpm -r typecheck` when TypeScript changed.
- **Commit after every task.** End commit messages with
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.

## File structure

**Modified**

| File | Responsibility after this step |
|---|---|
| `crates/havenkeys-core/src/store.rs` | Schema 4 only: header, items (with `revision`), account, settings. No tombstones, no `dirty`, no migrations. |
| `crates/havenkeys-core/src/vault.rs` | One key scheme; `stage_write`/`commit_write`; no folder join, no scheme upgrades. |
| `crates/havenkeys-core/src/sync.rs` | Header encode/adopt and the pull applier. Roughly a third of its current size. |
| `crates/havenkeys-core/src/crypto/keys.rs` | v3 derivations only. |
| `crates/havenkeys-core/src/error.rs` | Adds `Offline` and `ItemChangedElsewhere`. |
| `crates/havenkeys-core/tests/common/mod.rs` | One vault helper: an activated account vault. |
| `apps/desktop/src-tauri/src/{account,commands,state,lib}.rs` | No folder sync, no Secret Key set-up, no vault creation; connectivity state and the read-only gate. |
| `apps/desktop/src/{App.tsx,lib/types.ts,lib/api.ts,views/SyncSection.tsx,views/UnlockScreen.tsx}` | No folder UI, no Secret Key set-up; offline banner. |

**Deleted**

| File | Why |
|---|---|
| `crates/havenkeys-core/src/sync/folder.rs` (and the `folder` module) | Folder transport removed. |
| `crates/havenkeys-core/tests/sync.rs` | Folder-specific suite; the header and apply cases move to `tests/replica.rs`. |
| `crates/havenkeys-core/tests/delta_sync.rs` | Replaced by `tests/replica.rs`. |
| `apps/desktop/src-tauri/src/sync.rs` | The folder sync runner. |

**Created**

| File | Responsibility |
|---|---|
| `crates/havenkeys-core/tests/replica.rs` | Pull application, header adoption, hostile-server cases. |
| `crates/havenkeys-core/tests/writes.rs` | `stage_write`/`commit_write` and the lock-epoch rules. |

---

### Task 1: Remove folder sync, end to end

Folder sync is the first transport to go (spec §11). It is removed before
anything else so later tasks are not rewriting code that is about to be
deleted. The desktop is changed in the same task because the workspace must
compile.

**Files:**
- Delete: `crates/havenkeys-core/src/sync/folder.rs` (or the `pub mod folder`
  block inside `src/sync.rs`, whichever holds it)
- Delete: `crates/havenkeys-core/tests/sync.rs`
- Delete: `apps/desktop/src-tauri/src/sync.rs`
- Modify: `crates/havenkeys-core/src/sync.rs` (remove `prepare_join`,
  `SyncReport::{unreadable_devices, header_rejected}`)
- Modify: `crates/havenkeys-core/src/lib.rs` (module list, if `folder` is
  declared there)
- Modify: `apps/desktop/src-tauri/src/account.rs` (remove
  `choose_sync_folder`, `sync_now`, `stop_sync`, `pick_join_folder`,
  `join_synced_vault`, `folder_name`, `pick_folder`, `pick_folder_async`,
  `DeviceStatus::{sync_folder, last_sync}`)
- Modify: `apps/desktop/src-tauri/src/{lib.rs,state.rs,device.rs}` (drop the
  command registrations, the `sync` field on `AppState`, and
  `Device::{sync_folder, set_sync_folder}`)
- Modify: `apps/desktop/src-tauri/build.rs` and
  `apps/desktop/src-tauri/capabilities/main.json` (the command allowlist —
  the module header of `commands.rs` says these three stay in sync)
- Modify: `apps/desktop/src/views/SyncSection.tsx`,
  `apps/desktop/src/lib/{types.ts,api.ts}`, `apps/desktop/src/App.tsx`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `SyncReport { added: usize, updated: usize, deleted: usize,
  skipped_items: usize, header_adopted: bool }` — the shape every later task
  asserts on.

- [ ] **Step 1: Find every reference before deleting anything**

```bash
cd /home/sams/havenkeys
grep -rn "folder\|prepare_join\|unreadable_devices\|header_rejected" \
  crates/havenkeys-core/src crates/havenkeys-core/tests \
  apps/desktop/src-tauri/src apps/desktop/src | grep -v node_modules
```

Expected: hits in the files listed above and nowhere else. Anything outside
that list is a surprise — read it before continuing.

- [ ] **Step 2: Delete the core folder module and `prepare_join`**

Remove the `folder` module and the `prepare_join` function from
`crates/havenkeys-core/src/sync.rs`, and the `unreadable_devices` and
`header_rejected` fields from `SyncReport` together with every assignment to
them.

- [ ] **Step 3: Delete the folder test suite**

```bash
git rm crates/havenkeys-core/tests/sync.rs
```

- [ ] **Step 4: Run the core tests**

Run: `cargo test -p havenkeys-core`
Expected: compilation errors only in `tests/delta_sync.rs`, `tests/fuzz.rs`
and `tests/common/mod.rs` where they touch the removed items. Fix those by
deleting the affected assertions and helpers (`second_device` keeps working;
it does not use the folder). Re-run until green.

- [ ] **Step 5: Delete the desktop folder runner and commands**

```bash
git rm apps/desktop/src-tauri/src/sync.rs
```

Then remove from `apps/desktop/src-tauri/src/account.rs` the five commands and
three helpers listed under **Files**, and shrink `DeviceStatus` to:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    /// "account_bound"; null without a vault.
    key_scheme: Option<KeyScheme>,
    /// The vault needs a Secret Key and this device does not have it.
    needs_secret_key: bool,
}
```

- [ ] **Step 6: Remove the registrations and the allowlist entries**

In `apps/desktop/src-tauri/src/lib.rs`, drop `account::choose_sync_folder`,
`account::sync_now`, `account::stop_sync`, `account::pick_join_folder` and
`account::join_synced_vault` from `generate_handler!`, and remove the `sync`
field from `AppState` in `state.rs` along with every `state.sync.request()`
call. Mirror the command removals in `build.rs` and
`capabilities/main.json`.

- [ ] **Step 7: Remove the folder UI**

In `apps/desktop/src/views/SyncSection.tsx`, delete everything gated on
`canSyncThroughFolder` and the `describe(device.lastSync)` line, leaving the
Emergency Kit block. In `lib/types.ts` drop `SyncReport`, `SyncStatus`, and
the `syncFolder`/`lastSync` fields of `DeviceStatus`; in `lib/api.ts` drop the
five removed commands. Remove the "Set up from a sync folder" branch from
`App.tsx`.

- [ ] **Step 8: Verify the whole tree**

Run:
```bash
cargo test
cargo clippy -p havenkeys-core -p havenkeys-desktop --all-targets -- -D warnings
pnpm -r typecheck
```
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
refactor: remove folder sync

The server becomes the only transport (spec 2026-09-20 §11). Removes
sync::folder, prepare_join, the folder fields on SyncReport, the five
desktop folder commands and the folder UI.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: One key scheme in the core

Key schemes 1 and 2 leave the product (spec §4). `KeyScheme` keeps its enum
shape and its serde form so the header stays forward-compatible, but has one
variant.

**Files:**
- Modify: `crates/havenkeys-core/src/store.rs` (`KeyScheme`, `to_db`,
  `from_db`, `uses_secret_key`)
- Modify: `crates/havenkeys-core/src/crypto/keys.rs` (delete `derive_kek`,
  `derive_kek_with_secret_key` and their unit tests)
- Modify: `crates/havenkeys-core/src/vault.rs` (delete `derive_kek_for`'s
  dead arms, `prepare_new_vault`, `prepare_new_vault_with_secret_key`,
  `prepare`, `unlock`, `UnlockTicket::{derive, derive_with_secret_key}`,
  `RekeyTicket::{derive, derive_with_secret_key, derive_secret_key_upgrade,
  derive_account_upgrade, unwrap}`, `VaultService::{create_vault,
  commit_account_upgrade, change_master_password}`)
- Modify: `crates/havenkeys-core/tests/{common/mod.rs,vault.rs,account.rs,security.rs,fuzz.rs}`

**Interfaces:**
- Consumes: `SyncReport` from Task 1.
- Produces:
  - `KeyScheme::AccountBound` as the only variant; `KeyScheme::uses_secret_key(self) -> bool` always `true`.
  - `UnlockTicket::derive_for_account(&self, &SecretString, &SecretKey, &AccountRef) -> Result<UnlockKey>` (unchanged signature, now the only one).
  - `RekeyTicket::derive_for_account(&self, current: &SecretString, new: &SecretString, KdfParams, &SecretKey, &AccountRef) -> Result<Rekeyed>` (unchanged).
  - `VaultService::create_account_vault(&mut self, PreparedVault, &AccountRecord) -> Result<()>` (unchanged).
  - `VaultService::change_master_password_for_account(&mut self, current: &SecretString, new: &SecretString, KdfParams, &SecretKey) -> Result<()>` — new convenience wrapper for tests, taking the account from the store.

- [ ] **Step 1: Write the failing test for the surviving scheme**

Append to `crates/havenkeys-core/tests/vault.rs`:

```rust
#[test]
fn the_header_refuses_any_scheme_but_account_bound() {
    use havenkeys_core::store::KeyScheme;
    // Serde form is part of the sync header on the wire; freeze it.
    assert_eq!(
        serde_json::to_string(&KeyScheme::AccountBound).unwrap(),
        "\"account_bound\""
    );
    // A database written by an older build carries key_scheme 1 or 2.
    assert!(serde_json::from_str::<KeyScheme>("\"password_only\"").is_err());
    assert!(serde_json::from_str::<KeyScheme>("\"password_and_secret_key\"").is_err());
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p havenkeys-core --test vault the_header_refuses -- --exact`
Expected: FAIL — both `from_str` calls currently succeed.

- [ ] **Step 3: Reduce `KeyScheme` to one variant**

In `store.rs`:

```rust
/// How the key-encryption key is derived. One scheme: the master password,
/// the device's Secret Key and the account (docs/crypto.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyScheme {
    AccountBound,
}

impl KeyScheme {
    /// Kept as a method so call sites do not match on the variant; every
    /// scheme this build understands needs the Secret Key.
    pub fn uses_secret_key(self) -> bool {
        true
    }

    fn to_db(self) -> i64 {
        3
    }

    fn from_db(v: i64) -> Result<Self> {
        match v {
            3 => Ok(KeyScheme::AccountBound),
            _ => Err(Error::UnsupportedVersion),
        }
    }
}
```

- [ ] **Step 4: Delete the v1 and v2 derivations**

In `crates/havenkeys-core/src/crypto/keys.rs`, delete `derive_kek`,
`derive_kek_with_secret_key`, and the `mod tests` cases that call them
(`derive_kek` separation by vault id, and the v2/v3 comparison). Keep
`account_bound`, `derive_kek_v3`, `derive_auth_key_from_master` and
`derive_data_key` untouched.

In `vault.rs`, replace `derive_kek_for` with:

```rust
/// The KEK for this vault. One scheme; the Secret Key and the account are
/// both required, and a missing one is refused before any expensive work.
pub(crate) fn derive_kek_for(
    password: &SecretString,
    kdf: &KdfParams,
    secret_key: Option<&SecretKey>,
    account: Option<&AccountRef>,
) -> Result<Key256> {
    let secret_key = secret_key.ok_or(Error::SecretKeyRequired)?;
    let account = account.ok_or(Error::InvalidInput(
        "this vault belongs to an account; sign in instead",
    ))?;
    derive_kek_v3(&derive_master_key(password, kdf)?, secret_key, account)
}
```

Then delete, in `vault.rs`: `prepare_new_vault`,
`prepare_new_vault_with_secret_key`, the private `prepare`,
`UnlockTicket::{derive, derive_with_secret_key}`,
`RekeyTicket::{derive, derive_with_secret_key, derive_secret_key_upgrade,
derive_account_upgrade, unwrap}`, `VaultService::{create_vault, unlock,
commit_account_upgrade, change_master_password}`, and the now-unused
`insert_vault` guard that refused `AccountBound` (its body folds back into
`create_account_vault`). Delete `UnlockTicket::{needs_secret_key,
needs_account}` only if nothing calls them; the desktop does, so keep them
returning `true`.

Add the test convenience that replaces `change_master_password`:

```rust
    /// Re-wrap the vault key under a new master password. The account comes
    /// from the local store, never from the caller.
    pub fn change_master_password_for_account(
        &mut self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
        secret_key: &SecretKey,
    ) -> Result<()> {
        let ticket = self.begin_rekey()?;
        let account = ticket
            .account()
            .cloned()
            .ok_or(Error::InvalidInput("this vault is not linked to an account"))?;
        let rekeyed = ticket.derive_for_account(current, new, new_kdf, secret_key, &account);
        self.commit_rekey(ticket, rekeyed)
    }
```

and the accessor it needs on `RekeyTicket`:

```rust
    pub fn account(&self) -> Option<&AccountRef> {
        self.account.as_ref()
    }
```

- [ ] **Step 5: Rewrite the test helpers around the single scheme**

In `crates/havenkeys-core/tests/common/mod.rs`, delete `new_vault_in`,
`new_vault` and `open_file`'s scheme-1 assumptions, and make `activated_vault`
the only constructor. Add a file-backed variant for the persistence tests:

```rust
/// An activated account vault on disk, unlocked, with its Secret Key.
pub fn activated_vault_at(path: &Path) -> (VaultService, SecretKey) {
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let sk = made.secret_key;
    let mut vault = VaultService::new(Store::open(path).unwrap());
    vault
        .create_account_vault(made.prepared, &account_record())
        .unwrap();
    (vault, sk)
}
```

- [ ] **Step 6: Fix every test that used a removed constructor**

Run: `cargo test -p havenkeys-core 2>&1 | head -60`

Work through the compilation errors in `tests/vault.rs`, `tests/account.rs`,
`tests/security.rs` and `tests/fuzz.rs`. The mechanical substitutions are:

| Was | Becomes |
|---|---|
| `new_vault()` | `common::activated_vault().0` |
| `v.unlock(&secret(PASSWORD))` | `v.unlock_for_account(&secret(PASSWORD), &sk, &account())` |
| `v.change_master_password(a, b, kdf)` | `v.change_master_password_for_account(a, b, kdf, &sk)` |

Delete outright: the scheme 1→2 upgrade tests, the 2→3 upgrade tests
(`a_secret_key_vault_upgrades_to_an_account_without_touching_items`,
`committing_an_account_upgrade_without_the_account_record_is_refused`,
`commit_account_upgrade_stores_the_account_record`,
`a_failed_account_upgrade_leaves_no_account_record_behind`,
`an_upgrade_refused_for_a_changed_header_leaves_no_account_record`,
`commit_account_upgrade_refuses_a_rekey_that_is_not_an_upgrade`), and
`create_vault_refuses_an_account_bound_vault` (there is no `create_vault`).
Keep `create_account_vault_stores_the_account_record` and
`rekey_is_refused_when_the_account_record_is_missing`.

- [ ] **Step 7: Run the new test and the suite**

Run: `cargo test -p havenkeys-core`
Expected: PASS, including `the_header_refuses_any_scheme_but_account_bound`.

- [ ] **Step 8: Commit**

```bash
cargo fmt -p havenkeys-core
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add -A
git commit -m "$(cat <<'EOF'
refactor(core): one key scheme

Key schemes 1 and 2 leave the product (spec 2026-09-20 §4). KeyScheme
keeps its enum shape and serde form for the header on the wire, with
one variant; derive_kek, derive_kek_with_secret_key and every creation,
unlock and upgrade path reachable only from the old schemes are gone.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Remove vault creation and Secret Key set-up from the desktop

Task 2 deleted the core functions these commands call, so the desktop does not
compile until this lands. Creating a vault now means activation against the
server, which arrives in step 3 of the spec's §13.

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs` (delete `create_vault`;
  rewrite `unlock_vault` for the single scheme)
- Modify: `apps/desktop/src-tauri/src/account.rs` (delete `setup_secret_key`)
- Modify: `apps/desktop/src-tauri/src/lib.rs`, `build.rs`,
  `capabilities/main.json`
- Modify: `apps/desktop/src/views/UnlockScreen.tsx`,
  `apps/desktop/src/views/SyncSection.tsx`, `apps/desktop/src/lib/api.ts`,
  `apps/desktop/src/App.tsx`

**Interfaces:**
- Consumes: `UnlockTicket::derive_for_account`, `KeyScheme::AccountBound` from Task 2.
- Produces: no new core API. The renderer loses `createVault` and
  `setupSecretKey`, and `App.tsx` renders a "no vault on this device" screen
  when `vault_status().vault_exists` is false.

- [ ] **Step 1: Delete the two commands**

Remove `create_vault` from `commands.rs` and `setup_secret_key` from
`account.rs`, then their entries in `generate_handler!`, `build.rs` and
`capabilities/main.json`.

- [ ] **Step 2: Rewrite `unlock_vault` for the single scheme**

The account now comes from the store, so the command reads it and passes it
to the only derivation there is:

```rust
#[tauri::command]
pub async fn unlock_vault(
    app: AppHandle,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    let account = state
        .vault()?
        .account()?
        .ok_or(havenkeys_core::Error::NoVault)?
        .to_ref()?;
    let stored = state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .secret_key();
    let typed = match secret_key {
        Some(t) if !t.is_empty() => Some(SecretKey::parse(t.expose())?),
        _ => None,
    };
    let Some(key) = typed.clone().or(stored) else {
        let ticket = state.vault()?.begin_unlock()?;
        let r = state
            .vault()?
            .finish_unlock(ticket, Err(havenkeys_core::Error::SecretKeyRequired));
        return Err(r
            .err()
            .unwrap_or(havenkeys_core::Error::SecretKeyRequired)
            .into());
    };
    let key_text = typed.as_ref().map(|k| k.to_text());
    let ticket = state.vault()?.begin_unlock()?;
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let derived = ticket.derive_for_account(&password, &key, &account);
        (ticket, derived)
    })
    .await;
    let (ticket, derived) = match joined {
        Ok(v) => v,
        Err(_) => {
            state.lock(&app, "error");
            return Err(CmdError::internal());
        }
    };

    let mut v = state.vault()?;
    v.finish_unlock(ticket, derived)?;
    let minutes = v.settings()?.auto_lock_minutes;
    let status = v.status()?;
    state.arm_auto_lock(minutes);
    state.notify_unlocked();
    drop(v);
    if let Some(text) = key_text {
        if let Ok(k) = SecretKey::parse(text.expose()) {
            if let Ok(mut d) = state.device.lock() {
                let _ = d.set_secret_key(&k);
            }
        }
    }
    Ok(status)
}
```

- [ ] **Step 3: Replace the create-vault screen**

In `App.tsx`, where `vault_exists === false` currently renders the create
form, render instead:

```tsx
<div className="empty-state">
  <h1>No vault on this computer</h1>
  <p>
    HavenKeys vaults belong to an account. Activation with an invite is not
    available in this build yet.
  </p>
</div>
```

Remove `createVault` and `setupSecretKey` from `lib/api.ts`, the "create"
branch from `UnlockScreen.tsx`, and the `AddSecretKey` block from
`SyncSection.tsx`.

- [ ] **Step 4: Verify the tree**

Run:
```bash
cargo test
cargo clippy -p havenkeys-core -p havenkeys-desktop --all-targets -- -D warnings
pnpm -r typecheck
```
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
refactor(desktop): vaults come from activation, not from the app

create_vault and setup_secret_key are gone: every vault is account-bound
and is created by activating against the server (spec 2026-09-20 §5),
which lands with the sync client. unlock_vault now reads the account
from the store and uses the single derivation.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Local store schema 4

The replica needs a per-item revision and has no use for tombstones or dirty
flags (spec §8.2). This is a clean schema, not a migration: a database from an
older build is refused.

**Files:**
- Modify: `crates/havenkeys-core/src/store.rs`
- Modify: `crates/havenkeys-core/tests/vault.rs`

**Interfaces:**
- Consumes: `KeyScheme::AccountBound` from Task 2.
- Produces:
  - `SCHEMA_VERSION: i64 = 4`
  - `Store::upsert_item(&mut self, id: &Uuid, overview: &[u8], details: &[u8], revision: i64) -> Result<()>`
  - `Store::delete_item(&mut self, id: &Uuid) -> Result<bool>` — no tombstone
  - `Store::item_revision(&self, id: &Uuid) -> Result<Option<i64>>`
  - `Store::upsert_items(&mut self, rows: &[(Uuid, Vec<u8>, Vec<u8>, i64)]) -> Result<()>` — one transaction
  - Removed: `insert_items`, `item_rows`, `dirty_rows`, `dirty_tombstones`, `clear_dirty`, `tombstones`, `MIGRATE_1_TO_2`, `MIGRATE_2_TO_3`

- [ ] **Step 1: Write the failing tests**

Append to `crates/havenkeys-core/tests/vault.rs`:

```rust
#[test]
fn a_database_from_an_older_build_is_refused() {
    use havenkeys_core::store::Store;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.sqlite3");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 3i64).unwrap();
    }
    let err = Store::open(&path).err().unwrap();
    assert_eq!(err.code(), "unsupported_version");
}

#[test]
fn item_rows_carry_the_server_revision() {
    use havenkeys_core::store::Store;
    let mut store = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::from_u128(9);
    store.upsert_item(&id, b"ov", b"det", 7).unwrap();
    assert_eq!(store.item_revision(&id).unwrap(), Some(7));

    store.upsert_item(&id, b"ov2", b"det2", 9).unwrap();
    assert_eq!(store.item_revision(&id).unwrap(), Some(9));

    assert!(store.delete_item(&id).unwrap());
    assert_eq!(store.item_revision(&id).unwrap(), None);
    // No tombstone survives the delete: the server holds those.
    assert!(!store.delete_item(&id).unwrap());
}
```

Add `tempfile` and `rusqlite` to `[dev-dependencies]` of
`crates/havenkeys-core/Cargo.toml` if they are not already there (check
first — `tempfile` is used by the existing persistence tests).

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-core --test vault a_database_from_an_older item_rows_carry`
Expected: FAIL — `user_version` 3 is accepted today, and `upsert_item` takes
three arguments.

- [ ] **Step 3: Write schema 4**

Replace the schema constants and `init` in `store.rs`:

```rust
pub const SCHEMA_VERSION: i64 = 4;

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
";

    fn init(conn: Connection) -> Result<Self> {
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
```

- [ ] **Step 4: Rewrite the item accessors**

```rust
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

    /// Apply many rows atomically: either all land or none.
    pub fn upsert_items(&mut self, rows: &[(Uuid, Vec<u8>, Vec<u8>, i64)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO items (id, overview, details, revision) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                               details  = excluded.details,
                                               revision = excluded.revision",
            )?;
            for (id, overview, details, revision) in rows {
                stmt.execute(params![id.to_string(), overview, details, revision])?;
            }
        }
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
```

Delete `insert_items`, `item_rows`, `dirty_rows`, `dirty_tombstones`,
`clear_dirty`, `tombstones` and the two migration constants.

- [ ] **Step 5: Follow the compiler**

Run: `cargo test -p havenkeys-core 2>&1 | head -40`

`vault.rs::persist`, `vault.rs::delete_item`, `vault.rs::import_items` and
`sync.rs` call the changed methods. Do the minimum to compile — Tasks 5 and 6
rewrite them properly. For now pass `0` as the revision at those call sites
and leave a comment `// revision set by Task 5`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p havenkeys-core`
Expected: PASS, including both new tests.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p havenkeys-core
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add -A
git commit -m "$(cat <<'EOF'
feat(core): schema 4, the read-only replica

Items carry the server revision that writes send as baseRevision and
pulls overwrite. The tombstones table and the dirty columns are gone —
the server holds tombstones — and so are the 1->2 and 2->3 migrations:
a database from an older build is refused rather than migrated
(spec 2026-09-20 §8.2).

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Staged writes

Writes stop being local commits and become proposals the server accepts (spec
§8.4). `create_item`, `update_item`, `delete_item` and `import_items` are
replaced by one staging call and one commit call.

**Files:**
- Modify: `crates/havenkeys-core/src/error.rs`
- Modify: `crates/havenkeys-core/src/vault.rs`
- Modify: `crates/havenkeys-core/src/import/mod.rs` (only if it calls
  `import_items`; the parsers themselves are untouched)
- Create: `crates/havenkeys-core/tests/writes.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `Store::{upsert_item, upsert_items, delete_item, item_revision}` from Task 4.
- Produces:

```rust
/// A write the server has not accepted yet. Holds sealed blobs; never logged.
pub struct StagedWrite {
    pub item_id: Uuid,
    /// The revision this device last saw, or None for a new item.
    pub base_revision: Option<i64>,
    /// None for a deletion.
    pub overview: Option<Vec<u8>>,
    pub details: Option<Vec<u8>>,
    epoch: u64,
    plain: Option<ItemOverview>,
}

impl VaultService {
    pub fn stage_create(&self, input: ItemInput, now_ms: i64) -> Result<StagedWrite>;
    pub fn stage_update(&self, id: &Uuid, input: ItemInput, now_ms: i64) -> Result<StagedWrite>;
    pub fn stage_delete(&self, id: &Uuid) -> Result<StagedWrite>;
      pub fn commit_write(&mut self, staged: StagedWrite, revision: i64) -> Result<Option<ItemOverview>>;
}
```

- New error variants: `Error::Offline` (code `offline`) and
  `Error::ItemChangedElsewhere` (code `item_changed_elsewhere`).

- [ ] **Step 1: Write the failing tests**

Create `crates/havenkeys-core/tests/writes.rs`:

```rust
//! Staged writes: the server accepts them, the replica records them.

mod common;

use common::{account, activated_vault, login, secret, NOW, PASSWORD};
use havenkeys_core::model::SecretField;

#[test]
fn a_staged_create_is_not_visible_until_it_is_committed() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).unwrap();

    // Nothing is stored yet: the server has not seen it.
    assert!(vault.list_items().unwrap().is_empty());
    assert_eq!(staged.base_revision, None);

    let overview = vault.commit_write(staged, 12).unwrap().unwrap();
    assert_eq!(vault.list_items().unwrap().len(), 1);
    assert_eq!(
        vault.reveal(&overview.id, SecretField::Password).unwrap().expose(),
        "pw"
    );
}

#[test]
fn an_update_stages_the_revision_it_last_saw() {
    let (mut vault, _sk) = activated_vault();
    let created = vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let id = created.item_id;
    vault.commit_write(created, 12).unwrap();

    let staged = vault
        .stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW + 1)
        .unwrap();
    assert_eq!(staged.base_revision, Some(12));
}

#[test]
fn a_staged_delete_carries_no_blobs() {
    let (mut vault, _sk) = activated_vault();
    let created = vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let id = created.item_id;
    vault.commit_write(created, 12).unwrap();

    let staged = vault.stage_delete(&id).unwrap();
    assert!(staged.overview.is_none() && staged.details.is_none());
    assert_eq!(staged.base_revision, Some(12));

    assert!(vault.commit_write(staged, 13).unwrap().is_none());
    assert!(vault.list_items().unwrap().is_empty());
}

#[test]
fn a_write_staged_before_a_lock_is_discarded() {
    let (mut vault, sk) = activated_vault();
    let staged = vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    vault.lock();
    vault.unlock_for_account(&secret(PASSWORD), &sk, &account()).unwrap();

    // The blobs were sealed under the previous session; the server may even
    // have accepted them, but this device must not record them blindly.
    assert!(vault.commit_write(staged, 12).is_err());
    assert!(vault.list_items().unwrap().is_empty());
}

#[test]
fn staging_a_write_while_locked_is_refused() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).is_err());
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-core --test writes`
Expected: FAIL to compile — `stage_create` does not exist.

- [ ] **Step 3: Add the error variants**

In `crates/havenkeys-core/src/error.rs`, following the existing pattern:

```rust
    /// No server session: the vault is readable but cannot be changed.
    Offline,
    /// The server holds a newer revision of this item than this device staged.
    ItemChangedElsewhere,
```

with `code()` returning `"offline"` and `"item_changed_elsewhere"`, and
`Display` messages `"HavenKeys is offline — the vault is read-only until it
reconnects."` and `"This item changed on another device."`.

- [ ] **Step 4: Implement staging and commit**

In `vault.rs`, replacing `create_item`, `update_item`, `delete_item` and
`persist`:

```rust
/// A write the server has not accepted yet.
pub struct StagedWrite {
    pub item_id: Uuid,
    pub base_revision: Option<i64>,
    pub overview: Option<Vec<u8>>,
    pub details: Option<Vec<u8>>,
    epoch: u64,
    plain: Option<ItemOverview>,
}

impl std::fmt::Debug for StagedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagedWrite")
            .field("item_id", &self.item_id)
            .field("base_revision", &self.base_revision)
            .finish_non_exhaustive()
    }
}

impl VaultService {
    pub fn stage_create(&self, input: ItemInput, now_ms: i64) -> Result<StagedWrite> {
        let id = Uuid::new_v4();
        let (overview, details) = build_item(id, input, None, now_ms, now_ms)?;
        self.stage(overview, Some(&details), None)
    }

    pub fn stage_update(&self, id: &Uuid, input: ItemInput, now_ms: i64) -> Result<StagedWrite> {
        let existing = self.get_item(id)?;
        if existing.item_type != input.item_type {
            return Err(Error::InvalidInput("item type cannot change"));
        }
        let current = self.load_details(id)?;
        let (overview, details) =
            build_item(*id, input, Some(current), existing.created_at, now_ms)?;
        let base = self.store.item_revision(id)?;
        self.stage(overview, Some(&details), base)
    }

    pub fn stage_delete(&self, id: &Uuid) -> Result<StagedWrite> {
        let session = self.session()?;
        if !session.overviews.contains_key(id) {
            return Err(Error::NotFound);
        }
        Ok(StagedWrite {
            item_id: *id,
            base_revision: self.store.item_revision(id)?,
            overview: None,
            details: None,
            epoch: self.epoch,
            plain: None,
        })
    }

    fn stage(
        &self,
        overview: ItemOverview,
        details: Option<&ItemDetails>,
        base_revision: Option<i64>,
    ) -> Result<StagedWrite> {
        let session = self.session()?;
        let id = overview.id;
        let ov_blob = seal_json(
            &session.data_key,
            &BlobContext::item(Purpose::ItemOverview, session.vault_id, id),
            &overview,
        )?;
        let det_blob = match details {
            Some(d) => Some(seal_json(
                &session.data_key,
                &BlobContext::item(Purpose::ItemDetails, session.vault_id, id),
                d,
            )?),
            None => None,
        };
        Ok(StagedWrite {
            item_id: id,
            base_revision,
            overview: Some(ov_blob),
            details: det_blob,
            epoch: self.epoch,
            plain: Some(overview),
        })
    }

    /// Record a write the server accepted at `revision`. Returns the stored
    /// overview, or None for a deletion.
    ///
    /// Refused if the vault locked since the write was staged: the blobs were
    /// sealed under a session that no longer exists, and recording them would
    /// put the replica ahead of what this device can read back.
    pub fn commit_write(
        &mut self,
        staged: StagedWrite,
        revision: i64,
    ) -> Result<Option<ItemOverview>> {
        self.session()?;
        if staged.epoch != self.epoch {
            return Err(Error::Locked);
        }
        match (staged.overview, staged.details, staged.plain) {
            (Some(ov), Some(det), Some(overview)) => {
                self.store
                    .upsert_item(&staged.item_id, &ov, &det, revision)?;
                self.session_mut()?
                    .overviews
                    .insert(staged.item_id, overview.clone());
                Ok(Some(overview))
            }
            (None, None, None) => {
                self.store.delete_item(&staged.item_id)?;
                self.session_mut()?.overviews.remove(&staged.item_id);
                Ok(None)
            }
            _ => Err(Error::InvalidInput("malformed staged write")),
        }
    }
}
```

Delete `import_items` and `dedupe_keys` with it. Import writes many items and
so needs the server exactly as a single write does; it is rebuilt on top of
staged writes in step 3 of the spec's §13, where there is something to send
them to. `import::import_1pux` refuses with `Error::Offline` meanwhile.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-core --test writes`
Expected: PASS, all five.

- [ ] **Step 6: Point the desktop at the new API**

In `commands.rs`, `create_item`, `update_item` and `delete_item` have no
server to ask, so they refuse:

```rust
#[tauri::command]
pub fn create_item(state: State<'_, AppState>, _input: ItemInput) -> CmdResult<ItemOverview> {
    state.touch();
    // Writes need a server session (spec 2026-09-20 §8.4); Task 7 gates this
    // on the connectivity state once there is one to be in.
    Err(havenkeys_core::Error::Offline.into())
}
```

Apply the same to `update_item`, `delete_item` and `import::import_1pux`.

- [ ] **Step 7: Verify the tree**

Run:
```bash
cargo test
cargo clippy -p havenkeys-core -p havenkeys-desktop --all-targets -- -D warnings
pnpm -r typecheck
```
Expected: green. Tests in `tests/vault.rs` and `tests/security.rs` that called
`create_item` need the two-step form; make that substitution as the compiler
finds them:

```rust
let staged = vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
let item = vault.commit_write(staged, 1).unwrap().unwrap();
```

- [ ] **Step 8: Commit**

```bash
cargo fmt -p havenkeys-core
git add -A
git commit -m "$(cat <<'EOF'
feat(core): staged writes

Item writes become proposals: stage_create/update/delete seal the blobs
under the vault lock without touching disk, and commit_write records the
row with the revision the server assigned (spec 2026-09-20 §8.4). A lock
between the two discards the write, as it does for unlock and rekey.

Adds Error::Offline and Error::ItemChangedElsewhere. The desktop's write
commands refuse with Offline until there is a session to have.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: The pull applier, without merge

A pull is the server's truth applied in cursor order (spec §8.5). There is
nothing to decide, so `decide_merge` and the push bookkeeping go.

**Files:**
- Modify: `crates/havenkeys-core/src/sync.rs`
- Create: `crates/havenkeys-core/tests/replica.rs`
- Delete: `crates/havenkeys-core/tests/delta_sync.rs`

**Interfaces:**
- Consumes: `Store::{upsert_items, delete_item, set_cursor}` from Task 4.
- Produces:

```rust
/// One item as the server serves it.
pub struct RemoteChange {
    pub item_id: Uuid,
    pub revision: i64,
    /// None for a deletion.
    pub overview: Option<Vec<u8>>,
    pub details: Option<Vec<u8>>,
    pub deleted: bool,
}

impl VaultService {
    pub fn apply_remote_changes(
        &mut self,
        cursor: i64,
        changes: Vec<RemoteChange>,
        now_ms: i64,
    ) -> Result<SyncReport>;
}
```

- Removed: `decide_merge`, `apply_merge`, `Candidate`, `pending_push`,
  `confirm_push`, `PendingPush`, `MAX_FUTURE_DELETION_SKEW_MS`.

- [ ] **Step 1: Write the failing tests**

Create `crates/havenkeys-core/tests/replica.rs`:

```rust
//! Applying what the server serves. The server is untrusted but authoritative:
//! it cannot corrupt the replica, and it can empty it.

mod common;

use common::{activated_vault, login, NOW};
use havenkeys_core::sync::RemoteChange;

fn change(vault: &havenkeys_core::vault::VaultService, title: &str, revision: i64) -> RemoteChange {
    let staged = vault.stage_create(login(title, "me", "pw", "github.com"), NOW).unwrap();
    RemoteChange {
        item_id: staged.item_id,
        revision,
        overview: staged.overview.clone(),
        details: staged.details.clone(),
        deleted: false,
    }
}

#[test]
fn a_pull_adds_items_and_moves_the_cursor() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);
    let id = c.item_id;

    let report = vault.apply_remote_changes(5, vec![c], NOW).unwrap();

    assert_eq!(report.added, 1);
    assert_eq!(vault.get_item(&id).unwrap().title, "GitHub");
    assert_eq!(vault.account().unwrap().unwrap().server_cursor, 5);
}

#[test]
fn a_pull_deletes_without_asking_who_is_newer() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);
    let id = c.item_id;
    vault.apply_remote_changes(5, vec![c], NOW).unwrap();

    // No timestamp comparison, no resurrection arm: the server said so.
    let report = vault
        .apply_remote_changes(
            6,
            vec![RemoteChange {
                item_id: id,
                revision: 6,
                overview: None,
                details: None,
                deleted: true,
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.deleted, 1);
    assert!(vault.get_item(&id).is_err());
}

#[test]
fn a_blob_that_does_not_open_leaves_the_existing_row_alone() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);
    let id = c.item_id;
    vault.apply_remote_changes(5, vec![c], NOW).unwrap();

    let report = vault
        .apply_remote_changes(
            6,
            vec![RemoteChange {
                item_id: id,
                revision: 6,
                overview: Some(vec![0u8; 64]),
                details: Some(vec![0u8; 64]),
                deleted: false,
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.updated, 0);
    assert_eq!(vault.get_item(&id).unwrap().title, "GitHub");
}

#[test]
fn a_change_whose_blob_is_for_another_item_is_skipped() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);

    let report = vault
        .apply_remote_changes(
            5,
            vec![RemoteChange {
                item_id: uuid::Uuid::from_u128(999),
                ..c
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.skipped_items, 1);
    assert!(vault.list_items().unwrap().is_empty());
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-core --test replica`
Expected: FAIL to compile — `RemoteChange` has `deleted_at`, not `revision`
and `deleted`.

- [ ] **Step 3: Rewrite `RemoteChange` and the applier**

In `sync.rs`:

```rust
/// One item as the server serves it. Blobs are the same per-item ciphertexts
/// stored locally, copied without re-encryption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteChange {
    pub item_id: Uuid,
    pub revision: i64,
    pub overview: Option<Vec<u8>>,
    pub details: Option<Vec<u8>>,
    pub deleted: bool,
}

impl VaultService {
    /// Apply a pull in cursor order and advance the stored cursor.
    ///
    /// This is not a merge. The server is the single writer, so a change is
    /// an upsert or a deletion. The only judgement is structural: a blob
    /// that does not open under this vault's key, or that carries another
    /// item's id, is counted in `skipped_items` and leaves the existing row
    /// untouched — a hostile server can fail to update the replica, not
    /// corrupt it.
    pub fn apply_remote_changes(
        &mut self,
        cursor: i64,
        changes: Vec<RemoteChange>,
        now_ms: i64,
    ) -> Result<SyncReport> {
        self.require_account()?;
        let vault_id = self.session()?.vault_id;
        let mut report = SyncReport::default();
        let mut rows: Vec<(Uuid, Vec<u8>, Vec<u8>, i64)> = Vec::new();
        let mut overviews: Vec<ItemOverview> = Vec::new();
        let mut deletions: Vec<Uuid> = Vec::new();

        for change in changes {
            if change.deleted {
                deletions.push(change.item_id);
                continue;
            }
            let (Some(ov), Some(det)) = (change.overview, change.details) else {
                report.skipped_items += 1;
                continue;
            };
            match self.check_item_bytes(vault_id, change.item_id, ov, det) {
                Some(((id, ov, det), overview)) => {
                    if self.session()?.overviews.contains_key(&id) {
                        report.updated += 1;
                    } else {
                        report.added += 1;
                    }
                    rows.push((id, ov, det, change.revision));
                    overviews.push(overview);
                }
                None => report.skipped_items += 1,
            }
        }

        self.store.upsert_items(&rows)?;
        for id in &deletions {
            if self.store.delete_item(id)? {
                report.deleted += 1;
            }
            self.session_mut()?.overviews.remove(id);
        }
        for overview in overviews {
            self.session_mut()?.overviews.insert(overview.id, overview);
        }
        self.store.set_cursor(cursor, now_ms)?;
        Ok(report)
    }
}
```

Then delete `decide_merge`, `apply_merge`, `Candidate`, `pending_push`,
`confirm_push`, `PendingPush` and `MAX_FUTURE_DELETION_SKEW_MS`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-core --test replica`
Expected: PASS, all four.

- [ ] **Step 5: Delete the superseded suite**

```bash
git rm crates/havenkeys-core/tests/delta_sync.rs
```

Then move any case from `tests/fuzz.rs` that fuzzed `RemoteChange` to the new
shape — the fuzzer must still feed arbitrary bytes as `overview`/`details`
and assert no panic.

- [ ] **Step 6: Verify the tree**

Run: `cargo test && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: green.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p havenkeys-core
git add -A
git commit -m "$(cat <<'EOF'
feat(core): apply pulls without merging

The server is the single writer, so a pull is an upsert or a deletion in
cursor order (spec 2026-09-20 §8.5). decide_merge, the push bookkeeping
and the future-skew heuristic are gone, and with them the unauthenticated
deleted_at that the old delta format consumed.

A blob that does not open, or that carries another item's id, is counted
in skipped_items and leaves the existing row alone: a hostile server can
fail to update the replica, not corrupt it.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Connectivity state and the read-only gate

The desktop learns the difference between locked and offline (spec §8.6). With
no sync client yet, the state is always `Offline` — which is the real
behaviour of a disconnected device, not a placeholder.

**Files:**
- Modify: `apps/desktop/src-tauri/src/state.rs` (the `Connectivity` state)
- Modify: `apps/desktop/src-tauri/src/commands.rs` (the gate; `device_status`)
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/App.tsx`,
  `apps/desktop/src/views/{VaultScreen,ItemEditor,ItemDetail}.tsx`

**Interfaces:**
- Consumes: `Error::Offline` from Task 5.
- Produces: `DeviceStatus { keyScheme, needsSecretKey, online: boolean }` on
  the renderer side.

- [ ] **Step 1: Add the state**

In `state.rs`:

```rust
/// Whether this device has a server session. Independent of the lock state:
/// a locked vault is never online, and an unlocked one may be offline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connectivity {
    Offline,
    Online,
}
```

Hold it as `Mutex<Connectivity>` on `AppState`, defaulting to `Offline`, with
`fn is_online(&self) -> bool` and `fn require_online(&self) -> CmdResult<()>`
returning `Err(havenkeys_core::Error::Offline.into())` when offline.

- [ ] **Step 2: Gate the mutating commands**

Replace the temporary bodies from Task 5 Step 6 with the real gate, which
calls the staging API and then has nothing to send it to — so it still ends in
`Offline`, but through the path that step 3 of the spec's §13 will complete:

```rust
#[tauri::command]
pub fn create_item(state: State<'_, AppState>, input: ItemInput) -> CmdResult<ItemOverview> {
    state.touch();
    state.require_online()?;
    let _staged = state.vault()?.stage_create(input, AppState::now_ms())?;
    // The sync client sends `_staged` and returns the server's revision,
    // which commit_write records. Until it exists, require_online above has
    // already returned.
    Err(havenkeys_core::Error::Offline.into())
}
```

Apply the same to `update_item`, `delete_item` and `change_master_password`
(whose header write needs the server).

Do **not** gate `update_settings`: settings are device-local and unsynced
(spec §8.2), so auto-lock and the clipboard timeout keep working offline.

- [ ] **Step 3: Report it to the renderer**

Add `online: bool` to `DeviceStatus` in `account.rs`, filled from
`state.is_online()`, and to the TypeScript interface.

- [ ] **Step 4: Show it**

In `App.tsx`, when `device?.online === false` and the vault is unlocked,
render a banner above the vault:

```tsx
<div className="banner banner-muted" role="status">
  Offline — the vault is read-only until it reconnects.
</div>
```

and pass `readOnly={!device?.online}` to `VaultScreen`, which disables the
new-item button and the edit and delete actions rather than letting them fail.

- [ ] **Step 5: Verify the tree**

Run:
```bash
cargo test
cargo clippy -p havenkeys-core -p havenkeys-desktop --all-targets -- -D warnings
pnpm -r typecheck && pnpm -r test
```
Expected: all green.

- [ ] **Step 6: Update the documentation this step invalidates**

- `docs/crypto.md`: delete the "Secret Key (key scheme 2)" section's scheme-1
  and scheme-2 paragraphs, the 2 → 3 upgrade bullet, the unauthenticated
  `deleted_at` section and the `apply_remote_changes` cursor contract note;
  keep the key hierarchy and key scheme 3.
- `docs/sync.md` → `docs/server-sync.md`, rewritten around the replica.
- `docs/architecture.md`: the diagram gains the server and the replica.
- `docs/roadmap.md`: replace §3 with the spec's §13 steps 2–5.

Leave `CLAUDE.md`, `README.md`, `threat-model.md` and `security-model.md` to
step 5 of the spec's §13, when the behaviour they describe actually ships.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat(desktop): offline is read-only, and says so

Connectivity is tracked separately from the lock state: reads work
offline, every mutating command refuses with Error::Offline, and the UI
disables those controls behind a banner rather than letting them fail
(spec 2026-09-20 §8.6). With no sync client yet the state is always
Offline, which is the real behaviour of a disconnected device.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## What this step does not do

Carried to steps 2–5 of the spec's §13, so they are not lost:

* **Activation and sign-in** have no path: `create_account_vault` is exercised
  only by tests. The first-run and second-device screens arrive with the sync
  client.
* **`CLAUDE.md` §1 still claims local-first, fully offline operation and no
  backend.** The spec's §10 amends it; that edit belongs with the change that
  makes it true, in step 5, not here.
* **`security-review.md` has no entry for the new model.** The review pass is
  step 5.
* **Import is removed, not ported.** `import_items` and its de-duplication go
  with the local write path; 1PUX parsing and `ImportReport` stay. Import is
  rebuilt on staged writes in step 3, in batches of at most 500 (spec §8.4).
* **Tested server backups** are a prerequisite of shipping (spec §9), not of
  this step.
