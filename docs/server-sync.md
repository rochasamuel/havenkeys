# Server sync and the read-only replica

Supersedes `docs/sync.md`. HavenKeys is moving from a local-first vault that
optionally synced through a shared folder to a server-authoritative one: a
`havenkeys-server` account is now required, and each device keeps a local
SQLite copy that is a **read-only replica** of what the server holds, not an
independently authoritative vault. This is the design in
`docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`;
this document describes it, and says plainly which parts exist in code today
and which do not.

> This software has not undergone an independent security audit.

## 1. Status

In code today, and tested:

* **`havenkeys-core`** — the account-bound key scheme (`docs/crypto.md`,
  "Key scheme 3"); the schema 4 local store (`account`, `items`, `settings`
  — no `tombstones` table, no `dirty` columns); `stage_create` /
  `stage_update` / `stage_delete` / `stage_save_login` / `stage_import` and
  `commit_write`, which seal an item's blobs and record the revision a
  server assigns them; `apply_remote_changes`, which applies a pull as an
  upsert or a deletion per item, in cursor order, not a merge; and
  `encode_account_header` / `adopt_account_header` / `prepare_sign_in`.
* **`havenkeys-server`** — activation from a single-use invite, sessions,
  the vault header, paged pull, optimistic per-item writes, devices and the
  admin CLI, tested against a real Postgres (`scripts/test-server.sh`):
  account isolation on every authenticated route, uniform answers from
  `auth/params`, rate limiting, size and shape limits, and a guard that no
  secret reaches a log line. See `docs/deployment.md`.
* **`havenkeys-sync-client`** — the HTTP client, with a hostile-server suite
  and a round trip that runs real core crypto against the real server and a
  real Postgres.
* **The desktop** — activation, second-device sign-in, the Emergency Kit,
  Account settings (devices, revoke, sign out), the online/offline
  distinction with a read-only gate on every mutating command, and a pull
  every 60 seconds while unlocked and online.
* **The browser extension** — unchanged trust boundary; its save-login
  writes through the same staged path.

**Not done, and blocking before a real vault is stored:** the backup restore
drill in `docs/deployment.md` §5 has not been run, and nothing has yet
crossed a real network between two devices. The local replica follows the
server, deletions included, so an untested backup means the vault has none.

## 2. Model

```text
                              Untrusted transport
 Device A ──┐                                              ┌── Device B
            │  HTTPS, session token from login              │
            ├──────────────►  havenkeys-server  ◄───────────┤
            │                 (Postgres, single             │
            │                  authoritative copy)           │
            ▼                                                ▼
   local replica (SQLite)                            local replica (SQLite)
   read-only w.r.t. sync:                             read-only w.r.t. sync:
   a pull overwrites rows,                             a pull overwrites rows,
   never merges them                                   never merges them
```

* The server is the single writer. A device's local database is a cache of
  what the server has last told it, plus whatever it has staged but not yet
  had accepted.
* There is no folder to point at a cloud drive, no per-device snapshot file,
  and no client-side merge. The "two devices never write the same file"
  model in the superseded `sync.md` is gone along with the folder transport.
* The server never sees plaintext. Item blobs are the same per-item
  AES-256-GCM ciphertexts the local store already used, copied to and from
  the server without re-encryption, bound to the vault ID and item ID
  (`docs/crypto.md`, "Associated data"). It cannot derive the KEK or the
  data key: neither the master password nor the Secret Key is ever sent to
  it (the auth key it does receive authenticates a login and unwraps
  nothing).

## 3. Secret Key and Emergency Kit

Unchanged in substance from the superseded document: every account vault has
a Secret Key, 128 random bits from the OS CSPRNG, shown as
`H1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX` (Base32 of the 16 bytes plus 2 typo
check characters — `docs/crypto.md`, "Key hierarchy"). It is generated once,
at activation, and is not something you can add or remove afterward: every
vault is account-bound from the moment it exists.

* Each device keeps the Secret Key in the OS keychain (Windows Credential
  Manager, macOS Keychain, the Secret Service on Linux), keyed by account ID.
  `device.json` (same folder, mode 0600) is the fallback when no keychain is
  available, a call to it errors, or it does not answer within 5 seconds — the
  file always still holds the device ID. §7 below covers the fallback and its
  limits.
* The Emergency Kit is a printable page with the Secret Key and a QR code of
  it. The desktop generates its payload as
  `havenkeys://kit/v1?vault=<vault id>&key=<Secret Key>` today
  (`apps/desktop/src-tauri/src/account.rs`). The design's kit v2, which also
  carries the account ID, email and server URL so a second device needs
  nothing else, is part of the activation/sign-in work that has not landed
  (§1).
* Losing both the Emergency Kit and every device that holds the Secret Key
  means losing the vault — there is no account-recovery path.

## 4. Online, offline and read-only

`AppState` tracks connectivity independently of the lock state
(`apps/desktop/src-tauri/src/state.rs`):

