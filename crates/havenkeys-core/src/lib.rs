//! HavenKeys security core.
//!
//! Owns all cryptography, vault state, lock state and authorization.
//! Nothing in this crate logs or prints; errors never carry secrets.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod account;
pub mod app_target;
pub mod card;
pub mod card_page;
pub mod crypto;
pub mod custom_field;
pub mod error;
pub mod generator;
pub mod identity;
pub mod identity_page;
pub mod import;
pub mod local;
pub mod lock;
pub mod model;
pub mod origin;
pub mod passkey;
pub mod secret;
pub mod sso;
pub mod store;
pub mod sync;
pub mod totp;
pub mod unlock_bundle;
pub mod vault;

pub use error::{Error, Result};
pub use secret::{SecretBytes, SecretString};
