//! Security regression tests (docs/threat-model.md §5).

mod common;

use common::*;
use havenkeys_core::model::{SecretField, Settings};
use havenkeys_core::store::Store;
use havenkeys_core::vault::VaultState;
use havenkeys_core::Error;
use rusqlite::{params, Connection};
use uuid::Uuid;

fn file_vault() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.sqlite3");
    (dir, path)
}

/// A3: a locked vault refuses every item and secret operation.
#[test]
fn locked_vault_refuses_everything() {
    let mut v = new_vault();
    let id = v
        .create_item(login("GitHub", "octo", "pw", "github.com"), NOW)
        .unwrap()
        .id;
    v.lock();

    assert_eq!(v.list_items().err(), Some(Error::Locked));
    assert_eq!(v.search("git").err(), Some(Error::Locked));
    assert_eq!(v.get_item(&id).err(), Some(Error::Locked));
    assert_eq!(
        v.reveal(&id, SecretField::Password).err(),
        Some(Error::Locked)
    );
    assert_eq!(v.totp_code(&id, 0).err(), Some(Error::Locked));
    assert_eq!(
        v.create_item(login("a", "b", "c", "a.com"), NOW).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.update_item(&id, login("a", "b", "c", "a.com"), NOW).err(),
        Some(Error::Locked)
    );
    assert_eq!(v.delete_item(&id).err(), Some(Error::Locked));
    assert_eq!(v.settings().err(), Some(Error::Locked));
    assert_eq!(
        v.update_settings(Settings::default()).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.change_master_password(&secret(PASSWORD), &secret("another password"), fast_kdf())
            .err(),
        Some(Error::Locked)
    );
}

/// Data must be encrypted at rest: no plaintext appears anywhere in the file.
#[test]
fn nothing_sensitive_in_database_file() {
    let (_d, path) = file_vault();
    {
        let mut v = new_vault_in(Store::open(&path).unwrap());
        let mut input = login(
            "UniqueTitleXYZ",
            "unique.user@example.org",
            "UniquePasswordQQQ",
            "unique-site.example",
        );
        input.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXPJBSWY3DP"));
        input.notes = havenkeys_core::model::SecretUpdate::Set(secret("UniqueLoginNotesRRR"));
        v.create_item(input, NOW).unwrap();
        v.create_item(note("UniqueNoteTitleWWW", "UniqueNoteBodyVVV"), NOW)
            .unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    for needle in [
        "UniqueTitleXYZ",
        "unique.user@example.org",
        "UniquePasswordQQQ",
        "unique-site",
        "JBSWY3DP",
        "UniqueLoginNotesRRR",
        "UniqueNoteTitleWWW",
        "UniqueNoteBodyVVV",
        "correct horse",
        "login",
        "secure_note",
    ] {
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "plaintext {needle:?} found in vault file"
        );
    }
}

