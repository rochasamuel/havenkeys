//! Passkeys for Android apps (spec 2026-10-01-android-app §8.1). An app has
//! no page URL: it may use an RP ID only when that site's Digital Asset
//! Links file grants it `common.get_login_creds`, and its origin is
//! `android:apk-key-hash:<cert>`. The cache is read here; the caller only
//! refreshes it. A new passkey's login gets the RP's website, which vouched
//! for the app, so the same passkey works in browsers.

use super::vault::{check_challenge, new_login_for_passkey, same_user_first, Homes, NewPasskey};
use super::{
    authorize_rp_for_app, Assertion, CreateCheck, PasskeyMatch, RpContext, StagedPasskey, Upgrade,
};
use crate::app_target::AppIdentity;
use crate::asset_links::AssetLinksCache;
use crate::error::Result;
use crate::local::LocalSlot;
use crate::vault::VaultService;
use uuid::Uuid;

/// An app's `create()` request, as far as the checks before saving need it.
pub struct AppCreateQuery<'a> {
    pub rp_id: &'a str,
    pub app: &'a AppIdentity,
    /// Hosts whose Digital Asset Links vouch for the app (`asset_links`).
    pub verified_hosts: &'a [String],
    pub user_name: &'a str,
    pub exclude: &'a [Vec<u8>],
}

/// An app's `create()` request, after the mobile crate decoded it.
pub struct AppPasskeyCreate<'a> {
    pub rp_id: &'a str,
    pub app: &'a AppIdentity,
    pub verified_hosts: &'a [String],
    pub challenge: &'a [u8],
    pub user_handle: &'a [u8],
    pub user_name: &'a str,
    pub display_name: Option<&'a str>,
    /// Attach to this login (it must be matched to the app); `None` makes a
    /// new login.
    pub item_id: Option<Uuid>,
}

impl VaultService {
    fn authorize_app_rp(&self, rp_id: &str, app: &AppIdentity, now_ms: i64) -> Result<RpContext> {
        self.session()?;
        // A cache that does not open vouches for nothing.
        let cache: AssetLinksCache = self
            .read_local(LocalSlot::AssetLinks)
            .ok()
            .flatten()
            .unwrap_or_default();
        authorize_rp_for_app(rp_id, app, &cache, now_ms)
    }

    pub fn find_passkeys_for_app(
        &self,
        rp_id: &str,
        app: &AppIdentity,
        allow: &[Vec<u8>],
        now_ms: i64,
    ) -> Result<Vec<PasskeyMatch>> {
        let ctx = self.authorize_app_rp(rp_id, app, now_ms)?;
        self.passkeys_for(&ctx, allow)
    }

    pub fn passkey_assert_for_app(
        &self,
        item_id: &Uuid,
        credential_id: &[u8],
        rp_id: &str,
        app: &AppIdentity,
        challenge: &[u8],
        now_ms: i64,
    ) -> Result<Assertion> {
        self.session()?;
        check_challenge(challenge)?;
        let ctx = self.authorize_app_rp(rp_id, app, now_ms)?;
        self.assert_for(item_id, credential_id, &ctx, challenge, None)
    }

    /// Before asking the user: is one of the app's `excludeCredentials`
    /// already here, and which logins matched to the app could hold the new
    /// passkey? No automatic upgrade for apps.
    pub fn check_passkey_create_for_app(
        &self,
        q: &AppCreateQuery<'_>,
        now_ms: i64,
    ) -> Result<CreateCheck> {
        let ctx = self.authorize_app_rp(q.rp_id, q.app, now_ms)?;
        let excluded = !q.exclude.is_empty() && !self.passkeys_for(&ctx, q.exclude)?.is_empty();
        let mut candidates = self.with_room(self.matches_for_app(q.app, q.verified_hosts)?);
        same_user_first(&mut candidates, q.user_name);
        Ok(CreateCheck {
            excluded,
            candidates,
            upgrade: Upgrade::None,
        })
    }

    pub fn stage_passkey_create_for_app(
        &mut self,
        req: AppPasskeyCreate<'_>,
        now_ms: i64,
    ) -> Result<StagedPasskey> {
        self.session()?;
        check_challenge(req.challenge)?;
        let ctx = self.authorize_app_rp(req.rp_id, req.app, now_ms)?;
        let offered: Vec<Uuid> = self
            .matches_for_app(req.app, req.verified_hosts)?
            .iter()
            .map(|s| s.id)
            .collect();
        let site = format!("https://{}/", ctx.rp_id);
        self.stage_create_for(
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
                refuse_replace: false,
            },
            |user_name| new_login_for_passkey(&site, user_name, now_ms),
            now_ms,
        )
    }
}
