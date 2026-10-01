//! Cards for web pages (spec 2026-09-29-card-autofill §6.2, §6.3).

mod common;

use common::{activated_vault, login, secret, NOW};
use havenkeys_core::card::{CardExpiry, CardInput};
use havenkeys_core::card_page::{CardFrame, CardRole, NewCard};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;
use uuid::Uuid;

const VISA: &str = "4111111111111111";
const SHOP: &str = "https://shop.example.com/checkout";

fn card_input(title: &str, number: &str, cvv: Option<&str>) -> ItemInput {
    ItemInput {
        item_type: ItemType::Card,
        title: title.into(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: Some(CardInput {
            cardholder_name: Some(secret("Samuel  S Rocha")),
            brand: None,
            number: SecretUpdate::Set(secret(number)),
            verification_number: cvv.map_or(SecretUpdate::Keep, |c| SecretUpdate::Set(secret(c))),
            expiry: Some(CardExpiry {
                year: 2033,
                month: 4,
            }),
            notes: None,
        }),
        sections: None,
    }
}

/// A vault holding one Visa (with CVV) and one login.
fn with_card() -> (VaultService, Uuid, Uuid) {
    let (mut v, _) = activated_vault();
    let staged = v
        .stage_create(card_input("Visa pessoal", VISA, Some("123")), NOW)
        .unwrap();
    let card = v.commit_write(staged, 1).unwrap().unwrap().id;
    let staged = v
        .stage_create(login("GitHub", "octo", "pw", "https://github.com"), NOW)
        .unwrap();
    let github = v.commit_write(staged, 2).unwrap().unwrap().id;
    (v, card, github)
}

fn values(
    v: &VaultService,
    id: &Uuid,
    top: &str,
    frames: &[(&str, &[CardRole])],
) -> Result<Vec<Vec<(CardRole, String)>>, Error> {
    let frames: Vec<CardFrame<'_>> = frames
        .iter()
        .map(|(url, roles)| CardFrame { url, roles })
        .collect();
    v.card_values_for_page(id, top, &frames).map(|all| {
        all.into_iter()
            .map(|f| {
                f.into_iter()
                    .map(|(r, s)| (r, s.expose().to_owned()))
                    .collect()
            })
            .collect()
    })
}

#[test]
fn https_pages_get_the_cards_overview_and_no_secrets() {
    let (v, card, _) = with_card();
    let list = v.cards_for_page(SHOP, None).unwrap();
    assert!(!list.insecure);
    assert_eq!(list.cards.len(), 1, "only cards, never logins");
    let c = &list.cards[0];
    assert_eq!(c.id, card);
    assert_eq!(c.title, "Visa pessoal");
    assert_eq!(c.last4.as_deref(), Some("1111"));
    assert_eq!(c.expiry.map(|e| (e.year, e.month)), Some((2033, 4)));
    // Debug is redacted. A fixed id keeps this deterministic: a random UUID
    // could contain any short digit run.
    let fixed = havenkeys_core::card_page::CardOffer {
        id: Uuid::nil(),
        title: c.title.clone(),
        brand: c.brand,
        last4: c.last4.clone(),
        expiry: c.expiry,
    };
    let dbg = format!("{fixed:?}");
    assert!(!dbg.contains("1111") && !dbg.contains("Visa pessoal") && !dbg.contains("2033"));
}

#[test]
fn http_pages_are_told_they_are_insecure_and_get_nothing() {
    let (v, card, _) = with_card();
    let list = v.cards_for_page("http://shop.example.com/", None).unwrap();
    assert!(list.insecure);
    assert!(list.cards.is_empty());
    let r = values(
        &v,
        &card,
        "http://shop.example.com/",
        &[("http://shop.example.com/", &[CardRole::Number])],
    );
    assert!(matches!(r, Err(Error::Denied)));
}

#[test]
fn processor_and_same_site_frames_are_served() {
    let (v, _card, _) = with_card();
    for frame in [
        "https://js.stripe.com/v3/elements-inner-card.html",
        "https://b.js.stripe.com/v3/fingerprinted/x.html",
        "https://checkoutshopper-live.adyen.com/checkoutshopper/securedfields/x/securedFields.html",
        "https://assets.braintreegateway.com/web/3.0/html/hosted-fields-frame.min.html",
        "https://secure-fields.mercadopago.com/",
        "https://pay.example.com/card",
    ] {
        assert!(
            !v.cards_for_page(frame, Some(SHOP)).unwrap().insecure,
            "{frame}"
        );
    }
}

#[test]
fn look_alike_and_cross_site_frames_are_denied() {
    let (v, _, _) = with_card();
    for frame in [
        "https://js.stripe.com.evil.com/",
        "https://evil-js.stripe.com/",
        "https://evil.com/card",
        "http://js.stripe.com/v3/",
        "https://js.stripe.com:8443/",
    ] {
        assert!(
            matches!(v.cards_for_page(frame, Some(SHOP)), Err(Error::Denied)),
            "{frame}"
        );
    }
}

#[test]
fn values_come_per_frame_as_asked_and_derived_in_rust() {
    let (v, card, _) = with_card();
    let got = values(
        &v,
        &card,
        SHOP,
        &[
            (
                SHOP,
                &[
                    CardRole::CardholderName,
                    CardRole::CardholderGivenName,
                    CardRole::CardholderFamilyName,
                ],
            ),
            (
                "https://js.stripe.com/v3/elements-inner-card.html",
                &[
                    CardRole::Number,
                    CardRole::ExpiryMonth,
                    CardRole::ExpiryYear,
                    CardRole::VerificationNumber,
                    CardRole::Brand,
                ],
            ),
        ],
    )
    .unwrap();
    assert_eq!(
        got[0],
        vec![
            (CardRole::CardholderName, "Samuel S Rocha".to_owned()),
            (CardRole::CardholderGivenName, "Samuel".to_owned()),
            (CardRole::CardholderFamilyName, "S Rocha".to_owned()),
        ]
    );
    assert_eq!(
        got[1],
        vec![
            (CardRole::Number, VISA.to_owned()),
            (CardRole::ExpiryMonth, "4".to_owned()),
            (CardRole::ExpiryYear, "2033".to_owned()),
            (CardRole::VerificationNumber, "123".to_owned()),
            (CardRole::Brand, "visa".to_owned()),
        ]
    );
}

#[test]
fn roles_without_a_value_are_left_out() {
    let (mut v, _, _) = with_card();
    let staged = v
        .stage_create(card_input("", "5555555555554444", None), NOW)
        .unwrap();
    let no_cvv = v.commit_write(staged, 3).unwrap().unwrap().id;
    let got = values(
        &v,
        &no_cvv,
        SHOP,
        &[(SHOP, &[CardRole::VerificationNumber, CardRole::Number])],
    )
    .unwrap();
    assert_eq!(
        got[0],
        vec![(CardRole::Number, "5555555555554444".to_owned())]
    );
}

#[test]
fn one_bad_frame_denies_the_whole_request() {
    let (v, card, _) = with_card();
    let r = values(
        &v,
        &card,
        SHOP,
        &[
            (SHOP, &[CardRole::Number]),
            ("https://ads.example.net/", &[CardRole::Number]),
        ],
    );
    assert!(matches!(r, Err(Error::Denied)));
}

/// Attack: evil.com frames shop.com's checkout to collect the card through the user's click.
#[test]
fn a_checkout_framed_by_another_site_gets_nothing() {
    let (v, card, _) = with_card();
    let r = values(
        &v,
        &card,
        "https://evil.com/",
        &[(SHOP, &[CardRole::Number])],
    );
    assert!(matches!(r, Err(Error::Denied)));
}

#[test]
fn a_login_and_a_missing_item_answer_the_same() {
    let (v, _, github) = with_card();
    let a = values(&v, &github, SHOP, &[(SHOP, &[CardRole::Number])]);
    let b = values(&v, &Uuid::new_v4(), SHOP, &[(SHOP, &[CardRole::Number])]);
    assert!(matches!(a, Err(Error::NotFound)));
    assert!(matches!(b, Err(Error::NotFound)));
}

#[test]
fn locked_vaults_answer_locked() {
    let (mut v, card, _) = with_card();
    v.lock();
    assert!(matches!(v.cards_for_page(SHOP, None), Err(Error::Locked)));
    assert!(matches!(
        values(&v, &card, SHOP, &[(SHOP, &[CardRole::Number])]),
        Err(Error::Locked)
    ));
}

fn new_card(number: &str) -> NewCard<'static> {
    NewCard {
        title: None,
        cardholder_name: Some("Samuel Rocha"),
        number: secret(number),
        verification_number: Some(secret("123")),
        expiry: Some("2030-01"),
    }
}

