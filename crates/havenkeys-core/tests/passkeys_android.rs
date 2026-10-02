//! Passkeys for Android apps (spec 2026-10-01-android-app §8.1): an app may
//! use an RP ID only when the site's Digital Asset Links file grants it
//! `common.get_login_creds`; its origin is `android:apk-key-hash:<cert>`.

mod common;

use common::*;
use havenkeys_core::app_target::AppIdentity;
use havenkeys_core::asset_links::{AppStatement, AssetLinksCache, FRESH_MS};
use havenkeys_core::local::LocalSlot;
use havenkeys_core::model::MatchType;
use havenkeys_core::passkey::{
    encode_b64url, AppCreateQuery, AppPasskeyCreate, PasskeyCreate, StagedPasskey,
};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const CERT: [u8; 32] = [0xab; 32];

fn github_app() -> AppIdentity {
    AppIdentity::new("com.github.android", &[CERT.to_vec()]).unwrap()
}

fn impostor() -> AppIdentity {
    AppIdentity::new("com.github.android", &[vec![0xcd; 32]]).unwrap()
}

fn vouch(v: &VaultService, host: &str, statement: Option<bool>, checked_at: i64) {
    let mut cache: AssetLinksCache = v
        .read_local(LocalSlot::AssetLinks)
        .unwrap()
        .unwrap_or_default();
    let statements = statement.map(|login_creds| {
        vec![AppStatement {
            package: "com.github.android".into(),
            certs: vec!["ab".repeat(32)],
            login_creds,
        }]
    });
    cache.record(host, checked_at, statements);
    v.write_local(LocalSlot::AssetLinks, &cache).unwrap();
}

fn hosts() -> Vec<String> {
    vec!["github.com".into()]
}

fn create<'a>(
    app: &'a AppIdentity,
    hosts: &'a [String],
    user: &'a str,
    handle: &'a [u8],
    item: Option<Uuid>,
) -> AppPasskeyCreate<'a> {
    AppPasskeyCreate {
        rp_id: "github.com",
        app,
        verified_hosts: hosts,
        challenge: &[7; 32],
        user_handle: handle,
        user_name: user,
        display_name: None,
        item_id: item,
    }
}

fn commit(v: &mut VaultService, s: StagedPasskey, rev: i64) -> (Uuid, Vec<u8>, Vec<u8>) {
    let out = (
        s.item_id,
        s.registration.credential_id.clone(),
        s.registration.public_key.clone(),
    );
    v.commit_write(s.write, rev).unwrap();
    out
}

fn verify(
    spki: &[u8],
    authenticator_data: &[u8],
    client_data_hash: &[u8],
    signature: &[u8],
) -> bool {
    let mut signed = authenticator_data.to_vec();
    signed.extend_from_slice(client_data_hash);
    VerifyingKey::from_public_key_der(spki)
        .unwrap()
        .verify(&signed, &Signature::from_der(signature).unwrap())
        .is_ok()
}

#[test]
fn a_vouched_app_creates_and_signs_in_with_its_own_origin() {
    let (mut v, _) = activated_vault();
    vouch(&v, "github.com", Some(true), NOW);
    let app = github_app();
    let hosts = hosts();
    let s = v
        .stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], None), NOW)
        .unwrap();
    let cdj = String::from_utf8(s.registration.client_data_json.clone()).unwrap();
    let origin = format!("android:apk-key-hash:{}", encode_b64url(&CERT));
    assert!(cdj.contains(&format!(r#""origin":"{origin}""#)), "{cdj}");
    let (id, cred, spki) = commit(&mut v, s, 1);

    let item = v.get_item(&id).unwrap();
    assert_eq!(item.title, "github.com");
    assert_eq!(item.urls[0].url, "https://github.com/");
    assert_eq!(item.urls[0].match_type, MatchType::Domain);

    let found = v
        .find_passkeys_for_app("github.com", &app, &[], NOW)
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].credential_id, cred);

    let a = v
        .passkey_assert_for_app(&id, &cred, "github.com", &app, &[3; 32], NOW)
        .unwrap();
    assert!(verify(
        &spki,
        &a.authenticator_data,
        &Sha256::digest(&a.client_data_json),
        &a.signature
    ));
    assert!(String::from_utf8(a.client_data_json)
        .unwrap()
        .contains(&origin));
}

