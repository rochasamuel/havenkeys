mod support;

use serde_json::json;
use uuid::Uuid;

/// The central guarantee: an authenticated account reaches nothing that
/// belongs to another one, on every route that takes a session.
#[tokio::test]
async fn account_a_cannot_touch_account_b() {
    let server = support::TestServer::start().await;
    let (account_a, a) = support::signed_in(&server, "a@example.com").await;
    let (_, b) = support::signed_in(&server, "b@example.com").await;

    let (b_item, b_rev) = support::create_item(&server, &b, b"b-overview", b"b-details").await;

    // Pull: A sees its own vault only.
    let a_changes = support::pull(&server, &a, 0).await;
    assert!(a_changes["changes"].as_array().unwrap().is_empty());

    // Fetch: A asking for B's item id gets nothing back, not an error — the
    // id is simply absent from A's vault, the same as an id that never
    // existed at all.
    let fetch_res = server
        .post_as("/v1/items/fetch", &a)
        .json(&json!({ "itemIds": [b_item] }))
        .send()
        .await
        .unwrap();
    assert_eq!(fetch_res.status(), 200);
    let fetch_body: serde_json::Value = fetch_res.json().await.unwrap();
    assert!(fetch_body["changes"].as_array().unwrap().is_empty());

    // Write: A writing B's item id lands in A's own vault, because item ids
    // are unique per vault, and leaves B's row untouched.
    let (status, _) = support::write(
        &server,
        &a,
        vec![support::change(b_item, None, b"a-overview", b"a-details")],
    )
    .await;
    assert_eq!(status, 200);
    let b_after = support::pull(&server, &b, 0).await;
    let b_row = &b_after["changes"].as_array().unwrap()[0];
    assert_eq!(
        b_row["overview"].as_str().unwrap(),
        data_encoding::BASE64.encode(b"b-overview")
    );
    assert_eq!(b_row["revision"], json!(b_rev));

    // A deletion aimed at B's item, with B's revision, hits A's copy only.
    let (status, _) = support::write(&server, &a, vec![support::deletion(b_item, Some(1))]).await;
    assert_eq!(status, 200);
    let b_after = support::pull(&server, &b, 0).await;
    assert_eq!(b_after["changes"].as_array().unwrap()[0]["deleted"], false);

    // Header: A's credential change does not move B's header.
    let new_key = [77u8; 32];
    assert_eq!(
        server
            .post_as("/v1/account/credentials", &a)
            .json(&support::credentials_body(
                &account_a.auth_key,
                &new_key,
                0,
                b"a-header-2",
            ))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let b_header: serde_json::Value = server
        .get_as("/v1/vault/header", &b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(b_header["headerRevision"], 0);
    assert_eq!(
        b_header["header"].as_str().unwrap(),
        data_encoding::BASE64.encode(b"header-bytes-v1")
    );

    // Devices: A's list holds only A's device, and A cannot revoke B's.
    let devices: serde_json::Value = server
        .get_as("/v1/devices", &a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let devices = devices.as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["id"], json!(a.device_id));
    assert_eq!(
        server
            .delete_as(&format!("/v1/devices/{}", b.device_id), &a)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );

    // B's session still works after all of it.
    assert_eq!(
        server
            .get_as("/v1/devices", &b)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    server.cleanup().await;
}

/// Every authenticated route refuses a missing, malformed or foreign token.
#[tokio::test]
async fn every_authenticated_route_needs_a_live_token() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    let device = sess.device_id;

    let routes: Vec<(&str, String)> = vec![
        ("GET", "/v1/vault/header".into()),
        ("POST", "/v1/account/credentials".into()),
        ("GET", "/v1/sync?since=0".into()),
        ("POST", "/v1/items".into()),
        ("POST", "/v1/items/fetch".into()),
        ("GET", "/v1/devices".into()),
        ("DELETE", format!("/v1/devices/{device}")),
        ("POST", "/v1/auth/logout".into()),
    ];

    for (method, path) in routes {
        for token in [None, Some("garbage"), Some("")] {
            let request = match method {
                "GET" => server.get(&path),
                "DELETE" => server.delete(&path),
                _ => server.post(&path).json(&json!({
                    "changes": [support::change(Uuid::new_v4(), None, b"ov", b"det")]
                })),
            };
            let request = match token {
                Some(t) => request.bearer_auth(t),
                None => request,
            };
            let status = request.send().await.unwrap().status();
            assert_eq!(status, 401, "{method} {path} with token {token:?}");
        }
    }
    server.cleanup().await;
}
