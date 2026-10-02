//! Saving a login Android Autofill saw being submitted (spec
//! 2026-10-01-android-app §7.5). Android's own save sheet is the user's
//! confirmation; Rust decides whether that adds a login, changes one
//! password, or does nothing, with the extension's rules for browsers and
//! their app version for apps.

use crate::autofill::TargetFacts;
use crate::error::MobileResult;
use crate::vault::MobileVault;
use havenkeys_core::app_target::FillTarget;
use havenkeys_core::vault::{SaveAction, SaveTarget, StagedWrite};
use havenkeys_core::SecretString;

/// What the user typed. No `Debug`: it carries the password.
#[derive(uniffi::Record)]
pub struct SaveLogin {
    pub username: Option<String>,
    pub password: String,
    /// A change-password form's current password: it finds the login to
    /// update when the form has no username.
    pub current_password: Option<String>,
    /// For a new app login: the app's label. A browser login is named
    /// after its site.
    pub title: Option<String>,
}

#[derive(uniffi::Enum)]
pub enum SaveResult {
    Added,
    Updated,
    Unchanged,
}

fn decided(action: &SaveAction) -> Option<(SaveTarget<'_>, SaveResult)> {
    match action {
        SaveAction::Add => Some((SaveTarget::New { title: None }, SaveResult::Added)),
        SaveAction::Update(id) => Some((SaveTarget::Update(id), SaveResult::Updated)),
        SaveAction::Unchanged => None,
    }
}

impl MobileVault {
    /// The decision and the sealed write, not sent. `None`: already saved.
    pub(crate) fn stage_autofill_save(
        &self,
        target: &TargetFacts,
        login: SaveLogin,
    ) -> MobileResult<Option<(StagedWrite, SaveResult)>> {
        self.unlocked()?;
        let password = SecretString::new(login.password);
        let current = login.current_password.map(SecretString::new);
        let username = login.username.as_deref().map(str::trim).filter(|u| !u.is_empty());
        let now = havenkeys_client::now_ms();
        match self.target(target)? {
            FillTarget::Browser { page_url } => {
                let vault = self.client.vault()?;
                let action = vault.check_login(&page_url, None, username, &password, current.as_ref())?;
                let Some((to, result)) = decided(&action) else { return Ok(None) };
                let staged = vault.stage_save_login(&page_url, None, username, password, to, now)?;
                Ok(Some((staged.write, result)))
            }
            FillTarget::App(app) => {
                // Fetched before taking the vault guard: it may reach the network.
                let hosts = self.verified_hosts(&app)?;
                let vault = self.client.vault()?;
                let action = vault.check_login_for_app(&app, &hosts, username, &password, current.as_ref())?;
                let Some((to, result)) = decided(&action) else { return Ok(None) };
                let to = match to {
                    SaveTarget::New { .. } => SaveTarget::New { title: login.title.as_deref() },
                    update => update,
                };
                let staged = vault.stage_save_login_for_app(&app, &hosts, username, password, to, now)?;
                Ok(Some((staged.write, result)))
            }
        }
    }
}

#[uniffi::export]
impl MobileVault {
    /// After Android's save sheet was confirmed. Online only; not app use,
    /// so the idle timer is not touched.
    pub fn autofill_save(&self, target: TargetFacts, login: SaveLogin) -> MobileResult<SaveResult> {
        self.unlocked()?;
        self.client.require_online()?;
        match self.stage_autofill_save(&target, login)? {
            None => Ok(SaveResult::Unchanged),
            Some((staged, result)) => {
                self.send(staged)?;
                Ok(result)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::{code, overdue_refuses, unlocked};
    use havenkeys_core::app_target::AppIdentity;
    use uuid::Uuid;
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};

    const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";

    fn chrome(domain: &str, scheme: Option<&str>) -> TargetFacts {
        TargetFacts {
            package_name: "com.android.chrome".into(),
            signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(CHROME).unwrap().to_vec()],
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

    fn login(user: Option<&str>, pw: &str) -> SaveLogin {
        SaveLogin {
            username: user.map(Into::into),
            password: pw.into(),
            current_password: None,
            title: Some("GitHub".into()),
        }
    }

    fn no_asset_links(v: &MobileVault) {
        let mut s = v.settings().unwrap();
        s.asset_links = false;
        v.update_settings(s).unwrap();
    }

    fn add_github(v: &MobileVault) -> Uuid {
        let input = ItemInput {
            item_type: ItemType::Login,
            title: "GitHub".into(),
            username: Some("octo".into()),
            urls: vec![UrlRule { url: "https://github.com".into(), match_type: MatchType::Domain }],
            password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
            totp: SecretUpdate::Keep,
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
        id
    }

    /// Stage, then record locally as if the server accepted it.
    fn save(v: &MobileVault, target: TargetFacts, l: SaveLogin) -> SaveResult {
        match v.stage_autofill_save(&target, l).unwrap() {
            None => SaveResult::Unchanged,
            Some((staged, result)) => {
                v.client.vault().unwrap().commit_write(staged, 9).unwrap();
                result
            }
        }
    }

    fn github_password(v: &MobileVault, id: &Uuid) -> String {
        v.reveal(id.to_string(), "password".into()).unwrap()
    }

    #[test]
    fn a_new_site_login_is_added_for_the_pages_host() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        assert!(matches!(save(&v, chrome("github.com", Some("https")), login(Some("octo"), "s3cret-pass")), SaveResult::Added));
        let m = v.autofill_matches(chrome("github.com", Some("https"))).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].title, "github.com");
        assert_eq!(m[0].username.as_deref(), Some("octo"));
    }

    #[test]
    fn a_new_password_updates_and_the_same_one_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_github(&v);
        let site = || chrome("github.com", Some("https"));
        assert!(matches!(save(&v, site(), login(Some("octo"), "hunter2hunter2")), SaveResult::Unchanged));
        assert!(matches!(save(&v, site(), login(Some("Octo"), "brand-new-pass")), SaveResult::Updated));
        assert_eq!(github_password(&v, &id), "brand-new-pass");
        assert_eq!(v.list_items().unwrap().len(), 1);
    }

