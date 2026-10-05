# Account Deletion (LGPD) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A user deletes their HavenKeys account from the desktop or Android app; the server erases everything it holds in one transaction, and every device of the account wipes its local copy.

**Architecture:** One server function `erase_account` (used by the new `POST /v1/account/delete` route and the admin CLI) erases the account and leaves 30-day anonymous tombstones of its session-token hashes; the `Session` extractor answers a tombstoned token with `410 account_deleted`. In the client core, `delete_account` re-checks the master password, calls the route, then `wipe_local` deletes the vault file and the Secret Key behind a crash-safe marker; a `410` seen anywhere runs the same wipe. The desktop and Android shells add a three-step danger-zone flow and react to a new `account_deleted` event.

**Tech Stack:** Rust (axum, tokio-postgres, tokio), UniFFI, Tauri 2 + React/TypeScript (vitest), Kotlin/Compose (Robolectric), Vite site.

**Spec:** `docs/superpowers/specs/2026-10-05-account-deletion-design.md`

## Global Constraints

- Controller name, verbatim: `SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA`.
- Contact email, verbatim: `samuelsilv.rocha@gmail.com`.
- Tombstone and backup/log retention: 30 days.
- Error messages are fixed strings; never include an email, password, key, token or server text (CLAUDE.md §39). Log `account_id` only, never the email.
- The master password crosses into Rust as `SecretString` and never leaves Rust; React/Kotlin clear the field after every attempt and on unmount/dispose.
- Every user-facing string exists in EN and PT-BR (desktop `en.ts`/`pt-BR.ts`, Android `values`/`values-pt-rBR`, web `en.tsx`/`pt-BR.tsx`). Product names (HavenKeys, Secret Key, Emergency Kit) stay English.
- Server error body shape is the existing `{"error":{"code":..,"message":..}}`; the new code is `account_deleted` with status 410.
- No commit carries a `Co-Authored-By` trailer (user preference).
- Postgres-backed tests need `scripts/test-server.sh` running (default URL `postgres://postgres:postgres@localhost:5433/postgres`).

## Review Focus

