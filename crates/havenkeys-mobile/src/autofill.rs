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
use havenkeys_core::model::{ItemType, SecretField};
use havenkeys_core::vault::{FillCredentials, MatchKind, Suggestion};
use havenkeys_core::{Error, SecretString};
use std::fmt;

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

/// What kind of site-bound item a match is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AutofillKind {
    Login,
    BankAccount,
}

/// A bank account's overview, shown instead of a username: the bank, the
/// agência and the last 3 characters of the conta (`45-6`). `Debug` prints
/// none of them.
#[derive(uniffi::Record)]
pub struct BankSummaryRecord {
    pub bank: Option<String>,
    pub branch: Option<String>,
    pub account_last3: Option<String>,
}

impl fmt::Debug for BankSummaryRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BankSummaryRecord(<redacted>)")
    }
}

/// The numbers a bank form is filled with. `Debug` prints none of them.
#[derive(uniffi::Record)]
pub struct BankFillRecord {
    pub branch: String,
    pub branch_digit: Option<String>,
    pub account: String,
    pub account_digit: Option<String>,
}

impl fmt::Debug for BankFillRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BankFillRecord(<redacted>)")
    }
}

#[derive(uniffi::Record)]
pub struct AutofillMatch {
    pub id: String,
    pub title: String,
    pub username: Option<String>,
    pub has_totp: bool,
    pub kind: AutofillKind,
    /// Set for a bank account only.
    #[uniffi(default = None)]
    pub bank: Option<BankSummaryRecord>,
}

/// A bank account's has no username, its password and `bank`.
#[derive(uniffi::Record)]
pub struct FillValues {
    pub username: Option<String>,
    pub password: Option<String>,
    #[uniffi(default = None)]
    pub bank: Option<BankFillRecord>,
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
        kind: match s.kind {
            MatchKind::Login => AutofillKind::Login,
            MatchKind::BankAccount => AutofillKind::BankAccount,
        },
        bank: s.bank.as_ref().map(|b| BankSummaryRecord {
            bank: b.bank.clone(),
            branch: b.branch.clone(),
            account_last3: b.account_last3.clone(),
        }),
    }
}

