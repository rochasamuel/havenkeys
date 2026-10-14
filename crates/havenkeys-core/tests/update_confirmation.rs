//! When a browser password change needs the desktop's Allow (spec
//! 2026-10-13-confirm-repeat-password-change §3, §7).

mod common;

use common::{activated_vault, login, second_device, secret, NOW};
use havenkeys_core::sync::RemoteChange;
use havenkeys_core::vault::{SaveTarget, StagedWrite, VaultService, REPEAT_CHANGE_WINDOW_MS};
use havenkeys_core::Error;
use uuid::Uuid;

const HOUR: i64 = 3_600_000;
const PAGE: &str = "https://github.com/login";

fn github(v: &mut VaultService) -> Uuid {
    let staged = v
        .stage_create(login("GitHub", "octo", "first-pw", "github.com"), NOW)
        .unwrap();
    v.commit_write(staged, 1).unwrap().unwrap().id
}

/// The browser's change, as the bridge stages it, committed as the server would.
fn browser_change(v: &mut VaultService, id: &Uuid, pw: &str, at: i64, revision: i64) {
    let staged = v
        .stage_save_login(PAGE, None, None, secret(pw), SaveTarget::Update(id), at)
        .unwrap();
    v.commit_write(staged.write, revision).unwrap();
}

fn remote(w: &StagedWrite, revision: i64) -> RemoteChange {
    RemoteChange {
        item_id: w.item_id,
        revision,
        overview: w.overview.clone(),
        details: w.details.clone(),
        deleted: false,
    }
}

#[test]
fn the_first_change_needs_no_confirmation() {
    let (mut v, _sk) = activated_vault();
    let id = github(&mut v);
    assert!(!v.update_needs_confirmation(&id, NOW).unwrap());
    assert!(!v.update_needs_confirmation(&id, NOW + HOUR).unwrap());
}

#[test]
fn a_change_in_the_last_24_hours_needs_confirmation() {
    let (mut v, _sk) = activated_vault();
    let id = github(&mut v);
    browser_change(&mut v, &id, "second-pw", NOW + HOUR, 2);
    assert!(v.update_needs_confirmation(&id, NOW + 2 * HOUR).unwrap());
    assert!(v
        .update_needs_confirmation(&id, NOW + HOUR + REPEAT_CHANGE_WINDOW_MS - 1)
        .unwrap());
    assert!(!v
        .update_needs_confirmation(&id, NOW + HOUR + REPEAT_CHANGE_WINDOW_MS)
        .unwrap());
}

#[test]
fn a_desktop_edit_counts() {
    let (mut v, _sk) = activated_vault();
    let id = github(&mut v);
    let staged = v
        .stage_update(
            &id,
            login("GitHub", "octo", "desktop-pw", "github.com"),
            NOW + HOUR,
        )
        .unwrap();
    v.commit_write(staged, 2).unwrap();
    assert!(v.update_needs_confirmation(&id, NOW + 2 * HOUR).unwrap());
}

#[test]
fn a_change_synced_from_another_device_counts() {
    let (mut a, sk) = activated_vault();
    let mut b = second_device(&a, &sk);
    let created = a
        .stage_create(login("GitHub", "octo", "first-pw", "github.com"), NOW)
        .unwrap();
    let id = created.item_id;
    b.apply_remote_changes(1, vec![remote(&created, 1)], NOW)
        .unwrap();
    a.commit_write(created, 1).unwrap();
    let changed = a
        .stage_save_login(
            PAGE,
            None,
            None,
            secret("second-pw"),
            SaveTarget::Update(&id),
            NOW + HOUR,
        )
        .unwrap()
        .write;
    b.apply_remote_changes(2, vec![remote(&changed, 2)], NOW + HOUR)
        .unwrap();
    assert!(b.update_needs_confirmation(&id, NOW + 2 * HOUR).unwrap());
}

#[test]
fn an_entry_from_the_future_counts_as_recent() {
    let (mut v, _sk) = activated_vault();
    let id = github(&mut v);
    // Another device's clock ahead, or this one went backwards.
    browser_change(&mut v, &id, "second-pw", NOW + 10 * HOUR, 2);
    assert!(v.update_needs_confirmation(&id, NOW).unwrap());
}

#[test]
fn locked_and_unknown_items_are_errors() {
    let (mut v, _sk) = activated_vault();
    let id = github(&mut v);
    assert!(matches!(
        v.update_needs_confirmation(&Uuid::new_v4(), NOW),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        v.confirm_update_summary(&Uuid::new_v4(), PAGE),
        Err(Error::NotFound)
    ));
    v.lock();
    assert!(matches!(
        v.update_needs_confirmation(&id, NOW),
        Err(Error::Locked)
    ));
    assert!(matches!(
        v.confirm_update_summary(&id, PAGE),
        Err(Error::Locked)
    ));
}

#[test]
fn the_summary_names_the_login_and_the_site_and_nothing_else() {
    let (mut v, _sk) = activated_vault();
    let id = github(&mut v);
    let s = v
        .confirm_update_summary(&id, "https://www.github.com/session")
        .unwrap();
    assert_eq!(s.item_id, id);
    assert_eq!(s.title, "GitHub");
    assert_eq!(s.username.as_deref(), Some("octo"));
    assert_eq!(s.site, "github.com");
    let debug = format!("{s:?}");
    assert!(!debug.contains("GitHub") && !debug.contains("octo") && !debug.contains("github.com"));
    assert!(matches!(
        v.confirm_update_summary(&id, "about:blank"),
        Err(Error::Denied)
    ));
}
