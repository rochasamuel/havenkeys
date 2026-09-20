mod support;

use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn an_oversized_body_is_refused_before_it_is_parsed() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    // 17 MiB against a 16 MiB ceiling.
    let body = format!(
        r#"{{"changes":[{{"itemId":"{}","padding":"{}"}}]}}"#,
        Uuid::new_v4(),
        "A".repeat(17 * 1024 * 1024)
    );
    let res = server
        .post_as("/v1/items", &sess)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 413);
    server.cleanup().await;
}

#[tokio::test]
async fn an_oversized_blob_is_refused() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    // 9 MiB of ciphertext against an 8 MiB per-blob ceiling.
    let big = vec![7u8; 9 * 1024 * 1024];
    let (status, _) = support::write(
        &server,
        &sess,
        vec![support::change(Uuid::new_v4(), None, &big, b"det")],
    )
    .await;
    assert_eq!(status, 400);
    server.cleanup().await;
}

#[tokio::test]
async fn an_oversized_header_is_refused() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let res = server
        .put_as("/v1/vault/header", &sess)
        .json(&json!({
            "header": data_encoding::BASE64.encode(&vec![1u8; 65 * 1024]),
            "headerRevision": 1,
            "keyScheme": 3,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn malformed_json_and_wrong_types_are_refused_without_quoting_the_request() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;

    let cases = vec![
        "not json at all",
        "{",
        r#"{"changes": "hunter2"}"#,
        r#"{"changes": [{"itemId": "not-a-uuid", "baseRevision": null}]}"#,
        r#"{"changes": [], "surprise": 1}"#,
    ];
    for body in cases {
        let res = server
            .post_as("/v1/items", &sess)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "accepted {body}");
        let text = res.text().await.unwrap();
        assert!(
            !text.contains("hunter2") && !text.contains("not-a-uuid"),
            "the error quoted the request back: {text}"
        );
    }
    server.cleanup().await;
}

#[tokio::test]
async fn a_body_that_names_its_own_account_or_vault_is_refused() {
    let server = support::TestServer::start().await;
    let (_, a) = support::signed_in(&server, "a@example.com").await;
    let (_, b) = support::signed_in(&server, "b@example.com").await;

    // Identity comes from the token. A body trying to name another vault is
    // an unknown field, so it is rejected rather than quietly ignored.
    let res = server
        .post_as("/v1/items", &a)
        .json(&json!({
            "vaultId": b.vault_id,
            "changes": [support::change(Uuid::new_v4(), None, b"ov", b"det")],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let rows: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM items", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(rows, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn a_non_uuid_path_parameter_is_refused() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let res = server
        .delete_as("/v1/devices/not-a-uuid", &sess)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}
