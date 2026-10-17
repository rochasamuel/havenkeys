mod support;

use havenkeys_server::pake::{self, Purpose};

#[tokio::test]
async fn the_server_setup_is_created_once_and_reloaded() {
    let server = support::TestServer::start().await;
    let a = pake::load_or_create(server.pool()).await.unwrap();
    let b = pake::load_or_create(server.pool()).await.unwrap();
    assert_eq!(a.to_bytes(), b.to_bytes());
    server.cleanup().await;
}

#[tokio::test]
async fn a_pending_exchange_is_read_once() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let id = pake::put_pending(&db, Purpose::Login, None, b"state")
        .await
        .unwrap();
    let first = pake::take_pending(&db, Purpose::Login, &id).await.unwrap();
    assert_eq!(
        first.map(|(a, s)| (a, s.to_vec())),
        Some((None, b"state".to_vec()))
    );
    assert!(pake::take_pending(&db, Purpose::Login, &id)
        .await
        .unwrap()
        .is_none());
    server.cleanup().await;
}

#[tokio::test]
async fn a_pending_exchange_is_bound_to_its_purpose_and_expires() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let id = pake::put_pending(&db, Purpose::Login, None, b"s")
        .await
        .unwrap();
    assert!(pake::take_pending(&db, Purpose::Reauth, &id)
        .await
        .unwrap()
        .is_none());
    let id = pake::put_pending(&db, Purpose::Login, None, b"s")
        .await
        .unwrap();
    db.execute(
        "UPDATE pake_pending SET expires_at = now() - interval '1 second'",
        &[],
    )
    .await
    .unwrap();
    assert!(pake::take_pending(&db, Purpose::Login, &id)
        .await
        .unwrap()
        .is_none());
    assert!(pake::take_pending(&db, Purpose::Login, "not-an-id")
        .await
        .unwrap()
        .is_none());
    server.cleanup().await;
}

#[tokio::test]
async fn the_table_holds_only_the_hash_of_the_id() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let id = pake::put_pending(&db, Purpose::Login, None, b"s")
        .await
        .unwrap();
    let stored: Vec<u8> = db
        .query_one("SELECT id_hash FROM pake_pending", &[])
        .await
        .unwrap()
        .get(0);
    assert_ne!(stored, id.as_bytes());
    assert_eq!(stored.len(), 32);
    server.cleanup().await;
}