fn to_values(c: FillCredentials) -> FillValues {
    let text = |s: SecretString| s.expose().to_owned();
    FillValues {
        username: c.username,
        password: c.password.map(text),
        bank: c.bank.map(|b| BankFillRecord {
            branch: b.branch,
            branch_digit: b.branch_digit,
            account: text(b.account),
            account_digit: b.account_digit.map(text),
        }),
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
                self.client
                    .vault()?
                    .find_matches(&page_url, None, havenkeys_client::now_ms())?
            }
            FillTarget::App(app) => {
                let hosts = self.verified_hosts(&app)?;
                self.client
                    .vault()?
                    .matches_for_app(&app, &hosts, havenkeys_client::now_ms())?
            }
        };
        Ok(found.iter().map(to_match).collect())
    }

    /// The saved site a browser page only looks like (spec
    /// 2026-10-13-lookalike-warning §6), for the warning row and the save
    /// sheet; `None` for apps, which are identified by package and signing
    /// certificate, not a domain. Answered while frozen: a warning, not a
    /// fill. No secrets.
    pub fn lookalike_for(&self, target: TargetFacts) -> MobileResult<Option<String>> {
        self.unlocked()?;
        Ok(match self.target(&target)? {
            FillTarget::Browser { page_url } => {
                self.client.vault()?.lookalike_for_page(&page_url, None)?
            }
            FillTarget::App(_) => None,
        })
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

    /// "Search HavenKeys…": any login or bank account, by title, username,
    /// website, bank or agência. No
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
                // authorization, so it reads the item directly.
                let o = vault.get_item(&item)?;
                if o.item_type == ItemType::BankAccount {
                    let f = vault.bank_fields(item)?;
                    FillCredentials {
                        username: None,
                        bank: f.values(),
                        password: f.password,
                    }
                } else {
                    let password = match vault.reveal(&item, SecretField::Password) {
                        Ok(p) => Some(p),
                        Err(Error::NotFound) => None,
                        Err(e) => return Err(e.into()),
                    };
                    FillCredentials {
                        username: o.username.clone(),
                        password,
                        bank: None,
                    }
                }
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
            bank: None,
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

    #[test]
    fn a_browser_look_alike_names_the_saved_site() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        add_login(&v, "https://github.com");
        // Chrome may report the domain in Unicode or punycode; both parse.
        for domain in ["g\u{0456}thub.com", "xn--gthub-n2e.com"] {
            assert_eq!(
                v.lookalike_for(chrome(domain, Some("https")))
                    .unwrap()
                    .as_deref(),
                Some("github.com"),
                "{domain}"
            );
        }
        assert_eq!(
            v.lookalike_for(chrome("github.com", Some("https")))
                .unwrap(),
            None
        );
        assert_eq!(
            v.lookalike_for(chrome("evilgithub.com", Some("https")))
                .unwrap(),
            None
        );
    }

    #[test]
    fn apps_are_never_look_alikes() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        add_login(&v, "https://github.com");
        assert_eq!(v.lookalike_for(app(1)).unwrap(), None);
    }

    #[test]
    fn look_alikes_need_an_unlocked_vault_and_are_answered_when_frozen() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        add_login(&v, "https://github.com");
        let page = || chrome("g\u{0456}thub.com", Some("https"));
        freeze(&v);
        assert_eq!(
            v.lookalike_for(page()).unwrap().as_deref(),
            Some("github.com")
        );
        // Leaves the vault locked, so the call after it is the locked case.
        overdue_refuses(&v, &seen, |v| v.lookalike_for(page()));
        assert!(v.lookalike_for(page()).is_err());
    }

    fn itau() -> TargetFacts {
        chrome("itau.com.br", Some("https"))
    }

    fn no_asset_links(v: &MobileVault) {
        let mut s = v.settings().unwrap();
        s.asset_links = false;
        v.update_settings(s).unwrap();
    }

    #[test]
    fn a_bank_is_offered_on_its_site_in_chrome() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = crate::testing::seed_bank(&v, "Conta Itaú");
        let m = v.autofill_matches(itau()).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, id);
        assert_eq!(m[0].kind, AutofillKind::BankAccount);
        assert_eq!(m[0].username, None);
        let b = m[0].bank.as_ref().unwrap();
        assert_eq!(b.bank.as_deref(), Some("Itaú"));
        assert_eq!(b.branch.as_deref(), Some("1234"));
        assert_eq!(b.account_last3.as_deref(), Some("45-6"));
        // A login is still a login, with no bank summary.
        add_login(&v, "https://github.com");
        let m = v
            .autofill_matches(chrome("github.com", Some("https")))
            .unwrap();
        assert_eq!(
            (m[0].kind, m[0].bank.is_none()),
            (AutofillKind::Login, true)
        );
    }

    #[test]
    fn a_bank_fill_gives_values_and_password() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = crate::testing::seed_bank(&v, "Conta Itaú");
        let f = v.autofill_fill(id, itau()).unwrap();
        assert_eq!(f.username, None);
        assert_eq!(f.password.as_deref(), Some("senha-itau-1"));
        let b = f.bank.unwrap();
        assert_eq!(
            (b.branch.as_str(), b.branch_digit.as_deref()),
            ("1234", Some("5"))
        );
        assert_eq!(
            (b.account.as_str(), b.account_digit.as_deref()),
            ("12345", Some("6"))
        );
        let shown = format!("{b:?}");
        assert!(
            !shown.contains("12345") && !shown.contains("1234"),
            "{shown}"
        );
    }

    #[test]
    fn a_bank_is_denied_on_another_site() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = crate::testing::seed_bank(&v, "Conta Itaú");
        for domain in ["itau.com.br.evil.com", "evilitau.com.br", "github.com"] {
            let page = || chrome(domain, Some("https"));
            assert!(v.autofill_matches(page()).unwrap().is_empty(), "{domain}");
            assert!(v.autofill_fill(id.clone(), page()).is_err(), "{domain}");
        }
        // Nor in an app it was never bound to.
        no_asset_links(&v);
        assert!(v.autofill_matches(app(1)).unwrap().is_empty());
        assert!(v.autofill_fill(id, app(1)).is_err());
    }

    #[test]
    fn a_frozen_account_offers_no_bank() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = crate::testing::seed_bank(&v, "Conta Itaú");
        no_asset_links(&v);
        freeze(&v);
        assert!(v.autofill_matches(itau()).unwrap().is_empty());
        assert!(v.autofill_search("itaú".into()).unwrap().is_empty());
        let err = v.autofill_fill(id.clone(), itau()).err().unwrap();
        assert_eq!(code(err), "account_frozen");
        let err = v.autofill_bind_and_fill(id, app(1)).err().unwrap();
        assert_eq!(code(err), "account_frozen");
    }

    #[test]
    fn a_bank_found_by_search_binds_to_the_app_and_fills() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = crate::testing::seed_bank(&v, "Conta Itaú");
        no_asset_links(&v);
        let found = v.autofill_search("itaú".into()).unwrap();
        assert_eq!(found[0].id, id);
        assert_eq!(found[0].kind, AutofillKind::BankAccount);
        // Offline: filled once from the user's confirmation, not stored.
        let once = v.autofill_bind_and_fill(id.clone(), app(1)).unwrap();
        assert!(!once.saved);
        assert_eq!(once.values.username, None);
        assert_eq!(once.values.password.as_deref(), Some("senha-itau-1"));
        let b = once.values.bank.unwrap();
        assert_eq!((b.branch.as_str(), b.account.as_str()), ("1234", "12345"));
        assert_eq!(v.frequently_used(6).unwrap()[0].id, id);
        // Stored (as when online): the app then gets it through its binding.
        let staged = {
            let item = uuid::Uuid::parse_str(&id).unwrap();
            let FillTarget::App(identity) = v.target(&app(1)).unwrap() else {
                panic!("not an app")
            };
            v.client
                .vault()
                .unwrap()
                .stage_bind_app(&item, &identity, 2)
                .unwrap()
        };
        v.client.vault().unwrap().commit_write(staged, 2).unwrap();
        assert_eq!(v.autofill_matches(app(1)).unwrap()[0].id, id);
        let f = v.autofill_fill(id, app(1)).unwrap();
        assert_eq!(f.bank.unwrap().account_digit.as_deref(), Some("6"));
    }

    #[test]
    fn autofill_search_finds_banks_by_title_and_bank() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = crate::testing::seed_bank(&v, "Conta salário");
        for query in ["salário", "itaú", "1234"] {
            let found = v.autofill_search(query.into()).unwrap();
            assert_eq!(found.len(), 1, "{query}");
            assert_eq!(found[0].id, id);
        }
        // Never by the full account number.
        assert!(v.autofill_search("12345".into()).unwrap().is_empty());
    }
}
