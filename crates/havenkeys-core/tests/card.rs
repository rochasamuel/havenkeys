//! The Card item (spec 2026-09-29-card-item).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::card::{CardBrand, CardExpiry, CardField, CardInput};
use havenkeys_core::model::{ItemInput, ItemType, SecretField, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::{Error, SecretString};
use uuid::Uuid;

fn card_input(title: &str, card: CardInput) -> ItemInput {
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
        card: Some(card),
    }
}

fn some(v: &str) -> Option<SecretString> {
    Some(secret(v))
}

fn mastercard() -> CardInput {
    CardInput {
        cardholder_name: some("Samuel S Rocha"),
        brand: None,
        number: SecretUpdate::Set(secret("5200 8282 8282 8210")),
        verification_number: SecretUpdate::Set(secret("123")),
        expiry: Some(CardExpiry::new(2033, 11).unwrap()),
        notes: None,
    }
}

fn create(v: &mut VaultService, input: ItemInput) -> Uuid {
    let staged = v.stage_create(input, NOW).unwrap();
    v.commit_write(staged, 1).unwrap().unwrap().id
}

fn update(v: &mut VaultService, id: &Uuid, input: ItemInput, rev: i64) {
    let staged = v.stage_update(id, input, NOW + 1).unwrap();
    v.commit_write(staged, rev).unwrap();
}

#[test]
fn a_card_round_trips_and_its_overview_holds_only_the_summary() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("", mastercard()));

    let ov = v.get_item(&id).unwrap();
    assert_eq!(ov.item_type, ItemType::Card);
    assert_eq!(ov.title, "Mastercard", "empty title → brand name");
    assert_eq!(ov.username, None);
    assert!(ov.urls.is_empty());
    assert!(!ov.has_password && !ov.has_totp && !ov.has_notes);
    let summary = ov.card.as_ref().unwrap();
    assert_eq!(summary.brand, CardBrand::Mastercard);
    assert_eq!(summary.last4.as_deref(), Some("8210"));
    assert_eq!(summary.expiry, Some(CardExpiry::new(2033, 11).unwrap()));
    let json = serde_json::to_string(&ov).unwrap();
    assert!(!json.contains("5200828282828210") && !json.contains("Samuel"));
    assert!(
        !json.contains("verification"),
        "no CVV field in the overview"
    );

    let f = v.card_fields(id).unwrap();
    assert_eq!(f.number.as_ref().unwrap().expose(), "5200828282828210");
    assert_eq!(f.verification_number.as_ref().unwrap().expose(), "123");
    assert_eq!(
        f.cardholder_name.as_ref().unwrap().expose(),
        "Samuel S Rocha"
    );
    assert_eq!(f.brand, None, "detection is not stored as a choice");
    assert_eq!(
        v.card_value(&id, CardField::Expiry).unwrap().expose(),
        "11/2033"
    );
}

#[test]
fn brand_override_is_kept_and_unknown_numbers_are_other() {
    let (mut v, _) = activated_vault();
    let chosen = create(
        &mut v,
        card_input(
            "",
            CardInput {
                brand: Some(CardBrand::Elo),
                ..mastercard()
            },
        ),
    );
    assert_eq!(
        v.get_item(&chosen).unwrap().card.as_ref().unwrap().brand,
        CardBrand::Elo
    );
    assert_eq!(v.get_item(&chosen).unwrap().title, "Elo");

    let unknown = create(
        &mut v,
        card_input(
            "",
            CardInput {
                number: SecretUpdate::Set(secret("9999 9999 9999 9995")),
                ..mastercard()
            },
        ),
    );
    let ov = v.get_item(&unknown).unwrap();
    assert_eq!(ov.card.as_ref().unwrap().brand, CardBrand::Other);
    assert_eq!(ov.title, "Card");
}

#[test]
fn update_keeps_number_and_cvv_and_redetects() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("Nubank", mastercard()));
    update(
        &mut v,
        &id,
        card_input(
            "Nubank",
            CardInput {
                cardholder_name: some("S S Rocha"),
                number: SecretUpdate::Keep,
                verification_number: SecretUpdate::Keep,
                ..mastercard()
            },
        ),
        2,
    );
    let f = v.card_fields(id).unwrap();
    assert_eq!(f.number.as_ref().unwrap().expose(), "5200828282828210");
    assert_eq!(f.verification_number.as_ref().unwrap().expose(), "123");
    assert_eq!(f.cardholder_name.as_ref().unwrap().expose(), "S S Rocha");
    assert_eq!(
        v.get_item(&id).unwrap().card.as_ref().unwrap().brand,
        CardBrand::Mastercard
    );

    update(
        &mut v,
        &id,
        card_input(
            "Nubank",
            CardInput {
                number: SecretUpdate::Set(secret("4111111111111111")),
                verification_number: SecretUpdate::Clear,
                ..mastercard()
            },
        ),
        3,
    );
    let ov = v.get_item(&id).unwrap();
    assert_eq!(ov.card.as_ref().unwrap().brand, CardBrand::Visa);
    assert_eq!(ov.card.as_ref().unwrap().last4.as_deref(), Some("1111"));
    assert!(v.card_fields(id).unwrap().verification_number.is_none());
    assert_eq!(
        v.card_value(&id, CardField::VerificationNumber).err(),
        Some(Error::NotFound)
    );
}

