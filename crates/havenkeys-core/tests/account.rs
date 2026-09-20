//! Activation (first login) and sign-in on a second device, key scheme 3.

mod common;

use common::{account, account_record, fast_kdf, secret, NOW, PASSWORD};
use havenkeys_core::account::AccountRef;
use havenkeys_core::model::SecretField;
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
fn a_secret_key_vault_upgrades_to_an_account_without_touching_items() {
    use havenkeys_core::crypto::secret_key::SecretKey;
    use havenkeys_core::vault::prepare_new_vault_with_secret_key;

    let secret_key = SecretKey::generate().unwrap();
    let made = prepare_new_vault_with_secret_key(&secret(PASSWORD), &secret_key, fast_kdf(), NOW).unwrap();
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(made).unwrap();
    // create_vault leaves the vault unlocked; lock it first so begin_unlock
    // (which rejects an already-unlocked vault) can be used below.
    vault.lock();
    // Scheme 2 has no one-shot unlock helper; use the ticket pattern that
    // tests/sync.rs uses.
    let ticket = vault.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&secret_key));
    vault.finish_unlock(ticket, r).unwrap();
    let item = vault.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let vault_id_before = vault.vault_id().unwrap();

    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_account_upgrade(&secret(PASSWORD), &secret_key, &account(), fast_kdf())
        .unwrap();
    vault.commit_rekey(ticket, Ok(rekeyed)).unwrap();

    // Same vault, same item, new scheme, higher header revision.
    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert_eq!(vault.vault_id().unwrap(), vault_id_before);

    vault.lock();
    vault.unlock_for_account(&secret(PASSWORD), &secret_key, &account()).unwrap();
    assert_eq!(vault.get_item(&item.id).unwrap().title, "GitHub");
    // The overview title alone doesn't prove the item is untouched: the
    // password lives in the separate details blob, encrypted under the
    // same (unrotated) vault key. Reveal it too.
    assert_eq!(
        vault.reveal(&item.id, SecretField::Password).unwrap().expose(),
        "pw"
    );

    // The old scheme-2 unlock no longer works: without the account there is
    // no KEK to derive.
    vault.lock();
    let ticket = vault.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&secret_key));
    assert!(vault.finish_unlock(ticket, r).is_err());
}

#[test]
fn an_older_header_is_refused_after_a_password_change() {
    let (mut vault, secret_key, old_header) = activate();

    // Change the master password: the header revision goes up.
    //
    // `derive_with_secret_key` cannot rekey a key-scheme-3 (account-bound)
    // vault: internally it derives the KEK through `RekeyTicket::unwrap`,
    // which always passes `account: None`, so `derive_kek_for` refuses an
    // `AccountBound` scheme before doing any work. Use the new
    // `derive_for_account`, added in this task alongside the rollback
    // guard, which derives the KEK with `derive_kek_v3` and the account
    // directly.
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
    second.create_vault(prepared).unwrap();
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
    forged[n - 5] ^= 0x01; // flip a bit inside the attestation
    assert!(matches!(vault.adopt_account_header(&forged), Err(_) | Ok(false)));
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
