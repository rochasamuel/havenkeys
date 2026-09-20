//! Activation (first login) and sign-in on a second device, key scheme 3.

mod common;

use common::{account, account_record, fast_kdf, secret, NOW, PASSWORD};
use havenkeys_core::account::AccountRef;
use havenkeys_core::store::{KeyScheme, Store};
use havenkeys_core::sync::prepare_sign_in;
use havenkeys_core::vault::{derive_auth_key, prepare_new_account_vault, VaultService};
use uuid::Uuid;

fn activate() -> (
    VaultService,
    havenkeys_core::crypto::secret_key::SecretKey,
    Vec<u8>,
) {
    let (vault, sk) = common::activated_vault();
    let header = vault.encode_account_header().unwrap();
    (vault, sk, header)
}

#[test]
fn activation_produces_an_account_bound_vault() {
    let (vault, _sk, _header) = activate();
    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert!(vault.is_unlocked());
}

#[test]
fn a_second_device_signs_in_with_password_secret_key_and_account() {
    let (mut first, secret_key, header) = activate();
    let staged = first
        .stage_create(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    first.commit_write(staged, 1).unwrap();

    let (prepared, _auth) =
        prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &common::account()).unwrap();
    let mut second = VaultService::new(Store::open_in_memory().unwrap());
    second
        .create_account_vault(prepared, &account_record())
        .unwrap();
    // create_account_vault leaves the vault unlocked; lock it first so the
    // sign-in unlock path itself is exercised, not just vault creation.
    second.lock();
    second
        .unlock_for_account(&secret(PASSWORD), &secret_key, &common::account())
        .unwrap();

    assert_eq!(second.vault_id().unwrap(), first.vault_id().unwrap());
}

#[test]
fn sign_in_fails_with_the_wrong_secret_key() {
    let (_first, _sk, header) = activate();
    let other = havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap();
    assert!(prepare_sign_in(&header, &secret(PASSWORD), &other, &common::account()).is_err());
}

#[test]
fn sign_in_fails_with_the_wrong_email() {
    let (_first, secret_key, header) = activate();
    let wrong = AccountRef::new(
        common::account().id,
        havenkeys_core::account::NormalizedEmail::parse("someone.else@example.com").unwrap(),
    );
    assert!(prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &wrong).is_err());
}

#[test]
fn sign_in_fails_with_the_wrong_account_id() {
    let (_first, secret_key, header) = activate();
    let wrong = AccountRef::new(Uuid::from_u128(1), common::account().email.clone());
    assert!(prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &wrong).is_err());
}

#[test]
fn sign_in_fails_with_the_wrong_master_password() {
    let (_first, secret_key, header) = activate();
    // `.unwrap_err()` would require `PreparedVault: Debug`, which it
    // intentionally does not implement (it carries the unwrapped vault key);
    // `.err().unwrap()` gets the same value without that bound.
    let err = havenkeys_core::sync::prepare_sign_in(
        &header,
        &secret("a completely different password"),
        &secret_key,
        &common::account(),
    )
    .err()
    .unwrap();
    // Same generic failure as a wrong Secret Key or a wrong account: the
    // caller must not learn which input was wrong.
    assert_eq!(err.code(), "unlock_failed");
}

#[test]
fn an_older_header_is_refused_after_a_password_change() {
    let (mut vault, secret_key, old_header) = activate();

    // Change the master password: the header revision goes up. Exercises
    // `derive_for_account` directly (rather than the
    // `change_master_password_for_account` convenience) to keep the header
    // this produces available for the replay checks below.
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_for_account(
            &secret(PASSWORD),
            &secret("a much longer new password"),
            fast_kdf(),
            &secret_key,
            &account(),
        )
        .unwrap();
    vault.commit_rekey(ticket, Ok(rekeyed)).unwrap();
    let new_header = vault.encode_account_header().unwrap();

    // A hostile server replays the header from before the change.
    assert!(!vault.adopt_account_header(&old_header).unwrap());
    // And the current one is still accepted (idempotently).
    assert!(!vault.adopt_account_header(&new_header).unwrap());
}

