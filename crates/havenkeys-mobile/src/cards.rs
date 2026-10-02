//! Cards in Android Autofill (spec 2026-10-01-android-app §7.6). Kotlin
//! reports the target, the frames its card fields are in and the roles each
//! asks for; the extension's card rules (`card_page.rs`) decide here: https
//! pages or apps only, a frame only when it is the page's site or a payment
//! processor's, only the roles asked for, and saving only after Android's
//! save sheet was confirmed.

use crate::autofill::{frame_url, FrameFacts, TargetFacts};
use crate::error::MobileResult;
use crate::items::parse_id;
use crate::save::SaveResult;
use crate::vault::MobileVault;
use havenkeys_core::app_target::FillTarget;
use havenkeys_core::card_page::{
    frame_may_get_cards, CardFrame, CardOffer, CardRole as CoreRole, NewCard,
};
use havenkeys_core::vault::StagedWrite;
use havenkeys_core::{Error, SecretString};

/// Frames per request, at most: a checkout with a few processor iframes.
pub const MAX_CARD_FRAMES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CardRole {
    CardholderName,
    CardholderGivenName,
    CardholderFamilyName,
    Number,
    VerificationNumber,
    ExpiryMonth,
    ExpiryYear,
    Brand,
}

impl CardRole {
    fn core(self) -> CoreRole {
        match self {
            CardRole::CardholderName => CoreRole::CardholderName,
            CardRole::CardholderGivenName => CoreRole::CardholderGivenName,
            CardRole::CardholderFamilyName => CoreRole::CardholderFamilyName,
            CardRole::Number => CoreRole::Number,
            CardRole::VerificationNumber => CoreRole::VerificationNumber,
            CardRole::ExpiryMonth => CoreRole::ExpiryMonth,
            CardRole::ExpiryYear => CoreRole::ExpiryYear,
            CardRole::Brand => CoreRole::Brand,
        }
    }

    fn of(role: CoreRole) -> Self {
        match role {
            CoreRole::CardholderName => CardRole::CardholderName,
            CoreRole::CardholderGivenName => CardRole::CardholderGivenName,
            CoreRole::CardholderFamilyName => CardRole::CardholderFamilyName,
            CoreRole::Number => CardRole::Number,
            CoreRole::VerificationNumber => CardRole::VerificationNumber,
            CoreRole::ExpiryMonth => CardRole::ExpiryMonth,
            CoreRole::ExpiryYear => CardRole::ExpiryYear,
            CoreRole::Brand => CardRole::Brand,
        }
    }
}

/// A card offered in Android's list: overview data only.
#[derive(Debug, uniffi::Record)]
pub struct CardChoice {
    pub id: String,
    pub title: String,
    /// The wire id: `"visa"`, `"mastercard"`, …, `"other"`.
    pub brand: String,
    pub last4: Option<String>,
    /// `"YYYY-MM"`.
    pub expiry: Option<String>,
}

#[derive(Debug, uniffi::Record)]
pub struct CardChoices {
    /// A browser page that is not https: no card is offered there.
    pub insecure: bool,
    pub cards: Vec<CardChoice>,
    /// Per frame of the request: may it be filled with a card?
    pub frames: Vec<bool>,
}

#[derive(Clone, uniffi::Record)]
pub struct CardFrameRoles {
    pub frame: FrameFacts,
    pub roles: Vec<CardRole>,
}

/// One value of the card. `Debug` names the role only: the value is the
/// number or the code.
#[derive(uniffi::Record)]
pub struct CardValue {
    pub role: CardRole,
    pub value: String,
}

impl std::fmt::Debug for CardValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardValue")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// A card typed into a form, after Android's save sheet. No `Debug`.
#[derive(uniffi::Record)]
pub struct SaveCard {
    pub cardholder_name: Option<String>,
    pub number: String,
    pub verification_number: Option<String>,
    /// `"YYYY-MM"`.
    pub expiry: Option<String>,
}

fn choice(c: CardOffer) -> CardChoice {
    CardChoice {
        id: c.id.to_string(),
        title: c.title,
        brand: c.brand.map_or("other", |b| b.id()).to_owned(),
        last4: c.last4,
        expiry: c.expiry.map(|e| e.to_wire()),
    }
}

