//! Passkeys through the vault: origin binding, lifecycle and the security
//! regressions in docs/threat-model.md §5 (passkey variants).

mod common;

use common::*;
use havenkeys_core::model::{MatchType, Settings, UrlRule};
use havenkeys_core::passkey::{
    CreateQuery, PasskeyCreate, StagedPasskey, Upgrade, MAX_PASSKEYS_PER_LOGIN,
};
use havenkeys_core::vault::{VaultService, MAX_RECENT_FILLS, UPGRADE_WINDOW_MS};
use havenkeys_core::Error;
use uuid::Uuid;

const GH: &str = "https://github.com/login";

fn create_req<'a>(user: &'a str, handle: &'a [u8], item: Option<Uuid>) -> PasskeyCreate<'a> {
    PasskeyCreate {
        rp_id: "github.com",
        page_url: GH,
        top_url: None,
        challenge: &[7; 32],
        user_handle: handle,
        user_name: user,
        display_name: None,
        item_id: item,
        conditional: false,
    }
}

/// A non-conditional `check_passkey_create` query, matching the calls this
/// file made before `CreateQuery` existed.
fn plain<'a>(user: &'a str, exclude: &'a [Vec<u8>]) -> CreateQuery<'a> {
    CreateQuery {
        rp_id: "github.com",
        page_url: GH,
        top_url: None,
        user_name: user,
        exclude,
        conditional: false,
    }
}

struct Rev(i64);
impl Rev {
    fn next(&mut self) -> i64 {
        self.0 += 1;
        self.0
    }
}

fn commit(v: &mut VaultService, rev: &mut Rev, s: StagedPasskey) -> (Uuid, Vec<u8>) {
    let id = s.item_id;
    let cred = s.registration.credential_id.clone();
    v.commit_write(s.write, rev.next()).unwrap();
    (id, cred)
}

#[test]
fn create_then_sign_in() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (id, cred) = commit(&mut v, &mut rev, s);
    let item = v.get_item(&id).unwrap();
    assert!(item.has_passkey);
    assert_eq!(item.title, "github.com");
    assert_eq!(item.username.as_deref(), Some("octo"));

    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].item_id, id);
    assert_eq!(found[0].credential_id, cred);
    assert_eq!(found[0].user_name, "octo");

    let a = v
        .passkey_assert(&id, &cred, "github.com", GH, None, &[3; 32])
        .unwrap();
    assert_eq!(a.credential_id, cred);
    // From a subdomain, with the parent rpId, too.
    v.passkey_assert(
        &id,
        &cred,
        "github.com",
        "https://accounts.github.com/",
        None,
        &[3; 32],
    )
    .unwrap();
}

#[test]
fn attack_wrong_origin_is_denied() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (id, cred) = commit(&mut v, &mut rev, s);
    for page in [
        "https://evil.com/",
        "https://github.com.evil.com/",
        "https://evilgithub.com/",
    ] {
        // Claiming github.com's rpId from another site.
        assert_eq!(
            v.passkey_assert(&id, &cred, "github.com", page, None, &[1])
                .err(),
            Some(Error::Denied),
            "{page}"
        );
        assert_eq!(
            v.find_passkeys("github.com", page, None, &[]).err(),
            Some(Error::Denied)
        );
        // Using the page's own rpId: the passkey is bound to github.com.
        let own = url::Url::parse(page)
            .unwrap()
            .host_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            v.passkey_assert(&id, &cred, &own, page, None, &[1]).err(),
            Some(Error::Denied)
        );
        assert!(v.find_passkeys(&own, page, None, &[]).unwrap().is_empty());
    }
    // A github.com frame inside evil.com.
    assert_eq!(
        v.passkey_assert(
            &id,
            &cred,
            "github.com",
            GH,
            Some("https://evil.com/"),
            &[1]
        )
        .err(),
        Some(Error::Denied)
    );
}

#[test]
fn attack_arbitrary_ids_are_denied() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (id, cred) = commit(&mut v, &mut rev, s);
    let other = v
        .stage_create(login("Bank", "a", "pw", "bank.example"), NOW)
        .unwrap();
    let bank = v.commit_write(other, rev.next()).unwrap().unwrap().id;
    assert_eq!(
        v.passkey_assert(&id, &[0; 16], "github.com", GH, None, &[1])
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.passkey_assert(&bank, &cred, "github.com", GH, None, &[1])
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.passkey_assert(&Uuid::new_v4(), &cred, "github.com", GH, None, &[1])
            .err(),
        Some(Error::NotFound)
    );
    // Attaching to a login that is not saved for the page.
    assert_eq!(
        v.stage_passkey_create(create_req("octo", &[2], Some(bank)), NOW)
            .err(),
        Some(Error::Denied)
    );
}

