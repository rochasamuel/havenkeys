//! Exporting the vault (spec 2026-10-05-export): an encrypted HavenKeys
//! backup, or plaintext Bitwarden JSON / CSV for other password managers.
//!
//! Plaintext exports never contain passkey private keys. Nothing here logs;
//! errors are fixed strings; summaries are counts only.

use crate::error::{Error, Result};
use crate::secret::SecretString;
use crate::vault::{secrets_equal, MAX_MASTER_PASSWORD_CHARS, MIN_MASTER_PASSWORD_CHARS};

/// The backup password: as long as a master password may be, and not the
/// master password itself (a backup must not fall with the account).
pub fn check_backup_password(backup: &SecretString, master: &SecretString) -> Result<()> {
    let n = backup.char_len();
    if n < MIN_MASTER_PASSWORD_CHARS {
        return Err(Error::InvalidInput(
            "backup password must be at least 10 characters",
        ));
    }
    if n > MAX_MASTER_PASSWORD_CHARS {
        return Err(Error::InvalidInput("backup password is too long"));
    }
    if secrets_equal(backup, master) {
        return Err(Error::InvalidInput(
            "backup password must differ from the master password",
        ));
    }
    Ok(())
}
