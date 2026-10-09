mod support;

use data_encoding::{BASE64, BASE64URL_NOPAD};
use havenkeys_server::billing::Status;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn a_frozen_account_is_refused_every_write_and_new_device() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let (item, rev) = create_item(&server, &sess, b"o", b"d").await;
    set_plan(&server, account.account_id, Status::Frozen).await;

    // Writes: 402, nothing applied.
    let (status, body) = write(&server, &sess, vec![change(item, Some(rev), b"o2", b"d2")]).await;
    assert_eq!(status, 402);
    assert_eq!(body["error"]["code"], "account_frozen");
    let (status, _) = write(&server, &sess, vec![deletion(item, Some(rev))]).await;
    assert_eq!(status, 402);
    let res = server
        .post_as("/v1/account/credentials", &sess)
        .json(&credentials_body(&account.auth_key, &[1u8; 32], 0, b"h2"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 402);

    // A device the account never used: 402. A known device, even after its
    // session expired: 200.
    let (status, body) = login_with_device(&server, &account, "New laptop", Uuid::new_v4()).await;
    assert_eq!(status, 402, "{body}");
    assert_eq!(body["error"]["code"], "account_frozen");
    server
        .db()
        .await
        .execute(
            "DELETE FROM sessions WHERE device_id = $1",
            &[&sess.device_id],
        )
        .await
        .unwrap();
    let (status, body) = login_with_device(&server, &account, "Desktop", sess.device_id).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["account"]["entitlement"], "frozen");
    let token = body["token"].as_str().unwrap().to_string();
    let sess = Sess { token, ..sess };

    // Reads and the ways out keep working.
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["entitlement"], "frozen");
    assert_eq!(
        pulled["changes"].as_array().unwrap().len(),
        1,
        "the item is still served"
    );
    assert_eq!(
        server
            .get_as("/v1/vault/header", &sess)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .get_as("/v1/devices", &sess)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let res = server
        .post_as("/v1/account/delete", &sess)
        .json(&json!({
            "currentAuthKey": BASE64.encode(&account.auth_key),
            "email": account.email,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "deletion is never blocked");
    server.cleanup().await;
}

#[tokio::test]
async fn a_frozen_account_cannot_approve_a_new_device() {
    let server = TestServer::start().await;
    let (account, phone) = signed_in(&server, "user@example.com").await;
    let desktop = Uuid::new_v4();
    let res = server
        .post("/v1/pairings")
        .json(&json!({
            "deviceId": desktop,
            "deviceName": "Desktop · Linux",
            "claimHash": BASE64URL_NOPAD.encode(&Sha256::digest([7u8; 32])),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let id = res.json::<Value>().await.unwrap()["pairingId"]
        .as_str()
        .unwrap()
        .to_string();
    set_plan(&server, account.account_id, Status::Frozen).await;
    let status = server
        .post_as(&format!("/v1/pairings/{id}/approve"), &phone)
        .json(&json!({ "envelope": BASE64.encode(&[1u8; 200]) }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 402);
    let registered: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM devices WHERE id = $1", &[&desktop])
        .await
        .unwrap()
        .get(0);
    assert_eq!(registered, 0, "a refused approval registers nothing");
    server.cleanup().await;
}

#[tokio::test]
async fn a_trial_past_its_end_is_frozen_without_any_job_running() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    set_plan(&server, account.account_id, Status::Trialing).await;
    let db = server.db().await;
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() - interval '1 minute' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let (status, _) = write(
        &server,
        &sess,
        vec![change(Uuid::new_v4(), None, b"o", b"d")],
    )
    .await;
    assert_eq!(status, 402);
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() + interval '1 day' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let (status, _) = write(
        &server,
        &sess,
        vec![change(Uuid::new_v4(), None, b"o", b"d")],
    )
    .await;
    assert_eq!(status, 200);
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["status"], "trialing");
    assert!(pulled["account"]["trialEndsAt"].is_string());
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn past_due_is_full_during_grace_then_frozen() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let db = server.db().await;
    havenkeys_server::billing::set_status(
        &db,
        account.account_id,
        havenkeys_server::billing::Actor::Provider,
        Status::PastDue,
        Some(chrono::Utc::now() + chrono::Duration::days(7)),
        "payment failed",
    )
    .await
    .unwrap();
    let (status, _) = write(
        &server,
        &sess,
        vec![change(Uuid::new_v4(), None, b"o", b"d")],
    )
    .await;
    assert_eq!(status, 200);
    db.execute(
        "UPDATE subscriptions SET grace_ends_at = now() - interval '1 minute' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let (status, _) = write(
        &server,
        &sess,
        vec![change(Uuid::new_v4(), None, b"o", b"d")],
    )
    .await;
    assert_eq!(status, 402);
    let events: i64 = db
        .query_one(
            "SELECT count(*) FROM billing_events WHERE account_id = $1 AND actor = 'provider' AND new_status = 'past_due'",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(events, 1);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn an_existing_account_is_complimentary_and_full() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["status"], "complimentary");
    assert_eq!(pulled["account"]["entitlement"], "full");
    assert!(pulled["account"]["trialEndsAt"].is_null());
    let (status, body) = login_with_device(&server, &account, "Phone", Uuid::new_v4()).await;
    assert_eq!(status, 200);
    assert_eq!(body["account"]["entitlement"], "full");
    server.cleanup().await;
}

#[tokio::test]
async fn activation_starts_a_fourteen_day_trial_for_a_trialing_account() {
    let server = TestServer::start().await;
    let printed = havenkeys_server::admin::run(
        havenkeys_server::admin::AdminCommand::NewAccount {
            email: "trial@example.com".into(),
            server_url: "https://vault.example.com".into(),
            trial: true,
        },
        server.pool(),
    )
    .await
    .unwrap();
    let invite = printed.trim().to_string();
    let vault_id = Uuid::new_v4();
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body(
            "trial@example.com",
            &invite,
            vault_id,
            &[5u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let account_id: Uuid = res.json::<Value>().await.unwrap()["accountId"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let days: f64 = server
        .db()
        .await
        .query_one(
            "SELECT (extract(epoch FROM trial_ends_at - now()) / 86400)::float8 FROM subscriptions WHERE account_id = $1",
            &[&account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert!((13.9..=14.0).contains(&days), "{days}");
    server.cleanup().await;
}

#[tokio::test]
async fn a_provider_event_is_the_only_other_writer() {
    use havenkeys_server::billing::provider::{apply_provider_event, ProviderEvent};
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let db = server.db().await;
    apply_provider_event(
        &db,
        account.account_id,
        ProviderEvent {
            provider: "stripe",
            customer: "cus_123".into(),
            reference: "sub_456".into(),
            status: Status::Active,
            period_end: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            reason: "invoice.payment_failed".into(),
        },
    )
    .await
    .unwrap();
    let row = db
        .query_one(
            "SELECT status, provider, provider_customer, provider_ref FROM subscriptions WHERE account_id = $1",
            &[&account.account_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "active");
    assert_eq!(row.get::<_, Option<String>>(1).as_deref(), Some("stripe"));
    assert_eq!(row.get::<_, Option<String>>(2).as_deref(), Some("cus_123"));
    assert_eq!(row.get::<_, Option<String>>(3).as_deref(), Some("sub_456"));
    let (status, _) = write(
        &server,
        &sess,
        vec![change(Uuid::new_v4(), None, b"o", b"d")],
    )
    .await;
    assert_eq!(
        status, 402,
        "active with a period end in the past is frozen"
    );
    let actor: String = db
        .query_one(
            "SELECT actor FROM billing_events WHERE account_id = $1 ORDER BY id DESC LIMIT 1",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(actor, "provider");
    drop(db);
    server.cleanup().await;
}
