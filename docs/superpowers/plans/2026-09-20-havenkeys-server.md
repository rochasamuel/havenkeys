# havenkeys-server — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `havenkeys-server`: the blind-relay sync server that owns the
vault's authority — accounts, sessions, the vault header, per-item revisions
and the monotonic sync cursor — plus its admin CLI, its test suite against a
real Postgres, and everything needed to deploy it.

**Architecture:** Rust + axum + sqlx + Postgres, one crate in this monorepo.
The server stores ciphertext it cannot open: item blobs and the vault header
are opaque bytes, never parsed. Identity always comes from the session token,
never from a request body. Every mutating request runs in one transaction that
bumps `vaults.revision` once and stamps every touched row with it, so the pull
cursor is monotonic and no reader sees half a batch. Writes are optimistic per
item: a change carries the revision the client last saw, and a stale one
refuses the **whole batch**.

**Tech Stack:** Rust 2021 (MSRV 1.88), `axum` 0.8, `tokio`, `sqlx` 0.8
(Postgres, `rustls`, **no macros** — runtime queries only, so the crate builds
without a database), `argon2`, `sha2`, `hkdf`, `rand`, `uuid`, `serde`,
`serde_json`, `data-encoding`, `zeroize`, `tracing`, `clap`.

**Spec:** `docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`
(§7 and §12), which cites `2026-09-19-server-accounts-sync-design.md` §5–§7,
§10 and §12 for the parts it does not restate.

## Global Constraints

Copied from `CLAUDE.md` and the spec; every task's requirements include these.

- **Never invent cryptography.** Argon2id (`argon2` crate) for the auth
  verifier, SHA-256 (`sha2`) for token and invite hashes, HKDF-SHA256 (`hkdf`)
  for the enumeration-resistant salt. Nothing else.
- **Never log secrets.** No token, `authKey`, invite string, blob, header,
  password or email in any log line, error body or panic message. Emails are
  identifiers the server holds, but the account UUID is what logs carry
  (spec §7.5). `tests/no_logging.rs` enforces it.
- **The client is not trusted.** `account_id` and `vault_id` come from the
  session, never from a body. A body carrying them is rejected, not ignored.
- **`deny_unknown_fields` on every request DTO**, as in the native messaging
  protocol.
- **Limits:** 8 MiB per blob, 16 MiB per request body, 500 changes per batch,
  64 devices per account, 64 KiB per header.
- **Uniform failures:** every authentication failure answers
  `401 {"error":{"code":"unauthorized","message":"Authentication failed."}}`.
  The server never says whether the email exists.
- **Blobs are opaque.** The server never parses, validates or re-encodes item
  ciphertext or the header. Its only checks are length and base64 shape.
- **CORS denied by default.** No browser origin is allowed unless
  `HAVENKEYS_CORS_ORIGIN` names exactly one.
- **No telemetry, no analytics, no third-party APIs** (`CLAUDE.md` §1).
- **MSRV 1.88**, edition 2021, `cargo fmt` clean, `cargo clippy -p
  havenkeys-server --all-targets -- -D warnings` clean.
- **Every task ends green:** `cargo test -p havenkeys-server` (with Postgres
  running, see Task 1) and the workspace default members still build.
- **Commit after every task.** End commit messages with
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`.

---

## File structure

**Created**

| File | Responsibility |
|---|---|
| `crates/havenkeys-server/Cargo.toml` | Crate manifest and dependency floors. |
| `crates/havenkeys-server/src/main.rs` | Binary entry point: config, pool, migrations, router, graceful shutdown, or the `admin` subcommand. |
| `crates/havenkeys-server/src/lib.rs` | Library root, so tests can build the router in-process. |
| `crates/havenkeys-server/src/config.rs` | Environment parsing: `DATABASE_URL`, `SERVER_SECRET`, `PORT`, `HAVENKEYS_CORS_ORIGIN`. |
| `crates/havenkeys-server/src/db.rs` | Pool construction and `sqlx::migrate!`. |
| `crates/havenkeys-server/src/error.rs` | `ApiError`: status + stable code + uniform message, `IntoResponse`. |
| `crates/havenkeys-server/src/limits.rs` | Every size limit in one place. |
| `crates/havenkeys-server/src/b64.rs` | Base64 blob wrapper with length checking, used by every DTO. |
| `crates/havenkeys-server/src/auth/mod.rs` | Session extractor, token issue/verify, `Argon2` verifier params. |
| `crates/havenkeys-server/src/auth/rate_limit.rs` | `login_attempts` accounting and the escalating block. |
| `crates/havenkeys-server/src/routes/mod.rs` | Router assembly and shared state. |
| `crates/havenkeys-server/src/routes/health.rs` | `GET /v1/health`. |
| `crates/havenkeys-server/src/routes/accounts.rs` | `POST /v1/accounts/activate`. |
| `crates/havenkeys-server/src/routes/auth.rs` | `POST /v1/auth/params`, `/login`, `/logout`. |
| `crates/havenkeys-server/src/routes/vault.rs` | `GET`/`PUT /v1/vault/header`. |
| `crates/havenkeys-server/src/routes/sync.rs` | `GET /v1/sync`. |
| `crates/havenkeys-server/src/routes/items.rs` | `POST /v1/items`. |
| `crates/havenkeys-server/src/routes/devices.rs` | `GET /v1/devices`, `DELETE /v1/devices/:id`. |
| `crates/havenkeys-server/src/admin.rs` | `admin new-account`, `list-accounts`, `delete-account`. |
| `crates/havenkeys-server/src/invite.rs` | Invite secret, `HKINV1-…` encoding, hashing. |
| `crates/havenkeys-server/migrations/0001_init.sql` | The whole schema. |
| `crates/havenkeys-server/tests/support/mod.rs` | Disposable database + in-process server + typed HTTP client. |
| `crates/havenkeys-server/tests/auth.rs` | Activation, login, sessions, rate limiting, enumeration. |
| `crates/havenkeys-server/tests/sync.rs` | Header, pull, write, conflict, cursor monotonicity. |
| `crates/havenkeys-server/tests/isolation.rs` | Account A against every authenticated route of account B. |
| `crates/havenkeys-server/tests/limits.rs` | Sizes, shapes, unknown fields, non-UUIDs. |
| `crates/havenkeys-server/tests/no_logging.rs` | No secret-bearing value reaches a log line. |
| `crates/havenkeys-server/Dockerfile` | Multi-stage build for Railway. |
| `crates/havenkeys-server/railway.json` | Railway service definition. |
| `scripts/test-server.sh` | Starts the disposable Postgres and runs the suite. |
| `docs/deployment.md` | Deploy, environment, backup **and tested restore** runbook. |

**Modified**

| File | Change |
|---|---|
| `Cargo.toml` | `crates/havenkeys-server` in `members` and `default-members`. |
| `deny.toml` | Any new licence/advisory exceptions the server's tree needs. |
| `docs/roadmap.md` | Step 2 status at the end of the plan. |

---

## Task 1: Crate skeleton, config, health, and a disposable-Postgres harness

**Files:**
- Create: `crates/havenkeys-server/Cargo.toml`, `src/lib.rs`, `src/main.rs`,
  `src/config.rs`, `src/db.rs`, `src/error.rs`, `src/limits.rs`,
  `src/routes/mod.rs`, `src/routes/health.rs`,
  `migrations/0001_init.sql` (empty marker table only in this task),
  `tests/support/mod.rs`, `tests/health.rs`, `scripts/test-server.sh`
- Modify: `Cargo.toml`

**Interfaces:**
- Produces: `havenkeys_server::router(AppState) -> axum::Router`,
  `havenkeys_server::AppState { pool: PgPool, server_secret: [u8;32], cors_origin: Option<String> }`,
  `havenkeys_server::db::connect(&str) -> Result<PgPool>`,
  `havenkeys_server::db::migrate(&PgPool)`,
  test support `TestServer::start().await -> TestServer` with
  `TestServer::get/post/put/delete(path) -> reqwest::RequestBuilder` and
  `TestServer::pool() -> &PgPool`.

- [ ] **Step 1: Start Postgres for tests**

```bash
docker run -d --name havenkeys-test-pg -p 5433:5432 \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=postgres \
  postgres:17-alpine
