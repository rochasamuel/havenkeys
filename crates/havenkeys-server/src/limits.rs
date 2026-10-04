//! Every size limit the API enforces, in one place.
//!
//! They exist so a request cannot make the server allocate or work without
//! bound. The blob limit matches `havenkeys-core`'s, so a blob the client can
//! produce is a blob the server accepts, and nothing larger.

/// One encrypted item blob (overview or details).
pub const MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;

/// A whole request body, checked before the JSON is parsed.
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// The attested vault header (`header.json`), the same ceiling the core
/// applies when parsing one.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Item changes in one write batch.
pub const MAX_CHANGES_PER_BATCH: usize = 500;

/// Devices an account may have before it must revoke one.
pub const MAX_DEVICES_PER_ACCOUNT: i64 = 64;

/// Item changes in one pull page.
pub const MAX_PULL_PAGE: i64 = 500;

/// How long a session token stays valid.
pub const SESSION_TTL_HOURS: i64 = 24;

/// Longest device label a client may set.
pub const MAX_DEVICE_NAME_CHARS: usize = 64;

/// Pull and fetch answers stop adding rows past this many bytes (blobs as
/// base64, plus a per-row allowance for the JSON around them), so every
/// answer stays under the client's 17 MiB cap. A page always holds at
/// least one whole revision, which `MAX_BODY_BYTES` already bounds.
pub const MAX_PAGE_BYTES: usize = 12 * 1024 * 1024;

/// A pairing's QR code is good for this long.
pub const PAIRING_TTL_SECONDS: f64 = 120.0;

/// Every pairing older than this is deleted when a new one is created.
pub const PAIRING_MAX_AGE_MINUTES: i32 = 10;

/// Pairings one address may create in `PAIRING_MAX_AGE_MINUTES`.
pub const PAIRINGS_PER_IP: i64 = 10;

/// Pairings one address may have waiting at once.
pub const PENDING_PAIRINGS_PER_IP: i64 = 3;

/// The largest envelope an approving device may send.
pub const MAX_PAIRING_ENVELOPE_BYTES: usize = 4096;
