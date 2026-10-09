//! The phone's writes against the real server and a real Postgres. Needs
//! Postgres: `scripts/test-server.sh` starts one. Run with
//! `cargo test -p havenkeys-mobile --features server-tests --test round_trip`.

use havenkeys_mobile::*;
use havenkeys_server::admin::{self, AdminCommand};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const EMAIL: &str = "user@example.com";
const DEFAULT_URL: &str = "postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable";
const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";

struct Quiet;
impl VaultEvents for Quiet {
    fn locked(&self, _: String) {}
    fn unlocked(&self) {}
    fn connectivity(&self, _: bool) {}
    fn signed_out(&self) {}
    fn items_changed(&self) {}
    fn removed(&self) {}
    fn account_deleted(&self) {}
}

struct Xor;
impl KeystoreCipher for Xor {
    fn seal(&self, p: Vec<u8>) -> Result<Vec<u8>, CipherError> {
        Ok(p.iter().map(|b| b ^ 1).collect())
    }
    fn open(&self, s: Vec<u8>) -> Result<Vec<u8>, CipherError> {
        Ok(s.iter().map(|b| b ^ 1).collect())
    }
}

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
        let db_name = format!("hk_mobile_{}", Uuid::new_v4().simple());
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

    async fn invite(&self) -> String {
        admin::run(
            AdminCommand::NewAccount {
                email: EMAIL.into(),
                server_url: self.base.clone(),
                trial: false,
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

/// Counts `items_changed`, the app's cue to reload its lists.
#[derive(Default)]
struct ItemsChanged(std::sync::atomic::AtomicUsize);
impl ItemsChanged {
    fn take(&self) -> usize {
        self.0.swap(0, std::sync::atomic::Ordering::SeqCst)
    }

    /// Events reach the app on another thread: wait for one.
    fn arrives(&self) -> bool {
        for _ in 0..200 {
            if self.take() > 0 {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}
impl VaultEvents for ItemsChanged {
    fn locked(&self, _: String) {}
    fn unlocked(&self) {}
    fn connectivity(&self, _: bool) {}
    fn signed_out(&self) {}
    fn items_changed(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn removed(&self) {}
    fn account_deleted(&self) {}
}

fn phone(dir: &std::path::Path) -> Arc<MobileVault> {
    phone_with(dir, Arc::new(Quiet))
}

fn phone_with(dir: &std::path::Path, events: Arc<dyn VaultEvents>) -> Arc<MobileVault> {
    MobileVault::new(
        MobileConfig {
            data_dir: dir.to_string_lossy().into_owned(),
            own_package: "net.havenkeys.android".into(),
            device_name: "Pixel 8".into(),
        },
        events,
        Arc::new(Xor),
    )
    .unwrap()
}

fn online(v: &MobileVault) {
    for _ in 0..200 {
        if v.status().unwrap().online {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("never came online");
}

fn code(e: MobileError) -> String {
    match e {
        MobileError::Failed { code, .. } => code,
    }
}

fn chrome(domain: &str) -> TargetFacts {
    TargetFacts {
        package_name: "com.android.chrome".into(),
        signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(CHROME)
            .unwrap()
            .to_vec()],
        web_domain: Some(domain.into()),
        web_scheme: Some("https".into()),
    }
}

/// `base_revision` is the revision of the `ItemEdit` the draft was made from.
fn draft(title: &str, changes: Vec<FieldChange>, base_revision: Option<i64>) -> ItemDraft {
    ItemDraft {
        kind: ItemKind::Login,
        title: title.into(),
        websites: vec![Website {
            url: "github.com".into(),
            match_kind: MatchKind::Domain,
        }],
        changes,
        base_revision,
        tags: Vec::new(),
    }
}

fn replace(key: &str, value: &str) -> FieldChange {
    FieldChange {
        key: key.into(),
        change: Change::Replace {
            value: value.into(),
        },
    }
}

#[test]
fn two_phones_edit_one_vault_through_the_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start());
    let invite = rt.block_on(server.invite());

    let dir_a = tempfile::tempdir().unwrap();
    let a = phone(dir_a.path());
    a.activate(invite, PASSWORD.into()).unwrap();
    online(&a);

    // Create, then read it back.
    let id = a
        .create_item(draft(
            "GitHub",
            vec![
                replace("username", "octo"),
                replace("password", "hunter2hunter2"),
            ],
            None,
        ))
        .unwrap();
    assert_eq!(
        a.reveal(id.clone(), "password".into()).unwrap(),
        "hunter2hunter2"
    );

    // A second phone signs in and sees it.
    let dir_b = tempfile::tempdir().unwrap();
    let b = phone(dir_b.path());
    b.sign_in(
        server.base.clone(),
        EMAIL.into(),
        PASSWORD.into(),
        testing::secret_key_text(&a),
    )
    .unwrap();
    online(&b);
    b.sync_now().unwrap();
    assert!(b.list_items().unwrap().iter().any(|s| s.id == id));

    // B edits first; A's edit, made from the older revision, is refused by
    // the server.
    let a_opened = a.item_edit(id.clone()).unwrap().revision;
    let b_opened = b.item_edit(id.clone()).unwrap().revision;
    b.update_item(id.clone(), draft("B's title", vec![], b_opened))
        .unwrap();
    let refused = a
        .update_item(id.clone(), draft("A's title", vec![], a_opened))
        .err()
        .unwrap();
    assert_eq!(code(refused), "item_changed_elsewhere");
    assert_eq!(a.item_view(id.clone()).unwrap().summary.title, "GitHub");

    // A pull brings B's change while A's editor is still open: that edit is
    // refused on the phone, before anything is sent.
    a.sync_now().unwrap();
    let refused = a
        .update_item(id.clone(), draft("A's title", vec![], a_opened))
        .err()
        .unwrap();
    assert_eq!(code(refused), "item_changed_elsewhere");
    assert_eq!(a.item_view(id.clone()).unwrap().summary.title, "B's title");

    // Reopened, A sees B's change and can edit again; the password stayed.
    let reopened = a.item_edit(id.clone()).unwrap();
    assert_eq!(reopened.title, "B's title");
    a.update_item(id.clone(), draft("A's title", vec![], reopened.revision))
        .unwrap();
    assert_eq!(
        a.reveal(id.clone(), "password".into()).unwrap(),
        "hunter2hunter2"
    );

    // Saving from Autofill: add, then unchanged, then update.
    let login = |pw: &str| SaveLogin {
        username: Some("ana".into()),
        password: pw.into(),
        current_password: None,
        title: None,
    };
    assert!(matches!(
        a.autofill_save(chrome("example.com"), login("first-pass"))
            .unwrap(),
        SaveResult::Added
    ));
    assert!(matches!(
        a.autofill_save(chrome("example.com"), login("first-pass"))
            .unwrap(),
        SaveResult::Unchanged
    ));
    assert!(matches!(
        a.autofill_save(chrome("example.com"), login("second-pass"))
            .unwrap(),
        SaveResult::Updated
    ));

    // Trash, then delete for good: both reach the other phone.
    a.trash_item(id.clone()).unwrap();
    assert!(a.list_items().unwrap().iter().all(|s| s.id != id));
    assert!(a.list_trash().unwrap().iter().any(|s| s.item.id == id));
    b.sync_now().unwrap();
    assert!(b.list_items().unwrap().iter().all(|s| s.id != id));
    assert!(b.list_trash().unwrap().iter().any(|s| s.item.id == id));
    a.purge_item(id.clone()).unwrap();
    assert!(a.list_trash().unwrap().iter().all(|s| s.item.id != id));
    b.sync_now().unwrap();
    assert!(b.list_items().unwrap().iter().all(|s| s.id != id));
    assert!(b.list_trash().unwrap().iter().all(|s| s.item.id != id));

    // A passkey created on phone A for a site in Chrome reaches phone B,
    // which signs in with it.
    let chrome_on = |origin: &str| CredentialCaller {
        package_name: "com.android.chrome".into(),
        signing_certs: chrome("github.com").signing_certs,
        origin: Some(origin.into()),
    };
    let create = r#"{"rp":{"id":"webauthn.io"},"user":{"id":"AQID","name":"ana"},"challenge":"BwcH","pubKeyCredParams":[{"type":"public-key","alg":-7}]}"#;
    let registration: serde_json::Value = serde_json::from_str(
        &a.passkey_create(chrome_on("https://webauthn.io"), create.into(), None)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(registration["type"], "public-key");
    b.sync_now().unwrap();
    let get = r#"{"challenge":"AwMD","rpId":"webauthn.io"}"#;
    let offers = b
        .passkey_offers(chrome_on("https://webauthn.io"), get.into())
        .unwrap();
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].user_name, "ana");
    b.passkey_sign_in(
        chrome_on("https://webauthn.io"),
        get.into(),
        None,
        offers[0].item_id.clone(),
        offers[0].credential_id.clone(),
    )
    .unwrap();
    // Asked again with the passkey excluded, A refuses to make a second one.
    let excluding = create.replace(
        r#""challenge""#,
        &format!(
            r#""excludeCredentials":[{{"type":"public-key","id":"{}"}}],"challenge""#,
            registration["id"].as_str().unwrap()
        ),
    );
    assert_eq!(
        code(
            a.passkey_create(chrome_on("https://webauthn.io"), excluding, None)
                .err()
                .unwrap()
        ),
        "passkey_exists"
    );

    // A card typed into a checkout on phone A and confirmed in Android's
    // save sheet reaches phone B, which fills it; typing it again changes
    // nothing.
    let typed = || SaveCard {
        cardholder_name: Some("Ana Souza".into()),
        number: "4000 0566 5566 5556".into(),
        verification_number: Some("321".into()),
        expiry: Some("2031-07".into()),
    };
    let checkout = FrameFacts {
        web_domain: None,
        web_scheme: None,
    };
    assert!(matches!(
        a.autofill_save_card(chrome("shop.example.com"), checkout.clone(), typed())
            .unwrap(),
        SaveResult::Added
    ));
    assert!(matches!(
        a.autofill_save_card(chrome("shop.example.com"), checkout.clone(), typed())
            .unwrap(),
        SaveResult::Unchanged
    ));
    b.sync_now().unwrap();
    let on_b = b
        .autofill_cards(chrome("shop.example.com"), vec![checkout.clone()])
        .unwrap();
    let saved = on_b
        .cards
        .iter()
        .find(|c| c.last4.as_deref() == Some("5556"))
        .unwrap();
    assert_eq!(saved.expiry.as_deref(), Some("2031-07"));
    let values = b
        .autofill_card_values(
            saved.id.clone(),
            chrome("shop.example.com"),
            vec![CardFrameRoles {
                frame: checkout,
                roles: vec![CardRole::VerificationNumber],
            }],
        )
        .unwrap();
    assert_eq!(values[0][0].value, "321");

    rt.block_on(server.cleanup());
}

#[test]
fn a_refused_empty_trash_still_tells_the_app_to_reload() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start());
    let invite = rt.block_on(server.invite());
    let dir_a = tempfile::tempdir().unwrap();
    let events = Arc::new(ItemsChanged::default());
    let a = phone_with(dir_a.path(), events.clone());
    a.activate(invite, PASSWORD.into()).unwrap();
    online(&a);
    let id = a.create_item(draft("GitHub", vec![], None)).unwrap();
    a.trash_item(id.clone()).unwrap();

    // Phone B deletes it for good first; A has not synced since.
    let dir_b = tempfile::tempdir().unwrap();
    let b = phone(dir_b.path());
    b.sign_in(
        server.base.clone(),
        EMAIL.into(),
        PASSWORD.into(),
        testing::secret_key_text(&a),
    )
    .unwrap();
    online(&b);
    b.sync_now().unwrap();
    b.purge_item(id.clone()).unwrap();

    while events.arrives() {}
    let refused = a
        .empty_trash()
        .err()
        .expect("the server refuses A's stale tombstone");
    assert_eq!(code(refused), "item_changed_elsewhere");
    assert!(events.arrives(), "the app is told to reload its counts");

    rt.block_on(server.cleanup());
}

#[test]
fn a_phone_dismisses_a_health_check_through_the_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start());
    let invite = rt.block_on(server.invite());
    let dir = tempfile::tempdir().unwrap();
    let v = phone(dir.path());
    v.activate(invite, PASSWORD.into()).unwrap();
    online(&v);
    v.create_item(draft(
        "GitHub",
        vec![
            replace("username", "octo"),
            replace("password", "hunter2hunter2"),
        ],
        None,
    ))
    .unwrap();
    let report = v.health_report().unwrap();
    let issue = report
        .issues
        .iter()
        .find(|i| !i.dismissed)
        .expect("the github login has an issue");
    assert!(issue.kinds.contains(&HealthKind::Passkey));
    v.set_health_ignored(issue.item_id.clone(), vec![HealthKind::Passkey])
        .unwrap();
    let after = v.health_report().unwrap();
    assert!(after
        .issues
        .iter()
        .any(|i| i.dismissed && i.kinds == vec![HealthKind::Passkey]));
    rt.block_on(server.cleanup());
}

struct QuietClient;

impl havenkeys_client::ClientEvents for QuietClient {
    fn unlocked(&self, _: u32) {}
    fn locked(&self, _: &'static str, _: bool) {}
    fn connectivity(&self, _: bool) {}
    fn signed_out(&self) {}
    fn synced(&self, _: havenkeys_core::sync::SyncReport) {}
    fn items_changed(&self) {}
    fn removed(&self, _: bool) {}
    fn account_deleted(&self, _: bool) {}
}

/// A bare desktop client with no vault, as the sign-in screen has.
fn desktop_client(dir: &std::path::Path) -> Arc<havenkeys_client::HavenClient> {
    use havenkeys_client::device::Device;
    use havenkeys_client::key_store::MemoryKeyStore;
    havenkeys_client::HavenClient::new(
        Arc::new(std::sync::Mutex::new(
            havenkeys_core::vault::VaultService::new(
                havenkeys_core::store::Store::open_in_memory().unwrap(),
            ),
        )),
        Device::load(dir, Box::new(MemoryKeyStore::default())),
        None,
        Arc::new(QuietClient),
        havenkeys_client::ClientConfig {
            device_name: "Test".into(),
            vault_path: dir.join("vault.sqlite3"),
        },
    )
}

#[test]
fn the_phone_approves_a_new_desktop_once() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start());
    let invite = rt.block_on(server.invite());

    let phone_dir = tempfile::tempdir().unwrap();
    let phone = phone(phone_dir.path());
    phone.activate(invite, PASSWORD.into()).unwrap();
    online(&phone);

    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = desktop_client(desk_dir.path());
    let start = rt
        .block_on(desktop.start_pairing(format!("{}/", server.base), "Desktop"))
        .unwrap();

    let request = phone.pairing_request(start.link.clone()).unwrap();
    assert_eq!(request.device_name, "Desktop");
    phone.approve_pairing(start.link.clone()).unwrap();
    assert!(matches!(
        rt.block_on(desktop.poll_pairing()).unwrap(),
        havenkeys_client::PairingPoll::Approved(_)
    ));

    let again = phone.approve_pairing(start.link).err().unwrap();
    assert_eq!(code(again), "pairing_gone");

    rt.block_on(server.cleanup());
}

