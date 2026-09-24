//! Passkey operations on an unlocked vault.
//!
//! Same rule as the password path (`fill_for_page`): the page URL is
//! re-checked on every call and nothing the caller claims is trusted. A
//! passkey is bound to its relying-party ID; `authorize_rp` decides whether
//! the page may use that ID, and the stored `rp_id` must equal it. The
//! login's own website rules decide only which logins are offered as a home
//! for a new passkey.

use super::{
    assert, authorize_rp, clean_site_name, register, Assertion, B64Url, NewUser, Registration,
    CREDENTIAL_ID_LEN, MAX_CHALLENGE_BYTES, MAX_PASSKEYS_PER_LOGIN, MAX_USER_HANDLE_BYTES,
    MIN_CHALLENGE_BYTES,
};
use crate::error::{Error, Result};
use crate::model::{ItemDetails, ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use crate::origin::PageUrl;
use crate::vault::{build_item, StagedWrite, Suggestion, VaultService};
use serde::Serialize;
use std::fmt;
use uuid::Uuid;

/// A passkey offered for a page. No secrets.
#[derive(Clone)]
pub struct PasskeyMatch {
    pub item_id: Uuid,
    pub credential_id: Vec<u8>,
    pub title: String,
    pub user_name: String,
}

impl fmt::Debug for PasskeyMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasskeyMatch")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

/// Result of [`VaultService::check_passkey_create`].
#[derive(Debug)]
pub struct CreateCheck {
    /// The vault already holds one of the site's `excludeCredentials`.
    pub excluded: bool,
    /// Logins saved for the page that can take another passkey, the one
    /// with the same username first.
    pub candidates: Vec<Suggestion>,
}

/// A site's `create()` request, after the bridge decoded it.
pub struct PasskeyCreate<'a> {
    pub rp_id: &'a str,
    pub page_url: &'a str,
    pub top_url: Option<&'a str>,
    pub challenge: &'a [u8],
    pub user_handle: &'a [u8],
    pub user_name: &'a str,
    pub display_name: Option<&'a str>,
    /// Attach to this login (it must be saved for the page); `None` makes a
    /// new login.
    pub item_id: Option<Uuid>,
}

/// A passkey sealed into its login, ready to send. `registration` goes back
/// to the site only after the server accepted `write`.
pub struct StagedPasskey {
    pub write: StagedWrite,
    pub item_id: Uuid,
    pub registration: Registration,
}

impl fmt::Debug for StagedPasskey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagedPasskey")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

/// What the desktop shows about a passkey. Never the key.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyInfo {
    pub credential_id: String,
    pub rp_id: String,
    pub user_name: String,
    pub display_name: Option<String>,
    pub created_at: i64,
}

fn check_challenge(c: &[u8]) -> Result<()> {
    if (MIN_CHALLENGE_BYTES..=MAX_CHALLENGE_BYTES).contains(&c.len()) {
        Ok(())
    } else {
        Err(Error::InvalidInput("invalid challenge"))
    }
}

fn fold(s: &str) -> String {
    s.trim().to_lowercase()
}

