// The look-alike warning through the public vault API
// (spec 2026-10-13-lookalike-warning §3.2, §8).

mod common;
use common::{add_login, unlocked_vault, NOW};
use havenkeys_core::model::{MatchType, Settings};
use havenkeys_core::Error;

/// The page URL `url` makes of `host` (punycode for IDNs): never hand-written.
fn page(host: &str) -> String {
    url::Url::parse(&format!("https://{host}/login"))
        .unwrap()
        .to_string()
}

#[test]
fn look_alikes_name_the_saved_site() {
    let mut v = unlocked_vault();
    add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    add_login(&mut v, "Google", "https://google.com", MatchType::Origin);
    add_login(
        &mut v,
        "Microsoft",
        "https://login.microsoft.com/x",
        MatchType::Exact,
    );
    let cyrillic = page("p\u{0430}ypal.com");
    assert!(cyrillic.contains("xn--"), "{cyrillic}");
    for (url, saved) in [
        (cyrillic.as_str(), "paypal.com"),
        ("https://g00gle.com/", "google.com"),
        ("https://www.rnicrosoft.com/", "microsoft.com"),
        ("https://paypa1.com/", "paypal.com"),
    ] {
        assert_eq!(
            v.lookalike_for_page(url, None).unwrap().as_deref(),
            Some(saved),
            "{url}"
        );
        // Still nothing to fill there.
        assert!(v.find_matches(url, None, NOW).unwrap().is_empty(), "{url}");
    }
}

#[test]
fn other_suffixes_unrelated_sites_and_hosts_without_a_site_do_not_warn() {
    let mut v = unlocked_vault();
    add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    for url in [
        "https://paypal.co.uk/",
        "https://paypal-login.com/",
        "https://example.org/",
        "https://192.168.1.1/",
        "http://localhost:8080/",
        "https://paypal.invalidsuffix/",
        "about:blank",
        "not a url",
    ] {
        assert_eq!(v.lookalike_for_page(url, None).unwrap(), None, "{url}");
    }
}

#[test]
fn a_matching_page_never_warns_even_if_it_resembles_another_saved_site() {
    let mut v = unlocked_vault();
    add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    assert_eq!(
        v.lookalike_for_page("https://www.paypal.com/", None)
            .unwrap(),
        None
    );
    // Without a login for paypa1.com the page warns...
    assert_eq!(
        v.lookalike_for_page("https://paypa1.com/", None)
            .unwrap()
            .as_deref(),
        Some("paypal.com"),
    );
    // ...and once the user really has one, it matches and the guard silences it.
    add_login(&mut v, "Paypa1", "https://paypa1.com", MatchType::Domain);
    assert_eq!(
        v.lookalike_for_page("https://paypa1.com/", None).unwrap(),
        None
    );
}

#[test]
fn an_equivalent_site_does_not_warn() {
    let mut v = unlocked_vault();
    add_login(&mut v, "Apple", "https://apple.com", MatchType::Domain);
    assert!(v.settings().unwrap().equivalent_domains);
    assert_eq!(
        v.lookalike_for_page("https://www.icloud.com/", None)
            .unwrap(),
        None
    );
}

#[test]
fn several_candidates_answer_the_first_in_sorted_order() {
    let mut v = unlocked_vault();
    add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    add_login(&mut v, "Paypa1", "https://paypa1.com", MatchType::Domain);
    let url = page("p\u{0430}ypal.com");
    for _ in 0..3 {
        assert_eq!(
            v.lookalike_for_page(&url, None).unwrap().as_deref(),
            Some("paypa1.com")
        );
    }
}

#[test]
fn a_frame_inside_a_look_alike_top_page_warns() {
    let mut v = unlocked_vault();
    add_login(&mut v, "Google", "https://google.com", MatchType::Domain);
    assert_eq!(
        v.lookalike_for_page("https://accounts.google.com/", Some("https://g00gle.com/"))
            .unwrap()
            .as_deref(),
        Some("google.com"),
    );
    // A look-alike frame inside an unrelated top page warns too.
    assert_eq!(
        v.lookalike_for_page("https://g00gle.com/", Some("https://news.example/"))
            .unwrap()
            .as_deref(),
        Some("google.com"),
    );
}

#[test]
fn an_unparsable_top_page_is_no_warning() {
    let mut v = unlocked_vault();
    add_login(&mut v, "Google", "https://google.com", MatchType::Domain);
    assert_eq!(
        v.lookalike_for_page("https://g00gle.com/", Some("not a url"))
            .unwrap(),
        None
    );
}

#[test]
fn a_locked_vault_answers_locked() {
    let mut v = unlocked_vault();
    add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    v.lock();
    assert!(matches!(
        v.lookalike_for_page("https://paypa1.com/", None),
        Err(Error::Locked)
    ));
    assert!(matches!(
        v.lookalike_for_page("not a url", None),
        Err(Error::Locked)
    ));
}

#[test]
fn trashed_logins_are_ignored() {
    let mut v = unlocked_vault();
    let id = add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    let staged = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(staged, 100).unwrap();
    assert_eq!(
        v.lookalike_for_page("https://paypa1.com/", None).unwrap(),
        None
    );
}

#[test]
fn the_setting_for_related_sites_does_not_change_look_alikes() {
    let mut v = unlocked_vault();
    add_login(&mut v, "PayPal", "https://paypal.com", MatchType::Domain);
    v.update_settings(Settings {
        equivalent_domains: false,
        ..v.settings().unwrap()
    })
    .unwrap();
    assert_eq!(
        v.lookalike_for_page("https://paypa1.com/", None)
            .unwrap()
            .as_deref(),
        Some("paypal.com")
    );
}
