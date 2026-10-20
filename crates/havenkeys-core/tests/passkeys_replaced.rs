//! Re-registering a passkey keeps the old one until the site shows it knows
//! the new one (security-review PK5): a site that rejects the new passkey
//! must not leave the user without a working one.

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
        conditional: false,
    }
}

fn commit(v: &mut VaultService, rev: &mut i64, s: StagedPasskey) -> (Uuid, Vec<u8>) {
    let id = s.item_id;
    let cred = s.registration.credential_id.clone();
    *rev += 1;
    v.commit_write(s.write, *rev).unwrap();
    (id, cred)
}

fn register(v: &mut VaultService, rev: &mut i64, user: &str, handle: &[u8]) -> (Uuid, Vec<u8>) {
    let s = v
        .stage_passkey_create(create_req(user, handle, None), NOW)
        .unwrap();
    commit(v, rev, s)
}

/// Drop the replaced passkeys the site's `allowCredentials` shows it no
/// longer needs, committing the write if there is one.
fn tidy(v: &mut VaultService, rev: &mut i64, page: &str, allow: &[Vec<u8>]) -> bool {
    match v.stage_drop_replaced_passkeys("github.com", page, None, allow, NOW + 10) {
        Ok(Some(w)) => {
            *rev += 1;
            v.commit_write(w, *rev).unwrap();
            true
        }
        Ok(None) => false,
        Err(e) => panic!("unexpected {e:?}"),
    }
}

fn signs_in(v: &VaultService, id: &Uuid, cred: &[u8]) -> bool {
    v.passkey_assert(id, cred, "github.com", GH, None, &[3; 32])
        .is_ok()
}

#[test]
fn re_registering_keeps_the_old_passkey_marked_replaced() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    let (id, old) = register(&mut v, &mut rev, "octo", &[1]);
    let (same, new) = register(&mut v, &mut rev, "octo", &[1]);
    assert_eq!(same, id);

    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].credential_id, new);
    assert!(!found[0].replaced);
    assert_eq!(found[1].credential_id, old);
    assert!(found[1].replaced);
    // The old one still signs in, in case the site never took the new one.
    assert!(signs_in(&v, &id, &old));
    assert!(signs_in(&v, &id, &new));

    let infos = v.list_passkeys(&id).unwrap();
    let old_info = infos
        .iter()
        .find(|p| p.credential_id == havenkeys_core::passkey::encode_b64url(&old))
        .unwrap();
    assert!(old_info.replaced);
}

#[test]
fn only_the_newest_old_passkey_is_kept() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    let (id, first) = register(&mut v, &mut rev, "octo", &[1]);
    let (_, second) = register(&mut v, &mut rev, "octo", &[1]);
    let (_, third) = register(&mut v, &mut rev, "octo", &[1]);
    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    let creds: Vec<_> = found.iter().map(|m| m.credential_id.clone()).collect();
    assert_eq!(creds, vec![third, second]);
    assert!(!signs_in(&v, &id, &first));
}

#[test]
fn the_site_naming_the_new_passkey_drops_the_old_one() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    let (id, old) = register(&mut v, &mut rev, "octo", &[1]);
    let (_, new) = register(&mut v, &mut rev, "octo", &[1]);

    assert!(tidy(&mut v, &mut rev, GH, std::slice::from_ref(&new)));
    let found = v.find_passkeys("github.com", GH, None, &[]).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].credential_id, new);
    assert!(!found[0].replaced);
    assert!(!signs_in(&v, &id, &old));
    // Nothing left to drop.
    assert!(!tidy(&mut v, &mut rev, GH, std::slice::from_ref(&new)));
}

#[test]
fn nothing_is_dropped_without_proof_the_site_knows_the_new_one() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    let (id, old) = register(&mut v, &mut rev, "octo", &[1]);
    register(&mut v, &mut rev, "octo", &[1]);

    // A discoverable request (no list), or one naming only the old one.
    assert!(!tidy(&mut v, &mut rev, GH, &[]));
    assert!(!tidy(&mut v, &mut rev, GH, std::slice::from_ref(&old)));
    assert!(!tidy(&mut v, &mut rev, GH, &[vec![9; 16]]));
    assert!(signs_in(&v, &id, &old));
}

#[test]
fn another_account_or_site_changes_nothing() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    let (id, old) = register(&mut v, &mut rev, "octo", &[1]);
    register(&mut v, &mut rev, "octo", &[1]);
    // A second account's current passkey proves nothing about octo's.
    let (_, work) = register(&mut v, &mut rev, "work", &[2]);
    assert!(!tidy(&mut v, &mut rev, GH, std::slice::from_ref(&work)));
    assert!(signs_in(&v, &id, &old));
}

#[test]
fn a_page_that_may_not_use_the_rp_id_is_refused() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    register(&mut v, &mut rev, "octo", &[1]);
    let (_, new) = register(&mut v, &mut rev, "octo", &[1]);
    assert_eq!(
        v.stage_drop_replaced_passkeys(
            "github.com",
            "https://evil.com/",
            None,
            std::slice::from_ref(&new),
            NOW
        )
        .err(),
        Some(Error::Denied)
    );
}

#[test]
fn a_locked_vault_is_refused() {
    let (mut v, _) = activated_vault();
    v.lock();
    assert_eq!(
        v.stage_drop_replaced_passkeys("github.com", GH, None, &[], NOW)
            .err(),
        Some(Error::Locked)
    );
}

#[test]
fn a_full_login_replaces_outright_as_before() {
    let (mut v, _) = activated_vault();
    let mut rev = 0;
    let (id, old) = register(&mut v, &mut rev, "u", &[1]);
    for i in 1..MAX_PASSKEYS_PER_LOGIN {
        let handle = [i as u8 + 1];
        let s = v
            .stage_passkey_create(create_req("u", &handle, Some(id)), NOW)
            .unwrap();
        commit(&mut v, &mut rev, s);
    }
    // No room to keep the old one: it goes, and the new one still saves.
    let (_, new) = register(&mut v, &mut rev, "u", &[1]);
    assert!(signs_in(&v, &id, &new));
    assert!(!signs_in(&v, &id, &old));
    assert_eq!(v.list_passkeys(&id).unwrap().len(), MAX_PASSKEYS_PER_LOGIN);
}
