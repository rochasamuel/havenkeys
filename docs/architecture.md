# Architecture

## Components

```text
havenkeys/
├── crates/
│   ├── havenkeys-protocol/    bridge wire types, framing, socket endpoint (no core dependency)
│   ├── havenkeys-bridge/      desktop side of the browser bridge (authorization, rate limits, server)
│   ├── havenkeys-native-host/ binary launched by the browser; relays stdio ↔ socket
│   └── havenkeys-core/        Rust security core (no UI, no Tauri dependency)
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
│           └── error.rs       secret-free error type
├── apps/
│   ├── desktop/
│   │   ├── src/               React + TypeScript UI (no crypto)
│   │   └── src-tauri/         Thin Tauri shell: commands, clipboard, auto-lock ticker
│   └── extension/             MV3 extension: background worker, popup, messaging
├── packages/
│   └── protocol/              TypeScript mirror of the wire protocol + validators
├── scripts/                   native host registration
└── docs/
```

The core crate is deliberately independent of Tauri so that:

* it can be audited and tested in isolation (`cargo test -p havenkeys-core`),
* the bridge can link the same code without Tauri,
* UI changes cannot accidentally change security behaviour.

## Data flow: unlock

```text
UI: unlock_vault(password)
  → Tauri command (spawn_blocking)
    → VaultService::begin_unlock()     state: LOCKED → UNLOCKING, returns header snapshot
    → UnlockTicket::derive(password)   Argon2id + HKDF (outside the vault mutex)
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
created with mode `0600` on Unix. Schema v1:

```sql
CREATE TABLE vault_header (
  id                INTEGER PRIMARY KEY CHECK (id = 1),
  format_version    INTEGER NOT NULL,
  vault_id          TEXT    NOT NULL,
  kdf               TEXT    NOT NULL,   -- JSON: algorithm, m, t, p, salt (base64)
  wrapped_vault_key BLOB    NOT NULL,   -- EncryptedBlob v1
  created_at        INTEGER NOT NULL
);
CREATE TABLE items (
  id       TEXT PRIMARY KEY,            -- UUIDv4
  overview BLOB NOT NULL,               -- EncryptedBlob: title, username, urls, flags, timestamps
  details  BLOB NOT NULL                -- EncryptedBlob: password, totp, notes / note content
);
CREATE TABLE settings (
  id   INTEGER PRIMARY KEY CHECK (id = 1),
  blob BLOB NOT NULL                    -- EncryptedBlob: settings JSON
);
```

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
vault when it returns a reason (idle timeout or suspend detected). Every
command and a throttled UI activity ping reset the idle timer.

## Browser integration

```text
popup ─► background worker ─stdio─► havenkeys-native-host ─socket─► havenkeys-bridge ─► VaultService
```

The browser launches `havenkeys-native-host`, which cannot open the vault.
It validates each length-prefixed JSON frame against the typed protocol and
relays it to the running desktop app over a user-private local socket. The
bridge, running inside the desktop process, shares the `VaultService` mutex
with the Tauri commands. It re-validates every request, applies rate limits
and the integration switch, and calls the core's origin-bound functions
(`find_matches`, `fill_for_page`, `totp_for_page`). Lock events are pushed
back to connected hosts over per-connection bounded queues, so a stalled
peer never delays locking. Details: `native-messaging.md`.

Lock ordering in the desktop process: `vault` → `lock_manager`; the bridge
takes its own small mutexes (rate limiter, connection list) either before
the vault or with nothing else held, and calls the lock hook with nothing
held.
