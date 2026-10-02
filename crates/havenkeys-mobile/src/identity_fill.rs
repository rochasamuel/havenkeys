//! The identity in Android Autofill (spec 2026-10-01-android-app §7.6), by
//! the extension's identity rules (`identity_page.rs`): any page or app; a
//! frame only when it is the page's site; only the roles asked for;
//! documents only after the user confirmed them, and never on http.

use crate::autofill::{frame_url, FrameFacts, TargetFacts};
use crate::error::MobileResult;
use crate::vault::MobileVault;
use havenkeys_core::app_target::FillTarget;
use havenkeys_core::identity::FillRole;
use havenkeys_core::identity_page::IdentitySummary;
use havenkeys_core::origin::PageUrl;
use havenkeys_core::{Error, SecretString};

/// Roles per request, at most: every role once.
pub const MAX_IDENTITY_ROLES: usize = FillRole::ALL.len();

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum IdentityRole {
    FullName,
    FirstName,
    MiddleName,
    LastName,
    Email,
    Phone,
    BirthDate,
    BirthDay,
    BirthMonth,
    BirthYear,
    Company,
    Street,
    Number,
    Complement,
    AddressLine1,
    AddressLine2,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
    Username,
    Cpf,
    Rg,
    Passport,
    DriversLicense,
}

/// The two lists name the same roles in the same order (a test pins it).
const ROLES: [IdentityRole; MAX_IDENTITY_ROLES] = [
    IdentityRole::FullName,
    IdentityRole::FirstName,
    IdentityRole::MiddleName,
    IdentityRole::LastName,
    IdentityRole::Email,
    IdentityRole::Phone,
    IdentityRole::BirthDate,
    IdentityRole::BirthDay,
    IdentityRole::BirthMonth,
    IdentityRole::BirthYear,
    IdentityRole::Company,
    IdentityRole::Street,
    IdentityRole::Number,
    IdentityRole::Complement,
    IdentityRole::AddressLine1,
    IdentityRole::AddressLine2,
    IdentityRole::Neighborhood,
    IdentityRole::City,
    IdentityRole::State,
    IdentityRole::PostalCode,
    IdentityRole::Country,
    IdentityRole::Username,
    IdentityRole::Cpf,
    IdentityRole::Rg,
    IdentityRole::Passport,
    IdentityRole::DriversLicense,
];

impl IdentityRole {
    fn core(self) -> FillRole {
        let at = ROLES.iter().position(|r| *r == self).unwrap_or_default();
        FillRole::ALL[at]
    }

    fn of(role: FillRole) -> Self {
        let at = FillRole::ALL
            .iter()
            .position(|r| *r == role)
            .unwrap_or_default();
        ROLES[at]
    }
}

/// Document numbers: filled only after the user confirms them, and only on
/// https pages or in apps.
pub(crate) fn is_document_role(role: IdentityRole) -> bool {
    role.core().is_document()
}

#[derive(uniffi::Record)]
pub struct IdentityChoice {
    pub title: String,
    pub email: Option<String>,
    pub roles: Vec<IdentityRole>,
}

/// One identity value. No `Debug`.
#[derive(uniffi::Record)]
pub struct IdentityValue {
    pub role: IdentityRole,
    pub value: String,
}

/// Where the identity is asked for: the frame's page and the tab's, or an app.
enum Place {
    Page { page: String, top: String },
    App,
}

impl MobileVault {
    fn place(&self, target: &TargetFacts, frame: &FrameFacts) -> MobileResult<Place> {
        Ok(match self.target(target)? {
            FillTarget::Browser { page_url } => Place::Page {
                page: frame_url(&page_url, frame).ok_or(Error::Denied)?,
                top: page_url,
            },
            FillTarget::App(_) => Place::App,
        })
    }
}

fn choice(summary: IdentitySummary, documents_allowed: bool) -> Option<IdentityChoice> {
    let roles: Vec<IdentityRole> = summary
        .roles
        .into_iter()
        .map(IdentityRole::of)
        .filter(|r| documents_allowed || !is_document_role(*r))
        .collect();
    (!roles.is_empty()).then_some(IdentityChoice {
        title: summary.title,
        email: summary.email,
        roles,
    })
}

