# Design: accounts and server sync

Status: approved design, not yet implemented.
Date: 2026-09-19.

Replaces folder sync (`docs/sync.md`) with a hosted, zero-knowledge sync
server, and replaces "a vault on this machine" with "an account you sign in
to". Login becomes email + master password, plus the Secret Key the first
time a device signs in.

> This software has not undergone an independent security audit.

## 1. Why

Folder sync works but pushes the hard part onto the user: pick a folder, hope
the sync service behaves, live with wall-clock conflict resolution and with
snapshots that rewrite the whole vault on every change. A server owned by the
user removes all three.

What it costs is stated plainly in §9: the server learns metadata the
local-first design never exposed.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Server's view of secrets | Blind relay: ciphertext and metadata only | A server able to decrypt (account recovery, server-side search) |
| Offline | Local SQLite stays authoritative; server is transport | Requiring the network to open the vault |
| Accounts | Closed: created by an admin CLI, activated with a single-use invite | Public signup (needs email delivery, abuse handling, quotas) |
| Folder sync | Removed | Keeping both transports |
| Server auth | Derived auth key over TLS, verified as Argon2id on the server | OPAQUE (`opaque-ke`), SRP-6a (`srp` crate is unmaintained — CLAUDE.md §48) |
| Sync shape | Per-item rows with a monotonic per-vault cursor | Porting the per-device snapshot model to HTTP |
| Server stack | Rust + axum + sqlx + Postgres, in this monorepo | Node + Fastify (would duplicate protocol types and input validation) |

## 3. What does not change

The parts that carry the security properties stay exactly as they are:

* the encrypted blob format and its AAD binding (`docs/crypto.md`);
* `VaultService` and the vault state machine;
* the merge rules in `docs/sync.md` §5, which decide on decrypted content;
* the header attestation;
* the local SQLite store as the vault of record on each device;
* the browser extension, which keeps talking only to the desktop app over
  native messaging and never reaches the server (see §11 for what changes
  when the standalone extension lands).

`havenkeys-core` keeps **no network dependency**. All HTTP lives in a new
crate.

## 4. Identity and key hierarchy (key scheme 3)

### 4.1 Derivation

```text
 email (NFC, lowercased)      master password            Secret Key (128 bits)
        │                          │                            │
        │                  Argon2id(salt, m, t, p)              │
        │                          ▼                            │
        │                   master key (MK)                     │
        │                          │                            │
        ├──────────────┐           ├────────────────────────────┤
        ▼              │           ▼                            ▼
   account_id ─────────┴──► HKDF(ikm = MK ‖ SecretKey,   HKDF(ikm = MK ‖ SecretKey,
                              salt = account_id ‖ email,      salt = account_id ‖ email,
                              info = "havenkeys/v3/kek")      info = "havenkeys/v3/auth")
                                    │                            │
                                    ▼                            ▼
                                   KEK                        auth_key
                                    │                            │
                        unwraps the vault key          sent to the server;
                        (never leaves the device)      never unwraps anything
```

`account_id ‖ email` means the 16 raw bytes of the account UUID followed by
the NFC-normalized, lowercased email as UTF-8. Binding the email in means a
vault is tied to the account it belongs to, the way 1Password binds
derivation to the account id.

KEK and `auth_key` share input keying material but are separated by HKDF
`info` strings, which is what makes handing `auth_key` to the server safe:
it cannot be walked back to MK, to the Secret Key, or to the KEK.

### 4.2 Scheme migration

`key_scheme` 1 (password only) and 2 (password + Secret Key, vault-id salt)
keep opening locally, unchanged. Connecting an existing vault to an account
rewraps the vault key under a scheme 3 KEK:

1. unlock normally (scheme 1 or 2);
2. if the vault has no Secret Key, generate one;
3. derive a fresh Argon2id salt, derive MK, KEK (scheme 3) and `auth_key`;
4. rewrap the vault key, bump `header_revision`, write the header;
5. reissue the Emergency Kit (its payload now carries email and server).

