# Vault and Account Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A master-password change that updates the server atomically and that other devices can follow; unreadable pulled items that are retried and shown; the Secret Key in the OS keychain; and a "Remove this device" path.

**Architecture:** Four parts that land in order (A → C → D → E), each ending green. A adds one server route that replaces the header, the login verifier and the KDF in one transaction, makes the desktop commit locally only after the server accepts, and adds an online fallback to unlock. C records unreadable items in the local SQLite, retries them through a new fetch-by-ID route, and surfaces the count. D moves the Secret Key behind a `KeyStore` trait backed by the OS keychain, with `device.json` as a fallback. E closes the store, renames the vault file aside, and forgets the Secret Key.

**Tech Stack:** Rust (axum + tokio-postgres server, rusqlite core, Tauri 2 desktop), `keyring-core` + platform store crates, React 19 + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-09-23-vault-account-fixes-design.md`

## Global Constraints

- Never log or put in an error: passwords, auth keys, the Secret Key, tokens, blobs, headers (CLAUDE.md §39–40). Server errors are fixed strings (`ApiError`).
- Request bodies use `#[serde(deny_unknown_fields)]`; responses do not.
- The vault may be reset: no SQLite migrations. `SCHEMA_VERSION` goes 4 → 5 and any other version is refused, as today.
- The server has one migration file; server schema is unchanged by this plan (no new tables or columns).
- Every new Tauri command is added in four places: `src-tauri/build.rs` `COMMANDS`, `lib.rs` `generate_handler!`, `capabilities/main.json` (`allow-<kebab-name>`), and `src/lib/api.ts`. `src/lib/commands.test.ts` enforces this.
- No vault-key rotation, no item-replay protection, no export (spec §8).
- Commit after each task. No `Co-Authored-By` trailers (user preference).
- Server/sync-client tests need Postgres: run them with `scripts/test-server.sh` (it forwards extra args to `cargo test -p havenkeys-server -p havenkeys-sync-client`).

## Review Focus

1. **The server commits a password change but the response is lost.** The device must heal on the next sync (adopt the newer header), not stay stuck. Test: Task 5 round trip, "lost response".
2. **The user types the old password on a device that has not adopted the change.** The local unlock succeeds, login is rejected, and the app says why instead of the generic offline banner. Test: Task 4, `signed-out` event emitted on 401 from `connect`.
3. **The server serves a forged header during the unlock fallback.** It must be refused (attestation under the unchanged data key) and the unlock must report "wrong password", not unlock. Test: Task 3, `adopt_and_unlock` with a forged header.
4. **An item stays unreadable forever (a truly corrupt blob).** Each sync retries it cheaply (one fetch of ≤500 IDs), the banner stays, and nothing loops. Test: Task 6, repeated `apply_refetched` with a bad blob leaves the count at 1.
5. **No Secret Service on Linux (WSL, minimal WM).** The keychain call must fail within the timeout and the key must land in `device.json` with `secretKeyStorage: "file"`. Test: Task 10, a `KeyStore` fake that errors, and one that sleeps past the timeout.

---

## Part A — Master-password change

### Task 1: Server route `POST /v1/account/credentials`, remove `PUT /v1/vault/header`

**Files:**
- Create: `crates/havenkeys-server/src/routes/account.rs`
- Modify: `crates/havenkeys-server/src/routes/mod.rs` (register route, drop `put_header`, drop `Method::PUT` from CORS)
- Modify: `crates/havenkeys-server/src/routes/vault.rs` (delete `HeaderUpdate`, `put_header`; update module doc)
- Modify: `crates/havenkeys-server/src/routes/accounts.rs` (make `KdfDto` and `KdfDto::check` `pub(crate)`)
- Test: `crates/havenkeys-server/tests/credentials.rs` (new)
- Modify tests that used `PUT /v1/vault/header`: `tests/sync.rs` (delete `a_header_write_must_be_exactly_one_revision_ahead`, `a_lower_key_scheme_is_refused`), `tests/isolation.rs` (lines ~46 and ~116: switch to the new route), `tests/no_logging.rs` (~78: switch), `tests/limits.rs` (~49: switch to the new route with an oversized header)
- Modify: `crates/havenkeys-server/tests/support/mod.rs` (add `credentials_body` helper)

**Interfaces:**
- Produces: `POST /v1/account/credentials`, body `{currentAuthKey, kdf, newAuthKey, header, baseHeaderRevision}` → `200 {"headerRevision": n}`; `401` wrong current key; `409` stale base; `400` bad KDF/header/key; `429` rate-limited.

- [ ] **Step 1: Add the test helper** in `tests/support/mod.rs`:

```rust
/// A credential change as the desktop sends it.
pub fn credentials_body(current: &[u8; 32], new: &[u8; 32], base: i64, header: &[u8]) -> Value {
    json!({
        "currentAuthKey": data_encoding::BASE64.encode(current),
        "kdf": {
            "algorithm": "argon2id",
            "memoryKib": 131072,
            "iterations": 4,
            "parallelism": 4,
            "salt": data_encoding::BASE64.encode(&[4u8; 16]),
        },
        "newAuthKey": data_encoding::BASE64.encode(new),
        "header": data_encoding::BASE64.encode(header),
        "baseHeaderRevision": base,
    })
}
```

- [ ] **Step 2: Write the failing tests** in `tests/credentials.rs`:

```rust
mod support;

use serde_json::Value;
use support::*;

const NEW_KEY: [u8; 32] = [42u8; 32];

async fn change(server: &TestServer, sess: &Sess, current: &[u8; 32], base: i64) -> (u16, Value) {
    let res = server
        .post_as("/v1/account/credentials", sess)
        .json(&credentials_body(current, &NEW_KEY, base, b"header-bytes-v2"))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    let body = res.json().await.unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
async fn a_change_replaces_verifier_kdf_and_header_together() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "change@example.com").await;

    let (status, body) = change(&server, &sess, &account.auth_key, 0).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["headerRevision"], 1);

    // The header moved.
    let header: Value = server.get_as("/v1/vault/header", &sess).send().await.unwrap().json().await.unwrap();
    assert_eq!(header["headerRevision"], 1);
    assert_eq!(
        data_encoding::BASE64.decode(header["header"].as_str().unwrap().as_bytes()).unwrap(),
        b"header-bytes-v2".to_vec()
    );
    // The params moved.
    let params: Value = server
        .post("/v1/auth/params")
        .json(&serde_json::json!({ "email": account.email }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(params["kdf"]["salt"], data_encoding::BASE64.encode(&[4u8; 16]));
    server.cleanup().await;
}

#[tokio::test]
async fn the_old_key_stops_working_and_the_new_one_logs_in() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "keys@example.com").await;
    let old_key = account.auth_key;
    assert_eq!(change(&server, &sess, &old_key, 0).await.0, 200);

    let res = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&old_key),
            "deviceId": uuid::Uuid::new_v4(),
            "deviceName": "Old",
        }))
        .send().await.unwrap();
    assert_eq!(res.status(), 401);

    let renewed = Account { auth_key: NEW_KEY, ..account };
    login(&server, &renewed, "Laptop").await;
    server.cleanup().await;
}

#[tokio::test]
async fn other_sessions_are_revoked_and_the_caller_is_kept() {
    let server = TestServer::start().await;
    let (account, first) = signed_in(&server, "revoke-others@example.com").await;
    let second = login(&server, &account, "Laptop").await;

    assert_eq!(change(&server, &first, &account.auth_key, 0).await.0, 200);

    assert_eq!(server.get_as("/v1/sync?since=0", &first).send().await.unwrap().status(), 200);
    assert_eq!(server.get_as("/v1/sync?since=0", &second).send().await.unwrap().status(), 401);
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_current_key_is_refused_and_counted() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "wrong@example.com").await;

    let (status, _) = change(&server, &sess, &[1u8; 32], 0).await;
    assert_eq!(status, 401);
    let failures: i32 = server
        .db().await
        .query_one(
            "SELECT failures FROM login_attempts WHERE key = $1",
            &[&format!("acct:{}", account.account_id)],
        )
        .await.unwrap().get(0);
    assert_eq!(failures, 1);
    // Nothing moved.
    let header: Value = server.get_as("/v1/vault/header", &sess).send().await.unwrap().json().await.unwrap();
    assert_eq!(header["headerRevision"], 0);
    server.cleanup().await;
}

#[tokio::test]
async fn a_stale_base_revision_is_a_conflict() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "stale@example.com").await;
    let (status, _) = change(&server, &sess, &account.auth_key, 5).await;
    assert_eq!(status, 409);
    server.cleanup().await;
}

#[tokio::test]
async fn a_malformed_change_is_refused() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "malformed@example.com").await;
    let mut body = credentials_body(&account.auth_key, &NEW_KEY, 0, b"h");
    body["kdf"]["memoryKib"] = 1.into();
    let res = server.post_as("/v1/account/credentials", &sess).json(&body).send().await.unwrap();
    assert_eq!(res.status(), 400);

    let mut body = credentials_body(&account.auth_key, &NEW_KEY, 0, b"h");
    body["extra"] = true.into();
    let res = server.post_as("/v1/account/credentials", &sess).json(&body).send().await.unwrap();
    assert_eq!(res.status(), 400);

    let body = credentials_body(&account.auth_key, &NEW_KEY, 0, b"");
    let res = server.post_as("/v1/account/credentials", &sess).json(&body).send().await.unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn the_header_route_no_longer_accepts_writes() {
    let server = TestServer::start().await;
    let (_account, sess) = signed_in(&server, "noput@example.com").await;
    let res = server.put_as("/v1/vault/header", &sess).json(&serde_json::json!({})).send().await.unwrap();
    assert_eq!(res.status(), 405);
    server.cleanup().await;
}
```

Check that `Account` derives nothing that blocks struct-update syntax; if its fields are all `pub` (they are), `Account { auth_key: NEW_KEY, ..account }` compiles. Check that `json::Json` maps a `deny_unknown_fields` failure to 400 (see `src/json.rs`); if it maps to 422, assert 422.

- [ ] **Step 3: Run to verify failure**

Run: `scripts/test-server.sh --test credentials`
Expected: FAIL (404/405 on the new route).

- [ ] **Step 4: Implement `routes/account.rs`**

