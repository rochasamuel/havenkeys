//! Nothing secret may reach a log line.
//!
//! The server is the one place where every account's ciphertext passes
//! through, so a log that quoted a request would be a breach of the whole
//! fleet at once. This runs a full session — activate, log in, write, pull,
//! revoke — with every trace event captured, and then looks for the values
//! that must never appear (CLAUDE.md §40, design §7.5).

mod support;

use std::io;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Captured;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::test]
async fn no_secret_reaches_a_log_line() {
    let buffer = Captured(Arc::new(Mutex::new(Vec::new())));
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_writer(buffer.clone())
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .finish(),
    )
    .expect("this test owns the subscriber");

    let server = support::TestServer::start().await;
    let email = "user@example.com";
    let invite = support::new_invite(&server, email).await;
    let vault_id = uuid::Uuid::new_v4();
    let auth_key = [42u8; 32];

    let res = server
        .post("/v1/accounts/activate")
        .json(&support::activate_body(email, &invite, vault_id, &auth_key))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let account_id: String = res.json::<serde_json::Value>().await.unwrap()["accountId"]
        .as_str()
        .unwrap()
        .to_string();

    let account = support::Account {
        email: email.into(),
        account_id: account_id.parse().unwrap(),
        vault_id,
        auth_key,
    };
    let session = support::login(&server, &account, "Desktop").await;

    let overview = b"OVERVIEW-CIPHERTEXT-MARKER";
    let details = b"DETAILS-CIPHERTEXT-MARKER";
    let (item, _) = support::create_item(&server, &session, overview, details).await;
    support::pull(&server, &session, 0).await;
    let new_key = [88u8; 32];
    server
        .post_as("/v1/account/credentials", &session)
        .json(&support::credentials_body(
            &auth_key,
            &new_key,
            0,
            b"HEADER-MARKER",
        ))
        .send()
        .await
        .unwrap();
    server
        .delete_as(&format!("/v1/devices/{}", session.device_id), &session)
        .send()
        .await
        .unwrap();
    // A failed login, because the error path is where a careless log lands.
    server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": email,
            "authKey": data_encoding::BASE64.encode(&[0u8; 32]),
            "deviceId": uuid::Uuid::new_v4(),
            "deviceName": "Desktop",
        }))
        .send()
        .await
        .unwrap();

    let logs = String::from_utf8_lossy(&buffer.0.lock().unwrap().clone()).to_string();
    assert!(
        logs.contains(&account_id),
        "the account id is what logs identify by; without it this test would pass on silence"
    );

    let forbidden: Vec<(&str, String)> = vec![
        ("the session token", session.token.clone()),
        ("the invite string", invite.clone()),
        (
            "the invite secret",
            havenkeys_server::invite::decode(&invite).unwrap().secret,
        ),
        ("the auth key", data_encoding::BASE64.encode(&auth_key)),
        ("the new auth key", data_encoding::BASE64.encode(&new_key)),
        ("the overview blob", data_encoding::BASE64.encode(overview)),
        (
            "the overview plaintext",
            String::from_utf8_lossy(overview).to_string(),
        ),
        ("the details blob", data_encoding::BASE64.encode(details)),
        (
            "the details plaintext",
            String::from_utf8_lossy(details).to_string(),
        ),
        ("the header bytes", "HEADER-MARKER".to_string()),
        ("the email address", email.to_string()),
    ];
    for (what, value) in forbidden {
        assert!(
            !logs.contains(&value),
            "{what} appeared in the logs:\n{logs}"
        );
    }
    let _ = item;
    server.cleanup().await;
}
