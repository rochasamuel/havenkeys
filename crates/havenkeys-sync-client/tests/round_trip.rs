//! The whole wire, end to end: real core crypto, the real server, a real
//! Postgres. Nothing is stubbed, because what this proves is exactly what a
//! stub cannot — that the bytes one side writes are the bytes the other side
//! reads, and that a second device can open a vault it never created.
//!
//! Needs Postgres: `scripts/test-server.sh` starts one.

use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretField, SecretUpdate, UrlRule};
use havenkeys_core::store::{AccountRecord, Store};
use havenkeys_core::sync::prepare_sign_in;
use havenkeys_core::vault::{derive_auth_key, prepare_new_account_vault, VaultService};
use havenkeys_core::SecretString;
use havenkeys_server::admin::{self, AdminCommand};
use havenkeys_sync_client::{Activation, HttpTransport, Session, SyncClient, SyncError};
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const NOW: i64 = 1_700_000_000_000;
const DEFAULT_URL: &str = "postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable";

/// The cheapest cost the core accepts. Real vaults use far more; this test is
/// about the wire, and four Argon2id runs at production cost would make it
/// slow enough to be skipped, which is worse.
fn cheap_kdf() -> KdfParams {
    KdfParams::with_cost(19 * 1024, 2, 1).unwrap()
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
        let pool = havenkeys_server::db::connect(&url).unwrap();
        havenkeys_server::db::migrate(&pool).await.unwrap();

        let state = havenkeys_server::AppState {
            pool: pool.clone(),
            server_secret: [5u8; 32],
            trust_forwarded_for: false,
            cors_origin: None,
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

    fn client(&self) -> SyncClient<HttpTransport> {
        SyncClient::new(HttpTransport::new(&self.base).unwrap())
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

/// Send staged writes and record what the server accepted, which is the
/// `stage` → send → `commit_write` cycle every desktop write goes through.
async fn push(
    client: &SyncClient<HttpTransport>,
    device: &mut Device,
    staged: Vec<havenkeys_core::vault::StagedWrite>,
) -> Result<i64, SyncError> {
    let ack = client.write(&device.session, &staged).await?;
    let revisions: std::collections::HashMap<Uuid, i64> = ack.applied.iter().copied().collect();
    for write in staged {
        let revision = *revisions
            .get(&write.item_id)
            .expect("the server acknowledged every item it applied");
        device.vault.commit_write(write, revision).unwrap();
    }
    Ok(ack.cursor)
}

fn login_item(title: &str, password: &str) -> ItemInput {
    ItemInput {
        item_type: ItemType::Login,
        title: title.into(),
        username: Some("me@example.com".into()),
        urls: vec![UrlRule {
            url: "github.com".into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(SecretString::from(password)),
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
    }
}

/// A device that has activated an account: its vault, its Secret Key and its
/// session.
struct Device {
    vault: VaultService,
    secret_key: SecretKey,
    account: AccountRef,
    session: Session,
}

async fn activate(server: &Server, email: &str) -> Device {
    let invite = server.invite(email).await;
    let parsed = havenkeys_server::invite::decode(&invite).unwrap();
    let account = AccountRef::new(parsed.account, NormalizedEmail::parse(email).unwrap());

    // Everything cryptographic happens here, on the device.
    let made = prepare_new_account_vault(&SecretString::from(PASSWORD), &account, cheap_kdf(), NOW)
        .unwrap();
    let kdf = made.prepared.kdf().clone();
    let vault_id = made.prepared.vault_id();
    let secret_key = made.secret_key;
    let auth_key = made.auth_key;

    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault
        .create_account_vault(
            made.prepared,
            &AccountRecord {
                account_id: account.id,
                email: email.into(),
                server_url: server.base.clone(),
                server_cursor: 0,
                max_header_rev: 0,
                last_synced_at: None,
            },
        )
        .unwrap();
    let header = vault.encode_account_header().unwrap();

    let client = server.client();
    let activated = client
        .activate(Activation {
            email,
            invite: &invite,
            kdf: &kdf,
            auth_key: &auth_key,
            vault_id,
            header: &header,
        })
        .await
        .unwrap();
    assert_eq!(activated.account_id, account.id);
    assert_eq!(activated.vault_id, vault_id);

    let session = client
        .login(email, &auth_key, account.id, Uuid::new_v4(), "Desktop")
        .await
        .unwrap();
    assert_eq!(session.vault_id, vault_id);

    Device {
        vault,
        secret_key,
        account,
        session,
    }
}

#[tokio::test]
async fn an_item_written_on_one_device_opens_on_another() {
    let server = Server::start().await;
    let client = server.client();
    let mut first = activate(&server, "user@example.com").await;

    // Write an item: staged under the vault lock, sent, then recorded with
    // the revision the server assigned.
    let staged = first
        .vault
        .stage_create(login_item("GitHub", "s3cret-pw"), NOW)
        .unwrap();
    let item_id = staged.item_id;
    push(&client, &mut first, vec![staged]).await.unwrap();

    // A second device signs in with the master password and the Secret Key.
    let params = client.auth_params("user@example.com").await.unwrap();
    assert_eq!(params.account_id, first.account.id);
    let auth_key = derive_auth_key(
        &SecretString::from(PASSWORD),
        &first.secret_key,
        &params.kdf,
        &first.account,
    )
    .unwrap();
    let second_session = client
        .login(
            "user@example.com",
            &auth_key,
            first.account.id,
            Uuid::new_v4(),
            "Laptop",
        )
        .await
        .unwrap();

    let header = client.header(&second_session).await.unwrap();
    let (prepared, _) = prepare_sign_in(
        &header.bytes,
        &SecretString::from(PASSWORD),
        &first.secret_key,
        &first.account,
    )
    .unwrap();

    let mut second = VaultService::new(Store::open_in_memory().unwrap());
    second
        .create_account_vault(
            prepared,
            &AccountRecord {
                account_id: first.account.id,
                email: "user@example.com".into(),
                server_url: server.base.clone(),
                server_cursor: 0,
                max_header_rev: header.revision,
                last_synced_at: None,
            },
        )
        .unwrap();
    // `create_account_vault` leaves the vault unlocked: the caller has just
    // proved it holds the keys.

    let pulled = client.pull(&second_session, 0).await.unwrap();
    assert!(!pulled.changes.is_empty());
    let report = second
        .apply_remote_changes(pulled.cursor, pulled.changes, NOW)
        .unwrap();
    assert!(report.added >= 1);
    assert_eq!(
        report.skipped_items, 0,
        "the blobs opened on the new device"
    );

    let item = second.get_item(&item_id).unwrap();
    assert_eq!(item.title, "GitHub");
    assert_eq!(
        second
            .reveal(&item_id, SecretField::Password)
            .unwrap()
            .expose(),
        "s3cret-pw",
        "the plaintext never left either device, and both agree on it"
    );

    server.cleanup().await;
}

#[tokio::test]
async fn a_write_that_lost_a_race_comes_back_as_a_conflict() {
    let server = Server::start().await;
    let client = server.client();
    let mut device = activate(&server, "race@example.com").await;

    let staged = device
        .vault
        .stage_create(login_item("GitHub", "first"), NOW)
        .unwrap();
    let id = staged.item_id;
    let first_cursor = push(&client, &mut device, vec![staged]).await.unwrap();

    // Stage an edit, then let another write land before it is sent: the
    // staged one now carries a revision the server has moved past.
    let stale = device
        .vault
        .stage_update(&id, login_item("GitHub", "third"), NOW)
        .unwrap();
    let winner = device
        .vault
        .stage_update(&id, login_item("GitHub", "second"), NOW)
        .unwrap();
    let second_cursor = push(&client, &mut device, vec![winner]).await.unwrap();
    assert!(second_cursor > first_cursor);

    let err = client.write(&device.session, &[stale]).await.unwrap_err();
    match err {
        SyncError::Conflict(items) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].item_id, id);
        }
        other => panic!("expected a conflict, got {other:?}"),
    }

    server.cleanup().await;
}

#[tokio::test]
async fn a_header_published_by_one_device_is_adopted_by_another() {
    let server = Server::start().await;
    let client = server.client();
    let device = activate(&server, "header@example.com").await;

    let header = device.vault.encode_account_header().unwrap();
    client
        .put_header(&device.session, &header, 1)
        .await
        .unwrap();

    let fetched = client.header(&device.session).await.unwrap();
    assert_eq!(fetched.bytes, header);
    assert_eq!(fetched.revision, 1);
    assert_eq!(fetched.key_scheme, 3);

    // The same revision twice is a conflict, not an overwrite.
    let err = client
        .put_header(&device.session, &header, 1)
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::Conflict(_)), "{err:?}");

    server.cleanup().await;
}

#[tokio::test]
async fn a_revoked_device_is_signed_out_at_its_next_request() {
    let server = Server::start().await;
    let client = server.client();
    let device = activate(&server, "revoke@example.com").await;

    let devices = client.devices(&device.session).await.unwrap();
    assert_eq!(devices.len(), 1);
    assert!(devices[0].current);

    client
        .revoke_device(&device.session, devices[0].id)
        .await
        .unwrap();
    assert_eq!(
        client.pull(&device.session, 0).await.unwrap_err(),
        SyncError::Unauthorized
    );

    server.cleanup().await;
}
