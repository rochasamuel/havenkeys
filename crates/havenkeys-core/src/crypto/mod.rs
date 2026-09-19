//! Composition of audited primitives. No primitive is implemented here.
//!
//! * `kdf`  — Argon2id password → master key
//! * `keys` — 256-bit key type and HKDF-based key separation
//! * `blob` — versioned AES-256-GCM encrypted blob format
//! * `secret_key` — the 128-bit Secret Key mixed into the KEK (key scheme 2)

pub mod blob;
pub mod kdf;
pub mod keys;
pub mod secret_key;

use crate::error::{Error, Result};
use rand::rngs::SysRng;
use rand::TryRng;

/// Fill `dest` from the operating system CSPRNG.
pub fn fill_random(dest: &mut [u8]) -> Result<()> {
    SysRng.try_fill_bytes(dest).map_err(|_| Error::Rng)
}
