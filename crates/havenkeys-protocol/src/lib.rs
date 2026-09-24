//! HavenKeys bridge protocol.
//!
//! Shared by the two untrusted-input boundaries of the browser integration:
//!
//! ```text
//! extension ──stdio──► native host ──local socket──► desktop bridge ──► core
//! ```
//!
//! Both hops use the same framing (a 4-byte native-endian length followed by
//! UTF-8 JSON, as Chrome and Firefox native messaging define it) and the same
//! typed messages. Every message is parsed into the types in [`message`], with
//! unknown fields and unknown request types rejected, before anything acts on
//! it. See docs/native-messaging.md.
//!
//! This crate has no access to the vault and nothing in it logs.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod endpoint;
pub mod frame;
pub mod message;
pub mod secret;

pub use message::*;
pub use secret::WireSecret;

/// Version carried in every message as `v`. Bump on incompatible changes.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest request accepted from the extension or on the socket. Real
/// requests are well under 1 KiB; the largest field is a URL.
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// Largest message sent back. Chrome refuses host messages over 1 MiB; this
/// stays far below that.
pub const MAX_RESPONSE_BYTES: usize = 256 * 1024;

/// Page URLs longer than this are rejected.
pub const MAX_URL_BYTES: usize = 4096;

/// Largest password accepted in `check_login`/`save_login` (the core allows
/// 4096 characters; this is the byte bound).
pub const MAX_SECRET_BYTES: usize = 4 * 4096;

/// Largest username accepted in `check_login`/`save_login`.
pub const MAX_USERNAME_BYTES: usize = 4 * 512;

/// `find_matches` never returns more suggestions than this.
pub const MAX_MATCHES: usize = 50;

/// Credential IDs HavenKeys creates, and the only length it accepts.
pub const CREDENTIAL_ID_BYTES: usize = 16;
/// WebAuthn challenge bounds, in bytes.
pub const MAX_CHALLENGE_BYTES: usize = 1024;
/// WebAuthn user handle bound, in bytes.
pub const MAX_USER_HANDLE_BYTES: usize = 64;
/// Relying-party ID bound (a DNS name).
pub const MAX_RP_ID_BYTES: usize = 253;
/// `allowCredentials` / `excludeCredentials` entries accepted.
pub const MAX_CREDENTIAL_LIST: usize = 64;
/// COSE ES256, the only algorithm HavenKeys creates keys for.
pub const COSE_ES256: i64 = -7;