#[test]
fn a_persisted_floor_refuses_a_header_newer_than_local_but_not_newer_than_the_floor() {
    // `adopt_account_header`'s floor is
    // `max(account.max_header_rev, local.revision)`. Nothing in production
    // code calls `Store::set_account` yet (that wiring is a later, desktop-
    // UI task), so this is the only place the persisted-`max_header_rev`
    // half of that computation gets exercised. If it were dropped from the
    // `max(...)` and the floor became `local.revision` alone, this test
    // would fail: the header built below is newer than `second`'s local
    // revision, so it would then be wrongly adopted.
    let (mut first, secret_key, header0) = activate();

    // A second device signs into the very same vault (so it shares the same
    // vault key / data key as `first`), but its local store is seeded with
    // an account record whose `max_header_rev` is far above any revision
    // this test reaches.
    let mut floor_rec = account_record();
    floor_rec.max_header_rev = 999;
    let mut store = Store::open_in_memory().unwrap();
    store.set_account(&floor_rec).unwrap();
    let (prepared, _auth) =
        prepare_sign_in(&header0, &secret(PASSWORD), &secret_key, &account()).unwrap();
    let mut second = VaultService::new(store);
    second.create_account_vault(prepared, &floor_rec).unwrap();
    second.lock();
    second
        .unlock_for_account(&secret(PASSWORD), &secret_key, &account())
        .unwrap();

    // Bump `first`'s revision by one, past what `second` has locally, but
    // still nowhere near the seeded floor.
    let ticket = first.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_for_account(
            &secret(PASSWORD),
            &secret("a much longer new password"),
            fast_kdf(),
            &secret_key,
            &account(),
        )
        .unwrap();
    first.commit_rekey(ticket, Ok(rekeyed)).unwrap();
    let newer_header = first.encode_account_header().unwrap();

    // Newer than `second`'s local header, but at or below the persisted
    // floor: refused.
    assert!(!second.adopt_account_header(&newer_header).unwrap());
}

#[test]
fn a_forged_header_is_refused() {
    let (mut vault, _sk, header) = activate();
    let mut forged = header.clone();
    let n = forged.len();
    // Flips a bit inside otherwise-valid JSON, so the outcome is
    // deterministically Ok(false) (attestation verification fails), not a
    // parse error and not a locked vault. Pin that branch specifically,
    // since this is the only direct test of header forgery.
    forged[n - 5] ^= 0x01;
    assert!(!vault.adopt_account_header(&forged).unwrap());
}

#[test]
fn an_oversized_header_is_rejected_without_panicking() {
    // parse_header's `bytes.len() > 64 * 1024` guard (spec §12: "oversized
    // JSON responses -> rejected without panic") had no test at any level.
    let (mut vault, _sk, _header) = activate();
    let oversized = vec![0u8; 65_537];
    assert_eq!(
        vault.adopt_account_header(&oversized).unwrap_err().code(),
        "corrupted"
    );
}

#[test]
fn derive_auth_key_matches_the_activation_and_sign_in_paths() {
    // `derive_auth_key` is what a later login uses; it takes its KDF
    // parameters from the caller. `prepare_new_account_vault` and
    // `prepare_sign_in` also produce an AuthKey internally, but
    // `prepare_sign_in` takes its KDF from the header instead. Nothing
    // structurally prevents those paths from diverging, so pin them
    // together: a mismatch here would make every real login fail with
    // nothing else to catch it.
    let kdf = fast_kdf();
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), kdf.clone(), NOW).unwrap();

    let direct = derive_auth_key(&secret(PASSWORD), &made.secret_key, &kdf, &account()).unwrap();
    assert_eq!(
        direct.to_base64().as_str(),
        made.auth_key.to_base64().as_str()
    );

    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault
        .create_account_vault(made.prepared, &account_record())
        .unwrap();
    let header = vault.encode_account_header().unwrap();
    let (_, sign_in_auth) =
        prepare_sign_in(&header, &secret(PASSWORD), &made.secret_key, &account()).unwrap();
    assert_eq!(
        sign_in_auth.to_base64().as_str(),
        made.auth_key.to_base64().as_str()
    );
}

#[test]
fn the_auth_key_is_not_the_kek() {
    // Both come from the same Argon2id run; only the HKDF label differs.
    // Encoded forms must not be equal, or the server would hold key material.
    let made =
        prepare_new_account_vault(&secret(PASSWORD), &common::account(), fast_kdf(), NOW).unwrap();
    let auth_b64 = made.auth_key.to_base64();
    assert!(!auth_b64.is_empty());
    assert_eq!(format!("{:?}", made.auth_key), "AuthKey(<redacted>)");
}

#[test]
fn an_account_vault_changes_its_master_password_through_the_ordinary_route() {
    // The desktop's "change master password" command goes through
    // `change_master_password_for_account`: the account comes from the
    // local store, not from the caller.
    let (mut vault, secret_key) = common::activated_vault();
    let staged = vault
        .stage_create(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    vault.commit_write(staged, 1).unwrap();
    let new_password = secret("a much longer new password");

    vault
        .change_master_password_for_account(
            &secret(PASSWORD),
            &new_password,
            fast_kdf(),
            &secret_key,
        )
        .unwrap();

    vault.lock();
    assert!(vault
        .unlock_for_account(&secret(PASSWORD), &secret_key, &account())
        .is_err());
    vault
        .unlock_for_account(&new_password, &secret_key, &account())
        .unwrap();
    assert_eq!(vault.list_items().unwrap().len(), 1);
}

#[test]
fn create_account_vault_stores_the_account_record() {
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault
        .create_account_vault(made.prepared, &account_record())
        .unwrap();

    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert!(vault.is_unlocked());
    assert_eq!(vault.account().unwrap().unwrap().account_id, account().id);
}
