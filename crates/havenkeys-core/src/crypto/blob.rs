//! Versioned authenticated-encryption blob.
//!
//! Layout (v1):
//!
//! ```text
//! [0]      blob version (0x01)
//! [1]      algorithm    (0x01 = AES-256-GCM)
//! [2..14]  96-bit random nonce
//! [14..]   ciphertext || 128-bit GCM tag
//! ```
//!
//! Associated data binds each blob to its purpose, vault and item, plus the
//! header bytes above. See docs/crypto.md.

use crate::crypto::fill_random;
use crate::crypto::keys::Key256;
use crate::error::{Error, Result};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const BLOB_V1: u8 = 0x01;
pub const ALG_AES_256_GCM: u8 = 0x01;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;
pub const HEADER_LEN: usize = 2 + NONCE_LEN;
pub const MIN_BLOB_LEN: usize = HEADER_LEN + TAG_LEN;
/// Upper bound for a single blob. Secure notes are capped well below this.
pub const MAX_BLOB_LEN: usize = 8 * 1024 * 1024;

const AAD_PREFIX: &[u8] = b"havenkeys\0";

/// What a blob is used for. Part of the associated data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    VaultKey,
    ItemOverview,
    ItemDetails,
    Settings,
    /// Proof that an account's header was written by a vault-key holder.
    SyncHeader,
}

impl Purpose {
    fn label(self) -> &'static [u8] {
        match self {
            Purpose::VaultKey => b"vault-key",
            Purpose::ItemOverview => b"item-overview",
            Purpose::ItemDetails => b"item-details",
            Purpose::Settings => b"settings",
            Purpose::SyncHeader => b"sync-header",
        }
    }

    fn needs_item_id(self) -> bool {
        matches!(self, Purpose::ItemOverview | Purpose::ItemDetails)
    }
}

/// Context that every blob is cryptographically bound to.
#[derive(Clone, Copy, Debug)]
pub struct BlobContext {
    pub purpose: Purpose,
    pub vault_id: Uuid,
    pub item_id: Option<Uuid>,
}

impl BlobContext {
    pub fn vault(purpose: Purpose, vault_id: Uuid) -> Self {
        Self {
            purpose,
            vault_id,
            item_id: None,
        }
    }

    pub fn item(purpose: Purpose, vault_id: Uuid, item_id: Uuid) -> Self {
        Self {
            purpose,
            vault_id,
            item_id: Some(item_id),
        }
    }

    fn aad(&self, version: u8, algorithm: u8) -> Result<Vec<u8>> {
        if self.purpose.needs_item_id() != self.item_id.is_some() {
            return Err(Error::InvalidInput("blob context"));
        }
        let mut aad = Vec::with_capacity(64);
        aad.extend_from_slice(AAD_PREFIX);
        aad.push(version);
        aad.push(algorithm);
        aad.extend_from_slice(self.purpose.label());
        aad.push(0);
        aad.extend_from_slice(self.vault_id.as_bytes());
        if let Some(item_id) = self.item_id {
            aad.extend_from_slice(item_id.as_bytes());
        }
        Ok(aad)
    }
}

/// Borrowed view of a structurally valid blob. Parsing does not authenticate.
#[derive(Debug)]
pub struct ParsedBlob<'a> {
    pub version: u8,
    pub algorithm: u8,
    pub nonce: &'a [u8; NONCE_LEN],
    pub ciphertext_and_tag: &'a [u8],
}

/// Structural parse. Rejects unknown versions/algorithms and bad lengths.
pub fn parse(blob: &[u8]) -> Result<ParsedBlob<'_>> {
    if blob.len() < MIN_BLOB_LEN || blob.len() > MAX_BLOB_LEN {
        return Err(Error::Corrupted);
    }
    let version = blob[0];
    let algorithm = blob[1];
    if version != BLOB_V1 {
        return Err(Error::UnsupportedVersion);
    }
    if algorithm != ALG_AES_256_GCM {
        return Err(Error::UnsupportedVersion);
    }
    let nonce: &[u8; NONCE_LEN] = blob[2..HEADER_LEN]
        .try_into()
        .map_err(|_| Error::Corrupted)?;
    Ok(ParsedBlob {
        version,
        algorithm,
        nonce,
        ciphertext_and_tag: &blob[HEADER_LEN..],
    })
}

/// Encrypt `plaintext` under `key` with a fresh random nonce.
pub fn seal(key: &Key256, ctx: &BlobContext, plaintext: &[u8]) -> Result<Vec<u8>> {
    if plaintext.len() > MAX_BLOB_LEN - MIN_BLOB_LEN {
        return Err(Error::InvalidInput("item too large"));
    }
    let aad = ctx.aad(BLOB_V1, ALG_AES_256_GCM)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    fill_random(&mut nonce_bytes)?;

    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).map_err(|_| Error::Encryption)?;
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::Encryption)?;

    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.push(BLOB_V1);
    out.push(ALG_AES_256_GCM);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Authenticate and decrypt. Any failure returns an error and no plaintext.
