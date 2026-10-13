// Equivalent domains through the public vault API (spec 2026-10-13 §4).

mod common;
use havenkeys_core::model::{MatchType, Settings};
use havenkeys_core::origin::MatchStrength;
use havenkeys_core::vault::SaveAction;

const NOW: i64 = 1_800_000_000_000;

#[test]
fn apple_login_is_offered_and_filled_on_icloud() {
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    let m = v
        .find_matches("https://www.icloud.com/", None, NOW)
        .unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].strength, MatchStrength::Equivalent);
    assert!(v
        .fill_for_page(&id, "https://www.icloud.com/", None, NOW)
        .is_ok());
    assert!(v
        .totp_for_page(&id, "https://www.icloud.com/", None, 1_800_000_000)
        .is_ok());
}

#[test]
fn setting_off_denies_equivalent_sites() {
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    v.update_settings(Settings {
        equivalent_domains: false,
        ..v.settings().unwrap()
    })
    .unwrap();
    assert!(v
        .find_matches("https://www.icloud.com/", None, NOW)
        .unwrap()
        .is_empty());
    assert!(v
        .fill_for_page(&id, "https://www.icloud.com/", None, NOW)
        .is_err());
    assert!(v
        .totp_for_page(&id, "https://www.icloud.com/", None, 1_800_000_000)
        .is_err());
    // Direct matches are unaffected.
    assert_eq!(
        v.find_matches("https://apple.com/", None, NOW)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn origin_rules_never_widen() {
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Origin);
    assert!(v
        .find_matches("https://icloud.com/", None, NOW)
        .unwrap()
        .is_empty());
    assert!(v
        .fill_for_page(&id, "https://icloud.com/", None, NOW)
        .is_err());
}

#[test]
fn frames_need_both_pages_to_match() {
    let mut v = common::unlocked_vault();
    common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    assert!(v
        .find_matches("https://www.icloud.com/", Some("https://evil.com/"), NOW)
        .unwrap()
        .is_empty());
    assert_eq!(
        v.find_matches("https://www.icloud.com/", Some("https://apple.com/"), NOW)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn direct_matches_always_rank_first() {
    let mut v = common::unlocked_vault();
    common::add_login(&mut v, "A Apple", "https://apple.com", MatchType::Domain);
    common::add_login(&mut v, "Z iCloud", "https://icloud.com", MatchType::Domain);
    let m = v.find_matches("https://icloud.com/", None, NOW).unwrap();
    assert_eq!(
        m.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        ["Z iCloud", "A Apple"]
    );
}

#[test]
fn settings_without_the_field_default_on() {
    let json = r#"{"autoLockMinutes":15,"clipboardClearSeconds":30}"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert!(s.equivalent_domains);
    assert!(Settings::default().equivalent_domains);
}

#[test]
fn restrictive_settings_turn_equivalence_off() {
    assert!(!Settings::restrictive().equivalent_domains);
}

#[test]
fn save_dedup_never_widens() {
    let mut v = common::unlocked_vault();
    common::add_login(&mut v, "YouTube", "https://youtube.com", MatchType::Domain);
    // Save-login dedup on an equivalent site proposes a new login, not an update.
    let pw = havenkeys_core::SecretString::from("a-new-password-123");
    assert_eq!(
        v.check_login("https://google.com/", None, Some("octo"), &pw, None)
            .unwrap(),
        SaveAction::Add
    );
}

#[test]
fn save_update_never_widens() {
    use havenkeys_core::vault::SaveTarget;
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "YouTube", "https://youtube.com", MatchType::Domain);
    let pw = || havenkeys_core::SecretString::from("a-new-password-123");
    // The login fills on google.com, but a save there may not overwrite it.
    assert!(v
        .fill_for_page(&id, "https://google.com/", None, NOW)
        .is_ok());
    assert!(v
        .stage_save_login(
            "https://google.com/",
            None,
            Some("octo"),
            pw(),
            SaveTarget::Update(&id),
            NOW
        )
        .is_err());
    assert!(v
        .stage_save_login(
            "https://youtube.com/",
            None,
            Some("octo"),
            pw(),
            SaveTarget::Update(&id),
            NOW
        )
        .is_ok());
}

fn equivalence_off(v: &mut havenkeys_core::vault::VaultService) {
    v.update_settings(Settings {
        equivalent_domains: false,
        ..v.settings().unwrap()
    })
    .unwrap();
}

#[test]
fn a_login_filled_on_a_related_site_is_not_offered_again() {
    let mut v = common::unlocked_vault();
    common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    let page = "https://www.icloud.com/";
    let same = common::secret("hunter2hunter2");
    let other = common::secret("a-new-password-123");
    assert_eq!(
        v.check_login(page, None, Some("octo"), &same, None)
            .unwrap(),
        SaveAction::Unchanged
    );
    // A password-only form with the saved password is unchanged too.
    assert_eq!(
        v.check_login(page, None, None, &same, None).unwrap(),
        SaveAction::Unchanged
    );
    // A different password never updates a login matched only by equivalence.
    assert_eq!(
        v.check_login(page, None, Some("octo"), &other, None)
            .unwrap(),
        SaveAction::Add
    );
    assert_eq!(
        v.check_login(page, None, None, &other, Some(&same))
            .unwrap(),
        SaveAction::Add
    );
    // A frame embedded in an unrelated page widens nothing.
    assert_eq!(
        v.check_login(page, Some("https://evil.com/"), Some("octo"), &same, None)
            .unwrap(),
        SaveAction::Add
    );
    equivalence_off(&mut v);
    assert_eq!(
        v.check_login(page, None, Some("octo"), &same, None)
            .unwrap(),
        SaveAction::Add
    );
}

#[test]
fn a_direct_update_still_wins_over_a_related_unchanged() {
    let mut v = common::unlocked_vault();
    common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    let icloud = common::add_login(&mut v, "iCloud", "https://icloud.com", MatchType::Domain);
    let staged = v
        .stage_save_login(
            "https://icloud.com/",
            None,
            Some("octo"),
            common::secret("icloud-only-pass"),
            havenkeys_core::vault::SaveTarget::Update(&icloud),
            NOW,
        )
        .unwrap();
    v.commit_write(staged.write, 3).unwrap();
    // Typing apple.com's password on icloud.com proposes updating the
    // direct login, as before.
    assert_eq!(
        v.check_login(
            "https://icloud.com/",
            None,
            Some("octo"),
            &common::secret("hunter2hunter2"),
            None
        )
        .unwrap(),
        SaveAction::Update(icloud)
    );
}

fn sso_login(title: &str, account: Option<&str>, url: &str) -> havenkeys_core::model::ItemInput {
    let mut input = common::login(title, "", "", url);
    input.username = None;
    input.password = havenkeys_core::model::SecretUpdate::Keep;
    input.sign_in_with = Some(havenkeys_core::sso::SignInWith {
        provider: havenkeys_core::sso::SsoProvider::Google,
        account: account.map(str::to_owned),
    });
    input
}

#[test]
fn sign_in_with_on_a_related_site_is_unchanged_but_never_updated() {
    use havenkeys_core::sso::SsoProvider;
    let mut v = common::unlocked_vault();
    let staged = v
        .stage_create(
            sso_login("Apple", Some("me@gmail.com"), "https://apple.com"),
            NOW,
        )
        .unwrap();
    v.commit_write(staged, 1).unwrap();
    let staged = v
        .stage_create(sso_login("YouTube", None, "https://youtube.com"), NOW)
        .unwrap();
    v.commit_write(staged, 2).unwrap();
    assert_eq!(
        v.check_sso(
            "https://icloud.com/",
            None,
            SsoProvider::Google,
            Some("ME@gmail.com")
        )
        .unwrap(),
        SaveAction::Unchanged
    );
    assert_eq!(
        v.check_sso(
            "https://icloud.com/",
            None,
            SsoProvider::Google,
            Some("other@gmail.com")
        )
        .unwrap(),
        SaveAction::Add
    );
    // youtube.com's login lacks an account: on google.com it is not updated.
    assert_eq!(
        v.check_sso(
            "https://google.com/",
            None,
            SsoProvider::Google,
            Some("me@gmail.com")
        )
        .unwrap(),
        SaveAction::Add
    );
    equivalence_off(&mut v);
    assert_eq!(
        v.check_sso(
            "https://icloud.com/",
            None,
            SsoProvider::Google,
            Some("me@gmail.com")
        )
        .unwrap(),
        SaveAction::Add
    );
}

#[test]
fn page_membership_follows_the_setting_and_needs_no_ranking() {
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    assert!(v
        .matches_page(&id, "https://www.icloud.com/", None)
        .unwrap());
    assert!(v.matches_page(&id, "https://apple.com/", None).unwrap());
    assert!(!v.matches_page(&id, "https://evil.com/", None).unwrap());
    assert!(!v
        .matches_page(&id, "https://www.icloud.com/", Some("https://evil.com/"))
        .unwrap());
    assert!(!v.matches_page(&id, "not a url", None).unwrap());
    assert!(!v
        .matches_page(&uuid::Uuid::new_v4(), "https://apple.com/", None)
        .unwrap());
    let note = v.stage_create(common::note("n", "c"), NOW).unwrap();
    let note_id = note.item_id;
    v.commit_write(note, 2).unwrap();
    assert!(!v
        .matches_page(&note_id, "https://apple.com/", None)
        .unwrap());
    equivalence_off(&mut v);
    assert!(!v
        .matches_page(&id, "https://www.icloud.com/", None)
        .unwrap());
    assert!(v.matches_page(&id, "https://apple.com/", None).unwrap());
    let _ = v.lock();
    assert!(v.matches_page(&id, "https://apple.com/", None).is_err());
}

fn some_app() -> havenkeys_core::app_target::AppIdentity {
    havenkeys_core::app_target::AppIdentity::new("com.apple.icloud", &[vec![7; 32]]).unwrap()
}

#[test]
fn an_app_vouched_for_by_a_related_site_gets_the_login() {
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    let hosts = vec!["icloud.com".to_owned()];
    let m = v.matches_for_app(&some_app(), &hosts, NOW).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].strength, MatchStrength::Equivalent);
    assert!(v.fill_for_app(&id, &some_app(), &hosts).is_ok());
    equivalence_off(&mut v);
    assert!(v
        .matches_for_app(&some_app(), &hosts, NOW)
        .unwrap()
        .is_empty());
    assert!(v.fill_for_app(&id, &some_app(), &hosts).is_err());
}

#[test]
fn an_app_save_widens_only_to_unchanged() {
    use havenkeys_core::vault::SaveTarget;
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    let hosts = vec!["icloud.com".to_owned()];
    let same = common::secret("hunter2hunter2");
    let other = common::secret("a-new-password-123");
    assert_eq!(
        v.check_login_for_app(&some_app(), &hosts, Some("octo"), &same, None)
            .unwrap(),
        SaveAction::Unchanged
    );
    assert_eq!(
        v.check_login_for_app(&some_app(), &hosts, Some("octo"), &other, None)
            .unwrap(),
        SaveAction::Add
    );
    assert_eq!(
        v.stage_save_login_for_app(
            &some_app(),
            &hosts,
            Some("octo"),
            other,
            SaveTarget::Update(&id),
            NOW
        )
        .err(),
        Some(havenkeys_core::Error::Denied)
    );
    equivalence_off(&mut v);
    assert_eq!(
        v.check_login_for_app(&some_app(), &hosts, Some("octo"), &same, None)
            .unwrap(),
        SaveAction::Add
    );
}
