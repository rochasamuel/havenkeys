//! Filling Android apps (spec 2026-10-01-android-app §7.2). An app target
//! matches a login only if the user bound that app (package and signing
//! certificate) to it, or if one of the login's websites vouches for the app
//! through Digital Asset Links. `verified_hosts` is that second set, computed
//! by the caller from `asset_links` — never from anything the app reported.

use crate::app_target::{AppIdentity, CERT_LEN};
use crate::error::{Error, Result};
use crate::model::{AppBinding, ItemDetails, ItemOverview, ItemType, MAX_APP_BINDINGS};
use crate::origin::{match_item, MatchStrength, PageUrl};
use crate::totp::TotpCode;
use crate::vault::{FillCredentials, StagedWrite, Suggestion, VaultService};
use data_encoding::HEXLOWER_PERMISSIVE;
use std::collections::BTreeSet;
use uuid::Uuid;

fn suggestion(o: &ItemOverview, strength: MatchStrength) -> Suggestion {
    Suggestion {
        id: o.id,
        title: o.title.clone(),
        username: o.username.clone(),
        has_totp: o.has_totp,
        strength,
        account: o.sign_in_with.as_ref().and_then(|s| s.account.clone()),
        provider: o.sign_in_with.as_ref().map(|s| s.provider),
    }
}

fn binding_cert(b: &AppBinding) -> Option<[u8; CERT_LEN]> {
    HEXLOWER_PERMISSIVE
        .decode(b.cert_sha256.as_bytes())
        .ok()
        .and_then(|v| <[u8; CERT_LEN]>::try_from(v.as_slice()).ok())
}

