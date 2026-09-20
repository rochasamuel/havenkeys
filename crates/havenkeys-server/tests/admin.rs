mod support;

use havenkeys_server::admin::{self, AdminCommand};
use havenkeys_server::invite;

fn new_account(email: &str) -> AdminCommand {
    AdminCommand::NewAccount {
        email: email.into(),
        server_url: "https://vault.example.com".into(),
    }
}

#[tokio::test]
async fn new_account_stores_only_the_invite_hash() {
    let server = support::TestServer::start().await;
    let printed = admin::run(new_account("  User@Example.COM "), server.pool())
        .await
        .unwrap();

    let parsed = invite::decode(printed.trim()).unwrap();
    assert_eq!(
        parsed.email, "user@example.com",
        "the email is normalized once, by the server"
    );

    let db = server.db().await;
    let row = db
        .query_one(
            "SELECT email_normalized, status, invite_hash FROM accounts WHERE id = $1",
            &[&parsed.account],
        )
        .await
        .unwrap();
    let email: String = row.get(0);
    let status: String = row.get(1);
    let stored: Option<Vec<u8>> = row.get(2);
    assert_eq!(email, "user@example.com");
    assert_eq!(status, "invited");
    assert_eq!(stored.unwrap(), invite::hash(&parsed.secret).to_vec());

    // The secret itself is nowhere in the row.
    let plaintext: i64 = db
        .query_one(
            "SELECT count(*) FROM accounts WHERE encode(invite_hash, 'escape') LIKE $1",
            &[&format!("%{}%", parsed.secret)],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(plaintext, 0);

    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn two_invites_for_the_same_email_are_refused() {
    let server = support::TestServer::start().await;
    admin::run(new_account("dup@example.com"), server.pool())
        .await
        .unwrap();
    assert!(admin::run(new_account("dup@example.com"), server.pool())
        .await
        .is_err());
    server.cleanup().await;
}

#[tokio::test]
async fn an_invalid_email_never_reaches_the_database() {
    let server = support::TestServer::start().await;
    assert!(admin::run(new_account("not-an-email"), server.pool())
        .await
        .is_err());
    let count: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM accounts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn accounts_can_be_listed_and_deleted() {
    let server = support::TestServer::start().await;
    admin::run(new_account("a@example.com"), server.pool())
        .await
        .unwrap();
    let listed = admin::run(AdminCommand::ListAccounts, server.pool())
        .await
        .unwrap();
    assert!(listed.contains("a@example.com"));
    assert!(listed.contains("invited"));

    assert_eq!(
        admin::run(
            AdminCommand::DeleteAccount {
                email: "A@Example.com".into()
            },
            server.pool()
        )
        .await
        .unwrap(),
        "deleted"
    );
    assert!(admin::run(
        AdminCommand::DeleteAccount {
            email: "a@example.com".into()
        },
        server.pool()
    )
    .await
    .is_err());
    server.cleanup().await;
}
