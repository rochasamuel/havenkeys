mod support;

use havenkeys_server::pake::{self, Purpose};

#[tokio::test]
async fn the_server_setup_is_created_once_and_reloaded() {
    let server = support::TestServer::start().await;
    let a = pake::load_or_create(server.pool()).await.unwrap();
    let b = pake::load_or_create(server.pool()).await.unwrap();
    assert_eq!(a.to_bytes(), b.to_bytes());
    server.cleanup().await;
}

#[tokio::test]
async fn a_pending_exchange_is_read_once() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let id = pake::put_pending(&db, Purpose::Login, None, b"state")
        .await
        .unwrap();
    let first = pake::take_pending(&db, Purpose::Login, &id).await.unwrap();
    assert_eq!(
        first.map(|(a, s)| (a, s.to_vec())),
        Some((None, b"state".to_vec()))
    );
    assert!(pake::take_pending(&db, Purpose::Login, &id)
        .await
        .unwrap()
        .is_none());
    server.cleanup().await;
}

#[tokio::test]
async fn a_pending_exchange_is_bound_to_its_purpose_and_expires() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let id = pake::put_pending(&db, Purpose::Login, None, b"s")
        .await
        .unwrap();
    assert!(pake::take_pending(&db, Purpose::Reauth, &id)
        .await
        .unwrap()
        .is_none());
    let id = pake::put_pending(&db, Purpose::Login, None, b"s")
        .await
        .unwrap();
    db.execute(
        "UPDATE pake_pending SET expires_at = now() - interval '1 second'",
        &[],
    )
    .await
    .unwrap();
    assert!(pake::take_pending(&db, Purpose::Login, &id)
        .await
        .unwrap()
        .is_none());
    assert!(pake::take_pending(&db, Purpose::Login, "not-an-id")
        .await
        .unwrap()
        .is_none());
    server.cleanup().await;
}

#[tokio::test]
async fn the_table_holds_only_the_hash_of_the_id() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let id = pake::put_pending(&db, Purpose::Login, None, b"s")
        .await
        .unwrap();
    let stored: Vec<u8> = db
        .query_one("SELECT id_hash FROM pake_pending", &[])
        .await
        .unwrap()
        .get(0);
    assert_ne!(stored, id.as_bytes());
    assert_eq!(stored.len(), 32);
    server.cleanup().await;
}
