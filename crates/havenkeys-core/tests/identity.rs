//! The account's one Identity item (spec 2026-09-29-identity-item).

mod common;

use common::{activated_vault, login, second_device, secret, NOW};
use havenkeys_core::identity::{CustomField, IdentityField, IdentityFields};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;
use uuid::Uuid;

fn identity_input(fields: IdentityFields) -> ItemInput {
    ItemInput {
        item_type: ItemType::Identity,
        title: String::new(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: Some(fields),
        card: None,
    }
}

fn some(v: &str) -> Option<havenkeys_core::SecretString> {
    Some(secret(v))
}

/// Create the identity the way a device does at connect, and commit it.
fn created(vault: &mut VaultService) -> Uuid {
    let staged = vault
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .expect("missing, so staged");
    assert_eq!(staged.base_revision, None);
    let id = staged.item_id;
    vault.commit_write(staged, 1).unwrap();
    id
}

#[test]
fn the_identity_id_is_the_same_on_every_device_of_an_account() {
    let (a, sk) = activated_vault();
    let b = second_device(&a, &sk);
    assert_eq!(a.identity_item_id().unwrap(), b.identity_item_id().unwrap());

    let (other, _) = activated_vault();
    assert_ne!(
        a.identity_item_id().unwrap(),
        other.identity_item_id().unwrap(),
        "a different vault key gives a different id"
    );
}

#[test]
fn the_identity_id_needs_an_unlocked_vault() {
    let (mut vault, _) = activated_vault();
    vault.lock();
    assert!(matches!(vault.identity_item_id(), Err(Error::Locked)));
}

#[test]
fn the_identity_is_staged_once_with_the_email_prefilled() {
    let (mut vault, _) = activated_vault();
    let id = created(&mut vault);
    assert_eq!(id, vault.identity_item_id().unwrap());
    assert!(vault
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .is_none());

    let overview = vault.get_item(&id).unwrap();
    assert_eq!(overview.item_type, ItemType::Identity);
    assert_eq!(overview.title, "");
    assert_eq!(overview.username.as_deref(), Some("user@example.com"));
    let fields = vault.reveal_identity(&id).unwrap();
    assert_eq!(
        fields.email.as_ref().map(|e| e.expose()),
        Some("user@example.com")
    );
}

#[test]
fn an_update_round_trips_every_value_and_updates_the_overview() {
    let (mut vault, _) = activated_vault();
    let id = created(&mut vault);
    let fields = IdentityFields {
        first_name: some("Samuel"),
        last_name: some("Rocha"),
        birth_date: some("2000-04-20"),
        cpf: some("123.456.789-00"),
        email: some("samuel@example.com"),
        mobile_phone: some("+55 61 99999-0000"),
        street: some("Quadra 02 Conjunto 01"),
        postal_code: some("71266-105"),
        country: some("Brasil"),
        custom: vec![CustomField {
            label: "Blood type".into(),
            value: "O+".into(),
            hidden: true,
        }],
        notes: some("Remember me"),
        ..Default::default()
    };
    let staged = vault
        .stage_update(&id, identity_input(fields), NOW + 1)
        .unwrap();
    vault.commit_write(staged, 2).unwrap();

    let overview = vault.get_item(&id).unwrap();
    assert_eq!(overview.title, "Samuel Rocha");
    assert_eq!(overview.username.as_deref(), Some("samuel@example.com"));
    assert!(overview.has_notes);
    assert!(!overview.has_password && !overview.has_totp && overview.urls.is_empty());

    let back = vault.reveal_identity(&id).unwrap();
    assert_eq!(back.cpf.as_ref().unwrap().expose(), "123.456.789-00");
    assert_eq!(back.custom.len(), 1);
    assert!(back.custom[0].hidden);
    assert_eq!(
        vault
            .identity_value(&id, IdentityField::MobilePhone)
            .unwrap()
            .expose(),
        "+55 61 99999-0000"
    );
    assert!(vault
        .identity_value(&id, IdentityField::Address)
        .unwrap()
        .expose()
        .contains("CEP 71266-105"));
    assert_eq!(vault.identity_custom_value(&id, 0).unwrap().expose(), "O+");
    assert!(matches!(
        vault.identity_custom_value(&id, 1),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        vault.identity_value(&id, IdentityField::Rg),
        Err(Error::NotFound)
    ));

    // Search finds it by name and by email, like any item.
    assert_eq!(vault.search("rocha").unwrap().len(), 1);
    assert_eq!(vault.search("samuel@").unwrap().len(), 1);
}

#[test]
fn invalid_values_are_refused() {
    let (mut vault, _) = activated_vault();
    let id = created(&mut vault);
    let bad = IdentityFields {
        birth_date: some("2000-02-30"),
        ..Default::default()
    };
    assert!(matches!(
        vault.stage_update(&id, identity_input(bad), NOW),
        Err(Error::InvalidInput(_))
    ));
    let mut with_password = identity_input(IdentityFields::default());
    with_password.password = SecretUpdate::Set(secret("pw"));
    assert!(matches!(
        vault.stage_update(&id, with_password, NOW),
        Err(Error::InvalidInput(_))
    ));
    let mut missing = identity_input(IdentityFields::default());
    missing.identity = None;
    assert!(matches!(
        vault.stage_update(&id, missing, NOW),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn a_login_cannot_carry_identity_values() {
    let (vault, _) = activated_vault();
    let mut input = login("GitHub", "me", "pw", "github.com");
    input.identity = Some(IdentityFields::default());
    assert!(matches!(
        vault.stage_create(input, NOW),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn an_identity_cannot_be_created_by_hand_or_deleted() {
    let (mut vault, _) = activated_vault();
    assert!(matches!(
        vault.stage_create(identity_input(IdentityFields::default()), NOW),
        Err(Error::Denied)
    ));
    let id = created(&mut vault);
    assert!(matches!(vault.stage_delete(&id), Err(Error::Denied)));
}

#[test]
fn only_identities_are_revealed_as_identities() {
    let (mut vault, _) = activated_vault();
    let staged = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let login_id = staged.item_id;
    vault.commit_write(staged, 1).unwrap();
    assert!(matches!(
        vault.reveal_identity(&login_id),
        Err(Error::Denied)
    ));
    assert!(matches!(
        vault.identity_value(&login_id, IdentityField::Email),
        Err(Error::Denied)
    ));
}

// ----------------------------------------------------------------- security

/// Attack: a page (or a compromised extension) asks for the identity. Every
/// page-bound path refuses it, on any origin, whatever its email looks like.
#[test]
fn the_extension_can_never_read_the_identity() {
    let (mut vault, _) = activated_vault();
    let id = created(&mut vault);
    let fields = IdentityFields {
        email: some("me@github.com"),
        website: some("https://github.com"),
        ..Default::default()
    };
    let staged = vault
        .stage_update(&id, identity_input(fields), NOW)
        .unwrap();
    vault.commit_write(staged, 2).unwrap();

    for page in ["https://github.com/login", "https://example.com/"] {
        assert!(vault.find_matches(page, None).unwrap().is_empty());
        assert!(matches!(
            vault.fill_for_page(&id, page, None, NOW),
            Err(Error::Denied)
        ));
    }
    assert!(vault.totp_code(&id, 1_700_000_000).is_err());
}

#[test]
fn identity_values_never_appear_in_debug_output() {
    let (mut vault, _) = activated_vault();
    let id = created(&mut vault);
    let fields = vault.reveal_identity(&id).unwrap();
    let overview = vault.get_item(&id).unwrap();
    let debug = format!("{fields:?} {overview:?}");
    assert!(!debug.contains("user@example.com"));
}
