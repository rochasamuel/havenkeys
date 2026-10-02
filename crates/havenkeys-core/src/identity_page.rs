//! The Identity for web pages (spec 2026-09-29-identity-autofill §6.2).
//!
//! Unlike a login, the identity is not bound to a site: any page may ask.
//! What protects it is here and in the extension: only http(s) pages; a
//! frame only when it is the same site as the tab's page; only the roles
//! asked for; documents only when the user confirmed them and only on https.

use crate::error::{Error, Result};
use crate::identity::FillRole;
use crate::origin::{same_site, PageUrl};
use crate::secret::SecretString;
use crate::vault::VaultService;
use std::fmt;
use uuid::Uuid;

/// Longest summary title, in bytes: the native-messaging limit on a title
/// (`havenkeys_protocol::MAX_TITLE_BYTES`). A name of three 256-character
/// parts with accents can exceed it, and a longer title would fail the
/// whole `find_identity` reply.
pub const MAX_SUMMARY_TITLE_BYTES: usize = 4 * 256;

/// `s` cut to at most `max` bytes, on a character boundary.
pub(crate) fn truncate_bytes(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// What the menu row needs: no values, only which roles have one.
pub struct IdentitySummary {
    pub title: String,
    pub email: Option<String>,
    pub roles: Vec<FillRole>,
}

impl fmt::Debug for IdentitySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentitySummary")
            .field("roles", &self.roles.len())
            .finish_non_exhaustive()
    }
}

/// The frame's page, if it may be served at all.
fn checked_page(page_url: &str, top_url: Option<&str>) -> Result<PageUrl> {
    let page = PageUrl::parse(page_url).ok_or(Error::Denied)?;
    if let Some(top) = top_url {
        let top = PageUrl::parse(top).ok_or(Error::Denied)?;
        if !same_site(&page, &top) {
            return Err(Error::Denied);
        }
    }
    Ok(page)
}

impl VaultService {
    /// The identity's ID, for a page that may use it.
    pub fn identity_id_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<Uuid> {
        let id = self.identity_item_id()?;
        checked_page(page_url, top_url)?;
        self.get_item(&id)?;
        Ok(id)
    }

    fn identity_summary(&self, id: Uuid) -> Result<IdentitySummary> {
        let overview = self.get_item(&id)?;
        let fields = self.reveal_identity(&id)?;
        let roles = FillRole::ALL
            .into_iter()
            .filter(|r| fields.fill_value(*r).is_some())
            .collect();
        Ok(IdentitySummary {
            title: truncate_bytes(overview.title.clone(), MAX_SUMMARY_TITLE_BYTES),
            email: overview.username.clone(),
            roles,
        })
    }

    /// The values for `roles`, in that order, skipping roles with no value;
    /// documents only when `documents`.
    fn identity_values(
        &self,
        roles: &[FillRole],
        documents: bool,
    ) -> Result<Vec<(FillRole, SecretString)>> {
        let fields = self.reveal_identity(&self.identity_item_id()?)?;
        Ok(roles
            .iter()
            .filter(|r| !r.is_document() || documents)
            .filter_map(|r| fields.fill_value(*r).map(|v| (*r, v)))
            .collect())
    }

    /// Title, email and the roles the identity has a value for.
    pub fn identity_summary_for_page(
        &self,
        page_url: &str,
        top_url: Option<&str>,
    ) -> Result<IdentitySummary> {
        let id = self.identity_id_for_page(page_url, top_url)?;
        self.identity_summary(id)
    }

    /// The values for `roles`, in that order, skipping roles with no value.
    /// Documents only with `documents` and on an https page.
    pub fn identity_values_for_page(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        roles: &[FillRole],
        documents: bool,
    ) -> Result<Vec<(FillRole, SecretString)>> {
        // Locked before denied, as before.
        self.identity_item_id()?;
        let page = checked_page(page_url, top_url)?;
        self.identity_values(roles, documents && page.is_https())
    }

    /// The identity for an Android app (spec 2026-10-01-android-app §7.6):
    /// an app owns what it shows, so no page rule applies.
    pub fn identity_summary_for_app(&self) -> Result<IdentitySummary> {
        let id = self.identity_item_id()?;
        self.identity_summary(id)
    }

    /// Documents only when `documents`: the user confirmed them in HavenKeys.
    pub fn identity_values_for_app(
        &self,
        roles: &[FillRole],
        documents: bool,
    ) -> Result<Vec<(FillRole, SecretString)>> {
        self.identity_values(roles, documents)
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_bytes;

    #[test]
    fn truncation_keeps_whole_characters() {
        assert_eq!(truncate_bytes("abc".into(), 10), "abc");
        assert_eq!(truncate_bytes("abc".into(), 2), "ab");
        // "é" is two bytes: cutting inside it drops it whole.
        assert_eq!(truncate_bytes("aé".into(), 2), "a");
        assert_eq!(truncate_bytes("aé".into(), 3), "aé");
    }
}