/// A4: modified ciphertext → authentication failure, no plaintext.
#[test]
fn tampered_details_blob_is_rejected() {
    let (_d, path) = file_vault();
    let id = {
        let mut v = new_vault_in(Store::open(&path).unwrap());
        v.create_item(login("GitHub", "octo", "pw", "github.com"), NOW)
            .unwrap()
            .id
    };
    let c = Connection::open(&path).unwrap();
    let mut blob: Vec<u8> = c
        .query_row(
            "SELECT details FROM items WHERE id = ?1",
            params![id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let last = blob.len() - 1;
    blob[last] ^= 0x01;
    c.execute(
        "UPDATE items SET details = ?1 WHERE id = ?2",
        params![blob, id.to_string()],
    )
    .unwrap();
    drop(c);

    let mut v = open_file(&path);
    v.unlock(&secret(PASSWORD)).unwrap();
    assert_eq!(
        v.reveal(&id, SecretField::Password).err(),
        Some(Error::Decryption)
    );
}

#[test]
fn tampered_overview_marks_item_damaged_without_blocking_vault() {
    let (_d, path) = file_vault();
    let (bad, good) = {
        let mut v = new_vault_in(Store::open(&path).unwrap());
        (
            v.create_item(login("A", "a", "pw", "a.com"), NOW)
                .unwrap()
                .id,
            v.create_item(login("B", "b", "pw", "b.com"), NOW)
                .unwrap()
                .id,
        )
    };
    let c = Connection::open(&path).unwrap();
    c.execute(
        "UPDATE items SET overview = X'0101000000000000000000000000000000000000000000000000000000' WHERE id = ?1",
        params![bad.to_string()],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    v.unlock(&secret(PASSWORD)).unwrap();
    assert_eq!(v.status().unwrap().damaged_items, 1);
    assert!(v.get_item(&good).is_ok());
    assert_eq!(v.get_item(&bad).err(), Some(Error::NotFound));
}

/// A8: blobs cannot be swapped between items (AAD binds item ID).
#[test]
fn swapped_blobs_are_rejected() {
    let (_d, path) = file_vault();
    let (a, b) = {
        let mut v = new_vault_in(Store::open(&path).unwrap());
        (
            v.create_item(login("A", "a", "pw-a", "a.com"), NOW)
                .unwrap()
                .id,
            v.create_item(login("B", "b", "pw-b", "b.com"), NOW)
                .unwrap()
                .id,
        )
    };
    let c = Connection::open(&path).unwrap();
    let details_b: Vec<u8> = c
        .query_row(
            "SELECT details FROM items WHERE id = ?1",
            params![b.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    c.execute(
        "UPDATE items SET details = ?1 WHERE id = ?2",
        params![details_b, a.to_string()],
    )
    .unwrap();
    // Also try moving a details blob into the overview slot (role confusion).
    c.execute(
        "UPDATE items SET overview = details WHERE id = ?1",
        params![b.to_string()],
    )
    .unwrap();
    drop(c);

    let mut v = open_file(&path);
    v.unlock(&secret(PASSWORD)).unwrap();
    assert_eq!(
        v.reveal(&a, SecretField::Password).err(),
        Some(Error::Decryption)
    );
    assert_eq!(v.get_item(&b).err(), Some(Error::NotFound));
    assert_eq!(v.status().unwrap().damaged_items, 1);
}

/// A9: unknown format version is refused, not misparsed.
#[test]
fn unsupported_format_version_refused() {
    let (_d, path) = file_vault();
    drop(new_vault_in(Store::open(&path).unwrap()));
    let c = Connection::open(&path).unwrap();
    c.execute("UPDATE vault_header SET format_version = 2", [])
        .unwrap();
    drop(c);
    let mut v = open_file(&path);
    assert_eq!(v.unlock(&secret(PASSWORD)), Err(Error::UnsupportedVersion));
    assert_eq!(v.state(), VaultState::Locked);
}

#[test]
fn weakened_kdf_params_refused() {
    let (_d, path) = file_vault();
    drop(new_vault_in(Store::open(&path).unwrap()));
    let c = Connection::open(&path).unwrap();
    let kdf: String = c
        .query_row("SELECT kdf FROM vault_header", [], |r| r.get(0))
        .unwrap();
    let weakened = kdf.replace("\"memory_kib\":19456", "\"memory_kib\":8");
    assert_ne!(kdf, weakened);
    c.execute("UPDATE vault_header SET kdf = ?1", params![weakened])
        .unwrap();
    drop(c);
    let mut v = open_file(&path);
    assert_eq!(v.unlock(&secret(PASSWORD)), Err(Error::Corrupted));
}

#[test]
fn garbage_header_fails_safely() {
    let (_d, path) = file_vault();
    drop(new_vault_in(Store::open(&path).unwrap()));
    let c = Connection::open(&path).unwrap();
    c.execute(
        "UPDATE vault_header SET kdf = '{not json', vault_id = 'nope'",
        [],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    assert!(v.status().is_err());
    assert!(v.unlock(&secret(PASSWORD)).is_err());
    assert_eq!(v.state(), VaultState::Locked);
}

#[test]
fn tampered_wrapped_key_fails_unlock() {
    let (_d, path) = file_vault();
    drop(new_vault_in(Store::open(&path).unwrap()));
    let c = Connection::open(&path).unwrap();
    let mut wrapped: Vec<u8> = c
        .query_row("SELECT wrapped_vault_key FROM vault_header", [], |r| {
            r.get(0)
        })
        .unwrap();
    wrapped[20] ^= 0x80;
    c.execute(
        "UPDATE vault_header SET wrapped_vault_key = ?1",
        params![wrapped],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    assert_eq!(v.unlock(&secret(PASSWORD)), Err(Error::UnlockFailed));
}

#[test]
fn unknown_item_ids_rejected() {
    let v = new_vault();
    let random = Uuid::new_v4();
    assert_eq!(v.get_item(&random).err(), Some(Error::NotFound));
    assert_eq!(
        v.reveal(&random, SecretField::Password).err(),
        Some(Error::NotFound)
    );
    assert_eq!(v.totp_code(&random, 0).err(), Some(Error::NotFound));
}

/// Row injected directly into SQLite (not created through the core) is never
/// trusted: it fails authentication and is not listed.
#[test]
fn injected_rows_are_not_trusted() {
    let (_d, path) = file_vault();
    drop(new_vault_in(Store::open(&path).unwrap()));
    let c = Connection::open(&path).unwrap();
    let id = Uuid::new_v4();
    c.execute(
        "INSERT INTO items (id, overview, details) VALUES (?1, ?2, ?2)",
        params![id.to_string(), vec![1u8; 64]],
    )
    .unwrap();
    c.execute(
        "INSERT INTO items (id, overview, details) VALUES ('not-a-uuid', X'00', X'00')",
        [],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    v.unlock(&secret(PASSWORD)).unwrap();
    assert!(v.list_items().unwrap().is_empty());
    assert_eq!(v.status().unwrap().damaged_items, 2);
}

/// Error messages are fixed strings; none may echo user input.
#[test]
fn errors_never_echo_input() {
    let mut v = new_vault();
    let marker = "SENSITIVE-MARKER-123";
    let mut input = login(marker, marker, marker, &format!("javascript:{marker}"));
    let e1 = v.create_item(input, NOW).unwrap_err();
    input = login(&format!("{marker}\u{0}"), "u", "p", "a.com");
    let e2 = v.create_item(input, NOW).unwrap_err();
    let e3 = v.unlock(&secret(marker)).unwrap_err();
    for e in [e1, e2, e3] {
        assert!(!e.to_string().contains(marker));
        assert!(!format!("{e:?}").contains(marker));
    }
}

/// Debug output of secret-bearing values never contains the secret.
#[test]
fn debug_output_is_redacted() {
    let mut v = new_vault();
    let ov = v
        .create_item(
            login("DebugTitle", "debug-user", "debug-pass", "a.com"),
            NOW,
        )
        .unwrap();
    let dbg = format!("{ov:?}");
    assert!(!dbg.contains("DebugTitle") && !dbg.contains("debug-user"));
    let pw = v.reveal(&ov.id, SecretField::Password).unwrap();
    assert!(!format!("{pw:?}").contains("debug-pass"));
}

// ---------------------------------------------------------------- origin binding

fn github_vault() -> (havenkeys_core::vault::VaultService, Uuid) {
    let mut v = new_vault();
    let mut input = login("GitHub", "octo", "gh-secret", "github.com");
    input.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXPJBSWY3DP"));
    let id = v.create_item(input, NOW).unwrap().id;
    v.create_item(
        login("Bank", "alice", "bank-secret", "mybank.com"),
        NOW,
    )
    .unwrap();
    (v, id)
}

/// A1: a page on evil.com asks for the github.com credential.
#[test]
fn a1_wrong_origin_is_denied() {
    let (v, id) = github_vault();
    for page in [
        "https://evil.com/login",
        "https://github.com.evil.com/login",
        "https://github-login.example.com/",
        "http://github.com/login", // downgrade
        "javascript:alert(1)",
        "",
    ] {
        assert_eq!(
            v.fill_for_page(&id, page).err(),
            Some(Error::Denied),
            "{page}"
        );
        assert_eq!(
            v.totp_for_page(&id, page, 59).err(),
            Some(Error::Denied),
            "{page}"
        );
        assert!(v.find_matches(page).unwrap().is_empty(), "{page}");
    }
}

/// A2: arbitrary item IDs are only served for pages they match.
#[test]
fn a2_item_only_for_matching_origin() {
    let (v, gh) = github_vault();
    let creds = v.fill_for_page(&gh, "https://github.com/session").unwrap();
    assert_eq!(creds.username.as_deref(), Some("octo"));
    assert_eq!(creds.password.unwrap().expose(), "gh-secret");
    assert!(v
        .totp_for_page(&gh, "https://github.com/sessions/two-factor", 59)
        .is_ok());

    // The bank item exists but must not be served on github.com.
    let bank = v.find_matches("https://mybank.com/").unwrap()[0].id;
    assert_eq!(
        v.fill_for_page(&bank, "https://github.com/").err(),
        Some(Error::Denied)
    );
    // Unknown IDs.
    assert_eq!(
        v.fill_for_page(&Uuid::new_v4(), "https://github.com/")
            .err(),
        Some(Error::NotFound)
    );
}

#[test]
fn find_matches_returns_no_secrets_and_ranks() {
    let (mut v, _) = github_vault();
    v.create_item(
        login("GitHub Gist", "octo2", "pw", "https://gist.github.com"),
        NOW,
    )
    .unwrap();
    let matches = v.find_matches("https://gist.github.com/new").unwrap();
    let titles: Vec<&str> = matches.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["GitHub Gist", "GitHub"],
        "exact host ranks above same site"
    );
    let json = serde_json::to_string(&matches).unwrap();
    assert!(!json.contains("gh-secret") && !json.contains("JBSWY3DP"));
}

#[test]
fn notes_and_locked_vault_are_never_served_to_pages() {
    let (mut v, gh) = github_vault();
    let note = v
        .create_item(note("github.com", "secret note"), NOW)
        .unwrap()
        .id;
    assert_eq!(
        v.fill_for_page(&note, "https://github.com/").err(),
        Some(Error::Denied)
    );
    v.lock();
    assert_eq!(
        v.find_matches("https://github.com/").err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.fill_for_page(&gh, "https://github.com/").err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.totp_for_page(&gh, "https://github.com/", 59).err(),
        Some(Error::Locked)
    );
}
