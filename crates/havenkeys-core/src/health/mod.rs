//! Vault health (spec 2026-10-07-vault-health): which logins have a weak,
//! reused or old password, an http website, a duplicate, or a site that
//! offers passkeys or one-time codes the login does not use. Computed from
//! the unlocked vault; the report holds item IDs and check kinds only.

#[allow(dead_code)] // used by compute (next task)
pub(crate) mod directory;