```

Write `scripts/test-server.sh` doing the same idempotently, then running
`cargo test -p havenkeys-server`, with
`HAVENKEYS_TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5433/postgres`.

- [ ] **Step 2: Write the failing test**

`crates/havenkeys-server/tests/health.rs`:

```rust
mod support;

#[tokio::test]
async fn health_reports_ok() {
    let server = support::TestServer::start().await;
    let res = server.get("/v1/health").send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}
```

- [ ] **Step 3: Run it and watch it fail**

Run: `cargo test -p havenkeys-server --test health`
Expected: compile failure — the crate does not exist yet.

- [ ] **Step 4: Write the manifest**

`crates/havenkeys-server/Cargo.toml`:

```toml
[package]
name = "havenkeys-server"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
axum = { version = "0.8", default-features = false, features = ["http1", "json", "query", "tokio", "macros"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal", "net"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["limit", "trace", "cors"] }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls-ring", "postgres", "uuid", "chrono", "migrate"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", default-features = false, features = ["clock", "serde"] }
argon2 = "0.5"
sha2 = "0.10"
hkdf = "0.12"
rand = "0.8"
data-encoding = "2"
zeroize = { version = "1", features = ["zeroize_derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
clap = { version = "4", features = ["derive"] }
thiserror = "2"

[dev-dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

Add the crate to the workspace `members` **and** `default-members`.

- [ ] **Step 5: Write config, error, limits, db and the router**

`src/limits.rs`:

```rust
pub const MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_CHANGES_PER_BATCH: usize = 500;
pub const MAX_DEVICES_PER_ACCOUNT: i64 = 64;
pub const MAX_PULL_PAGE: i64 = 500;
pub const SESSION_TTL_HOURS: i64 = 24;
```

`src/config.rs`:

```rust
use data_encoding::BASE64;

pub struct Config {
    pub database_url: String,
    pub server_secret: [u8; 32],
    pub port: u16,
    pub cors_origin: Option<String>,
}

impl Config {
    /// Reads the environment. Fails loudly rather than inventing a default
    /// secret: a predictable `SERVER_SECRET` would make `auth/params` salts
    /// guessable and reintroduce account enumeration.
    pub fn from_env() -> Result<Self, String> {
        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_string())?;
        let raw = std::env::var("SERVER_SECRET")
            .map_err(|_| "SERVER_SECRET is not set (32 random bytes, base64)".to_string())?;
        let decoded = BASE64
            .decode(raw.trim().as_bytes())
            .map_err(|_| "SERVER_SECRET is not valid base64".to_string())?;
        let server_secret: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| "SERVER_SECRET must decode to exactly 32 bytes".to_string())?;
        let port = match std::env::var("PORT") {
            Ok(p) => p.parse().map_err(|_| "PORT is not a number".to_string())?,
            Err(_) => 8080,
        };
        Ok(Self {
            database_url,
            server_secret,
            port,
            cors_origin: std::env::var("HAVENKEYS_CORS_ORIGIN").ok(),
        })
    }
}
```

`src/error.rs`:

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// Every failure the API can return. Messages are fixed strings: they never
/// carry a value from the request, so nothing secret can reach a client or a
/// log through an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError {
    Unauthorized,
    NotFound,
    Conflict,
    InvalidRequest(&'static str),
    TooLarge,
    RateLimited,
    Internal,
}

impl ApiError {
    fn parts(self) -> (StatusCode, &'static str, &'static str) {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "Authentication failed."),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", "Not found."),
            Self::Conflict => (StatusCode::CONFLICT, "conflict", "The stored revision has moved on."),
            Self::InvalidRequest(m) => (StatusCode::BAD_REQUEST, "invalid_request", m),
            Self::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "too_large", "Request is too large."),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many attempts. Try again later."),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal", "Internal error."),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
        (status, Json(serde_json::json!({"error": {"code": code, "message": message}}))).into_response()
    }
}

/// A database failure is logged with its context and reported as `internal`.
/// The `sqlx::Error` never reaches the client: it can quote SQL, which can
/// quote values.
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!(kind = %kind_of(&err), "database error");
        Self::Internal
    }
}

fn kind_of(err: &sqlx::Error) -> &'static str {
    match err {
        sqlx::Error::RowNotFound => "row_not_found",
        sqlx::Error::Database(_) => "database",
        sqlx::Error::PoolTimedOut => "pool_timeout",
        _ => "other",
    }
}
```

`src/db.rs`:

```rust
use sqlx::postgres::{PgPool, PgPoolOptions};

pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
```

`src/routes/health.rs`:

```rust
use axum::Json;

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
```

`src/routes/mod.rs`:

```rust
use crate::limits::MAX_BODY_BYTES;
use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use tower_http::limit::RequestBodyLimitLayer;

pub mod health;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub server_secret: [u8; 32],
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health::health))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .with_state(state)
}
```

`src/lib.rs` re-exports `router`, `AppState`, `config`, `db`, `error`,
`limits`. `src/main.rs` parses `Config`, connects, migrates, and serves; the
`admin` subcommand arrives in Task 3.

`migrations/0001_init.sql` contains the full schema in Task 2; for this task
it may be a single `CREATE TABLE IF NOT EXISTS schema_marker (id INT PRIMARY KEY);`
that Task 2 replaces (the migration has not shipped anywhere, so editing it is
honest, not a rewrite of history).

- [ ] **Step 6: Write the test harness**

`crates/havenkeys-server/tests/support/mod.rs`:

```rust
#![allow(dead_code)]

use havenkeys_server::{router, AppState};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

const DEFAULT_URL: &str = "postgres://postgres:postgres@localhost:5433/postgres";

/// A server on an ephemeral port with a database of its own.
///
/// Every test gets a fresh database rather than a transaction, because the
/// isolation guarantees under test are SQL-level (foreign keys, unique
/// constraints, one transaction per write) and a shared connection would
/// hide them.
pub struct TestServer {
    base: String,
    pool: PgPool,
    admin_url: String,
    db_name: String,
    client: reqwest::Client,
}

