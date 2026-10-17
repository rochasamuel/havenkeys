//! OPAQUE sign-in on the server (spec 2026-10-13-pake-login): the stored
//! setup, half-finished exchanges, and the legacy cutoff.

use crate::error::ApiError;
use crate::routes::AppState;
use data_encoding::{BASE64, BASE64URL_NOPAD};
use deadpool_postgres::{GenericClient, Pool};
use havenkeys_opaque::{ServerKeys, MAX_MESSAGE_BYTES};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

/// How long a started exchange may wait for its second message.
pub const PENDING_TTL_SECONDS: f64 = 60.0;

#[derive(Clone, Copy)]
pub enum Purpose {
    Login,
    Reauth,
}

impl Purpose {
    fn as_str(self) -> &'static str {
        match self {
            Purpose::Login => "login",
            Purpose::Reauth => "reauth",
        }
    }
}

/// The stored setup, or a new one if this database has none. Two servers
/// starting at once both insert; `ON CONFLICT DO NOTHING` keeps the first
/// and both read it back.
///
/// A missing setup while some account already has an OPAQUE record is an
/// error, not a fresh start: a new setup would make every record fail with
/// no way back, so the server refuses to start until the row is restored.
pub async fn load_or_create(pool: &Pool) -> Result<Arc<ServerKeys>, String> {
    let db = pool.get().await.map_err(|_| "no database".to_string())?;
    let fresh = ServerKeys::generate();
    let fresh_bytes = fresh.to_bytes();
    // One statement, so a concurrent start cannot slip between the check
    // and the insert: it inserts only when no setup row and no enrolled
    // account exist.
    db.execute(
        "INSERT INTO pake_server_setup (id, setup)
         SELECT 1, $1
         WHERE NOT EXISTS (SELECT 1 FROM accounts WHERE opaque_record IS NOT NULL)
         ON CONFLICT DO NOTHING",
        &[&fresh_bytes.as_slice()],
    )
    .await
    .map_err(|_| "could not store the OPAQUE setup".to_string())?;
    let row = db
        .query_opt("SELECT setup FROM pake_server_setup WHERE id = 1", &[])
        .await
        .map_err(|_| "could not read the OPAQUE setup".to_string())?;
    let Some(row) = row else {
        return Err(
            "the OPAQUE setup is missing but accounts are enrolled: restore pake_server_setup from the backup"
                .to_string(),
        );
    };
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(row.get(0));
    ServerKeys::from_bytes(&bytes)
        .map(Arc::new)
        .map_err(|_| "the stored OPAQUE setup is not valid".to_string())
}

/// Legacy (auth-key) paths stop at the operator's cutoff, except for an
/// account the operator gave a grace period (`admin allow-legacy-sign-in`)
/// so its owner can sign in once more and enrol. 426, never 410: old apps
/// read 410 as "account deleted" and wipe their copy.
pub fn legacy_allowed(
    state: &AppState,
    grace_until: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), ApiError> {
    let now = chrono::Utc::now();
    match state.legacy_auth_until {
        Some(until) if now >= until && !grace_until.is_some_and(|g| now < g) => {
            Err(ApiError::UpgradeRequired)
        }
        _ => Ok(()),
    }
}

pub fn decode_message(raw: &str) -> Result<Vec<u8>, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("message is not valid");
    if raw.is_empty() || raw.len() > MAX_MESSAGE_BYTES.div_ceil(3) * 4 {
        return Err(BAD);
    }
    BASE64.decode(raw.as_bytes()).map_err(|_| BAD)
}

pub fn encode(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

fn id_hash(id: &str) -> Vec<u8> {
    Sha256::digest(id.as_bytes()).to_vec()
}

pub async fn put_pending(
    db: &impl GenericClient,
    purpose: Purpose,
    account_id: Option<Uuid>,
    state: &[u8],
) -> Result<String, ApiError> {
    let mut raw = Zeroizing::new([0u8; 32]);
    rand::thread_rng().fill_bytes(raw.as_mut());
    let id = BASE64URL_NOPAD.encode(raw.as_ref());
    db.execute(
        "INSERT INTO pake_pending (id_hash, purpose, account_id, state, expires_at)
         VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))",
        &[
            &id_hash(&id),
            &purpose.as_str(),
            &account_id,
            &state,
            &PENDING_TTL_SECONDS,
        ],
    )
    .await?;
    Ok(id)
}

/// The pending exchange, removed in the same statement: a second read of
/// the same id finds nothing, whether or not the first one succeeded.
pub async fn take_pending(
    db: &impl GenericClient,
    purpose: Purpose,
    id: &str,
) -> Result<Option<(Option<Uuid>, Zeroizing<Vec<u8>>)>, ApiError> {
    if id.len() > 64 {
        return Ok(None);
    }
    let row = db
        .query_opt(
            "DELETE FROM pake_pending
              WHERE id_hash = $1 AND purpose = $2 AND expires_at > now()
          RETURNING account_id, state",
            &[&id_hash(id), &purpose.as_str()],
        )
        .await?;
    Ok(row.map(|r| {
        (
            r.get::<_, Option<Uuid>>(0),
            Zeroizing::new(r.get::<_, Vec<u8>>(1)),
        )
    }))
}
