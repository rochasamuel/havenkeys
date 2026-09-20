# Server-authoritative vault — Design

Status: accepted, 2026-09-20.
Supersedes `2026-09-19-server-accounts-sync-design.md`, which kept the local
SQLite authoritative and treated the server as transport. The sections that
survive unchanged are cited rather than repeated.

> This software has not undergone an independent security audit.

## 1. Why

The previous design carried two sync transports' worth of machinery: a folder
model with per-device snapshots and wall-clock conflict resolution, and a
delta model with a cursor, dirty flags and a merge function that had to decide
between two versions of an item. Both existed because every device held an
authoritative copy and any two of them could diverge.

Making the server the single writer removes the divergence, and with it the
merge. A device no longer decides anything: it reads what the server has and
sends writes the server serializes. That deletes more code than it adds.

What it costs is stated plainly in §9 and §10, and it is not small: the
application stops working fully offline, which the project charter previously
required.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Source of truth | The server | The local SQLite (previous design) |
| Offline | Encrypted read-only replica on each device | Nothing local (cannot unlock, cannot autofill); a local write queue (reintroduces merge) |
| Local-only vaults | Removed; every vault belongs to an account | Two modes (local-only and account) |
| Existing vaults | Clean break: schemes 1 and 2 are not readable | Migration 2 → 3 |
| Write path | Synchronous HTTP; the server assigns the revision | A local queue flushed later |
| Concurrency | Optimistic, per item: the write carries the item's known revision | Last write wins |
| Server's view of secrets | Blind relay: ciphertext and metadata only | A server able to decrypt |
| Accounts | Closed: admin CLI, single-use invite | Public signup |
| Server auth | Derived auth key over TLS, verified as Argon2id | OPAQUE, SRP-6a |
| Server stack | Rust + axum + sqlx + Postgres, in this monorepo | Node + Fastify |

## 3. What does not change

* The encrypted blob format and its AAD binding (`docs/crypto.md`). Blobs the
  server stores are byte-identical to the ones on disk; it holds nothing it
  could open.
* Key scheme 3's derivation: `MK = Argon2id(password)`, `KEK =
  HKDF(MK ‖ Secret Key, salt = account_id ‖ email, info = "havenkeys/v3/kek")`,
  and the auth key from the same run under `"havenkeys/v3/auth"`.
* The Secret Key and the Emergency Kit. Scheme 3 depends on the Secret Key;
  removing local vaults does not remove it.
* The header attestation and the `max_header_rev` rollback guard. The server
  is untrusted, so a header it serves is still verified and still cannot go
  backwards.
* The vault lock state machine, auto-lock, and the OS lock integration.
* The browser extension's trust boundary: it talks only to the desktop app
  over native messaging, never to the server, and still receives one login at
  a time for the page it is on, origin-checked in Rust.
* `havenkeys-core` keeps **no network dependency**. All HTTP lives in a new
  crate.

## 4. Identity and key hierarchy

Key scheme 3 as specified in `docs/crypto.md`, "Key scheme 3 (account)", and
in the superseded spec §4.1, unchanged — except that it is now the **only**
scheme. `KeyScheme` loses `PasswordOnly` and `PasswordAndSecretKey`; the
header keeps the `key_scheme` field for forward compatibility and rejects any
value but 3.

## 5. Account lifecycle

Provisioning (admin CLI, single-use invite), activation, second-device sign-in
and later unlocks are as in the superseded spec §5, with two changes:

* there is no folder join path, so `prepare_join` and the "set up from a sync
  folder" screen are gone;
* activation is the **only** way to create a vault. The first-run screen asks
  for an invite string, not just a master password.

The Emergency Kit payload stays at v2:

```text
havenkeys://kit/v2?account=<uuid>&email=<pct-encoded>&key=<H1-…>&server=<https url>
```

## 6. Sessions and devices

As in the superseded spec §6, unchanged: no long-lived server credential on
disk, the auth key re-derived at every unlock, a 32-byte opaque token held in
memory only and zeroized on lock, 24-hour expiry, device revocation.

One consequence is now load-bearing rather than incidental: **locked ⇒ no
token ⇒ no writes**, and **no token ⇒ read-only**, which is the same state an
offline device is in. The client has one code path for both.

## 7. Server

Schema, API shape, enforced rules, write path, logging and CORS as in the
superseded spec §7, with the changes below.

### 7.1 Schema changes

`items` already carries a per-item `revision`; it becomes the optimistic
concurrency token, returned to clients and required on writes.

The `tombstones` concept stays server-side only: a deleted item keeps its row
with `overview` and `details` NULL and `deleted_at` set. Clients no longer
keep tombstone rows (§8.2).

### 7.2 API changes

Pull is unchanged:

```text
GET /v1/sync?since=<cursor> → { cursor, hasMore, changes: [ { itemId, revision, overview, details, deleted } ] }
```

`deleted` is a boolean. The client does not need `deletedAt`: it deletes the
row and keeps no tombstone, so there is nothing for a timestamp to decide.
This removes the unauthenticated-`deleted_at` hazard documented in
`docs/crypto.md` — not by authenticating it, but by deleting the code that
consumed it (see §9 for what remains true).

Writes replace `POST /v1/sync`:

```text
POST /v1/items
  { changes: [ { itemId, baseRevision, overview, details }
             | { itemId, baseRevision, deleted: true } ] }
  → 200 { cursor, applied: [ { itemId, revision } ] }
  → 409 { conflicts: [ { itemId, revision } ] }
