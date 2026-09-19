//! HavenKeys security core.
//!
//! Owns all cryptography, vault state, lock state and authorization.
//! Nothing in this crate logs or prints; errors never carry secrets.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod crypto;
pub mod error;
pub mod generator;
pub mod import;
pub mod lock;
pub mod model;
pub mod origin;
pub mod secret;
pub mod store;
pub mod sync;
pub mod totp;
pub mod vault;

pub use error::{Error, Result};
pub use secret::SecretString;