#[test]
fn a_passkey_made_in_the_app_works_on_the_website_and_back() {
    let (mut v, _) = activated_vault();
    vouch(&v, "github.com", Some(true), NOW);
    let app = github_app();
    let hosts = hosts();
    let s = v
        .stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], None), NOW)
        .unwrap();
    let (id, cred, _) = commit(&mut v, s, 1);
    assert_eq!(
        v.find_passkeys("github.com", "https://github.com/", None, &[])
            .unwrap()
            .len(),
        1
    );
    v.passkey_assert(
        &id,
        &cred,
        "github.com",
        "https://github.com/",
        None,
        &[3; 32],
    )
    .unwrap();

    let web = v
        .stage_passkey_create(
            PasskeyCreate {
                rp_id: "github.com",
                page_url: "https://github.com/",
                top_url: None,
                challenge: &[7; 32],
                user_handle: &[2],
                user_name: "hubot",
                display_name: None,
                item_id: None,
                conditional: false,
            },
            NOW,
        )
        .unwrap();
    commit(&mut v, web, 2);
    assert_eq!(
        v.find_passkeys_for_app("github.com", &app, &[], NOW)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn attack_an_app_the_site_does_not_vouch_for_gets_nothing() {
    let (mut v, _) = activated_vault();
    vouch(&v, "github.com", Some(true), NOW);
    let app = github_app();
    let hosts = hosts();
    let s = v
        .stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], None), NOW)
        .unwrap();
    let (id, cred, _) = commit(&mut v, s, 1);

    let bad = impostor();
    assert_eq!(
        v.find_passkeys_for_app("github.com", &bad, &[], NOW).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.passkey_assert_for_app(&id, &cred, "github.com", &bad, &[3; 32], NOW)
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_passkey_create_for_app(create(&bad, &hosts, "octo", &[9], None), NOW)
            .err(),
        Some(Error::Denied)
    );
    let q = AppCreateQuery {
        rp_id: "github.com",
        app: &bad,
        verified_hosts: &hosts,
        user_name: "octo",
        exclude: &[],
    };
    assert_eq!(
        v.check_passkey_create_for_app(&q, NOW).err(),
        Some(Error::Denied)
    );
}

#[test]
fn a_stale_failed_or_fill_only_lookup_vouches_for_nothing() {
    let app = github_app();
    let hosts = hosts();
    for (statement, checked_at) in [
        (Some(false), NOW),               // handle_all_urls only
        (None, NOW),                      // the fetch failed
        (Some(true), NOW - FRESH_MS - 1), // older than 7 days
    ] {
        let (mut v, _) = activated_vault();
        vouch(&v, "github.com", statement, checked_at);
        assert_eq!(
            v.stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], None), NOW)
                .err(),
            Some(Error::Denied),
            "{statement:?} at {checked_at}"
        );
    }
}

#[test]
fn a_site_vouches_only_for_its_own_rp_id() {
    let (mut v, _) = activated_vault();
    vouch(&v, "evil.com", Some(true), NOW);
    let app = github_app();
    let hosts: Vec<String> = vec!["evil.com".into()];
    assert_eq!(
        v.stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], None), NOW)
            .err(),
        Some(Error::Denied)
    );
    // A passkey for github.com saved from the website is not offered to an
    // app that only evil.com vouches for.
    let web = v
        .stage_passkey_create(
            PasskeyCreate {
                rp_id: "github.com",
                page_url: "https://github.com/",
                top_url: None,
                challenge: &[7; 32],
                user_handle: &[2],
                user_name: "octo",
                display_name: None,
                item_id: None,
                conditional: false,
            },
            NOW,
        )
        .unwrap();
    commit(&mut v, web, 1);
    assert!(v
        .find_passkeys_for_app("evil.com", &app, &[], NOW)
        .unwrap()
        .is_empty());
    assert_eq!(
        v.find_passkeys_for_app("github.com", &app, &[], NOW).err(),
        Some(Error::Denied)
    );
}

#[test]
fn a_chosen_login_must_be_matched_to_the_app() {
    let (mut v, _) = activated_vault();
    vouch(&v, "github.com", Some(true), NOW);
    let app = github_app();
    let hosts = hosts();
    let github = v
        .stage_create(
            login("GitHub", "octo", "pw-123456", "https://github.com"),
            NOW,
        )
        .unwrap();
    let github_id = github.item_id;
    v.commit_write(github, 1).unwrap();
    let other = v
        .stage_create(
            login("Example", "octo", "pw-123456", "https://example.com"),
            NOW,
        )
        .unwrap();
    let other_id = other.item_id;
    v.commit_write(other, 2).unwrap();

    let q = AppCreateQuery {
        rp_id: "github.com",
        app: &app,
        verified_hosts: &hosts,
        user_name: "octo",
        exclude: &[],
    };
    let check = v.check_passkey_create_for_app(&q, NOW).unwrap();
    assert!(!check.excluded);
    assert_eq!(
        check.candidates.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![github_id]
    );

    assert_eq!(
        v.stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], Some(other_id)), NOW)
            .err(),
        Some(Error::Denied)
    );
    let s = v
        .stage_passkey_create_for_app(create(&app, &hosts, "octo", &[1], Some(github_id)), NOW)
        .unwrap();
    let (id, cred, _) = commit(&mut v, s, 3);
    assert_eq!(id, github_id);

    let excluded = [cred];
    let q = AppCreateQuery {
        exclude: &excluded,
        ..q
    };
    assert!(v.check_passkey_create_for_app(&q, NOW).unwrap().excluded);
}

#[test]
fn a_locked_vault_is_refused_first() {
    let (mut v, _) = activated_vault();
    vouch(&v, "github.com", Some(true), NOW);
    v.lock();
    let app = github_app();
    assert_eq!(
        v.find_passkeys_for_app("github.com", &app, &[], NOW).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.passkey_assert_for_app(&Uuid::new_v4(), &[1; 16], "github.com", &app, &[3; 32], NOW)
            .err(),
        Some(Error::Locked)
    );
}