```

* `baseRevision` is the revision the client last saw for that item, or `null`
  for an item it believes is new.
* The server refuses the **whole batch** if any item's stored revision differs
  from its `baseRevision`, and names the conflicting items. Partial
  application would leave the client unable to say what happened.
* One transaction per request, bumping `vaults.revision` once and stamping
  every touched row with it, so the pull cursor stays monotonic and no reader
  observes half a batch (superseded spec §7.4, unchanged).

### 7.3 Rules

As in the superseded spec §7.3, with `baseCursor == vaults.revision` replaced
by the per-item `baseRevision` check above. Everything else holds: identity
from the session token and never from the body, `deny_unknown_fields`, size
limits (8 MiB per blob, 16 MiB per body, 500 changes per batch, 64 devices),
`PUT /v1/vault/header` requiring `revision + 1`, rate limiting, uniform
answers from `auth/params`, blobs never parsed.

## 8. Client

### 8.1 Crates

* `havenkeys-core` — vault, crypto, local replica, no network.
* `havenkeys-sync-client` — new: HTTP against the server, `reqwest` with
  `rustls`, no `openssl`. Owns the session token and zeroizes it on drop.
* `havenkeys-desktop` — wires the two and owns the UI state, including the
  online/offline distinction.

### 8.2 Local store (schema 4)

A clean schema, not a migration. `Store::init` refuses `user_version < 4`
with a message that says the vault predates accounts and cannot be opened.

```sql
vault_header (id=1, format_version, vault_id, kdf, wrapped_vault_key,
              created_at, key_scheme, header_revision)
items        (id PRIMARY KEY, overview BLOB, details BLOB, revision INTEGER)
account      (id=1, account_id, email, server_url, server_cursor,
              max_header_rev, last_synced_at)
settings     (id=1, blob)
```

Gone from schema 3: the `tombstones` table, the `dirty` columns on `items`
and `tombstones`, and the 1→2 and 2→3 migrations.

`items.revision` is the server's revision for that row: what a write sends as
`baseRevision`, and what a pull overwrites. A row the client has never
successfully written has no local existence — there is no "pending" state.

Settings (auto-lock minutes and the like) stay **device-local and unsynced**,
encrypted under the data key as today. They are not vault content and the
server has no business holding them.

### 8.3 Reading

Every read — list, search, reveal, TOTP, autofill — is served from the local
replica and works offline. This is unchanged code: the session's overview
cache and the details blobs are what they already are.

### 8.4 Writing

Writes follow the ticket idiom the codebase already uses for unlock and
rekey: prepare under the vault lock, do the slow or fallible part outside it,
commit.

```text
VaultService::stage_write(input, now_ms)   → StagedWrite { item_id, base_revision,
                                                           overview, details }
   ↓  (sync client: POST /v1/items)