```rust
//! Changing the account's credentials: the master password, from the
//! server's point of view.
//!
//! A new master password means a new KDF salt, a new auth key and a new key
//! wrap. All three have to move together: a header without the verifier
//! locks every device out of the server, and a verifier without the header
//! leaves other devices unable to open the vault. So this is one route and
//! one transaction.
//!
//! The session alone is not enough to ask. A stolen token could otherwise
//! replace the password and lock the owner out, so the caller also proves it
//! knows the current auth key, which is checked and rate limited exactly
//! like a login. Every other session on the account is deleted: a password
//! is often changed because something leaked.

use crate::auth::{self, rate_limit, Session};
use crate::b64::Blob;
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::MAX_HEADER_BYTES;
use crate::routes::accounts::KdfDto;
use crate::routes::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialChange {
    current_auth_key: String,
    kdf: KdfDto,
    new_auth_key: String,
    header: Blob,
    base_header_revision: i64,
}

pub async fn change_credentials(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<CredentialChange>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if req.header.is_empty() || req.header.len() > MAX_HEADER_BYTES {
        return Err(ApiError::InvalidRequest("header is not valid"));
    }
    if req.base_header_revision < 0 {
        return Err(ApiError::InvalidRequest("baseHeaderRevision is not valid"));
    }
    let salt = req.kdf.check()?;
    let current = auth::decode_auth_key(&req.current_auth_key)?;
    let new = auth::decode_auth_key(&req.new_auth_key)?;

    let mut db = state.pool.get().await?;
    let account_key = format!("acct:{}", session.account_id);
    let ip_key = format!("ip:{}", crate::routes::auth::client_ip(&state, &headers, peer));
    rate_limit::check(&db, &account_key).await?;
    rate_limit::check(&db, &ip_key).await?;

    // Verified before the transaction, like login: Argon2id takes tens of
    // milliseconds and no row lock should be held through it. The verifier is
    // compared again under the lock, so a concurrent change still loses.
    let stored: String = db
        .query_opt(
            "SELECT auth_verifier FROM accounts WHERE id = $1 AND status = 'active'",
            &[&session.account_id],
        )
        .await?
        .and_then(|r| r.get::<_, Option<String>>(0))
        .ok_or(ApiError::Unauthorized)?;
    if !auth::verify_auth_key(stored.clone(), current).await {
        rate_limit::record_failure(&db, &account_key).await?;
        rate_limit::record_failure(&db, &ip_key).await?;
        tracing::info!(account_id = %session.account_id, outcome = "rejected", "credential change");
        return Err(ApiError::Unauthorized);
    }
    let verifier = auth::hash_auth_key(new).await?;

    let tx = db.transaction().await?;
    let locked: String = tx
        .query_one(
            "SELECT auth_verifier FROM accounts WHERE id = $1 FOR UPDATE",
            &[&session.account_id],
        )
        .await?
        .get(0);
    if locked != stored {
        return Err(ApiError::Conflict);
    }
    let current_revision: i64 = tx
        .query_one(
            "SELECT header_revision FROM vaults WHERE id = $1 FOR UPDATE",
            &[&session.vault_id],
        )
        .await?
        .get(0);
    if req.base_header_revision != current_revision {
        return Err(ApiError::Conflict);
    }
    let next = current_revision + 1;
    tx.execute(
        "UPDATE accounts
            SET kdf_algorithm = 'argon2id', kdf_memory_kib = $2, kdf_iterations = $3,
                kdf_parallelism = $4, kdf_salt = $5, auth_verifier = $6
          WHERE id = $1",
        &[
            &session.account_id,
            &req.kdf.memory_kib,
            &req.kdf.iterations,
            &req.kdf.parallelism,
            &salt,
            &verifier,
        ],
    )
    .await?;
    tx.execute(
        "UPDATE vaults SET header = $2, header_revision = $3 WHERE id = $1",
        &[&session.vault_id, &req.header.0, &next],
    )
    .await?;
    tx.execute(
        "DELETE FROM sessions WHERE account_id = $1 AND token_hash <> $2",
        &[&session.account_id, &session.token_hash],
    )
    .await?;
    tx.commit().await?;

    rate_limit::clear(&db, &account_key).await?;
    rate_limit::clear(&db, &ip_key).await?;
    tracing::info!(account_id = %session.account_id, header_revision = next, "credentials changed");
    Ok(axum::Json(serde_json::json!({ "headerRevision": next })))
}
```

In `accounts.rs`, change `pub struct KdfDto` fields to `pub(crate)` and `fn check` to `pub(crate) fn check`. In `mod.rs` add `pub mod account;`, route `.route("/v1/account/credentials", post(account::change_credentials))`, change the header route to `get(vault::get_header)`, and remove `Method::PUT` from `allow_methods`. In `vault.rs`, delete `HeaderUpdate` and `put_header` and the now-unused imports, and rewrite the module doc: "The vault header: the wrapped vault key and its attestation. Served here; changed only through `account::change_credentials`, together with the login verifier." Confirm `Session` exposes `token_hash` (`src/auth/mod.rs:130`); if not `pub`, make it `pub`.

