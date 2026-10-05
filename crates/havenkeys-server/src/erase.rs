//! Erasing an account: everything the server holds about it, in the
//! caller's transaction (spec 2026-10-05-account-deletion §4.2).
//!
//! The in-app route and the admin CLI both come here, so a deletion asked
//! for by email leaves exactly what one made in the app leaves: nothing but
//! anonymous session-token hashes that expire in [`TOMBSTONE_DAYS`].

use deadpool_postgres::{Object, Transaction};
use uuid::Uuid;

/// How long the other devices of a deleted account can still learn of it.
pub const TOMBSTONE_DAYS: i32 = 30;

pub async fn erase_account(
    tx: &Transaction<'_>,
    account_id: Uuid,
) -> Result<(), tokio_postgres::Error> {
    tx.execute(
        "INSERT INTO deleted_sessions (token_hash, expires_at)
         SELECT token_hash, now() + make_interval(days => $2) FROM sessions
          WHERE account_id = $1
         ON CONFLICT (token_hash) DO NOTHING",
        &[&account_id, &TOMBSTONE_DAYS],
    )
    .await?;
    let devices: Vec<Uuid> = tx
        .query(
            "SELECT id FROM devices WHERE account_id = $1",
            &[&account_id],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect();
    tx.execute(
        "DELETE FROM login_attempts WHERE key = $1",
        &[&format!("acct:{account_id}")],
    )
    .await?;
    // A pending pairing has no account yet but carries a device id, a
    // device name and an IP.
    tx.execute(
        "DELETE FROM pairings WHERE account_id = $1 OR device_id = ANY($2)",
        &[&account_id, &devices],
    )
    .await?;
    // The cascade takes vaults, items, devices, sessions and linked pairings.
    tx.execute("DELETE FROM accounts WHERE id = $1", &[&account_id])
        .await?;
    Ok(())
}

/// Delete expired tombstones. Returns how many went.
pub async fn sweep_tombstones(db: &Object) -> Result<u64, tokio_postgres::Error> {
    db.execute(
        "DELETE FROM deleted_sessions WHERE expires_at <= now()",
        &[],
    )
    .await
}
