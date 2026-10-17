//! Changing the account's credentials: the master password, from the
//! server's point of view.
//!
//! A new master password means a new KDF salt, a new auth key and a new key
//! wrap. All three have to move together: a header without the verifier
//! locks every device out of the server, and a verifier without the header
//! leaves other devices unable to open the vault. So this is one route and
//! one transaction.
//!
//! The session alone is not enough to ask. A stolen token could otherwise
//! replace the password and lock the owner out, so the caller also proves it
//! knows the current auth key, which is checked and rate limited exactly
//! like a login. Every other session on the account is deleted: a password
//! is often changed because something leaked.
//!
//! The proof is either the legacy `currentAuthKey` or an OPAQUE exchange
//! started at `account/pake/reauth`; which one an account accepts depends on
//! its state (spec 2026-10-13-pake-login §4.4).

use crate::auth::{self, rate_limit, Session};
use crate::b64::Blob;
use crate::error::ApiError;
use crate::json::Json;
use crate::routes::accounts::{check_header, KdfDto};
use crate::routes::auth::client_ip;
use crate::routes::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use deadpool_postgres::GenericClient;
use serde::Deserialize;
use std::net::SocketAddr;
use uuid::Uuid;
use zeroize::Zeroizing;

const MALFORMED: ApiError = ApiError::InvalidRequest("message is not valid");

/// How a request proves the account's current key (spec §4.4).
enum Proof {
    Legacy(Zeroizing<Vec<u8>>),
    Pake {
        reauth_id: String,
        finalization: Vec<u8>,
    },
}

fn proof_from(
    current_auth_key: &Option<String>,
    reauth_id: &Option<String>,
    credential_finalization: &Option<String>,
) -> Result<Proof, ApiError> {
    match (current_auth_key, reauth_id, credential_finalization) {
        (Some(key), None, None) => Ok(Proof::Legacy(auth::decode_auth_key(key)?)),
        (None, Some(id), Some(f)) => Ok(Proof::Pake {
            reauth_id: id.clone(),
            finalization: crate::pake::decode_message(f)?,
        }),
        _ => Err(ApiError::InvalidRequest(
            "send currentAuthKey or a reauth proof",
        )),
    }
}

/// The stored credentials an optimistic lock compares.
#[derive(PartialEq)]
struct Stored {
    verifier: Option<String>,
    record: Option<Vec<u8>>,
    grace_until: Option<chrono::DateTime<chrono::Utc>>,
}

/// The account's credentials, `FOR UPDATE` when asked. Unauthorized when the
/// account is no longer active.
async fn load_stored(
    db: &impl GenericClient,
    account_id: Uuid,
    for_update: bool,
) -> Result<Stored, ApiError> {
    let sql = if for_update {
        "SELECT auth_verifier, opaque_record, legacy_grace_until FROM accounts
          WHERE id = $1 AND status = 'active' FOR UPDATE"
    } else {
        "SELECT auth_verifier, opaque_record, legacy_grace_until FROM accounts
          WHERE id = $1 AND status = 'active'"
    };
    let row = db
        .query_opt(sql, &[&account_id])
        .await?
        .ok_or(ApiError::Unauthorized)?;
    Ok(Stored {
        verifier: row.get(0),
        record: row.get(1),
        grace_until: row.get(2),
    })
}

/// Checks `proof` for the session's account against `stored` (spec §4.4):
///
/// * a legacy proof after the cutoff, or on an account that has both an
///   OPAQUE record and a verifier, is UpgradeRequired; on an account with
///   no verifier, or with the wrong key, Unauthorized;
/// * an OPAQUE proof consumes its reauth row whatever the outcome, and is
///   Unauthorized unless the row exists, is bound to this account and the
///   finalization verifies (an unenrolled account got a fake record, so it
///   never does).
async fn check_proof(
    state: &AppState,
    db: &impl GenericClient,
    account_id: Uuid,
    stored: &Stored,
    proof: Proof,
) -> Result<(), ApiError> {
    match proof {
        Proof::Legacy(key) => {
            crate::pake::legacy_allowed(state, stored.grace_until)?;
            let verifier = stored.verifier.clone().ok_or(ApiError::Unauthorized)?;
            if stored.record.is_some() {
                return Err(ApiError::UpgradeRequired);
            }
            if !auth::verify_auth_key(verifier, key).await? {
                return Err(ApiError::Unauthorized);
            }
            Ok(())
        }
        Proof::Pake {
            reauth_id,
            finalization,
        } => {
            let taken =
                crate::pake::take_pending(db, crate::pake::Purpose::Reauth, &reauth_id).await?;
            match taken {
                Some((Some(bound), pending))
                    if bound == account_id
                        && havenkeys_opaque::login_finish(&pending, &finalization).is_ok() =>
                {
                    Ok(())
                }
                _ => Err(ApiError::Unauthorized),
            }
        }
    }
}