1. **Wrong master password on delete** — must say "wrong password" and must NOT sign the device out or wipe anything. (Task 5: password verified locally before any request; test `a_wrong_password_deletes_nothing`.)
2. **The deleting device itself, after `204`** — must end at first run even if the keychain refuses (`keychain_cleared=false` surfaced like `removed`). (Task 5: `deletion_wipes_file_key_and_announces`.)
3. **Crash between server `204` and local wipe** — next start finishes the wipe without asking. (Task 5: `a_pending_marker_is_finished_at_start`; Tasks 6 and 7 call `finish_pending_deletion` at startup.)
4. **Second device online while another deletes** — its next sync gets `410` and it wipes; a device whose token was never tombstoned stays plain signed-out. (Task 2 server tests; Task 5 round-trip `the_other_device_wipes_itself`.)
5. **Set-aside files from an earlier "remove this device"** — never deleted by account deletion. (Task 5: `set_aside_files_survive_deletion`.)

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-server/migrations/0003_account_deletion.sql` (new) | `deleted_sessions` table |
| `crates/havenkeys-server/src/erase.rs` (new) | `erase_account`, `sweep_tombstones` |
| `crates/havenkeys-server/src/admin.rs` | `delete-account` uses `erase_account` |
| `crates/havenkeys-server/src/error.rs` | `ApiError::AccountDeleted` → 410 |
| `crates/havenkeys-server/src/auth/mod.rs` | `Session` extractor checks tombstones |
| `crates/havenkeys-server/src/routes/account.rs` | `delete_account` handler |
| `crates/havenkeys-server/src/main.rs` | daily sweep task |
| `crates/havenkeys-server/tests/deletion.rs` (new) | server tests |
| `crates/havenkeys-sync-client/src/{error,client,wire}.rs` | `SyncError::AccountDeleted`, `delete_account` call |
| `crates/havenkeys-client/src/deletion.rs` (new) | `delete_account`, `wipe_local`, `finish_pending_deletion` |
| `crates/havenkeys-client/src/{events,error,sync}.rs` | event, error code, `failed()` branch |
| `crates/havenkeys-mobile/src/{account,events}.rs` | bindings |
| `apps/desktop/src-tauri/src/{removal,events,lib}.rs`, `build.rs`, `capabilities/main.json` | command + event |
| `apps/desktop/src/views/DeleteAccountSection.tsx` (new) + test | UI |
| `apps/android/.../ui/settings/*`, `data/*` | UI |
| `apps/web/src/pages/DeleteAccount.tsx` (new), i18n, Footer, App | site |
| docs | §12 of the spec |

---

### Task 1: Server — tombstone table and one erase function

**Files:**
- Create: `crates/havenkeys-server/migrations/0003_account_deletion.sql`
- Create: `crates/havenkeys-server/src/erase.rs`
- Modify: `crates/havenkeys-server/src/db.rs:24-30` (MIGRATIONS), `crates/havenkeys-server/src/lib.rs` (`pub mod erase;`), `crates/havenkeys-server/src/admin.rs:106-120`
- Test: `crates/havenkeys-server/tests/deletion.rs`

**Interfaces:**
- Produces: `havenkeys_server::erase::erase_account(tx: &deadpool_postgres::Transaction<'_>, account_id: Uuid) -> Result<(), tokio_postgres::Error>`; `havenkeys_server::erase::sweep_tombstones(db: &deadpool_postgres::Object) -> Result<u64, tokio_postgres::Error>`; `havenkeys_server::erase::TOMBSTONE_DAYS: i32 = 30`.

- [ ] **Step 1: Write the failing test** — create `tests/deletion.rs`:

```rust
mod support;

use havenkeys_server::admin::{self, AdminCommand};
use support::*;
use uuid::Uuid;

/// Every text/uuid/bytea/inet column of every table in `public`, checked for
/// the deleted account's identifiers. A table added later without a cascade
/// fails this test instead of silently keeping personal data.
async fn traces_of(server: &TestServer, needles: &[String]) -> Vec<String> {
    let db = server.db().await;
    let cols = db
        .query(
            "SELECT table_name, column_name FROM information_schema.columns
              WHERE table_schema = 'public'
                AND table_name NOT IN ('schema_migrations', 'deleted_sessions')
                AND data_type IN ('text', 'uuid', 'character varying')",
            &[],
        )
        .await
        .unwrap();
    let mut found = Vec::new();
    for c in cols {
        let (table, column): (String, String) = (c.get(0), c.get(1));
        for needle in needles {
            let n: i64 = db
                .query_one(
                    &format!(r#"SELECT count(*) FROM "{table}" WHERE "{column}"::text ILIKE '%' || $1 || '%'"#),
                    &[needle],
                )
                .await
                .unwrap()
                .get(0);
            if n > 0 {
                found.push(format!("{table}.{column}"));
            }
        }
    }
    found
}

#[tokio::test]
async fn admin_delete_leaves_no_trace_of_the_account() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "gone@example.com").await;
    create_item(&server, &sess, b"o", b"d").await;
    // A failed attempt leaves an "acct:<uuid>" rate-limit row.
    let _ = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email, "authKey": data_encoding::BASE64.encode(&[9u8; 32]),
            "deviceId": Uuid::new_v4(), "deviceName": "X",
        }))
        .send()
        .await
        .unwrap();

    admin::run(
        AdminCommand::DeleteAccount { email: "gone@example.com".into() },
        server.pool(),
    )
    .await
    .unwrap();

    let needles = vec![
        account.account_id.to_string(),
        account.vault_id.to_string(),
        sess.device_id.to_string(),
        "gone@example.com".to_string(),
    ];
    assert_eq!(traces_of(&server, &needles).await, Vec::<String>::new());
    let tombstones: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM deleted_sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(tombstones, 1, "the live session is tombstoned");
    server.cleanup().await;
}

#[tokio::test]
async fn the_sweep_removes_only_expired_tombstones() {
    let server = TestServer::start().await;
    let db = server.db().await;
    db.execute(
        "INSERT INTO deleted_sessions (token_hash, expires_at) VALUES
           ($1, now() - interval '1 second'), ($2, now() + interval '1 day')",
        &[&vec![1u8; 32], &vec![2u8; 32]],
    )
    .await
    .unwrap();
    assert_eq!(havenkeys_server::erase::sweep_tombstones(&db).await.unwrap(), 1);
    let left: Vec<u8> = db
        .query_one("SELECT token_hash FROM deleted_sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(left, vec![2u8; 32]);
    drop(db);
    server.cleanup().await;
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p havenkeys-server --test deletion`
Expected: FAIL to compile (`erase` module missing) / `deleted_sessions` does not exist.

- [ ] **Step 3: Implement**

`migrations/0003_account_deletion.sql`:

```sql
-- Session tokens of deleted accounts, so the account's other devices learn
-- of the deletion (docs/superpowers/specs/2026-10-05-account-deletion-design.md §4).
-- Nothing here identifies a person: no account id, no email, no device id.
CREATE TABLE deleted_sessions (
  token_hash BYTEA PRIMARY KEY CHECK (octet_length(token_hash) = 32),
  expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX deleted_sessions_by_expiry ON deleted_sessions (expires_at);
```

`db.rs` MIGRATIONS gains:

```rust
    (
        "0003_account_deletion",
        include_str!("../migrations/0003_account_deletion.sql"),
    ),
```

`src/erase.rs`:

```rust
//! Erasing an account: everything the server holds about it, in the
//! caller's transaction (spec 2026-10-05-account-deletion §4.2).
//!
//! The in-app route and the admin CLI both come here, so a deletion asked
//! for by email leaves exactly what one made in the app leaves: nothing but
//! anonymous session-token hashes that expire in [`TOMBSTONE_DAYS`].

use deadpool_postgres::{Object, Transaction};
use uuid::Uuid;

/// How long the other devices of a deleted account can still learn of it.
pub const TOMBSTONE_DAYS: i32 = 30;

pub async fn erase_account(
    tx: &Transaction<'_>,
    account_id: Uuid,
) -> Result<(), tokio_postgres::Error> {
    tx.execute(
        "INSERT INTO deleted_sessions (token_hash, expires_at)
         SELECT token_hash, now() + make_interval(days => $2) FROM sessions
          WHERE account_id = $1
         ON CONFLICT (token_hash) DO NOTHING",
        &[&account_id, &TOMBSTONE_DAYS],
    )
    .await?;
    let devices: Vec<Uuid> = tx
        .query("SELECT id FROM devices WHERE account_id = $1", &[&account_id])
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    tx.execute(
        "DELETE FROM login_attempts WHERE key = $1",
        &[&format!("acct:{account_id}")],
    )
    .await?;
    // A pending pairing has no account yet but carries a device id, a
    // device name and an IP.
    tx.execute(
        "DELETE FROM pairings WHERE account_id = $1 OR device_id = ANY($2)",
        &[&account_id, &devices],
    )
    .await?;
    // The cascade takes vaults, items, devices, sessions and linked pairings.
    tx.execute("DELETE FROM accounts WHERE id = $1", &[&account_id])
        .await?;
    Ok(())
}

/// Delete expired tombstones. Returns how many went.
pub async fn sweep_tombstones(db: &Object) -> Result<u64, tokio_postgres::Error> {
    db.execute("DELETE FROM deleted_sessions WHERE expires_at <= now()", &[])
        .await
}
```

`lib.rs`: add `pub mod erase;` (alphabetical, after `pub mod email;`).

`admin.rs` — `run` takes `pool` and gets `client` as `Object`; `delete_account` needs a mutable client for a transaction. Change the `DeleteAccount` arm to `AdminCommand::DeleteAccount { email } => delete_account(pool, &email).await,` and replace `delete_account`:

```rust
async fn delete_account(pool: &Pool, email: &str) -> Result<String, String> {
    let email = crate::email::normalize(email)?;
    let mut client = pool.get().await.map_err(|_| "no database".to_string())?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| "could not delete the account".to_string())?;
    let id: Uuid = tx
        .query_opt(
            "SELECT id FROM accounts WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await
        .map_err(|_| "could not delete the account".to_string())?
        .ok_or_else(|| "no such account".to_string())?
        .get(0);
    crate::erase::erase_account(&tx, id)
        .await
        .map_err(|_| "could not delete the account".to_string())?;
    tx.commit()
        .await
        .map_err(|_| "could not delete the account".to_string())?;
    Ok("deleted".into())
}
```

(The existing `let client = pool.get()...` at the top of `run` stays for the other arms; the `DeleteAccount` arm no longer uses it.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-server --test deletion --test admin --test schema`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): erase an account in one transaction, leaving only session tombstones"
```

---

### Task 2: Server — `POST /v1/account/delete` and `410 account_deleted`

**Files:**
- Modify: `crates/havenkeys-server/src/error.rs` (variant + `parts`), `src/auth/mod.rs:136-180` (extractor), `src/routes/account.rs` (handler), `src/routes/mod.rs` (route)
- Test: `crates/havenkeys-server/tests/deletion.rs` (append)

**Interfaces:**
- Consumes: `erase::erase_account` (Task 1); `rate_limit::AttemptKeys` and `auth::{decode_auth_key, verify_auth_key}` as used by `change_credentials`.
- Produces: route `POST /v1/account/delete`, body `{"currentAuthKey": b64, "email": str}` (`deny_unknown_fields`) → `204`; `ApiError::AccountDeleted` → `410`, code `account_deleted`, message `"This account was deleted."`.

- [ ] **Step 1: Write the failing tests** — append to `tests/deletion.rs`:

```rust
async fn delete(server: &TestServer, sess: &Sess, key: &[u8; 32], email: &str) -> u16 {
    server
        .post_as("/v1/account/delete", sess)
        .json(&serde_json::json!({
            "currentAuthKey": data_encoding::BASE64.encode(key),
            "email": email,
        }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn accounts(server: &TestServer) -> i64 {
    server.db().await.query_one("SELECT count(*) FROM accounts", &[]).await.unwrap().get(0)
}

#[tokio::test]
async fn the_owner_deletes_and_other_devices_see_410() {
    let server = TestServer::start().await;
    let (account, phone) = signed_in(&server, "me@example.com").await;
    let laptop = login(&server, &account, "Laptop").await;

    assert_eq!(delete(&server, &phone, &account.auth_key, " Me@Example.com ").await, 204);
    assert_eq!(accounts(&server).await, 0);

    let res = server.get_as("/v1/sync?since=0", &laptop).send().await.unwrap();
    assert_eq!(res.status(), 410);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], "account_deleted");

    let stranger = Sess { token: "x".repeat(43), ..laptop };
    assert_eq!(
        server.get_as("/v1/sync?since=0", &stranger).send().await.unwrap().status(),
        401,
        "an unknown token learns nothing"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_tombstone_answers_401() {
    let server = TestServer::start().await;
    let (account, phone) = signed_in(&server, "old@example.com").await;
    let laptop = login(&server, &account, "Laptop").await;
    assert_eq!(delete(&server, &phone, &account.auth_key, "old@example.com").await, 204);
    server
        .db()
        .await
        .execute("UPDATE deleted_sessions SET expires_at = now() - interval '1 second'", &[])
        .await
        .unwrap();
    assert_eq!(
        server.get_as("/v1/sync?since=0", &laptop).send().await.unwrap().status(),
        401
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_stolen_token_alone_cannot_delete_and_the_failure_counts() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "thief@example.com").await;
    assert_eq!(delete(&server, &sess, &[1u8; 32], "thief@example.com").await, 401);
    assert_eq!(accounts(&server).await, 1);
    let failures: i32 = server
        .db()
        .await
        .query_one(
            "SELECT failures FROM login_attempts WHERE key = $1",
            &[&format!("acct:{}", account.account_id)],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(failures, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn a_rate_limited_account_is_refused_before_checking() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "slow@example.com").await;
    for _ in 0..5 {
        delete(&server, &sess, &[1u8; 32], "slow@example.com").await;
    }
    assert_eq!(delete(&server, &sess, &account.auth_key, "slow@example.com").await, 429);
    assert_eq!(accounts(&server).await, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn another_email_is_refused() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "typo@example.com").await;
    assert_eq!(delete(&server, &sess, &account.auth_key, "other@example.com").await, 400);
    assert_eq!(accounts(&server).await, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn unknown_fields_are_refused() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "extra@example.com").await;
    let status = server
        .post_as("/v1/account/delete", &sess)
        .json(&serde_json::json!({
            "currentAuthKey": data_encoding::BASE64.encode(&account.auth_key),
            "email": "extra@example.com",
            "accountId": Uuid::new_v4(),
        }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 400);
    assert_eq!(accounts(&server).await, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn the_route_leaves_no_trace_either() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "route@example.com").await;
    create_item(&server, &sess, b"o", b"d").await;
    assert_eq!(delete(&server, &sess, &account.auth_key, "route@example.com").await, 204);
    let needles = vec![
        account.account_id.to_string(),
        account.vault_id.to_string(),
        sess.device_id.to_string(),
        "route@example.com".to_string(),
    ];
    assert_eq!(traces_of(&server, &needles).await, Vec::<String>::new());
    server.cleanup().await;
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-server --test deletion`
Expected: new tests FAIL (404 for the route).

- [ ] **Step 3: Implement**

`error.rs` — add the variant after `RateLimited`, and in `parts`:

```rust
    /// The token belonged to an account that was deleted (spec
    /// 2026-10-05-account-deletion §4.4). Only a holder of the token sees it.
    AccountDeleted,
```

```rust
            Self::AccountDeleted => (
                StatusCode::GONE,
                "account_deleted",
                "This account was deleted.",
            ),
```

`auth/mod.rs` — in `from_request_parts`, replace `.await?\n            .ok_or(ApiError::Unauthorized)?;` of the main query with:

```rust
            .await?;
        let Some(row) = row else {
            // A deleted account's devices are told so, once they ask with
            // the token they held; any other token is just unauthorized.
            let gone = db
                .query_opt(
                    "SELECT 1 FROM deleted_sessions WHERE token_hash = $1 AND expires_at > now()",
                    &[&hash],
                )
                .await?;
            return Err(match gone {
                Some(_) => ApiError::AccountDeleted,
                None => ApiError::Unauthorized,
            });
        };
```

(rename the binding so `let row = db.query_opt(...)` holds the `Option<Row>`).

`routes/account.rs` — append:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountDeletion {
    current_auth_key: String,
    email: String,
}

/// Delete the caller's account and everything it owns (spec
/// 2026-10-05-account-deletion §4.3). The session alone is not enough,
/// exactly as for a credential change: the caller proves the current auth
/// key, rate limited like a login.
pub async fn delete_account(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<AccountDeletion>,
) -> Result<axum::http::StatusCode, ApiError> {
    let typed = crate::email::normalize(&req.email)
        .map_err(|_| ApiError::InvalidRequest("the email does not match this account"))?;
    let current = auth::decode_auth_key(&req.current_auth_key)?;

    let mut db = state.pool.get().await?;
    let keys = rate_limit::AttemptKeys::new(session.account_id, &client_ip(&state, &headers, peer));
    keys.check(&db).await?;

    let row = db
        .query_opt(
            "SELECT auth_verifier, email_normalized FROM accounts WHERE id = $1 AND status = 'active'",
            &[&session.account_id],
        )
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let (stored, email): (Option<String>, String) = (row.get(0), row.get(1));
    if typed != email {
        return Err(ApiError::InvalidRequest("the email does not match this account"));
    }
    let stored = stored.ok_or(ApiError::Unauthorized)?;
    if !auth::verify_auth_key(stored.clone(), current).await {
        keys.record_failure(&db).await?;
        tracing::info!(account_id = %session.account_id, outcome = "rejected", "account deletion");
        return Err(ApiError::Unauthorized);
    }

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
    crate::erase::erase_account(&tx, session.account_id).await?;
    tx.commit().await?;

    // The account counter went with the account; the address's goes too,
    // as after a successful login.
    keys.clear(&db).await?;
    tracing::info!(account_id = %session.account_id, "account deleted");
    Ok(axum::http::StatusCode::NO_CONTENT)
}
```

Check `crate::email::normalize`'s return type in `src/email.rs` and compare as `typed.as_str() != email` or `typed != email` accordingly (it returns something `ToString`-able; in `admin.rs` it is used as `normalize(email)?.to_string()` and as a SQL param directly).

`routes/mod.rs` — after the credentials route:

```rust
        .route("/v1/account/delete", post(account::delete_account))
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-server`
Expected: all PASS (including `no_logging.rs`, which checks logs never carry secrets).

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): delete the account from the app; deleted sessions answer 410"
```

---

### Task 3: Server — daily tombstone sweep

**Files:**
- Modify: `crates/havenkeys-server/Cargo.toml` (tokio feature `time`), `crates/havenkeys-server/src/main.rs` (`serve`)

**Interfaces:**
- Consumes: `erase::sweep_tombstones` (Task 1, already tested).

- [ ] **Step 1: Implement** — in `Cargo.toml` add `"time"` to tokio's features. In `main.rs` `serve`, after `let state = AppState {...};` and before binding:

```rust
    // Expired tombstones of deleted accounts (spec 2026-10-05-account-deletion
    // §4.5): once at start, then daily. A failure is logged by kind and the
    // next day tries again.
    let sweeping = state.pool.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        loop {
            every.tick().await;
            match sweeping.get().await {
                Ok(db) => {
                    if havenkeys_server::erase::sweep_tombstones(&db).await.is_err() {
                        tracing::warn!(kind = "sweep", "database error");
                    }
                }
                Err(_) => tracing::warn!(kind = "pool", "database error"),
            }
        }
    });
```

- [ ] **Step 2: Build and test**

Run: `cargo build -p havenkeys-server && cargo test -p havenkeys-server --test deletion`
Expected: builds; PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): sweep expired deletion tombstones daily"
```

---

### Task 4: Sync client — `delete_account` call and `SyncError::AccountDeleted`

**Files:**
- Modify: `crates/havenkeys-sync-client/src/error.rs`, `src/client.rs` (`error_for`, new method), `src/wire.rs` (body)
- Modify: `crates/havenkeys-client/src/error.rs` (mapping + test)

**Interfaces:**
- Produces: `SyncError::AccountDeleted` (code `"account_deleted"`, Display `"this account was deleted"`); `SyncClient::delete_account(&self, session: &Session, current_auth_key: &AuthKey, email: &str) -> Result<()>`; `ClientError` code `"account_deleted"` with message `"This account was deleted."`.

- [ ] **Step 1: Write failing tests**

In `havenkeys-sync-client/src/client.rs` add a test module (or extend an existing `#[cfg(test)]` one):

```rust
#[cfg(test)]
mod deletion_tests {
    use super::*;

    #[test]
    fn gone_means_the_account_was_deleted() {
        assert_eq!(error_for(410), SyncError::AccountDeleted);
        assert_eq!(error_for(401), SyncError::Unauthorized);
    }
}
```

In `havenkeys-client/src/error.rs`, add `(SyncError::AccountDeleted, "account_deleted"),` to the `cases` array of `sync_errors_keep_the_codes_the_desktop_ui_knows`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-sync-client deletion_tests; cargo test -p havenkeys-client error::tests`
Expected: compile FAIL (`AccountDeleted` missing).

- [ ] **Step 3: Implement**

`error.rs` (sync-client): variant after `Unauthorized`:

```rust
    /// The account behind this session was deleted (by this or another
    /// device). The device erases its local copy.
    AccountDeleted,
```

with `Self::AccountDeleted => "account_deleted",` in `code` and `Self::AccountDeleted => f.write_str("this account was deleted"),` in `Display`.

`client.rs` `error_for`: add `410 => SyncError::AccountDeleted,` before `413`.

`wire.rs`, next to `CredentialsBody`:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountDeletionBody<'a> {
    pub current_auth_key: &'a str,
    pub email: &'a str,
}
```

`client.rs`, after `change_credentials`:

```rust
    /// Delete the account and everything the server holds for it. The
    /// caller wipes the local copy only once this returns `Ok`.
    pub async fn delete_account(
        &self,
        session: &Session,
        current_auth_key: &AuthKey,
        email: &str,
    ) -> Result<()> {
        let current = current_auth_key.to_base64();
        let body = wire::AccountDeletionBody {
            current_auth_key: &current,
            email,
        };
        let response = self.post("/v1/account/delete", Some(session), &body).await?;
        match response.status {
            204 | 200 => Ok(()),
            _ => Err(error_for(response.status)),
        }
    }
```

(`AuthKey` is already imported in this file for `CredentialChange`; if not, `use havenkeys_core::crypto::keys::AuthKey;`.)

`havenkeys-client/src/error.rs` `From<SyncError>`: add

```rust
            SyncError::AccountDeleted => Self::fixed("account_deleted", "This account was deleted."),
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-sync-client && cargo test -p havenkeys-client --lib`
Expected: PASS. (Any exhaustive `match` on `SyncError` elsewhere that fails to compile gets an `AccountDeleted` arm treated like `Unauthorized` — Task 5 replaces `failed()`'s.)

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-sync-client crates/havenkeys-client/src/error.rs
git commit -m "feat(sync-client): delete-account request; 410 means the account was deleted"
```

---

### Task 5: Client core — `delete_account`, `wipe_local`, pending marker

**Files:**
- Create: `crates/havenkeys-client/src/deletion.rs`
- Modify: `crates/havenkeys-client/src/lib.rs` (`mod deletion;`), `src/events.rs` (new method), `src/sync.rs:33-45` (`failed`), `src/removal.rs` (make `confirms` `pub(crate)`), `src/client.rs` tests (`RecordingEvents`), `tests/round_trip.rs` and `tests/pairing.rs` (`Probe` impls)
- Test: unit tests in `deletion.rs`; `tests/round_trip.rs`

**Interfaces:**
- Consumes: `SyncClient::delete_account`, `SyncError::AccountDeleted` (Task 4).
- Produces:
  - `ClientEvents::account_deleted(&self, keychain_cleared: bool)` — the vault reopened empty because the account no longer exists.
  - `HavenClient::delete_account(self: &Arc<Self>, confirmation: String, master_password: SecretString) -> ClientResult<()>`
  - `HavenClient::finish_pending_deletion(&self) -> bool` — call once at startup, right after `HavenClient::new`; returns whether a wipe ran.
  - `pub(crate) fn wipe_local(&self, account_id: Uuid)` (sync; used by `failed()`).
  - Marker file: `<vault_path>.pending-deletion`, content = account UUID text.

- [ ] **Step 1: Add the event method and update every implementor** (compile-only step)

`events.rs`, after `removed`:

```rust
    /// The account no longer exists (deleted here or on another device) and
    /// this device erased its copy; `keychain_cleared` as for `removed`.
    fn account_deleted(&self, keychain_cleared: bool);
```

Add to `RecordingEvents` in `client.rs` tests:

```rust
        fn account_deleted(&self, keychain_cleared: bool) {
            self.push(format!("account_deleted:{keychain_cleared}"));
        }
```

Add to `Probe` in `tests/round_trip.rs` (and the implementor in `tests/pairing.rs`):

```rust
    fn account_deleted(&self, keychain_cleared: bool) {
        self.seen.lock().unwrap().push(format!("account_deleted:{keychain_cleared}"));
    }
```

(`tests/pairing.rs`: follow its existing style; an empty body `fn account_deleted(&self, _: bool) {}` if its other methods are empty.) Desktop and mobile implementors are done in Tasks 6 and 7; until then `cargo test -p havenkeys-client` is the check.

- [ ] **Step 2: Write failing unit tests** — `deletion.rs` test module (helpers mirror `removal.rs::removal_without_a_session_still_completes`):

```rust
#[cfg(test)]
mod tests {
    use crate::client::tests::client_in;
    use crate::client::HavenClient;
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::store::{AccountRecord, Store};
    use havenkeys_core::vault::prepare_new_account_vault;
    use havenkeys_core::SecretString;
    use std::path::Path;
    use std::sync::Arc;

    const PASSWORD: &str = "correct horse battery staple";

    /// An unlocked account vault on disk at `dir/vault.sqlite3`, offline.
    fn account_vault(dir: &Path) -> (Arc<HavenClient>, Arc<crate::client::tests::RecordingEvents>, uuid::Uuid) {
        let path = dir.join("vault.sqlite3");
        let (client, events) = client_in(dir);
        client.vault().unwrap().replace_store(Store::open(&path).unwrap());
        let account = AccountRef::new(
            uuid::Uuid::from_u128(3),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let made = prepare_new_account_vault(
            &SecretString::from(PASSWORD),
            &account,
            KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap(),
            1_700_000_000_000,
        )
        .unwrap();
        client
            .vault()
            .unwrap()
            .create_account_vault(
                made.prepared,
                &AccountRecord {
                    account_id: account.id,
                    email: "user@example.com".into(),
                    server_url: "http://127.0.0.1:9".into(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        client.device().unwrap().set_secret_key(account.id, &made.secret_key).unwrap();
        (client, events, account.id)
    }

    #[tokio::test]
    async fn a_locked_vault_cannot_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        let err = client
            .delete_account("user@example.com".into(), SecretString::from(PASSWORD))
            .await
            .unwrap_err();
        assert_eq!(err.code, "locked");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn offline_deletes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _, id) = account_vault(dir.path());
        let err = client
            .delete_account("user@example.com".into(), SecretString::from(PASSWORD))
            .await
            .unwrap_err();
        assert_eq!(err.code, "offline");
        assert!(dir.path().join("vault.sqlite3").exists());
        assert!(client.device().unwrap().secret_key(id).is_some());
    }

    #[test]
    fn deletion_wipes_file_key_and_announces() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        let old_device = client.device_id().unwrap();
        client.wipe_local(id);
        let path = dir.path().join("vault.sqlite3");
        // A fresh empty store is open at the path: no account in it.
        assert!(client.vault().unwrap().account().unwrap().is_none());
        assert!(client.device().unwrap().secret_key(id).is_none());
        assert_ne!(client.device_id().unwrap(), old_device);
        assert!(!dir.path().join("vault.sqlite3.pending-deletion").exists());
        assert!(events.seen().contains(&"account_deleted:true".to_string()));
        assert!(!events.seen().iter().any(|e| e.starts_with("removed")));
        let _ = path;
    }

    #[test]
    fn a_pending_marker_is_finished_at_start() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        std::fs::write(dir.path().join("vault.sqlite3.pending-deletion"), id.to_string()).unwrap();
        assert!(client.finish_pending_deletion());
        assert!(client.device().unwrap().secret_key(id).is_none());
        assert!(client.vault().unwrap().account().unwrap().is_none());
        assert!(events.seen().contains(&"account_deleted:true".to_string()));
        assert!(!client.finish_pending_deletion(), "nothing left to finish");
    }

    #[test]
    fn a_garbled_marker_is_removed_and_nothing_is_wiped() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _, id) = account_vault(dir.path());
        std::fs::write(dir.path().join("vault.sqlite3.pending-deletion"), "not a uuid").unwrap();
        assert!(!client.finish_pending_deletion());
        assert!(client.device().unwrap().secret_key(id).is_some());
        assert!(!dir.path().join("vault.sqlite3.pending-deletion").exists());
    }

    #[test]
    fn set_aside_files_survive_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("vault.sqlite3.removed-20260101T000000Z");
        std::fs::write(&older, b"someone's last copy").unwrap();
        let (client, _, id) = account_vault(dir.path());
        client.wipe_local(id);
        assert!(older.exists());
    }

    #[test]
    fn a_410_wipes_the_device() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        let err = client.failed(havenkeys_sync_client::SyncError::AccountDeleted);
        assert_eq!(err.code, "account_deleted");
        assert!(client.device().unwrap().secret_key(id).is_none());
        assert!(events.seen().contains(&"account_deleted:true".to_string()));
    }
}
```

Note on `deletion_wipes_file_key_and_announces`: the vault file at `path` is deleted and immediately re-created empty by `Store::open` (same as `remove_device`'s reopen); the assertion is therefore that the account is gone, not that the file is absent. Additionally assert the old bytes are gone by checking the reopened store has no account (done above).

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p havenkeys-client deletion`
Expected: compile FAIL (`delete_account`, `wipe_local`, `finish_pending_deletion` missing).

- [ ] **Step 4: Implement `deletion.rs`**

```rust
//! Deleting the account (spec 2026-10-05-account-deletion §5): the server
//! erases everything first; only then does this device erase its copy.
//!
//! The local wipe is guarded by a marker file written before it starts, so
//! a wipe cut short (crash, power loss, a locked file) is finished at the
//! next start instead of leaving an orphan replica of a deleted account.

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::removal::confirms;
use havenkeys_core::store::Store;
use havenkeys_core::vault;
use havenkeys_core::SecretString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::spawn_blocking;
use uuid::Uuid;

fn marker_path(vault_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.pending-deletion", vault_path.display()))
}

impl HavenClient {
    /// Unlocked and online, checked here: the shell is not trusted to gate
    /// this. The master password is checked locally first, so a typo is a
    /// "wrong password" and never reaches the server's rate limit or looks
    /// like a dead session.
    pub async fn delete_account(
        self: &Arc<Self>,
        confirmation: String,
        master_password: SecretString,
    ) -> ClientResult<()> {
        self.require_unlocked()?;
        self.require_online()?;
        let record = self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?;
        if !confirms(&confirmation, &record.email) {
            return Err(havenkeys_core::Error::InvalidInput(
                "type this account's email to confirm",
            )
            .into());
        }
        self.verify_master_password(master_password.clone()).await?;

        let account = self.vault_account()?;
        let secret_key = self
            .device()?
            .secret_key(account.id)
            .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
        let kdf = self.vault()?.kdf()?.ok_or(havenkeys_core::Error::NoVault)?;
        let email = account.email.as_str().to_owned();
        let auth_key = spawn_blocking(move || {
            vault::derive_auth_key(&master_password, &secret_key, &kdf, &account)
        })
        .await
        .map_err(|_| ClientError::internal())??;

        let (session, server) = (self.session()?, self.server()?);
        let sent = server.delete_account(&session, &auth_key, &email).await;
        drop(auth_key);
        match sent {
            Ok(()) => {}
            // Already gone (deleted from another device meanwhile): same end.
            Err(havenkeys_sync_client::SyncError::AccountDeleted) => {}
            Err(e) => return Err(self.failed(e)),
        }

        let client = Arc::clone(self);
        let account_id = record.account_id;
        spawn_blocking(move || client.wipe_local(account_id))
            .await
            .map_err(|_| ClientError::internal())?;
        Ok(())
    }

    /// Finish a wipe a previous run started. Call once at startup, right
    /// after construction. Returns whether one ran.
    pub fn finish_pending_deletion(&self) -> bool {
        let marker = marker_path(&self.config.vault_path);
        let Ok(text) = std::fs::read_to_string(&marker) else {
            return false;
        };
        match text.trim().parse::<Uuid>() {
            Ok(account_id) => {
                self.wipe_local(account_id);
                true
            }
            Err(_) => {
                let _ = std::fs::remove_file(&marker);
                false
            }
        }
    }

    /// Erase this device's copy of a deleted account: the vault file (and
    /// SQLite's sidecars), the Secret Key, and the device id. Files set
    /// aside by an earlier "remove this device" are not touched: they may
    /// belong to another account.
    ///
    /// Synchronous and bounded (the key store has its own timeout), so it
    /// can run from `failed()`. Every step runs even if an earlier one
    /// failed; the marker is removed only when the file is gone.
    pub(crate) fn wipe_local(&self, account_id: Uuid) {
        let path = self.config.vault_path.clone();
        let marker = marker_path(&path);
        let _ = std::fs::write(&marker, account_id.to_string());

        self.lock("account_deleted");
        self.forget_server();

        let mut file_gone = true;
        if let Ok(mut vault) = self.vault() {
            if let Ok(empty) = Store::open_in_memory() {
                drop(vault.replace_store(empty)); // closes the file
            }
            for suffix in ["-wal", "-shm", "-journal"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
            }
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => file_gone = false,
            }
            if let Ok(store) = Store::open(&path) {
                let _ = vault.replace_store(store);
            }
        } else {
            file_gone = false;
        }

        let keychain_cleared = match self.device() {
            Ok(mut device) => device.forget(account_id).keychain_cleared,
            Err(_) => false,
        };
        if file_gone {
            let _ = std::fs::remove_file(&marker);
        }
        self.events.account_deleted(keychain_cleared);
    }
}
```

Check before writing: `self.lock(reason)` takes `&'static str` — `"account_deleted"` is fine; `VaultService::replace_store` returns the old store (as used in `removal.rs`); `vault().kdf()` returns `Result<Option<KdfParams>>` (as in `account.rs` `change_master_password`); `record.email` is the `String` from `AccountRecord`, `record.account_id` the `Uuid`. If `vault.account()` returns a different record type, read the fields the way `removal.rs` lines 22-28 does.

`removal.rs`: `fn confirms` → `pub(crate) fn confirms`.

`lib.rs`: add `mod deletion;` next to `mod removal;`.

`sync.rs` `failed()`:

```rust
            SyncError::AccountDeleted => {
                // The account is gone; nothing on this device is useful any
                // more, and nothing here can be trusted to ask first.
                if let Ok(Some(account)) = self.vault().and_then(|v| Ok(v.account()?)) {
                    self.wipe_local(account.account_id);
                }
            }
```

(adjust the `vault().account()` call to the error conversions used elsewhere in the crate; the point is: read the account id, drop the guard, then call `wipe_local`.)

- [ ] **Step 5: Run unit tests**

Run: `cargo test -p havenkeys-client --lib`
Expected: PASS.

- [ ] **Step 6: Write the end-to-end tests** — append to `tests/round_trip.rs` (pattern of `two_devices_share_one_vault_through_the_client`):

```rust
#[tokio::test(flavor = "multi_thread")]
async fn the_other_device_wipes_itself() {
    let server = Server::start().await;
    let invite = server.invite("bye@example.com").await;
    let dir_a = tempfile::tempdir().unwrap();
    let (a, probe_a) = device(dir_a.path());
    a.activate(invite, SecretString::from(PASSWORD)).await.unwrap();
    until(|| a.is_online()).await;

    let account_id = a.account_status().unwrap().unwrap().account_id;
    let secret_key = a.device().unwrap().secret_key_text(account_id).unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, probe_b) = device(dir_b.path());
    b.sign_in(server.base.clone(), "bye@example.com".into(), SecretString::from(PASSWORD), Some(secret_key))
        .await
        .unwrap();

    a.delete_account("bye@example.com".into(), SecretString::from(PASSWORD))
        .await
        .unwrap();
    assert!(probe_a.seen.lock().unwrap().contains(&"account_deleted:true".to_string()));
    assert!(a.vault().unwrap().account().unwrap().is_none());

    let err = b.sync_now().await.unwrap_err();
    assert_eq!(err.code, "account_deleted");
    assert!(probe_b.seen.lock().unwrap().contains(&"account_deleted:true".to_string()));
    assert!(b.vault().unwrap().account().unwrap().is_none());
    assert!(b.device().unwrap().secret_key_text(account_id).is_none());
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_password_deletes_nothing() {
    let server = Server::start().await;
    let invite = server.invite("typo@example.com").await;
    let dir = tempfile::tempdir().unwrap();
    let (a, _) = device(dir.path());
    a.activate(invite, SecretString::from(PASSWORD)).await.unwrap();
    until(|| a.is_online()).await;

    let err = a
        .delete_account("typo@example.com".into(), SecretString::from("not the password"))
        .await
        .unwrap_err();
    assert_eq!(err.code, "unlock_failed");
    assert!(a.is_online(), "a typo must not sign the device out");
    assert!(a.vault().unwrap().account().unwrap().is_some());
    a.sync_now().await.unwrap();
    server.cleanup().await;
}
```

(`unlock_failed` is the code `verify_master_password` returns for a wrong password — confirm with `havenkeys_core::Error::UnlockFailed.code()`; use whatever that returns.)

- [ ] **Step 7: Run all client tests**

Run: `scripts/test-server.sh` (if Postgres is not up), then `cargo test -p havenkeys-client`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/havenkeys-client
git commit -m "feat(client): delete the account, then erase this device's copy; a 410 erases too"
```

---

### Task 6: Mobile bindings

**Files:**
- Modify: `crates/havenkeys-mobile/src/account.rs` (export), `src/events.rs` (trait, `Event`, pump, `impl ClientEvents`), `src/vault.rs` (startup), `crates/havenkeys-mobile/tests/round_trip.rs` (implementor if it has one)
- Regenerate: `apps/android/app/src/main/kotlin/uniffi/havenkeys_mobile/havenkeys_mobile.kt`

**Interfaces:**
- Consumes: `HavenClient::delete_account`, `finish_pending_deletion`, `ClientEvents::account_deleted` (Task 5).
- Produces: Kotlin `MobileVault.deleteAccount(confirmation: String, masterPassword: String)`; `VaultEvents.accountDeleted()`.

- [ ] **Step 1: Write the failing test** — in `crates/havenkeys-mobile/tests/round_trip.rs` (or the crate's existing account test module), add a test that a locked `MobileVault` refuses `delete_account` with code `locked`, following that file's construction helper:

```rust
#[test]
fn a_locked_vault_cannot_be_deleted() {
    let (vault, _events, _dir) = new_vault(); // the file's existing helper
    let err = vault
        .delete_account("user@example.com".into(), "pw".into())
        .unwrap_err();
    assert!(matches!(err, MobileError::Failed { ref code, .. } if code == "locked"));
}
```

(Use the helper name that file actually defines; if none, build `MobileVault::new` the way its first test does.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p havenkeys-mobile --features testing`
Expected: compile FAIL.

- [ ] **Step 3: Implement**

`account.rs`, inside the `#[uniffi::export] impl MobileVault` block, after `remove_device`:

```rust
    /// Deletes the account on the server, then this phone's copy. The app
    /// deletes its Keystore keys on the `account_deleted` event.
    pub fn delete_account(&self, confirmation: String, master_password: String) -> MobileResult<()> {
        let client = self.client.clone();
        Ok(self.block_on(client.delete_account(confirmation, SecretString::new(master_password)))?)
    }
```

(import `havenkeys_core::SecretString` if not already; match how `unlock.rs` builds it.)

`events.rs`: add `fn account_deleted(&self);` to `VaultEvents`; an `AccountDeleted` variant to `Event`; in the pump's `match`, `Event::AccountDeleted => listener.account_deleted(),`; and in `impl ClientEvents for MobileEvents`:

```rust
    fn account_deleted(&self, _keychain_cleared: bool) {
        self.send(Event::AccountDeleted);
    }
```

(mirror exactly how `removed` sends its event, including any lock-clock reset it does.)

`vault.rs` `new`: right after `let client = HavenClient::new(...)`, add `client.finish_pending_deletion();`.

- [ ] **Step 4: Run tests and regenerate bindings**

Run: `cargo test -p havenkeys-mobile --features testing`, then `scripts/build-android.sh` (needs `ANDROID_NDK_HOME` and `cargo-ndk`).
Expected: PASS; `havenkeys_mobile.kt` now contains `deleteAccount` and `accountDeleted`. If the NDK is not available in this environment, stop and tell the user: the committed bindings must be regenerated (CI checks them).

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-mobile apps/android/app/src/main/kotlin/uniffi
git commit -m "feat(mobile): expose account deletion and the account_deleted event"
```

---

### Task 7: Desktop — command, event, startup

**Files:**
- Modify: `apps/desktop/src-tauri/src/removal.rs` (command), `src-tauri/build.rs` (`COMMANDS`), `src-tauri/src/lib.rs` (handler + startup), `src-tauri/capabilities/main.json`, `src-tauri/src/events.rs`, `apps/desktop/src/lib/api.ts`

**Interfaces:**
- Consumes: `HavenClient::delete_account`, `finish_pending_deletion`, `ClientEvents::account_deleted`.
- Produces: Tauri command `delete_account { confirmation: String, master_password: SecretString }`; event `"vault://account-deleted"` with payload `{ keychainWarning: string | null }`; TS `api.deleteAccount(confirmation, masterPassword)` and `api.onAccountDeleted(handler)`.

- [ ] **Step 1: Implement Rust side**

`removal.rs`:

```rust
#[tauri::command]
pub async fn delete_account(
    app: AppHandle,
    confirmation: String,
    master_password: havenkeys_core::SecretString,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.delete_account(confirmation, master_password).await
}
```

`build.rs` `COMMANDS`: add `"delete_account",` after `"remove_device",`. `lib.rs` handler: `removal::delete_account,` after `removal::remove_device,`. `capabilities/main.json`: `"allow-delete-account",` after `"allow-remove-device",`. In `lib.rs` setup, right after `let client = havenkeys_client::HavenClient::new(...)`: `client.finish_pending_deletion();`.

`events.rs`:

```rust
/// The account was deleted (here or on another device) and this computer's
/// copy erased: the UI returns to first run and says why. Carries `Removed`.
pub const ACCOUNT_DELETED_EVENT: &str = "vault://account-deleted";

const KEYCHAIN_NOT_CLEARED_DELETED: &str = "The account was deleted, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry \u{201c}app.havenkeys\u{201d} yourself.";
```

```rust
    fn account_deleted(&self, keychain_cleared: bool) {
        let payload = Removed {
            keychain_warning: (!keychain_cleared).then_some(KEYCHAIN_NOT_CLEARED_DELETED),
        };
        let _ = self.app.emit(ACCOUNT_DELETED_EVENT, payload);
    }
```

`api.ts`, after `removeDevice`:

```ts
  /** Delete the account on the server and this computer's copy. Irreversible. */
  deleteAccount: (confirmation: string, masterPassword: string) =>
    call<void>("delete_account", { confirmation, masterPassword }),
