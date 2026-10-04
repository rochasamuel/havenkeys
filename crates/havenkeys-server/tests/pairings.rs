mod support;

use data_encoding::{BASE64, BASE64URL_NOPAD};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::{Sess, TestServer};
use uuid::Uuid;

const SECRET: [u8; 32] = [7u8; 32];

fn create_body(device_id: Uuid) -> Value {
    json!({
        "deviceId": device_id,
        "deviceName": "Desktop · Linux",
        "publicKey": BASE64URL_NOPAD.encode(&[3u8; 32]),
        "claimHash": BASE64URL_NOPAD.encode(&Sha256::digest(SECRET)),
    })
}

async fn create(server: &TestServer, device_id: Uuid) -> String {
    let res = server
        .post("/v1/pairings")
        .json(&create_body(device_id))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["expiresAt"].is_string());
    body["pairingId"].as_str().unwrap().to_string()
}

async fn claim(server: &TestServer, id: &str, secret: [u8; 32]) -> (u16, Value) {
    let res = server
        .post(&format!("/v1/pairings/{id}/claim"))
        .json(&json!({ "claimSecret": BASE64URL_NOPAD.encode(&secret) }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn approve(server: &TestServer, id: &str, phone: &Sess) -> u16 {
    server
        .post_as(&format!("/v1/pairings/{id}/approve"), phone)
        .json(&json!({ "envelope": BASE64.encode(&[1u8; 200]) }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn an_approved_pairing_is_claimed_once_with_a_working_session() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let desktop = Uuid::new_v4();
    let id = create(&server, desktop).await;

    assert_eq!(claim(&server, &id, SECRET).await.1["state"], "waiting");

    let details: Value = server
        .get_as(&format!("/v1/pairings/{id}"), &phone)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(details["deviceName"], "Desktop · Linux");
    assert_eq!(details["ip"], "127.0.0.1");
    assert!(details["location"].is_null());

    assert_eq!(approve(&server, &id, &phone).await, 204);
    let (status, body) = claim(&server, &id, SECRET).await;
    assert_eq!(status, 200);
    assert_eq!(body["state"], "approved");
    assert_eq!(body["accountId"], json!(phone.account_id));
    assert_eq!(body["vaultId"], json!(phone.vault_id));
    assert_eq!(
        BASE64
            .decode(body["envelope"].as_str().unwrap().as_bytes())
            .unwrap(),
        vec![1u8; 200]
    );

    // The token is the desktop's session, and the device says who approved it.
    let token = body["token"].as_str().unwrap();
    let devices: Value = server
        .get("/v1/devices")
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let me = devices
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["current"] == true)
        .unwrap();
    assert_eq!(me["id"], json!(desktop));
    assert_eq!(me["approvedBy"], json!(phone.device_id));

    // Single use.
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_secret_claims_nothing_and_counts_against_the_address() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(approve(&server, &id, &phone).await, 204);
    for _ in 0..5 {
        assert_eq!(claim(&server, &id, [8u8; 32]).await.0, 404);
    }
    // Blocked now, even with the right secret.
    assert_eq!(claim(&server, &id, SECRET).await.0, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn another_account_cannot_see_approve_or_deny_a_pairing_bound_to_the_first() {
    let server = TestServer::start().await;
    let (_, ana) = support::signed_in(&server, "ana@example.com").await;
    let (_, bob) = support::signed_in(&server, "bob@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(
        server
            .get_as(&format!("/v1/pairings/{id}"), &ana)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .get_as(&format!("/v1/pairings/{id}"), &bob)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(approve(&server, &id, &bob).await, 404);
    assert_eq!(
        server
            .post_as(&format!("/v1/pairings/{id}/deny"), &bob)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(approve(&server, &id, &ana).await, 204);
    server.cleanup().await;
}

#[tokio::test]
async fn a_denied_pairing_says_so_once() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(
        server
            .post_as(&format!("/v1/pairings/{id}/deny"), &phone)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(approve(&server, &id, &phone).await, 404);
    assert_eq!(claim(&server, &id, SECRET).await.1["state"], "denied");
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_pairing_is_gone() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    server
        .db()
        .await
        .execute(
            "UPDATE pairings SET expires_at = now() - interval '1 second' WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        server
            .get_as(&format!("/v1/pairings/{id}"), &phone)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(approve(&server, &id, &phone).await, 404);
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn an_approved_pairing_never_claimed_is_gone_after_ten_minutes() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(approve(&server, &id, &phone).await, 204);
    server
        .db()
        .await
        .execute(
            "UPDATE pairings SET created_at = now() - interval '11 minutes' WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();
    create(&server, Uuid::new_v4()).await;
    let left: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM pairings WHERE id = $1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        left, 0,
        "the envelope and token do not outlive the row's ten minutes"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn one_address_may_hold_three_pending_and_create_ten_per_window() {
    let server = TestServer::start().await;
    for _ in 0..3 {
        create(&server, Uuid::new_v4()).await;
    }
    let res = server
        .post("/v1/pairings")
        .json(&create_body(Uuid::new_v4()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 429);
    server
        .db()
        .await
        .execute("UPDATE pairings SET state = 'claimed'", &[])
        .await
        .unwrap();
    for _ in 0..7 {
        create(&server, Uuid::new_v4()).await;
        server
            .db()
            .await
            .execute("UPDATE pairings SET state = 'claimed'", &[])
            .await
            .unwrap();
    }
    let res = server
        .post("/v1/pairings")
        .json(&create_body(Uuid::new_v4()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 429);
    server.cleanup().await;
}

#[tokio::test]
async fn malformed_requests_are_refused() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    for body in [
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "D", "publicKey": "AA", "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]) }),
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "", "publicKey": BASE64URL_NOPAD.encode(&[0u8; 32]), "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]) }),
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "D\u{7}", "publicKey": BASE64URL_NOPAD.encode(&[0u8; 32]), "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]) }),
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "D", "publicKey": BASE64URL_NOPAD.encode(&[0u8; 32]), "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]), "accountId": Uuid::new_v4() }),
    ] {
        assert_eq!(
            server
                .post("/v1/pairings")
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    }
    let id = create(&server, Uuid::new_v4()).await;
    let too_big = json!({ "envelope": BASE64.encode(&vec![0u8; 4097]) });
    assert_eq!(
        server
            .post_as(&format!("/v1/pairings/{id}/approve"), &phone)
            .json(&too_big)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    // The approving device cannot approve itself in.
    let own = create(&server, phone.device_id).await;
    assert_eq!(approve(&server, &own, &phone).await, 400);
    server.cleanup().await;
}

#[tokio::test]
async fn a_revoked_device_id_cannot_be_approved_in() {
    let server = TestServer::start().await;
    let (account, phone) = support::signed_in(&server, "ana@example.com").await;
    let old = support::login(&server, &account, "Old laptop").await;
    assert_eq!(
        server
            .delete_as(&format!("/v1/devices/{}", old.device_id), &phone)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let id = create(&server, old.device_id).await;
    assert_eq!(approve(&server, &id, &phone).await, 401);
    server.cleanup().await;
}

#[tokio::test]
async fn two_concurrent_claims_get_the_session_once() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(approve(&server, &id, &phone).await, 204);
    let (a, b) = tokio::join!(claim(&server, &id, SECRET), claim(&server, &id, SECRET));
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 404]);
    server.cleanup().await;
}

#[tokio::test]
async fn an_approved_pairing_older_than_ten_minutes_is_gone_without_a_create() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(approve(&server, &id, &phone).await, 204);
    server
        .db()
        .await
        .execute(
            "UPDATE pairings SET created_at = now() - interval '11 minutes' WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}
