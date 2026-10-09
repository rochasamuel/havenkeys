//! Android Autofill (spec 2026-10-01-android-app §7). Kotlin reports the
//! facts Android gave it (caller package, signing certificates, the
//! structure's web domain and scheme); everything about what may be filled
//! is decided here, on every call.

use crate::asset_links_fetch::{client, fetch_from, url_for};
use crate::error::MobileResult;
use crate::items::parse_id;
use crate::vault::MobileVault;
use havenkeys_core::app_target::{classify, AppIdentity, FillTarget};
use havenkeys_core::asset_links::{candidate_hosts, AssetLinksCache};
use havenkeys_core::local::LocalSlot;
use havenkeys_core::model::SecretField;
use havenkeys_core::vault::{FillCredentials, Suggestion};
use havenkeys_core::Error;

#[derive(Clone, uniffi::Record)]
pub struct TargetFacts {
    pub package_name: String,
    pub signing_certs: Vec<Vec<u8>>,
    pub web_domain: Option<String>,
    pub web_scheme: Option<String>,
}

/// One frame of a browser page, as the structure reported it. No domain:
/// the page itself.
#[derive(Clone, uniffi::Record)]
pub struct FrameFacts {
    pub web_domain: Option<String>,
    pub web_scheme: Option<String>,
}

/// The URL of `frame` in a browser tab showing `top`.
pub(crate) fn frame_url(top: &str, frame: &FrameFacts) -> Option<String> {
    match &frame.web_domain {
        None => Some(top.to_owned()),
        Some(domain) => {
            havenkeys_core::app_target::browser_page_url(domain, frame.web_scheme.as_deref())
        }
    }
}

#[derive(uniffi::Enum)]
pub enum TargetKind {
    Browser,
    App,
}

#[derive(uniffi::Record)]
pub struct AutofillMatch {
    pub id: String,
    pub title: String,
    pub username: Option<String>,
    pub has_totp: bool,
}

#[derive(uniffi::Record)]
pub struct FillValues {
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(uniffi::Record)]
pub struct BoundFill {
    pub values: FillValues,
    pub saved: bool,
}

pub(crate) fn unix_seconds() -> u64 {
    u64::try_from(havenkeys_client::now_ms() / 1000).unwrap_or(0)
}

pub(crate) fn to_match(s: &Suggestion) -> AutofillMatch {
    AutofillMatch {
        id: s.id.to_string(),
        title: s.title.clone(),
        username: s.username.clone(),
        has_totp: s.has_totp,
    }
}

fn to_values(c: FillCredentials) -> FillValues {
    FillValues {
        username: c.username,
        password: c.password.map(|p| p.expose().to_owned()),
    }
}

impl MobileVault {
    pub(crate) fn target(&self, t: &TargetFacts) -> MobileResult<FillTarget> {
        Ok(classify(
            &self.own_package,
            &t.package_name,
            &t.signing_certs,
            t.web_domain.as_deref(),
            t.web_scheme.as_deref(),
        )?)
    }

    /// Hosts that vouch for `app`, refreshing stale cache entries first when
    /// the setting allows.
    pub(crate) fn verified_hosts(&self, app: &AppIdentity) -> MobileResult<Vec<String>> {
        self.verified_hosts_asking(app, &[])
    }

