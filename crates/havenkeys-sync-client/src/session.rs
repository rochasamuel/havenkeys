//! A server session: a bearer token held in memory for as long as the vault
//! is unlocked, and zeroized when it is dropped.
//!
//! Nothing here is ever written to disk. A locked vault has no session, which
//! is the same state as an offline device, and the client has one code path
//! for both (design §6).

use std::fmt;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Session {
    token: Zeroizing<String>,
    /// RFC 3339, as the server reported it. Advisory only: the server decides
    /// when a token stops working, and a client clock says nothing about it.
    pub expires_at: String,
    pub account_id: Uuid,
    pub vault_id: Uuid,
}

impl Session {
    pub fn new(token: String, expires_at: String, account_id: Uuid, vault_id: Uuid) -> Self {
        Self {
            token: Zeroizing::new(token),
            expires_at,
            account_id,
            vault_id,
        }
    }

    pub(crate) fn token(&self) -> Zeroizing<String> {
        self.token.clone()
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("account_id", &self.account_id)
            .field("vault_id", &self.vault_id)
            .field("token", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_token() {
        let session = Session::new(
            "super-secret-token".into(),
            "2026-09-21T00:00:00Z".into(),
            Uuid::nil(),
            Uuid::nil(),
        );
        let shown = format!("{session:?}");
        assert!(!shown.contains("super-secret-token"), "{shown}");
        assert!(shown.contains("<redacted>"));
    }
}
