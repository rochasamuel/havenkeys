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

/// `find_matches` never returns more suggestions than this.
pub const MAX_MATCHES: usize = 50;
