//! The API the HavenKeys Android app (and later iOS) calls, through UniFFI.
//!
//! Intent-level, like the desktop bridge: it never returns keys, blobs or
//! whole vault objects, and every call that returns a secret checks the lock
//! state, the item and the fill target here (spec 2026-10-01-android-app
//! §4.2). Calls block; the app runs them on `Dispatchers.IO`.

#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

uniffi::setup_scaffolding!();

mod error;
mod events;
mod key_file;
mod settings;
mod vault;

pub use error::{MobileError, MobileResult};
pub use events::VaultEvents;
pub use key_file::{CipherError, KeystoreCipher};
pub use settings::MobileSettings;
pub use vault::{LockState, MobileConfig, MobileVault, Status};