impl TestServer {
    pub async fn start() -> Self {
        let admin_url =
            std::env::var("HAVENKEYS_TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.into());
        let db_name = format!("hk_test_{}", Uuid::new_v4().simple());
        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("Postgres is not reachable — run scripts/test-server.sh");
        admin
            .execute(format!(r#"CREATE DATABASE "{db_name}""#).as_str())
            .await
            .unwrap();

        let url = swap_database(&admin_url, &db_name);
        let pool = havenkeys_server::db::connect(&url).await.unwrap();
        havenkeys_server::db::migrate(&pool).await.unwrap();

        let state = AppState { pool: pool.clone(), server_secret: [7u8; 32] };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router(state)).await.unwrap();
        });

        Self {
            base: format!("http://{addr}"),
            pool,
            admin_url,
            db_name,
            client: reqwest::Client::new(),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.get(self.url(path))
    }
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(self.url(path))
    }
    pub fn put(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.put(self.url(path))
    }
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.delete(self.url(path))
    }

    /// Best-effort cleanup; a leaked test database costs a disposable
    /// container nothing, so this never fails a test.
    pub async fn cleanup(self) {
        let Self { pool, admin_url, db_name, .. } = self;
        pool.close().await;
        if let Ok(mut admin) = PgConnection::connect(&admin_url).await {
            let _ = admin
                .execute(format!(r#"DROP DATABASE IF EXISTS "{db_name}" WITH (FORCE)"#).as_str())
                .await;
        }
    }
}

fn swap_database(url: &str, db: &str) -> String {
    match url.rsplit_once('/') {
        Some((head, _)) => format!("{head}/{db}"),
        None => format!("{url}/{db}"),
    }
}
```

- [ ] **Step 7: Run the test**

Run: `scripts/test-server.sh`
Expected: `health_reports_ok` passes.

- [ ] **Step 8: Commit**

```bash
git add crates/havenkeys-server Cargo.toml scripts/test-server.sh
git commit -m "feat(server): crate skeleton, config and a disposable-Postgres harness"
```

---

## Task 2: The schema

**Files:**
- Modify: `crates/havenkeys-server/migrations/0001_init.sql`
- Test: `crates/havenkeys-server/tests/schema.rs`

**Interfaces:**
- Produces: the tables every later task queries — `accounts`, `vaults`,
  `items`, `devices`, `sessions`, `login_attempts`.

- [ ] **Step 1: Write the failing test**

```rust
mod support;

/// The constraints below are the isolation guarantees other tests rely on,
/// so they are asserted directly rather than inferred from behaviour.
#[tokio::test]
async fn schema_enforces_one_vault_per_account_and_cascades() {
    let server = support::TestServer::start().await;
    let pool = server.pool();

    let account = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO accounts (id, email_normalized, status, created_at) VALUES ($1, $2, 'invited', now())")
        .bind(account).bind("a@example.com").execute(pool).await.unwrap();

    let vault = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO vaults (id, account_id, header, header_revision, key_scheme, revision, created_at) VALUES ($1, $2, $3, 0, 3, 0, now())")
        .bind(vault).bind(account).bind(vec![1u8, 2, 3]).execute(pool).await.unwrap();

    // A second vault for the same account must be impossible.
    let second = sqlx::query("INSERT INTO vaults (id, account_id, header, header_revision, key_scheme, revision, created_at) VALUES ($1, $2, $3, 0, 3, 0, now())")
        .bind(uuid::Uuid::new_v4()).bind(account).bind(vec![4u8]).execute(pool).await;
    assert!(second.is_err(), "one vault per account");

    sqlx::query("INSERT INTO items (vault_id, item_id, overview, details, revision) VALUES ($1, $2, $3, $4, 1)")
        .bind(vault).bind(uuid::Uuid::new_v4()).bind(vec![9u8]).bind(vec![9u8]).execute(pool).await.unwrap();

    sqlx::query("DELETE FROM accounts WHERE id = $1").bind(account).execute(pool).await.unwrap();
    let items: i64 = sqlx::query_scalar("SELECT count(*) FROM items").fetch_one(pool).await.unwrap();
    assert_eq!(items, 0, "deleting an account removes its vault and its items");

    server.cleanup().await;
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p havenkeys-server --test schema`
Expected: FAIL — relation "accounts" does not exist.

- [ ] **Step 3: Write the migration**

`migrations/0001_init.sql` — the spec's §7.1 schema, with `items.deleted_at`
kept as the server-side tombstone marker (clients no longer store tombstones,
spec §7.1):

```sql
CREATE TABLE accounts (
  id                UUID PRIMARY KEY,
  email_normalized  TEXT NOT NULL UNIQUE,
  status            TEXT NOT NULL CHECK (status IN ('invited', 'active', 'disabled')),
  kdf_algorithm     TEXT,
  kdf_memory_kib    INTEGER,
  kdf_iterations    INTEGER,
  kdf_parallelism   INTEGER,
  kdf_salt          BYTEA,
  auth_verifier     TEXT,
  invite_hash       BYTEA,
  invite_expires_at TIMESTAMPTZ,
  created_at        TIMESTAMPTZ NOT NULL,
  activated_at      TIMESTAMPTZ
);

CREATE TABLE vaults (
  id              UUID PRIMARY KEY,
  account_id      UUID NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
  header          BYTEA NOT NULL,
  header_revision BIGINT NOT NULL,
  key_scheme      SMALLINT NOT NULL,
  revision        BIGINT NOT NULL DEFAULT 0,
  created_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE items (
  vault_id   UUID NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  item_id    UUID NOT NULL,
  overview   BYTEA,
  details    BYTEA,
  deleted_at TIMESTAMPTZ,
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
CREATE INDEX devices_by_account ON devices (account_id);

CREATE TABLE sessions (
  token_hash BYTEA PRIMARY KEY,
  account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  device_id  UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_by_device ON sessions (device_id);

CREATE TABLE login_attempts (
  key           TEXT PRIMARY KEY,
  failures      INTEGER NOT NULL,
  window_start  TIMESTAMPTZ NOT NULL,
  blocked_until TIMESTAMPTZ
);
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p havenkeys-server --test schema`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server/migrations crates/havenkeys-server/tests/schema.rs
git commit -m "feat(server): the account, vault, item, device and session schema"
```

---

## Task 3: Invites and the admin CLI

**Files:**
- Create: `crates/havenkeys-server/src/invite.rs`, `src/admin.rs`
- Modify: `crates/havenkeys-server/src/main.rs`, `src/lib.rs`
- Test: unit tests in `src/invite.rs`, integration in `tests/admin.rs`

**Interfaces:**
- Produces: `invite::Invite { server, email, account, secret }`,
  `invite::generate_secret() -> Zeroizing<String>`,
  `invite::encode(&Invite) -> String` (`HKINV1-…`),
  `invite::decode(&str) -> Result<Invite, ApiError>`,
  `invite::hash(&str) -> [u8; 32]`,
  `admin::run(AdminCommand, &PgPool) -> anyhow-free Result<(), String>`.

- [ ] **Step 1: Write the failing unit test**

In `src/invite.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invite_round_trips_through_its_string() {
        let invite = Invite {
            server: "https://vault.example.com".into(),
            email: "user@example.com".into(),
            account: uuid::Uuid::from_u128(1),
            secret: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        };
        let encoded = encode(&invite);
        assert!(encoded.starts_with("HKINV1-"));
        assert_eq!(decode(&encoded).unwrap(), invite);
    }

    #[test]
    fn a_mangled_invite_is_refused_rather_than_guessed() {
        for bad in ["", "HKINV1-", "HKINV2-abc", "HKINV1-!!!", "not-an-invite"] {
            assert!(decode(bad).is_err(), "should have refused {bad:?}");
        }
    }

    #[test]
    fn two_generated_secrets_differ() {
        assert_ne!(*generate_secret(), *generate_secret());
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p havenkeys-server --lib invite`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement `invite.rs`**

```rust
//! The single-use string that authorizes activation.
//!
//! The secret is 128 bits from the CSPRNG and is stored only as SHA-256: the
//! input is already high-entropy, so a password hash would buy nothing. It
//! authorizes creating a vault for an account; it protects no data.

use crate::error::ApiError;
use data_encoding::BASE64URL_NOPAD;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

const PREFIX: &str = "HKINV1-";
const SECRET_BYTES: usize = 16;
pub const INVITE_TTL_DAYS: i64 = 7;
const MAX_INVITE_CHARS: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    pub server: String,
    pub email: String,
    pub account: Uuid,
    pub secret: String,
}

pub fn generate_secret() -> Zeroizing<String> {
    let mut bytes = Zeroizing::new([0u8; SECRET_BYTES]);
    rand::thread_rng().fill_bytes(bytes.as_mut());
    Zeroizing::new(BASE64URL_NOPAD.encode(bytes.as_ref()))
}

pub fn encode(invite: &Invite) -> String {
    let json = serde_json::to_vec(invite).expect("invite serializes");
    format!("{PREFIX}{}", BASE64URL_NOPAD.encode(&json))
}

pub fn decode(raw: &str) -> Result<Invite, ApiError> {
    let raw = raw.trim();
    if raw.len() > MAX_INVITE_CHARS {
        return Err(ApiError::InvalidRequest("invite is not valid"));
    }
    let body = raw
        .strip_prefix(PREFIX)
        .ok_or(ApiError::InvalidRequest("invite is not valid"))?;
    let json = BASE64URL_NOPAD
        .decode(body.as_bytes())
        .map_err(|_| ApiError::InvalidRequest("invite is not valid"))?;
    serde_json::from_slice(&json).map_err(|_| ApiError::InvalidRequest("invite is not valid"))
}

pub fn hash(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p havenkeys-server --lib invite`
Expected: PASS.

- [ ] **Step 5: Write the failing CLI test**

`tests/admin.rs`:

```rust
mod support;

use havenkeys_server::admin::{self, AdminCommand};

#[tokio::test]
async fn new_account_stores_only_the_invite_hash() {
    let server = support::TestServer::start().await;
    let printed = admin::run(
        AdminCommand::NewAccount {
            email: "  User@Example.COM ".into(),
            server_url: "https://vault.example.com".into(),
        },
        server.pool(),
    )
    .await
    .unwrap();

    let invite = havenkeys_server::invite::decode(printed.trim()).unwrap();
    assert_eq!(invite.email, "user@example.com", "the email is normalized once, by the server");

    let (email, status, stored): (String, String, Option<Vec<u8>>) =
        sqlx::query_as("SELECT email_normalized, status, invite_hash FROM accounts WHERE id = $1")
            .bind(invite.account)
            .fetch_one(server.pool())
            .await
            .unwrap();
    assert_eq!(email, "user@example.com");
    assert_eq!(status, "invited");
    assert_eq!(stored.unwrap(), havenkeys_server::invite::hash(&invite.secret).to_vec());

    // The secret itself is nowhere in the database.
    let raw: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE invite_hash::text LIKE $1")
        .bind(format!("%{}%", invite.secret))
        .fetch_one(server.pool())
        .await
        .unwrap();
    assert_eq!(raw, 0);

    server.cleanup().await;
}

#[tokio::test]
async fn a_second_account_for_the_same_email_is_refused() {
    let server = support::TestServer::start().await;
    let cmd = || AdminCommand::NewAccount {
        email: "dup@example.com".into(),
        server_url: "https://vault.example.com".into(),
    };
    admin::run(cmd(), server.pool()).await.unwrap();
    assert!(admin::run(cmd(), server.pool()).await.is_err());
    server.cleanup().await;
}
```

- [ ] **Step 6: Implement `admin.rs` and wire the subcommand**

```rust
//! Account provisioning. There is no public signup (spec §2), so this CLI is
//! the only way an account comes into existence.

use crate::invite::{self, Invite, INVITE_TTL_DAYS};
use chrono::{Duration, Utc};
use clap::Subcommand;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum AdminCommand {
    /// Create an account and print its single-use invite string.
    NewAccount {
        #[arg(long)]
        email: String,
        /// The public URL clients should talk to, carried in the invite.
        #[arg(long = "server-url")]
        server_url: String,
    },
    /// List accounts: id, email, status, activation date.
    ListAccounts,
    /// Delete an account and everything it owns. Irreversible.
    DeleteAccount {
        #[arg(long)]
        email: String,
    },
}

/// Returns what the operator should see on stdout. Returning it rather than
/// printing keeps the invite out of the library's own logs and lets the test
/// read it.
pub async fn run(cmd: AdminCommand, pool: &PgPool) -> Result<String, String> {
    match cmd {
        AdminCommand::NewAccount { email, server_url } => {
            let email = crate::email::normalize(&email)?;
            let account = Uuid::new_v4();
            let secret = invite::generate_secret();
            let now = Utc::now();
            sqlx::query(
                "INSERT INTO accounts (id, email_normalized, status, invite_hash, invite_expires_at, created_at)
                 VALUES ($1, $2, 'invited', $3, $4, $5)",
            )
            .bind(account)
            .bind(&email)
            .bind(invite::hash(&secret).to_vec())
            .bind(now + Duration::days(INVITE_TTL_DAYS))
            .bind(now)
            .execute(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db) if db.is_unique_violation() => {
                    "an account with that email already exists".to_string()
                }
                _ => "could not create the account".to_string(),
            })?;
            Ok(invite::encode(&Invite {
                server: server_url,
                email,
                account,
                secret: secret.to_string(),
            }))
        }
        AdminCommand::ListAccounts => {
            let rows: Vec<(Uuid, String, String, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
                "SELECT id, email_normalized, status, activated_at FROM accounts ORDER BY created_at",
            )
            .fetch_all(pool)
            .await
            .map_err(|_| "could not list accounts".to_string())?;
            Ok(rows
                .into_iter()
                .map(|(id, email, status, at)| {
                    format!("{id}  {email}  {status}  {}", at.map(|t| t.to_rfc3339()).unwrap_or_default())
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        AdminCommand::DeleteAccount { email } => {
            let email = crate::email::normalize(&email)?;
            let done = sqlx::query("DELETE FROM accounts WHERE email_normalized = $1")
                .bind(&email)
                .execute(pool)
                .await
                .map_err(|_| "could not delete the account".to_string())?;
            if done.rows_affected() == 0 {
                return Err("no such account".into());
            }
            Ok("deleted".into())
        }
    }
}
```

Add `src/email.rs` with `normalize(&str) -> Result<String, String>`: trim, NFC
(`unicode-normalization`, already a workspace dependency of the core), lowercase,
reject empty/whitespace/no-`@`/over 254 chars — the same rules as
`havenkeys-core::account::NormalizedEmail`, restated here because the server
must not depend on the client crate. Its unit test asserts the two agree on
`"  José@Example.COM "` → `"josé@example.com"` with the NFC form.

`main.rs` gains `Cli { #[command(subcommand)] command: Option<Command> }` where
`Command::Admin(AdminCommand)` runs the CLI against the pool and prints the
returned string; no subcommand means "serve".

- [ ] **Step 7: Run the tests**

Run: `cargo test -p havenkeys-server`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): invites and the account admin CLI"
```

---

## Task 4: Activation

**Files:**
- Create: `crates/havenkeys-server/src/routes/accounts.rs`, `src/b64.rs`
- Modify: `src/routes/mod.rs`
- Test: `crates/havenkeys-server/tests/auth.rs`

**Interfaces:**
- Consumes: `invite::{decode, hash}`, `email::normalize`.
- Produces: `POST /v1/accounts/activate`, the `Blob` newtype
  (`b64::Blob(Vec<u8>)`, base64 in JSON, refuses anything over
  `MAX_BLOB_BYTES`), and `auth::hash_auth_key(&str) -> String` (PHC).

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn activation_creates_the_vault_and_burns_the_invite() {
    let server = support::TestServer::start().await;
    let invite = support::new_invite(&server, "user@example.com").await;
    let vault_id = uuid::Uuid::new_v4();

    let body = serde_json::json!({
        "email": "user@example.com",
        "invite": invite,
        "kdf": {"algorithm": "argon2id", "memoryKib": 131072, "iterations": 4, "parallelism": 4,
                "salt": data_encoding::BASE64.encode(&[3u8; 16])},
        "authKey": data_encoding::BASE64.encode(&[9u8; 32]),
        "vaultId": vault_id,
        "header": data_encoding::BASE64.encode(b"header-bytes"),
        "keyScheme": 3,
    });

    let res = server.post("/v1/accounts/activate").json(&body).send().await.unwrap();
    assert_eq!(res.status(), 200);

    let (status, verifier): (String, Option<String>) =
        sqlx::query_as("SELECT status, auth_verifier FROM accounts WHERE email_normalized = 'user@example.com'")
            .fetch_one(server.pool()).await.unwrap();
    assert_eq!(status, "active");
    let verifier = verifier.unwrap();
    assert!(verifier.starts_with("$argon2id$"), "the auth key is stored hashed, never as received");
    assert!(!verifier.contains(&data_encoding::BASE64.encode(&[9u8; 32])));

    // Single use: the same invite cannot activate again.
    let again = server.post("/v1/accounts/activate").json(&body).send().await.unwrap();
    assert_eq!(again.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn activation_refuses_a_wrong_or_expired_invite() { /* wrong secret → 400;
    invite_expires_at moved into the past → 400; both leave status 'invited' */ }
```

`support::new_invite` calls `admin::run(AdminCommand::NewAccount { .. })` and
returns the printed string.

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p havenkeys-server --test auth`
Expected: FAIL — 404 from the router.

- [ ] **Step 3: Implement `b64.rs`**

```rust
//! Base64 blobs on the wire.
//!
//! Length is checked while decoding, before the bytes are kept, so an
//! oversized blob costs one decode and not a stored allocation. The server
//! never looks inside: these are AES-256-GCM ciphertexts it cannot open.

use data_encoding::BASE64;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, PartialEq, Eq)]
pub struct Blob(pub Vec<u8>);

impl std::fmt::Debug for Blob {
    /// Never print ciphertext, not even truncated.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Blob({} bytes)", self.0.len())
    }
}

impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        if raw.len() > crate::limits::MAX_BLOB_BYTES / 3 * 4 + 4 {
            return Err(D::Error::custom("blob is too large"));
        }
        let bytes = BASE64.decode(raw.as_bytes()).map_err(|_| D::Error::custom("blob is not base64"))?;
        if bytes.len() > crate::limits::MAX_BLOB_BYTES {
            return Err(D::Error::custom("blob is too large"));
        }
        Ok(Self(bytes))
    }
}
```

- [ ] **Step 4: Implement the route**

`src/routes/accounts.rs`:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateRequest {
    email: String,
    invite: String,
    kdf: KdfDto,
    auth_key: String,
    vault_id: Uuid,
    header: Blob,
    key_scheme: i16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KdfDto {
    algorithm: String,
    memory_kib: i32,
    iterations: i32,
    parallelism: i32,
    salt: String,
}
```

