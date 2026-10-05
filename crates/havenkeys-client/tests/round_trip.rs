//! Two devices through HavenClient, against the real server and a real
//! Postgres. Needs Postgres: `scripts/test-server.sh` starts one.

use havenkeys_client::device::Device;
use havenkeys_client::key_store::MemoryKeyStore;
use havenkeys_client::{ClientConfig, ClientEvents, HavenClient};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::store::Store;
use havenkeys_core::sync::SyncReport;
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
        let db_name = format!("hk_rt_{}", Uuid::new_v4().simple());
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

/// Records events, and checks on `unlocked` that the vault is held — the
/// contract `ClientEvents` documents.
struct Probe {
    vault: Arc<Mutex<VaultService>>,
    seen: Mutex<Vec<String>>,
}

impl ClientEvents for Probe {
    fn unlocked(&self, _: u32) {
        let held = matches!(
            self.vault.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        );
        self.seen
            .lock()
            .unwrap()
            .push(format!("unlocked:held={held}"));
    }
    fn locked(&self, reason: &'static str, was_open: bool) {
        self.seen
            .lock()
            .unwrap()
            .push(format!("locked:{reason}:{was_open}"));
    }
    fn connectivity(&self, online: bool) {
        self.seen.lock().unwrap().push(format!("online:{online}"));
    }
    fn signed_out(&self) {}
    fn synced(&self, _: SyncReport) {}
    fn items_changed(&self) {}
    fn removed(&self, _: bool) {}
    fn account_deleted(&self, keychain_cleared: bool) {
        self.seen
            .lock()
            .unwrap()
            .push(format!("account_deleted:{keychain_cleared}"));
    }
}

fn device(dir: &std::path::Path) -> (Arc<HavenClient>, Arc<Probe>) {
    let vault = Arc::new(Mutex::new(VaultService::new(
        Store::open_in_memory().unwrap(),
    )));
    let probe = Arc::new(Probe {
        vault: vault.clone(),
        seen: Mutex::new(Vec::new()),
    });
    let client = HavenClient::new(
        vault,
        Device::load(dir, Box::new(MemoryKeyStore::default())),
        None,
        probe.clone(),
        ClientConfig {
            device_name: "Test".into(),
            vault_path: dir.join("vault.sqlite3"),
        },
    );
    (client, probe)
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
async fn two_devices_share_one_vault_through_the_client() {
    let server = Server::start().await;
    let invite = server.invite("user@example.com").await;

    let dir_a = tempfile::tempdir().unwrap();
    let (a, probe_a) = device(dir_a.path());
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;
    assert!(probe_a
        .seen
        .lock()
        .unwrap()
        .contains(&"unlocked:held=true".to_string()));

    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("GitHub"), havenkeys_client::now_ms())
        .unwrap();
    a.push(staged).await.unwrap();

    let account_id = a.account_status().unwrap().unwrap().account_id;
    let secret_key = a.device().unwrap().secret_key_text(account_id).unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, probe_b) = device(dir_b.path());
    b.sign_in(
        server.base.clone(),
        "user@example.com".into(),
        SecretString::from(PASSWORD),
        Some(secret_key),
    )
    .await
    .unwrap();
    b.sync_now().await.unwrap();
    let titles: Vec<String> = b
        .vault()
        .unwrap()
        .list_items()
        .unwrap()
        .into_iter()
        .map(|o| o.title.clone())
        .collect();
    assert!(titles.contains(&"GitHub".to_string()));

    // Unlocking an unlocked vault is refused, and leaves it open.
    let again = b.unlock(SecretString::from(PASSWORD), None).await;
    assert_eq!(again.unwrap_err().code, "invalid_input");
    assert!(b.require_unlocked().is_ok());

    b.lock("user");
    assert!(!b.is_online());
    assert!(b
        .vault()
        .unwrap()
        .stage_create(login("Nope"), havenkeys_client::now_ms())
        .is_err());

    let wrong = b.unlock(SecretString::from("not the password"), None).await;
    assert!(wrong.is_err());
    assert!(!b.is_online());

    b.unlock(SecretString::from(PASSWORD), None).await.unwrap();
    until(|| b.is_online()).await;
    // Once when sign-in created the vault, once for the unlock above; both
    // announced while the vault was held.
    let unlocked: Vec<String> = probe_b
        .seen
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.starts_with("unlocked:"))
        .cloned()
        .collect();
    assert_eq!(unlocked, vec!["unlocked:held=true"; 2]);

    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_other_device_wipes_itself() {
    let server = Server::start().await;
    let invite = server.invite("bye@example.com").await;
    let dir_a = tempfile::tempdir().unwrap();
    let (a, probe_a) = device(dir_a.path());
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;

    let account_id = a.account_status().unwrap().unwrap().account_id;
    let secret_key = a.device().unwrap().secret_key_text(account_id).unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, probe_b) = device(dir_b.path());
    b.sign_in(
        server.base.clone(),
        "bye@example.com".into(),
        SecretString::from(PASSWORD),
        Some(secret_key),
    )
    .await
    .unwrap();

    a.delete_account("bye@example.com".into(), SecretString::from(PASSWORD))
        .await
        .unwrap();
    assert!(probe_a
        .seen
        .lock()
        .unwrap()
        .contains(&"account_deleted:true".to_string()));
    assert!(a.vault().unwrap().account().unwrap().is_none());

    let err = b.sync_now().await.unwrap_err();
    assert_eq!(err.code, "account_deleted");
    assert!(probe_b
        .seen
        .lock()
        .unwrap()
        .contains(&"account_deleted:true".to_string()));
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
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;

    let err = a
        .delete_account(
            "typo@example.com".into(),
            SecretString::from("not the password"),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, havenkeys_core::Error::UnlockFailed.code());
    assert!(a.is_online(), "a typo must not sign the device out");
    assert!(a.vault().unwrap().account().unwrap().is_some());
    a.sync_now().await.unwrap();
    server.cleanup().await;
}
