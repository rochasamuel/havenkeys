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

fn b64(bytes: &[u8]) -> String {
    data_encoding::BASE64.encode(bytes)
}

fn unb64(value: &serde_json::Value) -> Vec<u8> {
    data_encoding::BASE64
        .decode(value.as_str().unwrap().as_bytes())
        .unwrap()
}

/// POST `body`, assert success, and return the JSON answer (Null for 204).
async fn post_json(
    server: &support::TestServer,
    token: Option<&str>,
    path: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let mut req = server.post(path).json(&body);
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    let res = req.send().await.unwrap();
    assert!(res.status().is_success(), "{path}: {}", res.status());
    res.json().await.unwrap_or(serde_json::Value::Null)
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

    let (server, mailer) = support::TestServer::start_signup().await;
    let signup_email = "signup-marker@example.com";
    let (status, _) = support::start_signup_for(&server, signup_email).await;
    assert_eq!(status, 202);
    let code = support::code_in(mailer.sent().last().unwrap()).unwrap();
    let (status, body) = support::verify_signup(&server, signup_email, &code).await;
    assert_eq!(status, 200);
    let signup_invite = body["invite"].as_str().unwrap().to_string();

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

    // The OPAQUE paths, inline so every message and id can be looked for:
    // activation, a sign-in, a re-authentication with a credential change,
    // and one failing sign-in.
    let mut opaque: Vec<(&str, String)> = Vec::new();
    let pake_email = "pake-marker@example.com";
    let pake_key = [43u8; 32];
    let pake_new_key = [44u8; 32];
    let pake_invite = support::new_invite(&server, pake_email).await;
    let (reg, request) = havenkeys_opaque::ClientRegistration::start(&pake_key).unwrap();
    let body = post_json(
        &server,
        None,
        "/v1/accounts/activate/register",
        serde_json::json!({ "email": pake_email, "invite": pake_invite,
                            "registrationRequest": b64(&request) }),
    )
    .await;
    let response = unb64(&body["registrationResponse"]);
    let upload = reg.finish(&pake_key, &response).unwrap();
    opaque.push(("the activation registration request", b64(&request)));
    opaque.push(("the activation registration response", b64(&response)));
    opaque.push(("the activation registration upload", b64(&upload)));
    let pake_vault = uuid::Uuid::new_v4();
    let mut activation = support::activate_body(pake_email, &pake_invite, pake_vault, &pake_key);
    activation.as_object_mut().unwrap().remove("authKey");
    activation["registrationUpload"] = serde_json::json!(b64(&upload));
    post_json(&server, None, "/v1/accounts/activate", activation).await;

    let (client, request) = havenkeys_opaque::ClientLogin::start(&pake_key).unwrap();
    let body = post_json(
        &server,
        None,
        "/v1/auth/pake/start",
        serde_json::json!({ "email": pake_email, "credentialRequest": b64(&request) }),
    )
    .await;
    let login_id = body["loginId"].as_str().unwrap().to_string();
    let response = unb64(&body["credentialResponse"]);
    let finalization = client.finish(&pake_key, &response).unwrap();
    opaque.push(("the credential request", b64(&request)));
    opaque.push(("the credential response", b64(&response)));
    opaque.push(("the loginId", login_id.clone()));
    opaque.push(("the credential finalization", b64(&finalization)));
    let body = post_json(
        &server,
        None,
        "/v1/auth/pake/finish",
        serde_json::json!({ "loginId": login_id, "credentialFinalization": b64(&finalization),
                            "deviceId": uuid::Uuid::new_v4(), "deviceName": "Phone" }),
    )
    .await;
    let pake_token = body["token"].as_str().unwrap().to_string();
    opaque.push(("the OPAQUE session token", pake_token.clone()));

    let (client, request) = havenkeys_opaque::ClientLogin::start(&pake_key).unwrap();
    let body = post_json(
        &server,
        Some(&pake_token),
        "/v1/account/pake/reauth",
        serde_json::json!({ "credentialRequest": b64(&request) }),
    )
    .await;
    let reauth_id = body["reauthId"].as_str().unwrap().to_string();
    let response = unb64(&body["credentialResponse"]);
    let reauth_finalization = client.finish(&pake_key, &response).unwrap();
    opaque.push(("the reauth request", b64(&request)));
    opaque.push(("the reauth response", b64(&response)));
    opaque.push(("the reauthId", reauth_id.clone()));
    opaque.push(("the reauth finalization", b64(&reauth_finalization)));
    let (reg, request) = havenkeys_opaque::ClientRegistration::start(&pake_new_key).unwrap();
    let body = post_json(
        &server,
        Some(&pake_token),
        "/v1/account/pake/register",
        serde_json::json!({ "registrationRequest": b64(&request) }),
    )
    .await;
    let response = unb64(&body["registrationResponse"]);
    let upload = reg.finish(&pake_new_key, &response).unwrap();
    opaque.push(("the registration request", b64(&request)));
    opaque.push(("the registration response", b64(&response)));
    opaque.push(("the registration upload", b64(&upload)));
    let body = post_json(
        &server,
        Some(&pake_token),
        "/v1/account/credentials",
        serde_json::json!({
            "reauthId": reauth_id,
            "credentialFinalization": b64(&reauth_finalization),
            "kdf": support::kdf_block(),
            "registrationUpload": b64(&upload),
            "header": b64(b"PAKE-HEADER-MARKER"),
            "baseHeaderRevision": 0,
        }),
    )
    .await;
    assert_eq!(body["headerRevision"], 1, "{body}");

    // A failing OPAQUE sign-in: the server's half is refused at finish.
    let (_client, request) = havenkeys_opaque::ClientLogin::start(&pake_key).unwrap();
    let body = post_json(
        &server,
        None,
        "/v1/auth/pake/start",
        serde_json::json!({ "email": pake_email, "credentialRequest": b64(&request) }),
    )
    .await;
    let failed_login_id = body["loginId"].as_str().unwrap().to_string();
    opaque.push(("the failed credential request", b64(&request)));
    opaque.push((
        "the failed credential response",
        body["credentialResponse"].as_str().unwrap().to_string(),
    ));
    opaque.push(("the failed loginId", failed_login_id.clone()));
    let bogus = [5u8; 64];
    opaque.push(("the bogus finalization", b64(&bogus)));
    let res = server
        .post("/v1/auth/pake/finish")
        .json(&serde_json::json!({ "loginId": failed_login_id,
                                   "credentialFinalization": b64(&bogus),
                                   "deviceId": uuid::Uuid::new_v4(), "deviceName": "Phone" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    opaque.push(("the OPAQUE account's key", b64(&pake_key)));
    opaque.push(("the OPAQUE account's new key", b64(&pake_new_key)));
    opaque.push(("the OPAQUE email", pake_email.to_string()));
    opaque.push(("the OPAQUE invite", pake_invite.clone()));
    opaque.push(("the OPAQUE header", "PAKE-HEADER-MARKER".to_string()));

    let logs = String::from_utf8_lossy(&buffer.0.lock().unwrap().clone()).to_string();
    for (what, value) in &opaque {
        assert!(
            !logs.contains(value),
            "{what} appeared in the logs:\n{logs}"
        );
    }
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
        ("the signup email", signup_email.to_string()),
        ("the signup code", code.clone()),
        ("the signup invite", signup_invite.clone()),
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
