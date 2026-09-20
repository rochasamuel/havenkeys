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
fn local_edits_are_dirty_and_clear_on_confirmation() {
    use havenkeys_core::store::Store;

    let mut store = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::from_u128(9);
    store.upsert_item(&id, b"overview", b"details").unwrap();
    assert_eq!(store.dirty_rows().unwrap().len(), 1);

    store
        .clear_dirty(&[(id, b"overview".to_vec(), b"details".to_vec())], &[])
        .unwrap();
    assert!(store.dirty_rows().unwrap().is_empty());

    // Editing it again marks it dirty again.
    store.upsert_item(&id, b"overview2", b"details2").unwrap();
    assert_eq!(store.dirty_rows().unwrap().len(), 1);
}

#[test]
fn deletions_are_dirty_until_confirmed() {
    use havenkeys_core::store::Store;

    let mut store = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::from_u128(11);
    store.upsert_item(&id, b"overview", b"details").unwrap();
    store
        .clear_dirty(&[(id, b"overview".to_vec(), b"details".to_vec())], &[])
        .unwrap();

    store.delete_item(&id, NOW).unwrap();
    assert_eq!(store.dirty_tombstones().unwrap(), vec![(id, NOW)]);

    store.clear_dirty(&[], &[(id, NOW)]).unwrap();
    assert!(store.dirty_tombstones().unwrap().is_empty());
}

#[test]
fn upgrades_a_schema_2_database_in_place() {
    use havenkeys_core::store::Store;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.sqlite3");

    // Build a schema-2 database by hand: the tables as they were, and the
    // user_version that says so.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE vault_header (
                 id INTEGER PRIMARY KEY CHECK (id = 1), format_version INTEGER NOT NULL,
                 vault_id TEXT NOT NULL, kdf TEXT NOT NULL, wrapped_vault_key BLOB NOT NULL,
                 created_at INTEGER NOT NULL, key_scheme INTEGER NOT NULL DEFAULT 1,
                 header_revision INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, overview BLOB NOT NULL, details BLOB NOT NULL);
             CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1), blob BLOB NOT NULL);
             CREATE TABLE tombstones (id TEXT PRIMARY KEY NOT NULL, deleted_at INTEGER NOT NULL);
             INSERT INTO items (id, overview, details)
               VALUES ('11111111-1111-1111-1111-111111111111', x'00', x'01');
             PRAGMA user_version = 2;",
        )
        .unwrap();
    }

    let store = Store::open(&path).unwrap();
    // The pre-existing row must come back dirty, so it gets uploaded once.
    assert_eq!(store.dirty_rows().unwrap().len(), 1);
    assert!(store.account().unwrap().is_none());
}
