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
//! Associated data binds each blob to its purpose, vault and item, a details
//! blob to its overview blob, plus the header bytes above. See docs/crypto.md.

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

/// Upper bound for an encrypted backup (`Purpose::Backup`) — the whole
/// vault in one blob, so larger than any item's.
pub const MAX_BACKUP_BLOB_LEN: usize = 64 * 1024 * 1024;

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
    /// The Android app's own settings (never synced).
    DeviceSettings,
    /// The Digital Asset Links cache (never synced).
    AssetLinks,
    /// This device's item uses and recent searches (never synced).
    Activity,
    /// An encrypted export file (spec 2026-10-05-export).
    Backup,
}

impl Purpose {
    fn label(self) -> &'static [u8] {
        match self {
            Purpose::VaultKey => b"vault-key",
            Purpose::ItemOverview => b"item-overview",
            Purpose::ItemDetails => b"item-details",
            Purpose::Settings => b"settings",
            Purpose::SyncHeader => b"sync-header",
            Purpose::DeviceSettings => b"device-settings",
            Purpose::AssetLinks => b"asset-links",
            Purpose::Activity => b"activity",
            Purpose::Backup => b"backup",
        }
    }

    fn max_len(self) -> usize {
        match self {
            Purpose::Backup => MAX_BACKUP_BLOB_LEN,
            _ => MAX_BLOB_LEN,
        }
    }

    /// Purposes whose context carries a hash of another value.
    fn needs_binding(self) -> bool {
        matches!(self, Purpose::ItemDetails | Purpose::Backup)
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
    /// Details only: SHA-256 of the overview blob written with them, so the
    /// two halves of one item version cannot be paired with another
    /// version's (security-review CR1).
    pub bound_to: Option<[u8; 32]>,
}

impl BlobContext {
    pub fn vault(purpose: Purpose, vault_id: Uuid) -> Self {
        Self {
            purpose,
            vault_id,
            item_id: None,
            bound_to: None,
        }
    }

    pub fn item(purpose: Purpose, vault_id: Uuid, item_id: Uuid) -> Self {
        Self {
            purpose,
            vault_id,
            item_id: Some(item_id),
            bound_to: None,
        }
    }

    /// An item's details, bound to the overview blob they were written with.
    pub fn item_details(vault_id: Uuid, item_id: Uuid, overview_blob: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        Self {
            bound_to: Some(Sha256::digest(overview_blob).into()),
            ..Self::item(Purpose::ItemDetails, vault_id, item_id)
        }
    }

    /// An encrypted backup, bound to the file header written before it (KDF
    /// parameters, salt, version), so none of them can be changed.
    pub fn backup(header: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        Self {
            bound_to: Some(Sha256::digest(header).into()),
            ..Self::vault(Purpose::Backup, Uuid::nil())
        }
    }

