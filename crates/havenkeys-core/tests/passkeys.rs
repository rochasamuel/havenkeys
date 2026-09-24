//! Passkeys through the vault: origin binding, lifecycle and the security
//! regressions in docs/threat-model.md §5 (passkey variants).

mod common;

use common::*;
use havenkeys_core::passkey::{PasskeyCreate, StagedPasskey, MAX_PASSKEYS_PER_LOGIN};
use havenkeys_core::vault::VaultService;
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
        v.check_passkey_create("github.com", GH, None, "octo", &[])
            .err(),
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

    let check = v
        .check_passkey_create("github.com", GH, None, "OCTO", &[])
        .unwrap();
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
        v.check_passkey_create("github.com", GH, None, "octo", std::slice::from_ref(&cred))
            .unwrap()
            .excluded
    );
    assert!(
        !v.check_passkey_create("github.com", GH, None, "octo", &[vec![0; 16]])
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
        .check_passkey_create("github.com", GH, None, "u", &[])
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
    let (v, _) = activated_vault();
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
