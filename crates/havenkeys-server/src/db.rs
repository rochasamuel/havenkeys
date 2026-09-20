//! The connection pool and the migration runner.
//!
//! Migrations are embedded in the binary and applied in order inside one
//! transaction each, recorded in `schema_migrations`. The runner is small on
//! purpose: it is the only code that may change the shape of the database, so
//! it should be readable in one sitting (CLAUDE.md "easy to audit").

use deadpool_postgres::{Config as PoolConfig, ManagerConfig, Pool, RecyclingMethod, Runtime};
use std::fmt;
use tokio_postgres::NoTls;

/// Every migration, in the order they must run. Adding one means appending a
/// line here; editing a shipped one is not allowed — write the next.
const MIGRATIONS: &[(&str, &str)] = &[("0001_init", include_str!("../migrations/0001_init.sql"))];

#[derive(Debug)]
pub enum DbError {
    Config(String),
    Connect(String),
    Migrate(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // A connection string can carry a password, so only the kind of
            // failure is ever printed, never the string or the driver's
            // message about it.
            Self::Config(_) => f.write_str("the database URL is not valid"),
            Self::Connect(_) => f.write_str("could not connect to the database"),
            Self::Migrate(m) => write!(f, "could not apply migrations: {m}"),
        }
    }
}

/// Build a pool from a `postgres://` URL.
///
/// TLS is used unless the URL asks for `sslmode=disable`, which is what a
/// local container and a private-network deployment use. Certificates are
/// verified against the webpki roots; there is no "accept anything" mode.
pub fn connect(url: &str) -> Result<Pool, DbError> {
    let pg: tokio_postgres::Config = url
        .parse()
        .map_err(|_| DbError::Config("unparseable".into()))?;

    let mut cfg = PoolConfig::new();
    cfg.url = Some(url.to_string());
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    cfg.pool = Some(deadpool_postgres::PoolConfig::new(10));

    let wants_tls = !matches!(
        pg.get_ssl_mode(),
        tokio_postgres::config::SslMode::Disable
    );
    let pool = if wants_tls {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        cfg.create_pool(
            Some(Runtime::Tokio1),
            tokio_postgres_rustls::MakeRustlsConnect::new(tls_config),
        )
        .map_err(|e| DbError::Connect(e.to_string()))?
    } else {
        cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| DbError::Connect(e.to_string()))?
    };
    Ok(pool)
}

/// Apply every migration that has not run yet. Safe to call on every start.
pub async fn migrate(pool: &Pool) -> Result<(), DbError> {
    let mut client = pool
        .get()
        .await
        .map_err(|e| DbError::Connect(e.to_string()))?;
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
               version    TEXT PRIMARY KEY,
               applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
             )",
        )
        .await
        .map_err(|e| DbError::Migrate(e.to_string()))?;

    for (version, sql) in MIGRATIONS {
        let tx = client
            .transaction()
            .await
            .map_err(|e| DbError::Migrate(e.to_string()))?;
        // The lock serializes two instances starting at the same time: the
        // second waits, then finds the row and skips the migration.
        tx.batch_execute("LOCK TABLE schema_migrations IN EXCLUSIVE MODE")
            .await
            .map_err(|e| DbError::Migrate(e.to_string()))?;
        let done = tx
            .query_opt(
                "SELECT 1 FROM schema_migrations WHERE version = $1",
                &[version],
            )
            .await
            .map_err(|e| DbError::Migrate(e.to_string()))?
            .is_some();
        if !done {
            tx.batch_execute(sql)
                .await
                .map_err(|e| DbError::Migrate(format!("{version}: {e}")))?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES ($1)",
                &[version],
            )
            .await
            .map_err(|e| DbError::Migrate(e.to_string()))?;
            tracing::info!(version = *version, "migration applied");
        }
        tx.commit()
            .await
            .map_err(|e| DbError::Migrate(e.to_string()))?;
    }
    Ok(())
}
