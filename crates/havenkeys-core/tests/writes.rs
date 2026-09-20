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
