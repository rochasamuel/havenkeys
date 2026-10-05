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