Items are never re-encrypted. The Secret Key bytes are kept as they are —
rotating them would invalidate a kit the user may already have printed, and
they gain nothing from rotation here.

Downgrade is refused: a device on scheme 3 never adopts a scheme 1 or 2
header (the rule that already exists for scheme 2 in `sync.md` §4, extended).

## 5. Account lifecycle

### 5.1 Provisioning (admin)

```sh
havenkeys-server admin new-account --email you@example.com
# prints one invite string, shown once; only the secret's hash is stored
# HKINV1-<base64url of {"server":"https://…","email":"…","account":"<uuid>","secret":"<…>"}>
```

The invite secret is 128 bits from the CSPRNG, stored as SHA-256 (the input
is already high-entropy, so a password hash buys nothing), single-use,
expiring after 7 days. It authorizes activation; it protects no data.

The invite string carries the `account_id` because the client needs it
*before* deriving the KEK (§4.1), and fetching it by email first would turn
activation into an account-enumeration oracle. It also carries the server
URL and email so the user pastes one thing instead of typing four.

### 5.2 First login (activation)

The client does everything; the server only records the result.

```text
user pastes the invite string, then chooses a master password
client: Secret Key ← CSPRNG(16)      vault key ← CSPRNG(32)
        MK, KEK, auth_key            header = wrap(vault key) + attestation
POST /v1/accounts/activate { email, invite, kdf, salt, authKey, vaultId, header }
→ Emergency Kit screen; continuing requires confirming it was saved
```

The Emergency Kit now carries the email and the server URL alongside the
Secret Key, because a new device needs all three. QR payload:

```text
havenkeys://kit/v2?account=<uuid>&email=<pct-encoded>&key=<H1-…>&server=<https url>
```

The server hashes `authKey` on receipt with Argon2id and stores only the PHC
string. It is never stored as received, and never logged.

### 5.3 Second device

```text
user enters: server URL, email, master password, Secret Key
POST /v1/auth/params  { email } → { accountId, kdf, salt }
client derives MK, auth_key
POST /v1/auth/login   { email, authKey, deviceId, deviceName } → { token, vaultId }
GET  /v1/vault/header → header; attestation must verify; KEK must unwrap it
GET  /v1/sync?since=0 → full vault, written into the local SQLite
```

A failure at any step produces one message: *"Email, master password or
Secret Key is incorrect."* The client never says which.

### 5.4 Later unlocks on a known device

Master password only. The Secret Key comes from `device.json`, as today.

## 6. Sessions and devices

**No long-lived server credential is written to disk.** At every unlock the
client holds MK and the Secret Key anyway, so it re-derives `auth_key` and
logs in. The session token lives in memory only and is zeroized when the
vault locks. Locked vault ⇒ no token ⇒ no sync, which extends the rule that
already holds today.

Tokens are 32 opaque random bytes, stored server-side as SHA-256, with a
24-hour expiry. Revoking a device deletes its sessions and blocks new logins
from that device id; the device discovers it on its next request and drops
to local-only.

`device.json` gains the server URL and keeps the device id and Secret Key it
already holds. Device names are user-supplied labels (default "Desktop") —
the client never sends the hostname, which would be metadata the server has
no use for.

## 7. Server

### 7.1 Schema