Update the four test files listed above that used `PUT /v1/vault/header` so they call `/v1/account/credentials` with `credentials_body(&account.auth_key, &NEW, 0, …)` instead (for `isolation.rs`, assert account B's header is unchanged after A's change; in the route list at ~116 replace `("PUT", "/v1/vault/header")` with `("POST", "/v1/account/credentials")`; for `limits.rs`, send a header of `MAX_HEADER_BYTES + 1` bytes and expect 400; for `no_logging.rs`, assert the auth keys' Base64 never appears in captured logs).

- [ ] **Step 5: Run the server suite**

Run: `scripts/test-server.sh -p havenkeys-server`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): change credentials atomically and revoke other sessions"
```

### Task 2: Sync client `change_credentials`, remove `put_header`

**Files:**
- Modify: `crates/havenkeys-sync-client/src/client.rs`
- Modify: `crates/havenkeys-sync-client/src/wire.rs` (replace `HeaderBody` with `CredentialsBody`, add `CredentialsAckDto`)
- Modify: `crates/havenkeys-sync-client/src/lib.rs` (export `CredentialChange`)
- Test: `crates/havenkeys-sync-client/tests/hostile.rs`, `tests/round_trip.rs` (replace `a_header_published_by_one_device_is_adopted_by_another`)

**Interfaces:**
- Produces:
```rust
pub struct CredentialChange<'a> {
    pub current_auth_key: &'a AuthKey,
    pub kdf: &'a KdfParams,
    pub new_auth_key: &'a AuthKey,
    pub header: &'a [u8],
    pub base_header_revision: i64,
}
impl<T: Transport> SyncClient<T> {
    /// Returns the header revision the server assigned.
    pub async fn change_credentials(&self, session: &Session, change: CredentialChange<'_>) -> Result<i64>;
}
```

- [ ] **Step 1: Write failing hostile tests** in `tests/hostile.rs` (use the existing `Stub`, `session()`, and a helper that builds a `CredentialChange` from `AuthKey::from_bytes([1;32])`-style constructors; check `havenkeys_core::crypto::keys::AuthKey` for its constructor, and add `#[cfg(any(test, feature = "test-util"))] pub fn from_bytes` if none is public):

```rust
#[tokio::test]
async fn a_credential_change_ack_must_be_exactly_one_ahead() {
    // base 3 → the server must answer 4; anything else is a server lying.
    for body in [r#"{"headerRevision": 3}"#, r#"{"headerRevision": 9}"#, r#"{"headerRevision": -1}"#, "{}"] {
        let client = Stub::ok(body);
        let err = client.change_credentials(&session(), change(3)).await.unwrap_err();
        assert!(matches!(err, SyncError::Protocol(_)), "{body}: {err:?}");
    }
    let client = Stub::ok(r#"{"headerRevision": 4}"#);
    assert_eq!(client.change_credentials(&session(), change(3)).await.unwrap(), 4);
}

#[tokio::test]
async fn a_credential_change_conflict_and_rejection_are_mapped() {
    let err = Stub::status(409, "{}").change_credentials(&session(), change(0)).await.unwrap_err();
    assert!(matches!(err, SyncError::Conflict(_)));
    let err = Stub::status(401, "{}").change_credentials(&session(), change(0)).await.unwrap_err();
    assert_eq!(err, SyncError::Unauthorized);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-sync-client --test hostile`
Expected: FAIL to compile (`change_credentials` missing).

- [ ] **Step 3: Implement**

In `wire.rs`, replace `HeaderBody` with:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialsBody<'a> {
    pub current_auth_key: &'a str,
    pub kdf: KdfDto,
    pub new_auth_key: &'a str,
    pub header: String,
    pub base_header_revision: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsAckDto {
    pub header_revision: i64,
}
```

In `client.rs`, delete `put_header` and add (with `CredentialChange` defined next to `Activation`):

```rust
    /// Change the master password on the server: the header, the login
    /// verifier and the KDF parameters move together, and every other
    /// session on the account ends. Nothing local changes here; the caller
    /// commits the new wrap only once this returns.
    pub async fn change_credentials(
        &self,
        session: &Session,
        change: CredentialChange<'_>,
    ) -> Result<i64> {
        if change.header.is_empty() || change.header.len() > MAX_HEADER_BYTES {
            return Err(SyncError::Refused("header is not valid"));
        }
        if change.base_header_revision < 0 {
            return Err(SyncError::Refused("header revision is not valid"));
        }
        let current = change.current_auth_key.to_base64();
        let new = change.new_auth_key.to_base64();
        let body = wire::CredentialsBody {
            current_auth_key: &current,
            kdf: change.kdf.into(),
            new_auth_key: &new,
            header: BASE64.encode(change.header),
            base_header_revision: change.base_header_revision,
        };
        let response = self
            .send(Method::Post, "/v1/account/credentials", Some(session), Some(json(&body)?))
            .await?;
        let dto: wire::CredentialsAckDto = self.expect_ok(response)?;
        // The server assigns base + 1 and nothing else. Anything different
        // would leave this device committing a revision the server never
        // stored.
        if dto.header_revision != change.base_header_revision + 1 {
            return Err(SyncError::Protocol("header revision"));
        }
        Ok(dto.header_revision)
    }
```

Check what `to_base64` returns; if it is a zeroizing `SecretString`, bind with `.expose()` accordingly so the value is wiped after serialization.

In `tests/round_trip.rs`, replace `a_header_published_by_one_device_is_adopted_by_another` with a stub for now (Task 5 fills it in):

```rust
// Replaced by `a_password_change_reaches_the_second_device` (Task 5).
```

- [ ] **Step 4: Run**

Run: `scripts/test-server.sh -p havenkeys-sync-client`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-sync-client crates/havenkeys-core
git commit -m "feat(sync-client): change credentials in one request"
```

### Task 3: Core — login keys from a rekey, explicit-revision commit, adopt-and-unlock

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (`Rekeyed`, `RekeyTicket::derive_for_account`, `commit_rekey`, new `encode_rekeyed_header`, new `adopt_and_unlock`, extract `open_session`)
- Modify: `crates/havenkeys-core/src/sync.rs` (a `pub(crate) fn encode_header_record` wrapper if `encode_header` is private)
- Test: `crates/havenkeys-core/tests/account.rs`

**Interfaces:**
- Produces:
```rust
impl Rekeyed {
    pub fn kdf(&self) -> &KdfParams;
    pub fn current_auth_key(&self) -> &AuthKey;
    pub fn new_auth_key(&self) -> &AuthKey;
}
impl RekeyTicket {
    /// The revision the server is asked to move from.
    pub fn base_revision(&self) -> u64;
}
impl VaultService {
    /// The attested header the rekey will produce, at base + 1. Writes nothing.
    pub fn encode_rekeyed_header(&self, ticket: &RekeyTicket, rekeyed: &Rekeyed) -> Result<Vec<u8>>;
    /// Persist the rekey at the revision the server assigned (must be base + 1).
    pub fn commit_rekey(&mut self, ticket: RekeyTicket, rekeyed: Result<Rekeyed>, revision: u64) -> Result<()>;
    /// LOCKED → UNLOCKED with a header newer than the local one, already
    /// verified by `prepare_sign_in`. Refuses another vault, a revision at or
    /// below the floor, or any state but LOCKED.
    pub fn adopt_and_unlock(&mut self, prepared: PreparedVault) -> Result<()>;
}
```

- [ ] **Step 1: Write failing tests** in `tests/account.rs` (reuse its existing helpers for an activated vault; read the top of the file for their names):

```rust
#[test]
fn a_rekey_yields_both_login_keys_and_commits_at_the_given_revision() {
    let (mut vault, sk, account) = activated_unlocked_vault();
    let ticket = vault.begin_rekey().unwrap();
    let before = derive_auth_key(&SecretString::from(PASSWORD), &sk, &ticket_kdf(&vault), &account).unwrap();
    let rekeyed = ticket
        .derive_for_account(&SecretString::from(PASSWORD), &SecretString::from(NEW_PASSWORD), test_params(), &sk, &account)
        .unwrap();
    assert_eq!(rekeyed.current_auth_key().to_base64(), before.to_base64());
    assert_ne!(rekeyed.new_auth_key().to_base64(), before.to_base64());

    let header = vault.encode_rekeyed_header(&ticket, &rekeyed).unwrap();
    assert!(!header.is_empty());
    assert_eq!(vault.header_revision().unwrap(), Some(0)); // nothing written yet

    let err = vault.commit_rekey(ticket, Ok(rekeyed), 5).unwrap_err();
    assert_eq!(err.code(), "corrupted");
}

#[test]
fn a_rekey_commits_at_base_plus_one() {
    let (mut vault, sk, account) = activated_unlocked_vault();
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket.derive_for_account(&PASSWORD.into(), &NEW_PASSWORD.into(), test_params(), &sk, &account);
    vault.commit_rekey(ticket, rekeyed, 1).unwrap();
    assert_eq!(vault.header_revision().unwrap(), Some(1));
    vault.lock();
    vault.unlock_for_account(&NEW_PASSWORD.into(), &sk, &account).unwrap();
}

#[test]
fn another_device_adopts_a_changed_password_at_unlock() {
    // Device 1 changes the password; device 2 still has the old header.
    let (mut one, sk, account) = activated_unlocked_vault();
    let mut two = second_device_from(&one, &sk, &account); // same vault, own store, locked
    let ticket = one.begin_rekey().unwrap();
    let rekeyed = ticket.derive_for_account(&PASSWORD.into(), &NEW_PASSWORD.into(), test_params(), &sk, &account).unwrap();
    let served = one.encode_rekeyed_header(&ticket, &rekeyed).unwrap();
    one.commit_rekey(ticket, Ok(rekeyed), 1).unwrap();

    // The new password does not open device 2's local header...
    assert!(two.unlock_for_account(&NEW_PASSWORD.into(), &sk, &account).is_err());
    // ...but the header the server serves does, and it is adopted.
    let (prepared, _auth) = prepare_sign_in(&served, &NEW_PASSWORD.into(), &sk, &account).unwrap();
    two.adopt_and_unlock(prepared).unwrap();
    assert!(two.is_unlocked());
    assert_eq!(two.header_revision().unwrap(), Some(1));
    two.lock();
    two.unlock_for_account(&NEW_PASSWORD.into(), &sk, &account).unwrap();
}

#[test]
fn adopt_and_unlock_refuses_an_old_or_foreign_header() {
    let (mut one, sk, account) = activated_unlocked_vault();
    let mut two = second_device_from(&one, &sk, &account);
    // Same revision as local (0): refused.
    let current = one.encode_account_header().unwrap();
    let (prepared, _) = prepare_sign_in(&current, &PASSWORD.into(), &sk, &account).unwrap();
    assert!(two.adopt_and_unlock(prepared).is_err());
    assert!(!two.is_unlocked());
    // A different vault: refused.
    let (other, osk, oacc) = activated_unlocked_vault();
    let foreign = other.encode_account_header().unwrap();
    let (prepared, _) = prepare_sign_in(&foreign, &PASSWORD.into(), &osk, &oacc).unwrap();
    assert!(two.adopt_and_unlock(prepared).is_err());
}

#[test]
fn a_forged_header_does_not_pass_sign_in_verification() {
    let (one, sk, account) = activated_unlocked_vault();
    let mut served: serde_json::Value = serde_json::from_slice(&one.encode_account_header().unwrap()).unwrap();
    served["header"]["revision"] = 7.into(); // attestation no longer matches
    let bytes = serde_json::to_vec(&served).unwrap();
    assert!(prepare_sign_in(&bytes, &PASSWORD.into(), &sk, &account).is_err());
}
```

Write `second_device_from` in the test file: it builds a fresh `VaultService` on `Store::open_in_memory()`, calls `prepare_sign_in` with `one.encode_account_header()`, `create_account_vault` with `max_header_rev` = that header's revision, and locks it. Write `ticket_kdf` as reading the `kdf` from `vault.encode_account_header()` JSON, or drop that assertion if awkward and assert only `current != new`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-core --test account`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`Rekeyed` gains `current_auth_key: AuthKey, new_auth_key: AuthKey` and the three accessors. `derive_for_account` keeps the two master keys and derives both auth keys:

```rust
        let current_master = derive_master_key(current, &h.kdf)?;
        let current_kek = derive_kek_v3(&current_master, secret_key, account)?;
        let current_auth_key = derive_auth_key_from_master(&current_master, secret_key, account)?;
        let vault_key = unwrap_vault_key(&current_kek, h.vault_id, &h.wrapped_vault_key)?;
        let new_master = derive_master_key(new, &new_kdf)?;
        let new_kek = derive_kek_v3(&new_master, secret_key, account)?;
        let new_auth_key = derive_auth_key_from_master(&new_master, secret_key, account)?;
```

`RekeyTicket::base_revision()` returns `self.header.revision`.

`encode_rekeyed_header`: build a `HeaderRecord { kdf: rekeyed.kdf.clone(), wrapped_vault_key: rekeyed.wrapped_vault_key.clone(), key_scheme: rekeyed.key_scheme, revision: ticket.header.revision + 1, ..ticket.header.clone() }` and call `crate::sync::encode_header(&self.session()?.data_key, &record)` (make `encode_header` `pub(crate)`).

`commit_rekey(ticket, rekeyed, revision)`: keep every existing check, then `if revision != current.revision.saturating_add(1) { return Err(Error::Corrupted); }` and use `revision` instead of computing it. Update `change_master_password_for_account` to pass `ticket.base_revision() + 1` (read it before `commit_rekey` consumes the ticket); its doc comment gains: "Local only. The desktop goes through the server first (`change_credentials`); this remains for tests and tools."

Extract the body of `complete_unlock` after `unwrap_vault_key` into `fn open_session(&self, vault_id: Uuid, vault_key: &Key256) -> Result<Session>`; `complete_unlock` calls it.

`adopt_and_unlock`:

```rust
    pub fn adopt_and_unlock(&mut self, prepared: PreparedVault) -> Result<()> {
        if self.state != VaultState::Locked {
            return Err(Error::Busy);
        }
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        let floor = self
            .store
            .account()?
            .ok_or(Error::InvalidInput("this vault is not linked to an account"))?
            .max_header_rev
            .max(local.revision as i64);
        let h = &prepared.header;
        if h.vault_id != local.vault_id || h.key_scheme != KeyScheme::AccountBound {
            return Err(Error::UnlockFailed);
        }
        if (h.revision as i64) <= floor {
            return Err(Error::UnlockFailed);
        }
        self.store.update_key_wrap(&h.kdf, &h.wrapped_vault_key, h.key_scheme, h.revision)?;
        self.store.raise_max_header_rev(h.revision as i64)?;
        let session = self.open_session(h.vault_id, &prepared.vault_key)?;
        self.session = Some(session);
        self.state = VaultState::Unlocked;
        Ok(())
    }
```

`prepare_sign_in` already verifies the attestation under the data key from the unwrapped vault key; `adopt_and_unlock` trusts only a `PreparedVault`, whose fields are `pub(crate)`, so callers outside the core cannot forge one.

- [ ] **Step 4: Run**

Run: `cargo test -p havenkeys-core`
Expected: PASS (fix any callers of `commit_rekey` in core tests to pass `base + 1`).

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core
git commit -m "feat(core): rekey yields login keys, commits at the server's revision, and adopts at unlock"
```

### Task 4: Desktop — server-first password change, unlock fallback, signed-out notice

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs` (`change_master_password`, `unlock_vault`)
- Modify: `apps/desktop/src-tauri/src/sync.rs` (`sync_now` drops the publish branch; `connect` emits `SIGNED_OUT_EVENT` on 401; new `pub const SIGNED_OUT_EVENT: &str = "vault://signed-out"`)
- Modify: `apps/desktop/src/lib/api.ts` (`onSignedOut`), `apps/desktop/src/App.tsx` (banner text), the settings view that calls `api.changeMasterPassword` (grep for it) for the 409 message

**Interfaces:**
- Consumes: Task 2 `SyncClient::change_credentials`, `CredentialChange`; Task 3 `Rekeyed::{kdf,current_auth_key,new_auth_key}`, `RekeyTicket::base_revision`, `encode_rekeyed_header`, `commit_rekey(.., revision)`, `adopt_and_unlock`; existing `prepare_sign_in`, `vault::derive_auth_key`.

- [ ] **Step 1: `change_master_password`** — replace the body after `check_new_master_password` with:

```rust
    // Adopt a change made elsewhere first, so the base revision is current.
    sync::sync_now(&app).await?;
    let kdf = KdfParams::generate()?;
    let account = /* as today */;
    let secret_key = /* as today (Task 10 changes this call) */;
    let ticket = state.vault()?.begin_rekey()?;
    let (ticket, rekeyed) = /* spawn_blocking as today */;
    let rekeyed = rekeyed?;
    let header = state.vault()?.encode_rekeyed_header(&ticket, &rekeyed)?;
    let (session, client) = (state.session()?, sync::client(&state)?);
    let revision = client
        .change_credentials(
            &session,
            CredentialChange {
                current_auth_key: rekeyed.current_auth_key(),
                kdf: rekeyed.kdf(),
                new_auth_key: rekeyed.new_auth_key(),
                header: &header,
                base_header_revision: ticket.base_revision() as i64,
            },
        )
        .await
        .map_err(|e| match e {
            SyncError::Conflict(_) => CmdError {
                code: "password_changed_elsewhere",
                message: "Your master password was changed on another device. Lock and unlock with the new password.".into(),
            },
            other => sync::failed(&app, other),
        })?;
    // The server has it. If this commit fails (the vault was locked in the
    // meantime), the next sync adopts the header from the server instead.
    state.vault()?.commit_rekey(ticket, Ok(rekeyed), revision as u64)?;
    Ok(())
```

Make `sync::failed` `pub(crate)`. Update the doc comment: server first, local second, and why.

- [ ] **Step 2: `sync_now`** — replace the header block with:

```rust
    let remote = client.header(&session).await.map_err(|e| failed(app, e))?;
    {
        let state = app.state::<AppState>();
        let revision = state.vault()?.header_revision()?.unwrap_or(0);
        if remote.revision as u64 > revision {
            report.header_adopted = state.vault()?.adopt_account_header(&remote.bytes)?;
        } else if revision > remote.revision as u64 {
            // The header only changes through `change_credentials`, which
            // the server applies before this device does. Ahead means
            // something is wrong locally; send nothing.
            return Err(CmdError::internal());
        }
    }
```

- [ ] **Step 3: `connect` emits the notice** — in `connect`, replace `.map_err(|e| failed(app, e))?` on `login` with:

```rust
        .map_err(|e| {
            if e == SyncError::Unauthorized {
                let _ = app.emit(SIGNED_OUT_EVENT, ());
            }
            failed(app, e)
        })?;
```

- [ ] **Step 4: `unlock_vault` fallback** — after `let mut v = state.vault()?; v.finish_unlock(ticket, key)?;` becomes a match:

```rust
    let unlocked = {
        let mut v = state.vault()?;
        v.finish_unlock(ticket, key)
    };
    if let Err(err) = unlocked {
        if err.code() != "unlock_failed" {
            return Err(err.into());
        }
        // The password may have been changed on another device: this
        // device's header still has the old salt. Ask the server. Any
        // failure below is reported as the original wrong password.
        return match unlock_from_server(&app, password_for_fallback, stored_or_typed).await {
            Ok(status) => Ok(status),
            Err(_) => Err(err.into()),
        };
    }
```

Before the `spawn_blocking`, clone the password (`let password_for_fallback = password.clone();`) and keep the Secret Key text (`SecretKey` is not `Clone`; keep `key_text`/stored text as `SecretString` and re-parse it). Then add:

```rust
/// Unlock with a header the server serves, when the local one no longer
/// matches the password (changed on another device). The header is verified
/// exactly as a sign-in verifies it, and adopted only if it is newer.
async fn unlock_from_server(
    app: &AppHandle,
    password: SecretString,
    secret_key_text: SecretString,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    let record = state.vault()?.account()?.ok_or(havenkeys_core::Error::NoVault)?;
    let account = record.to_ref()?;
    let local_kdf = state.vault()?.kdf()?; // add `VaultService::kdf()` if missing: the header's KdfParams
    let client = sync::client(&state)?;
    let params = client.auth_params(account.email.as_str()).await?;
    if params.account_id != account.id || params.kdf == local_kdf {
        return Err(havenkeys_core::Error::UnlockFailed.into());
    }
    let (password, key_text, account_c, kdf) = (password.clone(), secret_key_text.clone(), account.clone(), params.kdf.clone());
    let auth_key = tauri::async_runtime::spawn_blocking(move || {
        let sk = SecretKey::parse(key_text.expose())?;
        vault::derive_auth_key(&password, &sk, &kdf, &account_c)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    let session = client
        .login(account.email.as_str(), &auth_key, account.id, state.device_id()?, sync::DEVICE_NAME)
        .await?;
    let header = client.header(&session).await?;
    let account_c = account.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let sk = SecretKey::parse(secret_key_text.expose())?;
        prepare_sign_in(&header.bytes, &password, &sk, &account_c).map(|(p, _)| p)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    let minutes = {
        let mut v = state.vault()?;
        v.adopt_and_unlock(prepared)?;
        v.settings()?.auto_lock_minutes
    };
    state.arm_auto_lock(minutes);
    state.notify_unlocked();
    state.set_online(session);
    let _ = app.emit(sync::CONNECTIVITY_EVENT, true);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sync::sync_now(&handle).await;
    });
    Ok(state.vault()?.status()?)
}
```

(Fix the double `password` move by cloning once per closure. Only reach this when an account record exists; `NoVault` otherwise.) Add `pub fn kdf(&self) -> Result<Option<KdfParams>>` to `VaultService` if there is no accessor, and compare against `Some`.

- [ ] **Step 5: Frontend** — in `api.ts` add:

```ts
  /** The server refused this computer's sign-in after an unlock. */
  onSignedOut: (handler: () => void): Promise<UnlistenFn> => listen("vault://signed-out", () => handler()),
```

In `App.tsx` keep a `signedOut` flag (set by `onSignedOut`, cleared on lock and on `onConnectivity(true)`), and when it is set show instead of the offline banner:

```tsx
<div className="banner" role="status">
  The server did not accept this computer's sign-in. If your master password was changed on another
  device, lock and unlock with the new one.
</div>
```

Find the password-change form (`grep -rn changeMasterPassword apps/desktop/src`): it already shows `ApiError.message`, so the 409 message appears as is; also add a line to that form's help text: "Other computers signed in to this account will be signed out."

- [ ] **Step 6: Verify**

Run: `cargo clippy -p havenkeys-desktop --all-targets -- -D warnings` (check the package name in `apps/desktop/src-tauri/Cargo.toml`), `cargo test -p havenkeys-desktop`, `pnpm -C apps/desktop typecheck`, `pnpm -C apps/desktop test` (check the package manager in the root `package.json`).
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop
git commit -m "feat(desktop): change the master password server-first and follow a change made elsewhere"
```

### Task 5: Round trip — a password change across two devices

**Files:**
- Test: `crates/havenkeys-sync-client/tests/round_trip.rs`

- [ ] **Step 1: Write the test** (uses the file's `Server`, `activate`, `push`, `login_item`, `cheap_kdf`):

```rust
#[tokio::test]
async fn a_password_change_reaches_the_second_device() {
    let server = Server::start().await;
    let client = server.client();
    let mut one = activate(&server, "rekey@example.com").await;

    // Device two signs in with the original password.
    let params = client.auth_params("rekey@example.com").await.unwrap();
    let auth = derive_auth_key(&PASSWORD.into(), &one.secret_key, &params.kdf, &one.account).unwrap();
    let session_two = client.login("rekey@example.com", &auth, one.account.id, Uuid::new_v4(), "Laptop").await.unwrap();

    // Device one changes it, server first.
    const NEW: &str = "a much better master password";
    let ticket = one.vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_for_account(&PASSWORD.into(), &NEW.into(), cheap_kdf(), &one.secret_key, &one.account)
        .unwrap();
    let header = one.vault.encode_rekeyed_header(&ticket, &rekeyed).unwrap();
    let revision = client
        .change_credentials(&one.session, CredentialChange {
            current_auth_key: rekeyed.current_auth_key(),
            kdf: rekeyed.kdf(),
            new_auth_key: rekeyed.new_auth_key(),
            header: &header,
            base_header_revision: 0,
        })
        .await
        .unwrap();
    assert_eq!(revision, 1);
    one.vault.commit_rekey(ticket, Ok(rekeyed), 1).unwrap();

    // Device two's session is gone; the old password no longer logs in.
    assert_eq!(client.pull(&session_two, 0).await.unwrap_err(), SyncError::Unauthorized);
    let old = derive_auth_key(&PASSWORD.into(), &one.secret_key, &params.kdf, &one.account).unwrap();
    assert!(client.login("rekey@example.com", &old, one.account.id, Uuid::new_v4(), "Laptop").await.is_err());

    // The new password, with the parameters the server now serves, does,
    // and the header it serves opens with it.
    let params = client.auth_params("rekey@example.com").await.unwrap();
    let auth = derive_auth_key(&NEW.into(), &one.secret_key, &params.kdf, &one.account).unwrap();
    let session_two = client.login("rekey@example.com", &auth, one.account.id, Uuid::new_v4(), "Laptop").await.unwrap();
    let served = client.header(&session_two).await.unwrap();
    assert_eq!(served.revision, 1);
    prepare_sign_in(&served.bytes, &NEW.into(), &one.secret_key, &one.account).unwrap();

    // Device one kept its session and can still write.
    let staged = one.vault.stage_create(login_item("After", "pw"), NOW).unwrap();
    push(&client, &mut one, vec![staged]).await.unwrap();

    server.cleanup().await;
}

#[tokio::test]
async fn a_lost_response_heals_on_the_next_sync() {
    let server = Server::start().await;
    let client = server.client();
    let mut one = activate(&server, "lost@example.com").await;
    const NEW: &str = "a much better master password";
    let ticket = one.vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_for_account(&PASSWORD.into(), &NEW.into(), cheap_kdf(), &one.secret_key, &one.account)
        .unwrap();
    let header = one.vault.encode_rekeyed_header(&ticket, &rekeyed).unwrap();
    client
        .change_credentials(&one.session, CredentialChange {
            current_auth_key: rekeyed.current_auth_key(),
            kdf: rekeyed.kdf(),
            new_auth_key: rekeyed.new_auth_key(),
            header: &header,
            base_header_revision: 0,
        })
        .await
        .unwrap();
    drop((ticket, rekeyed)); // the response "never arrived": nothing committed

    let served = client.header(&one.session).await.unwrap();
    assert!(one.vault.adopt_account_header(&served.bytes).unwrap());
    one.vault.lock();
    one.vault.unlock_for_account(&NEW.into(), &one.secret_key, &one.account).unwrap();
    server.cleanup().await;
}
```

Add `CredentialChange` to the file's `use havenkeys_sync_client::{…}`.

- [ ] **Step 2: Run**

Run: `scripts/test-server.sh -p havenkeys-sync-client --test round_trip`
Expected: PASS. (If it fails, the bug is in Tasks 1–3; fix it there.)

- [ ] **Step 3: Commit**

```bash
git add crates/havenkeys-sync-client/tests/round_trip.rs
git commit -m "test: a password change reaches a second device and survives a lost response"
```

---

## Part C — Unreadable items

### Task 6: Core — record, retry and count unreadable items

**Files:**
- Modify: `crates/havenkeys-core/src/store.rs` (schema 5, `unreadable_items`, `apply_pull`, `unreadable_ids`, `unreadable_count`, `reset_cursor`)
- Modify: `crates/havenkeys-core/src/sync.rs` (`apply_remote_changes` via a shared `apply_changes`; new `apply_refetched`, `unreadable_item_ids`; `reset_sync_cursor` uses `reset_cursor`)
- Modify: `crates/havenkeys-core/src/vault.rs` (`VaultStatus.unreadable_items`)
- Test: `crates/havenkeys-core/src/sync.rs` `mod tests`

**Interfaces:**
- Produces:
```rust
// Store
pub fn apply_pull(&mut self, rows: &[(Uuid, Vec<u8>, Vec<u8>, i64)], deletions: &[Uuid],
                  unreadable: &[(Uuid, i64)], cursor: Option<(i64, i64)>) -> Result<usize>; // returns rows deleted
pub fn unreadable_ids(&self) -> Result<Vec<Uuid>>;
pub fn unreadable_count(&self) -> Result<usize>;
pub fn reset_cursor(&mut self, synced_at: i64) -> Result<()>; // cursor 0 + clear unreadable, one tx
// VaultService
pub fn apply_refetched(&mut self, requested: &[Uuid], changes: Vec<RemoteChange>, now_ms: i64) -> Result<SyncReport>;
pub fn unreadable_item_ids(&self) -> Result<Vec<Uuid>>;
// VaultStatus { state, vault_exists, damaged_items, unreadable_items: usize }
```

- [ ] **Step 1: Write failing tests** in `sync.rs` `mod tests` (reuse `activated_vault`, `login`):

```rust
    fn good_change(vault: &mut VaultService, title: &str, revision: i64) -> RemoteChange {
        let staged = vault.stage_create(login(title), NOW).unwrap();
        let id = staged.item_id;
        let (ov, det) = (staged.overview.clone().unwrap(), staged.details.clone().unwrap());
        drop(staged);
        RemoteChange { item_id: id, revision, overview: Some(ov), details: Some(det), deleted: false }
    }

