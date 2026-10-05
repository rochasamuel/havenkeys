//! The JSON the server speaks, and the conversions to and from core types.
//!
//! Requests use `deny_unknown_fields` on their own shapes; responses do not,
//! so a server that gains a field does not brick older clients. Every value
//! the client *acts* on is checked here: a field it does not know is a field
//! it does not use.

use crate::error::{Conflict, Result, SyncError};
use data_encoding::BASE64;
use havenkeys_core::crypto::blob::MAX_BLOB_LEN;
use havenkeys_core::crypto::kdf::{KdfAlgorithm, KdfParams, SALT_LEN};
use havenkeys_core::sync::RemoteChange;
use havenkeys_core::vault::StagedWrite;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Item changes the server will accept in one batch, and the most it will
/// return in one pull page. Mirrors the server's limits; a page claiming more
/// is a server misbehaving.
pub const MAX_CHANGES: usize = 500;

/// The header ceiling, matching the core's parser.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

// ------------------------------------------------------------------ requests

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KdfDto {
    pub algorithm: &'static str,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: String,
}

impl From<&KdfParams> for KdfDto {
    fn from(kdf: &KdfParams) -> Self {
        Self {
            algorithm: "argon2id",
            memory_kib: kdf.memory_kib,
            iterations: kdf.iterations,
            parallelism: kdf.parallelism,
            salt: BASE64.encode(&kdf.salt),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateBody<'a> {
    pub email: &'a str,
    pub invite: &'a str,
    pub kdf: KdfDto,
    pub auth_key: &'a str,
    pub vault_id: Uuid,
    pub header: String,
    pub key_scheme: i16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParamsBody<'a> {
    pub email: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginBody<'a> {
    pub email: &'a str,
    pub auth_key: &'a str,
    pub device_id: Uuid,
    pub device_name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialsBody<'a> {
    pub current_auth_key: &'a str,
    pub kdf: KdfDto,
    pub new_auth_key: &'a str,
    pub header: String,
    pub base_header_revision: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountDeletionBody<'a> {
    pub current_auth_key: &'a str,
    pub email: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteBody {
    pub changes: Vec<ChangeDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeDto {
    pub item_id: Uuid,
    pub base_revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub deleted: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl TryFrom<&StagedWrite> for ChangeDto {
    type Error = SyncError;

    /// A staged write is either both blobs or neither. Anything else never
    /// leaves this machine: sending it would make the server decide what a
    /// half-formed change means.
    fn try_from(staged: &StagedWrite) -> Result<Self> {
        match (&staged.overview, &staged.details) {
            (Some(overview), Some(details)) => {
                if overview.len() > MAX_BLOB_LEN || details.len() > MAX_BLOB_LEN {
                    return Err(SyncError::Refused("an item is too large to sync"));
                }
                Ok(Self {
                    item_id: staged.item_id,
                    base_revision: staged.base_revision,
                    overview: Some(BASE64.encode(overview)),
                    details: Some(BASE64.encode(details)),
                    deleted: false,
                })
            }
            (None, None) => Ok(Self {
                item_id: staged.item_id,
                base_revision: staged.base_revision,
                overview: None,
                details: None,
                deleted: true,
            }),
            _ => Err(SyncError::Protocol("malformed staged write")),
        }
    }
}

// ----------------------------------------------------------------- responses

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivatedDto {
    pub account_id: Uuid,
    pub vault_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamsDto {
    pub account_id: Uuid,
    pub kdf: KdfResponseDto,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfResponseDto {
    pub algorithm: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: String,
}

impl TryFrom<KdfResponseDto> for KdfParams {
    type Error = SyncError;

    /// The cost is checked against the core's accepted range before it is
    /// used: a server that could name any parameters could make a device
    /// derive a key cheaply enough to guess, or exhaust its memory trying.
    fn try_from(dto: KdfResponseDto) -> Result<Self> {
        if dto.algorithm != "argon2id" {
            return Err(SyncError::Protocol("unsupported kdf"));
        }
        let decoded = BASE64
            .decode(dto.salt.as_bytes())
            .map_err(|_| SyncError::Protocol("kdf salt"))?;
        let salt: [u8; SALT_LEN] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| SyncError::Protocol("kdf salt"))?;
        let params = KdfParams {
            algorithm: KdfAlgorithm::Argon2id,
            memory_kib: dto.memory_kib,
            iterations: dto.iterations,
            parallelism: dto.parallelism,
            salt,
        };
        params
            .validate()
            .map_err(|_| SyncError::Protocol("kdf parameters out of range"))?;
        Ok(params)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDto {
    pub token: String,
    pub expires_at: String,
    pub vault_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderDto {
    pub header: String,
    pub header_revision: i64,
    pub key_scheme: i16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsAckDto {
    pub header_revision: i64,
}

/// The header as the server serves it, already decoded and bounded. The
/// attestation on the bytes is checked by the core, not here.
pub struct RemoteHeader {
    pub bytes: Vec<u8>,
    pub revision: i64,
    pub key_scheme: i16,
}

impl std::fmt::Debug for RemoteHeader {
    /// The header is not secret, but it is not something to print either: it
    /// carries the wrapped vault key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteHeader")
            .field("bytes", &self.bytes.len())
            .field("revision", &self.revision)
            .field("key_scheme", &self.key_scheme)
            .finish()
    }
}

impl TryFrom<HeaderDto> for RemoteHeader {
    type Error = SyncError;

    fn try_from(dto: HeaderDto) -> Result<Self> {
        let bytes = BASE64
            .decode(dto.header.as_bytes())
            .map_err(|_| SyncError::Protocol("header encoding"))?;
        if bytes.is_empty() || bytes.len() > MAX_HEADER_BYTES {
            return Err(SyncError::Protocol("header size"));
        }
        if dto.header_revision < 0 {
            return Err(SyncError::Protocol("header revision"));
        }
        Ok(Self {
            bytes,
            revision: dto.header_revision,
            key_scheme: dto.key_scheme,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FetchBody<'a> {
    pub item_ids: &'a [Uuid],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchDto {
    pub changes: Vec<RemoteChangeDto>,
    /// Asked-for items that exist but did not fit this answer.
    #[serde(default)]
    pub unanswered: Vec<Uuid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullDto {
    pub cursor: i64,
    pub has_more: bool,
    pub changes: Vec<RemoteChangeDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteChangeDto {
    pub item_id: Uuid,
    pub revision: i64,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub deleted: bool,
}

impl TryFrom<RemoteChangeDto> for RemoteChange {
    type Error = SyncError;

    /// Structure only. Whether the blobs open under this vault's key is the
    /// core's decision (`apply_remote_changes`), and a blob that does not is
    /// skipped there rather than refused here.
    fn try_from(dto: RemoteChangeDto) -> Result<Self> {
        if dto.revision < 0 {
            return Err(SyncError::Protocol("item revision"));
        }
        let overview = decode_blob(dto.overview)?;
        let details = decode_blob(dto.details)?;
        if dto.deleted && (overview.is_some() || details.is_some()) {
            return Err(SyncError::Protocol("a deletion carrying blobs"));
        }
        if !dto.deleted && (overview.is_none() || details.is_none()) {
            return Err(SyncError::Protocol("an item missing a blob"));
        }
        Ok(RemoteChange {
            item_id: dto.item_id,
            revision: dto.revision,
            overview,
            details,
            deleted: dto.deleted,
        })
    }
}

/// A base64 blob from the server, bounded before and after decoding.
fn decode_blob(value: Option<String>) -> Result<Option<Vec<u8>>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if raw.len() > MAX_BLOB_LEN / 3 * 4 + 4 {
        return Err(SyncError::TooLarge);
    }
    let bytes = BASE64
        .decode(raw.as_bytes())
        .map_err(|_| SyncError::Protocol("blob encoding"))?;
    if bytes.len() > MAX_BLOB_LEN {
        return Err(SyncError::TooLarge);
    }
    Ok(Some(bytes))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteAckDto {
    pub cursor: i64,
    pub applied: Vec<AppliedDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedDto {
    pub item_id: Uuid,
    pub revision: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictsDto {
    pub conflicts: Vec<ConflictDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDto {
    pub item_id: Uuid,
    #[serde(default)]
    pub revision: Option<i64>,
}

impl From<ConflictDto> for Conflict {
    fn from(dto: ConflictDto) -> Self {
        Self {
            item_id: dto.item_id,
            revision: dto.revision,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDto {
    pub id: Uuid,
    pub name: String,
    pub created_at: String,
    #[serde(default)]
    pub last_seen_at: Option<String>,
    pub current: bool,
    #[serde(default)]
    pub approved_by: Option<Uuid>,
}

// ------------------------------------------------------------------ pairing

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePairingBody<'a> {
    pub device_id: Uuid,
    pub device_name: &'a str,
    pub claim_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedPairingDto {
    pub pairing_id: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingDetailsDto {
    pub device_name: String,
    pub ip: String,
    #[serde(default)]
    pub location: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovePairingBody {
    pub envelope: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimPairingBody {
    pub claim_secret: String,
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ClaimDto {
    Waiting,
    Denied,
    #[serde(rename_all = "camelCase")]
    Approved {
        token: String,
        expires_at: String,
        account_id: Uuid,
        vault_id: Uuid,
        envelope: String,
    },
}

/// Parse a JSON body into a response type.
pub fn parse<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T> {
    serde_json::from_slice(body).map_err(|_| SyncError::Protocol("unexpected response"))
}

#[cfg(test)]
mod pairing_tests {
    use super::*;

    #[test]
    fn a_claim_answer_parses_each_state_and_refuses_unknown_ones() {
        let waiting: ClaimDto = parse(br#"{"state":"waiting"}"#).unwrap();
        assert!(matches!(waiting, ClaimDto::Waiting));
        let denied: ClaimDto = parse(br#"{"state":"denied"}"#).unwrap();
        assert!(matches!(denied, ClaimDto::Denied));
        let approved: ClaimDto = parse(
            br#"{"state":"approved","token":"t","expiresAt":"2099-01-01T00:00:00Z",
                 "accountId":"00000000-0000-0000-0000-000000000001",
                 "vaultId":"00000000-0000-0000-0000-000000000002","envelope":"AQID"}"#,
        )
        .unwrap();
        assert!(matches!(approved, ClaimDto::Approved { .. }));
        assert!(parse::<ClaimDto>(br#"{"state":"approved"}"#).is_err());
        assert!(parse::<ClaimDto>(br#"{"state":"granted"}"#).is_err());
    }
}