    /// Hosts that vouch for `app`, after refreshing the stale ones among the
    /// vault's candidates and `also` (when the setting allows). Bounded by
    /// `FETCH_TIMEOUT` per request, run in parallel.
    pub(crate) fn verified_hosts_asking(
        &self,
        app: &AppIdentity,
        also: &[String],
    ) -> MobileResult<Vec<String>> {
        let now = havenkeys_client::now_ms();
        // Read before taking the vault guard: it locks the vault itself.
        let asset_links = self.device_settings().asset_links;
        let (mut cache, stale) = {
            let vault = self.client.vault()?;
            let cache: AssetLinksCache = vault
                .read_local(LocalSlot::AssetLinks)
                .ok()
                .flatten()
                .unwrap_or_default();
            let stale: Vec<String> = if asset_links {
                let mut hosts = candidate_hosts(app.package(), &vault.login_hosts()?);
                for host in also {
                    if !hosts.contains(host) {
                        hosts.push(host.clone());
                    }
                }
                hosts
                    .into_iter()
                    .filter(|h| !cache.is_fresh(h, now))
                    .collect()
            } else {
                Vec::new()
            };
            (cache, stale)
        };
        // The vault guard is released here: the fetches below never hold it.
        if !stale.is_empty() {
            if let Some(http) = client() {
                let fetched = self.block_on(async {
                    let mut set = tokio::task::JoinSet::new();
                    for (i, host) in stale.iter().enumerate() {
                        let (http, url) = (http.clone(), url_for(host));
                        set.spawn(async move {
                            let statements = match url {
                                Some(url) => fetch_from(&http, &url).await,
                                None => None,
                            };
                            (i, statements)
                        });
                    }
                    let mut fetched = vec![None; stale.len()];
                    // A failed task leaves its host as a failure; the others
                    // are still collected.
                    while let Some(joined) = set.join_next().await {
                        if let Ok((i, statements)) = joined {
                            fetched[i] = statements;
                        }
                    }
                    fetched
                });
                for (host, statements) in stale.iter().zip(fetched) {
                    cache.record(host, now, statements);
                }
                // The vault may have locked while fetching: then nothing is
                // written and the caller's next vault access refuses.
                if let Ok(vault) = self.client.vault() {
                    let _ = vault.write_local(LocalSlot::AssetLinks, &cache);
                }
            }
        }
        Ok(cache.verified_hosts(app, now))
    }
}

#[uniffi::export]
impl MobileVault {
    /// Not a secret; answered while locked so the app can pick its UI.
    pub fn autofill_target_kind(&self, target: TargetFacts) -> MobileResult<TargetKind> {
        Ok(match self.target(&target)? {
            FillTarget::Browser { .. } => TargetKind::Browser,
            FillTarget::App(_) => TargetKind::App,
        })
    }

    pub fn confirm_before_filling(&self) -> bool {
        self.device_settings().confirm_before_filling
    }

    /// Empty while the account is frozen: nothing is offered.
    pub fn autofill_matches(&self, target: TargetFacts) -> MobileResult<Vec<AutofillMatch>> {
        self.unlocked()?;
        if self.is_frozen()? {
            return Ok(Vec::new());
        }
        let found = match self.target(&target)? {
            FillTarget::Browser { page_url } => {
                self.client.vault()?.find_matches(&page_url, None)?
            }
            FillTarget::App(app) => {
                let hosts = self.verified_hosts(&app)?;
                self.client.vault()?.matches_for_app(&app, &hosts)?
            }
        };
        Ok(found.iter().map(to_match).collect())
    }

