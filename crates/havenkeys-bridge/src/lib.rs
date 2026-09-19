//! Desktop side of the browser integration.
//!
//! Receives requests from the native host over a local socket, and answers
//! them from the vault. This is where the security boundary for the browser
//! is enforced: every request is re-validated here (vault unlocked?
//! integration enabled? rate limit? item saved for this page?), no matter
//! what the extension or native host already checked.
//!
//! Nothing in this crate logs.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

mod dispatch;
mod ratelimit;
mod server;

pub use dispatch::dispatch;
pub use ratelimit::{RateLimiter, RequestClass};
pub use server::{Bridge, MAX_CONNECTIONS};
