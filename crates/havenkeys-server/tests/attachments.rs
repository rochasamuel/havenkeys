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

/// The (id, committed) rows of an item's attachments, ordered by id.
async fn rows_of(server: &TestServer, item: Uuid) -> Vec<(Uuid, bool)> {
    server
        .db()
        .await
        .query(
            "SELECT attachment_id, committed FROM attachments WHERE item_id = $1 ORDER BY 1",
            &[&item],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect()
}

fn ids(n: usize) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = (0..n).map(|_| Uuid::new_v4()).collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn commit_lists_keep_and_delete_the_rest() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let item = Uuid::new_v4();
    let [a, b, c] = ids(3)[..] else {
        unreachable!()
    };
    for id in [a, b, c] {
        assert_eq!(put(&server, &sess, id, item, b"sealed").await.0, 200);
    }

    let (status, body) = write(
        &server,
        &sess,
        vec![change_with_attachments(item, None, b"o", b"d", &[a, b])],
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(rows_of(&server, item).await, [(a, true), (b, true)]);

    let rev = body["applied"][0]["revision"].as_i64();
    let (status, _) = write(
        &server,
        &sess,
        vec![change_with_attachments(item, rev, b"o", b"d", &[a])],
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(rows_of(&server, item).await, [(a, true)]);
    server.cleanup().await;
}

#[tokio::test]
async fn unknown_or_foreign_id_refuses_the_batch() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let (item, other) = (Uuid::new_v4(), Uuid::new_v4());
    let [mine, theirs, unknown] = ids(3)[..] else {
        unreachable!()
    };
    put(&server, &sess, mine, item, b"sealed").await;
    put(&server, &sess, theirs, other, b"sealed").await;
    let (id, rev) = create_item(&server, &sess, b"o", b"d").await;

    for bad in [theirs, unknown] {
        let (status, _) = write(
            &server,
            &sess,
            vec![
                change_with_attachments(item, None, b"o", b"d", &[mine]),
                change_with_attachments(id, Some(rev), b"new", b"new", &[bad]),
            ],
        )
        .await;
        assert_eq!(status, 400);
    }
    // Nothing was written: the other item is unchanged and nothing committed.
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["changes"].as_array().unwrap().len(), 1);
    assert_eq!(rows_of(&server, item).await, [(mine, false)]);
    assert_eq!(rows_of(&server, other).await, [(theirs, false)]);
    server.cleanup().await;
}

#[tokio::test]
async fn absent_ids_leave_rows_untouched() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let item = Uuid::new_v4();
    let a = Uuid::new_v4();
    put(&server, &sess, a, item, b"sealed").await;
    let (_, body) = write(
        &server,
        &sess,
        vec![change_with_attachments(item, None, b"o", b"d", &[a])],
    )
    .await;

    let rev = body["applied"][0]["revision"].as_i64();
    let (status, _) = write(&server, &sess, vec![change(item, rev, b"o2", b"d2")]).await;
    assert_eq!(status, 200);
    assert_eq!(rows_of(&server, item).await, [(a, true)]);
    server.cleanup().await;
}

#[tokio::test]
async fn tombstone_deletes_all() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let item = Uuid::new_v4();
    let [a, b] = ids(2)[..] else { unreachable!() };
    put(&server, &sess, a, item, b"sealed").await;
    put(&server, &sess, b, item, b"sealed").await;
    let (_, body) = write(
        &server,
        &sess,
        vec![change_with_attachments(item, None, b"o", b"d", &[a])],
    )
    .await;

    let rev = body["applied"][0]["revision"].as_i64();
    let (status, _) = write(&server, &sess, vec![deletion(item, rev)]).await;
    assert_eq!(status, 200);
    assert!(rows_of(&server, item).await.is_empty());
    server.cleanup().await;
}

#[tokio::test]
async fn attachment_ids_on_a_deletion_or_over_20_or_duplicated_is_400() {
    let server = TestServer::start().await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let item = Uuid::new_v4();
    let a = Uuid::new_v4();

    let mut deleting = deletion(item, None);
    deleting["attachmentIds"] = json!([]);
    let too_many = change_with_attachments(item, None, b"o", b"d", &ids(21));
    let twice = change_with_attachments(item, None, b"o", b"d", &[a, a]);
    for bad in [deleting, too_many, twice] {
        assert_eq!(write(&server, &sess, vec![bad]).await.0, 400);
    }
    // Twenty distinct ids is the most an item may name.
    let twenty = ids(20);
    for id in &twenty {
        put(&server, &sess, *id, item, b"s").await;
    }
    let (status, _) = write(
        &server,
        &sess,
        vec![change_with_attachments(item, None, b"o", b"d", &twenty)],
    )
    .await;
    assert_eq!(status, 200);
    server.cleanup().await;
}

#[tokio::test]
async fn vault_bytes_counts_attachments() {
    let server = TestServer::start_with_vault_limit(1000).await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    seed(&server, account.vault_id, 990).await;

    let (status, body) = write(
        &server,
        &sess,
        vec![change(Uuid::new_v4(), None, b"0123456", b"0123456")],
    )
    .await;
    assert_eq!(
        (status, &body["error"]["code"]),
        (413, &json!("vault_full"))
    );
    assert_eq!(
        write(
            &server,
            &sess,
            vec![change(Uuid::new_v4(), None, b"01234", b"01234")]
        )
        .await
        .0,
        200
    );
    server.cleanup().await;
}
