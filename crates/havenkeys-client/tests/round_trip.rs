//! Two devices through HavenClient, against the real server and a real
//! Postgres. Needs Postgres: `scripts/test-server.sh` starts one.

use havenkeys_client::device::Device;
use havenkeys_client::key_store::MemoryKeyStore;
use havenkeys_client::{ClientConfig, ClientEvents, HavenClient, NewFile};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::store::Store;
use havenkeys_core::sync::SyncReport;
use havenkeys_core::vault::VaultService;
use havenkeys_core::SecretString;
use havenkeys_server::admin::{self, AdminCommand};
use std::sync::atomic::{AtomicUsize, Ordering};
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
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn start() -> Self {
        Self::start_with_vault_limit(havenkeys_server::limits::MAX_VAULT_BYTES).await
    }

    /// A server whose vaults may hold at most `max_vault_bytes` of blobs.
    async fn start_with_vault_limit(max_vault_bytes: i64) -> Self {
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
            max_vault_bytes,
            mailer: None,
            signup_url: None,
            pake: havenkeys_server::pake::load_or_create(&pool).await.unwrap(),
            legacy_auth_until: None,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
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
            task,
        }
    }

    /// Stop answering: devices see an unreachable server.
    fn stop(&self) {
        self.task.abort();
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
    plan: AtomicUsize,
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
    fn plan_changed(&self) {
        self.plan.fetch_add(1, Ordering::SeqCst);
    }
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
        plan: AtomicUsize::new(0),
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

struct Fixture {
    server: Server,
    client: Arc<HavenClient>,
    events: Arc<Probe>,
    _dir: tempfile::TempDir,
}

impl Fixture {
    fn staged_login(&self) -> havenkeys_core::vault::StagedWrite {
        self.client
            .vault()
            .unwrap()
            .stage_create(login("Another"), havenkeys_client::now_ms())
            .unwrap()
    }

    fn password(&self) -> SecretString {
        SecretString::from(PASSWORD)
    }
}

impl Probe {
    fn plan_changes(&self) -> usize {
        self.plan.load(Ordering::SeqCst)
    }
}

async fn fixture() -> Fixture {
    let server = Server::start().await;
    let invite = server.invite("plan@example.com").await;
    let dir = tempfile::tempdir().unwrap();
    let (client, events) = device(dir.path());
    client
        .activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| client.is_online()).await;
    // Login records the plan just after going online; wait for it so the
    // reset below cannot race its announcement.
    until(|| {
        client
            .account_status()
            .ok()
            .flatten()
            .and_then(|a| a.plan_status)
            .is_some()
    })
    .await;
    // Count only what happens from here on.
    events.plan.store(0, Ordering::SeqCst);
    Fixture {
        server,
        client,
        events,
        _dir: dir,
    }
}

async fn set_plan_status(fx: &Fixture, status: havenkeys_server::billing::Status) {
    let account_id = fx.client.account_status().unwrap().unwrap().account_id;
    havenkeys_server::billing::set_status(
        &fx.server.pool.get().await.unwrap(),
        account_id,
        havenkeys_server::billing::Actor::Admin,
        status,
        None,
        "test",
    )
    .await
    .unwrap();
}

async fn freeze(fx: &Fixture) {
    set_plan_status(fx, havenkeys_server::billing::Status::Frozen).await;
}

