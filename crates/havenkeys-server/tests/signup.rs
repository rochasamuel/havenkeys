mod support;

use havenkeys_server::invite;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::{Digest, Sha256};
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn signup_routes_answer_404_when_signup_is_off() {
    let server = TestServer::start().await;
    let (status, _) = start_signup_for(&server, "new@example.com").await;
    assert_eq!(status, 404);
    let (status, _) = verify_signup(&server, "new@example.com", "123456").await;
    assert_eq!(status, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn start_answers_the_same_for_a_new_and_an_existing_email() {
    let (server, mailer) = TestServer::start_signup().await;
    signed_in(&server, "old@example.com").await;
    let (s1, b1) = start_signup_for(&server, "old@example.com").await;
    let (s2, b2) = start_signup_for(&server, "new@example.com").await;
    assert_eq!((s1, &b1), (202, &b2));
    assert_eq!(s2, 202);
    let sent = mailer.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].to, "old@example.com");
    assert!(
        code_in(&sent[0]).is_none(),
        "an existing account gets no code"
    );
    assert!(sent[0].subject.contains("already have"));
    assert!(code_in(&sent[1]).is_some());
    let codes: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 1, "no code row for an existing account");
    server.cleanup().await;
}

#[tokio::test]
async fn a_right_code_creates_an_invited_trial_account_and_mails_the_invite() {
    let (server, mailer) = TestServer::start_signup().await;
    let invite_str = signup(&server, &mailer, "New@Example.com").await;
    let parsed = invite::decode(&invite_str).unwrap();
    assert_eq!(parsed.email, "new@example.com");
    assert_eq!(parsed.server, "https://vault.example.com");
    assert_eq!(
        invite_in(mailer.sent().last().unwrap()).as_deref(),
        Some(invite_str.as_str())
    );

    let db = server.db().await;
    let row = db
        .query_one(
            "SELECT a.status, a.created_by, a.terms_version, a.terms_accepted_at IS NOT NULL,
                    a.locale, p.status, p.trial_ends_at,
                    extract(epoch FROM a.invite_expires_at - now())::float8 / 3600
               FROM accounts a JOIN subscriptions p ON p.account_id = a.id WHERE a.id = $1",
            &[&parsed.account],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "invited");
    assert_eq!(row.get::<_, String>(1), "signup");
    assert_eq!(
        row.get::<_, Option<String>>(2).as_deref(),
        Some("2026-10-20")
    );
    assert!(row.get::<_, bool>(3));
    assert_eq!(row.get::<_, Option<String>>(4).as_deref(), Some("en"));
    assert_eq!(row.get::<_, String>(5), "trialing");
    assert!(row
        .get::<_, Option<chrono::DateTime<chrono::Utc>>>(6)
        .is_none());
    let hours: f64 = row.get(7);
    assert!(
        (23.9..=24.0).contains(&hours),
        "a 24 h invite, not the admin's 7 days: {hours}"
    );
    let codes: i64 = db
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 0, "a spent code is deleted");
    drop(db);

    // The invite activates like any other, and the trial starts then.
    let vault_id = Uuid::new_v4();
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body(
            "new@example.com",
            &invite_str,
            vault_id,
            &[5u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let account = Account {
        email: "new@example.com".into(),
        account_id: parsed.account,
        vault_id,
        auth_key: [5u8; 32],
    };
    let sess = login(&server, &account, "Desktop").await;
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["status"], "trialing");
    assert_eq!(pulled["account"]["entitlement"], "full");
    assert!(pulled["account"]["trialEndsAt"].is_string());
    server.cleanup().await;
}

#[tokio::test]
async fn verify_accepts_the_email_in_any_spelling() {
    let (server, mailer) = TestServer::start_signup().await;
    let (status, _) = start_signup_for(&server, "  Mixed.Case@Example.COM ").await;
    assert_eq!(status, 202);
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let (status, _) = verify_signup(&server, "mixed.case@example.com", &code).await;
    assert_eq!(status, 200);
    server.cleanup().await;
}

#[tokio::test]
async fn five_wrong_codes_kill_the_code() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    for _ in 0..4 {
        let (status, body) = verify_signup(&server, "new@example.com", wrong).await;
        assert_eq!(status, 400);
        assert_eq!(body["error"]["code"], "invalid_request");
    }
    // The fifth wrong attempt is the last the code survives.
    let (status, _) = verify_signup(&server, "new@example.com", wrong).await;
    assert_eq!(status, 400);
    let (status, _) = verify_signup(&server, "new@example.com", &code).await;
    assert_eq!(status, 400, "dead after five wrong attempts");
    let codes: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_code_is_refused() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    server
        .db()
        .await
        .execute(
            "UPDATE signup_codes SET expires_at = now() - interval '1 second'",
            &[],
        )
        .await
        .unwrap();
    let (status, _) = verify_signup(&server, "new@example.com", &code).await;
    assert_eq!(status, 400);
    server.cleanup().await;
}

