//! Functional tests for the vault lifecycle and item operations.

mod common;

use common::*;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::model::{SecretField, SecretUpdate, Settings};
use havenkeys_core::vault::{prepare_new_account_vault, VaultService, VaultState};
use havenkeys_core::Error;

#[test]
fn create_lock_unlock_cycle() {
    let (mut v, sk) = activated_vault();
    let st = v.status().unwrap();
    assert_eq!(st.state, VaultState::Unlocked);
    assert!(st.vault_exists);

    assert!(v.lock());
    assert_eq!(v.state(), VaultState::Locked);
    assert!(!v.lock(), "lock is idempotent");

    assert_eq!(
        v.unlock_for_account(&secret("wrong password!"), &sk, &account()),
        Err(Error::UnlockFailed)
    );
    assert_eq!(v.state(), VaultState::Locked);

    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert!(v.is_unlocked());
}

#[test]
fn cannot_create_twice_or_unlock_without_vault() {
    let mut empty = VaultService::new(havenkeys_core::store::Store::open_in_memory().unwrap());
    assert!(!empty.status().unwrap().vault_exists);
    assert_eq!(
        empty
            .unlock_for_account(
                &secret(PASSWORD),
                &SecretKey::generate().unwrap(),
                &account()
            )
            .err(),
        Some(Error::NoVault)
    );
    assert_eq!(empty.state(), VaultState::Locked);

    let (mut v, _sk) = activated_vault();
    v.lock();
    let again = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    assert_eq!(
        v.create_account_vault(again.prepared, &account_record()),
        Err(Error::VaultExists)
    );
}

#[test]
fn weak_master_password_rejected() {
    assert!(prepare_new_account_vault(&secret("short"), &account(), fast_kdf(), NOW).is_err());
}

#[test]
fn vault_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.sqlite3");
    let (id, sk) = {
        let (mut v, sk) = activated_vault_at(&path);
        let id = v
            .create_item(login("GitHub", "octo", "gh-secret-pw", "github.com"), NOW)
            .unwrap()
            .id;
        (id, sk)
    };
    let mut v = open_file(&path);
    assert_eq!(v.state(), VaultState::Locked);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(v.get_item(&id).unwrap().title, "GitHub");
    assert_eq!(
        v.reveal(&id, SecretField::Password).unwrap().expose(),
        "gh-secret-pw"
    );
}

#[test]
fn login_item_crud_and_secret_minimization() {
    let (mut v, _sk) = activated_vault();
    let ov = v
        .create_item(
            login("GitHub", "octo@example.com", "pw-1", "github.com"),
            NOW,
        )
        .unwrap();
    assert_eq!(ov.urls[0].url, "https://github.com/");
    assert!(ov.has_password && !ov.has_totp && !ov.has_notes);

    // Overview serialization never contains the password.
    let json = serde_json::to_string(&ov).unwrap();
    assert!(!json.contains("pw-1"));

    // Update title only; password is kept without the UI ever seeing it.
    let mut edit = login("GitHub (work)", "octo@example.com", "", "github.com");
    edit.password = SecretUpdate::Keep;
    let updated = v.update_item(&ov.id, edit, NOW + 1000).unwrap();
    assert_eq!(updated.title, "GitHub (work)");
    assert_eq!(updated.created_at, NOW);
    assert_eq!(updated.updated_at, NOW + 1000);
    assert_eq!(
        v.reveal(&ov.id, SecretField::Password).unwrap().expose(),
        "pw-1"
    );

    // Clear the password.
    let mut edit = login("GitHub (work)", "octo@example.com", "", "github.com");
    edit.password = SecretUpdate::Clear;
    assert!(!v.update_item(&ov.id, edit, NOW).unwrap().has_password);
    assert_eq!(
        v.reveal(&ov.id, SecretField::Password).err(),
        Some(Error::NotFound)
    );

    v.delete_item(&ov.id, NOW).unwrap();
    assert_eq!(v.get_item(&ov.id).err(), Some(Error::NotFound));
    assert_eq!(v.delete_item(&ov.id, NOW), Err(Error::NotFound));
}