Handler, in one transaction:

1. `email::normalize`; `invite::decode`; the decoded invite's `email` and
   `account` must match the request's email and the stored row — a mismatch is
   `InvalidRequest("invite is not valid")`.
2. `SELECT … FROM accounts WHERE id = $1 AND email_normalized = $2 FOR UPDATE`.
   Row missing, `status <> 'invited'`, `invite_hash` mismatch (constant-time
   compare over the SHA-256), or `invite_expires_at < now()` → the same
   `InvalidRequest("invite is not valid")`. One message for every case: the
   endpoint must not tell a prober which part was wrong.
3. Validate `algorithm == "argon2id"` and the KDF cost against the core's
   accepted ranges (19 456–1 048 576 KiB, 2–16 iterations, 1–16 parallelism,
   16-byte salt). These come from the client, but a header the server hands to
   a *new* device should not be able to name parameters the client would
   refuse anyway; checking here fails fast and keeps the stored row sane.
4. `key_scheme` must be exactly 3 (spec §4), `header` at most
   `MAX_HEADER_BYTES`, `auth_key` exactly 32 bytes when base64-decoded.
5. `auth_verifier = Argon2id(m=19456, t=2, p=1).hash_password(auth_key)` —
   modest parameters on purpose: the input is 256 bits of HKDF output, so the
   hash protects a stolen database, and heavy parameters here would only be a
   denial-of-service lever on login (spec §7.3).
