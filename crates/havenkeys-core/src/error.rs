//! Error type for the security core.
//!
//! Every variant renders to a fixed message. Variants never carry user data,
//! secrets, file paths or underlying library errors, so an `Error` is always
//! safe to log or show in the UI.

use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    #[error("The vault is locked.")]
    Locked,
    #[error("The vault is busy. Try again in a moment.")]
    Busy,
    #[error("Incorrect master password or damaged vault.")]
    UnlockFailed,
    #[error("This vault needs your Secret Key.")]
    SecretKeyRequired,
    #[error("Failed to decrypt vault item.")]
    Decryption,
    #[error("Failed to encrypt vault item.")]
    Encryption,
    #[error("The vault file is corrupted.")]
    Corrupted,
    #[error("This vault was created by an unsupported version.")]
    UnsupportedVersion,
    #[error("A vault already exists.")]
    VaultExists,
    #[error("No vault exists yet.")]
    NoVault,
    #[error("Item not found.")]
    NotFound,
    #[error("This item is not saved for this website.")]
    Denied,
    #[error("Invalid input: {0}.")]
    InvalidInput(&'static str),
    #[error("Vault storage error.")]
    Storage,
    #[error("Key derivation failed.")]
    Kdf,
    #[error("Secure random number generator unavailable.")]
    Rng,
}

impl Error {
    /// Stable machine-readable code for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Locked => "locked",
            Error::Busy => "busy",
            Error::UnlockFailed => "unlock_failed",
            Error::SecretKeyRequired => "secret_key_required",
            Error::Decryption => "decryption",
            Error::Encryption => "encryption",
            Error::Corrupted => "corrupted",
            Error::UnsupportedVersion => "unsupported_version",
            Error::VaultExists => "vault_exists",
            Error::NoVault => "no_vault",
            Error::NotFound => "not_found",
            Error::Denied => "denied",
            Error::InvalidInput(_) => "invalid_input",
            Error::Storage => "storage",
            Error::Kdf => "kdf",
            Error::Rng => "rng",
        }
    }
}

// Underlying errors are deliberately discarded: rusqlite errors can embed SQL
// or values, serde errors can quote input.
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Error::Storage
    }
}

pub type Result<T> = std::result::Result<T, Error>;
