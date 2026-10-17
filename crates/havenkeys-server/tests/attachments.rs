mod support;

use data_encoding::BASE64;
use havenkeys_server::billing::Status;
use havenkeys_server::limits::{
    MAX_ATTACHMENT_BLOB_BYTES, MAX_ATTACHMENT_VAULT_BYTES, MAX_VAULT_BYTES,
};
use serde_json::{json, Value};
use support::*;
use uuid::Uuid;

async fn put(server: &TestServer, sess: &Sess, id: Uuid, item: Uuid, blob: &[u8]) -> (u16, Value) {
    let res = server
        .put_as(&format!("/v1/attachments/{id}"), sess)
        .json(&json!({ "itemId": item, "blob": BASE64.encode(blob) }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn get(server: &TestServer, sess: &Sess, id: Uuid) -> (u16, Value) {
    let res = server
        .get_as(&format!("/v1/attachments/{id}"), sess)
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

/// Marks an uploaded attachment as referenced by an item write, which Task 5
/// will do through `attachmentIds`.
async fn commit(server: &TestServer, id: Uuid) {
    let db = server.db().await;
    db.execute(
        "UPDATE attachments SET committed = true WHERE attachment_id = $1",
        &[&id],
    )
    .await
    .unwrap();
}

/// A row that counts towards the quotas, inserted without going through PUT.
async fn seed(server: &TestServer, vault: Uuid, bytes: i32) {
    let db = server.db().await;
    db.execute(
        "INSERT INTO attachments (vault_id, attachment_id, item_id, blob)
         VALUES ($1, $2, $3, decode(repeat('00', $4), 'hex'))",
        &[&vault, &Uuid::new_v4(), &Uuid::new_v4(), &bytes],
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn put_then_get_after_commit() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let (id, item) = (Uuid::new_v4(), Uuid::new_v4());

    assert_eq!(put(&server, &sess, id, item, b"sealed").await.0, 200);
    assert_eq!(get(&server, &sess, id).await.0, 404);

    commit(&server, id).await;
    let (status, body) = get(&server, &sess, id).await;
    assert_eq!(status, 200);
    assert_eq!(body["itemId"], json!(item));
    assert_eq!(body["blob"], BASE64.encode(b"sealed"));
    server.cleanup().await;
}

#[tokio::test]
async fn a_repeated_put_is_a_no_op() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let (id, item) = (Uuid::new_v4(), Uuid::new_v4());

    assert_eq!(put(&server, &sess, id, item, b"sealed").await.0, 200);
    assert_eq!(put(&server, &sess, id, item, b"sealed").await.0, 200);
    let rows: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM attachments", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(rows, 1);

    assert_eq!(put(&server, &sess, id, item, b"other").await.0, 409);
    assert_eq!(
        put(&server, &sess, id, Uuid::new_v4(), b"sealed").await.0,
        409
    );
    server.cleanup().await;
}

#[tokio::test]
async fn oversize_blob_is_413() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let big = vec![7u8; MAX_ATTACHMENT_BLOB_BYTES + 1];

    assert_eq!(
        put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), &big)
            .await
            .0,
        413
    );
    let exact = vec![7u8; MAX_ATTACHMENT_BLOB_BYTES];
    assert_eq!(
        put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), &exact)
            .await
            .0,
        200
    );
    server.cleanup().await;
}

#[tokio::test]
async fn attachment_quota_boundaries() {
    let server = TestServer::start_with_vault_limit(MAX_VAULT_BYTES).await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let cap = MAX_ATTACHMENT_VAULT_BYTES;

    seed(&server, account.vault_id, (cap - 1000) as i32).await;
    // Exactly the cap is accepted, one byte more is not.
    assert_eq!(
        put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), &[1u8; 1000])
            .await
            .0,
        200
    );
    let (status, body) = put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), &[1u8]).await;
    assert_eq!(
        (status, &body["error"]["code"]),
        (413, &json!("attachments_full"))
    );
    server.cleanup().await;
}

#[tokio::test]
async fn the_vault_total_caps_attachments_too() {
    let server = TestServer::start_with_vault_limit(1000).await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    seed(&server, account.vault_id, 900).await;

    let (status, body) = put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), &[1u8; 101]).await;
    assert_eq!(
        (status, &body["error"]["code"]),
        (413, &json!("vault_full"))
    );
    assert_eq!(
        put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), &[1u8; 100])
            .await
            .0,
        200
    );
    server.cleanup().await;
}

#[tokio::test]
async fn frozen_account_cannot_upload_but_can_download() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let id = Uuid::new_v4();
    put(&server, &sess, id, Uuid::new_v4(), b"sealed").await;
    commit(&server, id).await;
    set_plan(&server, account.account_id, Status::Frozen).await;

    assert_eq!(
        put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), b"x")
            .await
            .0,
        402
    );
    assert_eq!(get(&server, &sess, id).await.0, 200);
    server.cleanup().await;
}

#[tokio::test]
async fn another_vaults_attachment_is_not_found() {
    let server = TestServer::start().await;
    let (_, a) = signed_in(&server, "a@example.com").await;
    let (_, b) = signed_in(&server, "b@example.com").await;
    let id = Uuid::new_v4();
    put(&server, &b, id, Uuid::new_v4(), b"sealed").await;
    commit(&server, id).await;

    assert_eq!(get(&server, &a, id).await.0, 404);
    // The same id in A's vault is a new attachment, not B's.
    assert_eq!(put(&server, &a, id, Uuid::new_v4(), b"mine").await.0, 200);
    assert_eq!(
        get(&server, &b, id).await.1["blob"],
        BASE64.encode(b"sealed")
    );
    server.cleanup().await;
}

#[tokio::test]
async fn account_erase_removes_attachments() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    put(&server, &sess, Uuid::new_v4(), Uuid::new_v4(), b"sealed").await;

    let db = server.db().await;
    db.execute("DELETE FROM accounts WHERE id = $1", &[&account.account_id])
        .await
        .unwrap();
    let rows: i64 = db
        .query_one("SELECT count(*) FROM attachments", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(rows, 0);
    server.cleanup().await;
}