6. `UPDATE accounts SET status='active', kdf_*, auth_verifier, invite_hash=NULL,
   invite_expires_at=NULL, activated_at=now()`.
7. `INSERT INTO vaults (id, account_id, header, header_revision, key_scheme,
   revision, created_at) VALUES ($1,$2,$3,0,3,0,now())`.
8. Respond `200 {"accountId": …, "vaultId": …}`. No session: the client logs in
   next, with the auth key it already holds.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-server --test auth`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): activation from a single-use invite"
```

---

## Task 5: Sessions, login, and rate limiting

**Files:**
- Create: `crates/havenkeys-server/src/auth/mod.rs`, `src/auth/rate_limit.rs`,
  `src/routes/auth.rs`
- Modify: `src/routes/mod.rs`
- Test: `crates/havenkeys-server/tests/auth.rs`

**Interfaces:**
- Produces: `auth::Session { account_id, device_id, vault_id }` as an axum
  extractor (`FromRequestParts`), `auth::issue_token(&PgPool, account, device)
  -> Result<(String, DateTime<Utc>), ApiError>`,
  `rate_limit::check(&PgPool, &str) -> Result<(), ApiError>`,
  `rate_limit::record_failure(&PgPool, &str)`, `rate_limit::clear(&PgPool, &str)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn login_returns_a_session_and_the_vault_id() {
    let server = support::TestServer::start().await;
    let account = support::activate(&server, "user@example.com", &[9u8; 32]).await;

    let res = server.post("/v1/auth/login").json(&serde_json::json!({
        "email": "user@example.com",
        "authKey": data_encoding::BASE64.encode(&[9u8; 32]),
        "deviceId": uuid::Uuid::new_v4(),
        "deviceName": "Desktop",
    })).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["vaultId"], serde_json::json!(account.vault_id));
    let token = body["token"].as_str().unwrap().to_string();

    // The token is stored only as its SHA-256.
    let hashes: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_hash FROM sessions").fetch_all(server.pool()).await.unwrap();
    assert_eq!(hashes.len(), 1);
    assert_eq!(hashes[0], sha2::Sha256::digest(token.as_bytes()).to_vec());
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_auth_key_an_unknown_email_and_a_revoked_device_all_answer_the_same() { /* 401, identical body */ }

#[tokio::test]
async fn auth_params_answers_uniformly_for_an_unknown_email() {
    // Same shape for a real and an unknown account; the unknown one's salt is
    // stable across calls (HKDF over the server secret) so a prober cannot
    // tell them apart by asking twice.
}

#[tokio::test]
async fn five_failures_block_the_account_and_a_success_clears_the_counter() { /* 6th → 429 */ }

#[tokio::test]
async fn an_expired_or_logged_out_token_is_refused() { /* expires_at into the past → 401 */ }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p havenkeys-server --test auth`
Expected: FAIL — 404.

