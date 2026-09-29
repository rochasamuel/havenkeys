//! The Identity for web pages (spec 2026-09-29-identity-autofill §6.2).
//!
//! Unlike a login, the identity is not bound to a site: any page may ask.
//! What protects it is here and in the extension: only http(s) pages; a
//! frame only when it is the same site as the tab's page; only the roles
//! asked for; documents only when the user confirmed them and only on https.

use crate::error::{Error, Result};
use crate::identity::FillRole;
use crate::origin::{site_of, PageUrl};
use crate::secret::SecretString;
use crate::vault::VaultService;
use std::fmt;
use uuid::Uuid;

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
        match (site_of(&page), site_of(&top)) {
            (Some(a), Some(b)) if a == b => {}
            _ => return Err(Error::Denied),
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

    /// Title, email and the roles the identity has a value for.
    pub fn identity_summary_for_page(
        &self,
        page_url: &str,
        top_url: Option<&str>,
    ) -> Result<IdentitySummary> {
        let id = self.identity_id_for_page(page_url, top_url)?;
        let overview = self.get_item(&id)?;
        let fields = self.reveal_identity(&id)?;
        let roles = FillRole::ALL
            .into_iter()
            .filter(|r| fields.fill_value(*r).is_some())
            .collect();
        Ok(IdentitySummary {
            title: overview.title.clone(),
            email: overview.username.clone(),
            roles,
        })
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
        let id = self.identity_item_id()?;
        let page = checked_page(page_url, top_url)?;
        let https = page.url().scheme() == "https";
        let fields = self.reveal_identity(&id)?;
        Ok(roles
            .iter()
            .filter(|r| !r.is_document() || (documents && https))
            .filter_map(|r| fields.fill_value(*r).map(|v| (*r, v)))
            .collect())
    }
}