fn values(found: Vec<(CoreRole, SecretString)>) -> Vec<CardValue> {
    found
        .into_iter()
        .map(|(role, value)| CardValue {
            role: CardRole::of(role),
            value: value.expose().to_owned(),
        })
        .collect()
}

fn bounded<T>(frames: &[T]) -> MobileResult<()> {
    if frames.len() > MAX_CARD_FRAMES {
        return Err(Error::InvalidInput("too many frames").into());
    }
    Ok(())
}

impl MobileVault {
    /// The sealed write for a typed card, not sent. `None`: already saved.
    pub(crate) fn stage_card_save(
        &self,
        target: &TargetFacts,
        frame: &FrameFacts,
        card: SaveCard,
    ) -> MobileResult<Option<StagedWrite>> {
        self.unlocked()?;
        let number = SecretString::new(card.number);
        let target = self.target(target)?;
        let vault = self.client.vault()?;
        if vault.card_number_saved(&number)? {
            return Ok(None);
        }
        let new = NewCard {
            title: None,
            cardholder_name: card.cardholder_name.as_deref(),
            number,
            verification_number: card.verification_number.map(SecretString::new),
            expiry: card.expiry.as_deref(),
        };
        let now = havenkeys_client::now_ms();
        let staged = match target {
            FillTarget::Browser { page_url } => {
                let page = frame_url(&page_url, frame).ok_or(Error::Denied)?;
                vault.stage_save_card(&page, Some(page_url.as_str()), new, now)?
            }
            FillTarget::App(_) => vault.stage_save_card_for_app(new, now)?,
        };
        Ok(Some(staged.write))
    }
}

#[uniffi::export]
impl MobileVault {
    /// The cards a form may be offered, and which of its frames may get one.
    pub fn autofill_cards(
        &self,
        target: TargetFacts,
        frames: Vec<FrameFacts>,
    ) -> MobileResult<CardChoices> {
        bounded(&frames)?;
        self.unlocked()?;
        let vault = self.client.vault()?;
        Ok(match self.target(&target)? {
            FillTarget::Browser { page_url } => {
                let list = vault.cards_for_page(&page_url, None)?;
                let allowed = frames
                    .iter()
                    .map(|f| {
                        !list.insecure
                            && frame_url(&page_url, f)
                                .is_some_and(|url| frame_may_get_cards(&url, &page_url))
                    })
                    .collect();
                CardChoices {
                    insecure: list.insecure,
                    cards: list.cards.into_iter().map(choice).collect(),
                    frames: allowed,
                }
            }
            FillTarget::App(_) => CardChoices {
                insecure: false,
                cards: vault.cards_for_app()?.into_iter().map(choice).collect(),
                frames: vec![true; frames.len()],
            },
        })
    }

    /// One card's values for each frame, in order. Every frame must be
    /// allowed, or nothing is returned.
    pub fn autofill_card_values(
        &self,
        id: String,
        target: TargetFacts,
        frames: Vec<CardFrameRoles>,
    ) -> MobileResult<Vec<Vec<CardValue>>> {
        bounded(&frames)?;
        let id = parse_id(&id)?;
        self.unlocked()?;
        let roles: Vec<Vec<CoreRole>> = frames
            .iter()
            .map(|f| f.roles.iter().map(|r| r.core()).collect())
            .collect();
        let vault = self.client.vault()?;
        let found = match self.target(&target)? {
            FillTarget::Browser { page_url } => {
                let urls = frames
                    .iter()
                    .map(|f| frame_url(&page_url, &f.frame).ok_or(Error::Denied))
                    .collect::<Result<Vec<String>, Error>>()?;
                let core: Vec<CardFrame<'_>> = urls
                    .iter()
                    .zip(&roles)
                    .map(|(url, roles)| CardFrame { url, roles })
                    .collect();
                vault.card_values_for_page(&id, &page_url, &core)?
            }
            FillTarget::App(_) => roles
                .iter()
                .map(|r| vault.card_values_for_app(&id, r))
                .collect::<Result<Vec<_>, Error>>()?,
        };
        Ok(found.into_iter().map(values).collect())
    }