- [ ] **Step 3: Implement `auth/mod.rs`**

Key pieces:

```rust
/// A 32-byte opaque bearer token. Stored as SHA-256 only, so a database dump
/// cannot be replayed as a live session.
pub async fn issue_token(pool: &PgPool, account: Uuid, device: Uuid)
    -> Result<(Zeroizing<String>, DateTime<Utc>), ApiError>
{
    let mut raw = Zeroizing::new([0u8; 32]);
    rand::thread_rng().fill_bytes(raw.as_mut());
    let token = Zeroizing::new(BASE64URL_NOPAD.encode(raw.as_ref()));
    let expires = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
    sqlx::query("INSERT INTO sessions (token_hash, account_id, device_id, created_at, expires_at) VALUES ($1,$2,$3,now(),$4)")
        .bind(Sha256::digest(token.as_bytes()).to_vec())
        .bind(account).bind(device).bind(expires)
        .execute(pool).await?;
    Ok((token, expires))
}

/// Identity for an authenticated request. Built only from the token: the
/// account and vault in a request body are never read (spec §7.3).
pub struct Session {
    pub account_id: Uuid,
    pub device_id: Uuid,
    pub vault_id: Uuid,
}

impl FromRequestParts<AppState> for Session {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let raw = parts.headers.get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        if raw.len() > 128 { return Err(ApiError::Unauthorized); }
        let row: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
            "SELECT s.account_id, s.device_id, v.id
               FROM sessions s
               JOIN accounts a ON a.id = s.account_id
               JOIN devices  d ON d.id = s.device_id
               JOIN vaults   v ON v.account_id = s.account_id
              WHERE s.token_hash = $1 AND s.expires_at > now()
                AND a.status = 'active' AND d.revoked_at IS NULL",
        )
        .bind(Sha256::digest(raw.as_bytes()).to_vec())
        .fetch_optional(&state.pool).await?;
        let (account_id, device_id, vault_id) = row.ok_or(ApiError::Unauthorized)?;
        sqlx::query("UPDATE devices SET last_seen_at = now() WHERE id = $1")
            .bind(device_id).execute(&state.pool).await?;
        Ok(Session { account_id, device_id, vault_id })
    }
}
```

The join is the whole authorization story: one query proves the token is live,
the account is active, the device is not revoked, and which vault the request
may touch. No handler repeats any of it.

- [ ] **Step 4: Implement `auth/rate_limit.rs`**

```rust
/// After 5 failures in a 15-minute window, block for 1, then 5, then 30
/// minutes. Counters are keyed "acct:<uuid>" and "ip:<addr>" so one noisy
/// network cannot lock out an account it does not own, and one account cannot
/// be brute-forced from many addresses.
const MAX_FAILURES: i32 = 5;
const WINDOW_MINUTES: i64 = 15;

pub fn block_for(failures: i32) -> Duration {
    match failures {
        0..=5 => Duration::minutes(1),
        6..=9 => Duration::minutes(5),
        _ => Duration::minutes(30),
    }
}
```

`check` reads the row and returns `ApiError::RateLimited` when
`blocked_until > now()`. `record_failure` upserts, resets the window when it
has elapsed, and sets `blocked_until` once `failures >= MAX_FAILURES`. `clear`
deletes the row on success.

- [ ] **Step 5: Implement `routes/auth.rs`**

* `POST /v1/auth/params { email }` → `{ accountId, kdf: { algorithm, memoryKib,
  iterations, parallelism, salt } }`. For an unknown or unactivated email:
  `accountId = Uuid::from_bytes(HKDF(server_secret, salt=b"havenkeys/params/account", info=email)[..16])`,
  `salt = HKDF(server_secret, salt=b"havenkeys/params/salt", info=email)[..16]`,
  and the default cost. Stable per email, unguessable without the secret, and
  indistinguishable in shape from a real answer.
* `POST /v1/auth/login { email, authKey, deviceId, deviceName }` →
  `{ token, expiresAt, vaultId }`:
  1. `rate_limit::check` on `acct:<derived-or-real-uuid>` and on `ip:<peer>`
     (the peer address comes from `ConnectInfo<SocketAddr>`; when the service
     runs behind Railway's proxy the left-most `X-Forwarded-For` entry is used
     — configured explicitly, never trusted by default).
  2. Look the account up. When it is missing or not active, verify the given
     auth key against a **fixed dummy PHC string** computed once at startup, so
     the timing matches a real verification, then answer `Unauthorized`.
  3. Verify `authKey` against `auth_verifier` with `argon2::PasswordVerifier`.
  4. `deviceName`: at most 64 characters after trimming, non-empty, control
     characters rejected. `deviceId` must be a UUID the client supplies;
     insert the device if unknown, refuse when the account already has
     `MAX_DEVICES_PER_ACCOUNT` devices, refuse when the device is revoked.
  5. Delete this device's existing sessions, then `issue_token`.
  6. `rate_limit::clear`.
