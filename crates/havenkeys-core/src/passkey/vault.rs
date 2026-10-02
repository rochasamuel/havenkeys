//! Passkey operations on an unlocked vault.
//!
//! Same rule as the password path (`fill_for_page`): the page URL is
//! re-checked on every call and nothing the caller claims is trusted. A
//! passkey is bound to its relying-party ID; `authorize_rp` decides whether
//! the page may use that ID, and the stored `rp_id` must equal it. The
//! login's own website rules decide only which logins are offered as a home
//! for a new passkey.

use super::{
    assert, authorize_rp, clean_site_name, register, Assertion, B64Url, NewUser, Passkey,
    Registration, RpContext, CREDENTIAL_ID_LEN, MAX_CHALLENGE_BYTES, MAX_PASSKEYS_PER_LOGIN,
    MAX_USER_HANDLE_BYTES, MIN_CHALLENGE_BYTES,
};
use crate::error::{Error, Result};
use crate::model::{ItemDetails, ItemInput, ItemOverview, ItemType, MatchType, UrlRule};
use crate::origin::{site_of, PageUrl};
use crate::vault::{build_item, Session, StagedWrite, Suggestion, VaultService};
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

pub(super) fn check_challenge(c: &[u8]) -> Result<()> {
    if (MIN_CHALLENGE_BYTES..=MAX_CHALLENGE_BYTES).contains(&c.len()) {
        Ok(())
    } else {
        Err(Error::InvalidInput("invalid challenge"))
    }
}

fn fold(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Stable: the logins with the site's account name (folded) first.
pub(super) fn same_user_first(candidates: &mut [Suggestion], user_name: &str) {
    let wanted = fold(user_name);
    candidates.sort_by_key(|s| s.username.as_deref().map(fold) != Some(wanted.clone()));
}

/// What a site asked a new passkey to hold.
pub(super) struct NewPasskey<'a> {
    pub challenge: &'a [u8],
    pub user_handle: &'a [u8],
    pub user_name: &'a str,
    pub display_name: Option<&'a str>,
}

/// Where a new passkey may go, by the caller's own rules.
pub(super) struct Homes<'a> {
    /// The login the user chose; `None` makes a new login.
    pub chosen: Option<Uuid>,
    /// The logins this caller is offered: a chosen login must be one.
    pub offered: &'a [Uuid],
    /// A conditional create never replaces an existing passkey.
    pub refuse_replace: bool,
}

/// Logins whose overview says they hold a passkey.
fn passkey_logins(session: &Session) -> impl Iterator<Item = &ItemOverview> {
    session
        .overviews
        .values()
        .filter(|o| o.item_type == ItemType::Login && o.has_passkey)
}

/// A new login for a passkey created on `page_url`: titled by its host,
/// with the page's origin as its one whole-site rule and the site's account
/// name, if any, as its username.
pub(super) fn new_login_for_passkey(
    page_url: &str,
    user_name: String,
    now_ms: i64,
) -> Result<(ItemOverview, ItemDetails)> {
    let page = PageUrl::parse(page_url).ok_or(Error::Denied)?;
    let (title, origin) = page.title_and_origin().ok_or(Error::Denied)?;
    let input = ItemInput {
        username: Some(user_name).filter(|u| !u.is_empty()),
        urls: vec![UrlRule {
            url: origin,
            match_type: MatchType::Domain,
        }],
        ..ItemInput::blank(ItemType::Login, title)
    };
    build_item(Uuid::new_v4(), input, None, now_ms, now_ms)
}