VaultService::commit_write(staged, revision) → persists the row locally
```

* `stage_write` seals the blobs with the session's data key and touches
  nothing on disk. It fails if the vault is locked.
* The sync client sends them and returns the revision the server assigned.
* `commit_write` writes the row and its revision. It re-checks the lock epoch,
  as `commit_rekey` does, so a lock during the request discards the result.
* A 409 surfaces as `Error::ItemChangedElsewhere`. The desktop pulls, tells
  the user that the item changed on another device, and shows the current
  value rather than overwriting it. The user's edit is not silently applied
  and not silently lost.
* Deletion is the same path with a `deleted` change.
* Import is the same path in batches of at most 500.

Nothing stages a write while offline: the desktop refuses before reaching the
core (§8.6).

### 8.5 Syncing

```text
unlock (online):  derive keys → unwrap local header → login → GET header
                  → adopt if newer and attested → GET /v1/sync?since=cursor
after a write:    pull (the write already returned the new cursor)
while unlocked:   pull every 60s
unlock (offline): derive keys → unwrap local header → read-only
```

Applying a pull is not a merge. Each change is an upsert of `(overview,
details, revision)` or a row deletion, applied in cursor order. The only
validation is structural: blob size limits, and a blob that fails to open
under the vault key is counted in `skipped_items` and leaves the previous row
alone (a server cannot corrupt the replica by serving garbage, it can only
fail to update it).

`decide_merge`, `pending_push`, `confirm_push` and the `dirty` bookkeeping are
deleted.

### 8.6 Online, offline and read-only

`AppState` gains a connectivity state alongside the lock state:

```text
Offline            no token: reads work, writes refused
Online { token }   reads and writes
```

Every mutating Tauri command checks it first and returns `Error::Offline`
with "HavenKeys is offline — the vault is read-only until it reconnects."
The UI shows a persistent banner and disables the affected controls rather
than letting a click fail.

Losing connectivity mid-session drops to `Offline` without locking: the
replica is still readable and autofill keeps working.

The extension is unchanged, and inherits the behaviour: fill, TOTP and
generation work offline; the save-login prompt is not shown when offline,
because accepting it would fail.

### 8.7 Desktop UI

* **First run:** paste the invite string → choose a master password →
  Emergency Kit screen, which must be confirmed saved.
* **Second device:** server URL, email, master password, Secret Key.
* **Settings → Account:** email, server, device list, revoke device, sign out.
* **Removed:** the folder picker, "set up from a sync folder", and the
  Secret Key set-up block (every vault has one from activation).

## 9. Threat model delta

Everything in the superseded spec §9 still applies. What changes:

**The local replica is not a backup.** It follows the server, including
deletions. Previously each device's SQLite was an independent authoritative
copy and was named, in that spec §10, as the real backup until Postgres
backups were configured and a restore tested. That safety net is gone, which
makes tested server backups a **prerequisite** of shipping, not a follow-up.

**A hostile or compromised server can destroy data**, and this is now
inherent rather than a flaw to fix. Under the previous design a server
forging a deletion was exceeding its authority, which is why authenticated
tombstones were a blocker. Here the server *is* the authority; a deletion it
serves is indistinguishable from a real one by construction. The mitigation
is backups and the user noticing, not cryptography. Stated plainly in
`threat-model.md`.

It still cannot **read** anything: blobs are sealed under keys derived from
the master password and the Secret Key, neither of which it ever sees.

**Availability is now a correctness concern, not only a convenience.** A
server that is down means no new logins can be saved, no passwords rotated,
no items deleted. The previous design degraded to "changes stop propagating";
this one degrades to "changes cannot be made".

**Accepted limitations**, carried forward: no account recovery, the vault key
is never rotated (`security-review.md` #8), the Secret Key is stored in plain
text in `device.json`.

## 10. Charter change

This design contradicts `CLAUDE.md` §1, which requires local-first operation,
full offline capability and no backend. Those lines are amended as part of
the work, not left to contradict the code:

* "works completely without an internet connection" → "reads work without an
  internet connection; changes require the server".
* "no backend required" → removed; the backend is part of the product.
* `README.md` can no longer say the application runs no server.
* `threat-model.md` gains the server as an actor and §9 above as its section.

Shipping code that contradicts the documented security model is the failure
mode `CLAUDE.md` §49 and §56 exist to prevent, so the documents move with the
code, in the same change.

## 11. What is deleted

* `havenkeys-core::sync::folder`, `prepare_join`, the folder UI, and the
  folder parts of `docs/sync.md` (which becomes `docs/server-sync.md`).
* `KeyScheme::{PasswordOnly, PasswordAndSecretKey}` and everything reachable
  only from them: `derive_kek`, `derive_kek_with_secret_key`,
  `derive_secret_key_upgrade`, `derive_account_upgrade`,
  `commit_account_upgrade`, `prepare_new_vault`,
  `prepare_new_vault_with_secret_key`, `setup_secret_key`.
* The schema 1→2 and 2→3 migrations, the `tombstones` table and the `dirty`
  columns.
* `decide_merge`, `apply_merge`'s conflict arms, `pending_push`,
  `confirm_push`, and the merge cases in `tests/sync.rs` and
  `tests/delta_sync.rs`.

Deleting these is the point of the change, not a side effect: the surviving
code has one key scheme, one transport and no conflict resolution.

## 12. Testing

**Server**, as in the superseded spec §12: account isolation against every
authenticated route, auth failures, uniform `auth/params`, size and shape
limits, rate limiting, invite single-use and expiry, `no_logging`, all
against a real disposable Postgres. Plus, for this design: a stale
`baseRevision` is refused for the whole batch and names the conflicting
items.

**Client, with a hostile server:**

* a header whose attestation does not verify is refused;
* a header with a lower revision than `max_header_rev` is refused;
* a blob that fails to open leaves the existing row intact and is counted;
* an oversized blob or body is refused before allocation;
* a pull that deletes everything is applied (it is the server's authority)
  but the report says how many rows were deleted, so the UI can say so.

**Offline:**

* unlock, search, reveal, TOTP and autofill work with the network refused;
* every mutating command returns `Error::Offline` and writes nothing;
* losing connectivity mid-session does not lock the vault.

**Concurrency:** two clients writing the same item — the second gets 409, the
desktop surfaces it, and no version is silently lost.

## 13. Order of work

Each step below is too large for one implementation plan and gets its own,
written when the previous step lands and its surprises are known. This spec
is the shared contract between them.

1. **Core**: schema 4, single key scheme, `stage_write`/`commit_write`, the
   pull applier; delete everything in §11. No network. Tests pass at the end
   of this step with the vault unusable for writes, which is expected.
2. **`havenkeys-server`**: schema, auth, routes, admin CLI, tests against
   Postgres. Deploy to Railway; confirm the health check, TLS, and a
   **restored backup** (§9 makes this blocking).
3. **`havenkeys-sync-client`**: the HTTP client and the hostile-server suite.
4. **Desktop**: activation, sign-in, Emergency Kit, Account settings, the
   offline banner and read-only enforcement; remove the folder UI.
5. **Docs**: `CLAUDE.md` §1, `README.md`, `threat-model.md`,
   `security-model.md`, `crypto.md`, `architecture.md`, `sync.md` →
   `server-sync.md`, `roadmap.md`. Then a security review pass over the whole
   change, written to `security-review.md`.

## 14. Out of scope

Public signup, email verification, password reset, account recovery,
server-side search, sharing, passkeys, mobile, offline writes, and any change
to the browser extension's trust boundary.