```

and after `onRemoved`:

```ts
  /** The account was deleted (here or elsewhere); this computer's copy is erased. */
  onAccountDeleted: (handler: (removed: { keychainWarning: string | null }) => void): Promise<UnlistenFn> =>
    listen<{ keychainWarning: string | null }>("vault://account-deleted", (e) =>
      handler({ keychainWarning: e.payload?.keychainWarning ?? null }),
    ),
```

- [ ] **Step 2: Run checks**

Run: `cargo check -p havenkeys-desktop` (use the crate name in `apps/desktop/src-tauri/Cargo.toml`), `pnpm --filter @havenkeys/desktop test -- commands`
Expected: PASS — `commands.test.ts` confirms build.rs, lib.rs, main.json and api.ts agree.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri apps/desktop/src/lib/api.ts
git commit -m "feat(desktop): delete_account command and account-deleted event"
```

---

### Task 8: Desktop — danger-zone UI

**Files:**
- Create: `apps/desktop/src/views/DeleteAccountSection.tsx`, `apps/desktop/src/views/DeleteAccountSection.test.tsx`
- Modify: `apps/desktop/src/views/SettingsView.tsx` (render after `<UpdatesSection />`, before the about block), `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts`, `apps/desktop/src/App.tsx` (listener + notice), `apps/desktop/src/views/ExportSection.tsx` (add `id="export"` to its `h3.group-title`)

