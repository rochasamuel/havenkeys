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
    let m = v.find_matches("https://www.icloud.com/", None).unwrap();
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
        .find_matches("https://www.icloud.com/", None)
        .unwrap()
        .is_empty());
    assert!(v
        .fill_for_page(&id, "https://www.icloud.com/", None, NOW)
        .is_err());
    assert!(v
        .totp_for_page(&id, "https://www.icloud.com/", None, 1_800_000_000)
        .is_err());
    // Direct matches are unaffected.
    assert_eq!(v.find_matches("https://apple.com/", None).unwrap().len(), 1);
}

#[test]
fn origin_rules_never_widen() {
    let mut v = common::unlocked_vault();
    let id = common::add_login(&mut v, "Apple", "https://apple.com", MatchType::Origin);
    assert!(v
        .find_matches("https://icloud.com/", None)
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
        .find_matches("https://www.icloud.com/", Some("https://evil.com/"))
        .unwrap()
        .is_empty());
    assert_eq!(
        v.find_matches("https://www.icloud.com/", Some("https://apple.com/"))
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
    let m = v.find_matches("https://icloud.com/", None).unwrap();
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