#[test]
fn totp_codes_without_exposing_secret() {
    let (mut v, _sk) = activated_vault();
    let mut input = login("AWS", "root", "pw", "aws.amazon.com");
    // RFC 6238 SHA-1 secret "12345678901234567890".
    input.totp = SecretUpdate::Set(secret("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"));
    let ov = v.create_item(input, NOW).unwrap();
    assert!(ov.has_totp);
    let code = v.totp_code(&ov.id, 59).unwrap();
    assert_eq!(code.code.expose(), "287082");
    assert_eq!(code.seconds_remaining, 1);

    // Invalid TOTP input rejected; error does not echo the input.
    let mut bad = login("x", "y", "z", "x.com");
    bad.totp = SecretUpdate::Set(secret("not-base32-SECRETVALUE!"));
    let err = v.create_item(bad, NOW).unwrap_err();
    assert!(!err.to_string().contains("SECRETVALUE"));

    // Item without TOTP.
    let plain = v.create_item(login("b", "c", "d", "b.com"), NOW).unwrap();
    assert_eq!(v.totp_code(&plain.id, 59).err(), Some(Error::NotFound));
}

#[test]
fn secure_notes() {
    let (mut v, _sk) = activated_vault();
    let ov = v
        .create_item(note("Recovery codes", "abcd-efgh\nijkl-mnop"), NOW)
        .unwrap();
    assert_eq!(
        v.reveal(&ov.id, SecretField::Content).unwrap().expose(),
        "abcd-efgh\nijkl-mnop"
    );
    // Login-only fields are rejected on notes and vice versa.
    assert!(v.reveal(&ov.id, SecretField::Password).is_err());
    let mut bad = note("x", "y");
    bad.password = SecretUpdate::Set(secret("pw"));
    assert!(v.create_item(bad, NOW).is_err());
    let mut bad = login("x", "y", "z", "x.com");
    bad.content = SecretUpdate::Set(secret("c"));
    assert!(v.create_item(bad, NOW).is_err());
    // Type cannot change on update.
    assert!(v
        .update_item(&ov.id, login("x", "y", "z", "x.com"), NOW)
        .is_err());
}

#[test]
fn search_uses_overviews_only() {
    let (mut v, _sk) = activated_vault();
    v.create_item(login("GitHub", "octo", "pw", "github.com"), NOW)
        .unwrap();
    v.create_item(login("Bank", "alice@mail.com", "pw", "mybank.example"), NOW)
        .unwrap();
    v.create_item(note("Wifi", "password is githubwifi"), NOW)
        .unwrap();

    let titles = |q: &str| -> Vec<String> {
        v.search(q)
            .unwrap()
            .iter()
            .map(|i| i.title.clone())
            .collect()
    };
    assert_eq!(titles("git"), vec!["GitHub"]);
    assert_eq!(titles("ALICE"), vec!["Bank"]);
    assert_eq!(titles("mybank"), vec!["Bank"]);
    assert_eq!(titles(""), vec!["Bank", "GitHub", "Wifi"]);
    // Note bodies are never searched.
    assert!(titles("githubwifi").is_empty());
    assert!(v.search(&"x".repeat(1000)).is_err());
}

#[test]
fn change_master_password() {
    let (mut v, sk) = activated_vault();
    let id = v
        .create_item(login("A", "a", "pw-a", "a.com"), NOW)
        .unwrap()
        .id;
    assert_eq!(
        v.change_master_password_for_account(
            &secret("not the password"),
            &secret("brand new password"),
            fast_kdf(),
            &sk,
        ),
        Err(Error::UnlockFailed)
    );
    v.change_master_password_for_account(
        &secret(PASSWORD),
        &secret("brand new password"),
        fast_kdf(),
        &sk,
    )
    .unwrap();
    v.lock();
    assert_eq!(
        v.unlock_for_account(&secret(PASSWORD), &sk, &account()),
        Err(Error::UnlockFailed)
    );
    v.unlock_for_account(&secret("brand new password"), &sk, &account())
        .unwrap();
    assert_eq!(
        v.reveal(&id, SecretField::Password).unwrap().expose(),
        "pw-a"
    );
}

