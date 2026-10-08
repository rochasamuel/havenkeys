//! The Trash (spec 2026-10-08-trash).

mod common;

use common::{account, activated_vault, login, secret, NOW, PASSWORD};
use havenkeys_core::sync::RemoteChange;

use havenkeys_core::export::{
    self,
    backup::{open_backup, seal_backup},
    ExportFormat,
};
use havenkeys_core::model::SecretField;
use havenkeys_core::trash::TRASH_RETENTION_MS;
use havenkeys_core::Error;

const DAY: i64 = 86_400_000;

#[test]
fn an_old_overview_has_no_trashed_at_and_a_new_one_round_trips() {
    let ov: havenkeys_core::model::ItemOverview = serde_json::from_value(serde_json::json!({
        "id": uuid::Uuid::new_v4(), "itemType": "login", "title": "GitHub",
        "hasPassword": true, "hasTotp": false, "hasNotes": false,
        "createdAt": 1, "updatedAt": 1
    }))
    .unwrap();
    assert_eq!(ov.trashed_at, None);
    let json = serde_json::to_value(&ov).unwrap();
    assert!(json.get("trashedAt").is_none(), "absent when None");
}

#[test]
fn a_pulled_trashed_overview_goes_to_the_trash_and_a_pulled_restore_moves_the_item_back() {
    let (mut a, sk) = activated_vault();
    let mut b = common::second_device(&a, &sk);
    let staged = a
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let id = staged.item_id;
    let blobs = (
        staged.overview.clone().unwrap(),
        staged.details.clone().unwrap(),
    );
    a.commit_write(staged, 1).unwrap();
    b.apply_remote_changes(1, vec![change(id, 1, blobs)], NOW)
        .unwrap();
    assert_eq!(b.list_items().unwrap().len(), 1);

    let trashed = a.stage_trash(&id, NOW).unwrap();
    let blobs = (
        trashed.overview.clone().unwrap(),
        trashed.details.clone().unwrap(),
    );
    a.commit_write(trashed, 2).unwrap();
    b.apply_remote_changes(2, vec![change(id, 2, blobs)], NOW)
        .unwrap();
    assert!(b.list_items().unwrap().is_empty());
    assert_eq!(b.trash_count().unwrap(), 1);

    let restored = a.stage_restore_trashed(&id).unwrap();
    let blobs = (
        restored.overview.clone().unwrap(),
        restored.details.clone().unwrap(),
    );
    a.commit_write(restored, 3).unwrap();
    b.apply_remote_changes(3, vec![change(id, 3, blobs)], NOW)
        .unwrap();
    assert_eq!(b.list_items().unwrap().len(), 1);
    assert_eq!(b.trash_count().unwrap(), 0);
}

#[test]
fn the_trash_survives_lock_and_unlock_and_is_empty_while_locked() {
    let (mut v, sk) = activated_vault();
    let id = created(&mut v, "GitHub");
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 2).unwrap();
    v.lock();
    assert!(v.trash_count().is_err(), "locked");
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(v.trash_count().unwrap(), 1);
    assert!(v.list_items().unwrap().is_empty());
}

pub(crate) fn created(v: &mut havenkeys_core::vault::VaultService, title: &str) -> uuid::Uuid {
    let s = v
        .stage_create(login(title, "me", "pw", "github.com"), NOW)
        .unwrap();
    let id = s.item_id;
    v.commit_write(s, 1).unwrap();
    id
}

fn change(item_id: uuid::Uuid, revision: i64, (ov, det): (Vec<u8>, Vec<u8>)) -> RemoteChange {
    RemoteChange {
        item_id,
        revision,
        overview: Some(ov),
        details: Some(det),
        deleted: false,
    }
}

