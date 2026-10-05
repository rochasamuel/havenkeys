mod support;

use havenkeys_server::admin::{self, AdminCommand};
use support::*;
use uuid::Uuid;

/// Every text/uuid column of every table in `public`, checked for the
/// deleted account's identifiers. A table added later without a cascade
/// fails this test instead of silently keeping personal data.
async fn traces_of(server: &TestServer, needles: &[String]) -> Vec<String> {
    let db = server.db().await;
    let cols = db
        .query(
            "SELECT table_name::text, column_name::text FROM information_schema.columns
              WHERE table_schema = 'public'
                AND table_name NOT IN ('schema_migrations', 'deleted_sessions')
                AND data_type IN ('text', 'uuid', 'character varying')",
            &[],
        )
        .await
        .unwrap();
    let mut found = Vec::new();
    for c in cols {
        let (table, column): (String, String) = (c.get(0), c.get(1));
        for needle in needles {
            let n: i64 = db
                .query_one(
                    &format!(
                        r#"SELECT count(*) FROM "{table}" WHERE "{column}"::text ILIKE '%' || $1 || '%'"#
                    ),
                    &[needle],
                )
                .await
                .unwrap()
                .get(0);
            if n > 0 {
                found.push(format!("{table}.{column}"));
            }
        }
    }
    found
}

#[tokio::test]
async fn admin_delete_leaves_no_trace_of_the_account() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "gone@example.com").await;
    create_item(&server, &sess, b"o", b"d").await;
    // A failed attempt leaves an "acct:<uuid>" rate-limit row.
    let _ = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&[9u8; 32]),
            "deviceId": Uuid::new_v4(),
            "deviceName": "X",
        }))
        .send()
        .await
        .unwrap();

    admin::run(
        AdminCommand::DeleteAccount {
            email: "gone@example.com".into(),
        },
        server.pool(),
    )
    .await
    .unwrap();

    let needles = vec![
        account.account_id.to_string(),
        account.vault_id.to_string(),
        sess.device_id.to_string(),
        "gone@example.com".to_string(),
    ];
    assert_eq!(traces_of(&server, &needles).await, Vec::<String>::new());
    let tombstones: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM deleted_sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(tombstones, 1, "the live session is tombstoned");
    server.cleanup().await;
}

#[tokio::test]
async fn the_sweep_removes_only_expired_tombstones() {
    let server = TestServer::start().await;
    let db = server.db().await;
    db.execute(
        "INSERT INTO deleted_sessions (token_hash, expires_at) VALUES
           ($1, now() - interval '1 second'), ($2, now() + interval '1 day')",
        &[&vec![1u8; 32], &vec![2u8; 32]],
    )
    .await
    .unwrap();
    assert_eq!(
        havenkeys_server::erase::sweep_tombstones(&db)
            .await
            .unwrap(),
        1
    );
    let left: Vec<u8> = db
        .query_one("SELECT token_hash FROM deleted_sessions", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(left, vec![2u8; 32]);
    drop(db);
    server.cleanup().await;
}

async fn delete(server: &TestServer, sess: &Sess, key: &[u8; 32], email: &str) -> u16 {
    server
        .post_as("/v1/account/delete", sess)
        .json(&serde_json::json!({
            "currentAuthKey": data_encoding::BASE64.encode(key),
            "email": email,
        }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn accounts(server: &TestServer) -> i64 {
    server
        .db()
        .await
        .query_one("SELECT count(*) FROM accounts", &[])
        .await
        .unwrap()
        .get(0)
}

#[tokio::test]
async fn the_owner_deletes_and_other_devices_see_410() {
    let server = TestServer::start().await;
    let (account, phone) = signed_in(&server, "me@example.com").await;
    let laptop = login(&server, &account, "Laptop").await;

    assert_eq!(
        delete(&server, &phone, &account.auth_key, " Me@Example.com ").await,
        204
    );
    assert_eq!(accounts(&server).await, 0);

    let res = server
        .get_as("/v1/sync?since=0", &laptop)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 410);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"]["code"], "account_deleted");

    let stranger = Sess {
        token: "x".repeat(43),
        ..laptop
    };
    assert_eq!(
        server
            .get_as("/v1/sync?since=0", &stranger)
            .send()
            .await
            .unwrap()
            .status(),
        401,
        "an unknown token learns nothing"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_tombstone_answers_401() {
    let server = TestServer::start().await;
    let (account, phone) = signed_in(&server, "old@example.com").await;
    let laptop = login(&server, &account, "Laptop").await;
    assert_eq!(
        delete(&server, &phone, &account.auth_key, "old@example.com").await,
        204
    );
    server
        .db()
        .await
        .execute(
            "UPDATE deleted_sessions SET expires_at = now() - interval '1 second'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        server
            .get_as("/v1/sync?since=0", &laptop)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_stolen_token_alone_cannot_delete_and_the_failure_counts() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "thief@example.com").await;
    assert_eq!(
        delete(&server, &sess, &[1u8; 32], "thief@example.com").await,
        401
    );
    assert_eq!(accounts(&server).await, 1);
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
    server.cleanup().await;
}

#[tokio::test]
async fn a_rate_limited_account_is_refused_before_checking() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "slow@example.com").await;
    for _ in 0..5 {
        delete(&server, &sess, &[1u8; 32], "slow@example.com").await;
    }
    assert_eq!(
        delete(&server, &sess, &account.auth_key, "slow@example.com").await,
        429
    );
    assert_eq!(accounts(&server).await, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn another_email_is_refused() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "typo@example.com").await;
    assert_eq!(
        delete(&server, &sess, &account.auth_key, "other@example.com").await,
        400
    );
    assert_eq!(accounts(&server).await, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn unknown_fields_are_refused() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "extra@example.com").await;
    let status = server
        .post_as("/v1/account/delete", &sess)
        .json(&serde_json::json!({
            "currentAuthKey": data_encoding::BASE64.encode(&account.auth_key),
            "email": "extra@example.com",
            "accountId": Uuid::new_v4(),
        }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 400);
    assert_eq!(accounts(&server).await, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn the_route_leaves_no_trace_either() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "route@example.com").await;
    create_item(&server, &sess, b"o", b"d").await;
    assert_eq!(
        delete(&server, &sess, &account.auth_key, "route@example.com").await,
        204
    );
    let needles = vec![
        account.account_id.to_string(),
        account.vault_id.to_string(),
        sess.device_id.to_string(),
        "route@example.com".to_string(),
    ];
    assert_eq!(traces_of(&server, &needles).await, Vec::<String>::new());
    server.cleanup().await;
}