#[test]
fn a_phone_without_an_open_vault_cannot_delete_the_account() {
    let dir = tempfile::tempdir().unwrap();
    let v = phone(dir.path());
    let err = v.delete_account(EMAIL.into(), PASSWORD.into()).unwrap_err();
    assert_eq!(code(err), "locked");
}

#[test]
fn tags_round_trip_through_the_draft_and_the_summary() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(Server::start());
    let invite = rt.block_on(server.invite());
    let dir = tempfile::tempdir().unwrap();
    let a = phone(dir.path());
    a.activate(invite, PASSWORD.into()).unwrap();
    online(&a);

    let id = a
        .create_item(ItemDraft {
            tags: vec!["Work".into()],
            ..draft("GitHub", vec![], None)
        })
        .unwrap();
    let tags_of = |a: &MobileVault| {
        a.list_items()
            .unwrap()
            .into_iter()
            .find(|s| s.id == id)
            .unwrap()
            .tags
    };
    assert_eq!(tags_of(&a), vec!["work".to_string()]);
    let edit = a.item_edit(id.clone()).unwrap();
    assert_eq!(edit.tags, vec!["work".to_string()]);

    a.update_item(
        id.clone(),
        ItemDraft {
            tags: vec![],
            ..draft("GitHub", vec![], edit.revision)
        },
    )
    .unwrap();
    assert!(tags_of(&a).is_empty());
}
