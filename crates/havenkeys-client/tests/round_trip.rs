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
            max_vault_bytes: havenkeys_server::limits::MAX_VAULT_BYTES,
            mailer: None,
            signup_url: None,
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
                trial: false,
            },
            &self.pool,
            [7u8; 32],
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
        tags: None,
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

#[tokio::test(flavor = "multi_thread")]
async fn a_device_locked_during_the_deletion_wipes_itself_on_unlock() {
    let server = Server::start().await;
    let invite = server.invite("pocket@example.com").await;
    let dir_a = tempfile::tempdir().unwrap();
    let (a, _) = device(dir_a.path());
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
        "pocket@example.com".into(),
        SecretString::from(PASSWORD),
        Some(secret_key),
    )
    .await
    .unwrap();
    b.lock("user");

    a.delete_account("pocket@example.com".into(), SecretString::from(PASSWORD))
        .await
        .unwrap();

    // The vault opens offline from the replica; signing in then learns of
    // the deletion and erases it.
    let _ = b.unlock(SecretString::from(PASSWORD), None).await;
    let deleted = || {
        probe_b
            .seen
            .lock()
            .unwrap()
            .contains(&"account_deleted:true".to_string())
    };
    until(deleted).await;
    assert!(b.vault().unwrap().account().unwrap().is_none());
    assert!(b.device().unwrap().secret_key_text(account_id).is_none());
    server.cleanup().await;
}

/// SV-5: after the server is restored from an older backup, a device that
/// was ahead pulls the whole vault again and drops what the server no longer
/// has, instead of keeping it as a ghost.
#[tokio::test(flavor = "multi_thread")]
async fn a_restored_server_drops_items_it_no_longer_has() {
    let server = Server::start().await;
    let invite = server.invite("restore@example.com").await;
    let dir = tempfile::tempdir().unwrap();
    let (a, _) = device(dir.path());
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;
    let now = havenkeys_client::now_ms;
    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("Kept"), now())
        .unwrap();
    a.push(staged).await.unwrap();
    a.sync_now().await.unwrap();

    let db = server.pool.get().await.unwrap();
    let snapshot: i64 = db
        .query_one("SELECT revision FROM vaults", &[])
        .await
        .unwrap()
        .get(0);
    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("Ghost"), now())
        .unwrap();
    a.push(staged).await.unwrap();
    a.sync_now().await.unwrap();

    // The operator restores the backup taken at `snapshot`.
    db.execute("DELETE FROM items WHERE revision > $1", &[&snapshot])
        .await
        .unwrap();
    db.execute("UPDATE vaults SET revision = $1", &[&snapshot])
        .await
        .unwrap();

    let report = a.sync_now().await.unwrap();
    assert_eq!(report.deleted, 1);
    let titles = |c: &HavenClient| -> Vec<String> {
        c.vault()
            .unwrap()
            .list_items()
            .unwrap()
            .into_iter()
            .map(|o| o.title.clone())
            .collect()
    };
    assert!(titles(&a).contains(&"Kept".to_string()));
    assert!(!titles(&a).contains(&"Ghost".to_string()));

    // Writing works again, at revisions the device had seen before.
    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("After"), now())
        .unwrap();
    a.push(staged).await.unwrap();
    a.sync_now().await.unwrap();
    assert!(titles(&a).contains(&"After".to_string()));
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sync_purges_trash_older_than_30_days() {
    let server = Server::start().await;
    let invite = server.invite("user@example.com").await;
    let dir = tempfile::tempdir().unwrap();
    let (a, _) = device(dir.path());
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;

    let now = havenkeys_client::now_ms();
    let staged = a.vault().unwrap().stage_create(login("Old"), now).unwrap();
    let old = a.push(staged).await.unwrap().unwrap().id;
    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("Recent"), now)
        .unwrap();
    let recent = a.push(staged).await.unwrap().unwrap().id;

    let month = havenkeys_core::trash::TRASH_RETENTION_MS;
    let t = a
        .vault()
        .unwrap()
        .stage_trash(&old, now - month - 1)
        .unwrap();
    a.push(t).await.unwrap();
    let t = a.vault().unwrap().stage_trash(&recent, now).unwrap();
    a.push(t).await.unwrap();

    let report = a.sync_now().await.unwrap();
    assert_eq!(report.deleted, 1);
    let left: Vec<Uuid> = a
        .vault()
        .unwrap()
        .list_trash(now)
        .unwrap()
        .iter()
        .map(|e| e.overview.id)
        .collect();
    assert_eq!(left, vec![recent]);
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn restoring_an_item_purged_elsewhere_conflicts() {
    let server = Server::start().await;
    let invite = server.invite("user@example.com").await;

    let dir_a = tempfile::tempdir().unwrap();
    let (a, _) = device(dir_a.path());
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;

    let account_id = a.account_status().unwrap().unwrap().account_id;
    let secret_key = a.device().unwrap().secret_key_text(account_id).unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, _) = device(dir_b.path());
    b.sign_in(
        server.base.clone(),
        "user@example.com".into(),
        SecretString::from(PASSWORD),
        Some(secret_key),
    )
    .await
    .unwrap();

    // A creates an item and trashes it.
    let now = havenkeys_client::now_ms();
    let staged = a.vault().unwrap().stage_create(login("Gone"), now).unwrap();
    let id = a.push(staged).await.unwrap().unwrap().id;
    let t = a.vault().unwrap().stage_trash(&id, now).unwrap();
    a.push(t).await.unwrap();

    // B syncs: the item is in B's trash.
    b.sync_now().await.unwrap();
    let trash_of = |c: &HavenClient| -> Vec<Uuid> {
        c.vault()
            .unwrap()
            .list_trash(now)
            .unwrap()
            .iter()
            .map(|e| e.overview.id)
            .collect()
    };
    assert_eq!(trash_of(&b), vec![id]);

    // A purges it for good.
    let purge = a.vault().unwrap().stage_purge(&id).unwrap();
    a.push(purge).await.unwrap();

    // B restores from its stale replica: the server refuses.
    let restore = b.vault().unwrap().stage_restore_trashed(&id).unwrap();
    let err = b.push(restore).await.unwrap_err();
    assert_eq!(err.code, "item_changed_elsewhere");
    // The refused write changed nothing locally.
    assert_eq!(trash_of(&b), vec![id]);

    // B syncs: the item is gone from the trash and from the live list.
    b.sync_now().await.unwrap();
    assert!(trash_of(&b).is_empty());
    assert!(b
        .vault()
        .unwrap()
        .list_items()
        .unwrap()
        .iter()
        .all(|o| o.id != id));
    server.cleanup().await;
}
