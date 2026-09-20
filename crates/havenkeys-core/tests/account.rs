//! Activation (first login) and sign-in on a second device, key scheme 3.

mod common;

use common::{account, account_record, fast_kdf, secret, NOW, PASSWORD};
use havenkeys_core::account::AccountRef;
use havenkeys_core::model::SecretField;
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
    first
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();

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
    let made =
        prepare_new_vault_with_secret_key(&secret(PASSWORD), &secret_key, fast_kdf(), NOW).unwrap();
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
    let item = vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let vault_id_before = vault.vault_id().unwrap();

    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_account_upgrade(&secret(PASSWORD), &secret_key, &account(), fast_kdf())
        .unwrap();
    vault
        .commit_account_upgrade(ticket, Ok(rekeyed), &account_record())
        .unwrap();

    // Same vault, same item, new scheme, higher header revision.
    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert_eq!(vault.vault_id().unwrap(), vault_id_before);

    vault.lock();
    vault
        .unlock_for_account(&secret(PASSWORD), &secret_key, &account())
        .unwrap();
    assert_eq!(vault.get_item(&item.id).unwrap().title, "GitHub");
    // The overview title alone doesn't prove the item is untouched: the
    // password lives in the separate details blob, encrypted under the
    // same (unrotated) vault key. Reveal it too.
    assert_eq!(
        vault
            .reveal(&item.id, SecretField::Password)
            .unwrap()
            .expose(),
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
    // `begin_rekey` + `derive_with_secret_key`. That route must work for a
    // key scheme 3 vault too: the account comes from the local store, not
    // from the caller.
    let (mut vault, secret_key) = common::activated_vault();
    vault
        .create_item(common::login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let new_password = secret("a much longer new password");

    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket.derive_with_secret_key(
        &secret(PASSWORD),
        &new_password,
        fast_kdf(),
        Some(&secret_key),
    );
    vault.commit_rekey(ticket, rekeyed).unwrap();

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
fn create_vault_refuses_an_account_bound_vault() {
    // An account-bound vault must arrive with its account record, or the
    // rollback floor in `adopt_account_header` is never populated. Make the
    // route that cannot carry one refuse instead of accepting it.
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    let err = vault.create_vault(made.prepared).err().unwrap();
    assert_eq!(err.code(), "invalid_input");
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

#[test]
fn create_account_vault_refuses_a_vault_that_is_not_account_bound() {
    let made =
        havenkeys_core::vault::prepare_new_vault(&secret(PASSWORD), fast_kdf(), NOW).unwrap();
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    let err = vault
        .create_account_vault(made, &account_record())
        .err()
        .unwrap();
    assert_eq!(err.code(), "invalid_input");
}

/// A key scheme 2 vault, unlocked, with its Secret Key.
fn secret_key_vault() -> (VaultService, havenkeys_core::crypto::secret_key::SecretKey) {
    use havenkeys_core::crypto::secret_key::SecretKey;
    use havenkeys_core::vault::prepare_new_vault_with_secret_key;

    let secret_key = SecretKey::generate().unwrap();
    let made =
        prepare_new_vault_with_secret_key(&secret(PASSWORD), &secret_key, fast_kdf(), NOW).unwrap();
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(made).unwrap();
    (vault, secret_key)
}

#[test]
fn committing_an_account_upgrade_without_the_account_record_is_refused() {
    // The upgrade is the second route to an account-bound vault. It must
    // not be able to leave one behind without its account row either.
    let (mut vault, secret_key) = secret_key_vault();
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_account_upgrade(&secret(PASSWORD), &secret_key, &account(), fast_kdf())
        .unwrap();
    let err = vault.commit_rekey(ticket, Ok(rekeyed)).unwrap_err();
    assert_eq!(err.code(), "invalid_input");
    // And the vault is untouched: still scheme 2.
    assert_eq!(
        vault.key_scheme().unwrap(),
        Some(KeyScheme::PasswordAndSecretKey)
    );
}

#[test]
fn commit_account_upgrade_stores_the_account_record() {
    let (mut vault, secret_key) = secret_key_vault();
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_account_upgrade(&secret(PASSWORD), &secret_key, &account(), fast_kdf())
        .unwrap();
    vault
        .commit_account_upgrade(ticket, Ok(rekeyed), &account_record())
        .unwrap();

    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert_eq!(vault.account().unwrap().unwrap().account_id, account().id);
    // The local re-wrap raised the rollback floor, as commit_rekey does.
    assert_eq!(vault.account().unwrap().unwrap().max_header_rev, 1);
}

#[test]
fn a_failed_account_upgrade_leaves_no_account_record_behind() {
    // Wrong current password: nothing was re-wrapped, so the vault must not
    // come out of it claiming to belong to an account.
    let (mut vault, secret_key) = secret_key_vault();
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket.derive_account_upgrade(
        &secret("not the master password"),
        &secret_key,
        &account(),
        fast_kdf(),
    );
    assert!(vault
        .commit_account_upgrade(ticket, rekeyed, &account_record())
        .is_err());
    assert!(vault.account().unwrap().is_none());
}

#[test]
fn an_upgrade_refused_for_a_changed_header_leaves_no_account_record() {
    // Two upgrades race: the second ticket is stale by the time it commits.
    // The loser must not leave the account record of an upgrade that never
    // happened on what is still a scheme 2 vault.
    let (mut vault, secret_key) = secret_key_vault();
    let stale = vault.begin_rekey().unwrap();
    let stale_rekeyed =
        stale.derive_account_upgrade(&secret(PASSWORD), &secret_key, &account(), fast_kdf());

    // Meanwhile the master password changes, moving the header on.
    vault
        .change_master_password(
            &secret(PASSWORD),
            &secret("a much longer new password"),
            fast_kdf(),
        )
        .unwrap_err(); // scheme 2 needs the Secret Key; use the ticket route
    let t = vault.begin_rekey().unwrap();
    let r = t.derive_with_secret_key(
        &secret(PASSWORD),
        &secret("a much longer new password"),
        fast_kdf(),
        Some(&secret_key),
    );
    vault.commit_rekey(t, r).unwrap();

    assert!(vault
        .commit_account_upgrade(stale, stale_rekeyed, &account_record())
        .is_err());
    assert!(vault.account().unwrap().is_none());
    assert_eq!(
        vault.key_scheme().unwrap(),
        Some(KeyScheme::PasswordAndSecretKey)
    );
}

#[test]
fn commit_account_upgrade_refuses_a_rekey_that_is_not_an_upgrade() {
    // Misusing the upgrade commit for an ordinary password change would
    // link a scheme 2 vault to an account it is not bound to.
    let (mut vault, secret_key) = secret_key_vault();
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket.derive_with_secret_key(
        &secret(PASSWORD),
        &secret("a much longer new password"),
        fast_kdf(),
        Some(&secret_key),
    );
    assert!(vault
        .commit_account_upgrade(ticket, rekeyed, &account_record())
        .is_err());
    assert!(vault.account().unwrap().is_none());
}