    fn bad_change(id: Uuid, revision: i64) -> RemoteChange {
        RemoteChange { item_id: id, revision, overview: Some(vec![0u8; 64]), details: Some(vec![0u8; 64]), deleted: false }
    }

    #[test]
    fn an_unreadable_item_is_recorded_and_the_cursor_still_advances() {
        let mut vault = activated_vault();
        let id = Uuid::from_u128(9);
        vault.apply_remote_changes(3, vec![bad_change(id, 3)], NOW).unwrap();
        assert_eq!(vault.unreadable_item_ids().unwrap(), vec![id]);
        assert_eq!(vault.status().unwrap().unreadable_items, 1);
        assert_eq!(vault.account().unwrap().unwrap().server_cursor, 3);
    }

    #[test]
    fn a_later_good_revision_clears_it() {
        let mut vault = activated_vault();
        let good = good_change(&mut vault, "Fixed", 4);
        vault.apply_remote_changes(3, vec![bad_change(good.item_id, 3)], NOW).unwrap();
        vault.apply_remote_changes(4, vec![good], NOW).unwrap();
        assert!(vault.unreadable_item_ids().unwrap().is_empty());
    }

    #[test]
    fn a_deletion_clears_it() {
        let mut vault = activated_vault();
        let id = Uuid::from_u128(9);
        vault.apply_remote_changes(3, vec![bad_change(id, 3)], NOW).unwrap();
        vault.apply_remote_changes(4, vec![RemoteChange { item_id: id, revision: 4, overview: None, details: None, deleted: true }], NOW).unwrap();
        assert!(vault.unreadable_item_ids().unwrap().is_empty());
    }

