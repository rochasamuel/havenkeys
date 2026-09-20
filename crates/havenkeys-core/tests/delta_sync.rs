//! Delta sync against a server cursor. No network: the "server" here is a
//! Vec of changes, which is all the core ever sees.

mod common;

use common::{activated_with_account, second_device, NOW};

#[test]
fn a_new_item_is_pending_until_the_push_is_confirmed() {
    let (mut vault, _sk) = activated_with_account();
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();

    let pending = vault.pending_push().unwrap();
    assert_eq!(pending.base_cursor, 0);
    assert_eq!(pending.changes.len(), 1);
    assert_eq!(pending.changes[0].item_id, item.id);
    assert!(pending.changes[0].deleted_at.is_none());
    assert!(pending.changes[0].overview.is_some());

    vault.confirm_push(4, &[item.id], NOW).unwrap();
    assert!(vault.pending_push().unwrap().changes.is_empty());
    assert_eq!(vault.pending_push().unwrap().base_cursor, 4);
}

#[test]
fn a_deletion_is_pending_as_a_tombstone() {
    let (mut vault, _sk) = activated_with_account();
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    vault.confirm_push(1, &[item.id], NOW).unwrap();

    vault.delete_item(&item.id, NOW + 1000).unwrap();
    let pending = vault.pending_push().unwrap();
    assert_eq!(pending.changes.len(), 1);
    assert_eq!(pending.changes[0].deleted_at, Some(NOW + 1000));
    assert!(pending.changes[0].overview.is_none());
}

#[test]
fn a_remote_change_is_applied_and_advances_the_cursor() {
    // Two devices on one account: everything one pushes, the other applies.
    let (mut a, sk) = activated_with_account();
    let mut b = second_device(&a, &sk);

    let item = a
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let pending = a.pending_push().unwrap();

    let report = b.apply_remote_changes(9, pending.changes, NOW).unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(b.get_item(&item.id).unwrap().title, "GitHub");
    assert_eq!(b.pending_push().unwrap().base_cursor, 9);
    // Applying someone else's change must not make it pending here.
    assert!(b.pending_push().unwrap().changes.is_empty());
}

#[test]
fn a_tampered_remote_item_is_skipped_not_applied() {
    let (mut a, sk) = activated_with_account();
    let mut b = second_device(&a, &sk);

    let item = a
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let mut changes = a.pending_push().unwrap().changes;
    let blob = changes[0].overview.as_mut().unwrap();
    let n = blob.len();
    blob[n - 1] ^= 0x01;

    let report = b.apply_remote_changes(9, changes, NOW).unwrap();
    assert_eq!(report.added, 0);
    assert_eq!(report.skipped_items, 1);
    assert!(b.get_item(&item.id).is_err());
}

#[test]
fn a_blob_from_another_item_is_rejected() {
    // The AAD binds each blob to its item ID, so a server that swaps two
    // items' blobs must not be able to write either one.
    let (mut a, sk) = activated_with_account();
    let mut b = second_device(&a, &sk);

    a.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    a.create_item(common::login("GitLab", "me", "pw", "gitlab.com"), NOW)
        .unwrap();
    let mut changes = a.pending_push().unwrap().changes;
    assert_eq!(changes.len(), 2);
    let stolen = changes[0].overview.clone();
    changes[1].overview = stolen;

    let report = b.apply_remote_changes(9, changes, NOW).unwrap();
    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.added, 1);
}

#[test]
fn an_older_remote_version_loses_to_the_local_one() {
    let (mut a, sk) = activated_with_account();
    let mut b = second_device(&a, &sk);

    let item = a
        .create_item(
            common::login("GitHub", "me", "old", "github.com"),
            NOW,
        )
        .unwrap();
    let old = a.pending_push().unwrap().changes;
    b.apply_remote_changes(1, old.clone(), NOW).unwrap();

    // b edits it later, then the server replays the old version.
    b.update_item(
        &item.id,
        common::login("GitHub renamed", "me", "new", "github.com"),
        NOW + 5000,
    )
    .unwrap();
    let report = b.apply_remote_changes(2, old, NOW + 9000).unwrap();
    assert_eq!(report.updated, 0);
    assert_eq!(b.get_item(&item.id).unwrap().title, "GitHub renamed");
}
