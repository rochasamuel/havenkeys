//! Signing in a new desktop from the phone, through HavenClient, against the
//! real server and a real Postgres. Needs Postgres: `scripts/test-server.sh`.

use havenkeys_client::device::Device;
use havenkeys_client::key_store::MemoryKeyStore;
use havenkeys_client::{ClientConfig, HavenClient};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::store::Store;
use havenkeys_core::vault::VaultService;
use havenkeys_core::SecretString;
use havenkeys_server::admin::{self, AdminCommand};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const DEFAULT_URL: &str = "postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable";

struct Server {
    base: String,
    admin_url: String,
    db_name: String,
    pool: deadpool_postgres::Pool,
}

impl Server {
    async fn start() -> Self {
        let admin_url =
            std::env::var("HAVENKEYS_TEST_DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.into());
        let db_name = format!("hk_pair_{}", Uuid::new_v4().simple());
        exec(&admin_url, &format!(r#"CREATE DATABASE "{db_name}""#))
            .await
            .expect("Postgres is not reachable — run scripts/test-server.sh");

        let url = match admin_url.split_once('?') {
            Some((head, query)) => {
                format!("{}/{db_name}?{query}", head.rsplit_once('/').unwrap().0)
            }
            None => format!("{}/{db_name}", admin_url.rsplit_once('/').unwrap().0),
        };
        let pool = havenkeys_server::db::connect(&url).await.unwrap();
        havenkeys_server::db::migrate(&pool).await.unwrap();

        let state = havenkeys_server::AppState {
            pool: pool.clone(),
            server_secret: [5u8; 32],
            trust_forwarded_for: false,
            cors_origin: None,
            locator: None,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                havenkeys_server::router(state)
                    .into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        Self {
            base: format!("http://127.0.0.1:{}", addr.port()),
            admin_url,
            db_name,
            pool,
        }
    }

    async fn invite(&self, email: &str) -> String {
        admin::run(
            AdminCommand::NewAccount {
                email: email.into(),
                server_url: self.base.clone(),
            },
            &self.pool,
        )
        .await
        .unwrap()
        .trim()
        .to_string()
    }

    async fn cleanup(self) {
        self.pool.close();
        let _ = exec(
            &self.admin_url,
            &format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.db_name),
        )
        .await;
    }
}

async fn exec(url: &str, sql: &str) -> Result<(), tokio_postgres::Error> {
    let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
    let handle = tokio::spawn(connection);
    let result = client.batch_execute(sql).await;
    drop(client);
    handle.abort();
    result
}

struct Quiet;

impl havenkeys_client::ClientEvents for Quiet {
    fn unlocked(&self, _: u32) {}
    fn locked(&self, _: &'static str, _: bool) {}
    fn connectivity(&self, _: bool) {}
    fn signed_out(&self) {}
    fn synced(&self, _: havenkeys_core::sync::SyncReport) {}
    fn items_changed(&self) {}
    fn removed(&self, _: bool) {}
}

fn client_in(dir: &std::path::Path) -> Arc<HavenClient> {
    HavenClient::new(
        Arc::new(Mutex::new(VaultService::new(
            Store::open_in_memory().unwrap(),
        ))),
        Device::load(dir, Box::new(MemoryKeyStore::default())),
        None,
        Arc::new(Quiet),
        ClientConfig {
            device_name: "Test",
            vault_path: dir.join("vault.sqlite3"),
        },
    )
}

async fn exec_on(server: &Server, sql: &str) {
    server
        .pool
        .get()
        .await
        .unwrap()
        .batch_execute(sql)
        .await
        .unwrap();
}

async fn until(what: impl Fn() -> bool) {
    for _ in 0..200 {
        if what() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not reached in 5 s");
}

fn login(title: &str) -> ItemInput {
    ItemInput {
        item_type: ItemType::Login,
        title: title.into(),
        username: Some("me@example.com".into()),
        urls: vec![UrlRule {
            url: "github.com".into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(SecretString::from("hunter2-but-longer")),
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: None,
        sections: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_signs_a_new_desktop_in_without_the_password() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    let invite = server.invite("ana@example.com").await;
    phone
        .activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| phone.is_online()).await;
    let item = phone
        .vault()
        .unwrap()
        .stage_create(login("GitHub"), havenkeys_client::now_ms())
        .unwrap();
    phone.push(item).await.unwrap();

    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop
        .start_pairing(format!("{}/", server.base), "Desktop · Linux")
        .await
        .unwrap();
    assert!(matches!(
        desktop.poll_pairing().await.unwrap(),
        havenkeys_client::PairingPoll::Waiting
    ));

    let request = phone.pairing_request(&start.link).await.unwrap();
    assert_eq!(request.device_name, "Desktop · Linux");
    phone.approve_pairing(&start.link).await.unwrap();

    let status = match desktop.poll_pairing().await.unwrap() {
        havenkeys_client::PairingPoll::Approved(status) => status,
        other => panic!("not approved: {other:?}"),
    };
    assert_eq!(status.state, havenkeys_core::vault::VaultState::Unlocked);
    until(|| desktop.is_online()).await;
    desktop.sync_now().await.unwrap();
    let titles: Vec<String> = desktop
        .vault()
        .unwrap()
        .list_items()
        .unwrap()
        .iter()
        .map(|o| o.title.clone())
        .collect();
    assert!(titles.contains(&"GitHub".to_string()));

    // From now on an ordinary device: the master password unlocks it.
    desktop.lock("user");
    desktop
        .unlock(SecretString::from(PASSWORD), None)
        .await
        .unwrap();
    assert!(desktop.require_unlocked().is_ok());

    // Approving the same code again finds nothing.
    assert_eq!(
        phone.approve_pairing(&start.link).await.unwrap_err().code,
        "pairing_gone"
    );
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_denied_or_expired_pairing_leaves_the_desktop_empty() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone
        .activate(
            server.invite("ana@example.com").await,
            SecretString::from(PASSWORD),
        )
        .await
        .unwrap();
    until(|| phone.is_online()).await;

    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop
        .start_pairing(server.base.clone(), "Desktop")
        .await
        .unwrap();
    phone.deny_pairing(&start.link).await.unwrap();
    assert!(matches!(
        desktop.poll_pairing().await.unwrap(),
        havenkeys_client::PairingPoll::Denied
    ));
    assert!(desktop.vault().unwrap().account().unwrap().is_none());

    let start = desktop
        .start_pairing(server.base.clone(), "Desktop")
        .await
        .unwrap();
    let id = havenkeys_core::pairing::PairingLink::parse(&start.link)
        .unwrap()
        .pairing_id;
    exec_on(
        &server,
        &format!("UPDATE pairings SET expires_at = now() - interval '1 second' WHERE id = '{id}'"),
    )
    .await;
    assert!(matches!(
        desktop.poll_pairing().await.unwrap(),
        havenkeys_client::PairingPoll::Expired
    ));
    assert_eq!(
        phone.approve_pairing(&start.link).await.unwrap_err().code,
        "pairing_gone"
    );
    assert!(desktop.vault().unwrap().account().unwrap().is_none());
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_for_another_server_is_refused_by_the_phone() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone
        .activate(
            server.invite("ana@example.com").await,
            SecretString::from(PASSWORD),
        )
        .await
        .unwrap();
    until(|| phone.is_online()).await;
    let keys = havenkeys_core::pairing::PairingKeys::generate();
    let link = havenkeys_core::pairing::PairingLink {
        server_url: "https://elsewhere.example.com".into(),
        pairing_id: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        public_key: keys.public_key(),
    }
    .to_text();
    assert_eq!(
        phone.pairing_request(&link).await.unwrap_err().code,
        "pairing_other_server"
    );
    assert_eq!(
        phone.approve_pairing(&link).await.unwrap_err().code,
        "pairing_other_server"
    );
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_trailing_slash_on_the_desktop_still_matches_the_phone() {
    // Covered by `a_phone_signs_a_new_desktop_in_without_the_password`, which
    // starts with `format!("{}/", server.base)`; this test pins the locked case.
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone
        .activate(
            server.invite("ana@example.com").await,
            SecretString::from(PASSWORD),
        )
        .await
        .unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop
        .start_pairing(format!(" {}/ ", server.base), "Desktop")
        .await
        .unwrap();
    phone.lock("user");
    assert_eq!(
        phone.approve_pairing(&start.link).await.unwrap_err().code,
        "locked"
    );
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_refuses_a_device_that_already_has_a_vault() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let phone = client_in(dir.path());
    phone
        .activate(
            server.invite("ana@example.com").await,
            SecretString::from(PASSWORD),
        )
        .await
        .unwrap();
    assert_eq!(
        phone
            .start_pairing(server.base.clone(), "Desktop")
            .await
            .unwrap_err()
            .code,
        "vault_exists"
    );
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn approving_twice_is_refused_the_second_time() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone
        .activate(
            server.invite("ana@example.com").await,
            SecretString::from(PASSWORD),
        )
        .await
        .unwrap();
    until(|| phone.is_online()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop
        .start_pairing(server.base.clone(), "Desktop")
        .await
        .unwrap();
    phone.approve_pairing(&start.link).await.unwrap();
    assert_eq!(
        phone.approve_pairing(&start.link).await.unwrap_err().code,
        "pairing_gone"
    );
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn overlapping_polls_after_approval_still_create_the_vault() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone
        .activate(
            server.invite("ana@example.com").await,
            SecretString::from(PASSWORD),
        )
        .await
        .unwrap();
    until(|| phone.is_online()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop
        .start_pairing(server.base.clone(), "Desktop")
        .await
        .unwrap();
    phone.approve_pairing(&start.link).await.unwrap();

    let (a, b) = tokio::join!(desktop.poll_pairing(), desktop.poll_pairing());
    let approved = [&a, &b]
        .iter()
        .filter(|r| matches!(r, Ok(havenkeys_client::PairingPoll::Approved(_))))
        .count();
    assert_eq!(approved, 1, "{a:?} / {b:?}");
    assert!(desktop.require_unlocked().is_ok());
    assert!(desktop.vault().unwrap().account().unwrap().is_some());
    server.cleanup().await;
}
