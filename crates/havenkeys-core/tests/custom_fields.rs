//! A login's custom fields in the vault (spec 2026-09-30-login-custom-fields).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::custom_field::{AddressPart, FieldInput, FieldValueInput, SectionInput};
use havenkeys_core::model::{ItemInput, SecretUpdate};
use havenkeys_core::vault::{SaveTarget, VaultService};
use havenkeys_core::Error;
use uuid::Uuid;

const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

fn field(id: Option<Uuid>, label: &str, value: FieldValueInput) -> FieldInput {
    FieldInput {
        id,
        label: secret(label),
        value,
    }
}

fn bank_sections() -> Vec<SectionInput> {
    vec![SectionInput {
        id: None,
        title: Some(secret("Bank")),
        fields: vec![
            field(
                None,
                "PIN",
                FieldValueInput::Password(SecretUpdate::Set(secret("4321"))),
            ),
            field(
                None,
                "Token",
                FieldValueInput::Otp(SecretUpdate::Set(secret(RFC_SECRET))),
            ),
            field(None, "Site", FieldValueInput::Url(secret("bank.example"))),
        ],
    }]
}

fn github() -> ItemInput {
    login("GitHub", "octo", "pw", "github.com")
}

fn create_with_fields(v: &mut VaultService) -> Uuid {
    let mut input = github();
    input.sections = Some(bank_sections());
    let staged = v.stage_create(input, NOW).unwrap();
    v.commit_write(staged, 1).unwrap().unwrap().id
}

/// (section id, PIN id, Token id, Site id)
fn field_ids(v: &VaultService, id: &Uuid) -> (Uuid, Uuid, Uuid, Uuid) {
    let s = v.login_sections(id).unwrap();
    (
        s[0].id,
        s[0].fields[0].id,
        s[0].fields[1].id,
        s[0].fields[2].id,
    )
}

#[test]
fn fields_round_trip_through_the_encrypted_details() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let (_, pin, token, site) = field_ids(&v, &id);
    assert_eq!(
        v.login_field(&id, &pin)
            .unwrap()
            .concealed()
            .unwrap()
            .expose(),
        "4321"
    );
    assert_eq!(
        v.login_field(&id, &token)
            .unwrap()
            .totp_code(59)
            .unwrap()
            .code
            .expose(),
        "287082"
    );
    assert_eq!(
        v.login_field(&id, &site).unwrap().url().unwrap(),
        "https://bank.example/"
    );
    // Nothing in the overview.
    let ov = serde_json::to_string(&v.get_item(&id).unwrap()).unwrap();
    assert!(!ov.contains("Bank") && !ov.contains("PIN"));
}

#[test]
fn none_keeps_the_fields() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v.stage_update(&id, github(), NOW + 1).unwrap(); // sections: None
    v.commit_write(staged, 2).unwrap();
    assert_eq!(v.login_sections(&id).unwrap()[0].fields.len(), 3);
}

#[test]
fn an_empty_layout_removes_them() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let mut input = github();
    input.sections = Some(Vec::new());
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert!(v.login_sections(&id).unwrap().is_empty());
}

#[test]
fn a_browser_password_update_keeps_the_fields() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v
        .stage_save_login(
            "https://github.com/login",
            None,
            Some("octo"),
            secret("new-pw"),
            SaveTarget::Update(&id),
            NOW + 1,
        )
        .unwrap();
    v.commit_write(staged.write, 2).unwrap();
    let (_, pin, ..) = field_ids(&v, &id);
    assert_eq!(
        v.login_field(&id, &pin)
            .unwrap()
            .concealed()
            .unwrap()
            .expose(),
        "4321"
    );
}

#[test]
fn a_keep_cannot_reach_another_logins_field() {
    let (mut v, _) = activated_vault();
    let a = create_with_fields(&mut v);
    let b = create_with_fields(&mut v);
    let (_, a_pin, ..) = field_ids(&v, &a);
    // Save login B naming login A's PIN as one of B's fields.
    let mut input = github();
    input.sections = Some(vec![SectionInput {
        id: None,
        title: None,
        fields: vec![field(
            Some(a_pin),
            "PIN",
            FieldValueInput::Password(SecretUpdate::Keep),
        )],
    }]);
    assert!(matches!(
        v.stage_update(&b, input, NOW + 1),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn reveal_and_copy_of_an_empty_field_are_not_found() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let (sid, pin, token, site) = field_ids(&v, &id);
    let mut input = github();
    input.sections = Some(vec![SectionInput {
        id: Some(sid),
        title: Some(secret("Bank")),
        fields: vec![
            field(
                Some(pin),
                "PIN",
                FieldValueInput::Password(SecretUpdate::Clear),
            ),
            field(
                Some(token),
                "Token",
                FieldValueInput::Otp(SecretUpdate::Clear),
            ),
            field(Some(site), "Site", FieldValueInput::Url(secret(""))),
        ],
    }]);
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    for f in [pin, token, site] {
        assert_eq!(
            v.login_field(&id, &f).unwrap().copy_value(None, 59).err(),
            Some(Error::NotFound)
        );
    }
    assert_eq!(
        v.login_field(&id, &pin).unwrap().concealed().err(),
        Some(Error::NotFound)
    );
}

#[test]
fn lookups_outside_a_login_are_not_found_and_locked_is_first() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v.stage_create(note("n", "c"), NOW).unwrap();
    let n = v.commit_write(staged, 2).unwrap().unwrap().id;
    assert_eq!(v.login_sections(&n).err(), Some(Error::NotFound));
    assert_eq!(
        v.login_field(&id, &Uuid::new_v4()).err(),
        Some(Error::NotFound)
    );
    assert_eq!(
        v.login_field(&Uuid::new_v4(), &Uuid::new_v4()).err(),
        Some(Error::NotFound)
    );
    let (_, pin, ..) = field_ids(&v, &id);
    v.lock();
    assert_eq!(v.login_field(&id, &pin).err(), Some(Error::Locked));
    assert_eq!(v.login_sections(&Uuid::new_v4()).err(), Some(Error::Locked));
}

#[test]
fn only_a_login_has_custom_fields() {
    let (v, _) = activated_vault();
    let mut input = note("n", "c");
    input.sections = Some(Vec::new());
    assert_eq!(
        v.stage_create(input, NOW).err(),
        Some(Error::InvalidInput("only a login has custom fields"))
    );
}

#[test]
fn page_bound_answers_hold_no_custom_field() {
    let (mut v, _) = activated_vault();
    create_with_fields(&mut v);
    let matches = v.find_matches("https://github.com/", None).unwrap();
    let json = format!("{matches:?}");
    assert!(!json.contains("4321") && !json.contains("Bank") && !json.contains("PIN"));
}

#[test]
fn address_part_copy() {
    let (mut v, _) = activated_vault();
    let mut input = github();
    input.sections = Some(vec![SectionInput {
        id: None,
        title: None,
        fields: vec![field(
            None,
            "Home",
            FieldValueInput::Address(Box::new(havenkeys_core::custom_field::AddressValue {
                city: Some(secret("Recife")),
                ..Default::default()
            })),
        )],
    }]);
    let staged = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    let f = v.login_sections(&id).unwrap()[0].fields[0].id;
    let field = v.login_field(&id, &f).unwrap();
    assert_eq!(
        field
            .copy_value(Some(AddressPart::City), 0)
            .unwrap()
            .expose(),
        "Recife"
    );
}
