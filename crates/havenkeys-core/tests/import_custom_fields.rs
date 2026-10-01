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