```text
Offline   no server session: reads work, every mutating command refuses
Online    a live server session: reads and writes
```

A locked vault is never online. An unlocked one may still be offline — the
two states are orthogonal, which is why this is a separate flag rather than
a third lock state. Every mutating Tauri command checks
`AppState::require_online` before doing anything else and returns
`Error::Offline` ("HavenKeys is offline — the vault is read-only until it
reconnects.") when it fails. The desktop shows a persistent banner while
unlocked and offline, and disables the controls that would otherwise fail
(`apps/desktop/src/App.tsx`, `VaultScreen.tsx`).

Because there is no sync client yet (§1), `Connectivity` has no path to
`Online` in this build — it is always `Offline`. That is not a placeholder;
it is the real, correct behaviour of a device with no server session. The
device status the desktop reads (`device_status`) reports it as
`online: false` so the UI can show the banner from the very first unlock.

Losing connectivity mid-session is designed to drop to `Offline` without
locking the vault: the replica stays readable and autofill keeps working.
The extension inherits this — fill, TOTP and password generation work
offline; a save-login prompt should not be shown while offline, because
accepting it would only fail.

## 5. Writing

Writes follow the same ticket idiom as unlock and rekey: prepare under the
vault lock, do the part that can fail or block outside it, commit.

```text
VaultService::stage_create(input, now_ms)     ┐
VaultService::stage_update(&id, input, now_ms)├─► StagedWrite { item_id, base_revision, overview, details }
VaultService::stage_delete(&id)                ┘
        │
        │  (a sync client sends this to POST /v1/items and gets back
        │   the revision the server assigned)
        ▼
VaultService::commit_write(staged, revision)  →  persists the row locally
```

* `stage_*` seals the blobs with the session's data key and touches nothing
  on disk; it fails if the vault is locked. `base_revision` is the revision
  this device last saw for the item, or `None` for one it believes is new —
  the server's optimistic-concurrency token.
* `commit_write` re-checks the lock epoch, exactly as `commit_rekey` does,
  so a lock that happens mid-request discards the result instead of writing
  a row sealed under a session that no longer exists.
* A conflicting write on another device comes back as the server's 409,
  which the sync client turns into `SyncError::Conflict` naming the items and
  the desktop reports as `Error::ItemChangedElsewhere`. Nothing was written,
  so the user's edit is neither silently applied nor silently lost.
* Import is the same path, in batches of at most 500 items, each atomic on
  the server.
* A save from the browser extension is the same path too
  (`stage_save_login`), with the same origin binding it always had.

Every mutating desktop command checks `require_online` before it stages, so
an offline device refuses in the UI rather than failing at the network.

## 6. Syncing

```text
unlock (online):   derive keys → unwrap local header → login
                    → adopt the server's header if newer and attested
                    → pull everything since the stored cursor
after a write:      pull (the write's response already carries the new cursor)
while unlocked:     pull every 60 s
unlock (offline):   derive keys → unwrap local header → read-only
```

Applying a pull is **not a merge**. `apply_remote_changes` upserts
`(overview, details, revision)` or deletes a row, per item, in cursor order.
The only judgement it makes is structural: a blob that does not open under
this vault's data key or that carries another item's ID is counted in
`SyncReport.skipped_items` and leaves the existing row alone — a hostile or
malfunctioning server can fail to update the replica, but cannot corrupt it
into serving forged plaintext. `apply_remote_changes` itself imposes no size
limit on `overview`/`details` — the bounds are applied before that, in
`havenkeys-sync-client`: a page larger than 500 changes is refused, a blob
over 8 MiB is refused while still base64 (so an oversized one costs a
comparison, not an allocation), and the whole response is capped at 17 MiB
as it is read. What neither layer does is re-apply the *field-level* limits a
local write goes through (`MAX_NOTE_CONTENT_BYTES`, `MAX_PASSWORD_CHARS`, …)
to what the server sends; an item that is within the blob cap but larger than
the UI would ever produce is stored as served. The cursor advances
unconditionally once the batch is applied,
including past skipped items; there is no local merge state (`dirty` flags,
tombstone rows) left to reconcile, because the server is the only writer.
See §7 for what that unconditional advance costs a skipped item.

The account header a device publishes (`encode_account_header`) and adopts
from another device (`adopt_account_header`) works as before: a header is
only adopted if its attestation verifies under this vault's data key (proof
a vault-key holder wrote it), the key scheme has not gone backwards, and its
revision is strictly newer than both the local header and the durable
`max_header_rev` floor — which stops a genuine old header from being replayed
to undo a master-password change.

