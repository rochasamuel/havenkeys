//! Android Credential Manager (spec 2026-10-01-android-app §8): passkeys,
//! and passwords for apps that ask Credential Manager for one. Kotlin
//! reports the caller as Android gave it — package, signing certificates
//! and, for a browser, the origin `CallingAppInfo.getOrigin` returned — and
//! the request JSON. Everything else is decided here, on every call.

use crate::autofill::{to_match, AutofillMatch, FillValues};
use crate::error::MobileResult;
use crate::items::parse_id;
use crate::passkey_json::{
    assertion_json, invalid_request, parse_creation, parse_request, registration_json,
};
use crate::vault::MobileVault;
use crate::MobileError;
use havenkeys_core::app_target::{is_privileged_browser, AppIdentity};
use havenkeys_core::origin::PageUrl;
use havenkeys_core::passkey::{
    app_rp_id, AppCreateQuery, AppPasskeyCreate, CreateCheck, CreateQuery, PasskeyCreate,
};
use havenkeys_core::vault::Suggestion;
use havenkeys_core::Error;

const MAX_ORIGIN_LEN: usize = 2048;
const MAX_OFFERS: usize = 16;

#[derive(Clone, uniffi::Record)]
pub struct CredentialCaller {
    pub package_name: String,
    pub signing_certs: Vec<Vec<u8>>,
    /// Only from `CallingAppInfo.getOrigin` for a browser on the allowlist.
    pub origin: Option<String>,
}

/// A passkey offered in Android's sheet. No secrets.
#[derive(uniffi::Record)]
pub struct PasskeyOffer {
    pub item_id: String,
    pub credential_id: Vec<u8>,
    pub title: String,
    pub user_name: String,
}

#[derive(uniffi::Record)]
pub struct PasskeyCreatePlan {
    pub rp_id: String,
    pub user_name: String,
    /// The vault already holds one of the site's `excludeCredentials`.
    pub excluded: bool,
    /// Logins that can hold the new passkey, the same account first.
    pub homes: Vec<AutofillMatch>,
}

enum Caller {
    /// A browser on the privileged list, showing this page.
    Browser {
        page_url: String,
    },
    App(AppIdentity),
}

/// The vendored allowlist, for `CallingAppInfo.getOrigin`. The caller is
/// checked against the same list again here.
#[uniffi::export]
pub fn privileged_browsers_json() -> String {
    havenkeys_core::app_target::privileged_browsers_json().to_owned()
}

fn exists() -> MobileError {
    MobileError::Failed {
        code: "passkey_exists".into(),
        detail: "This account already has a passkey in HavenKeys.".into(),
    }
}

fn hash_32(h: &[u8]) -> MobileResult<[u8; 32]> {
    <[u8; 32]>::try_from(h).map_err(|_| invalid_request())
}