    pub fn autofill_fill(&self, id: String, target: TargetFacts) -> MobileResult<FillValues> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        self.refuse_when_frozen()?;
        let creds = match self.target(&target)? {
            FillTarget::Browser { page_url } => self.client.vault()?.fill_for_page(
                &id,
                &page_url,
                None,
                havenkeys_client::now_ms(),
            )?,
            FillTarget::App(app) => {
                let hosts = self.verified_hosts(&app)?;
                self.client.vault()?.fill_for_app(&id, &app, &hosts)?
            }
        };
        Ok(to_values(creds))
    }

    /// Answered while frozen too: the code is the user's way in, and Android
    /// never submits on its own (there is no auto-submit here).
    pub fn autofill_totp(&self, id: String, target: TargetFacts) -> MobileResult<String> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        let code = match self.target(&target)? {
            FillTarget::Browser { page_url } => {
                self.client
                    .vault()?
                    .totp_for_page(&id, &page_url, None, unix_seconds())?
            }
            FillTarget::App(app) => {
                let hosts = self.verified_hosts(&app)?;
                self.client
                    .vault()?
                    .totp_for_app(&id, &app, &hosts, unix_seconds())?
            }
        };
        Ok(code.code.expose().to_owned())
    }

    /// "Search HavenKeys…": any login, by title, username or website. No
    /// secrets; filling one goes through `autofill_bind_and_fill`.
    /// Empty while the account is frozen.
    pub fn autofill_search(&self, query: String) -> MobileResult<Vec<AutofillMatch>> {
        self.unlocked()?;
        if self.is_frozen()? {
            return Ok(Vec::new());
        }
        Ok(self
            .client
            .vault()?
            .search_logins(&query)?
            .iter()
            .map(to_match)
            .collect())
    }

    /// The user confirmed "Use <login> in <app>?". Stores the binding when
    /// online (an item write); offline, fills this once and stores nothing.
    /// Never for a browser: a site that does not match is never filled.
    /// Refused while frozen (no "fill once" fallback).
    pub fn autofill_bind_and_fill(
        &self,
        id: String,
        target: TargetFacts,
    ) -> MobileResult<BoundFill> {
        let item = parse_id(&id)?;
        self.unlocked()?;
        self.refuse_when_frozen()?;
        let FillTarget::App(app) = self.target(&target)? else {
            return Err(Error::Denied.into());
        };
        let staged =
            self.client
                .vault()?
                .stage_bind_app(&item, &app, havenkeys_client::now_ms())?;
        let saved = self.client.is_online() && self.block_on(self.client.push(staged)).is_ok();
        let creds = {
            let vault = self.client.vault()?;
            if saved {
                vault.fill_for_app(&item, &app, &[])?
            } else {
                // Not stored: the user's confirmation is this fill's only
                // authorization, so it reads the login directly.
                let username = vault.get_item(&item)?.username.clone();
                let password = match vault.reveal(&item, SecretField::Password) {
                    Ok(p) => Some(p),
                    Err(Error::NotFound) => None,
                    Err(e) => return Err(e.into()),
                };
                FillCredentials { username, password }
            }
        };
        self.note_use(&item);
        Ok(BoundFill {
            values: to_values(creds),
            saved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{code, freeze, overdue_refuses, unlocked};
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use havenkeys_core::SecretString;

    const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";

    fn chrome(domain: &str, scheme: Option<&str>) -> TargetFacts {
        TargetFacts {
            package_name: "com.android.chrome".into(),
            signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(CHROME)
                .unwrap()
                .to_vec()],
            web_domain: Some(domain.into()),
            web_scheme: scheme.map(Into::into),
        }
    }

    fn app(cert: u8) -> TargetFacts {
        TargetFacts {
            package_name: "com.github.android".into(),
            signing_certs: vec![vec![cert; 32]],
            web_domain: None,
            web_scheme: None,
        }
    }

    fn add_login(v: &MobileVault, url: &str) -> String {
        let input = ItemInput {
            tags: None,
            item_type: ItemType::Login,
            title: "GitHub".into(),
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: url.into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
            totp: SecretUpdate::Set(SecretString::from("JBSWY3DPEHPK3PXP")),
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: None,
            sections: None,
        };
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, 1).unwrap();
        let id = staged.item_id;
        vault.commit_write(staged, 1).unwrap();
        id.to_string()
    }

    #[test]
    fn chrome_on_the_right_site_fills() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        let m = v
            .autofill_matches(chrome("github.com", Some("https")))
            .unwrap();
        assert_eq!(m.len(), 1);
        let f = v
            .autofill_fill(id.clone(), chrome("github.com", Some("https")))
            .unwrap();
        assert_eq!(f.password.as_deref(), Some("hunter2hunter2"));
        assert_eq!(
            v.autofill_totp(id, chrome("github.com", Some("https")))
                .unwrap()
                .len(),
            6
        );
    }

    #[test]
    fn chrome_on_a_look_alike_gets_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        for domain in [
            "github.com.evil.com",
            "evilgithub.com",
            "github-login.example.com",
        ] {
            assert!(v
                .autofill_matches(chrome(domain, Some("https")))
                .unwrap()
                .is_empty());
            assert!(v
                .autofill_fill(id.clone(), chrome(domain, Some("https")))
                .is_err());
        }
    }

    #[test]
    fn an_https_login_is_not_filled_on_http_or_without_a_scheme() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        assert!(v
            .autofill_fill(id.clone(), chrome("github.com", Some("http")))
            .is_err());
        assert!(v.autofill_fill(id, chrome("github.com", None)).is_err());
    }

    #[test]
    fn an_app_gets_a_login_only_after_binding() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        let mut s = v.settings().unwrap();
        s.asset_links = false;
        v.update_settings(s).unwrap();
        assert!(v.autofill_matches(app(1)).unwrap().is_empty());
        assert!(v.autofill_fill(id.clone(), app(1)).is_err());
        // Offline: the fill happens once and nothing is stored.
        let once = v.autofill_bind_and_fill(id.clone(), app(1)).unwrap();
        assert!(!once.saved);
        assert_eq!(once.values.password.as_deref(), Some("hunter2hunter2"));
        assert!(v.autofill_matches(app(1)).unwrap().is_empty());
    }

    #[test]
    fn prefetching_values_records_nothing_but_a_confirmed_binding_does() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        let page = || chrome("github.com", Some("https"));
        v.autofill_fill(id.clone(), page()).unwrap();
        v.autofill_totp(id.clone(), page()).unwrap();
        assert!(v.frequently_used(6).unwrap().is_empty());
        v.autofill_bind_and_fill(id.clone(), app(1)).unwrap();
        assert_eq!(v.frequently_used(6).unwrap()[0].id, id);
    }

    #[test]
    fn card_and_identity_values_and_item_views_record_nothing() {
        use crate::cards::{CardFrameRoles, CardRole};
        use crate::identity_fill::IdentityRole;
        use crate::testing::{seed_card, seed_identity};
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let card = seed_card(&v, "Visa", "4111111111111111");
        seed_identity(&v);
        let frame = || FrameFacts {
            web_domain: None,
            web_scheme: None,
        };
        let shop = || chrome("shop.example.com", Some("https"));
        let values = v
            .autofill_card_values(
                card.clone(),
                shop(),
                vec![CardFrameRoles {
                    frame: frame(),
                    roles: vec![CardRole::Number],
                }],
            )
            .unwrap();
        assert_eq!(values[0].len(), 1);
        let identity = v
            .autofill_identity_values(shop(), frame(), vec![IdentityRole::FirstName], false)
            .unwrap();
        assert_eq!(identity.len(), 1);
        v.item_view(card.clone()).unwrap();
        v.reveal(card, "card.number".into()).unwrap();
        let login = add_login(&v, "https://github.com");
        v.item_view(login.clone()).unwrap();
        v.reveal(login, "password".into()).unwrap();
        assert!(v.frequently_used(6).unwrap().is_empty());
    }

    #[test]
    fn manual_binding_is_for_apps_only() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        assert!(v
            .autofill_bind_and_fill(id, chrome("evil.com", Some("https")))
            .is_err());
    }

    #[test]
    fn havenkeys_itself_and_bad_facts_get_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        add_login(&v, "https://github.com");
        let own = TargetFacts {
            package_name: "net.havenkeys.android".into(),
            ..app(1)
        };
        assert!(v.autofill_target_kind(own).is_err());
        let bad = TargetFacts {
            signing_certs: vec![vec![1; 5]],
            ..app(1)
        };
        assert!(v.autofill_matches(bad).is_err());
    }

    #[test]
    fn an_overdue_vault_locks_before_any_fill() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        let site = || chrome("github.com", Some("https"));
        overdue_refuses(&v, &seen, |v| v.autofill_matches(site()));
        overdue_refuses(&v, &seen, |v| v.autofill_fill(id.clone(), site()));
        overdue_refuses(&v, &seen, |v| v.autofill_totp(id.clone(), site()));
        overdue_refuses(&v, &seen, |v| v.autofill_search("git".into()));
        overdue_refuses(&v, &seen, |v| v.autofill_bind_and_fill(id.clone(), app(1)));
    }

    #[test]
    fn a_locked_vault_fills_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        v.lock();
        assert!(v
            .autofill_matches(chrome("github.com", Some("https")))
            .is_err());
        assert!(v
            .autofill_fill(id, chrome("github.com", Some("https")))
            .is_err());
        // The target kind is not a secret and is answered while locked.
        assert!(matches!(
            v.autofill_target_kind(app(1)).unwrap(),
            TargetKind::App
        ));
    }

    #[test]
    fn a_frozen_account_is_offered_nothing_but_the_code() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v, "https://github.com");
        let page = || chrome("github.com", Some("https"));
        let mut s = v.settings().unwrap();
        s.asset_links = false;
        v.update_settings(s).unwrap();
        freeze(&v);
        assert!(v.autofill_matches(page()).unwrap().is_empty());
        assert!(v.autofill_search("git".into()).unwrap().is_empty());
        let err = v.autofill_fill(id.clone(), page()).err().unwrap();
        assert_eq!(code(err), "account_frozen");
        // No "fill once" fallback while frozen, even offline.
        let err = v.autofill_bind_and_fill(id.clone(), app(1)).err().unwrap();
        assert_eq!(code(err), "account_frozen");
        assert!(v.frequently_used(6).unwrap().is_empty());
        assert_eq!(v.autofill_totp(id, page()).unwrap().len(), 6);
    }
}