    #[test]
    fn a_refetch_that_decrypts_clears_it_without_moving_the_cursor() {
        let mut vault = activated_vault();
        let good = good_change(&mut vault, "Fixed", 3);
        vault.apply_remote_changes(3, vec![bad_change(good.item_id, 3)], NOW).unwrap();
        let id = good.item_id;
        vault.apply_refetched(&[id], vec![good], NOW).unwrap();
        assert!(vault.unreadable_item_ids().unwrap().is_empty());
        assert_eq!(vault.account().unwrap().unwrap().server_cursor, 3);
        assert!(vault.get_item(&id).is_ok());
    }

    #[test]
    fn a_refetch_the_server_no_longer_has_removes_it() {
        let mut vault = activated_vault();
        let id = Uuid::from_u128(9);
        vault.apply_remote_changes(3, vec![bad_change(id, 3)], NOW).unwrap();
        vault.apply_refetched(&[id], vec![], NOW).unwrap();
        assert!(vault.unreadable_item_ids().unwrap().is_empty());
    }

    #[test]
    fn a_permanently_bad_item_stays_counted_once() {
        let mut vault = activated_vault();
        let id = Uuid::from_u128(9);
        vault.apply_remote_changes(3, vec![bad_change(id, 3)], NOW).unwrap();
        for _ in 0..3 {
            vault.apply_refetched(&[id], vec![bad_change(id, 3)], NOW).unwrap();
        }
        assert_eq!(vault.status().unwrap().unreadable_items, 1);
    }

    #[test]
    fn a_refetch_ignores_ids_it_did_not_ask_for() {
        let mut vault = activated_vault();
        let good = good_change(&mut vault, "Sneaky", 3);
        let report = vault.apply_refetched(&[Uuid::from_u128(1)], vec![good], NOW).unwrap();
        assert_eq!(report.added, 0);
        assert!(vault.list_items().unwrap().is_empty());
    }

    #[test]
    fn a_reset_clears_the_table() {
        let mut vault = activated_vault();
        vault.apply_remote_changes(3, vec![bad_change(Uuid::from_u128(9), 3)], NOW).unwrap();
        vault.reset_sync_cursor(NOW).unwrap();
        assert!(vault.unreadable_item_ids().unwrap().is_empty());
        assert_eq!(vault.account().unwrap().unwrap().server_cursor, 0);
    }
```

If `StagedWrite`'s blob fields are not readable from here, build the good change through `stage_create` + `commit_write` + reading back `store.item_overviews()`/`item_details()` then `store.delete_item`, as the existing `a_remote_change_adds_updates_and_deletes…` test does.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-core sync::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the store**

Add to `SCHEMA` and bump `SCHEMA_VERSION` to 5:

```sql
CREATE TABLE unreadable_items (
    id       TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL
);
```

```rust
    /// Everything one pull changes, in one transaction: upserts, deletions,
    /// the unreadable-item bookkeeping and (for a pull, not a refetch) the
    /// cursor. A crash can no longer leave the cursor past rows that were
    /// never written. Returns how many rows were deleted.
    pub fn apply_pull(
        &mut self,
        rows: &[(Uuid, Vec<u8>, Vec<u8>, i64)],
        deletions: &[Uuid],
        unreadable: &[(Uuid, i64)],
        cursor: Option<(i64, i64)>,
    ) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut deleted = 0;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO items (id, overview, details, revision) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                               details  = excluded.details,
                                               revision = excluded.revision",
            )?;
            let mut clear = tx.prepare("DELETE FROM unreadable_items WHERE id = ?1")?;
            let mut delete = tx.prepare("DELETE FROM items WHERE id = ?1")?;
            let mut record = tx.prepare(
                "INSERT INTO unreadable_items (id, revision) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET revision = excluded.revision",
            )?;
            for (id, overview, details, revision) in rows {
                upsert.execute(params![id.to_string(), overview, details, revision])?;
                clear.execute(params![id.to_string()])?;
            }
            for id in deletions {
                deleted += delete.execute(params![id.to_string()])?;
                clear.execute(params![id.to_string()])?;
            }
            for (id, revision) in unreadable {
                record.execute(params![id.to_string(), revision])?;
            }
        }
        if let Some((cursor, synced_at)) = cursor {
            tx.execute(
                "UPDATE account SET server_cursor = ?1, last_synced_at = ?2 WHERE id = 1",
                params![cursor, synced_at],
            )?;
        }
        tx.commit()?;
        Ok(deleted)
    }
```

Plus `unreadable_ids` (`SELECT id FROM unreadable_items ORDER BY id`, parse UUIDs, skip malformed), `unreadable_count` (`SELECT count(*)`), and `reset_cursor` (one transaction: `UPDATE account SET server_cursor = 0, last_synced_at = ?1`; `DELETE FROM unreadable_items`). Remove `upsert_items` if nothing else uses it (grep).

- [ ] **Step 4: Implement the service**

Rename the body of `apply_remote_changes` into `fn apply_changes(&mut self, changes: Vec<RemoteChange>, cursor: Option<i64>, now_ms: i64) -> Result<SyncReport>`, collecting `unreadable: Vec<(Uuid, i64)>` where it increments `skipped_items`, and replacing the writes with:

```rust
        let deleted = self.store.apply_pull(&rows, &deletions, &unreadable, cursor.map(|c| (c, now_ms)))?;
        report.deleted = deleted;
        for overview in overviews {
            self.session_mut()?.overviews.insert(overview.id, overview);
        }
        for id in &deletions {
            self.session_mut()?.overviews.remove(id);
        }
```

Then:

```rust
    pub fn apply_remote_changes(&mut self, cursor: i64, changes: Vec<RemoteChange>, now_ms: i64) -> Result<SyncReport> {
        self.apply_changes(changes, Some(cursor), now_ms)
    }

    /// Apply a fetch-by-ID of items that did not open before. The cursor does
    /// not move. Changes for IDs that were not asked for are dropped: the
    /// server does not get to add items through a retry. An ID the server
    /// no longer has is treated as deleted.
    pub fn apply_refetched(&mut self, requested: &[Uuid], changes: Vec<RemoteChange>, now_ms: i64) -> Result<SyncReport> {
        let wanted: std::collections::HashSet<Uuid> = requested.iter().copied().collect();
        let mut changes: Vec<RemoteChange> = changes.into_iter().filter(|c| wanted.contains(&c.item_id)).collect();
        let returned: std::collections::HashSet<Uuid> = changes.iter().map(|c| c.item_id).collect();
        for id in requested.iter().filter(|id| !returned.contains(id)) {
            changes.push(RemoteChange { item_id: *id, revision: 0, overview: None, details: None, deleted: true });
        }
        self.apply_changes(changes, None, now_ms)
    }

    pub fn unreadable_item_ids(&self) -> Result<Vec<Uuid>> {
        self.store.unreadable_ids()
    }
```

`reset_sync_cursor` calls `self.store.reset_cursor(now_ms)`; update its doc comment (the "silently, for good" paragraph is no longer true: point at `unreadable_items`). In `VaultService::status`, add `unreadable_items: self.store.unreadable_count().unwrap_or(0)` (0 when there is no vault). Update the module doc of `sync.rs` and the doc of `apply_remote_changes` ("counted in `skipped_items` and recorded in `unreadable_items` for a retry").

- [ ] **Step 5: Run**

Run: `cargo test -p havenkeys-core`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-core
git commit -m "feat(core): record unreadable pulled items for retry and apply each pull in one transaction"
```

### Task 7: Server route `POST /v1/items/fetch`

**Files:**
- Modify: `crates/havenkeys-server/src/routes/items.rs` (add `fetch`), `routes/mod.rs` (route)
- Test: `crates/havenkeys-server/tests/sync.rs`, `tests/isolation.rs` (route list)

**Interfaces:**
- Produces: `POST /v1/items/fetch` `{"itemIds": [uuid, …]}` (1–500, distinct) → `200 {"changes": [ {itemId, revision, overview, details, deleted} ]}` — same change shape as `/v1/sync`.

- [ ] **Step 1: Write failing tests** in `tests/sync.rs`:

```rust
async fn fetch(server: &TestServer, sess: &Sess, ids: &[uuid::Uuid]) -> (u16, Value) {
    let res = server.post_as("/v1/items/fetch", sess).json(&json!({ "itemIds": ids })).send().await.unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_fetch_returns_current_rows_and_tombstones() {
    let server = TestServer::start().await;
    let (_a, sess) = signed_in(&server, "fetch@example.com").await;
    let (live, _) = create_item(&server, &sess, b"ov", b"det").await;
    let (gone, rev) = create_item(&server, &sess, b"ov2", b"det2").await;
    write(&server, &sess, vec![deletion(gone, Some(rev))]).await;
    let unknown = uuid::Uuid::new_v4();

    let (status, body) = fetch(&server, &sess, &[live, gone, unknown]).await;
    assert_eq!(status, 200);
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    let by_id = |id: uuid::Uuid| changes.iter().find(|c| c["itemId"] == id.to_string()).unwrap();
    assert_eq!(by_id(live)["deleted"], false);
    assert_eq!(by_id(gone)["deleted"], true);
    assert!(by_id(gone)["overview"].is_null());
    server.cleanup().await;
}

#[tokio::test]
async fn a_fetch_is_bounded_and_well_formed() {
    let server = TestServer::start().await;
    let (_a, sess) = signed_in(&server, "fetch-bad@example.com").await;
    assert_eq!(fetch(&server, &sess, &[]).await.0, 400);
    let many: Vec<uuid::Uuid> = (0..501).map(|_| uuid::Uuid::new_v4()).collect();
    assert_eq!(fetch(&server, &sess, &many).await.0, 400);
    let id = uuid::Uuid::new_v4();
    assert_eq!(fetch(&server, &sess, &[id, id]).await.0, 400);
    let res = server.post_as("/v1/items/fetch", &sess).json(&json!({ "itemIds": [], "x": 1 })).send().await.unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}
```