```sql
CREATE TABLE accounts (
  id                UUID PRIMARY KEY,
  email_normalized  TEXT NOT NULL UNIQUE,          -- NFC + lowercase
  status            TEXT NOT NULL,                 -- invited | active | disabled
  kdf_algorithm     TEXT,                          -- set at activation
  kdf_memory_kib    INTEGER,
  kdf_iterations    INTEGER,
  kdf_parallelism   INTEGER,
  kdf_salt          BYTEA,                         -- 16 bytes, client-chosen
  auth_verifier     TEXT,                          -- PHC string (Argon2id of auth_key)
  invite_hash       BYTEA,
  invite_expires_at TIMESTAMPTZ,
  created_at        TIMESTAMPTZ NOT NULL,
  activated_at      TIMESTAMPTZ
);

CREATE TABLE vaults (
  id              UUID PRIMARY KEY,
  account_id      UUID NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
  header          BYTEA NOT NULL,                  -- header.json + attestation (sync.md §4)
  header_revision BIGINT NOT NULL,
  key_scheme      SMALLINT NOT NULL,
  revision        BIGINT NOT NULL DEFAULT 0,       -- the sync cursor
  created_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE items (
  vault_id   UUID NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  item_id    UUID NOT NULL,
  overview   BYTEA,                                -- NULL once deleted
  details    BYTEA,                                -- NULL once deleted
  deleted_at BIGINT,                               -- unix ms, NULL while live
  revision   BIGINT NOT NULL,
  PRIMARY KEY (vault_id, item_id)
);
CREATE INDEX items_by_revision ON items (vault_id, revision);

CREATE TABLE devices (
  id           UUID PRIMARY KEY,
  account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  created_at   TIMESTAMPTZ NOT NULL,
  last_seen_at TIMESTAMPTZ,
  revoked_at   TIMESTAMPTZ
);

CREATE TABLE sessions (
  token_hash BYTEA PRIMARY KEY,                    -- SHA-256 of the bearer token
  account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  device_id  UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE login_attempts (
  key          TEXT PRIMARY KEY,                   -- "acct:<uuid>" or "ip:<addr>"
  failures     INTEGER NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  blocked_until TIMESTAMPTZ
);
```

Deleted items keep their row with both blobs set to NULL — that is the
tombstone, and it keeps propagating exactly like the local one.

### 7.2 API

```text
POST   /v1/accounts/activate   { email, invite, kdf, salt, authKey, vaultId, header }
POST   /v1/auth/params         { email } → { accountId, kdf, salt }
POST   /v1/auth/login          { email, authKey, deviceId, deviceName } → { token, expiresAt, vaultId }
POST   /v1/auth/logout
GET    /v1/vault/header        → { header, headerRevision, keyScheme }
PUT    /v1/vault/header        { header, headerRevision, keyScheme } → 200 | 409
GET    /v1/sync?since=<cursor> → { cursor, hasMore, changes: [ { itemId, overview, details, deletedAt } ] }
POST   /v1/sync                { baseCursor, changes: [...] } → 200 { cursor } | 409 { cursor }
GET    /v1/devices             → [ { id, name, createdAt, lastSeenAt, current } ]
DELETE /v1/devices/:id
GET    /v1/health
```

### 7.3 Rules the server enforces

These are the server-side equivalent of the origin binding that Rust already
enforces for the extension. The client is not trusted to apply any of them.

* **`account_id` and `vault_id` come from the session token, never from the
  request body.** A body that carries them is rejected rather than ignored.
* Unknown JSON fields are rejected (`deny_unknown_fields`), as in the native
  messaging protocol.