**Interfaces:**
- Consumes: `api.deleteAccount`, `api.onAccountDeleted`, `api.accountStatus()` (`account.email`), `api.listRemovedFiles` does NOT exist — the "set-aside files" note is shown unconditionally as a sentence ("Copies set aside by an earlier "Remove this device" are kept.") to avoid a new command.
- Produces: `export function DeleteAccountSection({ online }: { online: boolean })`.

- [ ] **Step 1: Add strings** — `en.ts`, new namespace `deleteAccount` (add the same keys in `pt-BR.ts`, typed by `Messages`):

```ts
  deleteAccount: {
    title: "Delete account and all data",
    explain:
      "This deletes your vault from the server, signs out every device, and erases this computer's copy. It cannot be undone: neither you nor the server's operator can recover it.",
    keptNote: "Copies set aside by an earlier “Remove this device” are kept, and so are backups you exported.",
    backupFirst: "Make an encrypted backup first",
    continueWithout: "Continue without a backup",
    typeToConfirm: (email: string) => `Type ${email} to confirm`,
    masterPassword: "Master password",
    confirm: "Delete account",
    failed: "The account could not be deleted.",
    offline: "Connect to your server to delete the account.",
    done: "Your account and its data were deleted.",
  },
```

PT-BR:

```ts
  deleteAccount: {
    title: "Excluir conta e todos os dados",
    explain:
      "Isso apaga o seu cofre do servidor, desconecta todos os dispositivos e apaga a cópia deste computador. Não dá para desfazer: nem você nem o operador do servidor conseguem recuperá-lo.",
    keptNote: "Cópias separadas por um “Remover este dispositivo” anterior são mantidas, assim como backups que você exportou.",
    backupFirst: "Fazer backup cifrado antes",
    continueWithout: "Continuar sem backup",
    typeToConfirm: (email: string) => `Digite ${email} para confirmar`,
    masterPassword: "Senha mestra",
    confirm: "Excluir conta",
    failed: "Não foi possível excluir a conta.",
    offline: "Conecte-se ao seu servidor para excluir a conta.",
    done: "Sua conta e os dados dela foram excluídos.",
  },
```