async fn unfreeze(fx: &Fixture) {
    set_plan_status(fx, havenkeys_server::billing::Status::Complimentary).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_plan_is_stored_on_login_and_sync_and_a_402_freezes_at_once() {
    use havenkeys_core::store::Entitlement;
    let fx = fixture().await;
    let status = fx.client.account_status().unwrap().unwrap();
    assert_eq!(status.entitlement, Entitlement::Full);
    assert_eq!(status.plan_status.as_deref(), Some("complimentary"));

    freeze(&fx).await;
    let err = fx.client.push(fx.staged_login()).await.unwrap_err();
    assert_eq!(err.code, "account_frozen");
    assert_eq!(
        fx.client.account_status().unwrap().unwrap().entitlement,
        Entitlement::Frozen
    );
    assert_eq!(fx.events.plan_changes(), 1);

    assert_eq!(fx.client.require_full().unwrap_err().code, "account_frozen");
    assert_eq!(
        fx.client.push(fx.staged_login()).await.unwrap_err().code,
        "account_frozen"
    );
    fx.server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sync_that_says_full_unfreezes() {
    use havenkeys_core::store::Entitlement;
    let fx = fixture().await;
    freeze(&fx).await;
    fx.client.sync_now().await.unwrap();
    assert_eq!(
        fx.client.account_status().unwrap().unwrap().entitlement,
        Entitlement::Frozen
    );
    unfreeze(&fx).await;
    fx.client.sync_now().await.unwrap();
    assert_eq!(
        fx.client.account_status().unwrap().unwrap().entitlement,
        Entitlement::Full
    );
    // Freeze and unfreeze, each announced once through the pull path.
    assert_eq!(fx.events.plan_changes(), 2);
    fx.client.push(fx.staged_login()).await.unwrap();
    fx.server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_frozen_account_still_syncs_and_reads_but_refuses_writes() {
    let fx = fixture().await;
    fx.client.push(fx.staged_login()).await.unwrap();
    freeze(&fx).await;
    fx.client.sync_now().await.unwrap();
    assert!(!fx.client.vault().unwrap().list_items().unwrap().is_empty());
    let err = fx
        .client
        .change_master_password(fx.password(), fx.password())
        .await
        .unwrap_err();
    assert_eq!(err.code, "account_frozen");

    // A restore is refused before any staging or request.
    let staged = havenkeys_core::vault::StagedImport {
        writes: vec![fx.staged_login()],
        report: Default::default(),
    };
    let err = fx.client.push_restore(staged).await.unwrap_err();
    assert_eq!(err.code, "account_frozen");
    fx.server.cleanup().await;
}

/// A frozen account gains no device: a brand-new device that signs in with
/// the right password hears why, not "wrong password". A wrong password is
/// still the one sign-in message (the server checks it first).
#[tokio::test(flavor = "multi_thread")]
async fn a_frozen_account_refuses_a_new_device_with_its_own_code() {
    let fx = fixture().await;
    freeze(&fx).await;
    let account_id = fx.client.account_status().unwrap().unwrap().account_id;
    let secret_key = fx
        .client
        .device()
        .unwrap()
        .secret_key_text(account_id)
        .unwrap();

    let dir_b = tempfile::tempdir().unwrap();
    let (b, _) = device(dir_b.path());
    let err = b
        .sign_in(
            fx.server.base.clone(),
            "plan@example.com".into(),
            SecretString::from("not the password at all"),
            Some(secret_key.clone()),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, "sign_in_failed");

    let dir_c = tempfile::tempdir().unwrap();
    let (c, _) = device(dir_c.path());
    let err = c
        .sign_in(
            fx.server.base.clone(),
            "plan@example.com".into(),
            fx.password(),
            Some(secret_key),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, "account_frozen_new_device");
    assert!(c.account_status().unwrap().is_none(), "nothing was set up");
    fx.server.cleanup().await;
}

async fn server_items(fx: &Fixture) -> i64 {
    fx.server
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM items", &[])
        .await
        .unwrap()
        .get(0)
}

fn set_client_entitlement(fx: &Fixture, entitlement: havenkeys_core::store::Entitlement) {
    let mut vault = fx.client.vault().unwrap();
    let mut plan = vault.plan().unwrap();
    plan.entitlement = entitlement;
    vault.set_plan(&plan).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_frozen_client_refuses_a_restore_before_any_request() {
    use havenkeys_core::store::Entitlement;
    let fx = fixture().await;
    fx.client.push(fx.staged_login()).await.unwrap();
    let before = settled_server_items(&fx).await;

    // Only the client believes it is frozen; the server would accept.
    set_client_entitlement(&fx, Entitlement::Frozen);
    let staged = havenkeys_core::vault::StagedImport {
        writes: vec![fx.staged_login()],
        report: Default::default(),
    };
    let err = fx.client.push_restore(staged).await.unwrap_err();
    assert_eq!(err.code, "account_frozen");
    let backup = havenkeys_core::export::backup::OpenedBackup {
        items: vec![],
        unreadable: 0,
    };
    let err = fx
        .client
        .restore_backup(backup, std::iter::empty())
        .await
        .unwrap_err();
    assert_eq!(err.code, "account_frozen");
    assert_eq!(server_items(&fx).await, before);

    // Without the freeze the same write goes through: the guard decided.
    set_client_entitlement(&fx, Entitlement::Full);
    let staged = havenkeys_core::vault::StagedImport {
        writes: vec![fx.staged_login()],
        report: Default::default(),
    };
    fx.client.push_restore(staged).await.unwrap();
    assert_eq!(server_items(&fx).await, before + 1);
    fx.server.cleanup().await;
}

/// The count once the device's background identity creation (after login)
/// has landed: unchanged over a quarter second.
async fn settled_server_items(fx: &Fixture) -> i64 {
    let mut last = server_items(fx).await;
    let mut stable = 0;
    while stable < 5 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let now = server_items(fx).await;
        stable = if now == last { stable + 1 } else { 0 };
        last = now;
    }
    last
}

// OPAQUE sign-in and the move to it (spec 2026-10-13-pake-login §6).

const NEW_PASSWORD: &str = "a different but equally long passphrase";

fn pake_confirmed(client: &HavenClient) -> bool {
    client.vault().unwrap().pake_confirmed().unwrap()
}

async fn has_opaque_record(server: &Server, email: &str) -> bool {
    server
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT opaque_record IS NOT NULL FROM accounts WHERE email_normalized = $1",
            &[&email],
        )
        .await
        .unwrap()
        .get(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_account_signs_in_with_opaque_and_remembers_it() {
    let fx = fixture().await; // activate, then online
    assert!(pake_confirmed(&fx.client));
    // A second device signs in fresh, and is confirmed after that sign-in.
    let account_id = fx.client.account_status().unwrap().unwrap().account_id;
    let secret_key = fx
        .client
        .device()
        .unwrap()
        .secret_key_text(account_id)
        .unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, _) = device(dir_b.path());
    b.sign_in(
        fx.server.base.clone(),
        "plan@example.com".into(),
        fx.password(),
        Some(secret_key),
    )
    .await
    .unwrap();
    assert!(b.is_online());
    assert!(pake_confirmed(&b));
    fx.server.cleanup().await;
}

/// An account made before OPAQUE (legacy activation) is enrolled by the
/// first device that signs in, and that device then signs in with OPAQUE.
#[tokio::test(flavor = "multi_thread")]
async fn an_older_account_is_enrolled_on_its_first_sign_in() {
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_sync_client::{Activation, HttpTransport, SyncClient};

    const EMAIL: &str = "old@example.com";
    let server = Server::start().await;
    let raw = server.invite(EMAIL).await;
    let invite = havenkeys_sync_client::invite::decode(&raw).unwrap();
    let account = AccountRef::new(invite.account, NormalizedEmail::parse(EMAIL).unwrap());
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
    let made = havenkeys_core::vault::prepare_new_account_vault(
        &SecretString::from(PASSWORD),
        &account,
        kdf,
        havenkeys_client::now_ms(),
    )
    .unwrap();
    let header = havenkeys_core::sync::encode_header_for(&made.prepared).unwrap();
    SyncClient::new(HttpTransport::new(&server.base).unwrap())
        .activate_legacy(Activation {
            email: EMAIL,
            invite: &raw,
            kdf: made.prepared.kdf(),
            auth_key: &made.auth_key,
            vault_id: made.prepared.vault_id(),
            header: &header,
        })
        .await
        .unwrap();
    assert!(!has_opaque_record(&server, EMAIL).await);

    let dir = tempfile::tempdir().unwrap();
    let (a, _) = device(dir.path());
    a.sign_in(
        server.base.clone(),
        EMAIL.into(),
        SecretString::from(PASSWORD),
        Some(made.secret_key.to_text()),
    )
    .await
    .unwrap();
    assert!(a.is_online());
    assert!(has_opaque_record(&server, EMAIL).await, "enrolled");
    assert!(pake_confirmed(&a));

    // From now on this device signs in with OPAQUE only.
    a.lock("user");
    a.unlock(SecretString::from(PASSWORD), None).await.unwrap();
    until(|| a.is_online()).await;
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_an_opaque_account_uses_the_opaque_proof() {
    let fx = fixture().await;
    assert!(pake_confirmed(&fx.client));
    fx.client
        .delete_account("plan@example.com".into(), fx.password())
        .await
        .unwrap();
    assert!(fx.client.vault().unwrap().account().unwrap().is_none());
    let left: i64 = fx
        .server
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM accounts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(left, 0);
    fx.server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn changing_the_master_password_keeps_opaque_working() {
    let fx = fixture().await;
    fx.client
        .change_master_password(fx.password(), SecretString::from(NEW_PASSWORD))
        .await
        .unwrap();
    fx.client.lock("user");
    fx.client
        .unlock(SecretString::from(NEW_PASSWORD), None)
        .await
        .unwrap();
    // A confirmed device signs in with OPAQUE only: online means the new
    // record works.
    until(|| fx.client.is_online()).await;
    assert!(pake_confirmed(&fx.client));
    fx.server.cleanup().await;
}

fn png(body: &str) -> (String, zeroize::Zeroizing<Vec<u8>>) {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(body.as_bytes());
    ("scan.png".into(), zeroize::Zeroizing::new(bytes))
}

/// Two devices on one account, and the id of an item A created.
async fn two_devices(
    server: &Server,
) -> (
    Arc<HavenClient>,
    Arc<HavenClient>,
    Uuid,
    Vec<tempfile::TempDir>,
) {
    let invite = server.invite("files@example.com").await;
    let dir_a = tempfile::tempdir().unwrap();
    let (a, _) = device(dir_a.path());
    a.activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| a.is_online()).await;
    let staged = a
        .vault()
        .unwrap()
        .stage_create(login("Passport"), havenkeys_client::now_ms())
        .unwrap();
    let item = a.push(staged).await.unwrap().unwrap().id;

    let account_id = a.account_status().unwrap().unwrap().account_id;
    let secret_key = a.device().unwrap().secret_key_text(account_id).unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (b, _) = device(dir_b.path());
    b.sign_in(
        server.base.clone(),
        "files@example.com".into(),
        SecretString::from(PASSWORD),
        Some(secret_key),
    )
    .await
    .unwrap();
    b.sync_now().await.unwrap();
    (a, b, item, vec![dir_a, dir_b])
}

fn attachment_ids(client: &HavenClient, item: Uuid) -> Vec<Uuid> {
    let vault = client.vault().unwrap();
    let overview = vault
        .list_items()
        .unwrap()
        .into_iter()
        .find(|o| o.id == item);
    overview.unwrap().attachments.iter().map(|m| m.id).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn attach_then_read_on_another_device() {
    let server = Server::start().await;
    let (a, b, item, _dirs) = two_devices(&server).await;

    let updated = a.add_attachments(item, vec![png("front")]).await.unwrap();
    assert_eq!(updated.attachments.len(), 1);
    assert_eq!(updated.attachments[0].mime, "image/png");
    let id = updated.attachments[0].id;

    b.sync_now().await.unwrap();
    assert_eq!(attachment_ids(&b, item), vec![id]);
    assert!(b.vault().unwrap().cached_attachment(&id).unwrap().is_none());
    let opened = b.attachment_bytes(item, id).await.unwrap();
    assert!(opened.expose().ends_with(b"front"));
    // The download is kept for next time.
    assert!(b.vault().unwrap().cached_attachment(&id).unwrap().is_some());
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_read_uses_cache_and_misses_cleanly() {
    let server = Server::start().await;
    let (a, b, item, _dirs) = two_devices(&server).await;
    let id = a
        .add_attachments(item, vec![png("front")])
        .await
        .unwrap()
        .attachments[0]
        .id;
    b.sync_now().await.unwrap();

    server.stop();
    for client in [&a, &b] {
        client.lock("user");
        client
            .unlock(SecretString::from(PASSWORD), None)
            .await
            .unwrap();
    }
    // A cached its own upload; B never downloaded the file.
    assert!(a
        .attachment_bytes(item, id)
        .await
        .unwrap()
        .expose()
        .ends_with(b"front"));
    let err = b.attachment_bytes(item, id).await.unwrap_err();
    assert_eq!(err.code, "attachment_offline");
    assert_eq!(
        err.message,
        "You're offline \u{2014} reconnect to open this file."
    );
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conflicting_attach_surfaces_item_changed_elsewhere() {
    let server = Server::start().await;
    let (a, b, item, _dirs) = two_devices(&server).await;

    let trash = b
        .vault()
        .unwrap()
        .stage_trash(&item, havenkeys_client::now_ms())
        .unwrap();
    b.push(trash).await.unwrap();

    let err = a
        .add_attachments(item, vec![png("front")])
        .await
        .unwrap_err();
    assert_eq!(err.code, "item_changed_elsewhere");
    assert!(attachment_ids(&a, item).is_empty());
    // The upload stays pending on the server, for its sweep to remove.
    let db = server.pool.get().await.unwrap();
    let pending = db
        .query_one("SELECT count(*) FROM attachments WHERE NOT committed", &[])
        .await
        .unwrap();
    assert_eq!(pending.get::<_, i64>(0), 1);
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_evicts_cache() {
    let server = Server::start().await;
    let (a, b, item, _dirs) = two_devices(&server).await;
    let id = a
        .add_attachments(item, vec![png("front")])
        .await
        .unwrap()
        .attachments[0]
        .id;
    assert!(a.vault().unwrap().cached_attachment(&id).unwrap().is_some());

    let updated = a.remove_attachment(item, id).await.unwrap();
    assert!(updated.attachments.is_empty());
    assert!(a.vault().unwrap().cached_attachment(&id).unwrap().is_none());

    b.sync_now().await.unwrap();
    assert!(attachment_ids(&b, item).is_empty());
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn prune_keeps_trashed_items_files() {
    let server = Server::start().await;
    let (a, _b, item, _dirs) = two_devices(&server).await;
    let id = a
        .add_attachments(item, vec![png("front")])
        .await
        .unwrap()
        .attachments[0]
        .id;
    let trash = a
        .vault()
        .unwrap()
        .stage_trash(&item, havenkeys_client::now_ms())
        .unwrap();
    a.push(trash).await.unwrap();

    let stray = Uuid::new_v4();
    {
        let vault = a.vault().unwrap();
        vault
            .cache_attachment(&stray, b"stray", havenkeys_client::now_ms())
            .unwrap();
        vault.prune_attachment_cache(u64::MAX).unwrap();
        assert!(vault.cached_attachment(&id).unwrap().is_some());
        assert!(vault.cached_attachment(&stray).unwrap().is_none());
    }
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_cached_copy_is_evicted_and_refetched() {
    let server = Server::start().await;
    let (a, _b, item, _dirs) = two_devices(&server).await;
    let id = a
        .add_attachments(item, vec![png("front")])
        .await
        .unwrap()
        .attachments[0]
        .id;
    let now = havenkeys_client::now_ms();
    a.vault()
        .unwrap()
        .cache_attachment(&id, b"truncated", now)
        .unwrap();

    // Online: the bad copy is dropped and the file fetched again.
    assert!(a
        .attachment_bytes(item, id)
        .await
        .unwrap()
        .expose()
        .ends_with(b"front"));
    assert_ne!(
        a.vault().unwrap().cached_attachment(&id).unwrap().unwrap(),
        b"truncated"
    );

    // Offline: damaged, and the bad copy is gone rather than stuck.
    a.vault()
        .unwrap()
        .cache_attachment(&id, b"truncated", now)
        .unwrap();
    server.stop();
    a.lock("user");
    a.unlock(SecretString::from(PASSWORD), None).await.unwrap();
    let err = a.attachment_bytes(item, id).await.unwrap_err();
    assert_eq!(err.code, "attachment_offline");
    assert!(a.vault().unwrap().cached_attachment(&id).unwrap().is_none());
    server.cleanup().await;
}

fn new_file((name, bytes): (String, zeroize::Zeroizing<Vec<u8>>)) -> NewFile {
    NewFile { name, bytes }
}

#[tokio::test(flavor = "multi_thread")]
async fn limits_are_checked_before_any_upload() {
    let server = Server::start().await;
    let (a, _b, item, _dirs) = two_devices(&server).await;
    let too_big = NewFile {
        name: "big.bin".into(),
        bytes: zeroize::Zeroizing::new(vec![0u8; 5 * 1024 * 1024 + 1]),
    };
    let save = |files: Vec<NewFile>| {
        a.save_with_files(item, files, vec![], |vault, files| {
            vault.stage_update_with(&item, login("Passport"), files, havenkeys_client::now_ms())
        })
    };
    let err = save(vec![new_file(png("one")), new_file(png("two")), too_big])
        .await
        .unwrap_err();
    assert!(err.message.contains("Files can be up to 5 MB"));
    let many = (0..21).map(|i| new_file(png(&i.to_string()))).collect();
    let err = save(many).await.unwrap_err();
    assert!(err.message.contains("An item holds up to 20 files"));

    let db = server.pool.get().await.unwrap();
    let rows = db
        .query_one("SELECT count(*) FROM attachments", &[])
        .await
        .unwrap();
    assert_eq!(rows.get::<_, i64>(0), 0);
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_near_the_limit_is_refused_before_any_upload() {
    let server = Server::start().await;
    let (a, _b, item, _dirs) = two_devices(&server).await;
    let held = (0..19).map(|i| png(&i.to_string())).collect();
    let held = a.add_attachments(item, held).await.unwrap();
    let first = held.attachments[0].id;
    let cached = |client: &HavenClient| {
        client
            .vault()
            .unwrap()
            .cached_attachment(&first)
            .unwrap()
            .is_some()
    };
    assert!(cached(&a));

    let err = a
        .save_with_files(
            item,
            vec![new_file(png("x")), new_file(png("y"))],
            vec![],
            |vault, files| {
                vault.stage_update_with(&item, login("Passport"), files, havenkeys_client::now_ms())
            },
        )
        .await
        .unwrap_err();
    assert!(err.message.contains("An item holds up to 20 files"));

    let db = server.pool.get().await.unwrap();
    let rows = db
        .query_one("SELECT count(*) FROM attachments", &[])
        .await
        .unwrap();
    assert_eq!(rows.get::<_, i64>(0), 19);
    assert_eq!(attachment_ids(&a, item).len(), 19);
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn create_with_files_round_trips() {
    let server = Server::start().await;
    let (a, b, _item, _dirs) = two_devices(&server).await;
    let id = Uuid::new_v4();
    let saved = a
        .save_with_files(
            id,
            vec![new_file(png("front")), new_file(png("back"))],
            vec![],
            |vault, files| {
                vault.stage_create_with(id, login("Visa"), files, havenkeys_client::now_ms())
            },
        )
        .await
        .unwrap();
    assert_eq!(saved.id, id);
    assert_eq!(saved.attachments.len(), 2);

    b.sync_now().await.unwrap();
    let ids = attachment_ids(&b, id);
    assert_eq!(ids.len(), 2);
    let opened = b.attachment_bytes(id, ids[0]).await.unwrap();
    assert!(opened.expose().ends_with(b"front"));
    let opened = b.attachment_bytes(id, ids[1]).await.unwrap();
    assert!(opened.expose().ends_with(b"back"));
    server.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn update_removes_and_adds_in_one_save() {
    let server = Server::start().await;
    let (a, b, item, _dirs) = two_devices(&server).await;
    let first = a
        .add_attachments(item, vec![png("front"), png("back")])
        .await
        .unwrap();
    let (gone, kept) = (first.attachments[0].id, first.attachments[1].id);

    let saved = a
        .save_with_files(
            item,
            vec![new_file(png("new"))],
            vec![gone],
            |vault, files| {
                vault.stage_update_with(&item, login("Passport"), files, havenkeys_client::now_ms())
            },
        )
        .await
        .unwrap();
    let now: Vec<Uuid> = saved.attachments.iter().map(|m| m.id).collect();
    assert_eq!(now.len(), 2);
    assert_eq!(now[0], kept);
    assert!(a
        .vault()
        .unwrap()
        .cached_attachment(&gone)
        .unwrap()
        .is_none());

    b.sync_now().await.unwrap();
    assert_eq!(attachment_ids(&b, item), now);
    // The server holds exactly the committed files.
    let db = server.pool.get().await.unwrap();
    let rows = db
        .query_one(
            "SELECT count(*) FILTER (WHERE committed) AS live, count(*) AS total FROM attachments",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.get::<_, i64>(0), 2);
    assert_eq!(rows.get::<_, i64>(1), 2);
    server.cleanup().await;
}

/// A device signed in to a new account on `server`.
async fn signed_in(server: &Server, email: &str) -> (Arc<HavenClient>, tempfile::TempDir) {
    let invite = server.invite(email).await;
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = device(dir.path());
    client
        .activate(invite, SecretString::from(PASSWORD))
        .await
        .unwrap();
    until(|| client.is_online()).await;
    (client, dir)
}

/// A backup of `client`'s vault carrying every file of `item`.
async fn backup_with_files(client: &HavenClient, item: Uuid, password: &SecretString) -> Vec<u8> {
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::export::backup::BackupWriter;
    use havenkeys_core::export::{self, ExportFormat};
    let (payload, metas) = {
        let vault = client.vault().unwrap();
        let payload = export::render(&vault, ExportFormat::Backup, 1).unwrap();
        (payload, vault.get_item(&item).unwrap().attachments.clone())
    };
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
    let mut writer = BackupWriter::new(Vec::new(), password, &kdf, metas.len() as u32).unwrap();
    writer.items(&payload.bytes).unwrap();
    for meta in &metas {
        let bytes = client.attachment_bytes(item, meta.id).await.unwrap();
        writer.attachment(item, meta, bytes.expose()).unwrap();
    }
    writer.finish().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn restore_reports_skipped_attachments_when_quota_is_full() {
    use havenkeys_core::export::backup::open_backup_stream;
    let source = Server::start().await;
    let (a, _b, item, _dirs) = two_devices(&source).await;
    let big = (
        "big.bin".to_string(),
        zeroize::Zeroizing::new(vec![7u8; 64 * 1024]),
    );
    a.add_attachments(item, vec![png("small"), big])
        .await
        .unwrap();
    let password = SecretString::from("a separate backup passphrase");
    let file = backup_with_files(&a, item, &password).await;

    // Room for the items and the small file, not the big one.
    let target = Server::start_with_vault_limit(32 * 1024).await;
    let (c, _dir) = signed_in(&target, "restore@example.com").await;
    let (opened, files) = open_backup_stream(&file[..], &password).unwrap();
    let report = c.restore_backup(opened, files).await.unwrap();
    assert_eq!(
        (
            report.logins,
            report.attachments_skipped_storage,
            report.attachments_skipped
        ),
        (1, 1, 0)
    );

    // The restored item lists the file that fit, re-sealed for this vault.
    let kept = attachment_ids(&c, item);
    assert_eq!(kept.len(), 1);
    let bytes = c.attachment_bytes(item, kept[0]).await.unwrap();
    assert!(bytes.expose().ends_with(b"small"));
    source.cleanup().await;
    target.cleanup().await;
}

fn identity_of(client: &HavenClient) -> Uuid {
    let vault = client.vault().unwrap();
    let items = vault.list_items().unwrap();
    items
        .iter()
        .find(|o| o.item_type == ItemType::Identity)
        .unwrap()
        .id
}

/// Every account has its own Identity, so the backup's is never restored
/// as an item: its files are added to this account's Identity, once.
#[tokio::test(flavor = "multi_thread")]
async fn identity_attachments_restore_into_a_new_account() {
    use havenkeys_core::export::backup::open_backup_stream;
    let source = Server::start().await;
    let (a, _b, _item, _dirs) = two_devices(&source).await;
    let theirs = identity_of(&a);
    a.add_attachments(theirs, vec![png("passport")])
        .await
        .unwrap();
    let password = SecretString::from("a separate backup passphrase");
    let file = backup_with_files(&a, theirs, &password).await;

    let target = Server::start().await;
    let (c, _dir) = signed_in(&target, "restore@example.com").await;
    for _ in 0..2 {
        let (opened, files) = open_backup_stream(&file[..], &password).unwrap();
        let report = c.restore_backup(opened, files).await.unwrap();
        assert_eq!(report.identities, 0, "this account's Identity is kept");
    }

    let ours = identity_of(&c);
    let kept = attachment_ids(&c, ours);
    assert_eq!(kept.len(), 1, "a second restore adds nothing");
    let bytes = c.attachment_bytes(ours, kept[0]).await.unwrap();
    assert!(bytes.expose().ends_with(b"passport"));
    source.cleanup().await;
    target.cleanup().await;
}
