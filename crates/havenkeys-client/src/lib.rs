//! Account, session and sync logic shared by the HavenKeys apps.
//!
//! Everything a device does with its account lives here: activation,
//! signing in, unlocking, the server session, sync and writes. The shells
//! (Tauri on desktop, UniFFI on mobile) only translate calls and events.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

mod account;
mod bundle;
mod client;
pub mod device;
mod error;
mod events;
pub mod key_store;
mod removal;
#[cfg(test)]
mod stub_server;
mod sync;

pub use account::{AccountField, AccountStatus, DeviceEntry, DeviceStatus};
pub use client::{ClientConfig, HavenClient};
pub use error::{ClientError, ClientResult};
pub use events::ClientEvents;
pub use sync::PULL_INTERVAL;

/// Wall-clock time in Unix milliseconds.
pub fn now_ms() -> i64 {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}
