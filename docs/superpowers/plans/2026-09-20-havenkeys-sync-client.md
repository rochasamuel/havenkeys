# havenkeys-sync-client — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The client half of the wire: a crate that turns `havenkeys-core`'s
staged writes, header and cursor into HTTP against `havenkeys-server`, and
that treats every answer it gets as hostile until it has been checked.

**Architecture:** `SyncClient<T: Transport>` owns no state but the base URL
and the transport. HTTP lives behind a `Transport` trait so a `fetch`-based
implementation can replace `reqwest` later without touching the protocol code
(design §11), and so the hostile-server suite can hand the client crafted
bytes with no socket involved. The session token lives in a `Session` value
that zeroizes on drop; nothing is written to disk by this crate.

**Tech Stack:** Rust 2021 (MSRV 1.88), `reqwest` with `rustls` (no OpenSSL),
`serde`, `serde_json`, `uuid`, `zeroize`, `data-encoding`, `url`. Depends on
`havenkeys-core` for the shared types (`StagedWrite`, `RemoteChange`,
`KdfParams`, `AuthKey`) so the wire has one source of truth; `havenkeys-core`
gains no dependency in return.

**Spec:** `docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`
§8.1, §8.4, §8.5, §11, §12.

## Global Constraints

- **Never log secrets.** No token, auth key, invite, blob, header or email in
  any log line, `Debug` impl or error. Errors carry `&'static str` messages.
- **The server is untrusted.** Every response is bounded before it is
  allocated, every field is checked, and nothing is trusted to be the size or
  shape it claims. A server can fail to update the replica; it must not be
  able to make the client allocate without bound or panic.
- **The token goes to one origin only.** Redirects are refused: following one
  would hand the bearer token to a host the user never chose.
- **HTTPS**, except `localhost`/`127.0.0.1` for development.
- **No disk access, no SQLite, no key material** in this crate. It never sees
  the master password, the Secret Key, the KEK or the vault key — only the
  auth key, which is what the server is meant to receive.
- **MSRV 1.88**, `cargo fmt` clean, `cargo clippy --all-targets -D warnings`.
- **Commit after every task**, ending with
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.

## File structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-sync-client/src/lib.rs` | Crate docs, re-exports. |
| `crates/havenkeys-sync-client/src/error.rs` | `SyncError`, including `Conflict(Vec<Conflict>)`. |
| `crates/havenkeys-sync-client/src/transport.rs` | `Transport` trait, `HttpRequest`/`HttpResponse`, the `reqwest` implementation and its limits. |
| `crates/havenkeys-sync-client/src/wire.rs` | The DTOs, exactly the server's JSON, and the conversions to and from core types. |
| `crates/havenkeys-sync-client/src/client.rs` | `SyncClient`: one method per route. |
| `crates/havenkeys-sync-client/src/session.rs` | `Session` (token, expiry, ids), zeroized on drop. |
| `crates/havenkeys-sync-client/tests/hostile.rs` | A stub transport that misbehaves on purpose. |
| `crates/havenkeys-sync-client/tests/round_trip.rs` | The real server, a real Postgres and real core crypto, end to end. |

---

## Task 1: Transport, errors and session

- [ ] **Step 1:** `Transport` trait:

```rust
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub token: Option<Zeroizing<String>>,
    pub body: Option<Vec<u8>>,
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait Transport {
    fn send(&self, req: HttpRequest) -> impl Future<Output = Result<HttpResponse, SyncError>> + Send;
}
```

- [ ] **Step 2:** `ReqwestTransport::new(base_url)`: rejects a non-HTTPS URL
      that is not localhost, sets `redirect::Policy::none()`, a 30-second
      timeout, and reads at most `MAX_RESPONSE_BYTES` (17 MiB) by streaming
      chunks and stopping once the cap is passed — `Content-Length` is the
      server's claim, not a fact.
- [ ] **Step 3:** `Session { token, expires_at, account_id, vault_id }` with a
      redacting `Debug` and `ZeroizeOnDrop`.
- [ ] **Step 4:** unit tests: a non-HTTPS base URL is refused; `Debug` on a
      session prints no token.
- [ ] **Step 5:** commit.

## Task 2: The wire types

- [ ] **Step 1:** DTOs mirroring the server byte for byte (camelCase,
      `deny_unknown_fields` on responses too — a field the client does not know
      means the client is out of date, and guessing is worse than failing).
- [ ] **Step 2:** conversions: `KdfParams` ↔ `KdfDto`, `StagedWrite` →
      `ChangeDto`, `ChangeDto` → `RemoteChange`, each with the size checks.
- [ ] **Step 3:** unit tests for the conversions, including a change that is
      neither a write nor a deletion and a blob over 8 MiB.
- [ ] **Step 4:** commit.

## Task 3: The client

- [ ] **Step 1:** one method per route: `health`, `activate`, `auth_params`,
      `login`, `logout`, `header`, `put_header`, `pull`, `write`, `devices`,
      `revoke_device`.
- [ ] **Step 2:** status mapping: 401 → `Unauthorized`, 409 on a write →
      `Conflict(items)`, 429 → `RateLimited`, 4xx → `Refused`, 5xx and
      transport failures → `Unavailable` (which is what the desktop turns into
      its offline state).
- [ ] **Step 3:** commit.

## Task 4: The hostile-server suite

A stub transport, one test per misbehaviour:

- [ ] a response that is not JSON, truncated JSON, or a JSON scalar;
- [ ] a pull carrying more than `MAX_CHANGES_PER_BATCH` changes;
- [ ] a blob over 8 MiB, base64 that does not decode, an `itemId` that is not
      a UUID, a negative revision;
- [ ] a change that is `deleted: true` and carries blobs;
- [ ] a 200 with an empty body where an object is expected;
- [ ] a header response whose `keyScheme` is 2 (a downgrade);
- [ ] a 409 whose `conflicts` array is malformed;
- [ ] every case asserts a `SyncError`, never a panic and never an allocation
      proportional to what the server claimed.
- [ ] commit.

## Task 5: The round trip

Against the real server and a real Postgres, with real core crypto:

- [ ] `prepare_new_account_vault` → `activate` → `login` → `stage_create` →
      `write` → `commit_write` → `pull` on a second `VaultService` →
      `apply_remote_changes` → the item is readable, and its plaintext never
      left the two cores;
- [ ] a stale `base_revision` surfaces as `SyncError::Conflict` naming the
      item;
- [ ] `encode_account_header` → `put_header` → `header` → `adopt_account_header`
      on the second device;
- [ ] a pull that deletes everything is applied and reported;
- [ ] commit.