#[uniffi::export]
impl MobileVault {
    /// The identity a form may be offered. `None`: no identity, or nothing
    /// in it that can be filled here.
    pub fn autofill_identity(
        &self,
        target: TargetFacts,
        frame: FrameFacts,
    ) -> MobileResult<Option<IdentityChoice>> {
        self.unlocked()?;
        let place = self.place(&target, &frame)?;
        let vault = self.client.vault()?;
        let found = match &place {
            Place::Page { page, top } => vault.identity_summary_for_page(page, Some(top.as_str())),
            Place::App => vault.identity_summary_for_app(),
        };
        let documents_allowed = match &place {
            Place::Page { page, .. } => PageUrl::parse(page).is_some_and(|p| p.is_https()),
            Place::App => true,
        };
        match found {
            Ok(summary) => Ok(choice(summary, documents_allowed)),
            Err(Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// The values for `roles`, in that order, skipping roles with no value.
    /// `documents`: the user confirmed them in HavenKeys's own window.
    pub fn autofill_identity_values(
        &self,
        target: TargetFacts,
        frame: FrameFacts,
        roles: Vec<IdentityRole>,
        documents: bool,
    ) -> MobileResult<Vec<IdentityValue>> {
        if roles.len() > MAX_IDENTITY_ROLES {
            return Err(Error::InvalidInput("too many roles").into());
        }
        self.unlocked()?;
        let core: Vec<FillRole> = roles.iter().map(|r| r.core()).collect();
        let place = self.place(&target, &frame)?;
        let vault = self.client.vault()?;
        let found = match &place {
            Place::Page { page, top } => {
                vault.identity_values_for_page(page, Some(top.as_str()), &core, documents)?
            }
            Place::App => vault.identity_values_for_app(&core, documents)?,
        };
        Ok(found
            .into_iter()
            .map(|(role, value): (FillRole, SecretString)| IdentityValue {
                role: IdentityRole::of(role),
                value: value.expose().to_owned(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autofill::{FrameFacts, TargetFacts};
    use crate::testing::seed_identity;
    use crate::vault::tests::{code, overdue_refuses, unlocked};

    const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";

    fn chrome(domain: &str, scheme: &str) -> TargetFacts {
        TargetFacts {
            package_name: "com.android.chrome".into(),
            signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(CHROME)
                .unwrap()
                .to_vec()],
            web_domain: Some(domain.into()),
            web_scheme: Some(scheme.into()),
        }
    }

    fn app() -> TargetFacts {
        TargetFacts {
            package_name: "com.shop.android".into(),
            signing_certs: vec![vec![3; 32]],
            web_domain: None,
            web_scheme: None,
        }
    }

    fn page() -> FrameFacts {
        FrameFacts {
            web_domain: None,
            web_scheme: None,
        }
    }

    fn roles_of(values: Vec<IdentityValue>) -> Vec<IdentityRole> {
        values.into_iter().map(|v| v.role).collect()
    }

    const ASKED: [IdentityRole; 3] = [
        IdentityRole::FirstName,
        IdentityRole::Phone,
        IdentityRole::Cpf,
    ];

    #[test]
    fn a_page_is_offered_the_identity_and_its_roles() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_identity(&v);
        let c = v
            .autofill_identity(chrome("shop.example.com", "https"), page())
            .unwrap()
            .unwrap();
        assert_eq!(c.title, "Samuel Rocha");
        assert_eq!(c.email.as_deref(), Some("user@example.com"));
        assert!(c.roles.contains(&IdentityRole::Cpf));
        assert!(c.roles.contains(&IdentityRole::PostalCode));
    }

    #[test]
    fn documents_only_when_confirmed_and_never_on_http() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_identity(&v);
        let https = || chrome("shop.example.com", "https");
        let http = || chrome("shop.example.com", "http");
        assert_eq!(
            roles_of(
                v.autofill_identity_values(https(), page(), ASKED.to_vec(), false)
                    .unwrap()
            ),
            vec![IdentityRole::FirstName, IdentityRole::Phone]
        );
        assert_eq!(
            roles_of(
                v.autofill_identity_values(https(), page(), ASKED.to_vec(), true)
                    .unwrap()
            ),
            ASKED.to_vec()
        );
        assert_eq!(
            roles_of(
                v.autofill_identity_values(http(), page(), ASKED.to_vec(), true)
                    .unwrap()
            ),
            vec![IdentityRole::FirstName, IdentityRole::Phone]
        );
        let on_http = v.autofill_identity(http(), page()).unwrap().unwrap();
        assert!(!on_http.roles.iter().any(|r| is_document_role(*r)));
        assert_eq!(
            roles_of(
                v.autofill_identity_values(app(), page(), ASKED.to_vec(), true)
                    .unwrap()
            ),
            ASKED.to_vec()
        );
    }

    #[test]
    fn a_cross_site_frame_gets_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_identity(&v);
        let ads = FrameFacts {
            web_domain: Some("ads.example.net".into()),
            web_scheme: Some("https".into()),
        };
        let shop = || chrome("shop.example.com", "https");
        assert_eq!(
            code(v.autofill_identity(shop(), ads.clone()).err().unwrap()),
            "denied"
        );
        assert_eq!(
            code(
                v.autofill_identity_values(shop(), ads, ASKED.to_vec(), false)
                    .err()
                    .unwrap()
            ),
            "denied"
        );
    }

    #[test]
    fn no_identity_offers_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        assert!(v.autofill_identity(app(), page()).unwrap().is_none());
    }

    #[test]
    fn a_locked_or_overdue_vault_serves_no_identity() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        seed_identity(&v);
        overdue_refuses(&v, &seen, |v| v.autofill_identity(app(), page()));
        overdue_refuses(&v, &seen, |v| {
            v.autofill_identity_values(app(), page(), ASKED.to_vec(), false)
        });
    }

    #[test]
    fn requests_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_identity(&v);
        let many = vec![IdentityRole::City; MAX_IDENTITY_ROLES + 1];
        assert_eq!(
            code(
                v.autofill_identity_values(app(), page(), many, false)
                    .err()
                    .unwrap()
            ),
            "invalid_input"
        );
    }

    #[test]
    fn the_role_lists_match_the_core_one_for_one() {
        for (i, role) in ROLES.iter().enumerate() {
            assert_eq!(role.core(), FillRole::ALL[i]);
            assert_eq!(IdentityRole::of(FillRole::ALL[i]), *role);
        }
        assert!(is_document_role(IdentityRole::Rg));
        assert!(!is_document_role(IdentityRole::Email));
    }
}