#[test]
fn validation_errors_never_echo_values() {
    let (v, _) = activated_vault();
    let bad = |c: CardInput, title: &str| v.stage_create(card_input(title, c), NOW).err();
    let cases = [
        bad(
            CardInput {
                number: SecretUpdate::Set(secret("1234567")),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                number: SecretUpdate::Set(secret(&"4".repeat(20))),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                number: SecretUpdate::Set(secret("4111x111")),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                verification_number: SecretUpdate::Set(secret("12")),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                verification_number: SecretUpdate::Set(secret("123456789")),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                cardholder_name: some(&"a".repeat(257)),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                cardholder_name: some("Sam\u{7}uel"),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                notes: some(&"n".repeat(64 * 1024 + 1)),
                ..mastercard()
            },
            "",
        ),
        bad(
            CardInput {
                number: SecretUpdate::Keep,
                ..mastercard()
            },
            "  ",
        ),
    ];
    for (i, e) in cases.into_iter().enumerate() {
        match e {
            Some(Error::InvalidInput(msg)) => {
                for secret in ["4111", "5200", "Samuel", "123"] {
                    assert!(!msg.contains(secret), "case {i}: {msg}");
                }
            }
            other => panic!("case {i}: expected InvalidInput, got {other:?}"),
        }
    }
    // A title without a number is a valid card.
    assert!(v
        .stage_create(
            card_input(
                "Old card",
                CardInput {
                    number: SecretUpdate::Keep,
                    ..mastercard()
                }
            ),
            NOW
        )
        .is_ok());
}

#[test]
fn shapes_are_enforced() {
    let (mut v, _) = activated_vault();
    let mut with_username = card_input("x", mastercard());
    with_username.username = Some("me".into());
    assert!(matches!(
        v.stage_create(with_username, NOW),
        Err(Error::InvalidInput(_))
    ));

    let mut no_card = card_input("x", mastercard());
    no_card.card = None;
    assert!(matches!(
        v.stage_create(no_card, NOW),
        Err(Error::InvalidInput(_))
    ));

    let mut login_with_card = login("GitHub", "octo", "pw", "github.com");
    login_with_card.card = Some(mastercard());
    assert!(matches!(
        v.stage_create(login_with_card, NOW),
        Err(Error::InvalidInput(_))
    ));

    let login_id = create(&mut v, login("GitHub", "octo", "pw", "github.com"));
    assert!(matches!(
        v.stage_update(&login_id, card_input("x", mastercard()), NOW),
        Err(Error::InvalidInput(_))
    ));
    assert!(serde_json::from_str::<ItemInput>(
        r#"{"itemType":"card","title":"","card":{"number":{"op":"set","value":"4111111111111111"},"pin":"1"}}"#
    )
    .is_err());
}

#[test]
fn search_matches_last4_only() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("Nubank", mastercard()));
    create(&mut v, note("Wi-Fi 8210", "x"));
    let hits: Vec<Uuid> = v.search("8210").unwrap().iter().map(|o| o.id).collect();
    assert!(hits.contains(&id));
    assert!(v.search("82828282").unwrap().iter().all(|o| o.id != id));
    assert!(v.search("nubank").unwrap().iter().any(|o| o.id == id));
}

#[test]
fn card_reads_are_typed_and_need_an_unlocked_vault() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("", mastercard()));
    let login_id = create(&mut v, login("GitHub", "octo", "pw", "github.com"));
    assert_eq!(v.card_fields(login_id).err(), Some(Error::Denied));
    assert_eq!(v.card_fields(Uuid::new_v4()).err(), Some(Error::NotFound));
    assert!(v.reveal(&id, SecretField::Password).is_err());
    v.lock();
    assert_eq!(v.card_fields(id).err(), Some(Error::Locked));
    assert_eq!(
        v.card_value(&id, CardField::Number).err(),
        Some(Error::Locked)
    );
}

/// Nothing the browser extension can ask for today returns a card.
#[test]
fn the_extension_can_never_read_a_card() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("", mastercard()));
    for page in [
        "https://github.com/login",
        "https://shop.example/checkout",
        "http://localhost:3000/",
    ] {
        assert!(v
            .find_matches(page, None)
            .unwrap()
            .iter()
            .all(|s| s.id != id));
        assert_eq!(
            v.fill_for_page(&id, page, None, NOW).err(),
            Some(Error::Denied)
        );
    }
    assert_eq!(v.totp_code(&id, 0).err(), Some(Error::NotFound));
}

/// The page-bound requests never return a card, on any origin.
#[test]
fn page_requests_never_yield_a_card() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("", mastercard()));
    let staged = v
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .unwrap();
    let identity_id = staged.item_id;
    v.commit_write(staged, 2).unwrap();
    for page in [
        "https://github.com/login",
        "https://shop.example/checkout",
        "http://localhost:3000/",
    ] {
        assert!(matches!(
            v.totp_for_page(&id, page, None, 0).err(),
            Some(Error::Denied | Error::NotFound)
        ));
        assert_eq!(v.identity_id_for_page(page, None).ok(), Some(identity_id));
        if let Ok(summary) = v.identity_summary_for_page(page, None) {
            assert_ne!(summary.title, "Mastercard");
        }
        let all = havenkeys_core::identity::FillRole::ALL;
        if let Ok(values) = v.identity_values_for_page(page, None, &all, true) {
            for (_, value) in values {
                assert!(!value.expose().contains("5200") && !value.expose().contains("Samuel S"));
            }
        }
    }
}

#[test]
fn a_card_can_be_deleted() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, card_input("", mastercard()));
    let staged = v.stage_delete(&id).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert_eq!(v.get_item(&id).err(), Some(Error::NotFound));
}
