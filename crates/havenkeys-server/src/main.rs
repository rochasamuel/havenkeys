//! `havenkeys-server` — serve the API, or run an admin command against the
//! same database.

use clap::{Parser, Subcommand};
use havenkeys_server::admin::{self, AdminCommand};
use havenkeys_server::{db, router, AppState, Config};

#[derive(Parser)]
#[command(name = "havenkeys-server", about = "HavenKeys sync server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP server (the default when no command is given).
    Serve,
    /// Provision and inspect accounts.
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "havenkeys_server=info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(message) => {
            eprintln!("configuration error: {message}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let pool = match db::connect(&config.database_url).await {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("{err}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Err(err) = db::migrate(&pool).await {
        eprintln!("{err}");
        return std::process::ExitCode::FAILURE;
    }

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config, pool).await,
        Command::Admin { command } => {
            match admin::run(command, &pool, config.server_secret).await {
                Ok(output) => {
                    // Printed, never logged: this line can carry an invite.
                    println!("{output}");
                    std::process::ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("{message}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
    }
}

async fn serve(config: Config, pool: deadpool_postgres::Pool) -> std::process::ExitCode {
    let mailer: Option<std::sync::Arc<dyn havenkeys_server::mail::Mailer>> = match &config.smtp {
        Some(smtp) => match havenkeys_server::mail::Smtp::new(&smtp.url, &smtp.from) {
            Ok(sender) => Some(std::sync::Arc::new(sender)),
            Err(message) => {
                eprintln!("configuration error: {message}");
                return std::process::ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let state = AppState {
        pool,
        server_secret: config.server_secret,
        trust_forwarded_for: config.trust_forwarded_for,
        cors_origin: config.cors_origin.clone(),
        locator: match &config.geoip_database {
            Some(path) => match havenkeys_server::locate::Locator::open(path) {
                Ok(locator) => Some(std::sync::Arc::new(locator)),
                Err(why) => {
                    tracing::warn!(reason = why.as_str(), "IP location disabled");
                    None
                }
            },
            None => None,
        },
        max_vault_bytes: havenkeys_server::limits::MAX_VAULT_BYTES,
        mailer,
        signup_url: if config.signup_open {
            config.public_url.clone()
        } else {
            None
        },
    };
    // Once at start, then daily: expired tombstones of deleted accounts
    // (spec 2026-10-05 §4.5), expired signup codes, abandoned signups and
    // the trial notices (spec 2026-10-07 §4.5, §5.6). A failure is logged by
    // kind and the next day tries again.
    let sweeping = state.pool.clone();
    let notifying = state.mailer.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        loop {
            every.tick().await;
            let db = match sweeping.get().await {
                Ok(db) => db,
                Err(_) => {
                    tracing::warn!(kind = "pool", "database error");
                    continue;
                }
            };
            if havenkeys_server::erase::sweep_tombstones(&db)
                .await
                .is_err()
            {
                tracing::warn!(kind = "sweep", "database error");
            }
            match havenkeys_server::billing::jobs::run_daily(&db, notifying.as_deref()).await {
                Ok(report) => tracing::info!(
                    codes = report.codes_swept,
                    signups = report.signups_abandoned,
                    ending = report.ending_notices,
                    ended = report.ended_notices,
                    "daily task"
                ),
                Err(_) => tracing::warn!(kind = "daily", "database error"),
            }
        }
    });
    // `::` takes both families where the host allows it, which matters
    // because a platform's proxy may reach the container over IPv6 only.
    // Where IPv6 is unavailable, IPv4 alone is still correct.
    let listener = match bind(config.port).await {
        Some(l) => l,
        None => {
            eprintln!("could not bind port {}", config.port);
            return std::process::ExitCode::FAILURE;
        }
    };
    tracing::info!(port = config.port, "havenkeys-server listening");
    // `ConnectInfo` is how the login route keys its per-address rate limit.
    match axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await
    {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::FAILURE,
    }
}

async fn bind(port: u16) -> Option<tokio::net::TcpListener> {
    let dual = std::net::SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, port));
    if let Ok(listener) = tokio::net::TcpListener::bind(dual).await {
        return Some(listener);
    }
    let v4 = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tokio::net::TcpListener::bind(v4).await.ok()
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
