//! Authentication: what the auth key is, how it is stored, and how a request
//! proves which account it speaks for.
//!
//! The auth key is 256 bits of HKDF output derived on the client from the
//! master password and the Secret Key (docs/crypto.md, key scheme 3). The
//! server sees only that value, never the password, the Secret Key, the KEK
//! or the vault key, and it stores only an Argon2id hash of it.

pub mod rate_limit;

use crate::error::ApiError;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use data_encoding::BASE64;
use rand::rngs::OsRng;
use std::sync::OnceLock;
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
