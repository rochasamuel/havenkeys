mod support;

use data_encoding::BASE64;
use uuid::Uuid;

#[tokio::test]
async fn activation_creates_the_vault_and_burns_the_invite() {
    let server = support::TestServer::start().await;
    let invite = support::new_invite(&server, "user@example.com").await;
    let vault_id = Uuid::new_v4();
    let body = support::activate_body("user@example.com", &invite, vault_id, &[9u8; 32]);

    let res = server
        .post("/v1/accounts/activate")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let db = server.db().await;
    let row = db
        .query_one(
            "SELECT status, auth_verifier, kdf_memory_kib, invite_hash
               FROM accounts WHERE email_normalized = 'user@example.com'",
            &[],
        )
        .await
        .unwrap();
    let status: String = row.get(0);
    let verifier: Option<String> = row.get(1);
    let memory: Option<i32> = row.get(2);
    let invite_hash: Option<Vec<u8>> = row.get(3);
    assert_eq!(status, "active");
    assert_eq!(memory, Some(131072));
    assert!(invite_hash.is_none(), "the invite is spent, not kept");
    let verifier = verifier.unwrap();
    assert!(
        verifier.starts_with("$argon2id$"),
        "the auth key is stored hashed, never as received"
    );
    assert!(!verifier.contains(&BASE64.encode(&[9u8; 32])));

    let header: Vec<u8> = db
        .query_one("SELECT header FROM vaults WHERE id = $1", &[&vault_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(header, b"header-bytes-v1".to_vec());

    // Single use: the same invite cannot activate a second time.
    let again = server
        .post("/v1/accounts/activate")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), 400);

    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn activation_refuses_a_wrong_invite_an_expired_one_and_a_mismatched_email() {
    let server = support::TestServer::start().await;
    let invite = support::new_invite(&server, "user@example.com").await;
    let parsed = havenkeys_server::invite::decode(&invite).unwrap();

    // A different secret for the right account.
    let forged = havenkeys_server::invite::encode(&havenkeys_server::invite::Invite {
        secret: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        ..parsed.clone()
    });
    let res = server
        .post("/v1/accounts/activate")
        .json(&support::activate_body(
            "user@example.com",
            &forged,
            Uuid::new_v4(),
            &[1u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // A real invite presented for someone else's address.
    let res = server
        .post("/v1/accounts/activate")
        .json(&support::activate_body(
            "other@example.com",
            &invite,
            Uuid::new_v4(),
            &[1u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // Expired.
    server
        .db()
        .await
        .execute(
            "UPDATE accounts SET invite_expires_at = now() - interval '1 day' WHERE id = $1",
            &[&parsed.account],
        )
        .await
        .unwrap();
    let res = server
        .post("/v1/accounts/activate")
        .json(&support::activate_body(
            "user@example.com",
            &invite,
            Uuid::new_v4(),
            &[1u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    let status: String = server
        .db()
        .await
        .query_one("SELECT status FROM accounts WHERE id = $1", &[&parsed.account])
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "invited", "no failed attempt activated the account");
    server.cleanup().await;
}

#[tokio::test]
async fn activation_refuses_weak_kdf_parameters_a_wrong_scheme_and_a_short_auth_key() {
    let server = support::TestServer::start().await;

    for mutate in [
        |b: &mut serde_json::Value| b["kdf"]["memoryKib"] = serde_json::json!(1024),
        |b: &mut serde_json::Value| b["kdf"]["iterations"] = serde_json::json!(1),
        |b: &mut serde_json::Value| b["kdf"]["algorithm"] = serde_json::json!("pbkdf2"),
        |b: &mut serde_json::Value| {
            b["kdf"]["salt"] = serde_json::json!(BASE64.encode(&[0u8; 8]))
        },
        |b: &mut serde_json::Value| b["keyScheme"] = serde_json::json!(2),
        |b: &mut serde_json::Value| b["authKey"] = serde_json::json!(BASE64.encode(&[0u8; 16])),
        |b: &mut serde_json::Value| b["header"] = serde_json::json!(""),
    ] {
        let invite = support::new_invite(&server, &format!("u{}@example.com", Uuid::new_v4())).await;
        let parsed = havenkeys_server::invite::decode(&invite).unwrap();
        let mut body =
            support::activate_body(&parsed.email, &invite, Uuid::new_v4(), &[1u8; 32]);
        mutate(&mut body);
        let res = server
            .post("/v1/accounts/activate")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "accepted {body}");
    }

    let active: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM accounts WHERE status = 'active'", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(active, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn an_unknown_field_is_refused_rather_than_ignored() {
    let server = support::TestServer::start().await;
    let invite = support::new_invite(&server, "user@example.com").await;
    let mut body = support::activate_body("user@example.com", &invite, Uuid::new_v4(), &[1u8; 32]);
    body["accountId"] = serde_json::json!(Uuid::new_v4());
    let res = server
        .post("/v1/accounts/activate")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    server.cleanup().await;
}

#[tokio::test]
async fn login_returns_a_session_and_the_vault_id() {
    let server = support::TestServer::start().await;
    let account = support::activate(&server, "user@example.com", [9u8; 32]).await;
    let session = support::login(&server, &account, "Desktop").await;
    assert_eq!(session.vault_id, account.vault_id);

    let db = server.db().await;
    let rows = db.query("SELECT token_hash FROM sessions", &[]).await.unwrap();
    assert_eq!(rows.len(), 1);
    let stored: Vec<u8> = rows[0].get(0);
    assert_eq!(
        stored,
        havenkeys_server::auth::token_hash(&session.token),
        "only the token's hash is stored"
    );

    // The device was recorded under the account, with the label the client
    // chose and no hostname the server invented.
    let name: String = db
        .query_one("SELECT name FROM devices WHERE id = $1", &[&session.device_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(name, "Desktop");

    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_key_an_unknown_email_and_an_inactive_account_answer_identically() {
    let server = support::TestServer::start().await;
    let account = support::activate(&server, "user@example.com", [9u8; 32]).await;
    support::new_invite(&server, "invited@example.com").await;

    let mut answers = Vec::new();
    for (email, key) in [
        ("user@example.com", [1u8; 32]),      // right account, wrong key
        ("nobody@example.com", [9u8; 32]),    // no such account
        ("invited@example.com", [9u8; 32]),   // invited, never activated
    ] {
        let res = server
            .post("/v1/auth/login")
            .json(&serde_json::json!({
                "email": email,
                "authKey": BASE64.encode(&key),
                "deviceId": Uuid::new_v4(),
                "deviceName": "Desktop",
            }))
            .send()
            .await
            .unwrap();
        answers.push((res.status().as_u16(), res.text().await.unwrap()));
    }
    assert!(
        answers.iter().all(|a| a == &answers[0]),
        "login must not say which part was wrong: {answers:?}"
    );
    assert_eq!(answers[0].0, 401);

    // The real account still works afterwards.
    support::login(&server, &account, "Desktop").await;
    server.cleanup().await;
}

#[tokio::test]
async fn auth_params_does_not_reveal_whether_an_account_exists() {
    let server = support::TestServer::start().await;
    support::activate(&server, "user@example.com", [9u8; 32]).await;

    let real = ask_params(&server, "user@example.com").await;
    let fake = ask_params(&server, "nobody@example.com").await;
    let fake_again = ask_params(&server, "nobody@example.com").await;

    assert_eq!(
        real.as_object().unwrap().keys().collect::<Vec<_>>(),
        fake.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    assert_eq!(real["kdf"]["algorithm"], "argon2id");
    assert_eq!(fake["kdf"]["algorithm"], "argon2id");
    assert_eq!(fake, fake_again, "a decoy must be stable across asks");
    assert_ne!(fake["accountId"], real["accountId"]);
    assert_ne!(
        ask_params(&server, "someone-else@example.com").await["accountId"],
        fake["accountId"],
        "each unknown address gets its own decoy"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn repeated_failures_block_the_account_and_success_clears_the_counter() {
    let server = support::TestServer::start().await;
    let account = support::activate(&server, "user@example.com", [9u8; 32]).await;

    for _ in 0..5 {
        assert_eq!(try_login(&server, "user@example.com", [1u8; 32]).await, 401);
    }
    assert_eq!(
        try_login(&server, "user@example.com", [1u8; 32]).await,
        429,
        "the block is in force"
    );
    // Even the right key is refused while blocked.
    assert_eq!(try_login(&server, "user@example.com", [9u8; 32]).await, 429);

    server
        .db()
        .await
        .execute(
            "UPDATE login_attempts SET blocked_until = now() - interval '1 minute'",
            &[],
        )
        .await
        .unwrap();
    support::login(&server, &account, "Desktop").await;
    let left: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM login_attempts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(left, 0, "a success clears the counters");
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_missing_or_logged_out_token_is_refused() {
    let server = support::TestServer::start().await;
    let (_, session) = support::signed_in(&server, "user@example.com").await;

    assert_eq!(
        server.post("/v1/auth/logout").send().await.unwrap().status(),
        401,
        "no token at all"
    );
    assert_eq!(
        server
            .post("/v1/auth/logout")
            .bearer_auth("not-a-real-token")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // Logging out ends the session for good.
    assert_eq!(
        server
            .post_as("/v1/auth/logout", &session)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        server
            .post_as("/v1/auth/logout", &session)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // An expired row is refused even though it exists.
    let (_, other) = support::signed_in(&server, "two@example.com").await;
    server
        .db()
        .await
        .execute(
            "UPDATE sessions SET expires_at = now() - interval '1 hour' WHERE token_hash = $1",
            &[&havenkeys_server::auth::token_hash(&other.token)],
        )
        .await
        .unwrap();
    assert_eq!(
        server
            .post_as("/v1/auth/logout", &other)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_second_login_on_the_same_device_replaces_its_session() {
    let server = support::TestServer::start().await;
    let account = support::activate(&server, "user@example.com", [9u8; 32]).await;
    let first = support::login(&server, &account, "Desktop").await;

    let res = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": BASE64.encode(&account.auth_key),
            "deviceId": first.device_id,
            "deviceName": "Desktop",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    assert_eq!(
        server
            .post_as("/v1/auth/logout", &first)
            .send()
            .await
            .unwrap()
            .status(),
        401,
        "the old token is gone"
    );
    let sessions: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(sessions, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn a_device_name_that_is_empty_overlong_or_control_laden_is_refused() {
    let server = support::TestServer::start().await;
    let account = support::activate(&server, "user@example.com", [9u8; 32]).await;
    for name in ["", "   ", &"x".repeat(65), "Desk\u{0007}top"] {
        let res = server
            .post("/v1/auth/login")
            .json(&serde_json::json!({
                "email": account.email,
                "authKey": BASE64.encode(&account.auth_key),
                "deviceId": Uuid::new_v4(),
                "deviceName": name,
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "accepted device name {name:?}");
    }
    server.cleanup().await;
}


// ------------------------------------------------------------------ helpers

async fn ask_params(server: &support::TestServer, email: &str) -> serde_json::Value {
    let res = server
        .post("/v1/auth/params")
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    res.json().await.unwrap()
}

async fn try_login(server: &support::TestServer, email: &str, key: [u8; 32]) -> u16 {
    server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": email,
            "authKey": BASE64.encode(&key),
            "deviceId": Uuid::new_v4(),
            "deviceName": "Desktop",
        }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}