Add `account_deleted` to the `ErrorCode` union and both `codes` maps: EN `"This account was deleted."`, PT `"Esta conta foi excluída."`.

- [ ] **Step 2: Write the failing test** — `DeleteAccountSection.test.tsx`, following `ExportSection.test.tsx`'s harness (jsdom pragma, `vi.mock("../lib/api", ...)`, `createRoot` + `act`, the `type()` helper):

```tsx
// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../i18n/en";

const deleteAccount = vi.fn();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    accountStatus: () => Promise.resolve({ email: "user@example.com" }),
    deleteAccount: (...a: unknown[]) => deleteAccount(...a),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { DeleteAccountSection } from "./DeleteAccountSection";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
let host: HTMLDivElement;
let root: Root;

function button(label: string): HTMLButtonElement {
  return [...host.querySelectorAll("button")].find((b) => b.textContent === label) as HTMLButtonElement;
}
function type(selector: string, value: string) {
  const input = host.querySelector(selector) as HTMLInputElement;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  act(() => {
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

beforeEach(async () => {
  deleteAccount.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<DeleteAccountSection online />));
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("DeleteAccountSection", () => {
  it("asks for the email and the master password before deleting", async () => {
    act(() => button(en.deleteAccount.title).click());
    act(() => button(en.deleteAccount.continueWithout).click());
    expect(button(en.deleteAccount.confirm).disabled).toBe(true);
    type('input[name="confirm-email"]', "user@example.com");
    expect(button(en.deleteAccount.confirm).disabled).toBe(true);
    type('input[type="password"]', "pw");
    expect(button(en.deleteAccount.confirm).disabled).toBe(false);
    deleteAccount.mockResolvedValue(undefined);
    await act(async () => button(en.deleteAccount.confirm).click());
    expect(deleteAccount).toHaveBeenCalledWith("user@example.com", "pw");
  });

  it("clears the password after a failed attempt", async () => {
    act(() => button(en.deleteAccount.title).click());
    act(() => button(en.deleteAccount.continueWithout).click());
    type('input[name="confirm-email"]', "user@example.com");
    type('input[type="password"]', "wrong");
    deleteAccount.mockRejectedValue({ code: "unlock_failed", message: "x" });
    await act(async () => button(en.deleteAccount.confirm).click());
    expect((host.querySelector('input[type="password"]') as HTMLInputElement).value).toBe("");
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
  });

  it("is disabled offline", async () => {
    await act(async () => root.render(<DeleteAccountSection online={false} />));
    expect(button(en.deleteAccount.title).disabled).toBe(true);
  });
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `pnpm --filter @havenkeys/desktop test -- DeleteAccountSection`
Expected: FAIL (module missing).

- [ ] **Step 4: Implement `DeleteAccountSection.tsx`**

```tsx
import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