* `POST /v1/auth/logout` (authenticated) deletes the calling token's row.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p havenkeys-server --test auth`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): sessions, login, and login rate limiting"
```

---

## Task 6: The vault header

**Files:**
- Create: `crates/havenkeys-server/src/routes/vault.rs`
- Modify: `src/routes/mod.rs`
- Test: `crates/havenkeys-server/tests/sync.rs`

**Interfaces:**
- Produces: `GET /v1/vault/header → { header, headerRevision, keyScheme }`,
  `PUT /v1/vault/header { header, headerRevision, keyScheme } → 200 | 409`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn the_header_is_returned_byte_for_byte() { /* activate with b"header-bytes", GET returns it */ }

#[tokio::test]
async fn a_header_write_must_be_exactly_one_revision_ahead() {
    // revision + 1 → 200 and the stored bytes change;
    // the same revision again → 409 and the stored bytes do NOT change;
    // revision + 2 → 409.
}

#[tokio::test]
async fn a_lower_key_scheme_is_refused() { /* keyScheme 2 → 400, stored row unchanged */ }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p havenkeys-server --test sync`
Expected: FAIL — 404.

- [ ] **Step 3: Implement the routes**

`PUT` runs one transaction: `SELECT header_revision, key_scheme FROM vaults
WHERE id = $1 FOR UPDATE`, then refuse unless `header_revision == stored + 1`
(409) and `key_scheme >= stored` (400), then update. The vault id comes from
`Session`, never from the body — a body field named `vaultId` is an unknown
field and therefore a 400.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-server --test sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): the vault header, with the revision and scheme guards"
```

---

## Task 7: Pull

**Files:**
- Create: `crates/havenkeys-server/src/routes/sync.rs`
- Modify: `src/routes/mod.rs`
- Test: `crates/havenkeys-server/tests/sync.rs`

**Interfaces:**
- Produces: `GET /v1/sync?since=<cursor>` →
  `{ cursor, hasMore, changes: [ { itemId, revision, overview, details, deleted } ] }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn a_pull_from_zero_returns_every_live_item_and_every_deletion() {
    // Seed three items and delete one through POST /v1/items, then pull:
    // two live changes with blobs, one with deleted: true and null blobs.
}

#[tokio::test]
async fn a_pull_pages_and_the_cursor_advances() {
    // 600 items in two batches (500 is the write limit), pull since=0:
    // hasMore true and 500 changes; pulling since that cursor returns the rest
    // and hasMore false.
}

#[tokio::test]
async fn a_cursor_in_the_future_or_below_zero_is_refused() { /* since=-1 → 400 */ }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p havenkeys-server --test sync`
Expected: FAIL.

- [ ] **Step 3: Implement the route**

```sql
SELECT item_id, revision, overview, details, deleted_at
  FROM items
 WHERE vault_id = $1 AND revision > $2
 ORDER BY revision, item_id
 LIMIT $3
```

`LIMIT` is `MAX_PULL_PAGE + 1`; the extra row only sets `hasMore`. `cursor` is
the last returned row's `revision`, or `vaults.revision` when the page is
empty, so an idle client converges on the vault's cursor and stops asking for
the same rows. `deleted` is `deleted_at IS NOT NULL`; a deleted row's blobs are
already NULL in the database and serialize as `null`.

Ordering by `(revision, item_id)` matters: rows written in one batch share a
revision, and a stable secondary key is what makes paging across that boundary
safe.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-server --test sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): the pull endpoint with a paged, monotonic cursor"
```

---

## Task 8: Writes

**Files:**
- Create: `crates/havenkeys-server/src/routes/items.rs`
- Modify: `src/routes/mod.rs`
- Test: `crates/havenkeys-server/tests/sync.rs`

**Interfaces:**
- Produces: `POST /v1/items { changes: [ … ] }` →
  `200 { cursor, applied: [ { itemId, revision } ] }` or
  `409 { conflicts: [ { itemId, revision } ] }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn a_batch_is_applied_under_one_revision() {
    // Three creates in one request: all three come back with the same
    // revision, and the vault cursor advanced by exactly one.
}

#[tokio::test]
async fn a_stale_base_revision_refuses_the_whole_batch() {
    // Item A written twice by another session, then a batch of [A (stale), B (new)]:
    // 409 naming A with its current revision, and B must NOT exist afterwards.
}

#[tokio::test]
async fn creating_an_item_that_already_exists_is_a_conflict_not_an_overwrite() {
    // baseRevision: null for an item id the vault already has → 409.
}

#[tokio::test]
async fn a_deletion_clears_the_blobs_and_keeps_the_row() {
    // overview and details NULL, deleted_at set, revision bumped.
}

#[tokio::test]
async fn a_change_that_is_neither_a_write_nor_a_deletion_is_refused() {
    // { itemId, baseRevision } with no blobs and no deleted flag → 400;
    // { itemId, deleted: true, overview: "…" } → 400.
}

#[tokio::test]
async fn a_batch_over_the_limit_is_refused() { /* 501 changes → 400 */ }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p havenkeys-server --test sync`
Expected: FAIL.

- [ ] **Step 3: Implement the route**

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRequest {
    changes: Vec<Change>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    item_id: Uuid,
    /// The revision this client last saw, or `null` for an item it believes
    /// is new. Never trusted: it is compared against the stored row.
    base_revision: Option<i64>,
    #[serde(default)]
    overview: Option<Blob>,
    #[serde(default)]
    details: Option<Blob>,
    #[serde(default)]
    deleted: bool,
}
```

Handler:

1. `changes` non-empty and at most `MAX_CHANGES_PER_BATCH`; no duplicate
   `item_id` in one batch (a batch that names an item twice has no defined
   `baseRevision` for the second mention — 400, not a guess).
2. Each change is either a **write** (`!deleted`, both blobs present) or a
   **deletion** (`deleted`, both blobs absent). Anything else is 400.
3. One transaction, `SELECT revision FROM vaults WHERE id = $1 FOR UPDATE`
   first — the row lock serializes concurrent batches for this vault, which is
   what makes the revision assignment and the conflict check a single decision.
4. `SELECT item_id, revision, deleted_at FROM items WHERE vault_id = $1 AND
   item_id = ANY($2)`. A change conflicts when: the row exists and
   `base_revision != Some(row.revision)`; the row is missing and
   `base_revision.is_some()`; or the row exists, is already deleted, and the
   change is a deletion (deleting a deleted item is a no-op the client should
   learn about by pulling). Any conflict → roll back and answer 409 with every
   conflicting `{ itemId, revision }`, where a missing row reports `revision: null`.
5. `UPDATE vaults SET revision = revision + 1 WHERE id = $1 RETURNING revision`,
   then upsert each change with that revision (`ON CONFLICT (vault_id, item_id)
   DO UPDATE`), deletions setting `overview = NULL, details = NULL,
   deleted_at = now()`.
6. Commit, answer `200 { cursor, applied }` with the same revision on each row.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-server --test sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): optimistic per-item writes in one transaction"
```

---

## Task 9: Devices

**Files:**
- Create: `crates/havenkeys-server/src/routes/devices.rs`
- Modify: `src/routes/mod.rs`
- Test: `crates/havenkeys-server/tests/devices.rs`

**Interfaces:**
- Produces: `GET /v1/devices → [ { id, name, createdAt, lastSeenAt, current } ]`,
  `DELETE /v1/devices/:id → 204`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn the_device_list_marks_the_calling_device() { /* current: true for exactly one */ }

#[tokio::test]
async fn a_revoked_device_loses_its_session_immediately() {
    // Two logins; device B revoked by A; B's next authenticated call → 401,
    // and B logging in again → 401.
}