#[test]
fn attack_locked_vault_is_refused() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (id, cred) = commit(&mut v, &mut rev, s);
    v.lock();
    assert_eq!(
        v.find_passkeys("github.com", GH, None, &[]).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.passkey_assert(&id, &cred, "github.com", GH, None, &[1])
            .err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.check_passkey_create(&plain("octo", &[]), NOW).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.stage_passkey_create(create_req("octo", &[1], None), NOW)
            .err(),
        Some(Error::Locked)
    );
    assert_eq!(v.list_passkeys(&id).err(), Some(Error::Locked));
}

#[test]
fn attach_to_existing_login_and_keep_it_through_edits() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let staged = v
        .stage_create(login("GitHub", "octo", "pw", "github.com"), NOW)
        .unwrap();
    let gh = v.commit_write(staged, rev.next()).unwrap().unwrap().id;

    let check = v.check_passkey_create(&plain("OCTO", &[]), NOW).unwrap();
    assert!(!check.excluded);
    assert_eq!(check.candidates[0].id, gh);

    let s = v
        .stage_passkey_create(create_req("octo", &[1], Some(gh)), NOW)
        .unwrap();
    let (id, cred) = commit(&mut v, &mut rev, s);
    assert_eq!(id, gh);

    // A password edit from the desktop keeps the passkey.
    let edit = v
        .stage_update(
            &gh,
            login("GitHub", "octo", "new-pw", "github.com"),
            NOW + 1,
        )
        .unwrap();
    v.commit_write(edit, rev.next()).unwrap();
    assert!(v.get_item(&gh).unwrap().has_passkey);
    v.passkey_assert(&gh, &cred, "github.com", GH, None, &[1])
        .unwrap();

    // excludeCredentials naming it reports "excluded".
    assert!(
        v.check_passkey_create(&plain("octo", std::slice::from_ref(&cred)), NOW)
            .unwrap()
            .excluded
    );
    assert!(
        !v.check_passkey_create(&plain("octo", &[vec![0; 16]]), NOW)
            .unwrap()
            .excluded
    );
}

#[test]
fn two_accounts_and_re_registration() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let staged = v
        .stage_create(login("GitHub", "octo", "pw", "github.com"), NOW)
        .unwrap();
    let gh = v.commit_write(staged, rev.next()).unwrap().unwrap().id;
    let s = v
        .stage_passkey_create(create_req("octo", &[1], Some(gh)), NOW)
        .unwrap();
    let (_, first) = commit(&mut v, &mut rev, s);
    let s = v
        .stage_passkey_create(create_req("work", &[2], Some(gh)), NOW)
        .unwrap();
    let (_, second) = commit(&mut v, &mut rev, s);
    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 2);
    // allowCredentials filters.
    let only = v
        .find_passkeys("github.com", GH, None, std::slice::from_ref(&second))
        .unwrap();
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].credential_id, second);

    // Same user handle again replaces, it does not pile up.
    for _ in 0..(MAX_PASSKEYS_PER_LOGIN + 2) {
        let s = v
            .stage_passkey_create(create_req("octo", &[1], Some(gh)), NOW)
            .unwrap();
        commit(&mut v, &mut rev, s);
    }
    let infos = v.list_passkeys(&gh).unwrap();
    assert_eq!(infos.len(), 2);
    assert!(infos
        .iter()
        .all(|p| p.credential_id != havenkeys_core::passkey::encode_b64url(&first)));
}

#[test]
fn re_registering_with_no_item_id_finds_the_existing_holder() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);

    // First registration creates a new login (item_id: None).
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (first_login, _) = commit(&mut v, &mut rev, s);

    // Registering again for the same (rpId, user handle) with item_id:
    // None must find the existing holder rather than creating a second
    // login for the same account.
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW + 1)
        .unwrap();
    assert_eq!(s.item_id, first_login);
    commit(&mut v, &mut rev, s);

    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].item_id, first_login);
}