#[test]
fn trashing_moves_the_item_out_of_the_vault_and_keeps_its_secrets() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "GitHub");
    let t = v.stage_trash(&id, NOW).unwrap();
    assert_eq!(t.base_revision, Some(1));
    assert!(
        v.list_items().unwrap().len() == 1,
        "nothing changes before the server answers"
    );
    let ov = v.commit_write(t, 2).unwrap().expect("trashed, not deleted");
    assert_eq!(ov.trashed_at, Some(NOW));
    assert!(v.list_items().unwrap().is_empty());
    assert!(matches!(v.get_item(&id), Err(Error::NotFound)));

    let entries = v.list_trash(NOW + DAY).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].overview.id, id);
    assert_eq!(entries[0].days_left, 29);

    let r = v.stage_restore_trashed(&id).unwrap();
    let back = v.commit_write(r, 3).unwrap().unwrap();
    assert_eq!(back.trashed_at, None);
    assert_eq!(v.reveal(&id, SecretField::Password).unwrap().expose(), "pw");
    assert_eq!(v.trash_count().unwrap(), 0);
}

#[test]
fn trashing_and_restoring_keep_updated_at() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "GitHub");
    let t = v.stage_trash(&id, NOW + 5 * DAY).unwrap();
    let ov = v.commit_write(t, 2).unwrap().unwrap();
    assert_eq!(ov.updated_at, NOW);
}

#[test]
fn purge_and_empty_send_tombstones_for_trashed_items_only() {
    let (mut v, _sk) = activated_vault();
    let live = created(&mut v, "Live");
    let a = created(&mut v, "A");
    let b = created(&mut v, "B");
    for (id, rev) in [(a, 2), (b, 3)] {
        let t = v.stage_trash(&id, NOW).unwrap();
        v.commit_write(t, rev).unwrap();
    }
    assert!(matches!(v.stage_purge(&live), Err(Error::NotFound)));

    let p = v.stage_purge(&a).unwrap();
    assert!(p.overview.is_none() && p.details.is_none());
    assert_eq!(p.base_revision, Some(2));
    assert!(v.commit_write(p, 4).unwrap().is_none());
    assert_eq!(v.trash_count().unwrap(), 1);

    let all = v.stage_empty_trash().unwrap();
    assert_eq!(all.len(), 1);
    for w in all {
        v.commit_write(w, 5).unwrap();
    }
    assert_eq!(v.trash_count().unwrap(), 0);
    assert_eq!(v.list_items().unwrap().len(), 1);
}

#[test]
fn expiry_purges_at_30_days_not_before() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "Old");
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 2).unwrap();
    assert!(v
        .stage_expired_trash(NOW + TRASH_RETENTION_MS - 1)
        .unwrap()
        .is_empty());
    let due = v.stage_expired_trash(NOW + TRASH_RETENTION_MS).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].item_id, id);
    assert!(due[0].overview.is_none());
}

#[test]
fn a_future_trashed_at_is_not_purged() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "Future");
    let t = v.stage_trash(&id, NOW + 400 * DAY).unwrap();
    v.commit_write(t, 2).unwrap();
    assert!(v.stage_expired_trash(NOW).unwrap().is_empty());
}

#[test]
fn days_left_is_clamped() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "X");
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 2).unwrap();
    assert_eq!(
        v.list_trash(NOW - 10 * DAY).unwrap()[0].days_left,
        30,
        "clock behind"
    );
    assert_eq!(
        v.list_trash(NOW + 90 * DAY).unwrap()[0].days_left,
        0,
        "overdue"
    );
    assert_eq!(v.list_trash(NOW).unwrap()[0].days_left, 30);
    assert_eq!(
        v.list_trash(NOW + DAY / 2).unwrap()[0].days_left,
        30,
        "rounds up"
    );
}

#[test]
fn list_trash_is_newest_first() {
    let (mut v, _sk) = activated_vault();
    let a = created(&mut v, "A");
    let b = created(&mut v, "B");
    let t = v.stage_trash(&a, NOW).unwrap();
    v.commit_write(t, 2).unwrap();
    let t = v.stage_trash(&b, NOW + 1).unwrap();
    v.commit_write(t, 3).unwrap();
    let ids: Vec<_> = v
        .list_trash(NOW)
        .unwrap()
        .iter()
        .map(|e| e.overview.id)
        .collect();
    assert_eq!(ids, vec![b, a]);
}