**Changing the master password** goes through a dedicated route,
`POST /v1/account/credentials`, rather than through the general header write.
It replaces the account's KDF parameters, its login verifier and the vault
header in one server transaction: the caller proves it knows the *current*
auth key (checked and rate-limited exactly like a login, so a stolen session
token cannot change the password), `baseHeaderRevision` must match the
server's, and on success every other session on the account is deleted — a
password is usually changed because something leaked. The device making the
change commits its local rewrap only after the server has accepted it, at the
revision the server returned; nothing local changes on a failure. Every other
device is now signed out. It picks up the new password the next time it
unlocks: local unlock with the new password fails, and, if the server is
reachable, an online fallback derives the login key from the server's current
KDF, signs in, and verifies the served header (vault ID, scheme, attestation)
before adopting it — a wrong password still costs only a local Argon2id run
plus this fallback's own, so it can add a few seconds before "wrong password"
is shown. A device that unlocks offline with the old password stays usable
locally but is told, once, that the password changed elsewhere and it needs
to unlock again online.

## 7. Security properties and limitations

**Protected:**

* Confidentiality: every item blob the server stores or serves is
  AES-256-GCM ciphertext under the vault's data key. The server never
  receives the master password, the Secret Key, the KEK or the data key.
* Integrity of what a device accepts: a pulled item that does not
  authenticate under the data key is not applied.
* A header replay (an old, genuinely-signed header served again to undo a
  password change) is refused by the `max_header_rev` floor — **on a device
  that has already seen the newer header**. A device signing in for the first
  time has no floor, so a captured pre-rotation header will be accepted there
  by someone who also has the retired master password and the Secret Key.

**Not protected, or worth stating plainly:**

* **Item replay has no equivalent floor.** The header has `max_header_rev`;
  items have nothing. A hostile server can re-serve an item blob it was given
  earlier at a higher revision, and because the blob is genuine it
  authenticates and is applied — silently returning a login to a previous
  password, reported as an ordinary update. Password history (5 entries) is
  what bounds the damage, and a patient server can cycle through it. Adding
  a per-item monotonicity check in `apply_remote_changes` is the fix.

* **The local replica is not a backup.** It follows the server, including
  deletions. A hostile or compromised server can delete an item on every
  device that pulls afterward, including local edits never yet pushed — the
  server is the authority, not a peer being merged with. This is inherent to
  a server-authoritative design, not a bug to fix with more cryptography;
  the mitigation is tested server backups (`docs/deployment.md` §5 — a
  prerequisite of storing a real vault, and the drill has not been run yet)
  and the user noticing.
* **Availability is now a correctness concern.** A server that is down means
  no login can be saved, no password rotated, no item deleted — not merely
  "changes stop propagating," as under the old folder model.
* **The Secret Key lives in the OS keychain**, with `device.json` as a
  fallback. On a device, the master password alone protects the vault against
  someone who can read that device's files; the Secret Key protects every
  copy that is not on one of your devices — the keychain does not change
  that, it only moves the fallback file's contents somewhere the OS is
  supposed to protect better. In practice, any process running as the same
  user can usually read a Linux Secret Service or Windows Credential Manager
  entry too; macOS may prompt for access. When no keychain is available, or a
  call to it fails or does not answer within 5 seconds, the Secret Key is
  written to `device.json` instead (0600) and the desktop shows this in
  Settings → Account as `secretKeyStorage: "file"`. A key found in
  `device.json` at startup is moved to the keychain when one is available and
  removed from the file once the move is confirmed. A `set` that timed out
  and completes later could, in principle, re-add a keychain entry that
  "Remove this device" had just deleted — a narrow race, accepted.
* **The vault key is never rotated** (`security-review.md` #8). A master
  password change re-wraps the vault key; it does not replace it.
* **A pulled item that fails to decrypt is retried, not lost.** A change from
  the server that does not authenticate under the data key, or a non-deleted
  change with no blob, is recorded in a local `unreadable_items` table
  instead of only being counted — in the same transaction as the rest of that
  pull's changes, so the cursor and the retry list never disagree. At the end
  of every `sync_now`, the app fetches those IDs from `POST /v1/items/fetch`
  (chunks of up to 500) and applies them through the same path, without
  moving the cursor; an item that now decrypts, or that the server has since
  deleted, leaves the table. The vault screen shows a persistent banner while
  any remain, with a **Re-download** action. Item-level replay is still not
  addressed by this — a server that keeps re-serving the *same* stale blob at
  a higher revision is not an unreadable item; it is the "Item replay has no
  equivalent floor" limitation above.
* **Metadata visible to the server:** the vault ID, the account's email, the
  KDF parameters and salt, the wrapped vault key, item revisions, and the
  number and rough size of items. It cannot read any of it, but it can see
  that it exists and roughly how much of it there is. It also records failed
  login attempts per account and per client address in `login_attempts` for
  rate limiting, cleared on a successful login (`docs/deployment.md` §6).

## 8. For a future mobile app

Unchanged in intent from the superseded document: scan the Emergency Kit QR
(or type the Secret Key), sign in with the account's email and the master
password, and reuse `havenkeys-core` (for example through UniFFI) rather
than reimplementing the key derivation or the wire formats. Store the Secret
Key in the platform keystore (iOS Keychain, Android Keystore) rather than a
plaintext file. Argon2id parameters come from the header and must be
benchmarked on real phones (`docs/crypto.md`, "Argon2id parameters").
