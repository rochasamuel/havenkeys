#![allow(dead_code)]

//! A server on an ephemeral port with a database of its own.
//!
//! Every test gets a fresh database rather than a shared transaction, because
//! what is under test is SQL-level: foreign keys, unique constraints, row
//! locks and one transaction per write. A mocked store would test none of it.

use deadpool_postgres::{Object, Pool};
use havenkeys_server::{router, AppState};
use uuid::Uuid;

const DEFAULT_URL: &str = "postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable";

pub struct TestServer {
    base: String,
    pool: Pool,
    admin_url: String,
    db_name: String,
    client: reqwest::Client,
}

impl TestServer {
    pub async fn start() -> Self {
        let admin_url =
            std::env::var("HAVENKEYS_TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.into());
        let db_name = format!("hk_test_{}", Uuid::new_v4().simple());
        run_on_admin(&admin_url, &format!(r#"CREATE DATABASE "{db_name}""#)).await;

        let url = swap_database(&admin_url, &db_name);
        let pool = havenkeys_server::db::connect(&url).expect("pool");
        havenkeys_server::db::migrate(&pool).await.expect("migrate");

        let state = AppState {
            pool: pool.clone(),
            server_secret: [7u8; 32],
            trust_forwarded_for: false,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });

        Self {
            base: format!("http://{addr}"),
            pool,
            admin_url,
            db_name,
            client: reqwest::Client::new(),
        }
    }

    /// A pooled connection, for tests that assert on stored rows.
    pub async fn db(&self) -> Object {
        self.pool.get().await.unwrap()
    }

    pub fn pool(&self) -> &Pool {
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

    /// Best-effort cleanup: a leaked database in a disposable container costs
    /// nothing, so this never fails a test.
    pub async fn cleanup(self) {
        let Self {
            pool,
            admin_url,
            db_name,
            ..
        } = self;
        pool.close();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            run_on_admin_checked(
                &admin_url,
                &format!(r#"DROP DATABASE IF EXISTS "{db_name}" WITH (FORCE)"#),
            ),
        )
        .await;
    }
}

async fn run_on_admin(url: &str, sql: &str) {
    run_on_admin_checked(url, sql)
        .await
        .expect("Postgres is not reachable — run scripts/test-server.sh");
}

async fn run_on_admin_checked(url: &str, sql: &str) -> Result<(), tokio_postgres::Error> {
    let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
    let handle = tokio::spawn(connection);
    let result = client.batch_execute(sql).await;
    drop(client);
    handle.abort();
    result
}

fn swap_database(url: &str, db: &str) -> String {
    let (head, tail) = match url.split_once('?') {
        Some((head, query)) => (head, format!("?{query}")),
        None => (url, String::new()),
    };
    let base = head.rsplit_once('/').map(|(h, _)| h).unwrap_or(head);
    format!("{base}/{db}{tail}")
}
