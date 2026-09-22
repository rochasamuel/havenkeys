# Architecture

## Components

```text
havenkeys/
├── crates/
│   ├── havenkeys-protocol/    bridge wire types, framing, socket endpoint (no core dependency)
│   ├── havenkeys-bridge/      desktop side of the browser bridge (authorization, rate limits, server)
│   ├── havenkeys-native-host/ binary launched by the browser; relays stdio ↔ socket
│   ├── havenkeys-server/      the account server: blind relay, Postgres, admin CLI
│   ├── havenkeys-sync-client/ HTTP against that server; treats every answer as hostile
│   └── havenkeys-core/        Rust security core (no UI, no Tauri, no network)
│       └── src/
│           ├── crypto/        kdf.rs, keys.rs, blob.rs — composition of audited primitives
│           ├── secret.rs      SecretString: zeroize-on-drop, redacted Debug
│           ├── store.rs       SQLite persistence of header + encrypted blobs
│           ├── model.rs       item types and input validation
│           ├── vault.rs       VaultService: lock state machine, sessions, item ops
│           ├── lock.rs        LockManager: auto-lock policy (pure, clock-injected)
│           ├── generator.rs   CSPRNG password generator
│           ├── totp.rs        RFC 6238 + otpauth:// parsing
│           ├── origin.rs      URL parsing and domain matching (PSL-based)
│           ├── import/        1Password .1pux importer (hostile-input parsing)
│           ├── account.rs     account identity: email normalization for key derivation
│           ├── sync.rs        account header attestation + applying a server pull (no network)
│           └── error.rs       secret-free error type
├── apps/
│   ├── desktop/
│   │   ├── src/               React + TypeScript UI (no crypto)
│   │   └── src-tauri/         Thin Tauri shell: commands, clipboard, auto-lock ticker
│   └── extension/             MV3 extension: background worker, popup, content script,
│                              autofill engine, in-page menu/save frames, options, messaging
├── packages/
│   └── protocol/              TypeScript mirror of the wire protocol + validators
├── scripts/                   native host registration
└── docs/
```

The core crate is deliberately independent of Tauri so that:

* it can be audited and tested in isolation (`cargo test -p havenkeys-core`),
* the bridge can link the same code without Tauri,
* UI changes cannot accidentally change security behaviour.

## The server and the local replica

HavenKeys is server-authoritative (`docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`):
a `havenkeys-server` account is the single writer, and each device's SQLite
database is a **read-only replica** of it, not an independent vault. See
`docs/server-sync.md` for the full model and `docs/deployment.md` for running
the server.

```text
┌────────────────────────── Desktop app ──────────────────────────┐
│                                                                   │
│  React / TypeScript UI                                           │
│         │                                                        │
│         ▼                                                        │
│   Tauri commands ──── AppState::require_online() ──── Connectivity│
│         │             (reads always allowed;                Online/│
│         │              writes refused when offline)         Offline│
│         ▼                                                        │
│   Rust security core (havenkeys-core)                            │
│         │                                                        │
│    ┌────┴─────┐                                                  │
│    │          │                                                  │
│  Crypto     Vault ──stage_*/commit_write──┐                      │
│    │          │                            │                     │
│    └────┬─────┘                            │                     │
│         ▼                                  │                     │
│  Local SQLite replica                      │                     │
│  (schema 4: account, items, settings)      │                     │
│         ▲                                  │                     │
│         │ apply_remote_changes             │                     │
└─────────┼──────────────────────────────────┼─────────────────────┘
          │                                  │
          │        havenkeys-sync-client     │
          │        HTTPS, session token      ▼
          └──────────────────────── havenkeys-server
                                     (Postgres, the single
                                      authoritative copy;
                                      ciphertext only)
```

Native Messaging to the browser extension is unaffected by any of this for
reads — it talks to the same `VaultService` the Tauri commands do. Writes
from the browser take the same route as writes from the UI, by a different
door: `VaultService::stage_save_login` seals the login under the vault lock
with the usual origin binding, `dispatch` hands the staged write back out,
and the desktop's writer hook sends it and records the revision. The bridge
crate itself knows nothing about the server, so a bridge built without that
hook answers `offline` — which is exactly what a device with nowhere to send
a write is.

## Data flow: unlock

```text
UI: unlock_vault(password)
  → Tauri command (spawn_blocking)
    → VaultService::begin_unlock()     state: LOCKED → UNLOCKING, returns header snapshot
    → UnlockTicket::derive_for_account(password, secret_key, account)
                                       Argon2id + HKDF (outside the vault mutex)
    → VaultService::finish_unlock()    unwrap vault key, decrypt overviews
                                       state: UNLOCKED (or back to LOCKED on failure)
  ← { state: "unlocked" }
```

Argon2id runs without holding the vault mutex, so `vault_status` keeps
answering (and reports `unlocking`) while the key is being derived.

## Data flow: reveal a password