    /// After Android's save sheet was confirmed for a card. A card already
    /// saved is `Unchanged`, even offline; a new one needs the server. Not
    /// app use: the idle timer is not touched.
    pub fn autofill_save_card(
        &self,
        target: TargetFacts,
        frame: FrameFacts,
        card: SaveCard,
    ) -> MobileResult<SaveResult> {
        match self.stage_card_save(&target, &frame, card)? {
            None => Ok(SaveResult::Unchanged),
            Some(staged) => {
                self.client.require_online()?;
                self.send(staged)?;
                Ok(SaveResult::Added)
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::autofill::{FrameFacts, TargetFacts};
    use crate::testing::seed_card;
    use crate::vault::tests::{code, overdue_refuses, unlocked};

    const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";
    const VISA: &str = "4111111111111111";

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

    fn frame(domain: &str) -> FrameFacts {
        FrameFacts {
            web_domain: Some(domain.into()),
            web_scheme: Some("https".into()),
        }
    }

    fn roles(frame: FrameFacts, roles: &[CardRole]) -> CardFrameRoles {
        CardFrameRoles {
            frame,
            roles: roles.to_vec(),
        }
    }

    fn plain(values: Vec<Vec<CardValue>>) -> Vec<Vec<(CardRole, String)>> {
        values
            .into_iter()
            .map(|f| f.into_iter().map(|v| (v.role, v.value)).collect())
            .collect()
    }

    fn card(number: &str) -> SaveCard {
        SaveCard {
            cardholder_name: Some("Samuel Rocha".into()),
            number: number.into(),
            verification_number: Some("123".into()),
            expiry: Some("2030-01".into()),
        }
    }

    #[test]
    fn an_https_checkout_in_chrome_is_offered_the_cards() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = seed_card(&v, "Visa pessoal", VISA);
        let choices = v
            .autofill_cards(chrome("shop.example.com", "https"), vec![page()])
            .unwrap();
        assert!(!choices.insecure);
        assert_eq!(choices.frames, vec![true]);
        assert_eq!(choices.cards.len(), 1);
        let c = &choices.cards[0];
        assert_eq!(
            (c.id.as_str(), c.title.as_str(), c.brand.as_str()),
            (id.as_str(), "Visa pessoal", "visa")
        );
        assert_eq!(c.last4.as_deref(), Some("1111"));
        assert_eq!(c.expiry.as_deref(), Some("2033-04"));
        let values = v
            .autofill_card_values(
                id,
                chrome("shop.example.com", "https"),
                vec![roles(page(), &[CardRole::Number, CardRole::ExpiryYear])],
            )
            .unwrap();
        assert_eq!(
            plain(values),
            vec![vec![
                (CardRole::Number, VISA.to_owned()),
                (CardRole::ExpiryYear, "2033".to_owned()),
            ]]
        );
    }

    #[test]
    fn an_http_checkout_gets_no_card() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = seed_card(&v, "Visa", VISA);
        let choices = v
            .autofill_cards(chrome("shop.example.com", "http"), vec![page()])
            .unwrap();
        assert!(choices.insecure);
        assert!(choices.cards.is_empty());
        assert_eq!(choices.frames, vec![false]);
        let err = v
            .autofill_card_values(
                id,
                chrome("shop.example.com", "http"),
                vec![roles(page(), &[CardRole::Number])],
            )
            .unwrap_err();
        assert_eq!(code(err), "denied");
    }

    #[test]
    fn a_refused_frame_is_reported_and_values_refuse_it() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = seed_card(&v, "Visa", VISA);
        let shop = || chrome("shop.example.com", "https");
        let choices = v
            .autofill_cards(
                shop(),
                vec![page(), frame("js.stripe.com"), frame("ads.example.net")],
            )
            .unwrap();
        assert_eq!(choices.frames, vec![true, true, false]);
        // Only the allowed frames: answered, one list per frame.
        let ok = v
            .autofill_card_values(
                id.clone(),
                shop(),
                vec![
                    roles(page(), &[CardRole::CardholderName]),
                    roles(frame("js.stripe.com"), &[CardRole::Number]),
                ],
            )
            .unwrap();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok[1][0].value, VISA);
        // A refused frame anywhere in the request: nothing at all.
        let err = v
            .autofill_card_values(
                id,
                shop(),
                vec![
                    roles(page(), &[CardRole::CardholderName]),
                    roles(frame("ads.example.net"), &[CardRole::Number]),
                ],
            )
            .unwrap_err();
        assert_eq!(code(err), "denied");
    }

    #[test]
    fn an_app_gets_the_cards_whatever_its_frames_claim() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = seed_card(&v, "Visa", VISA);
        let choices = v
            .autofill_cards(app(), vec![page(), frame("evil.example")])
            .unwrap();
        assert_eq!(choices.frames, vec![true, true]);
        assert_eq!(choices.cards.len(), 1);
        let values = v
            .autofill_card_values(
                id,
                app(),
                vec![roles(
                    frame("evil.example"),
                    &[CardRole::VerificationNumber],
                )],
            )
            .unwrap();
        assert_eq!(values[0][0].value, "123");
    }

    #[test]
    fn a_login_asked_for_as_a_card_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        crate::testing::seed_with_github_login_unlocked(&v);
        let github = v
            .list_items()
            .unwrap()
            .into_iter()
            .find(|i| i.title == "GitHub")
            .unwrap()
            .id;
        let err = v
            .autofill_card_values(github, app(), vec![roles(page(), &[CardRole::Number])])
            .unwrap_err();
        assert_eq!(code(err), "not_found");
    }

    #[test]
    fn requests_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = seed_card(&v, "Visa", VISA);
        let many = vec![page(); MAX_CARD_FRAMES + 1];
        assert_eq!(
            code(v.autofill_cards(app(), many).unwrap_err()),
            "invalid_input"
        );
        let many = vec![roles(page(), &[CardRole::Number]); MAX_CARD_FRAMES + 1];
        assert_eq!(
            code(v.autofill_card_values(id, app(), many).unwrap_err()),
            "invalid_input"
        );
    }

    #[test]
    fn a_locked_or_overdue_vault_serves_no_card() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        let id = seed_card(&v, "Visa", VISA);
        overdue_refuses(&v, &seen, |v| v.autofill_cards(app(), vec![page()]));
        overdue_refuses(&v, &seen, |v| {
            v.autofill_card_values(id.clone(), app(), vec![roles(page(), &[CardRole::Number])])
        });
        overdue_refuses(&v, &seen, |v| {
            v.autofill_save_card(app(), page(), card(VISA))
        });
    }

    #[test]
    fn a_known_card_is_unchanged_even_offline() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        seed_card(&v, "Visa", VISA);
        assert!(matches!(
            v.autofill_save_card(app(), page(), card("4111 1111 1111 1111"))
                .unwrap(),
            SaveResult::Unchanged
        ));
        assert_eq!(v.list_items().unwrap().len(), 1);
    }

    #[test]
    fn a_new_card_needs_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let err = v
            .autofill_save_card(app(), page(), card("4000 0566 5566 5556"))
            .unwrap_err();
        assert_eq!(code(err), "offline");
        assert!(v.list_items().unwrap().is_empty());
    }

    #[test]
    fn a_new_card_is_staged_by_the_frame_rules() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let shop = || chrome("shop.example.com", "https");
        // The shop's own page: staged, named after its brand.
        let staged = v
            .stage_card_save(&shop(), &page(), card("4000 0566 5566 5556"))
            .unwrap()
            .unwrap();
        v.client.vault().unwrap().commit_write(staged, 7).unwrap();
        let saved = v.list_items().unwrap();
        assert_eq!(saved[0].title, "Visa");
        // A processor's frame or an http page: never.
        for (target, frame) in [
            (shop(), frame("js.stripe.com")),
            (chrome("shop.example.com", "http"), page()),
        ] {
            let err = v
                .stage_card_save(&target, &frame, card("5555 5555 5555 4444"))
                .unwrap_err();
            assert_eq!(code(err), "denied");
        }
        // A number that fails its check digit: refused.
        let err = v
            .stage_card_save(&app(), &page(), card("4000 0566 5566 5557"))
            .unwrap_err();
        assert_eq!(code(err), "invalid_input");
    }
}