impl VaultService {
    /// The first login with a passkey for which `pred` holds. A login whose
    /// details do not open is skipped, not fatal.
    fn find_login_with_passkey(&self, pred: impl Fn(&Passkey) -> bool) -> Result<Option<Uuid>> {
        let session = self.session()?;
        let candidates: Vec<Uuid> = passkey_logins(session).map(|o| o.id).collect();
        for id in candidates {
            if let Ok(ItemDetails::Login { passkeys, .. }) = self.load_details(&id) {
                if passkeys.iter().any(&pred) {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }

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
        self.find_login_with_passkey(|p| p.rp_id == rp_id && p.user_handle.0 == user_handle)
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

    /// Passkeys bound to `ctx.rp_id`, filtered by the site's
    /// `allowCredentials` when it sent any. A login whose details do not
    /// open is skipped, not fatal.
    pub(super) fn passkeys_for(
        &self,
        ctx: &RpContext,
        allow: &[Vec<u8>],
    ) -> Result<Vec<PasskeyMatch>> {
        let session = self.session()?;
        let holders: Vec<(Uuid, String)> = passkey_logins(session)
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
        self.session()?;
        let ctx = authorize_rp(rp_id, page_url, top_url)?;
        self.passkeys_for(&ctx, allow)
    }

    /// The assertion for one passkey bound to `ctx.rp_id`. The challenge was
    /// checked by the caller.
    pub(super) fn assert_for(
        &self,
        item_id: &Uuid,
        credential_id: &[u8],
        ctx: &RpContext,
        challenge: &[u8],
        client_data_hash: Option<&[u8; 32]>,
    ) -> Result<Assertion> {
        let ItemDetails::Login { passkeys, .. } = self.load_details(item_id)? else {
            return Err(Error::Denied);
        };
        let passkey = passkeys
            .iter()
            .find(|p| p.credential_id.0 == credential_id && p.rp_id == ctx.rp_id)
            .ok_or(Error::Denied)?;
        assert(passkey, ctx, challenge, client_data_hash)
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
        self.assert_for(item_id, credential_id, &ctx, challenge, None)
    }

    /// `passkey_assert` for a privileged Android browser that built
    /// `clientDataJSON` itself and sent only its hash (spec §8.1). Never for
    /// an app: an app's hash could name any origin.
    pub fn passkey_assert_with_hash(
        &self,
        item_id: &Uuid,
        credential_id: &[u8],
        rp_id: &str,
        page_url: &str,
        challenge: &[u8],
        client_data_hash: &[u8; 32],
    ) -> Result<Assertion> {
        self.session()?;
        check_challenge(challenge)?;
        let ctx = authorize_rp(rp_id, page_url, None)?;
        self.assert_for(
            item_id,
            credential_id,
            &ctx,
            challenge,
            Some(client_data_hash),
        )
    }

    /// `offered` without the logins that cannot take another passkey.
    pub(super) fn with_room(&self, offered: Vec<Suggestion>) -> Vec<Suggestion> {
        offered
            .into_iter()
            .filter(|s| match self.load_details(&s.id) {
                Ok(ItemDetails::Login { passkeys, .. }) => passkeys.len() < MAX_PASSKEYS_PER_LOGIN,
                _ => false,
            })
            .collect()
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
        let ctx = authorize_rp(q.rp_id, q.page_url, q.top_url)?;
        let excluded = !q.exclude.is_empty() && !self.passkeys_for(&ctx, q.exclude)?.is_empty();
        let mut candidates = self.with_room(self.find_matches(q.page_url, q.top_url)?);
        same_user_first(&mut candidates, q.user_name);
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

    /// Create a passkey for `ctx` and seal it into a login: the one already
    /// holding a passkey for this rpId + user handle anywhere in the vault
    /// (see `find_passkey_holder`), else `homes.chosen` if it is offered,
    /// else a new login from `new_login`. Nothing is stored until the caller
    /// sends `write` and commits it.
    pub(super) fn stage_create_for(
        &mut self,
        ctx: &RpContext,
        new: NewPasskey<'_>,
        homes: Homes<'_>,
        new_login: impl FnOnce(String) -> Result<(ItemOverview, ItemDetails)>,
        now_ms: i64,
    ) -> Result<StagedPasskey> {
        if new.user_handle.is_empty() || new.user_handle.len() > MAX_USER_HANDLE_BYTES {
            return Err(Error::InvalidInput("invalid user handle"));
        }
        let user_name = clean_site_name(Some(new.user_name))?.unwrap_or_default();
        let display_name = clean_site_name(new.display_name)?.filter(|d| !d.is_empty());
        let (passkey, registration) = register(
            ctx,
            new.challenge,
            NewUser {
                user_handle: new.user_handle,
                user_name: user_name.clone(),
                display_name,
            },
            now_ms,
        )?;
        let holder = self.find_passkey_holder(&ctx.rp_id, new.user_handle)?;
        // An upgrade only ever adds a passkey to the login that was filled.
        // Replacing an existing one (in that login or any other) needs the
        // card.
        if homes.refuse_replace && holder.is_some() {
            return Err(Error::Denied);
        }
        // The existing holder wins over the chosen login; a chosen login
        // must be one this caller is offered.
        let (mut overview, mut details, base) = match (holder, homes.chosen) {
            (Some(id), _) => self.load_login_for_edit(id, now_ms)?,
            (None, Some(id)) => {
                if !homes.offered.contains(&id) {
                    return Err(Error::Denied);
                }
                self.load_login_for_edit(id, now_ms)?
            }
            (None, None) => {
                let (overview, details) = new_login(user_name)?;
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
            let homes = self.with_room(self.find_matches(req.page_url, req.top_url)?);
            if self.upgrade_for(req.page_url, req.user_name, &homes, now_ms)? != Upgrade::Auto(item)
            {
                return Err(Error::Denied);
            }
        }
        let ctx = authorize_rp(req.rp_id, req.page_url, req.top_url)?;
        let offered: Vec<Uuid> = self
            .find_matches(req.page_url, req.top_url)?
            .iter()
            .map(|s| s.id)
            .collect();
        let page_url = req.page_url;
        let staged = self.stage_create_for(
            &ctx,
            NewPasskey {
                challenge: req.challenge,
                user_handle: req.user_handle,
                user_name: req.user_name,
                display_name: req.display_name,
            },
            Homes {
                chosen: req.item_id,
                offered: &offered,
                refuse_replace: req.conditional,
            },
            |user_name| new_login_for_passkey(page_url, user_name, now_ms),
            now_ms,
        )?;
        // One fill grants one silent passkey: spend it now, so a script on
        // the site cannot repeat the create with fresh user handles. Spent
        // even if the write later fails; the user can fill again.
        if req.conditional {
            if let Some(site) = PageUrl::parse(req.page_url).as_ref().and_then(site_of) {
                let item_id = staged.item_id;
                self.session_mut()?
                    .recent_fills
                    .retain(|f| !(f.item_id == item_id && f.site == site));
            }
        }
        Ok(staged)
    }

    /// Does the vault hold any passkey this page may use (its rpId passes
    /// `authorize_rp` for the page)? Nothing else about it is returned. A
    /// login whose details do not open is skipped.
    pub fn has_passkey_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<bool> {
        Ok(self
            .find_login_with_passkey(|p| authorize_rp(&p.rp_id, page_url, top_url).is_ok())?
            .is_some())
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
            ItemDetails::SecureNote { .. } | ItemDetails::Identity(_) | ItemDetails::Card(_) => {
                Ok(Vec::new())
            }
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
