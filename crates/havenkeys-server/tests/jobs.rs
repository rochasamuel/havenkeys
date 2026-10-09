mod support;

use havenkeys_server::billing::jobs::run_daily;
use havenkeys_server::billing::Status;
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn abandoned_signups_are_swept_after_seven_days_and_nothing_else_is() {
    let (server, mailer) = TestServer::start_signup().await;
    // Three invited accounts: a signup from 8 days ago, a signup from
    // yesterday, and an admin invite from 8 days ago.
    signup(&server, &mailer, "stale@example.com").await;
    signup(&server, &mailer, "fresh@example.com").await;
    new_invite(&server, "admin@example.com").await;
    // And one signup that activated 8 days ago.
    let invite = signup(&server, &mailer, "active@example.com").await;
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body(
            "active@example.com",
            &invite,
            Uuid::new_v4(),
            &[5u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let db = server.db().await;
    db.execute(
        "UPDATE accounts SET created_at = now() - interval '8 days'
          WHERE email_normalized IN ('stale@example.com', 'admin@example.com', 'active@example.com')",
        &[],
    )
    .await
    .unwrap();

    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.signups_abandoned, 1);
    let left: Vec<String> = db
        .query("SELECT email_normalized FROM accounts ORDER BY 1", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(
        left,
        [
            "active@example.com",
            "admin@example.com",
            "fresh@example.com"
        ]
    );
    let orphans: i64 = db
        .query_one(
            "SELECT count(*) FROM subscriptions p LEFT JOIN accounts a ON a.id = p.account_id WHERE a.id IS NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(orphans, 0);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn expired_codes_and_stale_signup_counters_are_swept() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "a@example.com").await;
    start_signup_for(&server, "b@example.com").await;
    let db = server.db().await;
    db.execute(
        "UPDATE signup_codes SET expires_at = now() - interval '1 minute' WHERE email_normalized = 'a@example.com'",
        &[],
    )
    .await
    .unwrap();
    db.execute(
        "UPDATE login_attempts SET window_start = now() - interval '3 hours' WHERE key LIKE 'signup-email:%'",
        &[],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.codes_swept, 1);
    assert_eq!(
        report.counters_swept, 2,
        "the two email counters; the address one is fresh"
    );
    let codes: i64 = db
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 1);
    let ip_rows: i64 = db
        .query_one(
            "SELECT count(*) FROM login_attempts WHERE key LIKE 'signup-ip:%'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(ip_rows, 1);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn trial_notices_go_out_once_each() {
    let (server, mailer) = TestServer::start_signup().await;
    let (account, _) = signed_in(&server, "user@example.com").await;
    set_plan(&server, account.account_id, Status::Trialing).await;
    let db = server.db().await;
    db.execute(
        "UPDATE accounts SET locale = 'pt-BR' WHERE id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let before = mailer.sent().len();

    // Ten days left: nothing.
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() + interval '10 days' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!((report.ending_notices, report.ended_notices), (0, 0));

    // Two and a half days left: the ending notice, once, in Portuguese.
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() + interval '60 hours' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ending_notices, 1);
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ending_notices, 0, "sent once");
    let sent = mailer.sent();
    assert_eq!(sent.len(), before + 1);
    assert_eq!(sent[before].to, "user@example.com");
    assert!(
        sent[before].subject.contains("termina em 3 dias"),
        "{}",
        sent[before].subject
    );

    // Ended: the ended notice, once.
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() - interval '1 hour' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ended_notices, 1);
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ended_notices, 0);
    assert!(mailer.sent().last().unwrap().subject.contains("terminou"));

    let marks: i64 = db
        .query_one(
            "SELECT count(*) FROM billing_events WHERE account_id = $1 AND reason LIKE 'trial_%_notice'",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(marks, 2);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn without_a_mailer_no_notice_is_marked_as_sent() {
    let server = TestServer::start().await;
    let (account, _) = signed_in(&server, "user@example.com").await;
    set_plan(&server, account.account_id, Status::Trialing).await;
    let db = server.db().await;
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() - interval '1 hour' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, None).await.unwrap();
    assert_eq!(report.ended_notices, 0);
    let marks: i64 = db
        .query_one(
            "SELECT count(*) FROM billing_events WHERE reason = 'trial_ended_notice'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        marks, 0,
        "unsent is unmarked, so it goes out once SMTP exists"
    );
    drop(db);
    server.cleanup().await;
}