    fn aad(&self, version: u8, algorithm: u8) -> Result<Vec<u8>> {
        if self.purpose.needs_item_id() != self.item_id.is_some()
            || self.purpose.needs_binding() != self.bound_to.is_some()
        {
            return Err(Error::InvalidInput("blob context"));
        }
        let mut aad = Vec::with_capacity(96);
        aad.extend_from_slice(AAD_PREFIX);
        aad.push(version);
        aad.push(algorithm);
        aad.extend_from_slice(self.purpose.label());
        aad.push(0);
        aad.extend_from_slice(self.vault_id.as_bytes());
        if let Some(item_id) = self.item_id {
            aad.extend_from_slice(item_id.as_bytes());
        }
        if let Some(bound) = self.bound_to {
            aad.extend_from_slice(if self.purpose == Purpose::Backup {
                b"header\0"
            } else {
                b"overview\0"
            });
            aad.extend_from_slice(&bound);
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
    parse_capped(blob, MAX_BLOB_LEN)
}

fn parse_capped(blob: &[u8], max: usize) -> Result<ParsedBlob<'_>> {
    if blob.len() < MIN_BLOB_LEN || blob.len() > max {
        return Err(Error::Corrupted);
    }
    let version = blob[0];
    let algorithm = blob[1];
    if version != BLOB_V1 || algorithm != ALG_AES_256_GCM {
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
    if plaintext.len() > ctx.purpose.max_len() - MIN_BLOB_LEN {
        return Err(Error::InvalidInput("item too large"));
    }
    let aad = ctx.aad(BLOB_V1, ALG_AES_256_GCM)?;
    seal_with_aad(key, &aad, plaintext)
}

fn seal_with_aad(key: &Key256, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    fill_random(&mut nonce_bytes)?;

    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).map_err(|_| Error::Encryption)?;
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
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
    let parsed = parse_capped(blob, ctx.purpose.max_len())?;
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
        BlobContext::item_details(Uuid::from_u128(1), Uuid::from_u128(item), b"overview")
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
        let blob = seal(&k, &BlobContext::item_details(vault, item, b"ov"), b"x").unwrap();

        // Other item, other role, other vault: all rejected.
        let other_item = BlobContext::item_details(vault, Uuid::from_u128(3), b"ov");
        let other_role = BlobContext::item(Purpose::ItemOverview, vault, item);
        let other_vault = BlobContext::item_details(Uuid::from_u128(9), item, b"ov");
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
            bound_to: Some([0; 32]),
        };
        assert!(seal(&k, &bad, b"x").is_err());
        let bad = BlobContext {
            purpose: Purpose::Settings,
            vault_id: Uuid::nil(),
            item_id: Some(Uuid::nil()),
            bound_to: None,
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

    #[test]
    fn details_open_only_with_the_overview_they_were_written_with() {
        let (vault, item) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let k = key();
        let ctx = BlobContext::item_details(vault, item, b"overview-v2");
        let blob = seal(&k, &ctx, b"details-v2").unwrap();
        assert_eq!(open(&k, &ctx, &blob).unwrap().as_slice(), b"details-v2");
        let other = BlobContext::item_details(vault, item, b"overview-v1");
        assert!(open(&k, &other, &blob).is_err());
    }

    #[test]
    fn an_unbound_details_blob_does_not_open() {
        let (vault, item) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let k = key();
        let unbound = BlobContext::item(Purpose::ItemDetails, vault, item);
        assert!(
            seal(&k, &unbound, b"x").is_err(),
            "details must be bound to an overview"
        );
        // A details blob sealed the pre-CR1 way (vault and item in the AAD,
        // no overview binding) does not open under the new context.
        let mut old_aad = BlobContext {
            bound_to: Some([0; 32]),
            ..unbound
        }
        .aad(BLOB_V1, ALG_AES_256_GCM)
        .unwrap();
        old_aad.truncate(old_aad.len() - b"overview\0".len() - 32);
        let old = seal_with_aad(&k, &old_aad, b"x").unwrap();
        assert!(open(&k, &BlobContext::item_details(vault, item, b"ov"), &old).is_err());
    }

    #[test]
    fn backup_blobs_are_bound_to_their_header_and_may_exceed_8_mb() {
        let k = key();
        let big = vec![0x42u8; MAX_BLOB_LEN + 1024];
        let sealed = seal(&k, &BlobContext::backup(b"header-a"), &big).unwrap();
        assert_eq!(
            open(&k, &BlobContext::backup(b"header-a"), &sealed)
                .unwrap()
                .len(),
            big.len()
        );
        assert!(open(&k, &BlobContext::backup(b"header-b"), &sealed).is_err());
        // The same bytes are refused for any other purpose (8 MB cap and AAD).
        assert!(open(
            &k,
            &BlobContext::vault(Purpose::Settings, Uuid::nil()),
            &sealed
        )
        .is_err());
    }

    #[test]
    fn backup_blobs_have_a_cap() {
        let k = key();
        let too_big = vec![0u8; MAX_BACKUP_BLOB_LEN];
        assert!(seal(&k, &BlobContext::backup(b"h"), &too_big).is_err());
    }

    #[test]
    fn only_details_carry_a_binding() {
        let (vault, item) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let bound_overview = BlobContext {
            bound_to: Some([0; 32]),
            ..BlobContext::item(Purpose::ItemOverview, vault, item)
        };
        assert!(seal(&key(), &bound_overview, b"x").is_err());
    }
}