In `tests/isolation.rs`, add a test that account B fetching account A's item ID gets `changes: []`, and add `("POST", "/v1/items/fetch")` to the authenticated-route list at ~116 (with a valid body if that list sends bodies; read how it does).

- [ ] **Step 2: Run to verify failure**

Run: `scripts/test-server.sh -p havenkeys-server --test sync --test isolation`
Expected: FAIL (405/404).

- [ ] **Step 3: Implement** in `items.rs`:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FetchRequest {
    item_ids: Vec<Uuid>,
}

/// The current row for each requested item in the caller's vault, in the
/// pull's change shape. For a client retrying items it could not open. IDs
/// in another vault, or not at all, are simply absent: the answer is the
/// same either way, so it is not an oracle.
pub async fn fetch(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<FetchRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if req.item_ids.is_empty() || req.item_ids.len() > MAX_CHANGES_PER_BATCH {
        return Err(ApiError::InvalidRequest("itemIds is not valid"));
    }
    let distinct: HashSet<Uuid> = req.item_ids.iter().copied().collect();
    if distinct.len() != req.item_ids.len() {
        return Err(ApiError::InvalidRequest("an item appears twice"));
    }
    let db = state.pool.get().await?;
    let rows = db
        .query(
            "SELECT item_id, revision, overview, details, deleted_at IS NOT NULL
               FROM items WHERE vault_id = $1 AND item_id = ANY($2)
              ORDER BY item_id",
            &[&session.vault_id, &req.item_ids],
        )
        .await?;
    let changes: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let overview: Option<Vec<u8>> = row.get(2);
            let details: Option<Vec<u8>> = row.get(3);
            serde_json::json!({
                "itemId": row.get::<_, Uuid>(0),
                "revision": row.get::<_, i64>(1),
                "overview": overview.map(Blob),
                "details": details.map(Blob),
                "deleted": row.get::<_, bool>(4),
            })
        })
        .collect();
    Ok(axum::Json(serde_json::json!({ "changes": changes })))
}
```

Register `.route("/v1/items/fetch", post(items::fetch))`.

- [ ] **Step 4: Run**

Run: `scripts/test-server.sh -p havenkeys-server`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): fetch items by id for retrying unreadable ones"
```

### Task 8: Sync client `fetch_items`

**Files:**
- Modify: `crates/havenkeys-sync-client/src/client.rs`, `src/wire.rs`
- Test: `crates/havenkeys-sync-client/tests/hostile.rs`

**Interfaces:**
- Produces: `pub async fn fetch_items(&self, session: &Session, ids: &[Uuid]) -> Result<Vec<RemoteChange>>`

- [ ] **Step 1: Failing tests** in `hostile.rs`:

```rust
#[tokio::test]
async fn a_fetch_answer_is_bounded_to_what_was_asked() {
    let asked = Uuid::from_u128(1);
    let other = Uuid::from_u128(2);
    let body = format!(r#"{{"changes":[{{"itemId":"{other}","revision":1,"deleted":true}}]}}"#);
    let err = Stub::ok(&body).fetch_items(&session(), &[asked]).await.unwrap_err();
    assert!(matches!(err, SyncError::Protocol(_)));

    let body = format!(r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}},{{"itemId":"{asked}","revision":1,"deleted":true}}]}}"#);
    let err = Stub::ok(&body).fetch_items(&session(), &[asked]).await.unwrap_err();
    assert!(matches!(err, SyncError::Protocol(_)));

    let body = format!(r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}}]}}"#);
    assert_eq!(Stub::ok(&body).fetch_items(&session(), &[asked]).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_fetch_request_is_bounded() {
    assert!(Stub::ok("{}").fetch_items(&session(), &[]).await.is_err());
    let many: Vec<Uuid> = (0..501).map(|i| Uuid::from_u128(i)).collect();
    assert!(Stub::ok("{}").fetch_items(&session(), &many).await.is_err());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-sync-client --test hostile`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`wire.rs`:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FetchBody<'a> {
    pub item_ids: &'a [Uuid],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchDto {
    pub changes: Vec<RemoteChangeDto>,
}
```

`client.rs`:

```rust
    /// The current version of specific items, for retrying ones that did not
    /// open. Every returned change must be for an ID that was asked for,
    /// and at most once.
    pub async fn fetch_items(&self, session: &Session, ids: &[Uuid]) -> Result<Vec<RemoteChange>> {
        if ids.is_empty() || ids.len() > MAX_CHANGES {
            return Err(SyncError::Refused("item list is not valid"));
        }
        let response = self
            .send(Method::Post, "/v1/items/fetch", Some(session), Some(json(&wire::FetchBody { item_ids: ids })?))
            .await?;
        let dto: wire::FetchDto = self.expect_ok(response)?;
        let asked: std::collections::HashSet<Uuid> = ids.iter().copied().collect();
        let mut seen = std::collections::HashSet::new();
        for change in &dto.changes {
            if !asked.contains(&change.item_id) || !seen.insert(change.item_id) {
                return Err(SyncError::Protocol("an item that was not asked for"));
            }
        }
        dto.changes.into_iter().map(RemoteChange::try_from).collect()
    }
```

- [ ] **Step 4: Run**

Run: `cargo test -p havenkeys-sync-client --test hostile`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-sync-client
git commit -m "feat(sync-client): fetch items by id"
```

### Task 9: Desktop — retry on every sync, persistent banner

**Files:**
- Modify: `apps/desktop/src-tauri/src/sync.rs` (`sync_now` retry)
- Modify: `apps/desktop/src/lib/types.ts` (`VaultStatus.unreadableItems`), `apps/desktop/src/App.tsx`, `apps/desktop/src/views/VaultScreen.tsx`
- Test: `crates/havenkeys-sync-client/tests/round_trip.rs` (end-to-end retry)

**Interfaces:**
- Consumes: Task 6 `unreadable_item_ids`, `apply_refetched`, `VaultStatus.unreadable_items`; Task 8 `fetch_items`.

- [ ] **Step 1: Round-trip test** — a blob written by a device that is valid for the server but unreadable for this vault is recorded, and fixing it on the server clears it on the next retry:

```rust
#[tokio::test]
async fn an_unreadable_item_is_retried_and_clears_once_fixed() {
    let server = Server::start().await;
    let client = server.client();
    let mut one = activate(&server, "retry@example.com").await;
    let staged = one.vault.stage_create(login_item("Good", "pw"), NOW).unwrap();
    let id = staged.item_id;
    let good = (staged.overview.clone().unwrap(), staged.details.clone().unwrap());
    // Put garbage on the server under that id, bypassing the core.
    let bad = havenkeys_core::vault::StagedWrite::for_test(id, None, vec![0u8; 64], vec![0u8; 64]);
    client.write(&one.session, &[bad]).await.unwrap();
    let pulled = client.pull(&one.session, 0).await.unwrap();
    one.vault.apply_remote_changes(pulled.cursor, pulled.changes, NOW).unwrap();
    assert_eq!(one.vault.unreadable_item_ids().unwrap(), vec![id]);

    // Fix it on the server, then retry by id.
    let revision = client.fetch_items(&one.session, &[id]).await.unwrap()[0].revision;
    let fixed = havenkeys_core::vault::StagedWrite::for_test(id, Some(revision), good.0, good.1);
    client.write(&one.session, &[fixed]).await.unwrap();
    let ids = one.vault.unreadable_item_ids().unwrap();
    let changes = client.fetch_items(&one.session, &ids).await.unwrap();
    one.vault.apply_refetched(&ids, changes, NOW).unwrap();
    assert!(one.vault.unreadable_item_ids().unwrap().is_empty());
    assert_eq!(one.vault.get_item(&id).unwrap().title, "Good");
    server.cleanup().await;
}
```

`StagedWrite` has private fields; add to the core, gated for tests:

```rust
impl StagedWrite {
    #[cfg(any(test, feature = "test-util"))]
    pub fn for_test(item_id: Uuid, base_revision: Option<i64>, overview: Vec<u8>, details: Vec<u8>) -> Self { /* fill fields; other fields default */ }
}
```

and enable `havenkeys-core/test-util` in `havenkeys-sync-client`'s `[dev-dependencies]` (add the feature to `havenkeys-core/Cargo.toml` if missing). If `StagedWrite`'s `overview`/`details` are already `pub` (the wire conversion reads them, so they are at least crate-visible to the sync client), construct it directly and skip `for_test`.

- [ ] **Step 2: Run to verify**

Run: `scripts/test-server.sh -p havenkeys-sync-client --test round_trip`
Expected: PASS (Tasks 6–8 already implement it). If it fails, fix in the owning task.

- [ ] **Step 3: `sync_now` retry** — after the pull loop, before `mark_sync_attempt`:

```rust
    // Items that did not open earlier are asked for again, by id, on every
    // sync, until they open, are deleted, or a Re-download clears them.
    let pending = app.state::<AppState>().vault()?.unreadable_item_ids()?;
    for chunk in pending.chunks(MAX_BATCH) {
        let changes = client
            .fetch_items(&session, chunk)
            .await
            .map_err(|e| failed(app, e))?;
        let state = app.state::<AppState>();
        let page = state.vault()?.apply_refetched(chunk, changes, AppState::now_ms())?;
        report.added += page.added;
        report.updated += page.updated;
        report.deleted += page.deleted;
    }
```

(Do not add `page.skipped_items` to the report: an item still unreadable is already counted in the status; a per-sync toast for it every 60 s would be noise.)

- [ ] **Step 4: Frontend** — `types.ts`: add `unreadableItems: number;` to `VaultStatus` with the comment "Items from the server that could not be opened; retried on every sync." `App.tsx`: pass `unreadableItems={status.unreadableItems}` to `VaultScreen`, and in the `onLocked` reset also set `unreadableItems: 0`. `VaultScreen.tsx`: add the prop, hold it in state, refresh it from `api.status()` inside the existing `onSynced` listener (always, not only when counts changed), and render above the list:

```tsx
{unreadable > 0 && (
  <div className="banner banner-warn" role="status">
    {unreadable === 1 ? "1 item couldn't be read from the server." : `${unreadable} items couldn't be read from the server.`}{" "}
    <button className="btn btn-quiet" type="button" disabled={readOnly || busyResync} onClick={redownload}>
      Re-download
    </button>
  </div>
)}
```

with `redownload` calling `api.resync()`, then `api.status()` to refresh the count and `refresh()` for the list, and toasting errors like `AccountSection.resync` does. Add a `.banner-warn` rule to `styles.css` using the existing warning/danger tokens (grep `styles.css` for the current `.banner` rule and the token names; reuse them, add no new colors).

- [ ] **Step 5: Verify**

Run: `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm -C apps/desktop typecheck`, `pnpm -C apps/desktop test`.
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop crates
git commit -m "feat(desktop): retry unreadable items on every sync and show them until resolved"
```

