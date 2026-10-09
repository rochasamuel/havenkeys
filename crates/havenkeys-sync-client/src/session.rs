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
    /// The plan the server last reported, if it reports one.
    pub account: Option<AccountInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entitlement {
    Full,
    Frozen,
}

/// The account's plan as the server last reported it. Not secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub status: String,
    pub entitlement: Entitlement,
    pub trial_ends_at: Option<String>,
    pub period_end: Option<String>,
}

impl From<crate::wire::AccountDto> for AccountInfo {
    fn from(dto: crate::wire::AccountDto) -> Self {
        // Only the one word the client acts on freezes it; anything else,
        // including a word a newer server might add, reads as Full.
        let entitlement = if dto.entitlement == "frozen" {
            Entitlement::Frozen
        } else {
            Entitlement::Full
        };
        let short = |s: String| {
            if s.chars().count() > 32 {
                String::new()
            } else {
                s
            }
        };
        Self {
            status: short(dto.status),
            entitlement,
            trial_ends_at: dto.trial_ends_at.map(short).filter(|s| !s.is_empty()),
            period_end: dto.period_end.map(short).filter(|s| !s.is_empty()),
        }
    }
}

impl Session {
    pub fn new(token: String, expires_at: String, account_id: Uuid, vault_id: Uuid) -> Self {
        Self {
            token: Zeroizing::new(token),
            expires_at,
            account_id,
            vault_id,
            account: None,
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
