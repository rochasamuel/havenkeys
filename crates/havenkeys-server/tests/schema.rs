mod support;

/// The constraints asserted here are what every later isolation test leans
/// on, so they are checked directly rather than inferred from behaviour.
#[tokio::test]
async fn schema_enforces_one_vault_per_account_and_cascades() {
    let server = support::TestServer::start().await;
    let db = server.db().await;

    let account = uuid::Uuid::new_v4();
    db.execute(
        "INSERT INTO accounts (id, email_normalized, status, created_at)
         VALUES ($1, $2, 'invited', now())",
        &[&account, &"a@example.com"],
    )
    .await
    .unwrap();

    let vault = uuid::Uuid::new_v4();
    let blob: Vec<u8> = vec![1, 2, 3];
    db.execute(
        "INSERT INTO vaults (id, account_id, header, header_revision, key_scheme, revision, created_at)
         VALUES ($1, $2, $3, 0, 3, 0, now())",
        &[&vault, &account, &blob],
    )
    .await
    .unwrap();

    let second = db
        .execute(
            "INSERT INTO vaults (id, account_id, header, header_revision, key_scheme, revision, created_at)
             VALUES ($1, $2, $3, 0, 3, 0, now())",
            &[&uuid::Uuid::new_v4(), &account, &blob],
        )
        .await;
    assert!(second.is_err(), "an account owns at most one vault");

    db.execute(
        "INSERT INTO items (vault_id, item_id, overview, details, revision)
         VALUES ($1, $2, $3, $4, 1)",
        &[&vault, &uuid::Uuid::new_v4(), &blob, &blob],
    )
    .await
    .unwrap();

    db.execute("DELETE FROM accounts WHERE id = $1", &[&account])
        .await
        .unwrap();
    let items: i64 = db
        .query_one("SELECT count(*) FROM items", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(items, 0, "deleting an account removes its vault and items");

    drop(db);
    server.cleanup().await;
}

/// An unknown status must be impossible: routes branch on it.
#[tokio::test]
async fn account_status_is_constrained() {
    let server = support::TestServer::start().await;
    let db = server.db().await;
    let bad = db
        .execute(
            "INSERT INTO accounts (id, email_normalized, status, created_at)
             VALUES ($1, $2, 'whatever', now())",
            &[&uuid::Uuid::new_v4(), &"b@example.com"],
        )
        .await;
    assert!(bad.is_err());
    drop(db);
    server.cleanup().await;
}

/// Running the migrator twice must be a no-op, because it runs on every boot.
#[tokio::test]
async fn migrations_are_idempotent() {
    let server = support::TestServer::start().await;
    havenkeys_server::db::migrate(server.pool()).await.unwrap();
    let applied: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM schema_migrations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(applied, 1);
    server.cleanup().await;
}