#[test]
fn the_identity_cannot_be_trashed() {
    let (mut v, _sk) = activated_vault();
    let staged = v
        .stage_identity_if_missing("me@example.com", NOW)
        .unwrap()
        .unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 1).unwrap();
    assert!(matches!(v.stage_trash(&id, NOW), Err(Error::Denied)));
}

#[test]
fn restore_and_trash_check_the_right_map() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "X");
    assert!(
        matches!(v.stage_restore_trashed(&id), Err(Error::NotFound)),
        "not in the trash"
    );
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 2).unwrap();
    assert!(
        matches!(v.stage_trash(&id, NOW), Err(Error::NotFound)),
        "already trashed"
    );
    assert!(
        matches!(v.stage_delete(&id), Err(Error::NotFound)),
        "live-item delete does not reach it"
    );
}

#[test]
fn unknown_overview_fields_survive_trash_and_restore() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "X");
    v.insert_extra_for_tests(&id, "fromTheFuture", serde_json::json!(7));
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 2).unwrap();
    let r = v.stage_restore_trashed(&id).unwrap();
    let back = v.commit_write(r, 3).unwrap().unwrap();
    assert_eq!(back.extra.get("fromTheFuture"), Some(&serde_json::json!(7)));
}

#[test]
fn unreadable_details_are_deleted_not_trashed() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "Broken");
    v.corrupt_details_for_tests(&id);
    let t = v.stage_trash(&id, NOW).unwrap();
    assert!(t.overview.is_none() && t.details.is_none(), "a tombstone");
    assert!(v.commit_write(t, 2).unwrap().is_none());
    assert_eq!(v.trash_count().unwrap(), 0);
}

#[test]
fn a_backup_restore_skips_items_in_the_trash() {
    let (mut v, _sk) = activated_vault();
    let id = created(&mut v, "Bank");
    let payload = export::render(&v, ExportFormat::Backup, NOW).unwrap();
    let file = seal_backup(
        &payload.bytes,
        &secret("a separate backup passphrase"),
        &common::fast_kdf(),
    )
    .unwrap();
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 2).unwrap();

    let opened = open_backup(&file, &secret("a separate backup passphrase")).unwrap();
    let staged = v.stage_restore(opened, NOW).unwrap();
    assert_eq!(staged.report.skipped_existing, 1);
    assert!(staged.writes.iter().all(|w| w.item_id != id));
}

const GH: &str = "https://github.com/login";

/// A GitHub login with a password, a TOTP secret and a passkey, then trashed.
/// Returns its id and the passkey's credential id.
fn trashed_github(v: &mut havenkeys_core::vault::VaultService) -> (uuid::Uuid, Vec<u8>) {
    let mut input = login("GitHub", "octo", "gh-secret-pw", "github.com");
    input.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXPJBSWY3DP"));
    let s = v.stage_create(input, NOW).unwrap();
    let id = s.item_id;
    v.commit_write(s, 1).unwrap();
    let pk = v
        .stage_passkey_create(
            havenkeys_core::passkey::PasskeyCreate {
                rp_id: "github.com",
                page_url: GH,
                top_url: None,
                challenge: &[7; 32],
                user_handle: &[1],
                user_name: "octo",
                display_name: None,
                item_id: Some(id),
                conditional: false,
            },
            NOW,
        )
        .unwrap();
    let cred = pk.registration.credential_id.clone();
    v.commit_write(pk.write, 2).unwrap();
    let t = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(t, 3).unwrap();
    (id, cred)
}

