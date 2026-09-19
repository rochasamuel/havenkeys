//! Functional tests for the vault lifecycle and item operations.

mod common;

use common::*;
use havenkeys_core::model::{SecretField, SecretUpdate, Settings};
use havenkeys_core::vault::{prepare_new_vault, VaultService, VaultState};
use havenkeys_core::Error;

#[test]
fn create_lock_unlock_cycle() {
    let mut v = new_vault();
    let st = v.status().unwrap();
    assert_eq!(st.state, VaultState::Unlocked);
    assert!(st.vault_exists);

    assert!(v.lock());
    assert_eq!(v.state(), VaultState::Locked);
    assert!(!v.lock(), "lock is idempotent");

    assert_eq!(
        v.unlock(&secret("wrong password!")),
        Err(Error::UnlockFailed)
    );
    assert_eq!(v.state(), VaultState::Locked);

    v.unlock(&secret(PASSWORD)).unwrap();
    assert!(v.is_unlocked());
}

#[test]
fn cannot_create_twice_or_unlock_without_vault() {
    let mut empty = VaultService::new(havenkeys_core::store::Store::open_in_memory().unwrap());
    assert!(!empty.status().unwrap().vault_exists);
    assert_eq!(empty.unlock(&secret(PASSWORD)).err(), Some(Error::NoVault));
    assert_eq!(empty.state(), VaultState::Locked);

    let mut v = new_vault();
    v.lock();
    let again = prepare_new_vault(&secret(PASSWORD), fast_kdf(), NOW).unwrap();
    assert_eq!(v.create_vault(again), Err(Error::VaultExists));
}

#[test]
fn weak_master_password_rejected() {
    assert!(prepare_new_vault(&secret("short"), fast_kdf(), NOW).is_err());
}

#[test]
fn vault_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.sqlite3");
    let id = {
        let mut v = VaultService::new(havenkeys_core::store::Store::open(&path).unwrap());
        v.create_vault(prepare_new_vault(&secret(PASSWORD), fast_kdf(), NOW).unwrap())
            .unwrap();
        v.create_item(login("GitHub", "octo", "gh-secret-pw", "github.com"), NOW)
            .unwrap()
            .id
    };
    let mut v = open_file(&path);
    assert_eq!(v.state(), VaultState::Locked);
    v.unlock(&secret(PASSWORD)).unwrap();
    assert_eq!(v.get_item(&id).unwrap().title, "GitHub");
    assert_eq!(
        v.reveal(&id, SecretField::Password).unwrap().expose(),
        "gh-secret-pw"
    );
}

#[test]
fn login_item_crud_and_secret_minimization() {
    let mut v = new_vault();
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
    let mut v = new_vault();
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
    let mut v = new_vault();
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
    let mut v = new_vault();
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
    let mut v = new_vault();
    let id = v
        .create_item(login("A", "a", "pw-a", "a.com"), NOW)
        .unwrap()
        .id;
    assert_eq!(
        v.change_master_password(
            &secret("not the password"),
            &secret("brand new password"),
            fast_kdf()
        ),
        Err(Error::UnlockFailed)
    );
    v.change_master_password(&secret(PASSWORD), &secret("brand new password"), fast_kdf())
        .unwrap();
    v.lock();
    assert_eq!(v.unlock(&secret(PASSWORD)), Err(Error::UnlockFailed));
    v.unlock(&secret("brand new password")).unwrap();
    assert_eq!(
        v.reveal(&id, SecretField::Password).unwrap().expose(),
        "pw-a"
    );
}

#[test]
fn settings_are_encrypted_and_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.db");
    {
        let mut v = new_vault_in(havenkeys_core::store::Store::open(&path).unwrap());
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
    }
    let mut v = open_file(&path);
    v.unlock(&secret(PASSWORD)).unwrap();
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
    let mut v = new_vault();
    v.lock();
    let ticket = v.begin_unlock().unwrap();
    assert_eq!(v.state(), VaultState::Unlocking);
    assert_eq!(v.begin_unlock().err(), Some(Error::Busy));
    let key = ticket.derive(&secret(PASSWORD));
    v.lock(); // e.g. app exit while Argon2 was running
    assert_eq!(v.finish_unlock(ticket, key), Err(Error::Locked));
    assert_eq!(v.state(), VaultState::Locked);
    assert!(v.list_items().is_err());
}

#[test]
fn epoch_advances_on_lock() {
    let mut v = new_vault();
    let e = v.epoch();
    v.lock();
    assert!(v.epoch() > e);
}

#[test]
fn rekey_is_discarded_if_vault_locked_meanwhile() {
    let mut v = new_vault();
    let ticket = v.begin_rekey().unwrap();
    let rekeyed = ticket.derive(&secret(PASSWORD), &secret("brand new password"), fast_kdf());
    v.lock();
    v.unlock(&secret(PASSWORD)).unwrap();
    assert_eq!(v.commit_rekey(ticket, rekeyed), Err(Error::Locked));
    v.lock();
    v.unlock(&secret(PASSWORD)).unwrap(); // old password still valid
}

#[test]
fn rekey_refused_if_header_changed_meanwhile() {
    let mut v = new_vault();
    let t1 = v.begin_rekey().unwrap();
    let r1 = t1.derive(&secret(PASSWORD), &secret("first new password"), fast_kdf());
    v.change_master_password(
        &secret(PASSWORD),
        &secret("second new password"),
        fast_kdf(),
    )
    .unwrap();
    assert_eq!(v.commit_rekey(t1, r1), Err(Error::Busy));
    v.lock();
    v.unlock(&secret("second new password")).unwrap();
}
