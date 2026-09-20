//! Authentication: what the auth key is, how it is stored, and how a request
//! proves which account it speaks for.
//!
//! The auth key is 256 bits of HKDF output derived on the client from the
//! master password and the Secret Key (docs/crypto.md, key scheme 3). The
//! server sees only that value, never the password, the Secret Key, the KEK
//! or the vault key, and it stores only an Argon2id hash of it.

pub mod rate_limit;

use crate::error::ApiError;
use crate::limits::SESSION_TTL_HOURS;
use crate::routes::AppState;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use chrono::{DateTime, Duration, Utc};
use data_encoding::{BASE64, BASE64URL_NOPAD};
use deadpool_postgres::Object;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Modest on purpose: the input is 256 bits of HKDF output, not a password,
/// so this hash protects a stolen database rather than slowing a guessing
/// attack. Heavy parameters here would only be a denial-of-service lever on
/// the login route (design §7.3).
const VERIFIER_MEMORY_KIB: u32 = 19 * 1024;
const VERIFIER_ITERATIONS: u32 = 2;
const VERIFIER_PARALLELISM: u32 = 1;

/// Exactly 32 bytes; anything else is not an auth key.
pub const AUTH_KEY_LEN: usize = 32;

fn hasher() -> Argon2<'static> {
    let params = Params::new(
        VERIFIER_MEMORY_KIB,
        VERIFIER_ITERATIONS,
        VERIFIER_PARALLELISM,
        None,
    )
    .expect("the verifier parameters are in range");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn decode_auth_key(raw: &str) -> Result<Zeroizing<Vec<u8>>, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("authKey is not valid");
    if raw.len() > 64 {
        return Err(BAD);
    }
    let bytes = Zeroizing::new(BASE64.decode(raw.as_bytes()).map_err(|_| BAD)?);
    if bytes.len() != AUTH_KEY_LEN {
        return Err(BAD);
    }
    Ok(bytes)
}

/// Hash an auth key for storage. Runs on a blocking thread: Argon2id holds a
/// core for tens of milliseconds, which would otherwise stall the runtime.
pub async fn hash_auth_key(auth_key: Zeroizing<Vec<u8>>) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        hasher()
            .hash_password(&auth_key, &salt)
            .map(|h| h.to_string())
            .map_err(|_| ApiError::Internal)
    })
    .await
    .map_err(|_| ApiError::Internal)?
}

/// Verify an auth key against a stored PHC string.
pub async fn verify_auth_key(stored: String, auth_key: Zeroizing<Vec<u8>>) -> bool {
    tokio::task::spawn_blocking(move || match PasswordHash::new(&stored) {
        Ok(parsed) => hasher().verify_password(&auth_key, &parsed).is_ok(),
        Err(_) => false,
    })
    .await
    .unwrap_or(false)
}

/// A verifier for an account that does not exist, so login spends the same
/// Argon2id time whether the email is known or not. Computed once.
pub fn dummy_verifier() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        hasher()
            .hash_password(&[0u8; AUTH_KEY_LEN], &salt)
            .expect("hashing a fixed value cannot fail")
            .to_string()
    })
}

/// A bearer token's SHA-256, which is all the database ever holds: a dump of
/// `sessions` cannot be replayed as a live session.
pub fn token_hash(raw: &str) -> Vec<u8> {
    Sha256::digest(raw.as_bytes()).to_vec()
}

/// Issue a session for a device. 32 opaque random bytes, valid for a day.
pub async fn issue_token(
    db: &Object,
    account_id: Uuid,
    device_id: Uuid,
) -> Result<(Zeroizing<String>, DateTime<Utc>), ApiError> {
    let mut raw = Zeroizing::new([0u8; 32]);
    rand::thread_rng().fill_bytes(raw.as_mut());
    let token = Zeroizing::new(BASE64URL_NOPAD.encode(raw.as_ref()));
    let expires = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
    db.execute(
        "INSERT INTO sessions (token_hash, account_id, device_id, created_at, expires_at)
         VALUES ($1, $2, $3, now(), $4)",
        &[&token_hash(&token), &account_id, &device_id, &expires],
    )
    .await?;
    Ok((token, expires))
}

/// Identity for an authenticated request.
///
/// Built only from the bearer token. No handler reads an account id, a vault
/// id or a device id from a request body — a body that carries one is an
/// unknown field, and therefore a 400 (design §7.3).
pub struct Session {
    pub account_id: Uuid,
    pub device_id: Uuid,
    pub vault_id: Uuid,
    pub token_hash: Vec<u8>,
}

impl FromRequestParts<AppState> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let raw = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        if raw.is_empty() || raw.len() > 128 {
            return Err(ApiError::Unauthorized);
        }
        let hash = token_hash(raw);
        let db = state.pool.get().await?;
        // One query is the whole authorization story: the token is live, the
        // account is active, the device is not revoked, and this is the vault
        // the request may touch.
        let row = db
            .query_opt(
                "SELECT s.account_id, s.device_id, v.id
                   FROM sessions s
                   JOIN accounts a ON a.id = s.account_id
                   JOIN devices  d ON d.id = s.device_id
                   JOIN vaults   v ON v.account_id = s.account_id
                  WHERE s.token_hash = $1
                    AND s.expires_at > now()
                    AND a.status = 'active'
                    AND d.revoked_at IS NULL",
                &[&hash],
            )
            .await?
            .ok_or(ApiError::Unauthorized)?;
        let device_id: Uuid = row.get(1);
        db.execute(
            "UPDATE devices SET last_seen_at = now() WHERE id = $1",
            &[&device_id],
        )
        .await?;
        Ok(Session {
            account_id: row.get(0),
            device_id,
            vault_id: row.get(2),
            token_hash: hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_auth_key_verifies_against_its_own_hash_and_nothing_else() {
        let key = Zeroizing::new(vec![3u8; AUTH_KEY_LEN]);
        let stored = hash_auth_key(key.clone()).await.unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(verify_auth_key(stored.clone(), key).await);
        assert!(!verify_auth_key(stored, Zeroizing::new(vec![4u8; AUTH_KEY_LEN])).await);
    }

    #[test]
    fn only_a_32_byte_auth_key_is_accepted() {
        assert!(decode_auth_key(&BASE64.encode(&[1u8; 32])).is_ok());
        assert!(decode_auth_key(&BASE64.encode(&[1u8; 31])).is_err());
        assert!(decode_auth_key(&BASE64.encode(&[1u8; 33])).is_err());
        assert!(decode_auth_key("not base64").is_err());
    }

    #[tokio::test]
    async fn the_dummy_verifier_never_accepts_a_real_key() {
        assert!(!verify_auth_key(dummy_verifier().to_string(), Zeroizing::new(vec![9u8; 32])).await);
    }
}
