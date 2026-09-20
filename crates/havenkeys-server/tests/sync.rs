mod support;

use data_encoding::BASE64;
use serde_json::json;
use uuid::Uuid;

// ------------------------------------------------------------------- header

#[tokio::test]
async fn the_header_is_served_back_byte_for_byte() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    let body: serde_json::Value = server
        .get_as("/v1/vault/header", &sess)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        BASE64
            .decode(body["header"].as_str().unwrap().as_bytes())
            .unwrap(),
        b"header-bytes-v1".to_vec()
    );
    assert_eq!(body["headerRevision"], 0);
    assert_eq!(body["keyScheme"], 3);
    server.cleanup().await;
}

#[tokio::test]
async fn a_header_write_must_be_exactly_one_revision_ahead() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    let put = |revision: i64, bytes: &'static [u8]| {
        server.put_as("/v1/vault/header", &sess).json(&json!({
            "header": BASE64.encode(bytes),
            "headerRevision": revision,
            "keyScheme": 3,
        }))
    };

    assert_eq!(put(1, b"second").send().await.unwrap().status(), 204);
    assert_eq!(
        put(1, b"third").send().await.unwrap().status(),
        409,
        "the same revision twice is a conflict"
    );
    assert_eq!(
        put(3, b"fourth").send().await.unwrap().status(),
        409,
        "skipping a revision is a conflict"
    );

    let stored: Vec<u8> = server
        .db()
        .await
        .query_one("SELECT header FROM vaults WHERE id = $1", &[&sess.vault_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        stored,
        b"second".to_vec(),
        "a refused write changed nothing"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_lower_key_scheme_is_refused() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let res = server
        .put_as("/v1/vault/header", &sess)
        .json(&json!({
            "header": BASE64.encode(b"downgrade"),
            "headerRevision": 1,
            "keyScheme": 2,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let scheme: i16 = server
        .db()
        .await
        .query_one(
            "SELECT key_scheme FROM vaults WHERE id = $1",
            &[&sess.vault_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(scheme, 3);
    server.cleanup().await;
}

// --------------------------------------------------------------------- pull

#[tokio::test]
async fn a_pull_returns_live_items_and_deletions() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    let (kept, _) = support::create_item(&server, &sess, b"ov-1", b"det-1").await;
    let (gone, gone_rev) = support::create_item(&server, &sess, b"ov-2", b"det-2").await;
    let (status, _) = support::write(
        &server,
        &sess,
        vec![support::deletion(gone, Some(gone_rev))],
    )
    .await;
    assert_eq!(status, 200);

    let body = support::pull(&server, &sess, 0).await;
    let changes = body["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(body["hasMore"], false);

    let live = changes.iter().find(|c| c["itemId"] == json!(kept)).unwrap();
    assert_eq!(live["deleted"], false);
    assert_eq!(
        BASE64
            .decode(live["overview"].as_str().unwrap().as_bytes())
            .unwrap(),
        b"ov-1".to_vec()
    );

    let dead = changes.iter().find(|c| c["itemId"] == json!(gone)).unwrap();
    assert_eq!(dead["deleted"], true);
    assert!(dead["overview"].is_null());
    assert!(dead["details"].is_null());

    // Pulling from the returned cursor yields nothing new.
    let after = support::pull(&server, &sess, body["cursor"].as_i64().unwrap()).await;
    assert!(after["changes"].as_array().unwrap().is_empty());
    assert_eq!(after["cursor"], body["cursor"]);
    server.cleanup().await;
}

#[tokio::test]
async fn a_pull_pages_and_the_cursor_carries_on() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    for _ in 0..2 {
        let changes: Vec<serde_json::Value> = (0..300)
            .map(|_| support::change(Uuid::new_v4(), None, b"ov", b"det"))
            .collect();
        let (status, body) = support::write(&server, &sess, changes).await;
        assert_eq!(status, 200, "{body}");
    }

    // 600 rows in two batches of 300. The page limit is 500, and a page ends
    // at a batch boundary, so the first page is the first batch's 300.
    let first = support::pull(&server, &sess, 0).await;
    assert_eq!(first["changes"].as_array().unwrap().len(), 300);
    assert_eq!(first["hasMore"], true);

    let second = support::pull(&server, &sess, first["cursor"].as_i64().unwrap()).await;
    assert_eq!(second["changes"].as_array().unwrap().len(), 300);
    assert_eq!(second["hasMore"], false);
    server.cleanup().await;
}

#[tokio::test]
async fn a_negative_or_unparseable_cursor_is_refused() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    for query in ["?since=-1", "?since=abc", "?since=1&extra=2"] {
        let res = server
            .get_as(&format!("/v1/sync{query}"), &sess)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "accepted {query}");
    }
    server.cleanup().await;
}

// -------------------------------------------------------------------- write

#[tokio::test]
async fn a_batch_is_applied_under_one_revision() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    let changes: Vec<serde_json::Value> = (0..3)
        .map(|_| support::change(Uuid::new_v4(), None, b"ov", b"det"))
        .collect();
    let (status, body) = support::write(&server, &sess, changes).await;
    assert_eq!(status, 200);
    let applied = body["applied"].as_array().unwrap();
    assert_eq!(applied.len(), 3);
    let revision = body["cursor"].as_i64().unwrap();
    assert!(applied.iter().all(|a| a["revision"] == json!(revision)));
    assert_eq!(revision, 1, "one batch advances the cursor once");
    server.cleanup().await;
}

#[tokio::test]
async fn a_stale_base_revision_refuses_the_whole_batch() {
    let server = support::TestServer::start().await;
    let (account, sess) = support::signed_in(&server, "user@example.com").await;
    let other = support::login(&server, &account, "Laptop").await;

    let (item, first_rev) = support::create_item(&server, &sess, b"ov", b"det").await;
    // Another device moves the item on.
    let (status, _) = support::write(
        &server,
        &other,
        vec![support::change(item, Some(first_rev), b"ov-2", b"det-2")],
    )
    .await;
    assert_eq!(status, 200);

    let newcomer = Uuid::new_v4();
    let (status, body) = support::write(
        &server,
        &sess,
        vec![
            support::change(item, Some(first_rev), b"ov-stale", b"det-stale"),
            support::change(newcomer, None, b"ov-new", b"det-new"),
        ],
    )
    .await;
    assert_eq!(status, 409);
    let conflicts = body["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["itemId"], json!(item));

    let exists: i64 = server
        .db()
        .await
        .query_one(
            "SELECT count(*) FROM items WHERE item_id = $1",
            &[&newcomer],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(exists, 0, "a refused batch writes nothing at all");

    let stored: Vec<u8> = server
        .db()
        .await
        .query_one("SELECT overview FROM items WHERE item_id = $1", &[&item])
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, b"ov-2".to_vec());
    server.cleanup().await;
}

#[tokio::test]
async fn creating_an_item_that_already_exists_is_a_conflict() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let (item, _) = support::create_item(&server, &sess, b"ov", b"det").await;

    let (status, body) = support::write(
        &server,
        &sess,
        vec![support::change(item, None, b"overwrite", b"overwrite")],
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(body["conflicts"][0]["itemId"], json!(item));
    server.cleanup().await;
}

#[tokio::test]
async fn deleting_a_deleted_item_is_a_conflict_not_a_second_tombstone() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let (item, rev) = support::create_item(&server, &sess, b"ov", b"det").await;

    let (status, body) =
        support::write(&server, &sess, vec![support::deletion(item, Some(rev))]).await;
    assert_eq!(status, 200);
    let deleted_rev = body["applied"][0]["revision"].as_i64().unwrap();

    let (status, _) = support::write(
        &server,
        &sess,
        vec![support::deletion(item, Some(deleted_rev))],
    )
    .await;
    assert_eq!(status, 409);

    let row = server
        .db()
        .await
        .query_one(
            "SELECT overview, details, deleted_at IS NOT NULL, revision
               FROM items WHERE item_id = $1",
            &[&item],
        )
        .await
        .unwrap();
    assert!(row.get::<_, Option<Vec<u8>>>(0).is_none());
    assert!(row.get::<_, Option<Vec<u8>>>(1).is_none());
    assert!(row.get::<_, bool>(2));
    assert_eq!(row.get::<_, i64>(3), deleted_rev);
    server.cleanup().await;
}

#[tokio::test]
async fn an_item_can_be_recreated_after_a_deletion() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let (item, rev) = support::create_item(&server, &sess, b"ov", b"det").await;
    let (_, body) = support::write(&server, &sess, vec![support::deletion(item, Some(rev))]).await;
    let deleted_rev = body["applied"][0]["revision"].as_i64().unwrap();

    let (status, _) = support::write(
        &server,
        &sess,
        vec![support::change(item, Some(deleted_rev), b"again", b"again")],
    )
    .await;
    assert_eq!(status, 200);

    let alive: bool = server
        .db()
        .await
        .query_one(
            "SELECT deleted_at IS NULL FROM items WHERE item_id = $1",
            &[&item],
        )
        .await
        .unwrap()
        .get(0);
    assert!(alive);
    server.cleanup().await;
}

#[tokio::test]
async fn a_malformed_batch_is_refused() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let id = Uuid::new_v4();

    let cases = vec![
        ("empty", vec![]),
        (
            "no blobs and no deletion",
            vec![json!({ "itemId": id, "baseRevision": null })],
        ),
        (
            "deletion carrying a blob",
            vec![json!({
                "itemId": id,
                "baseRevision": null,
                "deleted": true,
                "overview": BASE64.encode(b"ov"),
            })],
        ),
        (
            "half an item",
            vec![json!({
                "itemId": id,
                "baseRevision": null,
                "overview": BASE64.encode(b"ov"),
            })],
        ),
        (
            "the same item twice",
            vec![
                support::change(id, None, b"a", b"a"),
                support::change(id, None, b"b", b"b"),
            ],
        ),
        (
            "an unknown field",
            vec![json!({
                "itemId": id,
                "baseRevision": null,
                "overview": BASE64.encode(b"ov"),
                "details": BASE64.encode(b"det"),
                "vaultId": Uuid::new_v4(),
            })],
        ),
    ];
    for (name, changes) in cases {
        let (status, _) = support::write(&server, &sess, changes).await;
        assert_eq!(status, 400, "accepted {name}");
    }

    let (status, _) = support::write(
        &server,
        &sess,
        (0..501)
            .map(|_| support::change(Uuid::new_v4(), None, b"ov", b"det"))
            .collect(),
    )
    .await;
    assert_eq!(status, 400, "batch over the limit");

    let written: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM items", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(written, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn a_page_never_cuts_a_batch_in_half() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    // Three batches of 400: the first page cannot hold the second batch's
    // tail, so it must stop at the batch boundary rather than mid-revision.
    let mut ids = Vec::new();
    for _ in 0..3 {
        let changes: Vec<serde_json::Value> = (0..400)
            .map(|_| {
                let id = Uuid::new_v4();
                ids.push(id);
                support::change(id, None, b"ov", b"det")
            })
            .collect();
        let (status, _) = support::write(&server, &sess, changes).await;
        assert_eq!(status, 200);
    }

    let mut seen = std::collections::HashSet::new();
    let mut cursor = 0;
    loop {
        let body = support::pull(&server, &sess, cursor).await;
        for change in body["changes"].as_array().unwrap() {
            seen.insert(change["itemId"].as_str().unwrap().to_string());
        }
        let next = body["cursor"].as_i64().unwrap();
        assert!(
            next > cursor || !body["hasMore"].as_bool().unwrap(),
            "no progress"
        );
        cursor = next;
        if !body["hasMore"].as_bool().unwrap() {
            break;
        }
    }
    assert_eq!(seen.len(), 1200, "every item is pulled exactly once");
    server.cleanup().await;
}
