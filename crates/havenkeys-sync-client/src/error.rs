//! What can go wrong talking to a server that is not trusted.
//!
//! Messages are fixed strings. A server-supplied message is never shown to
//! the user and never logged: it is attacker-controlled text.

use uuid::Uuid;

/// One item the server refused to write because its revision moved on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub item_id: Uuid,
    /// The revision the server holds, or `None` when it holds no such item.
    pub revision: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncError {
    /// No answer, a timeout, or the server failing. The desktop turns this
    /// into its offline state: the replica stays readable, writes stop.
    Unavailable,
    /// The session is gone: expired, revoked, or never valid. The device
    /// must sign in again.
    Unauthorized,
    /// Too many failed logins. Waiting is the only cure.
    RateLimited,
    /// The server refused the request and the client cannot fix it by
    /// retrying.
    Refused(&'static str),
    /// A write lost a race. Nothing was written; the caller pulls and shows
    /// the current value rather than overwriting it.
    Conflict(Vec<Conflict>),
    /// The answer was not what the protocol says it should be. A hostile or
    /// broken server lands here, never in a panic.
    Protocol(&'static str),
    /// The server's answer was larger than anything the protocol allows.
    TooLarge,
    /// The configured server URL is not one a secret may be sent to.
    InvalidServerUrl,
}

impl SyncError {
    /// A stable identifier for logs and for the desktop's error mapping.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Unauthorized => "unauthorized",
            Self::RateLimited => "rate_limited",
            Self::Refused(_) => "refused",
            Self::Conflict(_) => "conflict",
            Self::Protocol(_) => "protocol",
            Self::TooLarge => "too_large",
            Self::InvalidServerUrl => "invalid_server_url",
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => f.write_str("the server could not be reached"),
            Self::Unauthorized => f.write_str("this device is signed out"),
            Self::RateLimited => f.write_str("too many attempts; try again later"),
            Self::Refused(what) => write!(f, "the server refused the request: {what}"),
            Self::Conflict(items) => {
                write!(f, "{} item(s) changed on another device", items.len())
            }
            Self::Protocol(what) => write!(f, "the server's answer was not valid: {what}"),
            Self::TooLarge => f.write_str("the server's answer was too large"),
            Self::InvalidServerUrl => f.write_str("the server address is not usable"),
        }
    }
}

impl std::error::Error for SyncError {}

pub type Result<T> = std::result::Result<T, SyncError>;
