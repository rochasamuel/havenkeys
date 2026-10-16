//! A login's or secure note's custom fields in the vault (specs
//! 2026-09-30-login-custom-fields, 2026-10-16-secure-note-fields).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::card::CardInput;
use havenkeys_core::custom_field::{AddressPart, FieldInput, FieldValueInput, SectionInput};
use havenkeys_core::import::{ImportReport, ImportedItem};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
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
    let s = v.item_sections(id).unwrap();
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
        v.item_field(&id, &pin)
            .unwrap()
            .concealed()
            .unwrap()
            .expose(),
        "4321"
    );
    assert_eq!(
        v.item_field(&id, &token)
            .unwrap()
            .totp_code(59)
            .unwrap()
            .code
            .expose(),
        "287082"
    );
    assert_eq!(
        v.item_field(&id, &site).unwrap().url().unwrap(),
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
    assert_eq!(v.item_sections(&id).unwrap()[0].fields.len(), 3);
}

#[test]
fn an_empty_layout_removes_them() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let mut input = github();
    input.sections = Some(Vec::new());
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert!(v.item_sections(&id).unwrap().is_empty());
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
        v.item_field(&id, &pin)
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
            v.item_field(&id, &f).unwrap().copy_value(None, 59).err(),
            Some(Error::NotFound)
        );
    }
    assert_eq!(
        v.item_field(&id, &pin).unwrap().concealed().err(),
        Some(Error::NotFound)
    );
}

fn note_with_fields(content: &str) -> ItemInput {
    let mut input = note("Server", content);
    input.sections = Some(bank_sections());
    input
}

fn card() -> ItemInput {
    ItemInput {
        tags: None,
        item_type: ItemType::Card,
        title: "Visa".into(),
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
            cardholder_name: None,
            brand: None,
            number: SecretUpdate::Set(secret("5200 8282 8282 8210")),
            verification_number: SecretUpdate::Keep,
            expiry: None,
            notes: None,
        }),
        sections: None,
    }
}