use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn an_enrolled_account_signs_in_with_opaque() {
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let (status, body) =
        support::pake_login(&server, &account.email, &account.auth_key, Uuid::new_v4()).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["vaultId"], json!(account.vault_id));
    assert!(body["token"].as_str().unwrap().len() >= 40);
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_key_fails_on_the_client_and_nothing_is_finished() {
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let (status, _) =
        support::pake_login(&server, &account.email, &[1u8; 32], Uuid::new_v4()).await;
    assert_eq!(status, 0, "the client stops before sending a finalization");
    let sessions: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(sessions, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn an_unknown_email_and_an_unenrolled_account_look_like_a_real_one() {
    let server = support::TestServer::start().await;
    let _enrolled = support::activate_pake(&server, "a@example.com", [9u8; 32]).await;
    support::activate(&server, "legacy@example.com", [9u8; 32]).await;
    let mut shapes = Vec::new();
    for email in ["a@example.com", "legacy@example.com", "nobody@example.com"] {
        let (_, request) = havenkeys_opaque::ClientLogin::start(&[9u8; 32]).unwrap();
        let res = server
            .post("/v1/auth/pake/start")
            .json(&json!({ "email": email, "credentialRequest": data_encoding::BASE64.encode(&request) }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body: serde_json::Value = res.json().await.unwrap();
        shapes.push((
            body["loginId"].as_str().unwrap().len(),
            body["credentialResponse"].as_str().unwrap().len(),
        ));
    }
    assert!(shapes.iter().all(|s| s == &shapes[0]), "{shapes:?}");
    let (status, _) =
        support::pake_login(&server, "nobody@example.com", &[9u8; 32], Uuid::new_v4()).await;
    assert_eq!(status, 0);
    server.cleanup().await;
}

/// A start for `email`, and the client's finalization of that exchange when
/// the client holds `key`: (start body, finalization).
async fn started(
    server: &support::TestServer,
    email: &str,
    key: &[u8; 32],
) -> (serde_json::Value, Vec<u8>) {
    let (client, request) = havenkeys_opaque::ClientLogin::start(key).unwrap();
    let body: serde_json::Value = server
        .post("/v1/auth/pake/start")
        .json(
            &json!({ "email": email, "credentialRequest": data_encoding::BASE64.encode(&request) }),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = data_encoding::BASE64
        .decode(body["credentialResponse"].as_str().unwrap().as_bytes())
        .unwrap();
    let finalization = client.finish(key, &response).unwrap();
    (body, finalization)
}

#[tokio::test]
async fn a_login_id_works_once() {
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let (body, finalization) = started(&server, &account.email, &account.auth_key).await;
    let finish = json!({
        "loginId": body["loginId"],
        "credentialFinalization": data_encoding::BASE64.encode(&finalization),
        "deviceId": Uuid::new_v4(),
        "deviceName": "Desktop",
    });
    let first = server
        .post("/v1/auth/pake/finish")
        .json(&finish)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    let replay = server
        .post("/v1/auth/pake/finish")
        .json(&finish)
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), 401);
    server.cleanup().await;
}

#[tokio::test]
async fn a_login_id_expires() {
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let (body, finalization) = started(&server, &account.email, &account.auth_key).await;
    server
        .db()
        .await
        .execute(
            "UPDATE pake_pending SET expires_at = now() - interval '1 second'",
            &[],
        )
        .await
        .unwrap();
    let res = server
        .post("/v1/auth/pake/finish")
        .json(&json!({
            "loginId": body["loginId"],
            "credentialFinalization": data_encoding::BASE64.encode(&finalization),
            "deviceId": Uuid::new_v4(), "deviceName": "Desktop",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    server.cleanup().await;
}

#[tokio::test]
async fn malformed_and_oversized_messages_are_400_without_detail() {
    let server = support::TestServer::start().await;
    for request in [
        "",
        "!!!",
        &data_encoding::BASE64.encode(&[0u8; 600]),
        &data_encoding::BASE64.encode(&[0u8; 96]),
    ] {
        let res = server
            .post("/v1/auth/pake/start")
            .json(&json!({ "email": "a@example.com", "credentialRequest": request }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "{request}");
        let text = res.text().await.unwrap();
        assert!(!text.contains(request) || request.is_empty());
    }
    server.cleanup().await;
}

#[tokio::test]
async fn opaque_start_is_rate_limited_like_login() {
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let mut last = 0;
    for _ in 0..20 {
        last = support::pake_login(&server, &account.email, &[1u8; 32], Uuid::new_v4())
            .await
            .0;
        if last == 429 {
            break;
        }
    }
    assert_eq!(last, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn a_frozen_account_refuses_a_new_device_over_opaque() {
    use havenkeys_server::billing::Status;
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let known = Uuid::new_v4();
    let (status, body) =
        support::pake_login(&server, &account.email, &account.auth_key, known).await;
    assert_eq!(status, 200, "{body}");
    support::set_plan(&server, account.account_id, Status::Frozen).await;
    let new_device = Uuid::new_v4();
    let (status, body) =
        support::pake_login(&server, &account.email, &account.auth_key, new_device).await;
    assert_eq!(status, 402, "{body}");
    assert_eq!(body["error"]["code"], "account_frozen");
    let registered: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM devices WHERE id = $1", &[&new_device])
        .await
        .unwrap()
        .get(0);
    assert_eq!(registered, 0);
    let (status, _) = support::pake_login(&server, &account.email, &account.auth_key, known).await;
    assert_eq!(status, 200, "a known device still signs in");
    server.cleanup().await;
}

#[tokio::test]
async fn a_device_of_a_deleted_account_signing_in_over_opaque_gets_410() {
    use havenkeys_server::admin::{self, AdminCommand};
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "gone@example.com", [9u8; 32]).await;
    let other = support::activate_pake(&server, "other@example.com", [8u8; 32]).await;
    let device = Uuid::new_v4();
    let (status, _) = support::pake_login(&server, &account.email, &account.auth_key, device).await;
    assert_eq!(status, 200);
    admin::run(
        AdminCommand::DeleteAccount {
            email: account.email.clone(),
        },
        server.pool(),
        [7u8; 32],
    )
    .await
    .unwrap();
    // No record exists any more, so the client cannot finish.
    let (status, _) = support::pake_login(&server, &account.email, &account.auth_key, device).await;
    assert_eq!(status, 0);
    // A well-formed finalization from another account's exchange fails
    // verification against the deleted account's (decoy) exchange.
    let (_, finalization) = started_for(&server, &other.email, &other.auth_key).await;
    let finish = |login_id: serde_json::Value, device: Uuid| {
        server.post("/v1/auth/pake/finish").json(&json!({
            "loginId": login_id,
            "credentialFinalization": data_encoding::BASE64.encode(&finalization),
            "deviceId": device, "deviceName": "Laptop",
        }))
    };
    let (request_start, _) = decoy_start(&server, &account.email).await;
    let res = finish(request_start, device).send().await.unwrap();
    assert_eq!(res.status(), 410);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], "account_deleted");
    let (request_start, _) = decoy_start(&server, &account.email).await;
    let res = finish(request_start, Uuid::new_v4()).send().await.unwrap();
    assert_eq!(
        res.status(),
        401,
        "a device that never belonged to it learns nothing"
    );
    server.cleanup().await;
}

async fn started_for(
    server: &support::TestServer,
    email: &str,
    key: &[u8; 32],
) -> (serde_json::Value, Vec<u8>) {
    started(server, email, key).await
}

/// A start whose exchange the client never completes: (loginId, ()).
async fn decoy_start(server: &support::TestServer, email: &str) -> (serde_json::Value, ()) {
    let (_, request) = havenkeys_opaque::ClientLogin::start(&[9u8; 32]).unwrap();
    let body: serde_json::Value = server
        .post("/v1/auth/pake/start")
        .json(
            &json!({ "email": email, "credentialRequest": data_encoding::BASE64.encode(&request) }),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (body["loginId"].clone(), ())
}

#[tokio::test]
async fn an_account_activated_with_opaque_has_no_legacy_verifier() {
    let server = support::TestServer::start().await;
    let account = support::activate_pake(&server, "user@example.com", [9u8; 32]).await;
    let row = server
        .db()
        .await
        .query_one(
            "SELECT auth_verifier, opaque_record FROM accounts WHERE id = $1",
            &[&account.account_id],
        )
        .await
        .unwrap();
    assert!(row.get::<_, Option<String>>(0).is_none());
    assert!(row.get::<_, Option<Vec<u8>>>(1).is_some());
    let (status, _) =
        support::pake_login(&server, &account.email, &account.auth_key, Uuid::new_v4()).await;
    assert_eq!(status, 200);
    let (status, _) =
        support::login_with_device(&server, &account, "Desktop", Uuid::new_v4()).await;
    assert_eq!(status, 401, "no legacy path into an OPAQUE-only account");
    server.cleanup().await;
}

#[tokio::test]
async fn activation_register_needs_a_plausible_invite() {
    let server = support::TestServer::start().await;
    let (_, request) = havenkeys_opaque::ClientRegistration::start(&[9u8; 32]).unwrap();
    let res = server
        .post("/v1/accounts/activate/register")
        .json(
            &json!({ "email": "user@example.com", "invite": "hk-invite-not-real",
                       "registrationRequest": data_encoding::BASE64.encode(&request) }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn activation_with_both_or_neither_key_is_refused() {
    let server = support::TestServer::start().await;
    let invite = support::new_invite(&server, "user@example.com").await;
    let mut both = support::activate_body("user@example.com", &invite, Uuid::new_v4(), &[9u8; 32]);
    both["registrationUpload"] = json!(data_encoding::BASE64.encode(&[0u8; 192]));
    let res = server
        .post("/v1/accounts/activate")
        .json(&both)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let mut neither =
        support::activate_body("user@example.com", &invite, Uuid::new_v4(), &[9u8; 32]);
    neither.as_object_mut().unwrap().remove("authKey");
    let res = server
        .post("/v1/accounts/activate")
        .json(&neither)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn legacy_paths_answer_426_after_the_cutoff_and_never_410() {
    let (server, _) = support::TestServer::start_with(support::Options {
        legacy_auth_until: Some(chrono::Utc::now() - chrono::Duration::seconds(1)),
        ..Default::default()
    })
    .await;
    let invite = support::new_invite(&server, "old@example.com").await;
    let res = server
        .post("/v1/accounts/activate")
        .json(&support::activate_body(
            "old@example.com",
            &invite,
            Uuid::new_v4(),
            &[9u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 426);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], "upgrade_required");

    let account = support::activate_pake(&server, "new@example.com", [9u8; 32]).await;
    let (status, _) =
        support::login_with_device(&server, &account, "Desktop", Uuid::new_v4()).await;
    assert_eq!(status, 426);
    let (status, _) =
        support::pake_login(&server, &account.email, &account.auth_key, Uuid::new_v4()).await;
    assert_eq!(status, 200, "OPAQUE is unaffected by the cutoff");
    server.cleanup().await;
}
