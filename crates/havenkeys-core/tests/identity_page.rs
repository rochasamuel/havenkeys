//! Identity access for web pages (spec 2026-09-29-identity-autofill §6.2).

mod common;

use common::{activated_vault, secret, NOW};
use havenkeys_core::identity::{FillRole, IdentityFields};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;

fn some(v: &str) -> Option<havenkeys_core::SecretString> {
    Some(secret(v))
}

/// A vault whose identity holds a name, a phone, an address and a CPF.
fn with_identity() -> VaultService {
    let (mut v, _) = activated_vault();
    let staged = v
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 1).unwrap();
    let fields = IdentityFields {
        first_name: some("Samuel"),
        last_name: some("Rocha"),
        email: some("user@example.com"),
        mobile_phone: some("+55 61 99999-0000"),
        postal_code: some("71266-105"),
        cpf: some("123.456.789-00"),
        ..Default::default()
    };
    let input = ItemInput {
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
        sections: None,
    };
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    v
}

const SHOP: &str = "https://shop.example.com/checkout";

fn values(
    v: &VaultService,
    url: &str,
    top: Option<&str>,
    roles: &[FillRole],
    docs: bool,
) -> Vec<(FillRole, String)> {
    v.identity_values_for_page(url, top, roles, docs)
        .unwrap()
        .into_iter()
        .map(|(r, s)| (r, s.expose().to_owned()))
        .collect()
}

#[test]
fn the_summary_names_the_roles_with_a_value_and_no_values() {
    let v = with_identity();
    let s = v.identity_summary_for_page(SHOP, None).unwrap();
    assert_eq!(s.title, "Samuel Rocha");
    assert_eq!(s.email.as_deref(), Some("user@example.com"));
    for r in [
        FillRole::FullName,
        FillRole::Phone,
        FillRole::PostalCode,
        FillRole::Cpf,
    ] {
        assert!(s.roles.contains(&r), "{r:?}");
    }
    assert!(!s.roles.contains(&FillRole::City));
    assert!(!format!("{s:?}").contains("Samuel"));
}

#[test]
fn a_long_accented_name_fits_the_summary_title_limit() {
    use havenkeys_core::identity::MAX_NAME_CHARS;
    use havenkeys_core::identity_page::MAX_SUMMARY_TITLE_BYTES;
    let v = with_identity();
    let id = v.identity_item_id().unwrap();
    let long = "é".repeat(MAX_NAME_CHARS);
    let fields = IdentityFields {
        first_name: some(&long),
        middle_name: some(&long),
        last_name: some(&long),
        ..Default::default()
    };
    let input = ItemInput {
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
        sections: None,
    };
    let staged = v.stage_update(&id, input, NOW + 2).unwrap();
    let mut v = v;
    v.commit_write(staged, 3).unwrap();
    let s = v.identity_summary_for_page(SHOP, None).unwrap();
    assert!(
        s.title.len() <= MAX_SUMMARY_TITLE_BYTES,
        "{}",
        s.title.len()
    );
    assert!(s.title.len() > MAX_SUMMARY_TITLE_BYTES - 4);
    assert!(s.title.chars().all(|c| c == 'é' || c == ' '));
}

#[test]
fn only_the_requested_roles_come_back() {
    let v = with_identity();
    let got = values(&v, SHOP, None, &[FillRole::FullName, FillRole::City], false);
    assert_eq!(got, vec![(FillRole::FullName, "Samuel Rocha".to_owned())]);
}

#[test]
fn documents_need_the_flag_and_https() {
    let v = with_identity();
    assert!(values(&v, SHOP, None, &[FillRole::Cpf], false).is_empty());
    assert_eq!(
        values(&v, SHOP, None, &[FillRole::Cpf], true),
        vec![(FillRole::Cpf, "123.456.789-00".to_owned())]
    );
    assert!(
        values(&v, "http://shop.example.com/", None, &[FillRole::Cpf], true).is_empty(),
        "never on http"
    );
    assert_eq!(
        values(
            &v,
            "http://shop.example.com/",
            None,
            &[FillRole::PostalCode],
            true
        )
        .len(),
        1,
        "other values still fill on http"
    );
}

#[test]
fn frames_must_be_same_site_as_the_top_page() {
    let v = with_identity();
    let same = values(
        &v,
        "https://pay.example.com/f",
        Some(SHOP),
        &[FillRole::FullName],
        false,
    );
    assert_eq!(same.len(), 1);
    assert!(matches!(
        v.identity_values_for_page(
            SHOP,
            Some("https://evil.com/"),
            &[FillRole::FullName],
            false
        ),
        Err(Error::Denied)
    ));
    assert!(matches!(
        v.identity_summary_for_page(SHOP, Some("https://evil.com/")),
        Err(Error::Denied)
    ));
    assert!(matches!(
        v.identity_id_for_page(SHOP, Some("not a url")),
        Err(Error::Denied)
    ));
}

#[test]
fn non_web_pages_and_a_locked_vault_are_refused() {
    let mut v = with_identity();
    for bad in [
        "file:///etc/passwd",
        "chrome://settings",
        "javascript:alert(1)",
        "",
    ] {
        assert!(
            matches!(v.identity_summary_for_page(bad, None), Err(Error::Denied)),
            "{bad}"
        );
    }
    v.lock();
    assert!(matches!(
        v.identity_summary_for_page(SHOP, None),
        Err(Error::Locked)
    ));
}

#[test]
fn a_vault_without_its_identity_says_not_found() {
    let (v, _) = activated_vault();
    assert!(matches!(
        v.identity_summary_for_page(SHOP, None),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        v.identity_id_for_page(SHOP, None),
        Err(Error::NotFound)
    ));
}

#[test]
fn the_id_for_page_is_the_identity() {
    let v = with_identity();
    assert_eq!(
        v.identity_id_for_page(SHOP, None).unwrap(),
        v.identity_item_id().unwrap()
    );
}

#[test]
fn an_app_gets_its_roles_and_documents_only_when_confirmed() {
    let v = with_identity();
    let s = v.identity_summary_for_app().unwrap();
    assert_eq!(s.title, "Samuel Rocha");
    assert!(s.roles.contains(&FillRole::Cpf));
    let plain: Vec<(FillRole, String)> = v
        .identity_values_for_app(&[FillRole::FirstName, FillRole::Cpf], false)
        .unwrap()
        .into_iter()
        .map(|(r, s)| (r, s.expose().to_owned()))
        .collect();
    assert_eq!(plain, vec![(FillRole::FirstName, "Samuel".to_owned())]);
    let with_docs: Vec<FillRole> = v
        .identity_values_for_app(&[FillRole::FirstName, FillRole::Cpf], true)
        .unwrap()
        .into_iter()
        .map(|(r, _)| r)
        .collect();
    assert_eq!(with_docs, vec![FillRole::FirstName, FillRole::Cpf]);
}

#[test]
fn a_vault_without_an_identity_gives_an_app_nothing() {
    let (v, _) = activated_vault();
    assert!(matches!(v.identity_summary_for_app(), Err(Error::NotFound)));
}

#[test]
fn a_locked_vault_gives_an_app_no_identity() {
    let mut v = with_identity();
    v.lock();
    assert!(matches!(v.identity_summary_for_app(), Err(Error::Locked)));
    assert!(matches!(
        v.identity_values_for_app(&[FillRole::FirstName], true),
        Err(Error::Locked)
    ));
}
