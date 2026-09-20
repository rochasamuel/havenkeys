//! Activation: the only way a vault comes into existence.
//!
//! The client does all the cryptography — it generates the Secret Key and the
//! vault key, derives the KEK and the auth key, and seals the header — and
//! this route records the result. The server learns the account's KDF
//! parameters (which are public by design: a second device needs them before
//! it can derive anything) and an Argon2id hash of the auth key.

use crate::auth;
use crate::b64::Blob;
use crate::error::ApiError;
use crate::invite;
use crate::json::Json;
use crate::limits::MAX_HEADER_BYTES;
use crate::routes::AppState;
use axum::extract::State;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Accepted Argon2id cost, matching `havenkeys-core::crypto::kdf`. The client
/// would refuse anything outside this range when it reads the header back, so
/// refusing it here fails fast and keeps the stored row sane.
const MIN_MEMORY_KIB: i32 = 19 * 1024;
const MAX_MEMORY_KIB: i32 = 1024 * 1024;
const MIN_ITERATIONS: i32 = 2;
const MAX_ITERATIONS: i32 = 16;
const MIN_PARALLELISM: i32 = 1;
const MAX_PARALLELISM: i32 = 16;
const SALT_LEN: usize = 16;

/// The only key scheme this server serves (design §4).
pub const KEY_SCHEME: i16 = 3;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateRequest {
    email: String,
    invite: String,
    kdf: KdfDto,
    auth_key: String,
    vault_id: Uuid,
    header: Blob,
    key_scheme: i16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KdfDto {
    algorithm: String,
    memory_kib: i32,
    iterations: i32,
    parallelism: i32,
    salt: String,
}

impl KdfDto {
    fn check(&self) -> Result<Vec<u8>, ApiError> {
        const BAD: ApiError = ApiError::InvalidRequest("kdf parameters are not valid");
        if self.algorithm != "argon2id" {
            return Err(BAD);
        }
        let in_range = (MIN_MEMORY_KIB..=MAX_MEMORY_KIB).contains(&self.memory_kib)
            && (MIN_ITERATIONS..=MAX_ITERATIONS).contains(&self.iterations)
            && (MIN_PARALLELISM..=MAX_PARALLELISM).contains(&self.parallelism);
        if !in_range {
            return Err(BAD);
        }
        let salt = data_encoding::BASE64
            .decode(self.salt.as_bytes())
            .map_err(|_| BAD)?;
        if salt.len() != SALT_LEN {
            return Err(BAD);
        }
        Ok(salt)
    }
}

pub async fn activate(
    State(state): State<AppState>,
    Json(req): Json<ActivateRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    // Every failure below answers the same way. A prober with a guessed
    // invite must not learn whether the email exists, whether the account was
    // already activated, or whether only the secret was wrong.
    const BAD_INVITE: ApiError = ApiError::InvalidRequest("invite is not valid");

    let email = crate::email::normalize(&req.email).map_err(ApiError::InvalidRequest)?;
    let parsed = invite::decode(&req.invite)?;
    if parsed.email != email {
        return Err(BAD_INVITE);
    }
    if req.key_scheme != KEY_SCHEME {
        return Err(ApiError::InvalidRequest("unsupported key scheme"));
    }
    if req.header.is_empty() || req.header.len() > MAX_HEADER_BYTES {
        return Err(ApiError::InvalidRequest("header is not valid"));
    }
    let salt = req.kdf.check()?;
    let auth_key = auth::decode_auth_key(&req.auth_key)?;

    // Hashed before the transaction opens: Argon2id takes tens of
    // milliseconds and nothing is gained by holding a row lock through it.
    let verifier = auth::hash_auth_key(auth_key).await?;

    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT status, invite_hash, invite_expires_at
               FROM accounts
              WHERE id = $1 AND email_normalized = $2
                FOR UPDATE",
            &[&parsed.account, &email],
        )
        .await?
        .ok_or(BAD_INVITE)?;

    let status: String = row.get(0);
    let stored_hash: Option<Vec<u8>> = row.get(1);
    let expires: Option<chrono::DateTime<chrono::Utc>> = row.get(2);
    if status != "invited" {
        return Err(BAD_INVITE);
    }
    let stored_hash = stored_hash.ok_or(BAD_INVITE)?;
    let offered = invite::hash(&parsed.secret);
    if stored_hash.len() != offered.len() || stored_hash.ct_eq(offered.as_slice()).unwrap_u8() != 1
    {
        return Err(BAD_INVITE);
    }
    if expires.map(|t| t < chrono::Utc::now()).unwrap_or(true) {
        return Err(BAD_INVITE);
    }

    tx.execute(
        "UPDATE accounts
            SET status = 'active',
                kdf_algorithm = 'argon2id',
                kdf_memory_kib = $2,
                kdf_iterations = $3,
                kdf_parallelism = $4,
                kdf_salt = $5,
                auth_verifier = $6,
                invite_hash = NULL,
                invite_expires_at = NULL,
                activated_at = now()
          WHERE id = $1",
        &[
            &parsed.account,
            &req.kdf.memory_kib,
            &req.kdf.iterations,
            &req.kdf.parallelism,
            &salt,
            &verifier,
        ],
    )
    .await?;

    let inserted = tx
        .execute(
            "INSERT INTO vaults
               (id, account_id, header, header_revision, key_scheme, revision, created_at)
             VALUES ($1, $2, $3, 0, $4, 0, now())
             ON CONFLICT DO NOTHING",
            &[&req.vault_id, &parsed.account, &req.header.0, &KEY_SCHEME],
        )
        .await?;
    if inserted != 1 {
        // A vault id already in use. With v4 UUIDs this is not an accident,
        // and the activation must not silently adopt someone else's row.
        return Err(ApiError::InvalidRequest("vaultId is not available"));
    }

    tx.commit().await?;
    tracing::info!(account_id = %parsed.account, "account activated");
    Ok(axum::Json(serde_json::json!({
        "accountId": parsed.account,
        "vaultId": req.vault_id,
    })))
}