#[test]
fn settings_are_encrypted_and_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.db");
    let sk = {
        let (mut v, sk) = activated_vault_at(&path);
        assert_eq!(v.settings().unwrap(), Settings::default());
        v.update_settings(Settings {
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 60,
            ..Default::default()
        })
        .unwrap();
        assert!(v
            .update_settings(Settings {
                auto_lock_minutes: 3,
                clipboard_clear_seconds: 60,
                ..Default::default()
            })
            .is_err());
        sk
    };
    let mut v = open_file(&path);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(v.settings().unwrap().auto_lock_minutes, 5);
    v.update_settings(Settings {
        theme: havenkeys_core::model::Theme::Light,
        ..v.settings().unwrap()
    })
    .unwrap();
    assert_eq!(
        v.settings().unwrap().theme,
        havenkeys_core::model::Theme::Light
    );
}

#[test]
fn lock_during_unlock_discards_result() {
    let (mut v, sk) = activated_vault();
    v.lock();
    let ticket = v.begin_unlock().unwrap();
    assert_eq!(v.state(), VaultState::Unlocking);
    assert_eq!(v.begin_unlock().err(), Some(Error::Busy));
    let key = ticket.derive_for_account(&secret(PASSWORD), &sk, &account());
    v.lock(); // e.g. app exit while Argon2 was running
    assert_eq!(v.finish_unlock(ticket, key), Err(Error::Locked));
    assert_eq!(v.state(), VaultState::Locked);
    assert!(v.list_items().is_err());
}

#[test]
fn epoch_advances_on_lock() {
    let (mut v, _sk) = activated_vault();
    let e = v.epoch();
    v.lock();
    assert!(v.epoch() > e);
}

#[test]
fn rekey_is_discarded_if_vault_locked_meanwhile() {
    let (mut v, sk) = activated_vault();
    let ticket = v.begin_rekey().unwrap();
    let rekeyed = ticket.derive_for_account(
        &secret(PASSWORD),
        &secret("brand new password"),
        fast_kdf(),
        &sk,
        &account(),
    );
    v.lock();
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(v.commit_rekey(ticket, rekeyed), Err(Error::Locked));
    v.lock();
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap(); // old password still valid
}

#[test]
fn rekey_refused_if_header_changed_meanwhile() {
    let (mut v, sk) = activated_vault();
    let t1 = v.begin_rekey().unwrap();
    let r1 = t1.derive_for_account(
        &secret(PASSWORD),
        &secret("first new password"),
        fast_kdf(),
        &sk,
        &account(),
    );
    v.change_master_password_for_account(
        &secret(PASSWORD),
        &secret("second new password"),
        fast_kdf(),
        &sk,
    )
    .unwrap();
    assert_eq!(v.commit_rekey(t1, r1), Err(Error::Busy));
    v.lock();
    v.unlock_for_account(&secret("second new password"), &sk, &account())
        .unwrap();
}

#[test]
fn key_scheme_serde_form_is_frozen() {
    use havenkeys_core::store::KeyScheme;
    // Serde form is part of the sync header on the wire; freeze it.
    assert_eq!(
        serde_json::to_string(&KeyScheme::AccountBound).unwrap(),
        "\"account_bound\""
    );
}

#[test]
fn the_header_refuses_any_scheme_but_account_bound() {
    use havenkeys_core::store::KeyScheme;
    // Serde form is part of the sync header on the wire; freeze it.
    assert_eq!(
        serde_json::to_string(&KeyScheme::AccountBound).unwrap(),
        "\"account_bound\""
    );
    // A database written by an older build carries key_scheme 1 or 2.
    assert!(serde_json::from_str::<KeyScheme>("\"password_only\"").is_err());
    assert!(serde_json::from_str::<KeyScheme>("\"password_and_secret_key\"").is_err());
}