/// The key a credential change installs.
enum NewKey {
    Legacy(Zeroizing<Vec<u8>>),
    /// The OPAQUE record built from a registration upload.
    Pake(Vec<u8>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialChange {
    #[serde(default)]
    current_auth_key: Option<String>,
    #[serde(default)]
    reauth_id: Option<String>,
    #[serde(default)]
    credential_finalization: Option<String>,
    kdf: KdfDto,
    #[serde(default)]
    new_auth_key: Option<String>,
    #[serde(default)]
    registration_upload: Option<String>,
    header: Blob,
    base_header_revision: i64,
}

pub async fn change_credentials(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<CredentialChange>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    session.require_full()?;
    check_header(&req.header)?;
    if req.base_header_revision < 0 {
        return Err(ApiError::InvalidRequest("baseHeaderRevision is not valid"));
    }
    let salt = req.kdf.check()?;
    let proof = proof_from(
        &req.current_auth_key,
        &req.reauth_id,
        &req.credential_finalization,
    )?;
    let new_key = match (&req.new_auth_key, &req.registration_upload, &proof) {
        (Some(key), None, Proof::Legacy(_)) => NewKey::Legacy(auth::decode_auth_key(key)?),
        (Some(_), None, Proof::Pake { .. }) => {
            return Err(ApiError::InvalidRequest("newAuthKey needs a legacy proof"))
        }
        (None, Some(upload), _) => NewKey::Pake(
            havenkeys_opaque::record_from_upload(&crate::pake::decode_message(upload)?)
                .map_err(|_| MALFORMED)?,
        ),
        _ => {
            return Err(ApiError::InvalidRequest(
                "send newAuthKey or registrationUpload",
            ))
        }
    };

    let mut db = state.pool.get().await?;
    let keys = rate_limit::AttemptKeys::new(session.account_id, &client_ip(&state, &headers, peer));
    keys.charge(&mut db).await?;

    // Verified before the transaction, like login: Argon2id takes tens of
    // milliseconds and no row lock should be held through it. The stored
    // credentials are compared again under the lock, so a concurrent change
    // still loses.
    let stored = load_stored(&db, session.account_id, false).await?;
    check_proof(&state, &db, session.account_id, &stored, proof)
        .await
        .inspect_err(|_| {
            tracing::info!(account_id = %session.account_id, outcome = "rejected", "credential change")
        })?;
    keys.clear(&db).await?;
    // A move to OPAQUE clears the legacy verifier (spec §4.4): a device
    // holding the old key could not sign in with it anyway.
    let (verifier, record) = match new_key {
        NewKey::Legacy(key) => (Some(auth::hash_auth_key(key).await?), None),
        NewKey::Pake(record) => (None, Some(record)),
    };

    let tx = db.transaction().await?;
    if load_stored(&tx, session.account_id, true).await? != stored {
        return Err(ApiError::Conflict);
    }
    let current_revision: i64 = tx
        .query_one(
            "SELECT header_revision FROM vaults WHERE id = $1 FOR UPDATE",
            &[&session.vault_id],
        )
        .await?
        .get(0);
    if req.base_header_revision != current_revision {
        return Err(ApiError::Conflict);
    }
    let next = current_revision + 1;
    tx.execute(
        "UPDATE accounts
            SET kdf_algorithm = 'argon2id', kdf_memory_kib = $2, kdf_iterations = $3,
                kdf_parallelism = $4, kdf_salt = $5, auth_verifier = $6, opaque_record = $7
          WHERE id = $1",
        &[
            &session.account_id,
            &req.kdf.memory_kib,
            &req.kdf.iterations,
            &req.kdf.parallelism,
            &salt,
            &verifier,
            &record,
        ],
    )
    .await?;
    tx.execute(
        "UPDATE vaults SET header = $2, header_revision = $3 WHERE id = $1",
        &[&session.vault_id, &req.header.0, &next],
    )
    .await?;
    tx.execute(
        "DELETE FROM sessions WHERE account_id = $1 AND token_hash <> $2",
        &[&session.account_id, &session.token_hash],
    )
    .await?;
    tx.commit().await?;

    tracing::info!(account_id = %session.account_id, header_revision = next, "credentials changed");
    Ok(axum::Json(serde_json::json!({ "headerRevision": next })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountDeletion {
    #[serde(default)]
    current_auth_key: Option<String>,
    #[serde(default)]
    reauth_id: Option<String>,
    #[serde(default)]
    credential_finalization: Option<String>,
    email: String,
}

/// Delete the caller's account and everything it owns (spec
/// 2026-10-05-account-deletion §4.3). The session alone is not enough,
/// exactly as for a credential change: the caller proves the current key,
/// checked and rate limited like a login. The typed email only guards
/// against deleting the wrong account by mistake.
pub async fn delete_account(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<AccountDeletion>,
) -> Result<axum::http::StatusCode, ApiError> {
    const MISMATCH: ApiError = ApiError::InvalidRequest("the email does not match this account");
    let typed = crate::email::normalize(&req.email).map_err(|_| MISMATCH)?;
    let proof = proof_from(
        &req.current_auth_key,
        &req.reauth_id,
        &req.credential_finalization,
    )?;

    let mut db = state.pool.get().await?;
    let keys = rate_limit::AttemptKeys::new(session.account_id, &client_ip(&state, &headers, peer));
    keys.charge(&mut db).await?;

    let email: String = db
        .query_opt(
            "SELECT email_normalized FROM accounts WHERE id = $1 AND status = 'active'",
            &[&session.account_id],
        )
        .await?
        .ok_or(ApiError::Unauthorized)?
        .get(0);
    if typed != email {
        return Err(MISMATCH);
    }
    // Verified before the transaction, like login: no row lock is held
    // through Argon2id. Compared again under the lock below.
    let stored = load_stored(&db, session.account_id, false).await?;
    check_proof(&state, &db, session.account_id, &stored, proof)
        .await
        .inspect_err(|_| {
            tracing::info!(account_id = %session.account_id, outcome = "rejected", "account deletion")
        })?;
    keys.clear(&db).await?;

    let tx = db.transaction().await?;
    if load_stored(&tx, session.account_id, true).await? != stored {
        return Err(ApiError::Conflict);
    }
    crate::erase::erase_account(&tx, session.account_id, &state.server_secret).await?;
    tx.commit().await?;

    tracing::info!(account_id = %session.account_id, "account deleted");
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PakeRegister {
    registration_request: String,
}

/// The server's half of an OPAQUE registration for the session's account.
/// Stateless: it stores nothing, and only `enroll` or `credentials` (each
/// with its own proof) turn the client's upload into the account's record.
pub async fn pake_register(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<PakeRegister>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    // Not gated on the plan: enrolment writes no vault data, and a frozen
    // account must still be able to enrol so it keeps sign-in and deletion.
    let request = crate::pake::decode_message(&req.registration_request)?;
    let response = state
        .pake
        .register(&request, session.account_id.as_bytes())
        .map_err(|_| MALFORMED)?;
    Ok(axum::Json(serde_json::json!({
        "registrationResponse": crate::pake::encode(&response),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PakeReauth {
    credential_request: String,
}

/// The first half of an OPAQUE proof inside a session (spec §4.3), bound to
/// the session's account and consumed by `credentials` or `delete`. Not
/// gated on the plan: a frozen account may still delete itself.
pub async fn pake_reauth(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<PakeReauth>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let request = crate::pake::decode_message(&req.credential_request)?;
    let mut db = state.pool.get().await?;
    let keys = rate_limit::AttemptKeys::new(session.account_id, &client_ip(&state, &headers, peer));
    keys.charge(&mut db).await?;
    let stored = load_stored(&db, session.account_id, false).await?;
    // No record yet: opaque-ke's fake response, so the proof cannot verify.
    let (pending, response) = state
        .pake
        .login_start(
            stored.record.as_deref(),
            &request,
            session.account_id.as_bytes(),
        )
        .map_err(|_| MALFORMED)?;
    let reauth_id = crate::pake::put_pending(
        &db,
        crate::pake::Purpose::Reauth,
        Some(session.account_id),
        &pending,
    )
    .await?;
    Ok(axum::Json(serde_json::json!({
        "reauthId": reauth_id,
        "credentialResponse": crate::pake::encode(&response),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PakeEnroll {
    current_auth_key: String,
    registration_upload: String,
}

/// An account made before OPAQUE gains its record (spec §4.4, §6). The
/// session alone is not enough: the current legacy key is checked, so a
/// stolen session cannot choose the account's OPAQUE key. The verifier is
/// kept, so old devices keep signing in until the cutoff; a grace from
/// `admin allow-legacy-sign-in` ends here.
pub async fn pake_enroll(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<PakeEnroll>,
) -> Result<axum::http::StatusCode, ApiError> {
    // Not gated on the plan, like `pake_register`: a frozen account that
    // could not enrol would lose sign-in and deletion at the cutoff.
    let key = auth::decode_auth_key(&req.current_auth_key)?;
    let record = havenkeys_opaque::record_from_upload(&crate::pake::decode_message(
        &req.registration_upload,
    )?)
    .map_err(|_| MALFORMED)?;
    let mut db = state.pool.get().await?;
    let keys = rate_limit::AttemptKeys::new(session.account_id, &client_ip(&state, &headers, peer));
    keys.charge(&mut db).await?;
    let stored = load_stored(&db, session.account_id, false).await?;
    // Before the proof, so an enrolled account answers 409, not 426.
    if stored.record.is_some() {
        return Err(ApiError::Conflict);
    }
    check_proof(&state, &db, session.account_id, &stored, Proof::Legacy(key))
        .await
        .inspect_err(|_| {
            tracing::info!(account_id = %session.account_id, outcome = "rejected", "pake enrol")
        })?;
    keys.clear(&db).await?;
    // Compared again under the lock, like credentials and delete: a legacy
    // change that committed during the Argon2 run rotated the verifier, and
    // a record built from the old key must not survive it.
    let tx = db.transaction().await?;
    if load_stored(&tx, session.account_id, true).await? != stored {
        return Err(ApiError::Conflict);
    }
    tx.execute(
        "UPDATE accounts SET opaque_record = $2, legacy_grace_until = NULL WHERE id = $1",
        &[&session.account_id, &record],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(account_id = %session.account_id, "account enrolled in OPAQUE");
    Ok(axum::http::StatusCode::NO_CONTENT)
}