#[test]
fn a_typed_card_is_saved_from_the_checkout() {
    let (mut v, _, _) = with_card();
    let staged = v
        .stage_save_card(SHOP, None, new_card("4000 0566 5566 5556"), NOW)
        .unwrap();
    let id = staged.item_id;
    v.commit_write(staged.write, 4).unwrap();
    let list = v.cards_for_page(SHOP, None).unwrap();
    let saved = list.cards.iter().find(|c| c.id == id).unwrap();
    assert_eq!(
        saved.title, "Visa",
        "the brand names a card saved without a title"
    );
    assert_eq!(saved.last4.as_deref(), Some("5556"));
    assert_eq!(saved.expiry.map(|e| (e.year, e.month)), Some((2030, 1)));
}

#[test]
fn saving_refuses_bad_numbers_and_foreign_or_insecure_frames() {
    let (v, _, _) = with_card();
    assert!(matches!(
        v.stage_save_card(SHOP, None, new_card("4111111111111112"), NOW),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        v.stage_save_card("http://shop.example.com/", None, new_card(VISA), NOW),
        Err(Error::Denied)
    ));
    // A processor's frame fills, but a save comes only from the shop's own pages.
    assert!(matches!(
        v.stage_save_card("https://js.stripe.com/v3/", Some(SHOP), new_card(VISA), NOW),
        Err(Error::Denied)
    ));
    assert!(matches!(
        v.stage_save_card(SHOP, Some("https://evil.com/"), new_card(VISA), NOW),
        Err(Error::Denied)
    ));
    let bad_expiry = NewCard {
        expiry: Some("2030-13"),
        ..new_card(VISA)
    };
    assert!(matches!(
        v.stage_save_card(SHOP, None, bad_expiry, NOW),
        Err(Error::InvalidInput(_))
    ));
}
