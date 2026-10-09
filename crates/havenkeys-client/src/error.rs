//! The one error type the shells see: a stable code and a fixed message.
//! Never carries a secret, and never the server's own words.

use havenkeys_sync_client::SyncError;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ClientError {
    pub code: &'static str,
    pub message: String,
}

pub type ClientResult<T> = Result<T, ClientError>;

impl From<havenkeys_core::Error> for ClientError {
    fn from(e: havenkeys_core::Error) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

impl From<SyncError> for ClientError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Conflict(_) => havenkeys_core::Error::ItemChangedElsewhere.into(),
            SyncError::Unavailable => havenkeys_core::Error::Offline.into(),
            SyncError::Unauthorized => Self::fixed(
                "signed_out",
                "HavenKeys is signed out of this account. Unlock again to reconnect.",
            ),
            SyncError::AccountDeleted => {
                Self::fixed("account_deleted", "This account was deleted.")
            }
            SyncError::AccountFrozen => Self::fixed(
                "account_frozen",
                "This account is frozen: the trial ended or payment lapsed. The vault is read-only.",
            ),
            SyncError::RateLimited => Self::fixed(
                "rate_limited",
                "Too many attempts. Try again in a few minutes.",
            ),
            SyncError::InvalidServerUrl => Self::fixed(
                "invalid_server_url",
                "That server address cannot be used. It must start with https://.",
            ),
            // The server's own words are never shown: they are text an
            // attacker could choose.
            SyncError::Refused(_) | SyncError::Protocol(_) | SyncError::TooLarge => {
                Self::fixed("sync_failed", "The server did not accept that request.")
            }
        }
    }
}

impl ClientError {
    fn fixed(code: &'static str, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn internal() -> Self {
        Self::fixed("internal", "Internal error.")
    }

    pub fn file() -> Self {
        Self::fixed("file", "Could not read or delete the file.")
    }

    /// One message for every way signing in can fail. The device never says
    /// whether it was the address, the password or the Secret Key.
    pub fn sign_in_failed() -> Self {
        Self::fixed(
            "sign_in_failed",
            "Email, master password or Secret Key is incorrect.",
        )
    }

    /// The code expired, was used, or was denied. One message for all.
    pub fn pairing_gone() -> Self {
        Self::fixed(
            "pairing_gone",
            "This code has expired. Ask the new device for a new one.",
        )
    }

    /// A code from a device signing in to a different server.
    pub fn pairing_other_server() -> Self {
        Self::fixed("pairing_other_server", "This code is for another server.")
    }

    /// The approval arrived but did not open or did not match this account.
    pub fn pairing_failed() -> Self {
        Self::fixed(
            "pairing_failed",
            "The sign-in could not be completed. Ask for a new code.",
        )
    }

    /// The vault file exists but this build cannot open it. Carries the
    /// folder so the message can tell the user where their file is; a path
    /// is not a secret, and without it the advice is unfollowable.
    pub fn vault_unreadable(dir: &std::path::Path) -> Self {
        Self {
            code: "vault_unreadable",
            message: format!(
                "This vault was created by an older version of HavenKeys and cannot be \
                 opened by this one. Your file is in {}. Move vault.sqlite3 and \
                 device.json somewhere safe — do not delete them — and start HavenKeys \
                 again to set this computer up with an invite.",
                dir.display()
            ),
        }
    }

    pub fn clipboard() -> Self {
        Self::fixed("clipboard", "Could not access the clipboard.")
    }

    pub fn open_website() -> Self {
        Self::fixed("open_website", "Could not open the website.")
    }

    /// The keychain did not give a definite answer at unlock.
    pub fn keychain_unavailable() -> Self {
        Self::fixed(
            "keychain_unavailable",
            "Your system keychain did not answer. Approve its prompt if one is showing, then try again.",
        )
    }

    pub fn password_change_unknown() -> Self {
        Self::fixed(
            "password_change_unknown",
            "HavenKeys could not confirm whether the server applied the new master password. If your current password stops working, use the new one.",
        )
    }

    pub fn invalid_kit() -> Self {
        Self::fixed(
            "invalid_kit",
            "That is not a HavenKeys Recovery Sheet code.",
        )
    }

    pub fn password_changed_elsewhere() -> Self {
        Self::fixed(
            "password_changed_elsewhere",
            "Your master password was changed on another device. Lock and unlock with the new password.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ClientError;
    use havenkeys_sync_client::SyncError;

    #[test]
    fn sync_errors_keep_the_codes_the_desktop_ui_knows() {
        let cases = [
            (SyncError::Conflict(vec![]), "item_changed_elsewhere"),
            (SyncError::Unavailable, "offline"),
            (SyncError::Unauthorized, "signed_out"),
            (SyncError::RateLimited, "rate_limited"),
            (SyncError::InvalidServerUrl, "invalid_server_url"),
            (SyncError::TooLarge, "sync_failed"),
            (SyncError::AccountDeleted, "account_deleted"),
            (SyncError::AccountFrozen, "account_frozen"),
        ];
        for (err, code) in cases {
            assert_eq!(ClientError::from(err).code, code);
        }
    }

    #[test]
    fn fixed_errors_keep_their_codes() {
        let cases = [
            (ClientError::internal(), "internal"),
            (ClientError::file(), "file"),
            (ClientError::sign_in_failed(), "sign_in_failed"),
            (ClientError::clipboard(), "clipboard"),
            (ClientError::open_website(), "open_website"),
            (ClientError::keychain_unavailable(), "keychain_unavailable"),
            (
                ClientError::password_change_unknown(),
                "password_change_unknown",
            ),
            (
                ClientError::password_changed_elsewhere(),
                "password_changed_elsewhere",
            ),
        ];
        for (err, code) in cases {
            assert_eq!(err.code, code);
        }
    }

    #[test]
    fn the_servers_own_words_are_never_shown() {
        let err = ClientError::from(SyncError::Refused("<script>pwned</script>"));
        assert_eq!(err.code, "sync_failed");
        assert!(!err.message.contains("pwned"));
    }
}