#[test]
fn a_note_holds_custom_fields() {
    let (mut v, _) = activated_vault();
    let staged = v.stage_create(note_with_fields("root box"), NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    let (_, pin, token, site) = field_ids(&v, &id);
    assert_eq!(
        v.item_field(&id, &pin)
            .unwrap()
            .concealed()
            .unwrap()
            .expose(),
        "4321"
    );
    assert_eq!(
        v.item_field(&id, &token)
            .unwrap()
            .totp_code(59)
            .unwrap()
            .code
            .expose()
            .len(),
        6
    );
    assert_eq!(
        v.item_field(&id, &site).unwrap().url().unwrap(),
        "https://bank.example/"
    );
    assert_eq!(
        v.reveal(&id, havenkeys_core::model::SecretField::Content)
            .unwrap()
            .expose(),
        "root box"
    );
}

#[test]
fn a_note_edit_keeps_or_clears_its_concealed_fields() {
    let (mut v, _) = activated_vault();
    let staged = v.stage_create(note_with_fields("x"), NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    let (section, pin, token, site) = field_ids(&v, &id);

    // `sections: None` keeps every field.
    let mut text_only = note("Server", "changed");
    text_only.sections = None;
    let staged = v.stage_update(&id, text_only, NOW).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert_eq!(v.item_sections(&id).unwrap()[0].fields.len(), 3);

    // Keep the PIN, clear the token, keep the site.
    let mut edit = note("Server", "changed");
    edit.sections = Some(vec![SectionInput {
        id: Some(section),
        title: Some(secret("Bank")),
        fields: vec![
            field(
                Some(pin),
                "PIN",
                FieldValueInput::Password(SecretUpdate::Keep),
            ),
            field(
                Some(token),
                "Token",
                FieldValueInput::Otp(SecretUpdate::Clear),
            ),
            field(
                Some(site),
                "Site",
                FieldValueInput::Url(secret("bank.example")),
            ),
        ],
    }]);
    let staged = v.stage_update(&id, edit, NOW).unwrap();
    v.commit_write(staged, 3).unwrap();
    assert_eq!(
        v.item_field(&id, &pin)
            .unwrap()
            .concealed()
            .unwrap()
            .expose(),
        "4321"
    );
    assert!(v.item_field(&id, &token).unwrap().totp_code(59).is_err());
}

#[test]
fn a_note_may_have_fields_and_no_text() {
    let (mut v, _) = activated_vault();
    let mut input = note_with_fields("");
    input.content = SecretUpdate::Keep;
    let staged = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    assert_eq!(v.item_sections(&id).unwrap()[0].fields.len(), 3);
}

#[test]
fn note_fields_follow_the_login_limits() {
    let (v, _) = activated_vault();
    let mut input = note("n", "c");
    input.sections = Some(
        (0..21)
            .map(|_| SectionInput {
                id: None,
                title: None,
                fields: vec![],
            })
            .collect(),
    );
    assert_eq!(
        v.stage_create(input, NOW).err(),
        Some(Error::InvalidInput("too many sections"))
    );
}

#[test]
fn lookups_outside_a_login_or_note_are_not_found_and_locked_is_first() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v.stage_create(card(), NOW).unwrap();
    let c = v.commit_write(staged, 2).unwrap().unwrap().id;
    assert_eq!(v.item_sections(&c).err(), Some(Error::NotFound));
    assert_eq!(
        v.item_field(&id, &Uuid::new_v4()).err(),
        Some(Error::NotFound)
    );
    assert_eq!(
        v.item_field(&Uuid::new_v4(), &Uuid::new_v4()).err(),
        Some(Error::NotFound)
    );
    let (_, pin, ..) = field_ids(&v, &id);
    v.lock();
    assert_eq!(v.item_field(&id, &pin).err(), Some(Error::Locked));
    assert_eq!(v.item_sections(&Uuid::new_v4()).err(), Some(Error::Locked));
}

#[test]
fn cards_have_no_custom_fields() {
    let (v, _) = activated_vault();
    let mut input = card();
    input.sections = Some(Vec::new());
    assert_eq!(
        v.stage_create(input, NOW).err(),
        Some(Error::InvalidInput(
            "only a login or a secure note has custom fields"
        ))
    );
}

#[test]
fn an_import_does_not_merge_notes_whose_fields_differ() {
    let (mut v, _) = activated_vault();
    let staged = v.stage_create(note_with_fields("same"), NOW).unwrap();
    v.commit_write(staged, 1).unwrap();

    let mut other = note("Server", "same");
    other.sections = Some(vec![SectionInput {
        id: None,
        title: None,
        fields: vec![field(
            None,
            "PIN",
            FieldValueInput::Password(SecretUpdate::Set(secret("9999"))),
        )],
    }]);
    let same = note_with_fields("same");
    let plain = note("Server", "same");
    let items = [other, same, plain]
        .into_iter()
        .map(|input| ImportedItem {
            input,
            created_at: None,
            updated_at: None,
        })
        .collect();
    let staged = v.stage_import(items, ImportReport::default(), NOW).unwrap();
    // `other` and `plain` are new; `same` is already there.
    assert_eq!(staged.writes.len(), 2);
    assert_eq!(staged.report.skipped_duplicates, 1);
}

#[test]
fn a_note_saved_before_fields_loads_without_them() {
    // The serialized shape of a note from before this change.
    let old: havenkeys_core::model::ItemDetails =
        serde_json::from_str(r#"{"type":"secure_note","content":"hi"}"#).unwrap();
    let havenkeys_core::model::ItemDetails::SecureNote { sections, .. } = &old else {
        panic!("not a note")
    };
    assert!(sections.is_empty());
    assert_eq!(
        serde_json::to_string(&old).unwrap(),
        r#"{"type":"secure_note","content":"hi"}"#
    );
}

#[test]
fn page_bound_answers_hold_no_custom_field() {
    let (mut v, _) = activated_vault();
    create_with_fields(&mut v);
    let matches = v.find_matches("https://github.com/", None, NOW).unwrap();
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
    let f = v.item_sections(&id).unwrap()[0].fields[0].id;
    let field = v.item_field(&id, &f).unwrap();
    assert_eq!(
        field
            .copy_value(Some(AddressPart::City), 0)
            .unwrap()
            .expose(),
        "Recife"
    );
}
