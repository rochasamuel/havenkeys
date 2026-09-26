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
use crate::model::{
    ItemDetails, ItemInput, ItemOverview, ItemType, MatchType, SecretUpdate, UrlRule,
};
use crate::origin::{site_of, PageUrl};
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

/// A site's automatic passkey upgrade (`create()` with conditional
/// mediation), decided here and nowhere else. `Auto`: save to this login
/// without asking; `Ask`: offer the save card with it preselected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    None,
    Ask(Uuid),
    Auto(Uuid),
}

/// A site's `create()` request, as far as the checks before saving need it.
pub struct CreateQuery<'a> {
    pub rp_id: &'a str,
    pub page_url: &'a str,
    pub top_url: Option<&'a str>,
    pub user_name: &'a str,
    pub exclude: &'a [Vec<u8>],
    /// `mediation: "conditional"`: the site's automatic upgrade.
    pub conditional: bool,
}

/// Result of [`VaultService::check_passkey_create`].
#[derive(Debug)]
pub struct CreateCheck {
    /// The vault already holds one of the site's `excludeCredentials`.
    pub excluded: bool,
    /// Logins saved for the page that can take another passkey, the one
    /// with the same username first.
    pub candidates: Vec<Suggestion>,
    /// The site's automatic upgrade, if any.
    pub upgrade: Upgrade,
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
    /// The site's automatic upgrade, saved without a click in HavenKeys UI.
    /// Refused unless the upgrade decision is `Auto(item_id)`.
    pub conditional: bool,
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
    /// The login that already holds a passkey for `rp_id` + `user_handle`,
    /// if any, anywhere in the vault. WebAuthn overwrites a credential
    /// source that shares an authenticator, rpId and user handle with an
    /// existing one (WebAuthn Level 3 §5.1.3 step 21.3) rather than
    /// creating a second one; that has to be checked across every login,
    /// not just the one the caller named, or a second registration for the
    /// same account into a different (or brand-new) login would leave the
    /// old passkey behind and `find_passkeys` would offer two credentials
    /// for the same account. A login whose details do not open is skipped,
    /// not fatal, matching `find_passkeys`.
    fn find_passkey_holder(&self, rp_id: &str, user_handle: &[u8]) -> Result<Option<Uuid>> {
        let session = self.session()?;
        let candidates: Vec<Uuid> = session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login && o.has_passkey)
            .map(|o| o.id)
            .collect();
        for id in candidates {
            if let Ok(ItemDetails::Login { passkeys, .. }) = self.load_details(&id) {
                if passkeys
                    .iter()
                    .any(|p| p.rp_id == rp_id && p.user_handle.0 == user_handle)
                {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }

    /// Load a login for editing, stamping `updated_at` and carrying the
    /// revision this device last saw so the server can detect a
    /// conflicting edit.
    fn load_login_for_edit(
        &self,
        id: Uuid,
        now_ms: i64,
    ) -> Result<(ItemOverview, ItemDetails, Option<i64>)> {
        let mut overview = self.get_item(&id)?;
        overview.updated_at = now_ms;
        let base = self.store.item_revision(&id)?;
        Ok((overview, self.load_details(&id)?, base))
    }

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

    /// Logins saved for the page that can take another passkey, in
    /// `find_matches` order.
    fn passkey_homes(&self, page_url: &str, top_url: Option<&str>) -> Result<Vec<Suggestion>> {
        let mut out = Vec::new();
        for s in self.find_matches(page_url, top_url)? {
            let full = match self.load_details(&s.id) {
                Ok(ItemDetails::Login { passkeys, .. }) => passkeys.len() >= MAX_PASSKEYS_PER_LOGIN,
                _ => true,
            };
            if !full {
                out.push(s);
            }
        }
        Ok(out)
    }

    /// The newest recent password fill on the page's site whose login is
    /// among `homes` and has the site's account name (folded) or none.
    /// `Auto` or `Ask` by the vault setting.
    fn upgrade_for(
        &self,
        page_url: &str,
        user_name: &str,
        homes: &[Suggestion],
        now_ms: i64,
    ) -> Result<Upgrade> {
        let session = self.session()?;
        let Some(site) = PageUrl::parse(page_url).as_ref().and_then(site_of) else {
            return Ok(Upgrade::None);
        };
        let wanted = fold(user_name);
        let same_account = |s: &Suggestion| {
            s.username
                .as_deref()
                .map(fold)
                .filter(|u| !u.is_empty())
                .is_none_or(|u| u == wanted)
        };
        let found = session
            .recent_fills
            .iter()
            .rev()
            .filter(|f| f.site == site && f.is_recent(now_ms))
            .find_map(|f| homes.iter().find(|s| s.id == f.item_id && same_account(s)))
            .map(|s| s.id);
        Ok(match found {
            None => Upgrade::None,
            Some(id) if session.settings.auto_passkey_upgrade => Upgrade::Auto(id),
            Some(id) => Upgrade::Ask(id),
        })
    }

    /// Before asking the user: is one of the site's `excludeCredentials`
    /// already here, which logins could hold the new passkey, and, for a
    /// conditional request, is this the site's automatic upgrade after a
    /// HavenKeys password fill?
    pub fn check_passkey_create(&self, q: &CreateQuery<'_>, now_ms: i64) -> Result<CreateCheck> {
        self.session()?;
        authorize_rp(q.rp_id, q.page_url, q.top_url)?;
        let excluded = !q.exclude.is_empty()
            && !self
                .find_passkeys(q.rp_id, q.page_url, q.top_url, q.exclude)?
                .is_empty();
        let wanted = fold(q.user_name);
        let mut candidates = self.passkey_homes(q.page_url, q.top_url)?;
        // Stable: same username first, otherwise find_matches' order.
        candidates.sort_by_key(|s| s.username.as_deref().map(fold) != Some(wanted.clone()));
        let upgrade = if q.conditional && !excluded {
            self.upgrade_for(q.page_url, q.user_name, &candidates, now_ms)?
        } else {
            Upgrade::None
        };
        Ok(CreateCheck {
            excluded,
            candidates,
            upgrade,
        })
    }

    /// Create a passkey and seal it into a login (a new one, or `item_id`,
    /// unless a login somewhere in the vault already holds a passkey for
    /// this rpId + user handle, in which case it replaces that one
    /// instead — see `find_passkey_holder`; a conditional create is denied
    /// then, never a replace). Nothing is stored until the
    /// caller sends `write` and commits it.
    pub fn stage_passkey_create(
        &mut self,
        req: PasskeyCreate<'_>,
        now_ms: i64,
    ) -> Result<StagedPasskey> {
        self.session()?;
        check_challenge(req.challenge)?;
        // The site's automatic upgrade: the consent is a HavenKeys password
        // fill of this very login on this site within the window, checked
        // here, never taken from the extension.
        if req.conditional {
            let item = req.item_id.ok_or(Error::Denied)?;
            let homes = self.passkey_homes(req.page_url, req.top_url)?;
            if self.upgrade_for(req.page_url, req.user_name, &homes, now_ms)? != Upgrade::Auto(item)
            {
                return Err(Error::Denied);
            }
        }
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

        // If some login in the vault already holds a passkey for this rpId
        // + user handle, the write goes there (see `find_passkey_holder`),
        // regardless of `req.item_id`. It is already bound to an rpId this
        // page may use, so the "offered for the page" check below is only
        // needed for a login named by the caller that is not already the
        // holder.
        let holder = self.find_passkey_holder(&ctx.rp_id, req.user_handle)?;
        // An upgrade only ever adds a passkey to the login that was filled.
        // Replacing an existing one (in that login or any other) needs the
        // card.
        if req.conditional && holder.is_some() {
            return Err(Error::Denied);
        }
        let (mut overview, mut details, base) = if let Some(id) = holder {
            self.load_login_for_edit(id, now_ms)?
        } else {
            match req.item_id {
                Some(id) => {
                    let offered = self
                        .find_matches(req.page_url, req.top_url)?
                        .iter()
                        .any(|s| s.id == id);
                    if !offered {
                        return Err(Error::Denied);
                    }
                    self.load_login_for_edit(id, now_ms)?
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
                        auto_sign_in: None,
                    };
                    let (overview, details) =
                        build_item(Uuid::new_v4(), input, None, now_ms, now_ms)?;
                    (overview, details, None)
                }
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
        // One fill grants one silent passkey: spend it now, so a script on
        // the site cannot repeat the create with fresh user handles. Spent
        // even if the write later fails; the user can fill again.
        if req.conditional {
            if let Some(site) = PageUrl::parse(req.page_url).as_ref().and_then(site_of) {
                self.session_mut()?
                    .recent_fills
                    .retain(|f| !(f.item_id == item_id && f.site == site));
            }
        }
        Ok(StagedPasskey {
            write,
            item_id,
            registration,
        })
    }

    /// Does the vault hold any passkey this page may use (its rpId passes
    /// `authorize_rp` for the page)? Nothing else about it is returned. A
    /// login whose details do not open is skipped.
    pub fn has_passkey_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<bool> {
        let session = self.session()?;
        let holders: Vec<Uuid> = session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login && o.has_passkey)
            .map(|o| o.id)
            .collect();
        for id in holders {
            let Ok(ItemDetails::Login { passkeys, .. }) = self.load_details(&id) else {
                continue;
            };
            if passkeys
                .iter()
                .any(|p| authorize_rp(&p.rp_id, page_url, top_url).is_ok())
            {
                return Ok(true);
            }
        }
        Ok(false)
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