#[test]
fn attack_a_trashed_login_is_not_filled_on_its_own_site() {
    let (mut v, _sk) = activated_vault();
    let (id, _) = trashed_github(&mut v);

    let page = "https://github.com/login";
    assert!(v.find_matches(page, None).unwrap().is_empty());
    assert!(v.fill_for_page(&id, page, None, NOW).is_err());
    assert!(v.totp_for_page(&id, page, None, 59).is_err());
    assert!(v.totp_code(&id, 59).is_err());
    assert!(v.search("git").unwrap().is_empty());
    assert!(v.search_logins("git").unwrap().is_empty());
    assert!(v.list_items().unwrap().is_empty());
    assert!(v
        .stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW)
        .is_err());
    assert!(v.record_use(&id, NOW).is_err());
    assert!(v
        .frequently_used(10, NOW)
        .unwrap()
        .iter()
        .all(|o| o.id != id));
    assert!(v.recently_created(10).unwrap().iter().all(|o| o.id != id));
    assert!(v.reveal(&id, SecretField::Password).is_err());
}

#[test]
fn a_trashed_login_is_offered_to_no_app_and_no_passkey_request() {
    use havenkeys_core::app_target::AppIdentity;
    let (mut v, _sk) = activated_vault();
    let (id, cred) = trashed_github(&mut v);

    // An app whose verified host is github.com.
    let app = AppIdentity::new("com.github.android", &[vec![0xab; 32]]).unwrap();
    let hosts = vec!["github.com".to_string()];
    assert!(v.matches_for_app(&app, &hosts).unwrap().is_empty());
    assert!(v.fill_for_app(&id, &app, &hosts).is_err());

    assert!(v
        .find_passkeys("github.com", GH, None, &[])
        .unwrap()
        .is_empty());
    assert!(!v.has_passkey_for_page(GH, None).unwrap());
    assert!(v
        .passkey_assert(&id, &cred, "github.com", GH, None, &[3; 32])
        .is_err());
    let cred_b64 = havenkeys_core::passkey::encode_b64url(&cred);
    assert!(v.stage_remove_passkey(&id, &cred_b64, NOW).is_err());
}

#[test]
fn health_and_export_leave_the_trash_out() {
    let (mut v, _sk) = activated_vault();
    let (id, _) = trashed_github(&mut v);
    // A live login with the same weak shape proves the readers do see live items.
    let s = v
        .stage_create(login("Bank", "alice", "pw", "mybank.com"), NOW)
        .unwrap();
    let live = s.item_id;
    v.commit_write(s, 4).unwrap();

    let report = v.health_report(NOW).unwrap();
    assert!(report.issues.iter().any(|i| i.item_id == live));
    assert!(report.issues.iter().all(|i| i.item_id != id));

    for format in [ExportFormat::BitwardenJson, ExportFormat::Csv] {
        let out = export::render(&v, format, NOW).unwrap();
        let text = std::str::from_utf8(&out.bytes).unwrap();
        assert!(!text.contains("gh-secret-pw") && !text.contains("JBSWY3DPEHPK3PXP"));
        assert!(!text.contains("octo") && !text.contains("GitHub"));
        assert!(text.contains("alice"), "live items are exported");
    }
    let backup = export::render(&v, ExportFormat::Backup, NOW).unwrap();
    assert_eq!(backup.summary.logins, 1);
    let file = seal_backup(
        &backup.bytes,
        &secret("a separate backup passphrase"),
        &common::fast_kdf(),
    )
    .unwrap();
    let opened = open_backup(&file, &secret("a separate backup passphrase")).unwrap();
    assert_eq!(opened.items.len(), 1, "only the live login");
}

#[test]
fn saving_a_login_ignores_the_trash() {
    use havenkeys_core::vault::SaveAction;
    let (mut v, _sk) = activated_vault();
    trashed_github(&mut v);
    let action = v
        .check_login(
            "https://github.com/session",
            None,
            Some("octo"),
            &secret("gh-secret-pw"),
            None,
        )
        .unwrap();
    assert_eq!(
        action,
        SaveAction::Add,
        "no trashed login to match or update"
    );
}