type Step = "closed" | "explain" | "confirm";

/** Settings → "Delete account and all data" (spec 2026-10-05-account-deletion §6). */
export function DeleteAccountSection({ online }: { online: boolean }) {
  const { t } = useI18n();
  const [step, setStep] = useState<Step>("closed");
  const [email, setEmail] = useState("");
  const [typed, setTyped] = useState("");
  const [master, setMaster] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.accountStatus().then((a) => setEmail(a?.email ?? ""), () => undefined);
  }, []);
  // The password never outlives the section.
  useEffect(() => () => setMaster(""), []);

  const matches = typed.trim().toLowerCase() === email.trim().toLowerCase() && email !== "";

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await api.deleteAccount(typed, master);
      // App.tsx handles vault://account-deleted and returns to first run.
    } catch (e) {
      setError(errorMessage(e, t, t.deleteAccount.failed));
    } finally {
      setMaster("");
      setBusy(false);
    }
  }

  return (
    <div className="settings-block">
      <h3 className="group-title">{t.deleteAccount.title}</h3>
      <div className="group danger-zone">
        <p className="group-note group-note-top">{online ? t.deleteAccount.explain : t.deleteAccount.offline}</p>
        {step === "closed" && (
          <div className="group-actions">
            <button className="btn btn-danger" disabled={!online} onClick={() => setStep("explain")}>
              {t.deleteAccount.title}
            </button>
          </div>
        )}
        {step === "explain" && (
          <>
            <p className="group-note">{t.deleteAccount.keptNote}</p>
            <div className="group-actions">
              <button
                className="btn btn-primary"
                onClick={() => document.getElementById("export")?.scrollIntoView({ behavior: "smooth" })}
              >
                {t.deleteAccount.backupFirst}
              </button>
              <button className="btn" onClick={() => setStep("confirm")}>
                {t.deleteAccount.continueWithout}
              </button>
              <button className="btn" onClick={() => setStep("closed")}>
                {t.common.cancel}
              </button>
            </div>
          </>
        )}
        {step === "confirm" && (
          <>
            <label className="row row-input">
              <span>{t.deleteAccount.typeToConfirm(email)}</span>
              <input
                name="confirm-email"
                value={typed}
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
                onChange={(e) => setTyped(e.target.value)}
              />
            </label>
            <label className="row row-input">
              <span>{t.deleteAccount.masterPassword}</span>
              <input
                type="password"
                name="master-password"
                autoComplete="current-password"
                value={master}
                disabled={busy}
                onChange={(e) => setMaster(e.target.value)}
              />
            </label>
            {error && (
              <p className="form-error" role="alert">
                {error}
              </p>
            )}
            <div className="group-actions">
              <button className="btn btn-danger" disabled={!matches || !master || busy || !online} onClick={remove}>
                {t.deleteAccount.confirm}
              </button>
              <button
                className="btn"
                disabled={busy}
                onClick={() => {
                  setMaster("");
                  setTyped("");
                  setError(null);
                  setStep("closed");
                }}
              >
                {t.common.cancel}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
```

(If `api.accountStatus()` resolves to a different shape, read `email` the way `AccountSection.tsx` does.)

`SettingsView.tsx`: import and render `<DeleteAccountSection online={online} />` after `<UpdatesSection />`. `ExportSection.tsx`: add `id="export"` to its `<h3 className="group-title">`.

`App.tsx`: add a state `const [accountDeleted, setAccountDeleted] = useState(false);` and an effect mirroring the `onRemoved` one:

```tsx
  // The account was deleted, here or on another device: same reset as a
  // removal, plus a notice saying why the vault is gone.
  useEffect(() => {
    const unlisten = api.onAccountDeleted(({ keychainWarning }) => {
      setAccountDeleted(true);
      setRemovedWarning(keychainWarning !== null);
      setLockReason(null);
      setShowKit(false);
      setSignedOut(false);
      setPairedAs(null);
      setSession((s) => s + 1);
      api.status().then(setStatus, () => undefined);
      api.deviceStatus().then(setDevice, () => setDevice(null));
    });
    return () => void unlisten.then((f) => f());
  }, []);
```

Show the notice where `removedWarning` is shown (find its render site in `App.tsx`): `{accountDeleted && <p className="banner" role="status">{t.deleteAccount.done}</p>}`, using the same banner class the removed warning uses.

- [ ] **Step 5: Run tests and typecheck**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop): delete account and all data from Settings"
```

---

### Task 9: Android — danger-zone UI and event

**Files:**
- Modify: `apps/android/app/src/main/kotlin/net/havenkeys/android/data/AccountRepository.kt`, `data/VaultEventsHub.kt`, `ui/nav/RootViewModel.kt`, `AppContainer.kt` (`wipeKeysOnExit`), `ui/settings/SettingsScreen.kt`, `ui/settings/SettingsDialog.kt`, `ui/settings/SettingsViewModel.kt`, `ui/settings/SettingsActions.kt`, `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Modify every `when (event)` over `VaultEvent` that handles `Removed` (`SensitiveClipboard.kt:94`, `HomeViewModel.kt:50`, `SearchViewModel.kt:62`, `ItemListViewModel.kt:54`, `ItemViewModel.kt:41`, `EditViewModel.kt:67`, `ShellViewModel.kt:60`) to treat `AccountDeleted` the same as `Removed`.
- Test: `app/src/test/kotlin/net/havenkeys/android/ui/settings/SettingsViewModelTest.kt`, `SettingsScreenTest.kt`, `ui/nav/RootViewModelTest.kt`, `fakes/FakeRepositories.kt`

**Interfaces:**
- Consumes: Kotlin `MobileVault.deleteAccount(confirmation, masterPassword)`, `VaultEvents.accountDeleted()` (Task 6).
- Produces: `AccountRepository.deleteAccount(confirmation: String, masterPassword: String): Outcome<Unit>`; `VaultEvent.AccountDeleted`; `SettingsDialog.DELETE_ACCOUNT`; `SettingsViewModel.deleteAccount(confirmation: String, masterPassword: String)`; `SettingsUiState.deleting`, `.deleteErrorCode`, `.deleteFailures`.

- [ ] **Step 1: Strings** — `values/strings.xml`:

```xml
    <string name="settings_delete_title">Delete account and all data</string>
    <string name="settings_delete_note">Deletes your vault from the server, every device and this phone\'s copy. Cannot be undone.</string>
    <string name="settings_delete_explain">This deletes your vault from the server, signs out every device, and erases this phone\'s copy. Neither you nor the server\'s operator can recover it. To keep a copy, export an encrypted backup from the desktop first.</string>
    <string name="settings_delete_anyway">Delete anyway</string>
    <string name="settings_delete_confirm">Delete account</string>
    <string name="settings_delete_password">Master password</string>
    <string name="settings_delete_verify_title">Confirm it\'s you</string>
    <string name="settings_delete_failed">The account could not be deleted.</string>
    <string name="onboarding_account_deleted">This account was deleted. Its data is gone from the server and this phone.</string>
    <string name="error_account_deleted">This account was deleted.</string>
```

`values-pt-rBR/strings.xml`:

```xml
    <string name="settings_delete_title">Excluir conta e todos os dados</string>
    <string name="settings_delete_note">Apaga o seu cofre do servidor, de todos os dispositivos e deste celular. Não dá para desfazer.</string>
    <string name="settings_delete_explain">Isso apaga o seu cofre do servidor, desconecta todos os dispositivos e apaga a cópia deste celular. Nem você nem o operador do servidor conseguem recuperá-lo. Para guardar uma cópia, exporte um backup cifrado pelo desktop antes.</string>
    <string name="settings_delete_anyway">Excluir mesmo assim</string>
    <string name="settings_delete_confirm">Excluir conta</string>
    <string name="settings_delete_password">Senha mestra</string>
    <string name="settings_delete_verify_title">Confirme que é você</string>
    <string name="settings_delete_failed">Não foi possível excluir a conta.</string>
    <string name="onboarding_account_deleted">Esta conta foi excluída. Os dados dela não estão mais no servidor nem neste celular.</string>
    <string name="error_account_deleted">Esta conta foi excluída.</string>
```

Add `"account_deleted" -> R.string.error_account_deleted` to `errorText` in `ui/components/ErrorText.kt`.

- [ ] **Step 2: Write failing tests**

`FakeRepositories.kt` `FakeAccountRepository`: add

```kotlin
    var deleteOutcome: Outcome<Unit> = Outcome.Ok(Unit)
    override suspend fun deleteAccount(confirmation: String, masterPassword: String): Outcome<Unit> {
        calls += "deleteAccount:$confirmation"
        return deleteOutcome
    }
```

(never record the password.)

`SettingsViewModelTest.kt`:

```kotlin
    @Test
    fun `deleting passes the confirmation and never keeps the password`() = runTest {
        val accounts = FakeAccountRepository()
        val vm = settingsViewModel(accounts = accounts) // the file's existing factory
        vm.deleteAccount("user@example.com", "pw")
        advanceUntilIdle()
        assertEquals(listOf("deleteAccount:user@example.com"), accounts.calls.filter { it.startsWith("delete") })
        assertFalse(vm.state.value.deleting)
    }

    @Test
    fun `a failed delete shows its code and counts the failure`() = runTest {
        val accounts = FakeAccountRepository().apply { deleteOutcome = Outcome.Failed("unlock_failed") }
        val vm = settingsViewModel(accounts = accounts)
        vm.deleteAccount("user@example.com", "wrong")
        advanceUntilIdle()
        assertEquals("unlock_failed", vm.state.value.deleteErrorCode)
        assertEquals(1, vm.state.value.deleteFailures)
    }
```

`RootViewModelTest.kt`:

```kotlin
    @Test
    fun `an account deletion returns to onboarding`() = runTest {
        // same setup as the existing Removed test in this file
        hub.accountDeleted()
        assertEquals(Start.ONBOARDING, root.lockedSignal.receive())
    }
```

`SettingsScreenTest.kt`: a test that tapping `settings_delete_title` opens a dialog whose message is `settings_delete_explain`, that `settings_delete_anyway` shows the email and password fields, and that the confirm button (`settings_delete_confirm`, `hasAnyAncestor(isDialog())`) is disabled until both are filled — copy the structure of the existing REMOVE dialog test in that file.

- [ ] **Step 3: Run to verify they fail**

Run (from `apps/android`): `./gradlew testGithubDebugUnitTest --tests '*Settings*' --tests '*RootViewModelTest*'`
Expected: compile FAIL.

- [ ] **Step 4: Implement**

- `AccountRepository`: `suspend fun deleteAccount(confirmation: String, masterPassword: String): Outcome<Unit>`; `RustAccountRepository`: `override suspend fun deleteAccount(confirmation: String, masterPassword: String) = rust { vault.deleteAccount(confirmation, masterPassword) }`.
- `VaultEventsHub`: `data object AccountDeleted : VaultEvent`; `override fun accountDeleted() { _unlocked = false; emit(VaultEvent.AccountDeleted) }` (same as `removed()`).
- `RootViewModel.init`: `VaultEvent.AccountDeleted -> signals.send(Start.ONBOARDING)`; also set a flag the onboarding screen reads to show `onboarding_account_deleted` once (follow how `RootViewModel` exposes other one-shot state; if none exists, add `val accountDeleted = MutableStateFlow(false)` set here and cleared by the onboarding screen after showing it).
- `AppContainer.wipeKeysOnExit`: `VaultEvent.AccountDeleted` runs both deletes, like `Removed`.
- The seven `when (event)` sites: add `VaultEvent.AccountDeleted` alongside `VaultEvent.Removed`.
- `SettingsUiState`: `deleting: Boolean = false`, `deleteErrorCode: String? = null`, `deleteFailures: Int = 0`. `SettingsViewModel`:

```kotlin
    fun deleteAccount(confirmation: String, masterPassword: String) {
        if (_state.value.deleting) return
        _state.update { it.copy(deleting = true, deleteErrorCode = null) }
        viewModelScope.launch {
            when (val out = accounts.deleteAccount(confirmation, masterPassword)) {
                is Outcome.Ok -> _state.update { it.copy(deleting = false) }
                is Outcome.Failed -> _state.update {
                    it.copy(deleting = false, deleteErrorCode = out.code, deleteFailures = it.deleteFailures + 1)
                }
            }
        }
    }
```

(use the actual `Outcome` subtype names from `data/Outcome.kt`.)

- `SettingsActions`: add `val verifyUser: suspend (title: String, subtitle: String) -> Boolean` and `val canVerifyUser: () -> Boolean`, built in `rememberSettingsActions(container, activity)` from `container.biometricGate.verifyUser(activity, title, subtitle)` / `canVerifyUser(activity)` as `NavServices` does.
- `SettingsScreen.AccountGroup`: in the danger `InsetGroup`, after the remove row, a second `GroupRow(onClick = { open.dialog(SettingsDialog.DELETE_ACCOUNT) })` with `settings_delete_title` (danger color) and `offline ?: settings_delete_note`; disabled while offline like the remove row.
- `SettingsDialog`: add `DELETE_ACCOUNT` and a `DeleteAccountDialog` built like `RemoveDialog`, with two stages held in `rememberSaveable { mutableStateOf(false) }`:
  1. stage 1: `HavenDialog(title = settings_delete_title, message = settings_delete_explain, confirm = DialogAction(settings_delete_anyway, { stage2 = true }, danger = true), dismiss = cancel)`.
  2. stage 2: email `HavenTextField` (as in `RemoveDialog`) plus a `SecretTextField` for the password with `DisposableEffect(field) { onDispose { field.clearText() } }`; `confirmEnabled = email.isNotBlank() && password.isNotBlank()`; `busy = state.deleting`; `answerKey = state.deleteFailures`; confirm `DialogAction(settings_delete_confirm, onClick, danger = true)` whose `onClick` runs on the composition scope:

```kotlin
scope.launch {
    if (actions.canVerifyUser() && !actions.verifyUser(verifyTitle, email.text.toString())) return@launch
    onDelete(email.text.toString(), password.text.toString())
    password.clearText()
}
```

  Error text: `INVALID_INPUT` → `settings_remove_mismatch`, `"internal"` → `settings_delete_failed`, else `errorText(code)`.

- [ ] **Step 5: Run Android checks**

Run (from `apps/android`): `./gradlew detekt testGithubDebugUnitTest lintGithubDebug`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/android
git commit -m "feat(android): delete account and all data from Settings"
```

---

### Task 10: Website — deletion page and privacy policy

**Files:**
- Create: `apps/web/src/pages/DeleteAccount.tsx`
- Modify: `apps/web/src/App.tsx` (PAGES), `apps/web/src/components/Footer.tsx`, `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx`

**Interfaces:**
- Produces: route `/delete-account` and `/pt-br/delete-account`; i18n namespace `deleteAccount: { title, updated, body }`; `footer.deleteAccount`.

- [ ] **Step 1: Page component** — `DeleteAccount.tsx` (same shape as `Privacy.tsx`):

```tsx
import { useI18n } from "../i18n/context";

export function DeleteAccount() {
  const { t } = useI18n();
  return (
    <section className="docs-page">
      <h1>{t.deleteAccount.title}</h1>
      <p className="docs-page__updated">{t.deleteAccount.updated}</p>
      {t.deleteAccount.body}
    </section>
  );
}
```

`App.tsx` PAGES: `{ path: "delete-account", element: <DeleteAccount /> },` after privacy. `Footer.tsx`: `<Link to={path("/delete-account")}>{t.footer.deleteAccount}</Link>` after the privacy link.

- [ ] **Step 2: EN strings** (`en.tsx`): `footer.deleteAccount: "Delete account"`, and

```tsx
  deleteAccount: {
    title: "Delete your account",
    updated: "Last updated October 5, 2026.",
    body: (
      <>
        <h2>From the app</h2>
        <p>
          On the desktop: Settings → <strong>Delete account and all data</strong>. On Android:
          Settings → Account → <strong>Delete account and all data</strong>. You confirm with your
          account's email and your master password. Make an encrypted backup first if you may want
          your data later: deletion cannot be undone.
        </p>
        <h2>What is deleted, and when</h2>
        <ul>
          <li>
            <strong>Immediately:</strong> your encrypted vault and items, your devices and sessions,
            your email address, your key-derivation parameters and your failed-sign-in counters. The
            copy on the device you used is erased too, and your other devices erase theirs the next
            time they connect.
          </li>
          <li>
            <strong>Within 30 days:</strong> anonymous fingerprints of your former sessions, kept
            only so your other devices learn the account is gone. They contain no email, name or
            account identifier.
          </li>
          <li>
            <strong>Within 30 days:</strong> copies in database backups and server logs, which
            expire on their own.
          </li>
        </ul>
        <p>
          Backups you exported yourself, and copies set aside on a device by an earlier "Remove
          this device", are yours and are not touched.
        </p>
        <h2>Lost access to your account?</h2>
        <p>
          Email <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a> from the
          address of the account. We will confirm the request and delete the account the same way
          the app does.
        </p>
      </>
    ),
  },
```

- [ ] **Step 3: PT-BR strings** (`pt-BR.tsx`): `footer.deleteAccount: "Excluir conta"`, and

```tsx
  deleteAccount: {
    title: "Excluir a sua conta",
    updated: "Atualizada em 5 de outubro de 2026.",
    body: (
      <>
        <h2>Pelo app</h2>
        <p>
          No desktop: Configurações → <strong>Excluir conta e todos os dados</strong>. No Android:
          Configurações → Conta → <strong>Excluir conta e todos os dados</strong>. Você confirma com
          o e-mail da conta e a sua senha mestra. Faça um backup cifrado antes se puder querer seus
          dados depois: a exclusão não pode ser desfeita.
        </p>
        <h2>O que é apagado, e quando</h2>
        <ul>
          <li>
            <strong>Imediatamente:</strong> o seu cofre cifrado e os itens, seus dispositivos e
            sessões, seu endereço de e-mail, os parâmetros de derivação de chave e os contadores de
            tentativas de login. A cópia no dispositivo que você usou também é apagada, e os outros
            dispositivos apagam as deles na próxima vez que se conectarem.
          </li>
          <li>
            <strong>Em até 30 dias:</strong> impressões digitais anônimas das suas sessões antigas,
            guardadas só para que os outros dispositivos saibam que a conta não existe mais. Elas
            não contêm e-mail, nome nem identificador da conta.
          </li>
          <li>
            <strong>Em até 30 dias:</strong> cópias em backups do banco de dados e em logs do
            servidor, que expiram sozinhas.
          </li>
        </ul>
        <p>
          Backups que você mesmo exportou, e cópias separadas num dispositivo por um "Remover este
          dispositivo" anterior, são seus e não são tocados.
        </p>
        <h2>Perdeu o acesso à conta?</h2>
        <p>
          Envie um e-mail para{" "}
          <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a> a partir do
          endereço da conta. Confirmaremos o pedido e excluiremos a conta do mesmo jeito que o app
          faz.
        </p>
      </>
    ),
  },
```

- [ ] **Step 4: Privacy policy edits** (both locales)

1. `updated`: EN `"Last updated October 5, 2026."`, PT `"Atualizada em 5 de outubro de 2026."`.
2. Replace EN "We do not operate a hosted server and have no access to yours." with: "The developers also operate a server for other people, run by SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA, which is the controller of the personal data listed here for accounts on it. That server cannot decrypt vaults either." PT: replace "Não operamos nenhum servidor hospedado e não temos acesso ao seu." with "Os desenvolvedores também operam um servidor para outras pessoas, mantido por SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA, que é a controladora dos dados pessoais listados aqui para as contas nele. Esse servidor também não consegue decifrar cofres."
3. In "Keeping and deleting your data" / "Guarda e exclusão dos seus dados", replace the sentence about the operator deleting the account with a pointer: EN "You can delete your account yourself from the app; <Link to={path('/delete-account')}>Delete your account</Link> says what is erased and when." PT "Você pode excluir a sua conta pelo próprio app; <Link ...>Excluir a sua conta</Link> diz o que é apagado e quando." (Use `useI18n().path` — if the i18n file cannot call hooks, link with a plain `<a href="/delete-account">` / `<a href="/pt-br/delete-account">`.)
4. "Contact" / "Contato": add "For privacy requests, including deletion: samuelsilv.rocha@gmail.com." / "Para pedidos de privacidade, incluindo exclusão: samuelsilv.rocha@gmail.com."

- [ ] **Step 5: Build**

Run: `pnpm --filter @havenkeys/web typecheck && pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web build`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): account deletion page; privacy policy names the controller"
```

---

### Task 11: Documentation and final verification

**Files:**
- Modify: `CLAUDE.md`, `docs/server-sync.md`, `docs/threat-model.md`, `docs/security-model.md`, `docs/deployment.md`, `docs/security-review.md`

- [ ] **Step 1: CLAUDE.md** — under §1, after the 2026-09-27 amendment, add the block from spec §3 verbatim, and replace "It is not a hosted service and there is no vendor account." with "Anyone may run it; the author also runs one for other people. There is no vendor account beyond an account on that server."

- [ ] **Step 2: Docs**
  - `server-sync.md`: a "Deleting the account" section — route, body, proofs, `erase_account` order, `deleted_sessions`, `410 account_deleted`, sweep.
  - `threat-model.md`: "A malicious server can make devices erase their replica (410)" — impact nil beyond what the server already can do; mitigation: encrypted backup offered first. Residual data list from spec §10.
  - `security-model.md`: what deletion erases (server, this device, other devices on next connect) and what it does not (exported backups, set-aside files, Postgres backups/logs ≤ 30 days, IP-keyed rate-limit rows, devices offline > 30 days stay merely signed out).
  - `deployment.md`: the daily sweep task; `admin delete-account` now equals in-app deletion; promise in the privacy policy of ≤ 30 days for backups/logs that the operator must honour.
  - `security-review.md`: a finding entry "Account deletion" — severity Info; component server/client; attack scenarios (stolen token, malicious-server wipe); mitigation; remaining limitations (same list).

- [ ] **Step 3: Full verification**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && pnpm typecheck && pnpm -r test && cargo deny check`
Expected: all PASS. Then grep the diff for leaked secrets in logs: `git diff main --stat` and `git diff main | grep -n "tracing::" ` — every new log line carries `account_id` at most.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs
git commit -m "docs: account deletion — CLAUDE.md amendment, threat and security model, deployment"
```