impl VaultService {
    /// Passkeys for `rp_id` that the page may use, filtered by the site's
    /// `allowCredentials` when it sent any. A login whose details do not
    /// open is skipped, not fatal.
    pub fn find_passkeys(
        &self,
        rp_id: &str,
        page_url: &str,
        top_url: Option<&str>,
        allow: &[Vec<u8>],
    ) -> Result<Vec<PasskeyMatch>> {
        let session = self.session()?;
        let ctx = authorize_rp(rp_id, page_url, top_url)?;
        let holders: Vec<(Uuid, String)> = session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login && o.has_passkey)
            .map(|o| (o.id, o.title.clone()))
            .collect();
        let mut out = Vec::new();
        for (id, title) in holders {
            let Ok(ItemDetails::Login { passkeys, .. }) = self.load_details(&id) else {
                continue;
            };
            for p in passkeys {
                if p.rp_id == ctx.rp_id && (allow.is_empty() || allow.contains(&p.credential_id.0))
                {
                    out.push(PasskeyMatch {
                        item_id: id,
                        credential_id: p.credential_id.0.clone(),
                        title: title.clone(),
                        user_name: p.user_name.clone(),
                    });
                }
            }
        }
        out.sort_by_cached_key(|m| {
            (
                m.title.to_lowercase(),
                m.user_name.to_lowercase(),
                m.item_id,
            )
        });
        Ok(out)
    }

    /// Sign in: the assertion for one passkey, if and only if it is bound to
    /// `rp_id` and the page may use `rp_id`.
    pub fn passkey_assert(
        &self,
        item_id: &Uuid,
        credential_id: &[u8],
        rp_id: &str,
        page_url: &str,
        top_url: Option<&str>,
        challenge: &[u8],
    ) -> Result<Assertion> {
        self.session()?;
        check_challenge(challenge)?;
        let ctx = authorize_rp(rp_id, page_url, top_url)?;
        let ItemDetails::Login { passkeys, .. } = self.load_details(item_id)? else {
            return Err(Error::Denied);
        };
        let passkey = passkeys
            .iter()
            .find(|p| p.credential_id.0 == credential_id && p.rp_id == ctx.rp_id)
            .ok_or(Error::Denied)?;
        assert(passkey, &ctx, challenge)
    }

    /// Before asking the user: is one of the site's `excludeCredentials`
    /// already here, and which logins could hold the new passkey?
    pub fn check_passkey_create(
        &self,
        rp_id: &str,
        page_url: &str,
        top_url: Option<&str>,
        user_name: &str,
        exclude: &[Vec<u8>],
    ) -> Result<CreateCheck> {
        self.session()?;
        authorize_rp(rp_id, page_url, top_url)?;
        let excluded = !exclude.is_empty()
            && !self
                .find_passkeys(rp_id, page_url, top_url, exclude)?
                .is_empty();
        let wanted = fold(user_name);
        let mut candidates: Vec<Suggestion> = Vec::new();
        for s in self.find_matches(page_url, top_url)? {
            let full = match self.load_details(&s.id) {
                Ok(ItemDetails::Login { passkeys, .. }) => passkeys.len() >= MAX_PASSKEYS_PER_LOGIN,
                _ => true,
            };
            if !full {
                candidates.push(s);
            }
        }
        // Stable: same username first, otherwise find_matches' order.
        candidates.sort_by_key(|s| s.username.as_deref().map(fold) != Some(wanted.clone()));
        Ok(CreateCheck {
            excluded,
            candidates,
        })
    }

    /// Create a passkey and seal it into a login (a new one, or `item_id`).
    /// Nothing is stored until the caller sends `write` and commits it.
    pub fn stage_passkey_create(
        &self,
        req: PasskeyCreate<'_>,
        now_ms: i64,
    ) -> Result<StagedPasskey> {
        self.session()?;
        check_challenge(req.challenge)?;
        if req.user_handle.is_empty() || req.user_handle.len() > MAX_USER_HANDLE_BYTES {
            return Err(Error::InvalidInput("invalid user handle"));
        }
        let user_name = clean_site_name(Some(req.user_name))?.unwrap_or_default();
        let display_name = clean_site_name(req.display_name)?.filter(|d| !d.is_empty());
        let ctx = authorize_rp(req.rp_id, req.page_url, req.top_url)?;
        let (passkey, registration) = register(
            &ctx,
            req.challenge,
            NewUser {
                user_handle: req.user_handle,
                user_name: user_name.clone(),
                display_name,
            },
            now_ms,
        )?;

        let (mut overview, mut details, base) = match req.item_id {
            Some(id) => {
                let offered = self
                    .find_matches(req.page_url, req.top_url)?
                    .iter()
                    .any(|s| s.id == id);
                if !offered {
                    return Err(Error::Denied);
                }
                let mut overview = self.get_item(&id)?;
                overview.updated_at = now_ms;
                let base = self.store.item_revision(&id)?;
                (overview, self.load_details(&id)?, base)
            }
            None => {
                let page = PageUrl::parse(req.page_url).ok_or(Error::Denied)?;
                let (title, origin) = page.title_and_origin().ok_or(Error::Denied)?;
                let input = ItemInput {
                    item_type: ItemType::Login,
                    title,
                    username: Some(user_name).filter(|u| !u.is_empty()),
                    urls: vec![UrlRule {
                        url: origin,
                        match_type: MatchType::Domain,
                    }],
                    password: SecretUpdate::Keep,
                    totp: SecretUpdate::Keep,
                    notes: SecretUpdate::Keep,
                    content: SecretUpdate::Keep,
                };
                let (overview, details) = build_item(Uuid::new_v4(), input, None, now_ms, now_ms)?;
                (overview, details, None)
            }
        };
        let ItemDetails::Login { passkeys, .. } = &mut details else {
            return Err(Error::Denied);
        };
        // Registering the same account again replaces its passkey.
        passkeys.retain(|p| !(p.rp_id == passkey.rp_id && p.user_handle == passkey.user_handle));
        if passkeys.len() >= MAX_PASSKEYS_PER_LOGIN {
            return Err(Error::InvalidInput(
                "this login already has the maximum number of passkeys",
            ));
        }
        passkeys.push(passkey);
        overview.has_passkey = true;
        let item_id = overview.id;
        let write = self.stage(overview, Some(&details), base)?;
        Ok(StagedPasskey {
            write,
            item_id,
            registration,
        })
    }

    /// Public details of a login's passkeys, for the desktop app.
    pub fn list_passkeys(&self, item_id: &Uuid) -> Result<Vec<PasskeyInfo>> {
        match self.load_details(item_id)? {
            ItemDetails::Login { passkeys, .. } => Ok(passkeys
                .iter()
                .map(|p| PasskeyInfo {
                    credential_id: p.credential_id.encode(),
                    rp_id: p.rp_id.clone(),
                    user_name: p.user_name.clone(),
                    display_name: p.display_name.clone(),
                    created_at: p.created_at,
                })
                .collect()),
            ItemDetails::SecureNote { .. } => Ok(Vec::new()),
        }
    }

    /// Seal the login without one passkey (desktop "Delete passkey").
    pub fn stage_remove_passkey(
        &self,
        item_id: &Uuid,
        credential_id: &str,
        now_ms: i64,
    ) -> Result<StagedWrite> {
        let credential = B64Url::decode(credential_id)?;
        if credential.0.len() != CREDENTIAL_ID_LEN {
            return Err(Error::InvalidInput("invalid credential ID"));
        }
        let mut overview = self.get_item(item_id)?;
        let mut details = self.load_details(item_id)?;
        let ItemDetails::Login { passkeys, .. } = &mut details else {
            return Err(Error::NotFound);
        };
        let before = passkeys.len();
        passkeys.retain(|p| p.credential_id != credential);
        if passkeys.len() == before {
            return Err(Error::NotFound);
        }
        overview.has_passkey = !passkeys.is_empty();
        overview.updated_at = now_ms;
        let base = self.store.item_revision(item_id)?;
        self.stage(overview, Some(&details), base)
    }
}