```text
UI (user clicks eye) → reveal_secret(id, "password")
  → VaultService::reveal(id, Password)
      requires Session; decrypts that item's details blob only;
      returns one SecretString; the decrypted buffer is zeroized on drop
  ← string shown in the UI until the user hides it, navigates away, or the vault locks
```

## Storage

SQLite (bundled via `rusqlite`), one file per vault at the platform data
directory (`$XDG_DATA_HOME/com.havenkeys.desktop/vault.sqlite3` on Linux),
created with mode `0600` on Unix. This is a read-only replica of the
server's account, not an independently authoritative vault
(`docs/server-sync.md`). Schema v4 (`Store::init` refuses to open anything
older, with a message that the vault predates accounts):

```sql
CREATE TABLE vault_header (
  id                INTEGER PRIMARY KEY CHECK (id = 1),
  format_version    INTEGER NOT NULL,
  vault_id          TEXT    NOT NULL,
  kdf               TEXT    NOT NULL,   -- JSON: algorithm, m, t, p, salt (base64)
  wrapped_vault_key BLOB    NOT NULL,   -- EncryptedBlob v1
  created_at        INTEGER NOT NULL,
  key_scheme        INTEGER NOT NULL,   -- always 3 (account-bound)
  header_revision   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE items (
  id       TEXT PRIMARY KEY,            -- UUIDv4
  overview BLOB NOT NULL,               -- EncryptedBlob: title, username, urls, flags, timestamps
  details  BLOB NOT NULL,               -- EncryptedBlob: password, totp, notes / note content
  revision INTEGER NOT NULL             -- the server's revision for this row (optimistic concurrency)
);
CREATE TABLE account (
  id             INTEGER PRIMARY KEY CHECK (id = 1),
  account_id     TEXT    NOT NULL,
  email          TEXT    NOT NULL,      -- as typed; normalized on use, not on store
  server_url     TEXT    NOT NULL,
  server_cursor  INTEGER NOT NULL DEFAULT 0,   -- highest server revision pulled
  max_header_rev INTEGER NOT NULL DEFAULT 0,   -- rollback floor; never goes down
  last_synced_at INTEGER
);
CREATE TABLE settings (
  id   INTEGER PRIMARY KEY CHECK (id = 1),
  blob BLOB NOT NULL                    -- EncryptedBlob: settings JSON (device-local, unsynced)
);
```

There is no `tombstones` table and no `dirty` column: the server is the only
writer, so there is nothing for the client to merge or to mark as locally
ahead of what it last pushed.

`PRAGMA secure_delete = ON` is set so deleted pages are overwritten.

### Overview / details split

Each item is two ciphertexts:

* **overview** — what the list, search, and (later) `find_matches` need. All
  overviews are decrypted once at unlock and kept in memory while unlocked.
* **details** — secrets. Decrypted per request and dropped immediately.

This satisfies "avoid repeatedly decrypting the whole vault" and "decrypt notes
only when necessary" without a plaintext index.

## Search

In-memory, over decrypted overviews (title, username, URL hosts),
case-insensitive substring match. Secure-note bodies and login notes are not
searched (they live in details blobs). There is no persistent index.

## Auto-lock

`LockManager` is pure logic fed with monotonic and wall-clock timestamps. The
Tauri shell ticks it every 5 seconds from a background thread and locks the
vault when it returns a reason (idle timeout or suspend detected). A throttled
UI activity ping (`record_activity`, fed by real input while the window has
focus) and the commands the user triggers reset the idle timer. Machine-driven
calls deliberately do not: TOTP refresh, the periodic pull, and the list
refreshes that follow a sync or a browser-extension save all leave the timer
alone, so neither a timer nor a hostile server can hold the vault open.

## Browser integration

```text
popup ──────────────┐
menu / save frames ─┼─► background worker ─stdio─► havenkeys-native-host ─socket─► havenkeys-bridge ─► VaultService
content scripts ────┘         │
       ▲                      │ fills (one frame, matched origin only)
       └──────────────────────┘
```

The browser launches `havenkeys-native-host`, which cannot open the vault.
It validates each length-prefixed JSON frame against the typed protocol and
relays it to the running desktop app over a user-private local socket. The
bridge, running inside the desktop process, shares the `VaultService` mutex
with the Tauri commands. It re-validates every request, applies rate limits
and the integration switch, and calls the core's origin-bound functions
(`find_matches`, `fill_for_page`, `totp_for_page`, `check_login`,
`save_login`). Lock events are pushed
back to connected hosts over per-connection bounded queues, so a stalled
peer never delays locking. Details: `native-messaging.md`.

In the browser, content scripts (opt-in, or injected into one tab on a popup
fill) classify login fields when the user interacts with them. The
background worker runs the suggestion and save-prompt sessions, and the menu
and prompt are extension pages framed into the site. The autofill engine
(`apps/extension/src/autofill/`) is pure DOM logic with no extension APIs,
so it is tested in jsdom. Details: `autofill.md`.

Lock ordering in the desktop process: `vault` → `lock_manager`; the bridge
takes its own small mutexes (rate limiter, connection list) either before
the vault or with nothing else held, and calls the lock hook with nothing
held.