#[test]
fn re_registering_into_a_different_login_still_finds_the_existing_holder() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);

    // Login A gets the passkey via item_id: None.
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (login_a, _) = commit(&mut v, &mut rev, s);

    // A second, unrelated login is also saved for github.com.
    let staged = v
        .stage_create(login("GitHub", "other", "pw", "github.com"), NOW)
        .unwrap();
    let login_b = v.commit_write(staged, rev.next()).unwrap().unwrap().id;

    // Registering the same account again, naming login B this time, must
    // still land on login A (the existing holder), not create a second
    // passkey for the same account in login B.
    let s = v
        .stage_passkey_create(create_req("octo", &[1], Some(login_b)), NOW + 1)
        .unwrap();
    assert_eq!(s.item_id, login_a);
    commit(&mut v, &mut rev, s);

    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].item_id, login_a);
    assert!(!v.get_item(&login_b).unwrap().has_passkey);
}

#[test]
fn per_login_limit() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let staged = v
        .stage_create(login("GitHub", "octo", "pw", "github.com"), NOW)
        .unwrap();
    let gh = v.commit_write(staged, rev.next()).unwrap().unwrap().id;
    for i in 0..MAX_PASSKEYS_PER_LOGIN {
        let handle = [i as u8 + 1];
        let s = v
            .stage_passkey_create(create_req("u", &handle, Some(gh)), NOW)
            .unwrap();
        commit(&mut v, &mut rev, s);
    }
    assert!(matches!(
        v.stage_passkey_create(create_req("u", &[99], Some(gh)), NOW)
            .err(),
        Some(Error::InvalidInput(_))
    ));
    // A full login is not offered as a candidate.
    assert!(v
        .check_passkey_create(&plain("u", &[]), NOW)
        .unwrap()
        .candidates
        .is_empty());
}

#[test]
fn remove_passkey() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    let (id, cred) = commit(&mut v, &mut rev, s);
    let info = v.list_passkeys(&id).unwrap();
    assert_eq!(info[0].rp_id, "github.com");
    let w = v
        .stage_remove_passkey(&id, &info[0].credential_id, NOW + 1)
        .unwrap();
    v.commit_write(w, rev.next()).unwrap();
    assert!(!v.get_item(&id).unwrap().has_passkey);
    assert_eq!(
        v.passkey_assert(&id, &cred, "github.com", GH, None, &[1])
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_remove_passkey(&id, &info[0].credential_id, NOW)
            .err(),
        Some(Error::NotFound)
    );
    assert!(matches!(
        v.stage_remove_passkey(&id, "not base64!", NOW).err(),
        Some(Error::InvalidInput(_))
    ));
}

#[test]
fn inputs_are_bounded() {
    let (mut v, _) = activated_vault();
    let big = vec![1u8; 65];
    let mut r = create_req("octo", &big, None);
    assert!(matches!(
        v.stage_passkey_create(r, NOW).err(),
        Some(Error::InvalidInput(_))
    ));
    r = create_req("octo", &[], None);
    assert!(matches!(
        v.stage_passkey_create(r, NOW).err(),
        Some(Error::InvalidInput(_))
    ));
    let huge = vec![0u8; 1025];
    r = PasskeyCreate {
        challenge: &huge,
        ..create_req("octo", &[1], None)
    };
    assert!(matches!(
        v.stage_passkey_create(r, NOW).err(),
        Some(Error::InvalidInput(_))
    ));
    r = PasskeyCreate {
        challenge: &[],
        ..create_req("octo", &[1], None)
    };
    assert!(matches!(
        v.stage_passkey_create(r, NOW).err(),
        Some(Error::InvalidInput(_))
    ));
    assert!(matches!(
        v.stage_passkey_create(create_req("a\u{0}b", &[1], None), NOW)
            .err(),
        Some(Error::InvalidInput(_))
    ));
}

// ---------------------------------------------------------------- automatic upgrade

/// A GitHub login with a password, committed.
fn github_login(v: &mut VaultService, rev: &mut Rev, user: &str) -> Uuid {
    let s = v
        .stage_create(login("GitHub", user, "gh-pw", "https://github.com"), NOW)
        .unwrap();
    v.commit_write(s, rev.next()).unwrap().unwrap().id
}

fn query<'a>(page: &'a str, user: &'a str) -> CreateQuery<'a> {
    CreateQuery {
        rp_id: "github.com",
        page_url: page,
        top_url: None,
        user_name: user,
        exclude: &[],
        conditional: true,
    }
}

fn upgrade(v: &VaultService, page: &str, user: &str, now: i64) -> Upgrade {
    v.check_passkey_create(&query(page, user), now)
        .unwrap()
        .upgrade
}

