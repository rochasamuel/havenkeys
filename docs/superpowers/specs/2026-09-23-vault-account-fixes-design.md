# Vault and account fixes — Design

Status: proposed, 2026-09-23.
Fixes three limitations carried forward in `docs/roadmap.md` §3 and one
defect found while designing them. Vault key rotation (`security-review.md`
#8) is deliberately **not** part of this design and stays open.

> This software has not undergone an independent security audit.

## 1. Goal

Four independent parts, implemented and landed in this order:

| Part | Problem today | Outcome |
|---|---|---|
| A | A master-password change updates the header only. The server's login verifier and KDF parameters stay at their activation values, so every device (including the one that made the change) fails to log in afterwards. Two offline changes also leave the local header revision two ahead, and every later publish conflicts. | A password change is one atomic server operation, other sessions are revoked, and other devices can pick up the new password at unlock. |
| C | A pulled item that does not decrypt is skipped, the cursor advances past it, and it is never retried. Only a manual Sync or Re-download shows the count. | Unreadable items are recorded, retried every sync, and shown until resolved. |
| D | The Secret Key is stored in plain text in `device.json`. | It lives in the OS keychain, with `device.json` as a warned fallback. |
| E | A vault cannot be moved to another account or server; `Store::set_account` refuses and no command removes a vault. | "Remove this device" returns the app to first run without destroying the only local copy. |

The vault format may change freely: the only vault in existence is the
owner's, and it can be reset. No migrations are written.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Order | A → C → D → E | — |
| Vault key rotation | Out of scope | Doing it with A (needs a key ID in every blob and a bulk re-encrypt endpoint) |
| Other sessions after a password change | Revoked | Kept (a hijacked session would survive the change) |
| Commit order for a password change | Server first, local after a 2xx | Local first, publish later (the source of the deadlock) |
| Proof for a password change | Current login key, checked against the verifier | Session token alone (a stolen token could lock the owner out) |
| Retrying unreadable items | Fetch by ID from a new endpoint | Holding the cursor back (a permanently bad item re-pulls everything after it on every sync) |
| No keychain available | Fall back to `device.json` (0600) with a visible warning | Refuse to store; type the Secret Key at every unlock |
| Re-pointing a vault | Remove this device, then Activate/Sign in fresh | Migrating items between accounts |
| The removed local vault | Renamed to `vault.sqlite3.removed-<timestamp>` | Deleted (it may be the last copy if the old server is gone; export does not exist yet) |

## 3. Part A — Master-password change

### 3.1 Server

New route `POST /v1/account/credentials`, authenticated by session:

```json
{
  "currentAuthKey": "<b64, 32 bytes>",
  "kdf": { "algorithm": "argon2id", "memoryKib": 0, "iterations": 0, "parallelism": 0, "salt": "<b64, 16 bytes>" },
  "newAuthKey": "<b64, 32 bytes>",
  "header": "<b64>",
  "baseHeaderRevision": 0
}
```

`deny_unknown_fields`. KDF parameters are validated with the same bounds as
activation, and the header with the same size limit as today.

One transaction:

1. `SELECT … FOR UPDATE` on the account and its vault.
2. Check `currentAuthKey` against `accounts.auth_verifier`. A failure is
   counted in `login_attempts` exactly like a failed login and returns the
   same error a failed login does. Rate limiting applies before the Argon2id
   verification, as it does for login (`security-review.md` S1).
3. `baseHeaderRevision` must equal `vaults.header_revision`, otherwise
   `409 Conflict`.
4. Update `accounts` (the KDF columns and `auth_verifier`, the Argon2id PHC
   string of `newAuthKey`, produced the way activation produces it) and
   `vaults` (`header`, `header_revision = base + 1`).
5. Delete every session on the account except the calling one.

The response is `{ "headerRevision": base + 1 }`. The log line records the
account ID and the new revision only.

`PUT /v1/vault/header` and its client method are removed. The header now
changes only through this route.

### 3.2 Desktop, on the device making the change

`change_master_password`:

1. `require_online`, check the new password, `KdfParams::generate()`.
2. `begin_rekey`. Off the vault lock, a single derivation produces the old
   login key (the current password and the current KDF), the new login key
   (the new password and the new KDF) and the re-wrapped vault key. This
   extends `RekeyTicket::derive_for_account`; the old-key derivation already
   happens there to unwrap.
3. The core encodes the header the rekey would produce at revision
   `local + 1`, without writing it.
4. `client.change_credentials(...)`.
5. Only after a 2xx: `commit_rekey` at the returned revision. It raises
   `max_header_rev` as today.
6. Any error: nothing local changes and the error is shown. A 409 means
   another device changed the password first; the message says so.

If the server committed but the response was lost, the local header is one
behind. The next `sync_now` adopts the newer remote header through the
existing `adopt_account_header` path (this device's session was the one
kept), so the device heals itself.

`sync_now` loses its "local revision ahead → publish" branch. Local ahead of
the server can no longer happen; if it is observed anyway, the sync is
treated as failed with an internal error and nothing is sent.

### 3.3 Desktop, on every other device

Those devices are now offline (their sessions were deleted) and still hold
the old wrap and the old KDF salt.

**Unlock with the new password.** The local unlock fails with
`WrongPassword`. If an account is configured and the server is reachable,
`unlock_vault` falls back to an online path:

1. `auth_params(email)`. If the returned KDF equals the local one, stop
   and report the wrong password; nothing changed server-side.
2. Derive the login key from the typed password, the server KDF and the
   Secret Key, then `login`.
3. `client.header`, then verify it exactly as `prepare_sign_in` does: same
   vault ID, scheme 3, the vault key unwraps under the new KEK, and the
   attestation opens under the derived data key. The data key is unchanged
   because the vault key is unchanged, so a header forged by the server
   fails here.
4. `adopt_account_header` (checks the revision floor), unlock with the key
   from step 3, and keep the session from step 2.

The unlock screen still says only "wrong password" for a wrong password; the
fallback costs a second Argon2id run in that case.

**Unlock with the old password.** The local unlock succeeds and the vault is
usable offline. `sync::connect` gets a login rejection; the app shows, once
per unlock: "Your master password was changed on another device. Lock and
unlock with the new password."

### 3.4 Tests

* Server (`scripts/test-server.sh` suites): wrong current key (counted, same
  error as login); rate limit applies; stale base revision → 409; success
  replaces verifier, KDF and header; other sessions return 401 afterwards
  and the calling one does not; the new password logs in and the old one
  does not; `auth/params` returns the new KDF.
* Sync client: request shape; 409 is mapped to a conflict error.
* Core: the rekey derivation returns both login keys; commit at an
  explicit revision; the online-unlock verification refuses a header with a
  wrong vault ID, a wrong scheme, a failed attestation, and a revision at or
  below the floor.
* Round trip (the existing real-server test): device 1 changes the
  password; device 2 unlocks with the new password via the fallback and
  then sees device 1's next write.

## 4. Part C — Unreadable items

### 4.1 Local

New table in the vault's SQLite (schema version bumped; no migration):

```sql
CREATE TABLE unreadable_items (
  id       BLOB PRIMARY KEY,  -- item UUID
  revision INTEGER NOT NULL   -- server revision that failed
);
```

`apply_remote_changes`:

* A change that fails `check_item_bytes`, or a non-deleted change missing a
  blob, is upserted into `unreadable_items` instead of only being counted.
* A change that applies (update or deletion) removes its item from
  `unreadable_items`.
* Upserts, deletions, the `unreadable_items` changes and the cursor advance
  run in **one transaction**. Today they are separate statements.

`reset_sync_cursor` clears `unreadable_items` in the same transaction; a full
re-download re-evaluates every item anyway.

### 4.2 Server

New route `POST /v1/items/fetch` with `{ "itemIds": [...] }`, 1–500 distinct
UUIDs, `deny_unknown_fields`. It returns the current row for each ID found
in the caller's vault, in the same change shape the pull returns
(tombstones included). Unknown IDs are omitted. It is read-only.

### 4.3 Retry

At the end of `sync_now`, if `unreadable_items` is not empty, the app
fetches those IDs (in chunks of 500) and passes the result through the same
apply path, without moving the cursor. An item that now decrypts, or is
deleted, leaves the table. An ID the server no longer has is removed from
the table and from the replica, as a deletion would be.

### 4.4 Reporting

* `SyncReport.skipped_items` keeps its meaning (skipped in this run).
  `VaultStatus` gains `unreadableItems: number`, the table's row count.
* The vault screen shows a persistent banner while `unreadableItems > 0`:
  "N items couldn't be read from the server." with a **Re-download**
  action. It updates on `vault://synced` and on unlock, so background syncs
  are covered.
* The existing error toasts in `AccountSection` stay.

### 4.5 Tests

Core: a bad blob is recorded and the cursor still advances in the same
transaction; a later good revision clears it; a deletion clears it; reset
clears it. Server: fetch returns only the caller's items, tombstones
included, and rejects more than 500 IDs, duplicates and unknown fields.
Desktop: the retry path clears an item that the server has since fixed.

## 5. Part D — Secret Key in the OS keychain

### 5.1 Module

`apps/desktop/src-tauri/src/secret_store.rs` wraps the `keyring` crate:
Windows Credential Manager, macOS Keychain, Linux Secret Service. It
exposes `load`, `store` and `delete`, and a `Storage` enum (`Keychain` or
`File`). The entry has service `app.havenkeys` and user `<account_id>`.
Only the `H1-…` text is stored. Errors never include the value.

`Device` keeps the device ID in `device.json` and moves the Secret Key
behind the store:

* `set_secret_key`: try the keychain. On success, drop `secretKey` from
  `device.json`. On failure, write it to `device.json` (0600, as today) and
  report `Storage::File`.
* `secret_key`: keychain first, then `device.json`.
* On start, a key found in `device.json` is moved to the keychain if one is
  available, and removed from the file after the keychain write is read
  back successfully.

The account ID keys the entry, so `set_secret_key` and `secret_key` take it
as an argument. Every caller already has it before storing: activation from
the invite, sign-in from `auth/params`, unlock and the rest from the account
record.

### 5.2 Reporting

`device_status` gains `secretKeyStorage: "keychain" | "file" | "none"`.
Settings → Account shows a warning for `"file"`: "Your Secret Key is stored
in a file on this computer because no system keychain is available."

### 5.3 Dependency

`keyring` with only the platform-native backends enabled (no vendored or
"mock" features in release). It is checked with `cargo deny` and
`cargo audit`, and its licence is recorded before it is added. On Linux
without a running Secret Service, the call must fail rather than hang, so
the file fallback is reached. If the crate cannot guarantee that, the call
runs on a worker thread with a 5-second timeout.

### 5.4 Tests

The move from file to keychain, the fallback when the store errors, and
that `device.json` never contains the key after a successful move. The
keychain itself is behind a trait so tests use an in-memory fake, with one
ignored test that talks to the real OS store for manual runs.

## 6. Part E — Remove this device

### 6.1 Command

`remove_device(confirmation: String)`, allowed while locked or unlocked:

1. `confirmation` must equal the account's email (typed by the user).
2. If online, revoke this device on the server (`DELETE /v1/devices/{id}`
   for its own ID). Best effort: a failure is reported but does not stop
   removal.
3. `state.lock(&app, "user")` and drop the session and the cached sync
   client.
4. Close the store, then rename `vault.sqlite3` (and any `-wal`/`-shm`
   files) to `vault.sqlite3.removed-<UTC timestamp>` in the same directory.
5. Delete the Secret Key from the keychain and from `device.json`. The device
   ID is replaced with a fresh one so the next account sees a new device.
6. Reopen an empty store, so the app is in the same state as first run.

A failure before step 4 leaves everything as it was. A failure after step
4 leaves a first-run app; the renamed file is intact either way.

### 6.2 UI

Settings → Account → **Remove this device**, in a danger zone. The dialog
says:

* The vault stays on the server; this only removes it from this computer.
* A copy of the encrypted file is kept as `vault.sqlite3.removed-…`, and
  opening it later needs the master password and the Secret Key from the
  Emergency Kit.
* Type your email to confirm.

After success the app shows the first-run screen (Activate / Sign in), where
any server URL can be used.

### 6.3 Tests

Rename and first-run state; the confirmation mismatch is refused; the
Secret Key is gone from both stores; a server failure in step 2 does not
block removal; activating a new account afterwards succeeds.

## 7. Documentation

* `docs/server-sync.md` §7: remove the Secret Key and skipped-item
  limitations as stated; describe the keychain fallback and the retry.
  Keep the vault-key-rotation limitation.
* `docs/security-review.md`: new findings for the stale-verifier defect
  (fixed) and the unreadable-item handling (fixed); S12 is removed (the
  local-ahead state no longer exists); #8 stays open.
* `docs/security-model.md` / `threat-model.md`: the keychain, the
  credential-change route and session revocation.
* `docs/roadmap.md` §3: update the carried-forward list.
* `docs/deployment.md`: the new routes, if it lists routes.

## 8. Out of scope

Vault key rotation (#8), item-replay protection (`server-sync.md` §7),
migrating items between accounts, and export (roadmap §4).
