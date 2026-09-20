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