fn set_auto(v: &mut VaultService, on: bool) {
    let s = v.settings().unwrap();
    v.update_settings(Settings {
        auto_passkey_upgrade: on,
        ..s
    })
    .unwrap();
}

#[test]
fn upgrade_is_auto_after_a_recent_fill_and_ask_when_the_setting_is_off() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW + 1000), Upgrade::Auto(gh));
    // Not conditional: never an upgrade.
    let q = CreateQuery {
        conditional: false,
        ..query(GH, "octo")
    };
    assert_eq!(
        v.check_passkey_create(&q, NOW).unwrap().upgrade,
        Upgrade::None
    );
    set_auto(&mut v, false);
    assert_eq!(upgrade(&v, GH, "octo", NOW + 1000), Upgrade::Ask(gh));
}

#[test]
fn upgrade_window_is_five_minutes_and_ignores_future_fills() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(
        upgrade(&v, GH, "octo", NOW + UPGRADE_WINDOW_MS),
        Upgrade::Auto(gh)
    );
    assert_eq!(
        upgrade(&v, GH, "octo", NOW + UPGRADE_WINDOW_MS + 1),
        Upgrade::None
    );
    // Clock moved back: a fill "from the future" is not recent.
    assert_eq!(upgrade(&v, GH, "octo", NOW - 1), Upgrade::None);
}

#[test]
fn upgrade_needs_the_same_account_name_folded_or_a_login_without_one() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "  OCTO ", NOW), Upgrade::Auto(gh));
    assert_eq!(upgrade(&v, GH, "someone-else", NOW), Upgrade::None);

    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let mut input = login("GitHub", "x", "pw", "https://github.com");
    input.username = None;
    let s = v.stage_create(input, NOW).unwrap();
    let anon = v.commit_write(s, rev.next()).unwrap().unwrap().id;
    v.fill_for_page(&anon, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "anyone", NOW), Upgrade::Auto(anon));
}

#[test]
fn upgrade_is_per_site_and_never_for_look_alikes() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    // Same registrable domain, login offered there (domain rule): allowed.
    let q = CreateQuery {
        rp_id: "gist.github.com",
        ..query("https://gist.github.com/", "octo")
    };
    assert_eq!(
        v.check_passkey_create(&q, NOW).unwrap().upgrade,
        Upgrade::Auto(gh)
    );
    // Look-alike: authorize_rp itself refuses.
    let evil = CreateQuery {
        rp_id: "github.com.evil.com",
        ..query("https://github.com.evil.com/", "octo")
    };
    assert!(!matches!(v.check_passkey_create(&evil, NOW), Ok(c) if c.upgrade != Upgrade::None));
}

#[test]
fn a_fill_without_a_password_is_not_remembered() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let mut input = login("GitHub", "octo", "x", "https://github.com");
    input.password = havenkeys_core::model::SecretUpdate::Keep;
    let s = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(s, rev.next()).unwrap().unwrap().id;
    v.fill_for_page(&id, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
}

#[test]
fn fill_memory_is_dropped_on_lock() {
    let (mut v, sk) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    v.lock();
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
}

#[test]
fn a_full_login_gets_no_upgrade() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    for i in 0..MAX_PASSKEYS_PER_LOGIN {
        let handle = [i as u8 + 10];
        let s = v
            .stage_passkey_create(create_req("octo", &handle, Some(gh)), NOW)
            .unwrap();
        commit(&mut v, &mut rev, s);
    }
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
}

#[test]
fn conditional_create_needs_auto_for_exactly_that_login() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    let other = github_login(&mut v, &mut rev, "work");
    let cond = |item| PasskeyCreate {
        conditional: true,
        ..create_req("octo", &[1], item)
    };
    // No fill yet.
    assert_eq!(
        v.stage_passkey_create(cond(Some(gh)), NOW).err(),
        Some(Error::Denied)
    );
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    // Another login, no login, too late.
    assert_eq!(
        v.stage_passkey_create(cond(Some(other)), NOW).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_passkey_create(cond(None), NOW).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_passkey_create(cond(Some(gh)), NOW + UPGRADE_WINDOW_MS + 1)
            .err(),
        Some(Error::Denied)
    );
    // Setting off: the extension must show the card (a non-conditional create).
    set_auto(&mut v, false);
    assert_eq!(
        v.stage_passkey_create(cond(Some(gh)), NOW).err(),
        Some(Error::Denied)
    );
    set_auto(&mut v, true);
    let s = v.stage_passkey_create(cond(Some(gh)), NOW).unwrap();
    assert_eq!(s.item_id, gh);
}

