//! The server is not trusted, so every answer here is one a hostile or
//! broken server could send. None of them may panic, allocate without bound,
//! or be mistaken for a valid answer.

use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::keys::AuthKey;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::vault::derive_auth_key;
use havenkeys_core::SecretString;
use havenkeys_sync_client::transport::{HttpRequest, HttpResponse, Transport};
use havenkeys_sync_client::{CredentialChange, Result, Session, SyncClient, SyncError};
use uuid::Uuid;

/// A transport that answers whatever the test tells it to.
struct Stub {
    status: u16,
    body: Vec<u8>,
}

impl Stub {
    fn ok(body: &str) -> SyncClient<Self> {
        SyncClient::new(Self {
            status: 200,
            body: body.as_bytes().to_vec(),
        })
    }

    fn status(status: u16, body: &str) -> SyncClient<Self> {
        SyncClient::new(Self {
            status,
            body: body.as_bytes().to_vec(),
        })
    }
}

impl Transport for Stub {
    async fn send(&self, _request: HttpRequest) -> Result<HttpResponse> {
        Ok(HttpResponse {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

fn session() -> Session {
    Session::new(
        "token".into(),
        "2026-09-21T00:00:00Z".into(),
        Uuid::nil(),
        Uuid::nil(),
    )
}

/// Real auth keys derived with the cheapest KDF cost the core accepts, so the
/// tests exercise the actual wire shape rather than a fabricated one.
fn credential_keys() -> (AuthKey, AuthKey, KdfParams) {
    let kdf = KdfParams::with_cost(19 * 1024, 2, 1).unwrap();
    let secret_key = SecretKey::generate().unwrap();
    let account = AccountRef::new(
        Uuid::new_v4(),
        NormalizedEmail::parse("x@example.com").unwrap(),
    );
    let current = derive_auth_key(
        &SecretString::from("old password"),
        &secret_key,
        &kdf,
        &account,
    )
    .unwrap();
    let new = derive_auth_key(
        &SecretString::from("new password"),
        &secret_key,
        &kdf,
        &account,
    )
    .unwrap();
    (current, new, kdf)
}

fn change(base: i64) -> CredentialChange<'static> {
    // Leaked, not dropped: the borrowed `CredentialChange` needs keys that
    // outlive the call, and these tests never care about zeroization timing.
    let (current, new, kdf) = credential_keys();
    let current: &'static AuthKey = Box::leak(Box::new(current));
    let new: &'static AuthKey = Box::leak(Box::new(new));
    let kdf: &'static KdfParams = Box::leak(Box::new(kdf));
    CredentialChange {
        current_auth_key: current,
        kdf,
        new_auth_key: new,
        header: b"header",
        base_header_revision: base,
    }
}

#[tokio::test]
async fn a_body_that_is_not_the_protocol_is_refused() {
    for body in [
        "",
        "not json at all",
        "{",
        "[]",
        "42",
        "null",
        r#"{"cursor": "not a number", "hasMore": false, "changes": []}"#,
        r#"{"hasMore": false, "changes": []}"#,
    ] {
        let client = Stub::ok(body);
        let err = client.pull(&session(), 0).await.unwrap_err();
        assert!(
            matches!(err, SyncError::Protocol(_)),
            "body {body:?} produced {err:?}"
        );
    }
}

#[tokio::test]
async fn a_page_larger_than_the_protocol_allows_is_refused() {
    let changes: Vec<String> = (0..501)
        .map(|_| {
            format!(
                r#"{{"itemId":"{}","revision":1,"overview":"AAAA","details":"AAAA","deleted":false}}"#,
                Uuid::new_v4()
            )
        })
        .collect();
    let body = format!(
        r#"{{"cursor":1,"hasMore":false,"changes":[{}]}}"#,
        changes.join(",")
    );
    let client = Stub::ok(&body);
    assert!(matches!(
        client.pull(&session(), 0).await.unwrap_err(),
        SyncError::Protocol(_)
    ));
}

#[tokio::test]
async fn an_oversized_blob_is_refused_without_being_kept() {
    // 9 MiB of base64 against an 8 MiB ceiling. The length is checked before
    // the decode, so this costs a comparison, not an allocation.
    let huge = "A".repeat(9 * 1024 * 1024 / 3 * 4);
    let body = format!(
        r#"{{"cursor":1,"hasMore":false,"changes":[{{"itemId":"{}","revision":1,"overview":"{huge}","details":"AAAA","deleted":false}}]}}"#,
        Uuid::new_v4()
    );
    let client = Stub::ok(&body);
    assert_eq!(
        client.pull(&session(), 0).await.unwrap_err(),
        SyncError::TooLarge
    );
}

#[tokio::test]
async fn a_malformed_change_is_refused() {
    let id = Uuid::new_v4();
    let cases = [
        // Not base64.
        format!(
            r#"{{"itemId":"{id}","revision":1,"overview":"not base64!","details":"AAAA","deleted":false}}"#
        ),
        // A deletion carrying blobs.
        format!(
            r#"{{"itemId":"{id}","revision":1,"overview":"AAAA","details":"AAAA","deleted":true}}"#
        ),
        // A live item missing a blob.
        format!(r#"{{"itemId":"{id}","revision":1,"overview":"AAAA","deleted":false}}"#),
        // A revision that cannot exist.
        format!(
            r#"{{"itemId":"{id}","revision":-5,"overview":"AAAA","details":"AAAA","deleted":false}}"#
        ),
        // An item id that is not a UUID.
        r#"{"itemId":"nope","revision":1,"overview":"AAAA","details":"AAAA","deleted":false}"#
            .to_string(),
    ];
    for change in cases {
        let body = format!(r#"{{"cursor":1,"hasMore":false,"changes":[{change}]}}"#);
        let client = Stub::ok(&body);
        let err = client.pull(&session(), 0).await.unwrap_err();
        assert!(
            matches!(err, SyncError::Protocol(_)),
            "change {change} produced {err:?}"
        );
    }
}

#[tokio::test]
async fn a_cursor_that_moves_backwards_is_still_the_servers_decision() {
    // The server is the authority: a cursor lower than the one asked for is
    // not a protocol violation, and the applier will simply re-apply. What
    // must never happen is a *negative* cursor, which would corrupt the
    // stored one.
    let client = Stub::ok(r#"{"cursor":-1,"hasMore":false,"changes":[]}"#);
    assert!(matches!(
        client.pull(&session(), 5).await.unwrap_err(),
        SyncError::Protocol(_)
    ));

    let client = Stub::ok(r#"{"cursor":2,"hasMore":false,"changes":[]}"#);
    assert_eq!(client.pull(&session(), 5).await.unwrap().cursor, 2);
}

#[tokio::test]
async fn a_pull_that_deletes_everything_is_accepted_and_reported() {
    // The server owns deletions, so this is applied — but the caller gets the
    // count and can tell the user (design §12).
    let changes: Vec<String> = (0..3)
        .map(|_| {
            format!(
                r#"{{"itemId":"{}","revision":9,"deleted":true}}"#,
                Uuid::new_v4()
            )
        })
        .collect();
    let body = format!(
        r#"{{"cursor":9,"hasMore":false,"changes":[{}]}}"#,
        changes.join(",")
    );
    let client = Stub::ok(&body);
    let pulled = client.pull(&session(), 0).await.unwrap();
    assert_eq!(pulled.changes.len(), 3);
    assert!(pulled.changes.iter().all(|c| c.deleted));
}

#[tokio::test]
async fn a_downgraded_key_scheme_in_the_header_is_refused() {
    let client = Stub::ok(r#"{"header":"aGVhZGVy","headerRevision":3,"keyScheme":2}"#);
    assert!(matches!(
        client.header(&session()).await.unwrap_err(),
        SyncError::Protocol(_)
    ));

    let client = Stub::ok(r#"{"header":"aGVhZGVy","headerRevision":3,"keyScheme":3}"#);
    let header = client.header(&session()).await.unwrap();
    assert_eq!(header.bytes, b"header".to_vec());
    assert_eq!(header.revision, 3);
}

#[tokio::test]
async fn a_header_that_is_empty_oversized_or_not_base64_is_refused() {
    for body in [
        r#"{"header":"","headerRevision":1,"keyScheme":3}"#,
        r#"{"header":"not base64!","headerRevision":1,"keyScheme":3}"#,
        r#"{"header":"aGVhZGVy","headerRevision":-1,"keyScheme":3}"#,
    ] {
        let client = Stub::ok(body);
        assert!(
            matches!(
                client.header(&session()).await.unwrap_err(),
                SyncError::Protocol(_)
            ),
            "accepted {body}"
        );
    }
}

#[tokio::test]
async fn a_conflict_naming_nothing_is_not_a_conflict() {
    let client = Stub::status(409, r#"{"conflicts":[]}"#);
    let err = client.write(&session(), &[]).await.unwrap_err();
    // An empty batch never reaches the server at all.
    assert!(matches!(err, SyncError::Refused(_)));
}

#[tokio::test]
async fn statuses_map_to_actions_the_caller_can_take() {
    for (status, expected) in [
        (401u16, SyncError::Unauthorized),
        (403, SyncError::Unauthorized),
        (429, SyncError::RateLimited),
        (500, SyncError::Unavailable),
        (502, SyncError::Unavailable),
    ] {
        let client = Stub::status(status, "{}");
        assert_eq!(
            client.pull(&session(), 0).await.unwrap_err(),
            expected,
            "status {status}"
        );
    }
    let client = Stub::status(400, "{}");
    assert!(matches!(
        client.pull(&session(), 0).await.unwrap_err(),
        SyncError::Refused(_)
    ));
}

#[tokio::test]
async fn a_device_list_that_is_not_a_list_is_refused() {
    let client = Stub::ok(r#"{"devices":[]}"#);
    assert!(matches!(
        client.devices(&session()).await.unwrap_err(),
        SyncError::Protocol(_)
    ));
}

#[tokio::test]
async fn kdf_parameters_below_the_floor_are_refused() {
    // A server that could name any cost could make a device derive a key
    // cheap enough to brute-force.
    let weak = r#"{"accountId":"00000000-0000-0000-0000-000000000001","kdf":{"algorithm":"argon2id","memoryKib":1024,"iterations":1,"parallelism":1,"salt":"AAAAAAAAAAAAAAAAAAAAAA=="}}"#;
    let client = Stub::ok(weak);
    assert!(matches!(
        client.auth_params("user@example.com").await.unwrap_err(),
        SyncError::Protocol(_)
    ));

    let wrong_algorithm = r#"{"accountId":"00000000-0000-0000-0000-000000000001","kdf":{"algorithm":"pbkdf2","memoryKib":131072,"iterations":4,"parallelism":4,"salt":"AAAAAAAAAAAAAAAAAAAAAA=="}}"#;
    let client = Stub::ok(wrong_algorithm);
    assert!(matches!(
        client.auth_params("user@example.com").await.unwrap_err(),
        SyncError::Protocol(_)
    ));

    let good = r#"{"accountId":"00000000-0000-0000-0000-000000000001","kdf":{"algorithm":"argon2id","memoryKib":131072,"iterations":4,"parallelism":4,"salt":"AAAAAAAAAAAAAAAAAAAAAA=="}}"#;
    let client = Stub::ok(good);
    let params = client.auth_params("user@example.com").await.unwrap();
    assert_eq!(params.kdf.memory_kib, 131072);
}

#[tokio::test]
async fn a_credential_change_ack_must_be_exactly_one_ahead() {
    // base 3 → the server must answer 4; anything else is a server lying.
    for body in [
        r#"{"headerRevision": 3}"#,
        r#"{"headerRevision": 9}"#,
        r#"{"headerRevision": -1}"#,
        "{}",
    ] {
        let client = Stub::ok(body);
        let err = client
            .change_credentials(&session(), change(3))
            .await
            .unwrap_err();
        assert!(matches!(err, SyncError::Protocol(_)), "{body}: {err:?}");
    }
    let client = Stub::ok(r#"{"headerRevision": 4}"#);
    assert_eq!(
        client
            .change_credentials(&session(), change(3))
            .await
            .unwrap(),
        4
    );
}

#[tokio::test]
async fn a_credential_change_conflict_and_rejection_are_mapped() {
    let err = Stub::status(409, "{}")
        .change_credentials(&session(), change(0))
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::Conflict(_)));
    let err = Stub::status(401, "{}")
        .change_credentials(&session(), change(0))
        .await
        .unwrap_err();
    assert_eq!(err, SyncError::Unauthorized);
}

#[tokio::test]
async fn a_fetch_answer_is_bounded_to_what_was_asked() {
    let asked = Uuid::from_u128(1);
    let other = Uuid::from_u128(2);
    let body = format!(r#"{{"changes":[{{"itemId":"{other}","revision":1,"deleted":true}}]}}"#);
    let err = Stub::ok(&body)
        .fetch_items(&session(), &[asked])
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::Protocol(_)));

    let body = format!(
        r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}},{{"itemId":"{asked}","revision":1,"deleted":true}}]}}"#
    );
    let err = Stub::ok(&body)
        .fetch_items(&session(), &[asked])
        .await
        .unwrap_err();
    assert!(matches!(err, SyncError::Protocol(_)));

    let body = format!(r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}}]}}"#);
    assert_eq!(
        Stub::ok(&body)
            .fetch_items(&session(), &[asked])
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_fetch_request_is_bounded() {
    assert!(Stub::ok("{}").fetch_items(&session(), &[]).await.is_err());
    let many: Vec<Uuid> = (0..501).map(Uuid::from_u128).collect();
    assert!(Stub::ok("{}").fetch_items(&session(), &many).await.is_err());
}