/// The RP ID asked for, or WebAuthn's default — the page's host — for a
/// browser. An app must name one.
fn rp_for(caller: &Caller, asked: Option<String>) -> MobileResult<String> {
    match (asked, caller) {
        (Some(id), _) => Ok(id),
        (None, Caller::Browser { page_url }) => url::Url::parse(page_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .ok_or_else(|| MobileError::from(Error::Denied)),
        (None, Caller::App(_)) => Err(invalid_request()),
    }
}

fn with_username(found: Vec<Suggestion>) -> Vec<AutofillMatch> {
    // A PasswordCredential needs an ID.
    found
        .iter()
        .filter(|s| s.username.as_deref().is_some_and(|u| !u.trim().is_empty()))
        .map(to_match)
        .collect()
}

impl MobileVault {
    fn credential_caller(&self, c: &CredentialCaller) -> MobileResult<Caller> {
        let app = AppIdentity::new(&c.package_name, &c.signing_certs)?;
        if app.package() == self.own_package {
            return Err(Error::Denied.into());
        }
        let Some(origin) = c.origin.as_deref() else {
            return Ok(Caller::App(app));
        };
        // An origin counts only from a browser on the list; anyone else
        // claiming one gets nothing rather than being treated as an app.
        let privileged = app
            .certs()
            .iter()
            .any(|cert| is_privileged_browser(app.package(), cert));
        if !privileged || origin.len() > MAX_ORIGIN_LEN || PageUrl::parse(origin).is_none() {
            return Err(Error::Denied.into());
        }
        Ok(Caller::Browser {
            page_url: origin.to_owned(),
        })
    }

    /// The normalized RP ID an app asked for and the hosts that vouch for
    /// the app, after refreshing that site's Digital Asset Links file when
    /// stale (and the setting allows).
    fn ask_about_rp(&self, app: &AppIdentity, rp_id: &str) -> MobileResult<(String, Vec<String>)> {
        let rp = app_rp_id(rp_id).ok_or(Error::Denied)?;
        let hosts = self.verified_hosts_asking(app, std::slice::from_ref(&rp))?;
        Ok((rp, hosts))
    }

    fn create_check(
        &self,
        caller: &Caller,
        rp: &str,
        user_name: &str,
        exclude: &[Vec<u8>],
    ) -> MobileResult<(String, CreateCheck)> {
        let now = havenkeys_client::now_ms();
        match caller {
            Caller::Browser { page_url } => {
                let check = self.client.vault()?.check_passkey_create(
                    &CreateQuery {
                        rp_id: rp,
                        page_url,
                        top_url: None,
                        user_name,
                        exclude,
                        conditional: false,
                    },
                    now,
                )?;
                Ok((app_rp_id(rp).unwrap_or_else(|| rp.to_owned()), check))
            }
            Caller::App(app) => {
                let (rp, hosts) = self.ask_about_rp(app, rp)?;
                let check = self.client.vault()?.check_passkey_create_for_app(
                    &AppCreateQuery {
                        rp_id: &rp,
                        app,
                        verified_hosts: &hosts,
                        user_name,
                        exclude,
                    },
                    now,
                )?;
                Ok((rp, check))
            }
        }
    }
}

#[uniffi::export]
impl MobileVault {
    /// Passkeys for Android's sheet (a Begin request). No secrets.
    pub fn passkey_offers(
        &self,
        caller: CredentialCaller,
        request_json: String,
    ) -> MobileResult<Vec<PasskeyOffer>> {
        self.unlocked()?;
        let caller = self.credential_caller(&caller)?;
        let req = parse_request(&request_json)?;
        let rp = rp_for(&caller, req.rp_id)?;
        let found = match &caller {
            Caller::Browser { page_url } => self
                .client
                .vault()?
                .find_passkeys(&rp, page_url, None, &req.allow)?,
            Caller::App(app) => {
                let (rp, _) = self.ask_about_rp(app, &rp)?;
                self.client.vault()?.find_passkeys_for_app(
                    &rp,
                    app,
                    &req.allow,
                    havenkeys_client::now_ms(),
                )?
            }
        };
        Ok(found
            .into_iter()
            .take(MAX_OFFERS)
            .map(|m| PasskeyOffer {
                item_id: m.item_id.to_string(),
                credential_id: m.credential_id,
                title: m.title,
                user_name: m.user_name,
            })
            .collect())
    }

    /// The user tapped a passkey and passed user verification. Works
    /// offline. A browser's `client_data_hash` is signed; an app's is
    /// ignored, since it could name any origin.
    pub fn passkey_sign_in(
        &self,
        caller: CredentialCaller,
        request_json: String,
        client_data_hash: Option<Vec<u8>>,
        item_id: String,
        credential_id: Vec<u8>,
    ) -> MobileResult<String> {
        let item = parse_id(&item_id)?;
        self.unlocked()?;
        let caller = self.credential_caller(&caller)?;
        let req = parse_request(&request_json)?;
        let rp = rp_for(&caller, req.rp_id)?;
        if !req.allow.is_empty() && !req.allow.contains(&credential_id) {
            return Err(Error::Denied.into());
        }
        let assertion = match &caller {
            Caller::Browser { page_url } => {
                let vault = self.client.vault()?;
                match client_data_hash.as_deref() {
                    Some(h) => vault.passkey_assert_with_hash(
                        &item,
                        &credential_id,
                        &rp,
                        page_url,
                        &req.challenge,
                        &hash_32(h)?,
                    )?,
                    None => vault.passkey_assert(
                        &item,
                        &credential_id,
                        &rp,
                        page_url,
                        None,
                        &req.challenge,
                    )?,
                }
            }
            Caller::App(app) => {
                let (rp, _) = self.ask_about_rp(app, &rp)?;
                self.client.vault()?.passkey_assert_for_app(
                    &item,
                    &credential_id,
                    &rp,
                    app,
                    &req.challenge,
                    havenkeys_client::now_ms(),
                )?
            }
        };
        Ok(assertion_json(&assertion))
    }

    /// What "Save a passkey to HavenKeys?" shows. Works offline.
    pub fn passkey_create_plan(
        &self,
        caller: CredentialCaller,
        request_json: String,
    ) -> MobileResult<PasskeyCreatePlan> {
        self.unlocked()?;
        let caller = self.credential_caller(&caller)?;
        let opts = parse_creation(&request_json)?;
        let rp = rp_for(&caller, opts.rp_id.clone())?;
        let (rp_id, check) = self.create_check(&caller, &rp, &opts.user_name, &opts.exclude)?;
        let homes = if check.excluded {
            Vec::new()
        } else {
            check.candidates.iter().map(to_match).collect()
        };
        Ok(PasskeyCreatePlan {
            rp_id,
            user_name: opts.user_name,
            excluded: check.excluded,
            homes,
        })
    }

    /// The user confirmed and passed user verification. Online only: the
    /// passkey is returned to the site only after the server accepted it.
    /// `item_id`: the login to hold it; `None` makes a new login.
    pub fn passkey_create(
        &self,
        caller: CredentialCaller,
        request_json: String,
        item_id: Option<String>,
    ) -> MobileResult<String> {
        let item = item_id.as_deref().map(parse_id).transpose()?;
        self.unlocked()?;
        self.client.require_online()?;
        let caller = self.credential_caller(&caller)?;
        let opts = parse_creation(&request_json)?;
        let rp = rp_for(&caller, opts.rp_id.clone())?;
        if self
            .create_check(&caller, &rp, &opts.user_name, &opts.exclude)?
            .1
            .excluded
        {
            return Err(exists());
        }
        let now = havenkeys_client::now_ms();
        let staged = match &caller {
            Caller::Browser { page_url } => self.client.vault()?.stage_passkey_create(
                PasskeyCreate {
                    rp_id: &rp,
                    page_url,
                    top_url: None,
                    challenge: &opts.challenge,
                    user_handle: &opts.user_handle,
                    user_name: &opts.user_name,
                    display_name: opts.display_name.as_deref(),
                    item_id: item,
                    conditional: false,
                },
                now,
            )?,
            Caller::App(app) => {
                let (rp, hosts) = self.ask_about_rp(app, &rp)?;
                self.client.vault()?.stage_passkey_create_for_app(
                    AppPasskeyCreate {
                        rp_id: &rp,
                        app,
                        verified_hosts: &hosts,
                        challenge: &opts.challenge,
                        user_handle: &opts.user_handle,
                        user_name: &opts.user_name,
                        display_name: opts.display_name.as_deref(),
                        item_id: item,
                    },
                    now,
                )?
            }
        };
        let response = registration_json(&staged.registration);
        self.send(staged.write)?;
        Ok(response)
    }

    /// Logins for a Credential Manager password request, by the M1 target
    /// rules. Only logins with a username. No secrets.
    pub fn credential_password_offers(
        &self,
        caller: CredentialCaller,
    ) -> MobileResult<Vec<AutofillMatch>> {
        self.unlocked()?;
        let found = match self.credential_caller(&caller)? {
            Caller::Browser { page_url } => self.client.vault()?.find_matches(&page_url, None)?,
            Caller::App(app) => {
                let hosts = self.verified_hosts(&app)?;
                self.client.vault()?.matches_for_app(&app, &hosts)?
            }
        };
        Ok(with_username(found).into_iter().take(MAX_OFFERS).collect())
    }

    /// The tapped login's username and password, re-checked for the caller.
    pub fn credential_password(
        &self,
        caller: CredentialCaller,
        item_id: String,
    ) -> MobileResult<FillValues> {
        let id = parse_id(&item_id)?;
        self.unlocked()?;
        let creds = match self.credential_caller(&caller)? {
            Caller::Browser { page_url } => self.client.vault()?.fill_for_page(
                &id,
                &page_url,
                None,
                havenkeys_client::now_ms(),
            )?,
            Caller::App(app) => {
                let hosts = self.verified_hosts(&app)?;
                self.client.vault()?.fill_for_app(&id, &app, &hosts)?
            }
        };
        Ok(FillValues {
            username: creds.username,
            password: creds.password.map(|p| p.expose().to_owned()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{seed_github_passkey, vouch};
    use crate::vault::tests::{code, overdue_refuses, unlocked};
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use havenkeys_core::SecretString;
    use p256::ecdsa::signature::Verifier;
    use p256::ecdsa::{Signature, VerifyingKey};
    use p256::pkcs8::DecodePublicKey;
    use sha2::{Digest, Sha256};

    const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";
    const GET: &str = r#"{"challenge":"AwMD","rpId":"github.com"}"#;
    const CREATE: &str = r#"{"rp":{"id":"github.com"},"user":{"id":"CQk","name":"hubot"},"challenge":"BwcH","pubKeyCredParams":[{"type":"public-key","alg":-7}]}"#;

    fn chrome(origin: &str) -> CredentialCaller {
        CredentialCaller {
            package_name: "com.android.chrome".into(),
            signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(CHROME)
                .unwrap()
                .to_vec()],
            origin: Some(origin.into()),
        }
    }

    fn github_app(cert: u8) -> CredentialCaller {
        CredentialCaller {
            package_name: "com.github.android".into(),
            signing_certs: vec![vec![cert; 32]],
            origin: None,
        }
    }

    fn no_fetches(v: &MobileVault) {
        let mut s = v.settings().unwrap();
        s.asset_links = false;
        v.update_settings(s).unwrap();
    }

    /// `unlocked()` already seeded the account, so `testing::seed_with_github_login`
    /// (which seeds it again) is not used here.
    fn add_github_login(v: &MobileVault) {
        let input = ItemInput {
            item_type: ItemType::Login,
            title: "GitHub".into(),
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: "https://github.com".into(),
                match_type: MatchType::Domain,
            }],
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
        vault.commit_write(staged, 1).unwrap();
    }

    fn json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).unwrap()
    }

    fn b64(v: &serde_json::Value) -> Vec<u8> {
        data_encoding::BASE64URL_NOPAD
            .decode(v.as_str().unwrap().as_bytes())
            .unwrap()
    }

    fn verifies(spki: &[u8], response: &serde_json::Value, client_data_hash: &[u8]) -> bool {
        let mut signed = b64(&response["authenticatorData"]);
        signed.extend_from_slice(client_data_hash);
        let sig = Signature::from_der(&b64(&response["signature"])).unwrap();
        VerifyingKey::from_public_key_der(spki)
            .unwrap()
            .verify(&signed, &sig)
            .is_ok()
    }

    #[test]
    fn chrome_lists_and_signs_in_on_the_right_site() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let (item, cred, spki) = seed_github_passkey(&v);
        let offers = v
            .passkey_offers(chrome("https://github.com"), GET.into())
            .unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].item_id, item);
        assert_eq!(offers[0].user_name, "octo");
        let out = json(
            &v.passkey_sign_in(chrome("https://github.com"), GET.into(), None, item, cred)
                .unwrap(),
        );
        let cdj = b64(&out["response"]["clientDataJSON"]);
        assert!(String::from_utf8(cdj.clone())
            .unwrap()
            .contains(r#""origin":"https://github.com""#));
        assert!(verifies(&spki, &out["response"], &Sha256::digest(&cdj)));
    }

    #[test]
    fn chromes_client_data_hash_is_what_gets_signed() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let (item, cred, spki) = seed_github_passkey(&v);
        let hash = vec![0x42; 32];
        let out = json(
            &v.passkey_sign_in(
                chrome("https://github.com"),
                GET.into(),
                Some(hash.clone()),
                item.clone(),
                cred.clone(),
            )
            .unwrap(),
        );
        assert!(verifies(&spki, &out["response"], &hash));
        assert_eq!(
            code(
                v.passkey_sign_in(
                    chrome("https://github.com"),
                    GET.into(),
                    Some(vec![1; 31]),
                    item,
                    cred
                )
                .err()
                .unwrap()
            ),
            "invalid_input"
        );
    }

    #[test]
    fn chrome_on_a_look_alike_gets_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let (item, cred, _) = seed_github_passkey(&v);
        for origin in [
            "https://github.com.evil.com",
            "https://evilgithub.com",
            "http://github.com",
        ] {
            assert!(
                v.passkey_offers(chrome(origin), GET.into()).is_err(),
                "{origin}"
            );
            assert!(
                v.passkey_sign_in(chrome(origin), GET.into(), None, item.clone(), cred.clone())
                    .is_err(),
                "{origin}"
            );
        }
    }

    #[test]
    fn a_browser_without_rp_id_uses_the_pages_host_and_an_app_must_name_one() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_github_passkey(&v);
        let no_rp = r#"{"challenge":"AwMD"}"#;
        assert_eq!(
            v.passkey_offers(chrome("https://github.com"), no_rp.into())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            code(
                v.passkey_offers(github_app(0xab), no_rp.into())
                    .err()
                    .unwrap()
            ),
            "invalid_input"
        );
    }

    #[test]
    fn an_app_needs_the_sites_get_login_creds() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        no_fetches(&v);
        let (item, cred, _) = seed_github_passkey(&v);
        assert_eq!(
            code(
                v.passkey_offers(github_app(0xab), GET.into())
                    .err()
                    .unwrap()
            ),
            "denied"
        );
        vouch(&v, "github.com", "com.github.android", &[0xab; 32], false);
        assert_eq!(
            code(
                v.passkey_offers(github_app(0xab), GET.into())
                    .err()
                    .unwrap()
            ),
            "denied"
        );
        vouch(&v, "github.com", "com.github.android", &[0xab; 32], true);
        assert_eq!(
            v.passkey_offers(github_app(0xab), GET.into())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            code(
                v.passkey_offers(github_app(0xcd), GET.into())
                    .err()
                    .unwrap()
            ),
            "denied"
        );
        assert!(v
            .passkey_sign_in(github_app(0xab), GET.into(), None, item, cred)
            .is_ok());
    }

    #[test]
    fn an_apps_client_data_hash_is_never_signed() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        no_fetches(&v);
        let (item, cred, spki) = seed_github_passkey(&v);
        vouch(&v, "github.com", "com.github.android", &[0xab; 32], true);
        let forged = vec![0x42; 32];
        let out = json(
            &v.passkey_sign_in(
                github_app(0xab),
                GET.into(),
                Some(forged.clone()),
                item,
                cred,
            )
            .unwrap(),
        );
        assert!(!verifies(&spki, &out["response"], &forged));
        let cdj = b64(&out["response"]["clientDataJSON"]);
        assert!(verifies(&spki, &out["response"], &Sha256::digest(&cdj)));
        let origin = format!(
            r#""origin":"android:apk-key-hash:{}""#,
            havenkeys_core::passkey::encode_b64url(&[0xab; 32])
        );
        assert!(String::from_utf8(cdj).unwrap().contains(&origin));
    }

    #[test]
    fn an_app_claiming_an_origin_or_being_havenkeys_gets_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_github_passkey(&v);
        let claiming = CredentialCaller {
            origin: Some("https://github.com".into()),
            ..github_app(0xab)
        };
        assert_eq!(
            code(v.passkey_offers(claiming, GET.into()).err().unwrap()),
            "denied"
        );
        let own = CredentialCaller {
            package_name: "net.havenkeys.android".into(),
            ..github_app(0xab)
        };
        assert_eq!(
            code(v.passkey_offers(own, GET.into()).err().unwrap()),
            "denied"
        );
        let no_cert = CredentialCaller {
            signing_certs: vec![],
            ..chrome("https://github.com")
        };
        assert_eq!(
            code(v.passkey_offers(no_cert, GET.into()).err().unwrap()),
            "denied"
        );
    }

    #[test]
    fn a_site_that_named_its_credentials_gets_only_those() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let (item, cred, _) = seed_github_passkey(&v);
        let other = r#"{"challenge":"AwMD","rpId":"github.com","allowCredentials":[{"type":"public-key","id":"CQkJ"}]}"#;
        assert!(v
            .passkey_offers(chrome("https://github.com"), other.into())
            .unwrap()
            .is_empty());
        assert_eq!(
            code(
                v.passkey_sign_in(chrome("https://github.com"), other.into(), None, item, cred)
                    .err()
                    .unwrap()
            ),
            "denied"
        );
    }

    #[test]
    fn the_create_plan_reports_homes_and_an_excluded_account() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        add_github_login(&v);
        let plan = v
            .passkey_create_plan(chrome("https://github.com"), CREATE.into())
            .unwrap();
        assert_eq!(plan.rp_id, "github.com");
        assert_eq!(plan.user_name, "hubot");
        assert!(!plan.excluded);
        assert_eq!(plan.homes.len(), 1);
        let (_, cred, _) = seed_github_passkey(&v);
        let excluding = CREATE.replace(
            r#""challenge""#,
            &format!(
                r#""excludeCredentials":[{{"type":"public-key","id":"{}"}}],"challenge""#,
                havenkeys_core::passkey::encode_b64url(&cred)
            ),
        );
        let plan = v
            .passkey_create_plan(chrome("https://github.com"), excluding)
            .unwrap();
        assert!(plan.excluded);
        assert!(plan.homes.is_empty());
    }

    #[test]
    fn creating_needs_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let before = v.list_items().unwrap().len();
        assert_eq!(
            code(
                v.passkey_create(chrome("https://github.com"), CREATE.into(), None)
                    .err()
                    .unwrap()
            ),
            "offline"
        );
        assert_eq!(v.list_items().unwrap().len(), before);
    }

    #[test]
    fn password_requests_follow_the_fill_target_rules() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        no_fetches(&v);
        add_github_login(&v);
        let offers = v
            .credential_password_offers(chrome("https://github.com"))
            .unwrap();
        assert_eq!(offers.len(), 1);
        let filled = v
            .credential_password(chrome("https://github.com"), offers[0].id.clone())
            .unwrap();
        assert_eq!(filled.password.as_deref(), Some("hunter2hunter2"));
        assert!(v
            .credential_password_offers(chrome("https://github.com.evil.com"))
            .unwrap()
            .is_empty());
        assert!(v
            .credential_password(chrome("https://evil.com"), offers[0].id.clone())
            .is_err());
        assert!(v
            .credential_password_offers(github_app(0xab))
            .unwrap()
            .is_empty());
        assert!(v
            .credential_password(github_app(0xab), offers[0].id.clone())
            .is_err());
    }

    #[test]
    fn a_locked_vault_lists_and_signs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        let (item, cred, _) = seed_github_passkey(&v);
        let site = || chrome("https://github.com");
        overdue_refuses(&v, &seen, |v| v.passkey_offers(site(), GET.into()));
        overdue_refuses(&v, &seen, |v| {
            v.passkey_sign_in(site(), GET.into(), None, item.clone(), cred.clone())
        });
        overdue_refuses(&v, &seen, |v| v.passkey_create_plan(site(), CREATE.into()));
        overdue_refuses(&v, &seen, |v| v.passkey_create(site(), CREATE.into(), None));
        overdue_refuses(&v, &seen, |v| v.credential_password_offers(site()));
        overdue_refuses(&v, &seen, |v| v.credential_password(site(), item.clone()));
    }

    #[test]
    fn the_allowlist_is_the_vendored_one() {
        assert!(privileged_browsers_json().contains("com.android.chrome"));
    }
}