#[test]
fn conditional_create_never_lands_in_another_login_that_holds_the_account() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    let other = github_login(&mut v, &mut rev, "octo2");
    // `other` already holds the passkey for user handle [1].
    let s = v
        .stage_passkey_create(create_req("octo", &[1], Some(other)), NOW)
        .unwrap();
    commit(&mut v, &mut rev, s);
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    let cond = PasskeyCreate {
        conditional: true,
        ..create_req("octo", &[1], Some(gh))
    };
    assert_eq!(v.stage_passkey_create(cond, NOW).err(), Some(Error::Denied));
}

#[test]
fn passkey_status_sees_only_passkeys_the_page_may_use() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    assert!(!v.has_passkey_for_page(GH, None).unwrap());
    let s = v
        .stage_passkey_create(create_req("octo", &[1], None), NOW)
        .unwrap();
    commit(&mut v, &mut rev, s);
    assert!(v.has_passkey_for_page(GH, None).unwrap());
    assert!(v
        .has_passkey_for_page("https://gist.github.com/", None)
        .unwrap());
    assert!(!v.has_passkey_for_page("https://gitlab.com/", None).unwrap());
    assert!(!v
        .has_passkey_for_page("https://github.com.evil.com/", None)
        .unwrap());
    assert!(!v.has_passkey_for_page("http://github.com/", None).unwrap());
    // A github.com frame inside another site.
    assert!(!v
        .has_passkey_for_page(GH, Some("https://evil.com/"))
        .unwrap());
    v.lock();
    assert_eq!(v.has_passkey_for_page(GH, None).err(), Some(Error::Locked));
}

#[test]
fn conditional_create_never_replaces_the_filled_logins_own_passkey() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    // `gh` itself already holds the passkey for user handle [1].
    let s = v
        .stage_passkey_create(create_req("octo", &[1], Some(gh)), NOW)
        .unwrap();
    let (_, old_cred) = commit(&mut v, &mut rev, s);
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    let cond = PasskeyCreate {
        conditional: true,
        ..create_req("octo", &[1], Some(gh))
    };
    assert_eq!(v.stage_passkey_create(cond, NOW).err(), Some(Error::Denied));
    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].item_id, gh);
    assert_eq!(found[0].credential_id, old_cred);
}

#[test]
fn one_fill_grants_one_silent_passkey() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    let cond = |handle| PasskeyCreate {
        conditional: true,
        ..create_req("octo", handle, Some(gh))
    };
    let s = v.stage_passkey_create(cond(&[1]), NOW).unwrap();
    commit(&mut v, &mut rev, s);
    // The fill is spent: another handle needs another fill (or the card).
    assert_eq!(
        v.stage_passkey_create(cond(&[2]), NOW).err(),
        Some(Error::Denied)
    );
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert!(v.stage_passkey_create(cond(&[2]), NOW).is_ok());
}

#[test]
fn a_clicked_create_does_not_spend_the_fill() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    let s = v
        .stage_passkey_create(create_req("octo", &[1], Some(gh)), NOW)
        .unwrap();
    commit(&mut v, &mut rev, s);
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::Auto(gh));
}

#[test]
fn only_the_newest_recent_fills_are_kept() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let ids: Vec<Uuid> = (0..=MAX_RECENT_FILLS)
        .map(|i| github_login(&mut v, &mut rev, &format!("u{i}")))
        .collect();
    for id in &ids {
        v.fill_for_page(id, GH, None, NOW).unwrap();
    }
    // The first fill was evicted; the last is still there.
    assert_eq!(upgrade(&v, GH, "u0", NOW), Upgrade::None);
    let last = format!("u{MAX_RECENT_FILLS}");
    assert_eq!(
        upgrade(&v, GH, &last, NOW),
        Upgrade::Auto(ids[MAX_RECENT_FILLS])
    );
}

#[test]
fn a_fill_on_one_site_is_no_upgrade_on_another_site_of_the_same_login() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let mut input = login("Git", "octo", "pw", "https://github.com");
    input.urls.push(UrlRule {
        url: "https://gitlab.com".into(),
        match_type: MatchType::Domain,
    });
    let s = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(s, rev.next()).unwrap().unwrap().id;
    v.fill_for_page(&id, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::Auto(id));
    let q = CreateQuery {
        rp_id: "gitlab.com",
        ..query("https://gitlab.com/", "octo")
    };
    assert_eq!(
        v.check_passkey_create(&q, NOW).unwrap().upgrade,
        Upgrade::None
    );
}
