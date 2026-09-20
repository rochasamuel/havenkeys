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
