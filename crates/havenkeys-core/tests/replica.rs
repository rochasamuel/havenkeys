//! Applying what the server serves. The server is untrusted but authoritative:
//! it cannot corrupt the replica, and it can empty it.

mod common;

use common::{activated_vault, login, NOW};
use havenkeys_core::model::SecretField;
use havenkeys_core::sync::RemoteChange;

fn change(vault: &havenkeys_core::vault::VaultService, title: &str, revision: i64) -> RemoteChange {
    let staged = vault
        .stage_create(login(title, "me", "pw", "github.com"), NOW)
        .unwrap();
    RemoteChange {
        item_id: staged.item_id,
        revision,
        overview: staged.overview.clone(),
        details: staged.details.clone(),
        deleted: false,
    }
}

#[test]
fn a_pull_adds_items_and_moves_the_cursor() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);
    let id = c.item_id;

    let report = vault.apply_remote_changes(5, vec![c], NOW).unwrap();

    assert_eq!(report.added, 1);
    assert_eq!(vault.get_item(&id).unwrap().title, "GitHub");
    assert_eq!(vault.account().unwrap().unwrap().server_cursor, 5);
}

#[test]
fn a_pull_deletes_without_asking_who_is_newer() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);
    let id = c.item_id;
    vault.apply_remote_changes(5, vec![c], NOW).unwrap();

    // No timestamp comparison, no resurrection arm: the server said so.
    let report = vault
        .apply_remote_changes(
            6,
            vec![RemoteChange {
                item_id: id,
                revision: 6,
                overview: None,
                details: None,
                deleted: true,
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.deleted, 1);
    assert!(vault.get_item(&id).is_err());
}

#[test]
fn a_blob_that_does_not_open_leaves_the_existing_row_alone() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);
    let id = c.item_id;
    vault.apply_remote_changes(5, vec![c], NOW).unwrap();

    let report = vault
        .apply_remote_changes(
            6,
            vec![RemoteChange {
                item_id: id,
                revision: 6,
                overview: Some(vec![0u8; 64]),
                details: Some(vec![0u8; 64]),
                deleted: false,
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.updated, 0);
    assert_eq!(vault.get_item(&id).unwrap().title, "GitHub");
}

#[test]
fn a_change_whose_blob_is_for_another_item_is_skipped() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);

    let report = vault
        .apply_remote_changes(
            5,
            vec![RemoteChange {
                item_id: uuid::Uuid::from_u128(999),
                ..c
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.skipped_items, 1);
    assert!(vault.list_items().unwrap().is_empty());
}

// --------------------------------------------------------- security regressions
//
// These three exist because a hostile server is not merely "sends garbage" —
// it can also replay a genuine ciphertext under the wrong label, or flip a
// single bit in one it legitimately stored. Random bytes do not exercise
// either case: they fail AEAD authentication before the AAD binding or the
// bit-flip detection is ever reached. Each test uses real sealed blobs from
// `stage_create` so the failure mode under test is the one that matters.

/// A blob that authenticates perfectly, but under a *different* item's id,
/// must not be accepted for this item. The AAD binds each blob to its own
/// item id (`crypto::blob::BlobContext::item`), so serving item A's genuine
/// ciphertext labeled as item B must fail to open under item B's context.
///
/// This fails if `check_item_bytes` ever derived the AAD from the change's
/// claimed `item_id` without also checking the decrypted overview's own
/// `id` against it, or if the AAD did not include the item id at all.
#[test]
fn a_blob_for_one_item_served_under_another_items_id_is_rejected() {
    let (vault, _sk) = activated_vault();
    let genuine = change(&vault, "GitHub", 5);
    let other_id = uuid::Uuid::from_u128(0xdead_beef);
    assert_ne!(genuine.item_id, other_id);

    let relabeled = RemoteChange {
        item_id: other_id,
        revision: genuine.revision,
        overview: genuine.overview.clone(),
        details: genuine.details.clone(),
        deleted: false,
    };

    let mut vault = vault;
    let report = vault.apply_remote_changes(5, vec![relabeled], NOW).unwrap();

    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.added, 0);
    assert!(vault.get_item(&other_id).is_err());
    assert!(vault.list_items().unwrap().is_empty());
}

/// A single flipped bit in an otherwise-genuine ciphertext must fail AEAD
/// authentication and be skipped — and, critically, must not touch whatever
/// row already exists for that item. A hostile server can withhold an
/// update; it must not be able to corrupt or blank a row by tampering with
/// one bit of a replayed blob.
///
/// This fails if `apply_remote_changes` ever wrote a row (or deleted one)
/// before authentication succeeded, e.g. if it applied deletions/upserts
/// optimistically instead of collecting them only after `check_item_bytes`
/// returned `Some`.
#[test]
fn a_single_flipped_bit_is_skipped_and_the_existing_row_survives() {
    let (mut vault, _sk) = activated_vault();
    let original = change(&vault, "GitHub", 5);
    let id = original.item_id;
    vault.apply_remote_changes(5, vec![original], NOW).unwrap();

    // Confirm the password is genuinely readable before we tamper.
    let before = vault.reveal(&id, SecretField::Password).unwrap();
    assert_eq!(before.expose(), "pw");

    // A genuine sealed update for the *same* item id — its blobs authenticate
    // under `id`'s AAD, so a bit flip is the only thing wrong with them.
    let staged_again = vault
        .stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW + 1)
        .unwrap();
    let mut tampered_details = staged_again.details.clone().unwrap();
    let last = tampered_details.len() - 1;
    tampered_details[last] ^= 0x01;

    let report = vault
        .apply_remote_changes(
            6,
            vec![RemoteChange {
                item_id: id,
                revision: 6,
                overview: staged_again.overview.clone(),
                details: Some(tampered_details),
                deleted: false,
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.updated, 0);

    // The old row is untouched: same title, same readable password.
    assert_eq!(vault.get_item(&id).unwrap().title, "GitHub");
    let after = vault.reveal(&id, SecretField::Password).unwrap();
    assert_eq!(after.expose(), "pw");
}

/// A change that is not a deletion but supplies only one of `overview` /
/// `details` is malformed. It must be counted in `skipped_items`, not
/// silently ignored and not applied partially.
///
/// This fails if the applier ever treated a missing `details` (or
/// `overview`) as "nothing to do" without incrementing `skipped_items`, or
/// if it tried to apply the half a change it did have.
#[test]
fn a_change_with_only_half_its_blobs_is_counted_not_dropped() {
    let (mut vault, _sk) = activated_vault();
    let c = change(&vault, "GitHub", 5);

    let report = vault
        .apply_remote_changes(
            5,
            vec![RemoteChange {
                item_id: c.item_id,
                revision: c.revision,
                overview: c.overview.clone(),
                details: None,
                deleted: false,
            }],
            NOW,
        )
        .unwrap();

    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.added, 0);
    assert!(vault.list_items().unwrap().is_empty());
}