#[tokio::test]
async fn the_stored_code_hash_is_keyed_by_the_server_secret() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let stored: Vec<u8> = server
        .db()
        .await
        .query_one("SELECT code_hash FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_ne!(
        stored,
        Sha256::digest(code.as_bytes()).to_vec(),
        "not a bare hash"
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(&[7u8; 32]).unwrap();
    mac.update(code.as_bytes());
    assert_eq!(stored, mac.finalize().into_bytes().to_vec());
    assert_eq!(
        stored,
        havenkeys_server::routes::signup::code_hash(&[7u8; 32], &code)
    );
    let keys: Vec<String> = server
        .db()
        .await
        .query("SELECT key FROM login_attempts", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert!(!keys.is_empty());
    assert!(
        keys.iter().all(|k| !k.contains("example.com")),
        "no counter key holds the email: {keys:?}"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_second_start_replaces_the_code() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let first = code_in(&mailer.sent()[0]).unwrap();
    // Burn an attempt, then resend: the new code starts with zero attempts.
    let wrong = if first == "000000" {
        "000001"
    } else {
        "000000"
    };
    verify_signup(&server, "new@example.com", wrong).await;
    start_signup_for(&server, "new@example.com").await;
    let second = code_in(&mailer.sent()[1]).unwrap();
    let attempts: i32 = server
        .db()
        .await
        .query_one("SELECT attempts FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 0);
    if first != second {
        let (status, _) = verify_signup(&server, "new@example.com", &first).await;
        assert_eq!(status, 400, "the old code is dead");
    }
    let (status, _) = verify_signup(&server, "new@example.com", &second).await;
    assert_eq!(status, 200);
    server.cleanup().await;
}

#[tokio::test]
async fn starts_are_limited_per_email() {
    let (server, _) = TestServer::start_signup().await;
    for _ in 0..3 {
        let (status, _) = start_signup_for(&server, "same@example.com").await;
        assert_eq!(status, 202);
    }
    let (status, body) = start_signup_for(&server, "same@example.com").await;
    assert_eq!(status, 429, "{body}");
    assert_eq!(body["error"]["code"], "rate_limited");
    // The address counter is separate: another email is still fine (4 of 5).
    let (status, _) = start_signup_for(&server, "other@example.com").await;
    assert_eq!(status, 202);
    server.cleanup().await;
}

#[tokio::test]
async fn starts_are_limited_per_address() {
    let (server, _) = TestServer::start_signup().await;
    for n in 0..5 {
        let (status, _) = start_signup_for(&server, &format!("u{n}@example.com")).await;
        assert_eq!(status, 202);
    }
    let (status, _) = start_signup_for(&server, "u6@example.com").await;
    assert_eq!(status, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn verify_failures_count_against_the_address() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    // One start plus nine failures fill the address window (10). The code
    // itself died at five; the later failures hit "no code" and still count.
    for _ in 0..9 {
        let (status, _) = verify_signup(&server, "new@example.com", wrong).await;
        assert_eq!(status, 400);
    }
    let (status, _) = verify_signup(&server, "nobody@example.com", "123456").await;
    assert_eq!(status, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn two_verify_calls_racing_on_one_code_one_wins() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let (a, b) = tokio::join!(
        verify_signup(&server, "new@example.com", &code),
        verify_signup(&server, "new@example.com", &code)
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 400]);
    let accounts: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM accounts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(accounts, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn an_admin_invited_account_gets_its_invite_replaced_by_signup() {
    let (server, mailer) = TestServer::start_signup().await;
    let old = new_invite(&server, "both@example.com").await;
    let new = signup(&server, &mailer, "both@example.com").await;
    assert_eq!(
        invite::decode(&old).unwrap().account,
        invite::decode(&new).unwrap().account
    );
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body(
            "both@example.com",
            &old,
            Uuid::new_v4(),
            &[5u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "the old invite stopped working");
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body(
            "both@example.com",
            &new,
            Uuid::new_v4(),
            &[5u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let plan: String = server
        .db()
        .await
        .query_one(
            "SELECT p.status FROM subscriptions p JOIN accounts a ON a.id = p.account_id WHERE a.email_normalized = $1",
            &[&"both@example.com"],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(plan, "complimentary", "the admin's plan row is kept");
    let created_by: String = server
        .db()
        .await
        .query_one(
            "SELECT created_by FROM accounts WHERE email_normalized = $1",
            &[&"both@example.com"],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(created_by, "admin", "provenance is not overwritten");
    server.cleanup().await;
}

#[tokio::test]
async fn start_answers_503_uniformly_when_mail_fails() {
    let (server, mailer) = TestServer::start_signup().await;
    signed_in(&server, "old@example.com").await;
    mailer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let (s1, b1) = start_signup_for(&server, "old@example.com").await;
    let (s2, b2) = start_signup_for(&server, "new@example.com").await;
    assert_eq!((s1, &b1), (503, &b2));
    assert_eq!(s2, 503);
    server.cleanup().await;
}

#[tokio::test]
async fn verify_still_returns_the_invite_when_mail_fails() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    mailer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let (status, body) = verify_signup(&server, "new@example.com", &code).await;
    assert_eq!(status, 200);
    assert!(body["invite"].as_str().unwrap().starts_with("HKINV1-"));
    server.cleanup().await;
}

#[tokio::test]
async fn malformed_signup_requests_are_refused() {
    let (server, _) = TestServer::start_signup().await;
    for body in [
        json!({ "email": "a@example.com", "locale": "fr", "acceptedTerms": "2026-10-20" }),
        json!({ "email": "a@example.com", "locale": "en", "acceptedTerms": "yes" }),
        json!({ "email": "a@example.com", "locale": "en", "acceptedTerms": "" }),
        json!({ "email": "not-an-email", "locale": "en", "acceptedTerms": "2026-10-20" }),
        json!({ "email": "a@example.com", "locale": "en" }),
        json!({ "email": "a@example.com", "locale": "en", "acceptedTerms": "2026-10-20", "extra": 1 }),
    ] {
        let status = server
            .post("/v1/signup/start")
            .json(&body)
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, 400, "{body}");
    }
    for code in ["12345", "1234567", "12345a", ""] {
        let (status, _) = verify_signup(&server, "a@example.com", code).await;
        assert_eq!(status, 400, "{code:?}");
    }
    server.cleanup().await;
}

#[tokio::test]
async fn a_code_for_an_account_activated_meanwhile_is_refused() {
    let (server, mailer) = TestServer::start_signup().await;
    let admin_invite = new_invite(&server, "late@example.com").await;
    let (status, _) = start_signup_for(&server, "late@example.com").await;
    assert_eq!(status, 202);
    let code = code_in(mailer.sent().last().unwrap()).expect("an invited account gets a code");
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body(
            "late@example.com",
            &admin_invite,
            Uuid::new_v4(),
            &[5u8; 32],
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let (status, _) = verify_signup(&server, "late@example.com", &code).await;
    assert_eq!(status, 400);
    let codes: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn start_for_a_disabled_account_mails_the_notice_and_leaves_no_code() {
    let (server, mailer) = TestServer::start_signup().await;
    signed_in(&server, "off@example.com").await;
    server
        .db()
        .await
        .execute("UPDATE accounts SET status = 'disabled'", &[])
        .await
        .unwrap();
    let (status, _) = start_signup_for(&server, "off@example.com").await;
    assert_eq!(status, 202);
    assert!(code_in(mailer.sent().last().unwrap()).is_none());
    let codes: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn starts_are_capped_globally() {
    let (server, _) = TestServer::start_signup().await;
    server
        .db()
        .await
        .execute(
            "INSERT INTO login_attempts (key, failures, window_start) VALUES ('signup-global', 300, now())",
            &[],
        )
        .await
        .unwrap();
    let (status, body) = start_signup_for(&server, "one@example.com").await;
    assert_eq!(status, 429, "{body}");
    server.cleanup().await;
}

#[tokio::test]
async fn a_slow_mailer_does_not_hold_database_connections() {
    let (server, mailer) = TestServer::start_with(Options {
        signup: true,
        trust_forwarded_for: true,
        ..Options::default()
    })
    .await;
    let (_, sess) = signed_in(&server, "user@example.com").await;
    let gate = std::sync::Arc::new(tokio::sync::Notify::new());
    *mailer.block.lock().unwrap() = Some(gate.clone());

    // More starts than the pool has connections (10), all stuck in SMTP.
    let mut starts = Vec::new();
    for n in 0..12 {
        let req = server
            .post("/v1/signup/start")
            .header("x-forwarded-for", format!("10.0.0.{n}"))
            .json(&json!({
                "email": format!("slow{n}@example.com"),
                "locale": "en",
                "acceptedTerms": "2026-10-20"
            }));
        starts.push(tokio::spawn(async move {
            req.send().await.unwrap().status().as_u16()
        }));
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        server.get_as("/v1/vault/header", &sess).send(),
    )
    .await
    .expect("the pool is not exhausted by pending mail")
    .unwrap();
    assert_eq!(res.status(), 200);

    gate.notify_waiters();
    // `notify_waiters` wakes only those already waiting; keep nudging until
    // every start has answered.
    for task in starts {
        let status = loop {
            if task.is_finished() {
                break task.await.unwrap();
            }
            gate.notify_waiters();
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        };
        assert_eq!(status, 202);
    }
    server.cleanup().await;
}
