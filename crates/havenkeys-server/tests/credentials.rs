mod support;

use serde_json::Value;
use support::*;

const NEW_KEY: [u8; 32] = [42u8; 32];

async fn change(server: &TestServer, sess: &Sess, current: &[u8; 32], base: i64) -> (u16, Value) {
    let res = server
        .post_as("/v1/account/credentials", sess)
        .json(&credentials_body(
            current,
            &NEW_KEY,
            base,
            b"header-bytes-v2",
        ))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    let body = res.json().await.unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
async fn a_change_replaces_verifier_kdf_and_header_together() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "change@example.com").await;

    let (status, body) = change(&server, &sess, &account.auth_key, 0).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["headerRevision"], 1);

    // The header moved.
    let header: Value = server
        .get_as("/v1/vault/header", &sess)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(header["headerRevision"], 1);
    assert_eq!(
        data_encoding::BASE64
            .decode(header["header"].as_str().unwrap().as_bytes())
            .unwrap(),
        b"header-bytes-v2".to_vec()
    );
    // The params moved.
    let params: Value = server
        .post("/v1/auth/params")
        .json(&serde_json::json!({ "email": account.email }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        params["kdf"]["salt"],
        data_encoding::BASE64.encode(&[4u8; 16])
    );
    server.cleanup().await;
}

#[tokio::test]
async fn the_old_key_stops_working_and_the_new_one_logs_in() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "keys@example.com").await;
    let old_key = account.auth_key;
    assert_eq!(change(&server, &sess, &old_key, 0).await.0, 200);

    let res = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&old_key),
            "deviceId": uuid::Uuid::new_v4(),
            "deviceName": "Old",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);

    let renewed = Account {
        auth_key: NEW_KEY,
        ..account
    };
    login(&server, &renewed, "Laptop").await;
    server.cleanup().await;
}

#[tokio::test]
async fn other_sessions_are_revoked_and_the_caller_is_kept() {
    let server = TestServer::start().await;
    let (account, first) = signed_in(&server, "revoke-others@example.com").await;
    let second = login(&server, &account, "Laptop").await;

    assert_eq!(change(&server, &first, &account.auth_key, 0).await.0, 200);

    assert_eq!(
        server
            .get_as("/v1/sync?since=0", &first)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        server
            .get_as("/v1/sync?since=0", &second)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_current_key_is_refused_and_counted() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "wrong@example.com").await;

    let (status, _) = change(&server, &sess, &[1u8; 32], 0).await;
    assert_eq!(status, 401);
    let failures: i32 = server
        .db()
        .await
        .query_one(
            "SELECT failures FROM login_attempts WHERE key = $1",
            &[&format!("acct:{}", account.account_id)],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(failures, 1);
    // Nothing moved.
    let header: Value = server
        .get_as("/v1/vault/header", &sess)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(header["headerRevision"], 0);
    server.cleanup().await;
}

#[tokio::test]
async fn a_stale_base_revision_is_a_conflict() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "stale@example.com").await;
    let (status, _) = change(&server, &sess, &account.auth_key, 5).await;
    assert_eq!(status, 409);
    server.cleanup().await;
}

#[tokio::test]
async fn a_malformed_change_is_refused() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "malformed@example.com").await;
    let mut body = credentials_body(&account.auth_key, &NEW_KEY, 0, b"h");
    body["kdf"]["memoryKib"] = 1.into();
    let res = server
        .post_as("/v1/account/credentials", &sess)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let mut body = credentials_body(&account.auth_key, &NEW_KEY, 0, b"h");
    body["extra"] = true.into();
    let res = server
        .post_as("/v1/account/credentials", &sess)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let body = credentials_body(&account.auth_key, &NEW_KEY, 0, b"");
    let res = server
        .post_as("/v1/account/credentials", &sess)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn the_header_route_no_longer_accepts_writes() {
    let server = TestServer::start().await;
    let (_account, sess) = signed_in(&server, "noput@example.com").await;
    let res = server
        .put_as("/v1/vault/header", &sess)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 405);
    server.cleanup().await;
}