---

## Part D — Secret Key in the OS keychain

### Task 10: `KeyStore` trait, keychain backend, `Device` refactor

**Files:**
- Create: `apps/desktop/src-tauri/src/secret_store.rs`
- Modify: `apps/desktop/src-tauri/src/device.rs`, `account.rs`, `commands.rs`, `lib.rs` (module + construction), `Cargo.toml`
- Modify: `apps/desktop/src/lib/types.ts` (`DeviceStatus.secretKeyStorage`), `apps/desktop/src/views/AccountSection.tsx` (warning)
- Modify: `deny.toml` only if a new licence or advisory needs an entry (investigate first; do not blanket-allow)
- Test: `device.rs` `mod tests`, `secret_store.rs` `mod tests`

**Interfaces:**
- Produces:
```rust
// secret_store.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Storage { Keychain, File, None }
pub trait KeyStore: Send {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError>;
    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError>;
    fn delete(&self, account: Uuid) -> Result<(), StoreError>;
}
pub struct StoreError; // no payload: never carries a value
pub struct OsKeyStore;  // real keychain, 5 s timeout per call
pub struct MemoryKeyStore; // tests (cfg(test))
// device.rs
impl Device {
    pub fn load(dir: &Path, store: Box<dyn KeyStore>) -> Self;
    pub fn secret_key(&mut self, account: Uuid) -> Option<SecretKey>;
    pub fn secret_key_text(&mut self, account: Uuid) -> Option<SecretString>;
    pub fn set_secret_key(&mut self, account: Uuid, key: &SecretKey) -> std::io::Result<Storage>;
    pub fn storage(&mut self, account: Uuid) -> Storage;
    /// Move a key left in device.json into the keychain, if one is available.
    pub fn migrate(&mut self, account: Uuid);
    /// Remove the key everywhere and start over with a new device id (Task 11).
    pub fn forget(&mut self, account: Uuid) -> std::io::Result<()>;
}
```

- [ ] **Step 1: Pick and verify the dependency.** Run `cargo info keyring-core`, `cargo info windows-native-keyring-store`, `cargo info apple-native-keyring-store`, `cargo info zbus-secret-service-keyring-store`, and read their docs (docs.rs, or context7) for: how to construct each platform store, how to make it the default (`keyring_core::set_default_store`), `Entry::new(service, user)`, `set_password`, `get_password`, `delete_credential`, and the "no entry" error variant. Record the licence of each (must be in `deny.toml`'s allow list). Add them with target-specific tables:

```toml
[dependencies]
keyring-core = "<version>"

[target.'cfg(windows)'.dependencies]
windows-native-keyring-store = "<version>"

[target.'cfg(target_os = "macos")'.dependencies]
apple-native-keyring-store = { version = "<version>", features = ["keychain"] }

[target.'cfg(target_os = "linux")'.dependencies]
zbus-secret-service-keyring-store = "<version>"
```

If the Linux store needs a crypto feature for the session (encrypted transfer over D-Bus), enable the pure-Rust one. Run `cargo deny check` and `cargo audit`; investigate anything new before continuing (Global Constraints). If `keyring-core`'s API differs from the names above, adapt `OsKeyStore` only; nothing else touches it.

- [ ] **Step 2: Failing tests** in `device.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store::{FailingKeyStore, MemoryKeyStore, SlowKeyStore, Storage};

    const ACCOUNT: Uuid = Uuid::from_u128(7);

    fn key() -> SecretKey { SecretKey::generate().unwrap() }

    fn file_text(dir: &Path) -> String { std::fs::read_to_string(dir.join("device.json")).unwrap() }

    #[test]
    fn a_key_goes_to_the_keychain_and_not_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Device::load(dir.path(), Box::new(MemoryKeyStore::default()));
        let k = key();
        assert_eq!(d.set_secret_key(ACCOUNT, &k).unwrap(), Storage::Keychain);
        assert!(!file_text(dir.path()).contains("secretKey\": \"H1"));
        assert_eq!(d.secret_key_text(ACCOUNT).unwrap().expose(), k.to_text().expose());
    }

    #[test]
    fn without_a_keychain_the_key_falls_back_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Device::load(dir.path(), Box::new(FailingKeyStore));
        let k = key();
        assert_eq!(d.set_secret_key(ACCOUNT, &k).unwrap(), Storage::File);
        assert!(file_text(dir.path()).contains(k.to_text().expose()));
        assert_eq!(d.storage(ACCOUNT), Storage::File);
    }

    #[test]
    fn a_keychain_that_hangs_falls_back_within_the_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Device::load(dir.path(), Box::new(SlowKeyStore::new(std::time::Duration::from_secs(30))));
        let started = std::time::Instant::now();
        assert_eq!(d.set_secret_key(ACCOUNT, &key()).unwrap(), Storage::File);
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    #[test]
    fn a_key_in_the_file_is_moved_to_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let k = key();
        {
            let mut d = Device::load(dir.path(), Box::new(FailingKeyStore));
            d.set_secret_key(ACCOUNT, &k).unwrap();
        }
        let store = MemoryKeyStore::default();
        let mut d = Device::load(dir.path(), Box::new(store.clone()));
        d.migrate(ACCOUNT);
        assert!(!file_text(dir.path()).contains(k.to_text().expose()));
        assert_eq!(store.get(ACCOUNT).unwrap().unwrap().expose(), k.to_text().expose());
        assert_eq!(d.storage(ACCOUNT), Storage::Keychain);
    }

    #[test]
    fn forget_removes_the_key_everywhere_and_renews_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryKeyStore::default();
        let mut d = Device::load(dir.path(), Box::new(store.clone()));
        let old_id = d.id;
        d.set_secret_key(ACCOUNT, &key()).unwrap();
        d.forget(ACCOUNT).unwrap();
        assert!(store.get(ACCOUNT).unwrap().is_none());
        assert!(d.secret_key(ACCOUNT).is_none());
        assert_ne!(d.id, old_id);
        assert_eq!(d.storage(ACCOUNT), Storage::None);
    }
}
```

`MemoryKeyStore` is `Clone` over an `Arc<Mutex<HashMap<Uuid, String>>>`; `FailingKeyStore` returns `Err(StoreError)`; `SlowKeyStore` sleeps then succeeds. All three are `#[cfg(test)]` in `secret_store.rs`. Add `tempfile` to `[dev-dependencies]` if absent.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p <desktop crate> device::tests`
Expected: FAIL to compile.

- [ ] **Step 4: Implement `secret_store.rs`**

```rust
//! Where the Secret Key lives on this computer: the OS keychain (Windows
//! Credential Manager, macOS Keychain, the Secret Service on Linux), or,
//! when none is available, `device.json` (see `device.rs`).
//!
//! Nothing here returns or logs the value in an error. Every keychain call
//! runs on its own thread with a timeout, because on a Linux desktop with no
//! Secret Service a D-Bus call can wait a long time, and the file fallback is
//! better than a frozen unlock screen.

const SERVICE: &str = "app.havenkeys";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub struct OsKeyStore;

impl OsKeyStore {
    /// Install the platform store as keyring-core's default. Called once at
    /// start; a failure leaves every call below failing, i.e. the file
    /// fallback.
    pub fn install() -> Self { /* platform-specific construction from Step 1, cfg-gated */ Self }

    fn with_timeout<T: Send + 'static>(f: impl FnOnce() -> Result<T, StoreError> + Send + 'static) -> Result<T, StoreError> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("keychain".into())
            .spawn(move || { let _ = tx.send(f()); })
            .map_err(|_| StoreError)?;
        rx.recv_timeout(TIMEOUT).map_err(|_| StoreError)?
    }
}

impl KeyStore for OsKeyStore {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
        Self::with_timeout(move || {
            let entry = keyring_core::Entry::new(SERVICE, &account.to_string()).map_err(|_| StoreError)?;
            match entry.get_password() {
                Ok(v) => Ok(Some(SecretString::new(v))),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(_) => Err(StoreError),
            }
        })
    }
    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
        let value = value.clone();
        Self::with_timeout(move || {
            let entry = keyring_core::Entry::new(SERVICE, &account.to_string()).map_err(|_| StoreError)?;
            entry.set_password(value.expose()).map_err(|_| StoreError)
        })
    }
    fn delete(&self, account: Uuid) -> Result<(), StoreError> {
        Self::with_timeout(move || {
            let entry = keyring_core::Entry::new(SERVICE, &account.to_string()).map_err(|_| StoreError)?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(_) => Err(StoreError),
            }
        })
    }
}
```

`StoreError` is a unit struct with a `Debug` that prints `StoreError` only.

- [ ] **Step 5: Implement `Device`**

- `OnDisk` stays `{ deviceId, secretKey? }` (file format unchanged; the file just stops holding the key when a keychain works).
- `Device { path, id, file_key: Option<SecretString>, cached: Option<(Uuid, SecretString)>, store: Box<dyn KeyStore> }`.
- `secret_key_text(account)`: return `cached` if its account matches; else `store.get(account)` → cache on `Ok(Some)`; else `file_key`. Parse-check with `SecretKey::parse` before returning.
- `set_secret_key(account, key)`: `store.set`, then `store.get` and compare (read-back); on success set `file_key = None`, save, cache, return `Keychain`; otherwise set `file_key`, save, cache, return `File`.
- `storage(account)`: `Keychain` if `store.get(account)` is `Ok(Some)`; `File` if `file_key` is set; else `None`. (Cache the answer alongside `cached` to avoid a keychain round trip per status call.)
- `migrate(account)`: if `file_key` is set, `set_secret_key` with it (parsed); nothing else.
- `forget(account)`: `store.delete(account)` (ignore error), `file_key = None`, `cached = None`, `id = Uuid::new_v4()`, save.
- Rewrite the module doc: the key lives in the OS keychain; `device.json` holds it only when no keychain answered, which Settings shows.

- [ ] **Step 6: Update callers.** Every `device.secret_key()` / `secret_key_text()` / `set_secret_key(&k)` gains the account ID:
  - `commands.rs::unlock_vault` and `change_master_password`: `account.id` (already in scope).
  - `account.rs::activate_account`: `account.id` from the invite.
  - `account.rs::sign_in`: move the stored-key lookup after `auth_params` and use `params.account_id`.
  - `account.rs::get_emergency_kit`: `account.account_id`.
  - `account.rs::device_status`: read the account from the vault; `needs_secret_key = account.is_some_and(|a| device.secret_key(a.account_id).is_none())`; add `secret_key_storage: Storage` (`None` without an account).
  - `lib.rs`: `let device = device::Device::load(&dir, Box::new(secret_store::OsKeyStore::install()));` then, if the store has an account, `device.migrate(account.account_id)`.
  - `state.rs` holds `Mutex<Device>`; the methods now take `&mut self`, so callers use `state.device.lock()` mutably (already a `MutexGuard`).

- [ ] **Step 7: UI.** `types.ts`: `secretKeyStorage: "keychain" | "file" | "none";` on `DeviceStatus`. `AccountSection`: take `secretKeyStorage` (pass from `App` via `SettingsView`, or call `api.deviceStatus()` in the section's `refresh`) and when it is `"file"` render under the Emergency Kit heading:

```tsx
<p className="group-note warn">
  Your Secret Key is stored in a file on this computer because no system keychain is available.
