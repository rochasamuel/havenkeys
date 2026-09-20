//! Staged writes: the server accepts them, the replica records them.

mod common;

use common::{account, activated_vault, login, secret, NOW, PASSWORD};
use havenkeys_core::model::SecretField;

#[test]
fn a_staged_create_is_not_visible_until_it_is_committed() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();

    // Nothing is stored yet: the server has not seen it.
    assert!(vault.list_items().unwrap().is_empty());
    assert_eq!(staged.base_revision, None);

    let overview = vault.commit_write(staged, 12).unwrap().unwrap();
    assert_eq!(vault.list_items().unwrap().len(), 1);
    assert_eq!(
        vault
            .reveal(&overview.id, SecretField::Password)
            .unwrap()
            .expose(),
        "pw"
    );
}

#[test]
fn an_update_stages_the_revision_it_last_saw() {
    let (mut vault, _sk) = activated_vault();
    let created = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let id = created.item_id;
    vault.commit_write(created, 12).unwrap();

    let staged = vault
        .stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW + 1)
        .unwrap();
    assert_eq!(staged.base_revision, Some(12));
}

#[test]
fn a_staged_delete_carries_no_blobs() {
    let (mut vault, _sk) = activated_vault();
    let created = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let id = created.item_id;
    vault.commit_write(created, 12).unwrap();

    let staged = vault.stage_delete(&id).unwrap();
    assert!(staged.overview.is_none() && staged.details.is_none());
    assert_eq!(staged.base_revision, Some(12));

    assert!(vault.commit_write(staged, 13).unwrap().is_none());
    assert!(vault.list_items().unwrap().is_empty());
}

#[test]
fn a_write_staged_before_a_lock_is_discarded() {
    let (mut vault, sk) = activated_vault();
    let staged = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    vault.lock();
    vault
        .unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();

    // The blobs were sealed under the previous session; the server may even
    // have accepted them, but this device must not record them blindly.
    assert!(vault.commit_write(staged, 12).is_err());
    assert!(vault.list_items().unwrap().is_empty());
}

#[test]
fn staging_a_write_while_locked_is_refused() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .is_err());
}

// ------------------------------------------------------------------- import

/// An import is a batch of ordinary staged writes: nothing is stored until
/// the server has accepted each one.
#[test]
fn a_staged_import_seals_new_items_and_skips_ones_already_here() {
    use havenkeys_core::import::{ImportReport, ImportedItem};

    let (mut vault, _sk) = activated_vault();
    let existing = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    vault.commit_write(existing, 1).unwrap();

    let staged = vault
        .stage_import(
            vec![
                // The same login as above: already here.
                ImportedItem {
                    input: login("GitHub", "me", "other-pw", "github.com"),
                    created_at: None,
                    updated_at: None,
                },
                ImportedItem {
                    input: login("Fastmail", "me", "pw2", "fastmail.com"),
                    created_at: Some(1_600_000_000_000),
                    updated_at: Some(1_600_000_500_000),
                },
                ImportedItem {
                    input: common::note("Recovery codes", "1234 5678"),
                    created_at: None,
                    updated_at: None,
                },
            ],
            ImportReport::default(),
            NOW,
        )
        .unwrap();

    assert_eq!(staged.report.skipped_duplicates, 1);
    assert_eq!(staged.report.logins, 1);
    assert_eq!(staged.report.secure_notes, 1);
    assert_eq!(staged.report.imported, 2);
    assert_eq!(staged.writes.len(), 2);
    // Still only the original item: an import that is never sent stores
    // nothing.
    assert_eq!(vault.list_items().unwrap().len(), 1);

    let mut revision = 1;
    for write in staged.writes {
        revision += 1;
        vault.commit_write(write, revision).unwrap();
    }
    let titles: Vec<String> = vault
        .list_items()
        .unwrap()
        .iter()
        .map(|i| i.title.clone())
        .collect();
    assert_eq!(titles.len(), 3);
    assert!(titles.iter().any(|t| t == "Fastmail"));
    assert!(titles.iter().any(|t| t == "Recovery codes"));

    // The imported login keeps the timestamps the export carried.
    let items = vault.list_items().unwrap();
    let fastmail = items.iter().find(|i| i.title == "Fastmail").unwrap();
    assert_eq!(fastmail.created_at, 1_600_000_000_000);
    assert_eq!(fastmail.updated_at, 1_600_000_500_000);
}

/// A locked vault cannot seal anything, import included.
#[test]
fn a_staged_import_needs_an_unlocked_vault() {
    use havenkeys_core::import::ImportReport;

    let (mut vault, _sk) = activated_vault();
    vault.lock();
    let err = vault
        .stage_import(vec![], ImportReport::default(), NOW)
        .unwrap_err();
    assert_eq!(err.code(), "locked");
}
