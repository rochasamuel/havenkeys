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

What exists in `havenkeys-core` today:

* the account-bound key scheme (`docs/crypto.md`, "Key scheme 3");
* the schema 4 local store (`account`, `items`, `settings` — no
  `tombstones` table, no `dirty` columns);
* `VaultService::stage_create` / `stage_update` / `stage_delete` and
  `commit_write`, which seal an item's blobs and record the revision a
  server assigns them;
* `apply_remote_changes`, which applies a pull from the server as an upsert
  or a deletion per item, in cursor order — not a merge;
* `encode_account_header` / `adopt_account_header`, which publish and adopt
  the attested header a device uses to prove its identity to a new device.

What does not exist yet: `havenkeys-server` (no server has been built),
`havenkeys-sync-client` (no HTTP client sends a staged write or pulls a
change), and the desktop screens for activation, second-device sign-in and
account settings. Concretely, this means:

* every vault-mutating desktop command (`create_item`, `update_item`,
  `delete_item`, `change_master_password`, import) refuses with
  `Error::Offline`, because there is nothing to send a staged write to;
* the connectivity state described in §4 below is always `Offline`, because
  nothing can ever obtain a server session yet;
* `create_account_vault` (the only way to create a vault) is exercised only
  by tests — there is no invite, no activation screen, and no sign-in screen
  reachable from the desktop UI.

Reading — list, search, reveal a secret, generate a TOTP code, autofill — is
unaffected by any of this and works exactly as before, from the local
replica, whether or not a server is reachable.

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

* Each device stores the Secret Key in its own `device.json`, outside the
  vault database, in plain text (§6 below).
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

## 5. Writing (once a sync client exists)

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
* A conflicting write on another device surfaces as `Error::ItemChangedElsewhere`
  once a sync client exists to detect the server's 409; the desktop is meant
  to pull, tell the user the item changed elsewhere, and show the current
  value rather than silently overwrite or silently drop the edit.
* Import is the same path, in batches of at most 500 items.

Today, every desktop command that would call `stage_*` checks
`require_online` first and returns `Error::Offline` before it does — see §1
and §4. `stage_*`/`commit_write` themselves are exercised only by
`havenkeys-core`'s own tests.

## 6. Syncing (once a sync client exists)

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
limit on `overview`/`details` — the only bound either blob passes through is
`crypto::blob`'s blanket 8 MiB cap on any sealed blob (`MAX_BLOB_LEN`), which
exists to bound decrypting arbitrary bytes under any purpose, not as a
judgement about a reasonable item size. The applier never re-applies the
field-level limits a local write goes through (`MAX_NOTE_CONTENT_BYTES`,
`MAX_PASSWORD_CHARS`, …) to what the server sends. A meaningful size policy
for pulled items, if wanted, is the sync client's job (design §13 step 3),
not the applier's, and it has not been built. The cursor advances
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

## 7. Security properties and limitations

**Protected:**

* Confidentiality: every item blob the server stores or serves is
  AES-256-GCM ciphertext under the vault's data key. The server never
  receives the master password, the Secret Key, the KEK or the data key.
* Integrity of what a device accepts: a pulled item that does not
  authenticate under the data key is not applied.
* A header replay (an old, genuinely-signed header served again to undo a
  password change) is refused by the `max_header_rev` floor.

**Not protected, or worth stating plainly:**

* **The local replica is not a backup.** It follows the server, including
  deletions. A hostile or compromised server can delete an item on every
  device that pulls afterward, including local edits never yet pushed — the
  server is the authority, not a peer being merged with. This is inherent to
  a server-authoritative design, not a bug to fix with more cryptography;
  the mitigation is tested server backups (a prerequisite of shipping, not
  built yet) and the user noticing.
* **Availability is now a correctness concern.** A server that is down means
  no login can be saved, no password rotated, no item deleted — not merely
  "changes stop propagating," as under the old folder model.
* **The Secret Key is stored in plain text** in each device's `device.json`,
  as before. On a device, the master password alone protects the vault
  against someone who can read that device's files; the Secret Key protects
  every copy that is not on one of your devices.
* **The vault key is never rotated** (`security-review.md` #8). A master
  password change re-wraps the vault key; it does not replace it.
* **A skipped item stays stale, permanently and silently.**
  `apply_remote_changes` advances the stored cursor unconditionally, even
  when `skipped_items > 0` for that batch. If the server ever serves one
  corrupt or mislabeled blob for an item, that item is never retried: the
  next pull starts at `since=cursor`, which is already past it. This is
  worse than failing to update — it is failing to update with no visible
  error, forever, on that device. `SyncReport.skipped_items` is the only
  signal that it happened. There is no resync-from-zero path today; the sync
  client (design §13 step 3) needs one — an explicit reset-cursor-to-zero
  operation a user or operator can invoke when a device's replica is
  suspected to be missing an item it should have.
* **Metadata visible to the server:** the vault ID, the account's email, the
  KDF parameters and salt, the wrapped vault key, item revisions, and the
  number and rough size of items. It cannot read any of it, but it can see
  that it exists and roughly how much of it there is.

## 8. For a future mobile app

Unchanged in intent from the superseded document: scan the Emergency Kit QR
(or type the Secret Key), sign in with the account's email and the master
password, and reuse `havenkeys-core` (for example through UniFFI) rather
than reimplementing the key derivation or the wire formats. Store the Secret
Key in the platform keystore (iOS Keychain, Android Keystore) rather than a
plaintext file. Argon2id parameters come from the header and must be
benchmarked on real phones (`docs/crypto.md`, "Argon2id parameters").