pub fn open(key: &Key256, ctx: &BlobContext, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let parsed = parse(blob)?;
    let aad = ctx.aad(parsed.version, parsed.algorithm)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).map_err(|_| Error::Decryption)?;
    let nonce = Nonce::from(*parsed.nonce);
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: parsed.ciphertext_and_tag,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Decryption)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn key() -> Key256 {
        Key256::random().unwrap()
    }

    fn ctx_item(item: u128) -> BlobContext {
        BlobContext::item(
            Purpose::ItemDetails,
            Uuid::from_u128(1),
            Uuid::from_u128(item),
        )
    }

    #[test]
    fn round_trip() {
        let k = key();
        let ctx = ctx_item(5);
        let blob = seal(&k, &ctx, b"secret payload").unwrap();
        assert_eq!(blob.len(), HEADER_LEN + 14 + TAG_LEN);
        assert_eq!(&open(&k, &ctx, &blob).unwrap()[..], b"secret payload");
    }

    #[test]
    fn empty_plaintext_round_trip() {
        let k = key();
        let ctx = ctx_item(5);
        let blob = seal(&k, &ctx, b"").unwrap();
        assert!(open(&k, &ctx, &blob).unwrap().is_empty());
    }

    #[test]
    fn ciphertext_does_not_contain_plaintext() {
        let k = key();
        let blob = seal(&k, &ctx_item(1), b"hunter2hunter2").unwrap();
        assert!(!blob.windows(7).any(|w| w == b"hunter2"));
    }

    #[test]
    fn wrong_key_fails() {
        let blob = seal(&key(), &ctx_item(1), b"x").unwrap();
        assert_eq!(open(&key(), &ctx_item(1), &blob), Err(Error::Decryption));
    }

    #[test]
    fn every_bit_flip_is_detected() {
        let k = key();
        let ctx = ctx_item(1);
        let blob = seal(&k, &ctx, b"tamper me").unwrap();
        for byte in 0..blob.len() {
            for bit in 0..8 {
                let mut t = blob.clone();
                t[byte] ^= 1 << bit;
                assert!(open(&k, &ctx, &t).is_err(), "flip at {byte}:{bit} accepted");
            }
        }
    }

    #[test]
    fn truncation_and_extension_fail() {
        let k = key();
        let ctx = ctx_item(1);
        let blob = seal(&k, &ctx, b"payload").unwrap();
        for len in 0..blob.len() {
            assert!(open(&k, &ctx, &blob[..len]).is_err());
        }
        let mut longer = blob.clone();
        longer.push(0);
        assert!(open(&k, &ctx, &longer).is_err());
    }

    #[test]
    fn context_binding() {
        let k = key();
        let vault = Uuid::from_u128(1);
        let item = Uuid::from_u128(2);
        let blob = seal(
            &k,
            &BlobContext::item(Purpose::ItemDetails, vault, item),
            b"x",
        )
        .unwrap();

        // Other item, other role, other vault: all rejected.
        let other_item = BlobContext::item(Purpose::ItemDetails, vault, Uuid::from_u128(3));
        let other_role = BlobContext::item(Purpose::ItemOverview, vault, item);
        let other_vault = BlobContext::item(Purpose::ItemDetails, Uuid::from_u128(9), item);
        for ctx in [other_item, other_role, other_vault] {
            assert_eq!(open(&k, &ctx, &blob), Err(Error::Decryption));
        }
    }

    #[test]
    fn context_shape_is_enforced() {
        let k = key();
        let bad = BlobContext {
            purpose: Purpose::ItemDetails,
            vault_id: Uuid::nil(),
            item_id: None,
        };
        assert!(seal(&k, &bad, b"x").is_err());
        let bad = BlobContext {
            purpose: Purpose::Settings,
            vault_id: Uuid::nil(),
            item_id: Some(Uuid::nil()),
        };
        assert!(seal(&k, &bad, b"x").is_err());
    }

    #[test]
    fn unknown_version_or_algorithm_rejected() {
        let k = key();
        let ctx = ctx_item(1);
        let mut blob = seal(&k, &ctx, b"x").unwrap();
        blob[0] = 2;
        assert_eq!(open(&k, &ctx, &blob), Err(Error::UnsupportedVersion));
        blob[0] = BLOB_V1;
        blob[1] = 7;
        assert_eq!(open(&k, &ctx, &blob), Err(Error::UnsupportedVersion));
    }

    #[test]
    fn nonces_are_unique() {
        let k = key();
        let ctx = ctx_item(1);
        let mut seen = HashSet::new();
        for _ in 0..20_000 {
            let blob = seal(&k, &ctx, b"same plaintext").unwrap();
            assert!(seen.insert(blob[2..HEADER_LEN].to_vec()), "nonce reused");
        }
    }

    #[test]
    fn parser_never_panics_on_garbage() {
        // Cheap deterministic fuzz: every length/first-bytes combination up to 64 bytes.
        let k = key();
        let ctx = ctx_item(1);
        for len in 0..64usize {
            for first in [0u8, 1, 2, 0xFF] {
                let mut data = vec![0xA5u8; len];
                if len > 0 {
                    data[0] = first;
                }
                if len > 1 {
                    data[1] = 1;
                }
                let _ = parse(&data);
                assert!(open(&k, &ctx, &data).is_err());
            }
        }
    }

    #[test]
    fn oversized_blob_rejected_before_crypto() {
        let big = vec![BLOB_V1; MAX_BLOB_LEN + 1];
        assert_eq!(parse(&big).err(), Some(Error::Corrupted));
    }
}
