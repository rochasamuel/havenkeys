//! The connection pool and the migration runner.
//!
//! Migrations are embedded in the binary and applied in order inside one
//! transaction each, recorded in `schema_migrations`. The runner is small on
//! purpose: it is the only code that may change the shape of the database, so
//! it should be readable in one sitting (CLAUDE.md "easy to audit").

use deadpool_postgres::{Config as PoolConfig, ManagerConfig, Pool, RecyclingMethod, Runtime};
use std::error::Error as _;
use std::fmt;
use std::time::{Duration, Instant};
use tokio_postgres::NoTls;

/// How long to keep trying the first connection before giving up.
///
/// A platform's private network is usually up before the container is, but
/// not always: on Railway it exists only at runtime and its DNS can lag the
/// first instruction the process runs. Crash-looping on that is both slower
/// to recover and much harder to read than waiting.
const CONNECT_DEADLINE: Duration = Duration::from_secs(60);

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
            Self::Connect(why) => write!(f, "could not connect to the database: {why}"),
            Self::Migrate(m) => write!(f, "could not apply migrations: {m}"),
        }
    }
}

/// Connect, waiting for the database to be reachable.
///
/// Returns a pool that has proved it can hand out a connection, so a failure
/// here is reported once, with a reason, instead of arriving later as a
/// request that mysteriously 500s.
pub async fn connect(url: &str) -> Result<Pool, DbError> {
    let pool = build_pool(url)?;
    let deadline = Instant::now() + CONNECT_DEADLINE;
    let mut wait = Duration::from_millis(250);
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let (failure, transient) = match pool.get().await {
            Ok(_) => return Ok(pool),
            Err(deadpool_postgres::PoolError::Backend(err)) => describe(&err),
            Err(_) => ("the connection pool gave up", false),
        };
        // A wrong password will still be wrong in a minute. Only the failures
        // a starting platform actually produces are worth waiting through.
        if !transient || Instant::now() >= deadline {
            return Err(DbError::Connect(failure.to_string()));
        }
        // Never the URL: it carries a password.
        tracing::warn!(attempt, reason = failure, "waiting for the database");
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(5));
    }
}

/// What went wrong, in words an operator can act on, and whether waiting
/// could fix it. The driver's own message can quote the host and the SQL, so
/// only this fixed set is ever shown.
fn describe(err: &tokio_postgres::Error) -> (&'static str, bool) {
    if let Some(code) = err.code() {
        return match code.code() {
            "28P01" | "28000" => ("the database rejected these credentials", false),
            "3D000" => ("that database does not exist on the server", false),
            "53300" => ("the database is out of connection slots", true),
            "57P03" => ("the database is still starting up", true),
            _ => ("the database refused the connection", false),
        };
    }
    let mut source = err.source();
    while let Some(cause) = source {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            let text = io.to_string();
            // A name that does not resolve yet is the usual shape of a
            // private network that has not finished coming up.
            if text.contains("lookup address") || text.contains("resolve") {
                return ("the database host name does not resolve yet", true);
            }
            return match io.kind() {
                std::io::ErrorKind::ConnectionRefused => {
                    ("nothing is listening at that address", true)
                }
                std::io::ErrorKind::TimedOut => ("the database did not answer in time", true),
                _ => ("the database could not be reached", true),
            };
        }
        source = cause.source();
    }
    ("the database could not be reached", true)
}

fn build_pool(url: &str) -> Result<Pool, DbError> {
    let pg: tokio_postgres::Config = url
        .parse()
        .map_err(|_| DbError::Config("unparseable".into()))?;

    let mut cfg = PoolConfig::new();
    cfg.url = Some(url.to_string());
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    cfg.pool = Some(deadpool_postgres::PoolConfig::new(10));

    let wants_tls = !matches!(pg.get_ssl_mode(), tokio_postgres::config::SslMode::Disable);
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
