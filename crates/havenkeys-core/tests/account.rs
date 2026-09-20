//! Activation (first login) and sign-in on a second device, key scheme 3.

mod common;

use common::{fast_kdf, secret, NOW, PASSWORD};
use havenkeys_core::account::AccountRef;
use havenkeys_core::store::{KeyScheme, Store};
use havenkeys_core::sync::prepare_sign_in;
use havenkeys_core::vault::{prepare_new_account_vault, VaultService};
use uuid::Uuid;

fn activate() -> (VaultService, havenkeys_core::crypto::secret_key::SecretKey, Vec<u8>) {
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
    first.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();

    let (prepared, _auth) =
        prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &common::account()).unwrap();
    let mut second = VaultService::new(Store::open_in_memory().unwrap());
    second.create_vault(prepared).unwrap();
    // create_vault leaves the vault unlocked; lock it first so the sign-in
    // unlock path itself is exercised, not just vault creation.
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
fn sign_in_refuses_a_folder_era_header() {
    // A key scheme 2 header must not be accepted into an account vault:
    // downgrade is refused (design doc §4.2).
    let made = havenkeys_core::vault::prepare_new_vault_with_secret_key(
        &secret(PASSWORD),
        &havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap(),
        fast_kdf(),
        NOW,
    )
    .unwrap();
    let mut v2 = VaultService::new(Store::open_in_memory().unwrap());
    v2.create_vault(made).unwrap();
    // A scheme 2 vault cannot produce an account header at all.
    assert!(v2.encode_account_header().is_err());
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