impl VaultService {
    pub fn matches_for_app(
        &self,
        app: &AppIdentity,
        verified_hosts: &[String],
    ) -> Result<Vec<Suggestion>> {
        let session = self.session()?;
        let mut out = Vec::new();
        for o in session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login)
        {
            if let Some(strength) = self.app_match(o, app, verified_hosts)? {
                out.push(suggestion(o, strength));
            }
        }
        out.sort_by_cached_key(|s| (s.strength, s.title.to_lowercase(), s.id));
        Ok(out)
    }

    fn app_match(
        &self,
        o: &ItemOverview,
        app: &AppIdentity,
        verified_hosts: &[String],
    ) -> Result<Option<MatchStrength>> {
        // Bound by the user: as strong as an exact page match.
        if self.bound_to(&o.id, app)? {
            return Ok(Some(MatchStrength::ExactUrl));
        }
        Ok(verified_hosts
            .iter()
            .filter_map(|h| PageUrl::parse(&format!("https://{h}/")))
            .filter_map(|page| match_item(o, &page))
            .min())
    }

    fn bound_to(&self, id: &Uuid, app: &AppIdentity) -> Result<bool> {
        match self.load_details(id) {
            Ok(ItemDetails::Login { app_bindings, .. }) => Ok(app_bindings.iter().any(|b| {
                b.package == app.package() && binding_cert(b).is_some_and(|c| app.signed_by(&c))
            })),
            Ok(_) => Ok(false),
            Err(Error::Locked) => Err(Error::Locked),
            // A damaged item is simply not offered.
            Err(_) => Ok(false),
        }
    }

    fn authorize_for_app(
        &self,
        id: &Uuid,
        app: &AppIdentity,
        verified_hosts: &[String],
    ) -> Result<&ItemOverview> {
        let o = self.session()?.overviews.get(id).ok_or(Error::NotFound)?;
        if o.item_type != ItemType::Login || self.app_match(o, app, verified_hosts)?.is_none() {
            return Err(Error::Denied);
        }
        Ok(o)
    }

    pub fn fill_for_app(
        &self,
        id: &Uuid,
        app: &AppIdentity,
        verified_hosts: &[String],
    ) -> Result<FillCredentials> {
        let username = self
            .authorize_for_app(id, app, verified_hosts)?
            .username
            .clone();
        let password = match self.load_details(id)? {
            ItemDetails::Login { password, .. } => password,
            _ => return Err(Error::Denied),
        };
        Ok(FillCredentials { username, password })
    }

    pub fn totp_for_app(
        &self,
        id: &Uuid,
        app: &AppIdentity,
        verified_hosts: &[String],
        unix_seconds: u64,
    ) -> Result<TotpCode> {
        self.authorize_for_app(id, app, verified_hosts)?;
        self.totp_code(id, unix_seconds)
    }

    /// The user confirmed "Use <login> in <app>?". One binding per signing
    /// certificate, so a multi-signer app matches with any of them.
    pub fn stage_bind_app(&self, id: &Uuid, app: &AppIdentity, now_ms: i64) -> Result<StagedWrite> {
        let existing = self.get_item(id)?;
        if existing.item_type != ItemType::Login {
            return Err(Error::Denied);
        }
        let mut details = self.load_details(id)?;
        let ItemDetails::Login { app_bindings, .. } = &mut details else {
            return Err(Error::Corrupted);
        };
        for cert in app.certs() {
            let binding = AppBinding {
                package: app.package().to_owned(),
                cert_sha256: data_encoding::HEXLOWER.encode(cert),
            };
            if !app_bindings.contains(&binding) {
                app_bindings.push(binding);
            }
        }
        if app_bindings.len() > MAX_APP_BINDINGS {
            return Err(Error::InvalidInput("too many apps for this login"));
        }
        let mut overview = existing;
        overview.updated_at = now_ms;
        let base = self.store.item_revision(id)?;
        self.stage(overview, Some(&details), base)
    }

    /// Every login whose title, username or website contains `query`, for
    /// "Search HavenKeys…". No secrets.
    pub fn search_logins(&self, query: &str) -> Result<Vec<Suggestion>> {
        Ok(self
            .search(query)?
            .iter()
            .filter(|o| o.item_type == ItemType::Login)
            .map(|o| suggestion(o, MatchStrength::SameSite))
            .collect())
    }

    /// Hosts the Digital Asset Links lookup may ask about.
    pub fn login_hosts(&self) -> Result<Vec<String>> {
        let session = self.session()?;
        let mut hosts = BTreeSet::new();
        for o in session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login)
        {
            for rule in &o.urls {
                let Ok(url) = url::Url::parse(&rule.url) else {
                    continue;
                };
                if let Some(url::Host::Domain(d)) = url.host() {
                    if d != "localhost" {
                        hosts.insert(d.to_ascii_lowercase());
                    }
                }
            }
        }
        Ok(hosts.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::app_target::AppIdentity;
    use crate::local::tests::unlocked_vault;
    use crate::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use crate::vault::VaultService;
    use crate::{Error, SecretString};
    use uuid::Uuid;

    const NOW: i64 = 1_800_000_000_000;

    fn github_app(cert: u8) -> AppIdentity {
        AppIdentity::new("com.github.android", &[vec![cert; 32]]).unwrap()
    }

    fn add_login(v: &mut VaultService, title: &str, url: &str) -> Uuid {
        let input = ItemInput {
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: url.into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
            totp: SecretUpdate::Set(SecretString::from("JBSWY3DPEHPK3PXP")),
            ..ItemInput::blank(ItemType::Login, title.into())
        };
        let staged = v.stage_create(input, NOW).unwrap();
        let id = staged.item_id;
        v.commit_write(staged, 1).unwrap();
        id
    }

    fn bind(v: &mut VaultService, id: &Uuid, app: &AppIdentity) {
        let staged = v.stage_bind_app(id, app, NOW).unwrap();
        v.commit_write(staged, 2).unwrap();
    }

    #[test]
    fn a_bound_app_gets_its_login() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        bind(&mut v, &id, &github_app(1));
        let m = v.matches_for_app(&github_app(1), &[]).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, id);
        let fill = v.fill_for_app(&id, &github_app(1), &[]).unwrap();
        assert_eq!(fill.password.unwrap().expose(), "hunter2hunter2");
        assert_eq!(
            v.totp_for_app(&id, &github_app(1), &[], 59)
                .unwrap()
                .code
                .expose()
                .len(),
            6
        );
    }

    #[test]
    fn the_same_package_signed_by_someone_else_gets_nothing() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        bind(&mut v, &id, &github_app(1));
        assert!(v.matches_for_app(&github_app(2), &[]).unwrap().is_empty());
        assert_eq!(
            v.fill_for_app(&id, &github_app(2), &[]).err(),
            Some(Error::Denied)
        );
        assert_eq!(
            v.totp_for_app(&id, &github_app(2), &[], 59).err(),
            Some(Error::Denied)
        );
    }

    #[test]
    fn a_host_that_vouches_for_the_app_matches_its_logins() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        let other = add_login(&mut v, "Evil", "https://evil.example");
        let hosts = vec!["github.com".to_string()];
        let m = v.matches_for_app(&github_app(1), &hosts).unwrap();
        assert_eq!(m.iter().map(|s| s.id).collect::<Vec<_>>(), vec![id]);
        assert!(v.fill_for_app(&id, &github_app(1), &hosts).is_ok());
        assert_eq!(
            v.fill_for_app(&other, &github_app(1), &hosts).err(),
            Some(Error::Denied)
        );
    }

    #[test]
    fn an_unbound_unvouched_app_gets_nothing() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        assert!(v.matches_for_app(&github_app(1), &[]).unwrap().is_empty());
        assert_eq!(
            v.fill_for_app(&id, &github_app(1), &[]).err(),
            Some(Error::Denied)
        );
    }

    #[test]
    fn a_locked_vault_fills_no_app() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        bind(&mut v, &id, &github_app(1));
        v.lock();
        assert_eq!(
            v.matches_for_app(&github_app(1), &[]).err(),
            Some(Error::Locked)
        );
        assert_eq!(
            v.fill_for_app(&id, &github_app(1), &[]).err(),
            Some(Error::Locked)
        );
    }

    #[test]
    fn a_note_cannot_be_bound_or_filled() {
        let mut v = unlocked_vault();
        let input = ItemInput {
            content: SecretUpdate::Set(SecretString::from("secret")),
            ..ItemInput::blank(ItemType::SecureNote, "Note".into())
        };
        let staged = v.stage_create(input, NOW).unwrap();
        let id = staged.item_id;
        v.commit_write(staged, 1).unwrap();
        assert_eq!(
            v.stage_bind_app(&id, &github_app(1), NOW).err(),
            Some(Error::Denied)
        );
        assert_eq!(
            v.fill_for_app(&id, &github_app(1), &["github.com".into()])
                .err(),
            Some(Error::Denied)
        );
    }

    #[test]
    fn an_edit_from_another_device_keeps_the_bindings() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        bind(&mut v, &id, &github_app(1));
        let edit = ItemInput {
            username: Some("octocat".into()),
            urls: vec![UrlRule {
                url: "https://github.com".into(),
                match_type: MatchType::Domain,
            }],
            ..ItemInput::blank(ItemType::Login, "GitHub (renamed)".into())
        };
        let staged = v.stage_update(&id, edit, NOW + 1).unwrap();
        v.commit_write(staged, 3).unwrap();
        assert_eq!(v.matches_for_app(&github_app(1), &[]).unwrap().len(), 1);
    }

    #[test]
    fn binding_twice_stores_one_binding() {
        let mut v = unlocked_vault();
        let id = add_login(&mut v, "GitHub", "https://github.com");
        bind(&mut v, &id, &github_app(1));
        bind(&mut v, &id, &github_app(1));
        match v.load_details(&id).unwrap() {
            crate::model::ItemDetails::Login { app_bindings, .. } => {
                assert_eq!(app_bindings.len(), 1)
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn login_hosts_lists_each_site_once_without_ips() {
        let mut v = unlocked_vault();
        add_login(&mut v, "A", "https://GitHub.com");
        add_login(&mut v, "B", "https://github.com/login");
        add_login(&mut v, "C", "https://192.168.1.1");
        add_login(&mut v, "D", "http://localhost:8080");
        assert_eq!(v.login_hosts().unwrap(), vec!["github.com".to_string()]);
    }

    #[test]
    fn search_logins_finds_by_title_and_returns_no_secrets() {
        let mut v = unlocked_vault();
        add_login(&mut v, "GitHub", "https://github.com");
        add_login(&mut v, "GitLab", "https://gitlab.com");
        let found = v.search_logins("hub").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "GitHub");
    }
}
