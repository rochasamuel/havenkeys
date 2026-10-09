mod support;

use havenkeys_server::admin::{self, AdminCommand};
use havenkeys_server::invite;

fn new_account(email: &str) -> AdminCommand {
    AdminCommand::NewAccount {
        email: email.into(),
        server_url: "https://vault.example.com".into(),
        trial: false,
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

#[tokio::test]
async fn new_account_is_complimentary_by_default_and_trialing_on_request() {
    let server = support::TestServer::start().await;
    let printed = admin::run(new_account("free@example.com"), server.pool())
        .await
        .unwrap();
    let free = invite::decode(printed.trim()).unwrap().account;
    let printed = admin::run(
        AdminCommand::NewAccount {
            email: "trial@example.com".into(),
            server_url: "https://vault.example.com".into(),
            trial: true,
        },
        server.pool(),
    )
    .await
    .unwrap();
    let trial = invite::decode(printed.trim()).unwrap().account;

    let db = server.db().await;
    for (id, expected) in [(free, "complimentary"), (trial, "trialing")] {
        let row = db
            .query_one(
                "SELECT status, trial_ends_at FROM subscriptions WHERE account_id = $1",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>(0), expected);
        assert!(
            row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(1)
                .is_none(),
            "the trial starts at activation, not at invitation"
        );
        let events: i64 = db
            .query_one(
                "SELECT count(*) FROM billing_events WHERE account_id = $1 AND actor = 'admin'",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(events, 1);
    }
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn set_plan_changes_the_status_and_records_it() {
    let server = support::TestServer::start().await;
    let (account, _) = support::signed_in(&server, "user@example.com").await;
    let out = admin::run(
        AdminCommand::SetPlan {
            email: "User@Example.com".into(),
            status: havenkeys_server::billing::Status::Active,
            until: Some("2027-01-31".into()),
        },
        server.pool(),
    )
    .await
    .unwrap();
    assert_eq!(out, "active until 2027-01-31T00:00:00+00:00");
    let db = server.db().await;
    let row = db
        .query_one(
            "SELECT status, current_period_end::text FROM subscriptions WHERE account_id = $1",
            &[&account.account_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "active");
    assert!(row.get::<_, String>(1).starts_with("2027-01-31"));
    let last: (Option<String>, String) = db
        .query_one(
            "SELECT old_status, new_status FROM billing_events WHERE account_id = $1 ORDER BY id DESC LIMIT 1",
            &[&account.account_id],
        )
        .await
        .map(|r| (r.get(0), r.get(1)))
        .unwrap();
    assert_eq!(last, (Some("complimentary".into()), "active".into()));
    assert!(admin::run(
        AdminCommand::SetPlan {
            email: "nobody@example.com".into(),
            status: havenkeys_server::billing::Status::Frozen,
            until: None,
        },
        server.pool(),
    )
    .await
    .is_err());
    assert!(admin::run(
        AdminCommand::SetPlan {
            email: "user@example.com".into(),
            status: havenkeys_server::billing::Status::Active,
            until: Some("next tuesday".into()),
        },
        server.pool(),
    )
    .await
    .is_err());
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn list_accounts_shows_the_plan() {
    let server = support::TestServer::start().await;
    support::signed_in(&server, "user@example.com").await;
    let out = admin::run(AdminCommand::ListAccounts, server.pool())
        .await
        .unwrap();
    assert!(
        out.contains("user@example.com  active  complimentary"),
        "{out}"
    );
    server.cleanup().await;
}