#[test]
fn account_record_round_trips() {
    use havenkeys_core::store::{AccountRecord, Store};

    let mut store = Store::open_in_memory().unwrap();
    assert!(store.account().unwrap().is_none());

    let rec = AccountRecord {
        account_id: uuid::Uuid::from_u128(42),
        email: "User@Example.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    };
    store.set_account(&rec).unwrap();

    let back = store.account().unwrap().unwrap();
    assert_eq!(back.account_id, rec.account_id);
    assert_eq!(back.email, "User@Example.com");
    assert_eq!(back.server_url, rec.server_url);
    assert_eq!(back.server_cursor, 0);
}

#[test]
fn cursor_and_header_guard_move_only_forward() {
    use havenkeys_core::store::{AccountRecord, Store};

    let mut store = Store::open_in_memory().unwrap();
    store
        .set_account(&AccountRecord {
            account_id: uuid::Uuid::from_u128(1),
            email: "a@b.com".into(),
            server_url: "https://x".into(),
            server_cursor: 0,
            max_header_rev: 0,
            last_synced_at: None,
        })
        .unwrap();

    store.set_cursor(7, NOW).unwrap();
    assert_eq!(store.account().unwrap().unwrap().server_cursor, 7);

    store.raise_max_header_rev(5).unwrap();
    store.raise_max_header_rev(3).unwrap(); // an older header must not lower it
    assert_eq!(store.account().unwrap().unwrap().max_header_rev, 5);
}

#[test]
fn item_rows_carry_the_server_revision() {
    use havenkeys_core::store::Store;
    let mut store = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::from_u128(9);
    store.upsert_item(&id, b"ov", b"det", 7).unwrap();
    assert_eq!(store.item_revision(&id).unwrap(), Some(7));

    store.upsert_item(&id, b"ov2", b"det2", 9).unwrap();
    assert_eq!(store.item_revision(&id).unwrap(), Some(9));

    assert!(store.delete_item(&id).unwrap());
    assert_eq!(store.item_revision(&id).unwrap(), None);
    // No tombstone survives the delete: the server holds those.
    assert!(!store.delete_item(&id).unwrap());
}

#[test]
fn set_account_refuses_a_different_account_on_the_same_vault() {
    use havenkeys_core::store::{AccountRecord, Store};

    let mut store = Store::open_in_memory().unwrap();
    let rec = AccountRecord {
        account_id: uuid::Uuid::from_u128(1),
        email: "a@b.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    };
    store.set_account(&rec).unwrap();
    store.set_cursor(7, NOW).unwrap();
    store.raise_max_header_rev(5).unwrap();

    // Re-pointing this vault at another account would leave the cursor and
    // the rollback floor behind: the first pull would silently skip
    // everything before cursor 7, and every header from the new account
    // would fail the floor check. Refuse instead of accepting it silently.
    let other = AccountRecord {
        account_id: uuid::Uuid::from_u128(2),
        ..rec.clone()
    };
    let err = store.set_account(&other).unwrap_err();
    assert_eq!(err.code(), "invalid_input");
    assert_eq!(store.account().unwrap().unwrap().account_id, rec.account_id);
}

#[test]
fn set_account_updates_email_and_server_without_losing_sync_state() {
    use havenkeys_core::store::{AccountRecord, Store};

    let mut store = Store::open_in_memory().unwrap();
    let rec = AccountRecord {
        account_id: uuid::Uuid::from_u128(1),
        email: "a@b.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    };
    store.set_account(&rec).unwrap();
    store.set_cursor(7, NOW).unwrap();
    store.raise_max_header_rev(5).unwrap();

    // Signing in again to the same account: the display email and the
    // server address may have changed, the sync bookkeeping must not.
    store
        .set_account(&AccountRecord {
            email: "A@B.com".into(),
            server_url: "https://vault2.example.com".into(),
            server_cursor: 0,
            max_header_rev: 0,
            ..rec.clone()
        })
        .unwrap();

    let back = store.account().unwrap().unwrap();
    assert_eq!(back.email, "A@B.com");
    assert_eq!(back.server_url, "https://vault2.example.com");
    assert_eq!(back.server_cursor, 7);
    assert_eq!(back.max_header_rev, 5);
}

#[test]
fn every_scheme_says_whether_it_needs_the_secret_key() {
    use havenkeys_core::store::KeyScheme;
    // The desktop asks this instead of matching on the scheme name, so a
    // scheme added later cannot silently stop asking for the Secret Key.
    assert!(KeyScheme::AccountBound.uses_secret_key());
}