* Size limits: 8 MiB per blob (matching the core's limit), 16 MiB per
  request body, 500 items per `POST /v1/sync` batch, 64 devices per account.
* `PUT /v1/vault/header` requires `headerRevision == current + 1`, and
  refuses a `keyScheme` lower than the stored one.
* `POST /v1/sync` requires `baseCursor == vaults.revision`; otherwise 409
  with the current cursor, and the client pulls, merges and retries.
* Blobs are opaque bytes. The server never parses them.
* Login is rate limited per account and per IP: after 5 failures a growing
  block (1 min, 5 min, 30 min), with the counters in `login_attempts`.
* `POST /v1/auth/params` answers uniformly for unknown emails, with a stable
  salt derived as `HKDF(server_secret, email)` and the default KDF
  parameters, so the endpoint does not enumerate accounts. `login` verifies
  against a dummy verifier in that case so the timing matches.
* `auth_key` is verified with Argon2id at modest parameters (19 MiB, t=2,
  p=1). The input is 256 bits of HKDF output, not a password, so the hash is
  there to protect a stolen database, not to slow a guessing attack; heavy
  parameters here would only be a denial-of-service lever on the login route.

### 7.4 Write path

Each mutating request runs in one transaction:

```sql
UPDATE vaults SET revision = revision + 1 WHERE id = $1 RETURNING revision;
-- stamp every inserted/updated/deleted item row with that revision
```

so the cursor is monotonic per vault and a reader can never observe half a
batch.

### 7.5 Logging

Structured logs keyed by request id, `account_id` and `device_id`. Never:
tokens, `auth_key`, invite codes, blobs, emails (the account id identifies
the account; the email adds nothing but exposure). The server crate gets a
`no_logging` test in the spirit of `crates/havenkeys-core/tests/no_logging.rs`.

### 7.6 CORS

Denied by default — no browser origin is allowed. The standalone extension
(§11) will need exactly one origin allowed, `chrome-extension://<id>`, as a
deliberate configuration entry, never a wildcard.

## 8. Client

### 8.1 Crates

* `havenkeys-core` — unchanged responsibilities, plus key scheme 3 and the
  delta merge entry point. Still no network, still fuzzable.
* `havenkeys-sync-client` — **new**. HTTP over `reqwest` with `rustls`,
  protocol types shared with the server through `havenkeys-protocol`. The
  transport is behind a trait so a `wasm32` build can swap `reqwest` for
  `fetch` later (§11). Requires `https://`; plain HTTP only under a
  debug-build environment flag for local development.
* `havenkeys-server` — **new**. axum + sqlx + Postgres, plus the admin CLI.

### 8.2 Local store changes

A schema 2 → 3 migration in `store.rs`, in the style of the existing
`MIGRATE_1_TO_2` (`user_version` bump, batch executed in one transaction):

```sql
ALTER TABLE items ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tombstones ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
CREATE TABLE account (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    account_id     TEXT    NOT NULL,
    email          TEXT    NOT NULL,     -- as typed, for display
    server_url     TEXT    NOT NULL,
    server_cursor  INTEGER NOT NULL DEFAULT 0,
    max_header_rev INTEGER NOT NULL DEFAULT 0,
    last_synced_at INTEGER
);
```

`dirty` marks rows changed locally since the last confirmed push; existing
rows default to 1 so a migrated vault uploads itself once. A push clears it
only for the rows the server acknowledged. `max_header_rev` is the rollback
guard from §8.4.

`HeaderRecord` gains no fields — `key_scheme` already exists and takes the
new value 3. `prepare_join` becomes `prepare_sign_in`, accepting scheme 3
and taking `account_id` and the email alongside the password and Secret Key.

### 8.3 Sync cycle

```text
unlock → login → pull(since = server_cursor) → merge → push(dirty, baseCursor) → store cursor
       on 409: pull again, merge, retry (bounded retries, then back off)
```

Merge keeps the rules from `sync.md` §5 verbatim: every remote version must
authenticate under the data key, the overview's id must match, the details
type must match the overview type, and the newest `updatedAt` wins with a
deletion winning ties. Those rules are already tested; only their input
changes, from device snapshots to a delta stream.

The cursor decides **what to fetch**, not who wins. Conflicts are still
decided on decrypted content, which is the only place the client can judge
them honestly.

Sync runs after unlock, shortly after each change, every minute while
unlocked, and on demand. Never while locked. Failures are silent and
retried; the UI shows the last successful sync time.

### 8.4 The server is treated as hostile

* A header with weakened Argon2id parameters or a forged wrapped key fails
  at the unwrap: different parameters give a different KEK, and AES-256-GCM
  rejects it. The floors in `crypto.md` reject absurd parameters before any
  derivation work is spent. The attestation then authenticates the rest of
  the header body — vault id, key scheme, revision — which the unwrap alone
  does not cover. Both checks must pass before a header is adopted. This is
  split across two functions: `adopt_account_header` (an already-unlocked
  device considering a header the server served later) verifies the
  attestation only — it holds no password and so cannot unwrap; it is
  `prepare_sign_in`, run at login with the password in hand, that performs
  both checks.
* **Header rollback:** an old header is genuinely attested, so the server
  could serve one from before a master-password change and make the previous
  password work again. The client therefore records the highest
  `header_revision` it has ever accepted and refuses anything lower. The
  server enforces the same on writes (§7.3); the client check is the one
  that matters, since the server is the attacker here.
* Every item blob is AEAD-bound to the vault id, item id and role, so the
  server cannot move a blob between items, roles or accounts undetected.
* An old item replayed by the server loses the merge, because its
  `updatedAt` is older.
* JSON from the server is untrusted input and is parsed under the same size
  and unknown-field rules applied to the extension's messages.

### 8.5 Desktop UI

New or changed screens:

* **First run** — *Sign in* or *Activate with an invite*. The "set up from a
  sync folder" path is removed.
* **Activate** — server URL, email, invite, new master password → Emergency
  Kit, with a confirmation checkbox before continuing.
* **Sign in on a new device** — server URL, email, master password, Secret
  Key.
* **Unlock** — unchanged: master password only.
* **Settings → Account** — email, server, last sync, *Sync now*, device list
  with revoke, Emergency Kit, change master password, and *Sign out on this
  device* (removes the device server-side and deletes the local copy, behind
  an explicit confirmation).

Tauri commands: `choose_sync_folder`, `stop_sync`, `pick_join_folder` and
`join_synced_vault` are replaced by `activate_account`, `sign_in`,
`sync_now`, `list_devices`, `revoke_device`, `sign_out`.

### 8.6 Removed

`havenkeys-core::sync::folder`, the folder picker UI in `SyncSection.tsx`,
the folder parts of `docs/sync.md`, and the folder-specific cases in
`crates/havenkeys-core/tests/sync.rs`. The merge tests stay, rewired to the
delta path.

## 9. Threat model delta

### What the server learns

The email, `account_id`, `vault_id`, **how many items exist**, the size of
each blob, **the time of every change**, device labels and ids, the IP
addresses that connect, and the KDF parameters and salt. This is materially
more than the local-first design exposed, and it is the price of the
decision. It goes into `docs/threat-model.md` as its own section, not as a
footnote.

### What it still cannot do

Read any item content, title, URL, username, password, note or TOTP secret;
learn the master password or the Secret Key; or forge a header the client
will accept.

### New attackers to consider

| Attacker | Gets | Blocked by |
|---|---|---|
| Stolen Postgres dump | Ciphertext, metadata, Argon2id verifiers | Content is AEAD under the vault key; verifiers are not `auth_key` |
| Actively compromised server | The above, plus `auth_key` at login time | `auth_key` authenticates only; it unwraps nothing, and it does not speed up guessing the vault (128 unknown Secret Key bits remain) |
| TLS interception | Metadata, `auth_key`, denial of service | Content stays AEAD; header forgery fails attestation |
| Railway as infrastructure operator | Same as a Postgres dump | Same |

### Accepted limitations

* **No account recovery.** Losing the master password or the Secret Key
  loses the vault. The server holds nothing that can recover it, by design.
* **The vault key is still never rotated** (`security-review.md` #8).
  Someone who once held a copy plus both secrets can read later copies.
* **Availability depends on the server.** Reading and editing work offline,
  but changes stop propagating while it is down.
* **The Secret Key is still stored in plain text in `device.json`**, as
  before.
* **A hostile or compromised server can delete items on every device.**
  Deletions in the delta format are not authenticated: `RemoteChange.deleted_at`
  is a plaintext field the server supplies, with nothing sealing it to the
  data key the way item content is, and unlike the folder model's
  tombstones, which travelled inside a snapshot blob only a vault-key holder
  could produce. A server that sends a `deleted_at` for any item ID, real or
  invented, deletes that item everywhere the change is pulled. This is data
  destruction, not the "refuses or delays propagation, each device keeps its
  own copy" failure mode the folder model had. Authenticating deletions — a
  sealed tombstone format, a store migration, and the matching server
  schema — is required before this ships, and is tracked as a blocker on the
  server plan, not on Task 8.

## 10. Deployment

Railway: one service built from a `Dockerfile` in `crates/havenkeys-server`,
plus the managed Postgres plugin. `DATABASE_URL` and a `SERVER_SECRET` (for
the enumeration-resistant salts) come from environment variables. Migrations
run at startup via `sqlx::migrate!`.

To verify rather than assume: Postgres backups are **not** automatic on all
Railway plans — scheduled backups have to be configured, and a restore has
to be tested once before this is relied on. Until then, each device's local
SQLite is the real backup.

`/v1/health` for the platform health check, and the service binds the port
Railway provides in `PORT`.

## 11. Forward compatibility: the standalone extension

The extension becoming independent of the desktop app is planned next
(`docs/roadmap.md` §5). Nothing here may block it, so:

* `havenkeys-sync-client` puts its HTTP calls behind a transport trait, and
  the crate must compile for `wasm32-unknown-unknown` with a `fetch`-based
  transport. This is also why `havenkeys-core` stays network-free.
* The session model — a token obtained at unlock and held in memory only —
  already fits a service worker; no disk credential has to be reworked.
* The server's CORS policy is denied by default and will allow exactly the
  extension's origin, as an explicit entry.
* Key scheme 3 derivation must be benchmarked for Argon2id in the browser
  before the extension ships; the parameters live in the header, so a
  browser that cannot afford the desktop parameters is a vault-level
  decision, not a protocol change.
* Per-device revocation already exists, which is what makes an extension
  device something the user can cut off.

The security trade-off of the standalone mode itself (the extension holding
the vault key) stays out of scope here and must be decided on its own, with
the threat model updated, as the roadmap says.

## 12. Testing

### Server

* Account A cannot read, write or delete anything in account B's vault —
  the central isolation test, run against every authenticated route.
* Wrong `auth_key` → 401; revoked device → 401; expired token → 401.
* `auth/params` gives identical shape and comparable timing for unknown
  emails.
* Oversized body → 413; oversized blob → 400; unknown field → 400;
  non-UUID ids → 400.
* `baseCursor` mismatch → 409, and a pull-merge-retry converges.
* `PUT /v1/vault/header` with a stale revision → 409; with a lower
  `keyScheme` → 400.
* Rate limiting blocks after the configured failures and unblocks on time.
* Invite: single use, expiry respected, wrong invite → 400.
* `no_logging`: no secret-bearing field appears in any log line.

Integration tests run against a real Postgres (a disposable database per
run), because the isolation guarantees are SQL-level and a mock would not
test them.

### Client, with a hostile server

A test server that misbehaves on purpose:

* forged header → attestation fails, header refused;
* an authentic but older header replayed after a password change → refused
  by the recorded `max_header_rev`, and the old password does not unlock;
* header with Argon2id below the floors → refused before derivation;
* tampered item blob → AEAD failure, item skipped and counted;
* blob moved between item ids or accounts → AEAD failure;
* replayed older item → loses the merge;
* malformed, truncated and oversized JSON responses → rejected without
  panic, and added to the existing fuzz suite (`tests/fuzz.rs`);
* key scheme downgrade in the header → refused.

### Core

Existing crypto, vault, merge and origin tests keep passing. New tests for
key scheme 3 derivation, for the scheme 2 → 3 migration (items untouched,
same Secret Key, higher header revision), and for the dirty/cursor
bookkeeping.

## 13. Order of work

1. **Key scheme 3 in the core**, with migration and tests. No network yet.
2. **`havenkeys-server`**: schema, auth, sync routes, admin CLI, tests
   against Postgres. Deploy to Railway and confirm the health check, TLS and
   a restored backup.
3. **`havenkeys-sync-client`** plus the local store's cursor and dirty
   bookkeeping, including the hostile-server suite.
4. **Desktop UI**: activation, sign-in, Emergency Kit, Settings → Account,
   device list. Remove `sync::folder` and the folder UI.
5. **Docs**: `sync.md` → `server-sync.md`, and updates to `threat-model.md`,
   `security-model.md`, `crypto.md`, `architecture.md`, `roadmap.md` and
   `README.md` — including that the README can no longer say "runs no
   server". Then a security review pass over the whole change.

## 14. Explicitly out of scope

Public signup, email verification, password reset, account recovery,
server-side search, sharing, passkeys, mobile, and any change to the
browser extension's current trust boundary.
