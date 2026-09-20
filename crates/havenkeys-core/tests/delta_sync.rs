//! Delta sync against a server cursor. No network: the "server" here is a
//! Vec of changes, which is all the core ever sees.

mod common;

use common::{activated_with_account, new_vault, second_device, NOW};
use havenkeys_core::sync::RemoteChange;
use havenkeys_core::Error;

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

    vault.confirm_push(4, &pending.changes, NOW).unwrap();
    assert!(vault.pending_push().unwrap().changes.is_empty());
    assert_eq!(vault.pending_push().unwrap().base_cursor, 4);
}

#[test]
fn a_deletion_is_pending_as_a_tombstone() {
    let (mut vault, _sk) = activated_with_account();
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let pending = vault.pending_push().unwrap();
    vault.confirm_push(1, &pending.changes, NOW).unwrap();

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
        .create_item(common::login("GitHub", "me", "old", "github.com"), NOW)
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

#[test]
fn an_edit_during_the_push_round_trip_stays_pending() {
    let (mut vault, _sk) = activated_with_account();
    vault
        .create_item(common::login("GitHub", "me", "old", "github.com"), NOW)
        .unwrap();
    let pending = vault.pending_push().unwrap();
    let item_id = pending.changes[0].item_id;

    // The user edits the item after pending_push read it but before the
    // server's ack for that version arrives.
    vault
        .update_item(
            &item_id,
            common::login("GitHub", "me", "new", "github.com"),
            NOW + 500,
        )
        .unwrap();

    vault.confirm_push(4, &pending.changes, NOW + 1000).unwrap();

    let still_pending = vault.pending_push().unwrap();
    assert_eq!(still_pending.changes.len(), 1);
    assert_eq!(still_pending.changes[0].item_id, item_id);
}

#[test]
fn a_deletion_during_the_push_round_trip_stays_pending() {
    let (mut vault, _sk) = activated_with_account();
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let pending = vault.pending_push().unwrap();

    // The user deletes the item after pending_push read the create, but
    // before the server's ack for that create arrives.
    vault.delete_item(&item.id, NOW + 500).unwrap();

    vault.confirm_push(4, &pending.changes, NOW + 1000).unwrap();

    let still_pending = vault.pending_push().unwrap();
    assert_eq!(still_pending.changes.len(), 1);
    assert_eq!(still_pending.changes[0].item_id, item.id);
    assert_eq!(still_pending.changes[0].deleted_at, Some(NOW + 500));
}

#[test]
fn sync_methods_require_a_linked_account() {
    let mut vault = new_vault();
    let want = Some(Error::InvalidInput(
        "this vault is not linked to an account",
    ));
    assert_eq!(vault.pending_push().err(), want);
    assert_eq!(vault.apply_remote_changes(1, vec![], NOW).err(), want);
    assert_eq!(vault.confirm_push(1, &[], NOW).err(), want);
}

#[test]
fn a_malformed_change_is_counted_not_dropped() {
    let (mut a, sk) = activated_with_account();
    let mut b = second_device(&a, &sk);

    let item = a
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let mut changes = a.pending_push().unwrap().changes;
    // Neither a well-formed upsert (both blobs) nor a deletion.
    changes[0].details = None;

    let report = b.apply_remote_changes(9, changes, NOW).unwrap();
    assert_eq!(report.added, 0);
    assert_eq!(report.skipped_items, 1);
    assert!(b.get_item(&item.id).is_err());
}

#[test]
fn a_deletion_far_in_the_future_is_skipped_and_counted() {
    // deleted_at is plaintext the server supplies and is not authenticated
    // (docs/crypto.md, "Key scheme 3 (account)"). The worst forgery,
    // deleted_at = i64::MAX, would otherwise poison this item ID forever:
    // apply_remote_changes must reject it instead of tombstoning the item.
    let (mut vault, _sk) = activated_with_account();
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();

    let change = RemoteChange {
        item_id: item.id,
        overview: None,
        details: None,
        deleted_at: Some(i64::MAX),
    };
    let report = vault.apply_remote_changes(9, vec![change], NOW).unwrap();
    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.deleted, 0);
    assert!(vault.get_item(&item.id).is_ok(), "item must survive");
}

#[test]
fn a_deletion_at_now_still_applies() {
    // The mitigation above must not reject ordinary, plausible deletions.
    let (mut vault, _sk) = activated_with_account();
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();

    let change = RemoteChange {
        item_id: item.id,
        overview: None,
        details: None,
        deleted_at: Some(NOW),
    };
    let report = vault.apply_remote_changes(9, vec![change], NOW).unwrap();
    assert_eq!(report.skipped_items, 0);
    assert_eq!(report.deleted, 1);
    assert!(vault.get_item(&item.id).is_err());
}
