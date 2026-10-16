//! Imported 1Password logins pass the vault's own checks, whatever their
//! custom fields hold (spec 2026-09-30-login-custom-fields §6).

mod common;

use common::{activated_vault, NOW};
use havenkeys_core::custom_field::FieldKind;
use havenkeys_core::model::SecretUpdate;
use serde_json::{json, Value};
use std::io::{Cursor, Write};

fn pux(data: &Value) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("export.data", opts).unwrap();
        zip.write_all(data.to_string().as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buf.into_inner()
}

fn login(form: Value, fields: Value) -> Value {
    json!({"accounts": [{"vaults": [{"items": [{
        "uuid": "a", "categoryUuid": "001", "state": "active",
        "overview": {"title": "Bank"},
        "details": {
            "loginFields": form,
            "sections": [{"title": "More", "fields": fields}]
        }
    }]}]}]})
}

fn notes_of(input: &havenkeys_core::model::ItemInput) -> String {
    match &input.notes {
        SecretUpdate::Set(n) => n.expose().to_owned(),
        _ => String::new(),
    }
}

#[test]
fn oversized_form_fields_keep_the_item_and_their_value() {
    let form = json!([
        {"value": "me", "name": "u", "fieldType": "T", "designation": "username"},
        {"value": "pw", "name": "p", "fieldType": "P", "designation": "password"},
        {"value": "x".repeat(5000), "name": "bigpin", "fieldType": "P"}
    ]);
    let parsed = havenkeys_core::import::onepux::parse(&pux(&login(form, json!([])))).unwrap();
    let input = parsed.items.into_iter().next().unwrap().input;
    let sections = input.sections.as_ref().unwrap();
    assert_eq!(
        sections[0].fields[0].value.kind(),
        FieldKind::Text,
        "too long for a password"
    );
    let (vault, _) = activated_vault();
    vault.stage_create(input, NOW).expect("the login stores");
}

#[test]
fn an_oversized_text_form_field_goes_to_the_notes_counted() {
    let form = json!([
        {"value": "me", "name": "u", "fieldType": "T", "designation": "username"},
        {"value": "pw", "name": "p", "fieldType": "P", "designation": "password"},
        {"value": "y".repeat(70_000), "name": "bigtext", "fieldType": "T"}
    ]);
    let parsed = havenkeys_core::import::onepux::parse(&pux(&login(form, json!([])))).unwrap();
    assert_eq!(parsed.report.fields_to_notes, 1);
    let notes = notes_of(&parsed.items[0].input);
    assert!(notes.contains("bigtext: yyyy"));
    assert!(parsed.items[0].input.sections.is_none());
}

#[test]
fn large_valid_fields_stop_at_the_total_and_the_login_stores() {
    let fields: Vec<Value> = (0..6)
        .map(|i| json!({"title": format!("f{i}"), "value": {"string": format!("{i}").repeat(60_000)}}))
        .collect();
    let form = json!([
        {"value": "me", "name": "u", "fieldType": "T", "designation": "username"},
        {"value": "pw", "name": "p", "fieldType": "P", "designation": "password"}
    ]);
    let parsed = havenkeys_core::import::onepux::parse(&pux(&login(form, json!(fields)))).unwrap();
    assert!(parsed.report.fields_to_notes > 0);
    let item = parsed.items.into_iter().next().unwrap();
    let (vault, _) = activated_vault();
    // Overflow went to the notes: the notes cap, not the fields total, is what
    // may remain; custom fields alone must be accepted.
    let mut input = item.input;
    input.notes = SecretUpdate::Keep;
    vault
        .stage_create(input, NOW)
        .expect("imported fields pass the vault");
}

#[test]
fn a_bitwarden_note_whose_field_is_too_long_keeps_it_in_the_text_and_stores() {
    let data = json!({"items": [
        {"id": "1", "type": 2, "name": "Big", "notes": "body", "secureNote": {"type": 0},
         "fields": [
            {"name": "Short", "value": "ok", "type": 0},
            {"name": "Huge", "value": "z".repeat(300_000), "type": 0}
         ]}
    ]});
    let parsed = havenkeys_core::import::bitwarden::parse(data.to_string().as_bytes()).unwrap();
    let input = parsed.items.into_iter().next().unwrap().input;
    let text = match &input.content {
        SecretUpdate::Set(c) => c.expose().to_owned(),
        _ => String::new(),
    };
    assert!(text.starts_with("body"));
    assert!(text.contains("Huge: zzzz"));
    assert_eq!(input.sections.as_ref().unwrap()[0].fields.len(), 1);
    let (vault, _) = activated_vault();
    vault.stage_create(input, NOW).expect("the note stores");
}

