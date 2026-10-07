//! Item tags through the vault: stored sealed in the overview, kept by
//! saves that do not send them, searched.

mod common;

use common::{activated_vault, login, NOW};
use havenkeys_core::model::{ItemOverview, ItemType};

fn tagged(tags: &[&str]) -> havenkeys_core::model::ItemInput {
    let mut input = login("GitHub", "me", "pw", "github.com");
    input.tags = Some(tags.iter().map(|s| s.to_string()).collect());
    input
}

#[test]
fn tags_are_normalised_and_stored() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault.stage_create(tagged(&[" Work ", "staging", "WORK"]), NOW).unwrap();
    let ov = vault.commit_write(staged, 1).unwrap().unwrap();
    assert_eq!(ov.tags, vec!["staging", "work"]);
    assert_eq!(vault.get_item(&ov.id).unwrap().tags, vec!["staging", "work"]);
}

#[test]
fn an_invalid_tag_refuses_the_save() {
    let (vault, _sk) = activated_vault();
    assert!(vault.stage_create(tagged(&["a,b"]), NOW).is_err());
}

#[test]
fn an_update_without_tags_keeps_them() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault.stage_create(tagged(&["prod"]), NOW).unwrap();
    let id = staged.item_id;
    vault.commit_write(staged, 1).unwrap();

    // e.g. the extension updating the password: it sends no tags.
    let staged = vault.stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW + 1).unwrap();
    vault.commit_write(staged, 2).unwrap();
    assert_eq!(vault.get_item(&id).unwrap().tags, vec!["prod"]);

    // An explicit empty list clears them.
    let mut clear = login("GitHub", "me", "pw2", "github.com");
    clear.tags = Some(Vec::new());
    let staged = vault.stage_update(&id, clear, NOW + 2).unwrap();
    vault.commit_write(staged, 3).unwrap();
    assert!(vault.get_item(&id).unwrap().tags.is_empty());
}

#[test]
fn search_matches_tags() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault.stage_create(tagged(&["staging"]), NOW).unwrap();
    vault.commit_write(staged, 1).unwrap();
    let staged = vault.stage_create(login("GitLab", "me", "pw", "gitlab.com"), NOW).unwrap();
    vault.commit_write(staged, 2).unwrap();

    let found = vault.search("stag").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "GitHub");
}

#[test]
fn an_overview_without_tags_reads_as_empty() {
    let json = r#"{"id":"00000000-0000-0000-0000-000000000001","itemType":"login","title":"t",
        "hasPassword":false,"hasTotp":false,"hasNotes":false,"createdAt":0,"updatedAt":0}"#;
    let ov: ItemOverview = serde_json::from_str(json).unwrap();
    assert!(ov.tags.is_empty());
    assert_eq!(ov.item_type, ItemType::Login);
}

#[test]
fn unknown_overview_fields_survive_a_round_trip() {
    let json = r#"{"id":"00000000-0000-0000-0000-000000000001","itemType":"login","title":"t",
        "hasPassword":false,"hasTotp":false,"hasNotes":false,"createdAt":0,"updatedAt":0,
        "fromTheFuture":{"x":1}}"#;
    let ov: ItemOverview = serde_json::from_str(json).unwrap();
    let back = serde_json::to_value(&ov).unwrap();
    assert_eq!(back["fromTheFuture"]["x"], 1);
}

#[test]
fn an_update_keeps_unknown_overview_fields() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault.stage_create(login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let id = staged.item_id;
    vault.commit_write(staged, 1).unwrap();
    vault.insert_extra_for_tests(&id, "fromTheFuture", serde_json::json!(true));

    let staged = vault.stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW + 1).unwrap();
    vault.commit_write(staged, 2).unwrap();
    assert_eq!(vault.get_item(&id).unwrap().extra["fromTheFuture"], true);
}
