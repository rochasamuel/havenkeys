mod support;

use serde_json::Value;
use uuid::Uuid;

#[tokio::test]
async fn the_device_list_marks_the_calling_device() {
    let server = support::TestServer::start().await;
    let (account, desktop) = support::signed_in(&server, "user@example.com").await;
    let laptop = support::login(&server, &account, "Laptop").await;

    let devices: Value = server
        .get_as("/v1/devices", &desktop)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let devices = devices.as_array().unwrap();
    assert_eq!(devices.len(), 2);
    let current: Vec<&Value> = devices.iter().filter(|d| d["current"] == true).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0]["id"], serde_json::json!(desktop.device_id));
    assert!(devices.iter().any(|d| d["name"] == "Laptop"));
    assert!(devices
        .iter()
        .all(|d| d["lastSeenAt"].is_string() && d["createdAt"].is_string()));
    let _ = laptop;
    server.cleanup().await;
}

#[tokio::test]
async fn a_revoked_device_loses_its_session_and_cannot_log_back_in() {
    let server = support::TestServer::start().await;
    let (account, desktop) = support::signed_in(&server, "user@example.com").await;
    let laptop = support::login(&server, &account, "Laptop").await;

    assert_eq!(
        server
            .delete_as(&format!("/v1/devices/{}", laptop.device_id), &desktop)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );

    assert_eq!(
        server.get_as("/v1/devices", &laptop).send().await.unwrap().status(),
        401,
        "revocation bites on the next request, not at expiry"
    );

    let res = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&account.auth_key),
            "deviceId": laptop.device_id,
            "deviceName": "Laptop",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "a revoked device id cannot sign in again");

    // The desktop is unaffected, and the list no longer shows the laptop.
    let devices: Value = server
        .get_as("/v1/devices", &desktop)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(devices.as_array().unwrap().len(), 1);
    server.cleanup().await;
}

#[tokio::test]
async fn a_device_that_is_not_yours_cannot_be_revoked() {
    let server = support::TestServer::start().await;
    let (_, mine) = support::signed_in(&server, "a@example.com").await;
    let (_, theirs) = support::signed_in(&server, "b@example.com").await;

    assert_eq!(
        server
            .delete_as(&format!("/v1/devices/{}", theirs.device_id), &mine)
            .send()
            .await
            .unwrap()
            .status(),
        404,
        "someone else's device answers exactly like one that does not exist"
    );
    assert_eq!(
        server
            .delete_as(&format!("/v1/devices/{}", Uuid::new_v4()), &mine)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        server.get_as("/v1/devices", &theirs).send().await.unwrap().status(),
        200,
        "their session is untouched"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn revoking_your_own_device_ends_your_session() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "user@example.com").await;
    assert_eq!(
        server
            .delete_as(&format!("/v1/devices/{}", sess.device_id), &sess)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        server.get_as("/v1/devices", &sess).send().await.unwrap().status(),
        401
    );
    server.cleanup().await;
}

#[tokio::test]
async fn an_account_cannot_exceed_its_device_ceiling() {
    let server = support::TestServer::start().await;
    let (account, first) = support::signed_in(&server, "user@example.com").await;

    // 64 devices is the limit; one is already signed in.
    let db = server.db().await;
    for i in 0..63 {
        db.execute(
            "INSERT INTO devices (id, account_id, name, created_at) VALUES ($1, $2, $3, now())",
            &[&Uuid::new_v4(), &account.account_id, &format!("filler-{i}")],
        )
        .await
        .unwrap();
    }
    drop(db);

    let res = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&account.auth_key),
            "deviceId": Uuid::new_v4(),
            "deviceName": "one too many",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // Revoking one makes room again.
    server
        .delete_as(&format!("/v1/devices/{}", first.device_id), &first)
        .send()
        .await
        .unwrap();
    let res = server
        .post("/v1/auth/login")
        .json(&serde_json::json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&account.auth_key),
            "deviceId": Uuid::new_v4(),
            "deviceName": "replacement",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    server.cleanup().await;
}