fn one_item(category: &str, title: &str, overview_extra: Value, details: Value) -> Value {
    let mut overview = json!({"title": title});
    if let (Some(o), Value::Object(extra)) = (overview.as_object_mut(), overview_extra) {
        o.extend(extra);
    }
    json!({"accounts": [{"vaults": [{"items": [{
        "uuid": "a", "categoryUuid": category, "state": "active",
        "overview": overview, "details": details
    }]}]}]})
}

fn content_of(input: &havenkeys_core::model::ItemInput) -> String {
    match &input.content {
        SecretUpdate::Set(n) => n.expose().to_owned(),
        _ => String::new(),
    }
}

#[test]
fn a_1password_secure_note_keeps_its_sections() {
    let data = one_item(
        "003",
        "Router",
        json!({}),
        json!({
            "notesPlain": "in the closet",
            "sections": [{"title": "Wi-Fi", "fields": [
                {"title": "Network", "value": {"string": "home"}},
                {"title": "Key", "value": {"concealed": "hunter22"}},
                {"title": "Admin", "value": {"url": "http://192.168.0.1"}}
            ]}]
        }),
    );
    let parsed = havenkeys_core::import::onepux::parse(&pux(&data)).unwrap();
    let input = parsed.items.into_iter().next().unwrap().input;
    assert_eq!(content_of(&input), "in the closet");
    let s = &input.sections.as_ref().unwrap()[0];
    assert_eq!(s.title.as_ref().unwrap().expose(), "Wi-Fi");
    let kinds: Vec<(&str, FieldKind)> = s
        .fields
        .iter()
        .map(|f| (f.label.expose(), f.value.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("Network", FieldKind::Text),
            ("Key", FieldKind::Password),
            ("Admin", FieldKind::Url)
        ]
    );
    let (vault, _) = activated_vault();
    vault.stage_create(input, NOW).expect("the note stores");
}

#[test]
fn a_1password_server_becomes_a_note_with_typed_fields() {
    let data = one_item(
        "110",
        "Prod",
        json!({"url": "https://admin.example"}),
        json!({
            "notesPlain": "ask ops",
            "sections": [{"title": "Admin console", "fields": [
                {"title": "username", "value": {"string": "root"}},
                {"title": "password", "value": {"concealed": "s3cret!"}},
                {"title": "one-time password", "value": {"totp": "JBSWY3DPEHPK3PXP"}}
            ]}]
        }),
    );
    let parsed = havenkeys_core::import::onepux::parse(&pux(&data)).unwrap();
    assert_eq!(parsed.report.converted_to_notes, 1);
    let input = parsed.items.into_iter().next().unwrap().input;
    let content = content_of(&input);
    assert!(content.starts_with("Imported from 1Password (Server)"));
    assert!(content.ends_with("ask ops"));
    assert!(
        !content.contains("s3cret!"),
        "the password is a field, not text"
    );
    let sections = input.sections.as_ref().unwrap();
    assert!(sections[0].title.is_none());
    assert_eq!(sections[0].fields[0].value.kind(), FieldKind::Url);
    assert_eq!(
        sections[1].title.as_ref().unwrap().expose(),
        "Admin console"
    );
    let kinds: Vec<FieldKind> = sections[1].fields.iter().map(|f| f.value.kind()).collect();
    assert_eq!(
        kinds,
        [FieldKind::Text, FieldKind::Password, FieldKind::Otp]
    );
    let (vault, _) = activated_vault();
    vault.stage_create(input, NOW).expect("the note stores");
}

#[test]
fn a_converted_item_too_big_for_fields_keeps_the_rest_in_the_text() {
    let fields: Vec<Value> = (0..6)
        .map(|i| json!({"title": format!("f{i}"), "value": {"string": format!("{i}").repeat(60_000)}}))
        .collect();
    let data = one_item(
        "112",
        "API",
        json!({}),
        json!({"sections": [{"title": "Keys", "fields": fields}]}),
    );
    let parsed = havenkeys_core::import::onepux::parse(&pux(&data)).unwrap();
    assert!(parsed.report.fields_to_notes > 0);
    let input = parsed.items.into_iter().next().unwrap().input;
    assert!(content_of(&input).contains("f5: 5555"));
    let (vault, _) = activated_vault();
    vault.stage_create(input, NOW).expect("the note stores");
}