</p>
```

- [ ] **Step 8: Verify**

Run: `cargo test -p <desktop crate>`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `cargo audit`, `pnpm -C apps/desktop typecheck`, `pnpm -C apps/desktop test`.
Expected: clean. Then one manual check on this machine (WSL, likely no Secret Service): start the app, unlock, confirm Settings shows the file warning and the unlock is not delayed by more than the timeout.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop Cargo.lock deny.toml
git commit -m "feat(desktop): keep the Secret Key in the OS keychain, falling back to device.json"
```

---

## Part E — Remove this device

### Task 11: `remove_device` command and UI

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (`replace_store`)
- Modify: `apps/desktop/src-tauri/src/account.rs` (`remove_device`), `state.rs` (`data_dir`, `forget_sync_client`), `lib.rs` (pass `dir`, register command), `build.rs` (`COMMANDS`), `capabilities/main.json` (`allow-remove-device`)
- Modify: `apps/desktop/src/lib/api.ts` (`removeDevice`, `onRemoved`), `App.tsx`, `views/AccountSection.tsx`
- Test: `crates/havenkeys-core/tests/vault.rs`, `apps/desktop/src-tauri/src/account.rs` `mod tests`

**Interfaces:**
- Produces:
```rust
impl VaultService {
    /// Lock and swap the underlying store, returning the old one (so the
    /// caller can drop it and close the file).
    pub fn replace_store(&mut self, store: Store) -> Store;
}
#[tauri::command] pub async fn remove_device(app: AppHandle, confirmation: String) -> CmdResult<()>;
pub const REMOVED_EVENT: &str = "vault://removed";
fn set_aside(path: &Path, stamp: &str) -> std::io::Result<PathBuf>; // renames vault.sqlite3 (+ -wal, -shm)
```

- [ ] **Step 1: Failing tests.** Core (`tests/vault.rs`):

```rust
#[test]
fn replacing_the_store_locks_and_starts_empty() {
    let (mut vault, _sk, _account) = activated_unlocked_vault(); // use the file's helper
    let _old = vault.replace_store(Store::open_in_memory().unwrap());
    assert!(!vault.is_unlocked());
    assert!(!vault.status().unwrap().vault_exists);
}
```

Desktop (`account.rs` tests), for the file handling, which is the part that can lose data:

```rust
    #[test]
    fn set_aside_renames_the_vault_and_its_journal_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        for suffix in ["", "-wal", "-shm"] {
            std::fs::write(format!("{}{suffix}", path.display()), b"x").unwrap();
        }
        let moved = super::set_aside(&path, "20260923T120000Z").unwrap();
        assert!(!path.exists());
        assert!(moved.ends_with("vault.sqlite3.removed-20260923T120000Z"));
        assert!(moved.exists());
        assert!(dir.path().join("vault.sqlite3.removed-20260923T120000Z-wal").exists());
    }

    #[test]
    fn set_aside_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.sqlite3");
        std::fs::write(&path, b"x").unwrap();
        std::fs::write(dir.path().join("vault.sqlite3.removed-S"), b"older").unwrap();
        assert!(super::set_aside(&path, "S").is_err());
        assert!(path.exists());
    }

    #[test]
    fn the_confirmation_must_be_the_account_email() {
        assert!(super::confirms("User@Example.com ", "user@example.com"));
        assert!(!super::confirms("someone@example.com", "user@example.com"));
        assert!(!super::confirms("", "user@example.com"));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-core --test vault` and `cargo test -p <desktop crate> account::tests`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the core**

```rust
    pub fn replace_store(&mut self, store: Store) -> Store {
        self.lock();
        std::mem::replace(&mut self.store, store)
    }
```

- [ ] **Step 4: Implement the command** in `account.rs`:

```rust
pub const REMOVED_EVENT: &str = "vault://removed";

/// Normalized comparison, so case and surrounding spaces do not matter.
fn confirms(typed: &str, email: &str) -> bool {
    match (NormalizedEmail::parse(typed), NormalizedEmail::parse(email)) {
        (Ok(a), Ok(b)) => a.as_str() == b.as_str(),
        _ => false,
    }
}

/// Rename the vault file (and SQLite's journal files, if any) aside. Never
/// overwrites: an earlier removal's file is somebody's last copy too.
fn set_aside(path: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    let target = PathBuf::from(format!("{}.removed-{stamp}", path.display()));
    if target.exists() {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    }
    std::fs::rename(path, &target)?;
    for suffix in ["-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", path.display()));
        if from.exists() {
            std::fs::rename(&from, format!("{}{suffix}", target.display()))?;
        }
    }
    Ok(target)
}

/// Take this computer off the account and return it to first run.
///
/// The vault stays on the server. The local file is renamed, not deleted:
/// if the old server is gone, it is the only copy left, and it still opens
/// with the master password and the Emergency Kit.
#[tauri::command]
pub async fn remove_device(app: AppHandle, confirmation: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let account = state.vault()?.account()?.ok_or(havenkeys_core::Error::NoVault)?;
    if !confirms(&confirmation, &account.email) {
        return Err(havenkeys_core::Error::InvalidInput("type this account's email to confirm").into());
    }
    // Best effort: the server may be gone, which may be why this is happening.
    if let (Ok(session), Ok(client)) = (state.session(), sync::client(&state)) {
        let _ = client.revoke_device(&session, state.device_id()?).await;
    }
    state.lock(&app, "user");
    state.forget_sync_client();

    let path = state.data_dir().join(crate::VAULT_FILE);
    let stamp = chrono_free_utc_stamp(); // "YYYYMMDDTHHMMSSZ" from SystemTime; no new dependency
    {
        let mut vault = state.vault()?;
        let old = vault.replace_store(Store::open_in_memory()?);
        drop(old); // closes the connection before the rename
        set_aside(&path, &stamp).map_err(|_| CmdError::file())?;
        vault.replace_store(Store::open(&path)?);
    }
    state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .forget(account.account_id)
        .map_err(|_| CmdError::file())?;
    let _ = app.emit(REMOVED_EVENT, ());
    Ok(())
}
```

If `set_aside` fails, reopen the original store before returning (`vault.replace_store(Store::open(&path)?)`), so a failure before the rename leaves everything as it was. Write `chrono_free_utc_stamp` with `SystemTime::now().duration_since(UNIX_EPOCH)` and civil-date arithmetic (days-from-civil), or, if `time`/`chrono` is already in the desktop's dependency tree, use it. Make `VAULT_FILE` `pub(crate)` in `lib.rs`. In `state.rs`, add `data_dir: PathBuf` to `AppState` (passed from `lib.rs` setup, where `dir` already exists), `pub fn data_dir(&self) -> &Path`, and `pub fn forget_sync_client(&self)` that sets the cached client to `None`. Register the command in all four places (Global Constraints).

- [ ] **Step 5: UI.** `api.ts`:

```ts
  /** Remove this computer from the account; the vault stays on the server. */
  removeDevice: (confirmation: string) => call<void>("remove_device", { confirmation }),
  onRemoved: (handler: () => void): Promise<UnlistenFn> => listen("vault://removed", () => handler()),
```

`App.tsx`: on `onRemoved`, re-read `api.status()` and `api.deviceStatus()`, clear `lockReason`, bump `session`; with `vaultExists: false` the existing first-run screen shows. `AccountSection.tsx`: a "Danger zone" block at the end:

```tsx
<h3 className="group-title">Remove this device</h3>
<div className="group danger-zone">
  <p className="muted">
    Takes this computer off the account and returns HavenKeys to its first-run screen, where you can sign in to
    any account or server. Your vault stays on the server. A copy of the encrypted file is kept as
    vault.sqlite3.removed-… in the app's data folder; opening it later needs your master password and the Secret
    Key from your Emergency Kit.
  </p>
  <label className="field">
    <span>Type {account?.email} to confirm</span>
    <input value={confirmText} onChange={(e) => setConfirmText(e.target.value)} autoComplete="off" spellCheck={false} />
  </label>
  <button className="btn btn-danger" type="button" disabled={!confirmText || removing} onClick={remove}>
    Remove this device
  </button>
</div>
```

with `remove` calling `api.removeDevice(confirmText)` and toasting `ApiError.message` on failure. Match existing class names (grep `styles.css` for `btn-danger`, `field`, `group`); add `.danger-zone` only if a border/spacing rule is needed, using existing tokens.

- [ ] **Step 6: Verify**

Run: `cargo test --workspace` (non-Postgres), `scripts/test-server.sh`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm -C apps/desktop typecheck`, `pnpm -C apps/desktop test` (the command-surface test must pass).
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-core apps/desktop
git commit -m "feat(desktop): remove this device and return to first run, keeping the vault file aside"
```

---

## Wrap-up

### Task 12: Documentation and full verification

**Files:**
- Modify: `docs/server-sync.md` §7 (and any route list), `docs/security-review.md`, `docs/security-model.md`, `docs/threat-model.md`, `docs/crypto.md` (Secret Key storage; password-change flow), `docs/roadmap.md` §3, `docs/deployment.md` (if it lists routes), `README.md` (if it mentions `device.json` holding the key)

- [ ] **Step 1: Update docs**
  - `server-sync.md` §7: remove "The Secret Key is stored in plain text…" and "A skipped item stays stale, permanently and silently…"; add the keychain (with the `device.json` fallback and the Settings warning) and the retry (`unreadable_items`, fetch by ID, banner). Keep the vault-key-rotation bullet. Describe the credential route and session revocation where the header's revision floor is described.
  - `security-review.md`: add a finding "Master-password change left the server's verifier and KDF stale (High, fixed)" with the attack/failure scenario (every device locked out after a change; two offline changes deadlocked the header) and the fix; add "Unreadable pulled items were skipped permanently (Low, fixed)"; remove S12 or mark it "Resolved: the header is now changed server-first"; #8 stays "Accepted, documented".
  - `security-model.md` / `threat-model.md`: the credential route requires the current auth key (a stolen token cannot change the password), other sessions are revoked, the keychain's protection and its limits (any process running as the user can usually read the keychain entry on Linux/Windows; macOS may prompt), and the removed-vault file left on disk (ciphertext; needs password + Secret Key).
  - `roadmap.md` §3 "Carried forward": keep only the hostile-server bullet and #8; add "Moving items between accounts has no path; Remove this device + sign-in is the way to change server." Update §3 step 2 to say the server is deployed.
- [ ] **Step 2: Full verification**

Run, and paste the summary lines into the task report:
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/test-server.sh
cargo deny check
cargo audit
pnpm -C apps/desktop typecheck
pnpm -C apps/desktop test
```
Expected: all green. Grep the diff for accidental secret logging: `git diff main --stat` then `git diff main | grep -nE 'tracing::|println!|eprintln!|console\.(log|error)'` and confirm no new line prints a key, token, password, header or blob.

- [ ] **Step 3: Commit**

```bash
git add docs README.md
git commit -m "docs: password change, unreadable items, keychain and device removal"
```