#[tokio::test]
async fn a_device_belonging_to_another_account_cannot_be_revoked() { /* 404, row untouched */ }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p havenkeys-server --test devices`
Expected: FAIL.

- [ ] **Step 3: Implement the routes**

`DELETE` sets `revoked_at = now()` **and** deletes that device's sessions, in
one transaction, scoped by `account_id` from the session — so revocation takes
effect on the very next request rather than at token expiry. Revoking a device
that is not the caller's account answers 404, which is also what a non-existent
device answers: the list of device ids is not an oracle.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-server --test devices`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): the device list and immediate revocation"
```

---

## Task 10: The isolation, limits and logging suites

**Files:**
- Create: `crates/havenkeys-server/tests/isolation.rs`, `tests/limits.rs`,
  `tests/no_logging.rs`
- Modify: `src/routes/mod.rs` (tracing layer, CORS), `src/main.rs`

**Interfaces:**
- Consumes: every route built so far.

- [ ] **Step 1: Write the isolation suite**

```rust
/// The central guarantee: an authenticated account reaches nothing that
/// belongs to another one, on every route that takes a session.
#[tokio::test]
async fn account_a_cannot_touch_account_b() {
    let server = support::TestServer::start().await;
    let a = support::activated_session(&server, "a@example.com").await;
    let b = support::activated_session(&server, "b@example.com").await;

    let b_item = support::create_item(&server, &b, b"b-overview", b"b-details").await;

    // Pull: A sees only its own vault.
    let changes = support::pull(&server, &a, 0).await;
    assert!(changes.iter().all(|c| c["itemId"] != serde_json::json!(b_item)));

    // Write: A writing B's item id creates a *new* item in A's own vault and
    // leaves B's untouched — item ids are unique per vault, not globally.
    support::write_item(&server, &a, b_item, None, b"a-overview", b"a-details").await;
    let b_after = support::pull(&server, &b, 0).await;
    assert_eq!(b_after[0]["overview"], serde_json::json!(BASE64.encode(b"b-overview")));

    // Header: A's PUT does not move B's header.
    // Devices: A cannot list or revoke B's devices.
    // Logout: A's token cannot end B's session.
    server.cleanup().await;
}
```

- [ ] **Step 2: Write the limits suite**

Oversized body → 413; oversized blob → 400; unknown field → 400; non-UUID id →
400; `since` not a number → 400; a header over 64 KiB → 400; a 65th device →
400; every authenticated route without a token → 401 and with a garbage token
→ 401.

- [ ] **Step 3: Write the logging suite**

Following `crates/havenkeys-core/tests/no_logging.rs`: install a
`tracing_subscriber` writing into a shared `Vec<u8>`, run activation, login, a
write, a pull and a revocation, then assert the captured output contains none
of the token, the auth key, the invite string, the blob bytes (base64 and raw),
the header bytes, or the email — and that it does contain the account UUID, so
the test proves logging still works rather than passing on silence.

- [ ] **Step 4: Make them pass**

Expected fixes: a `TraceLayer` configured to log method, path, status and
latency only — never the query string, never headers; `MakeSpan` carrying
`request_id`, `account_id` and `device_id`; CORS denied unless
`HAVENKEYS_CORS_ORIGIN` is set, in which case exactly that origin with
credentials allowed and a documented rationale; `Json` rejections mapped to
`ApiError::InvalidRequest("request is not valid")` so serde's message (which
quotes the offending input) never reaches the client.

- [ ] **Step 5: Run everything**

Run: `scripts/test-server.sh` then
`cargo clippy -p havenkeys-server --all-targets -- -D warnings` and `cargo fmt --check`.
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-server
git commit -m "test(server): account isolation, limits, and a no-logging guard"
```

---

## Task 11: Deployment, audit and docs

**Files:**
- Create: `crates/havenkeys-server/Dockerfile`, `railway.json`,
  `docs/deployment.md`
- Modify: `docs/roadmap.md`, `deny.toml`, `README.md` (a line naming the
  server, without claiming it is deployed)

- [ ] **Step 1: Write the Dockerfile**

Multi-stage: `rust:1.88-slim` builder with `cargo build --release -p
havenkeys-server`, then `debian:bookworm-slim` with `ca-certificates` and a
non-root user. `CMD ["havenkeys-server"]`, `EXPOSE 8080`. No build secrets, no
`DATABASE_URL` at build time — the crate uses runtime sqlx queries precisely so
the image builds without a database.

- [ ] **Step 2: Write `railway.json`**

Dockerfile builder, `healthcheckPath: "/v1/health"`, restart on failure.
`DATABASE_URL` from the Postgres plugin, `SERVER_SECRET` generated once with
`openssl rand -base64 32` and stored as a Railway variable.

- [ ] **Step 3: Write `docs/deployment.md`**

Sections: environment variables and how to generate the secret; first deploy;
creating the first account (`havenkeys-server admin new-account`); **backups**
— that Railway's scheduled backups are not on by default, the exact steps to
enable them, and a restore drill (`pg_dump` to a fresh database, point a test
server at it, confirm a client can pull); and the statement, from spec §9,
that the local replica is *not* a backup, so an untested backup means the vault
has none.

- [ ] **Step 4: Run the dependency audit**

Run: `cargo audit` and `cargo deny check` (add licence entries to `deny.toml`
only for what the new tree actually pulls in, each with a one-line note).
Expected: clean, or an investigated and documented exception.

- [ ] **Step 5: Update the roadmap**

`docs/roadmap.md` §3 step 2: code and tests done; the deploy and the restore
drill remain, and are listed as **blocking before anyone stores a real vault**,
with a pointer to `docs/deployment.md`.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-server docs/deployment.md docs/roadmap.md deny.toml README.md
git commit -m "feat(server): deployment, dependency audit and the backup runbook"
```

---

## Self-review against the spec

| Spec requirement | Task |
|---|---|
| §7.1 schema, tombstone as a NULL-blob row | 2 |
| §5.1 invite: 128-bit secret, SHA-256, single use, 7 days | 3, 4 |
| §5.2 activation records kdf, salt, auth verifier, vault, header | 4 |
| §5.3 `auth/params` then `login`; one message for every failure | 5 |
| §6 tokens: 32 bytes, SHA-256 at rest, 24 h, revocation | 5, 9 |
| §7.2 every route | 4–9 |
| §7.3 identity from the token, `deny_unknown_fields`, sizes, header `revision + 1`, no scheme downgrade, rate limiting, uniform `auth/params`, blobs never parsed | 4–8, 10 |
| §7.2 (new) per-item `baseRevision`, whole-batch refusal, named conflicts | 8 |
| §7.4 one transaction, one revision bump per batch | 8 |
| §7.5 logging | 10 |
| §7.6 CORS denied by default | 10 |
| §12 isolation, auth failures, limits, rate limiting, invite, `no_logging`, real Postgres | 10 (and each route's own task) |
| §10 Railway, health check, `PORT`, migrations at startup, tested backups | 1, 11 |

**Not covered here, by design:** the client half (`havenkeys-sync-client`) is
step 3 of the spec's §13 and gets its own plan; nothing in this plan may
reference it.
