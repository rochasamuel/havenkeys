//! HTTP against a `havenkeys-server` account.
//!
//! This crate carries ciphertext and never opens it. It never sees the master
//! password, the Secret Key, the KEK or the vault key; the only secret it
//! handles is the auth key, which exists to be sent to the server, and the
//! session token it gets back, which lives in memory for as long as the vault
//! is unlocked.
//!
//! The server is untrusted. Every answer is bounded while it is read and
//! checked before it is used, and a server that misbehaves produces a
//! `SyncError`, never a panic and never an unbounded allocation. Deciding
//! whether a blob is *authentic* is the core's job, not this crate's.

#![forbid(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod client;
pub mod error;
pub mod invite;
pub mod session;
pub mod transport;
pub mod wire;

pub use client::{Activated, Activation, AuthParams, Device, Pulled, SyncClient, WriteAck};
pub use error::{Conflict, Result, SyncError};
pub use invite::Invite;
pub use session::Session;
pub use transport::{HttpRequest, HttpResponse, HttpTransport, Method, Transport};