    #[test]
    fn a_change_password_form_updates_by_the_current_password() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_github(&v);
        let change = SaveLogin {
            username: None,
            password: "brand-new-pass".into(),
            current_password: Some("hunter2hunter2".into()),
            title: None,
        };
        assert!(matches!(save(&v, chrome("github.com", Some("https")), change), SaveResult::Updated));
        assert_eq!(github_password(&v, &id), "brand-new-pass");
    }

    #[test]
    fn a_look_alike_site_never_updates_the_real_login() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_github(&v);
        for domain in ["github.com.evil.com", "evilgithub.com", "github-login.example.com"] {
            assert!(matches!(save(&v, chrome(domain, Some("https")), login(Some("octo"), "phished-pass")), SaveResult::Added));
        }
        assert_eq!(github_password(&v, &id), "hunter2hunter2");
        assert_eq!(v.autofill_matches(chrome("github.com", Some("https"))).unwrap().len(), 1);
    }

    #[test]
    fn an_app_login_is_bound_to_that_app_only() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        no_asset_links(&v);
        assert!(matches!(save(&v, app(1), login(Some("octo"), "s3cret-pass")), SaveResult::Added));
        let m = v.autofill_matches(app(1)).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].title, "GitHub");
        assert!(v.autofill_matches(app(2)).unwrap().is_empty());
        assert!(v.autofill_matches(chrome("github.com", Some("https"))).unwrap().is_empty());
    }

    #[test]
    fn an_impostor_app_never_updates_a_bound_login() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        no_asset_links(&v);
        let id = add_github(&v);
        let bound = AppIdentity::new("com.github.android", &[vec![1; 32]]).unwrap();
        {
            let mut vault = v.client.vault().unwrap();
            let staged = vault.stage_bind_app(&id, &bound, 2).unwrap();
            vault.commit_write(staged, 2).unwrap();
        }
        assert!(matches!(save(&v, app(2), login(Some("octo"), "phished-pass")), SaveResult::Added));
        assert_eq!(github_password(&v, &id), "hunter2hunter2");
        assert!(matches!(save(&v, app(1), login(Some("octo"), "brand-new-pass")), SaveResult::Updated));
        assert_eq!(github_password(&v, &id), "brand-new-pass");
    }

    #[test]
    fn no_scheme_havenkeys_itself_and_an_empty_password_save_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        assert!(v.stage_autofill_save(&chrome("github.com", None), login(Some("octo"), "pw-pw-pw")).is_err());
        let own = TargetFacts { package_name: "net.havenkeys.android".into(), ..app(1) };
        assert!(v.stage_autofill_save(&own, login(Some("octo"), "pw-pw-pw")).is_err());
        assert_eq!(
            code(v.stage_autofill_save(&chrome("github.com", Some("https")), login(Some("octo"), "")).err().unwrap()),
            "invalid_input"
        );
        assert!(v.list_items().unwrap().is_empty());
    }

    #[test]
    fn saving_needs_the_server_and_an_open_vault() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        assert_eq!(
            code(v.autofill_save(chrome("github.com", Some("https")), login(Some("octo"), "pw-pw-pw")).err().unwrap()),
            "offline"
        );
        overdue_refuses(&v, &seen, |v| v.autofill_save(chrome("github.com", Some("https")), login(Some("octo"), "pw-pw-pw")));
        assert_eq!(
            code(v.stage_autofill_save(&chrome("github.com", Some("https")), login(Some("octo"), "pw-pw-pw")).err().unwrap()),
            "locked"
        );
    }
}
