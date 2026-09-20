//! HavenKeys sync server: a blind relay for one account's encrypted vault.
//!
//! It is the single writer — it assigns every item revision and the vault's
//! sync cursor — and it can read none of what it stores. Item blobs and the
//! vault header are AES-256-GCM ciphertext sealed under keys derived from the
//! master password and the Secret Key, neither of which reaches the server.
//!
//! Two rules shape every route here:
//!
//! * identity comes from the session token, never from a request body;
//! * blobs are opaque bytes, checked for size and nothing else.
//!
//! See `docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`.

pub mod config;
pub mod db;
pub mod email;
pub mod error;
pub mod limits;
pub mod routes;

pub use config::Config;
pub use error::ApiError;
pub use routes::{router, AppState};
