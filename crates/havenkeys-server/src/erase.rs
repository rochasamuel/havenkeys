//! Erasing an account: everything the server holds about it, in the
//! caller's transaction (spec 2026-10-05-account-deletion §4.2).
//!
//! The in-app route and the admin CLI both come here, so a deletion asked
//! for by email leaves exactly what one made in the app leaves: nothing but
//! anonymous session-token and device-id hashes that expire in
//! [`TOMBSTONE_DAYS`].

use deadpool_postgres::{GenericClient, Object, Transaction};
use sha2::{Digest, Sha256};
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
    for device in &devices {
        tx.execute(
            "INSERT INTO deleted_devices (device_hash, expires_at)
             VALUES ($1, now() + make_interval(days => $2))
             ON CONFLICT (device_hash) DO NOTHING",
            &[&device_hash(*device), &TOMBSTONE_DAYS],
        )
        .await?;
    }
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

/// The stored form of a device id: a device id is random, and only its
/// holder (and the account it belonged to) ever saw it.
fn device_hash(device: Uuid) -> Vec<u8> {
    Sha256::digest(device.as_bytes()).to_vec()
}

/// Was this device part of an account deleted in the last
/// [`TOMBSTONE_DAYS`]? Asked when its sign-in fails.
pub async fn was_deleted_device(
    db: &impl GenericClient,
    device: Uuid,
) -> Result<bool, tokio_postgres::Error> {
    Ok(db
        .query_opt(
            "SELECT 1 FROM deleted_devices WHERE device_hash = $1 AND expires_at > now()",
            &[&device_hash(device)],
        )
        .await?
        .is_some())
}

/// Delete expired tombstones. Returns how many went.
pub async fn sweep_tombstones(db: &Object) -> Result<u64, tokio_postgres::Error> {
    let sessions = db
        .execute(
            "DELETE FROM deleted_sessions WHERE expires_at <= now()",
            &[],
        )
        .await?;
    let devices = db
        .execute("DELETE FROM deleted_devices WHERE expires_at <= now()", &[])
        .await?;
    Ok(sessions + devices)
}
