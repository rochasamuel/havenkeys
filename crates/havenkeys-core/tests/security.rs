//! Security regression tests (docs/threat-model.md §5).

mod common;

use common::*;
use havenkeys_core::model::{ItemInput, ItemType, SecretField, SecretUpdate, Settings};
use havenkeys_core::vault::{SaveAction, VaultState};
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
    let (mut v, sk) = activated_vault();
    let staged = v
        .stage_create(login("GitHub", "octo", "pw", "github.com"), NOW)
        .unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
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
        v.stage_create(login("a", "b", "c", "a.com"), NOW).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.stage_update(&id, login("a", "b", "c", "a.com"), NOW)
            .err(),
        Some(Error::Locked)
    );
    assert_eq!(v.stage_delete(&id).err(), Some(Error::Locked));
    assert_eq!(v.settings().err(), Some(Error::Locked));
    assert_eq!(
        v.update_settings(Settings::default()).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.change_master_password_for_account(
            &secret(PASSWORD),
            &secret("another password"),
            fast_kdf(),
            &sk,
        )
        .err(),
        Some(Error::Locked)
    );
}

/// Data must be encrypted at rest: no plaintext appears anywhere in the file.
#[test]
fn nothing_sensitive_in_database_file() {
    let (_d, path) = file_vault();
    {
        let (mut v, _sk) = activated_vault_at(&path);
        let mut input = login(
            "UniqueTitleXYZ",
            "unique.user@example.org",
            "UniquePasswordQQQ",
            "unique-site.example",
        );
        input.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXPJBSWY3DP"));
        input.notes = havenkeys_core::model::SecretUpdate::Set(secret("UniqueLoginNotesRRR"));
        let s1 = v.stage_create(input, NOW).unwrap();
        v.commit_write(s1, 1).unwrap();
        let s2 = v
            .stage_create(note("UniqueNoteTitleWWW", "UniqueNoteBodyVVV"), NOW)
            .unwrap();
        v.commit_write(s2, 2).unwrap();
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
    let (id, sk) = {
        let (mut v, sk) = activated_vault_at(&path);
        let staged = v
            .stage_create(login("GitHub", "octo", "pw", "github.com"), NOW)
            .unwrap();
        let id = v.commit_write(staged, 1).unwrap().unwrap().id;
        (id, sk)
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
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(
        v.reveal(&id, SecretField::Password).err(),
        Some(Error::Decryption)
    );
}

#[test]
fn tampered_overview_marks_item_damaged_without_blocking_vault() {
    let (_d, path) = file_vault();
    let (bad, good, sk) = {
        let (mut v, sk) = activated_vault_at(&path);
        let s1 = v.stage_create(login("A", "a", "pw", "a.com"), NOW).unwrap();
        let bad = v.commit_write(s1, 1).unwrap().unwrap().id;
        let s2 = v.stage_create(login("B", "b", "pw", "b.com"), NOW).unwrap();
        let good = v.commit_write(s2, 2).unwrap().unwrap().id;
        (bad, good, sk)
    };
    let c = Connection::open(&path).unwrap();
    c.execute(
        "UPDATE items SET overview = X'0101000000000000000000000000000000000000000000000000000000' WHERE id = ?1",
        params![bad.to_string()],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert_eq!(v.status().unwrap().damaged_items, 1);
    assert!(v.get_item(&good).is_ok());
    assert_eq!(v.get_item(&bad).err(), Some(Error::NotFound));
}

/// A8: blobs cannot be swapped between items (AAD binds item ID).
#[test]
fn swapped_blobs_are_rejected() {
    let (_d, path) = file_vault();
    let (a, b, sk) = {
        let (mut v, sk) = activated_vault_at(&path);
        let sa = v
            .stage_create(login("A", "a", "pw-a", "a.com"), NOW)
            .unwrap();
        let a = v.commit_write(sa, 1).unwrap().unwrap().id;
        let sb = v
            .stage_create(login("B", "b", "pw-b", "b.com"), NOW)
            .unwrap();
        let b = v.commit_write(sb, 2).unwrap().unwrap().id;
        (a, b, sk)
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
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
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
    let sk = {
        let (v, sk) = activated_vault_at(&path);
        drop(v);
        sk
    };
    let c = Connection::open(&path).unwrap();
    c.execute("UPDATE vault_header SET format_version = 2", [])
        .unwrap();
    drop(c);
    let mut v = open_file(&path);
    assert_eq!(
        v.unlock_for_account(&secret(PASSWORD), &sk, &account()),
        Err(Error::UnsupportedVersion)
    );
    assert_eq!(v.state(), VaultState::Locked);
}

#[test]
fn weakened_kdf_params_refused() {
    let (_d, path) = file_vault();
    let sk = {
        let (v, sk) = activated_vault_at(&path);
        drop(v);
        sk
    };
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
    assert_eq!(
        v.unlock_for_account(&secret(PASSWORD), &sk, &account()),
        Err(Error::Corrupted)
    );
}

#[test]
fn garbage_header_fails_safely() {
    let (_d, path) = file_vault();
    let sk = {
        let (v, sk) = activated_vault_at(&path);
        drop(v);
        sk
    };
    let c = Connection::open(&path).unwrap();
    c.execute(
        "UPDATE vault_header SET kdf = '{not json', vault_id = 'nope'",
        [],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    assert!(v.status().is_err());
    assert!(v
        .unlock_for_account(&secret(PASSWORD), &sk, &account())
        .is_err());
    assert_eq!(v.state(), VaultState::Locked);
}

#[test]
fn tampered_wrapped_key_fails_unlock() {
    let (_d, path) = file_vault();
    let sk = {
        let (v, sk) = activated_vault_at(&path);
        drop(v);
        sk
    };
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
    assert_eq!(
        v.unlock_for_account(&secret(PASSWORD), &sk, &account()),
        Err(Error::UnlockFailed)
    );
}

#[test]
fn unknown_item_ids_rejected() {
    let (v, _sk) = activated_vault();
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
    let sk = {
        let (v, sk) = activated_vault_at(&path);
        drop(v);
        sk
    };
    let c = Connection::open(&path).unwrap();
    let id = Uuid::new_v4();
    c.execute(
        "INSERT INTO items (id, overview, details, revision) VALUES (?1, ?2, ?2, 0)",
        params![id.to_string(), vec![1u8; 64]],
    )
    .unwrap();
    c.execute(
        "INSERT INTO items (id, overview, details, revision) VALUES ('not-a-uuid', X'00', X'00', 0)",
        [],
    )
    .unwrap();
    drop(c);
    let mut v = open_file(&path);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert!(v.list_items().unwrap().is_empty());
    assert_eq!(v.status().unwrap().damaged_items, 2);
}

/// Error messages are fixed strings; none may echo user input.
#[test]
fn errors_never_echo_input() {
    let (mut v, sk) = activated_vault();
    let marker = "SENSITIVE-MARKER-123";
    let mut input = login(marker, marker, marker, &format!("javascript:{marker}"));
    let e1 = v.stage_create(input, NOW).unwrap_err();
    input = login(&format!("{marker}\u{0}"), "u", "p", "a.com");
    let e2 = v.stage_create(input, NOW).unwrap_err();
    let e3 = v
        .unlock_for_account(&secret(marker), &sk, &account())
        .unwrap_err();
    for e in [e1, e2, e3] {
        assert!(!e.to_string().contains(marker));
        assert!(!format!("{e:?}").contains(marker));
    }
}

/// Debug output of secret-bearing values never contains the secret.
#[test]
fn debug_output_is_redacted() {
    let (mut v, _sk) = activated_vault();
    let staged = v
        .stage_create(
            login("DebugTitle", "debug-user", "debug-pass", "a.com"),
            NOW,
        )
        .unwrap();
    let ov = v.commit_write(staged, 1).unwrap().unwrap();
    let dbg = format!("{ov:?}");
    assert!(!dbg.contains("DebugTitle") && !dbg.contains("debug-user"));
    let pw = v.reveal(&ov.id, SecretField::Password).unwrap();
    assert!(!format!("{pw:?}").contains("debug-pass"));
}

// ---------------------------------------------------------------- origin binding

fn github_vault() -> (havenkeys_core::vault::VaultService, Uuid) {
    let (mut v, _sk) = activated_vault();
    let mut input = login("GitHub", "octo", "gh-secret", "github.com");
    input.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXPJBSWY3DP"));
    let staged = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    let staged = v
        .stage_create(login("Bank", "alice", "bank-secret", "mybank.com"), NOW)
        .unwrap();
    v.commit_write(staged, 2).unwrap();
    (v, id)
}

/// A1: a page on evil.com asks for the github.com credential.
#[test]
fn a1_wrong_origin_is_denied() {
    let (mut v, id) = github_vault();
    for page in [
        "https://evil.com/login",
        "https://github.com.evil.com/login",
        "https://github-login.example.com/",
        "http://github.com/login", // downgrade
        "javascript:alert(1)",
        "",
    ] {
        assert_eq!(
            v.fill_for_page(&id, page, None, NOW).err(),
            Some(Error::Denied),
            "{page}"
        );
        assert_eq!(
            v.totp_for_page(&id, page, None, 59).err(),
            Some(Error::Denied),
            "{page}"
        );
        assert!(v.find_matches(page, None).unwrap().is_empty(), "{page}");
    }
}

/// A2: arbitrary item IDs are only served for pages they match.
#[test]
fn a2_item_only_for_matching_origin() {
    let (mut v, gh) = github_vault();
    let creds = v
        .fill_for_page(&gh, "https://github.com/session", None, NOW)
        .unwrap();
    assert_eq!(creds.username.as_deref(), Some("octo"));
    assert_eq!(creds.password.unwrap().expose(), "gh-secret");
    assert!(v
        .totp_for_page(&gh, "https://github.com/sessions/two-factor", None, 59)
        .is_ok());

    // The bank item exists but must not be served on github.com.
    let bank = v.find_matches("https://mybank.com/", None).unwrap()[0].id;
    assert_eq!(
        v.fill_for_page(&bank, "https://github.com/", None, NOW)
            .err(),
        Some(Error::Denied)
    );
    // Unknown IDs.
    assert_eq!(
        v.fill_for_page(&Uuid::new_v4(), "https://github.com/", None, NOW)
            .err(),
        Some(Error::NotFound)
    );
}

#[test]
fn find_matches_returns_no_secrets_and_ranks() {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(
            login("GitHub Gist", "octo2", "pw", "https://gist.github.com"),
            NOW,
        )
        .unwrap();
    v.commit_write(staged, 3).unwrap();
    let matches = v.find_matches("https://gist.github.com/new", None).unwrap();
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
    let staged = v
        .stage_create(note("github.com", "secret note"), NOW)
        .unwrap();
    let note = v.commit_write(staged, 3).unwrap().unwrap().id;
    assert_eq!(
        v.fill_for_page(&note, "https://github.com/", None, NOW)
            .err(),
        Some(Error::Denied)
    );
    v.lock();
    assert_eq!(
        v.find_matches("https://github.com/", None).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.fill_for_page(&gh, "https://github.com/", None, NOW).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.totp_for_page(&gh, "https://github.com/", None, 59).err(),
        Some(Error::Locked)
    );
}

// ---------------------------------------------------------------- frames

/// A1 (frames): a github.com login iframe embedded in evil.com gets nothing,
/// even though the frame itself is github.com.
#[test]
fn a1_frame_on_foreign_top_page_is_denied() {
    let (mut v, gh) = github_vault();
    let frame = "https://github.com/login";
    for top in [
        "https://evil.com/",
        "https://github.com.evil.com/",
        "http://github.com/", // downgrade of the embedding page
        "not a url",
    ] {
        assert!(
            v.find_matches(frame, Some(top)).unwrap().is_empty(),
            "{top}"
        );
        assert_eq!(
            v.fill_for_page(&gh, frame, Some(top), NOW).err(),
            Some(Error::Denied),
            "{top}"
        );
        assert_eq!(
            v.totp_for_page(&gh, frame, Some(top), 59).err(),
            Some(Error::Denied),
            "{top}"
        );
    }
    // Same-site embedding is fine for a whole-site rule.
    assert_eq!(
        v.find_matches("https://login.github.com/", Some("https://github.com/"))
            .unwrap()
            .len(),
        1
    );
    assert!(v
        .fill_for_page(
            &gh,
            "https://github.com/login",
            Some("https://gist.github.com/"),
            NOW,
        )
        .is_ok());
    // An evil.com frame inside github.com is still evil.com.
    assert!(v
        .find_matches("https://evil.com/", Some("https://github.com/"))
        .unwrap()
        .is_empty());
}

// ---------------------------------------------------------------- save login

#[test]
fn check_login_classifies_submissions() {
    let (v, gh) = github_vault();
    let page = "https://github.com/session";
    let check =
        |user: Option<&str>, pw: &str| v.check_login(page, None, user, &secret(pw)).unwrap();
    assert_eq!(check(Some("octo"), "gh-secret"), SaveAction::Unchanged);
    assert_eq!(check(Some("  OCTO "), "gh-secret"), SaveAction::Unchanged);
    assert_eq!(check(Some("octo"), "new-secret"), SaveAction::Update(gh));
    assert_eq!(check(Some("someone-else"), "gh-secret"), SaveAction::Add);
    // Password-only step: unchanged if it is any saved password for the page.
    assert_eq!(check(None, "gh-secret"), SaveAction::Unchanged);
    assert_eq!(check(None, "other"), SaveAction::Add);
    // Another site's logins are never considered.
    assert_eq!(
        v.check_login(
            "https://evil.com/",
            None,
            Some("octo"),
            &secret("gh-secret")
        )
        .unwrap(),
        SaveAction::Add
    );
    assert!(v
        .check_login(page, None, Some("octo"), &secret(""))
        .is_err());
}

#[test]
fn save_login_adds_for_the_page_site_only() {
    let (mut v, _) = github_vault();
    // A save is an ordinary staged write: sealed here, stored only once the
    // server has accepted it (spec 2026-09-20 §8.4).
    let staged = v
        .stage_save_login(
            "https://www.example.com/signin?next=/x",
            None,
            Some("me@example.com"),
            secret("s3cret-pw"),
            None,
            NOW,
        )
        .unwrap();
    assert!(
        v.find_matches("https://www.example.com/", None)
            .unwrap()
            .is_empty(),
        "nothing is stored before the server accepts it"
    );

    v.commit_write(staged.write, 7).unwrap();
    let matches = v.find_matches("https://www.example.com/", None).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].username.as_deref(), Some("me@example.com"));
    // Saved as a whole-site rule for the page's own site, so a subdomain
    // matches and an unrelated site does not.
    assert_eq!(
        v.find_matches("https://login.example.com/", None)
            .unwrap()
            .len(),
        1
    );
    assert!(v
        .find_matches("https://example.org/", None)
        .unwrap()
        .is_empty());

    // Pages that cannot hold a login are refused before anything is sealed.
    assert_eq!(
        v.stage_save_login("file:///etc/passwd", None, None, secret("x"), None, NOW)
            .err(),
        Some(Error::Denied)
    );
}

/// A2 (writes): an update is only accepted for a login saved for the page.
#[test]
fn save_login_update_is_origin_bound_and_keeps_history() {
    let (mut v, gh) = github_vault();
    let bank = v.find_matches("https://mybank.com/", None).unwrap()[0].id;
    assert_eq!(
        v.stage_save_login(
            "https://github.com/",
            None,
            None,
            secret("x"),
            Some(&bank),
            NOW
        )
        .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_save_login("https://evil.com/", None, None, secret("x"), Some(&gh), NOW)
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.reveal(&bank, SecretField::Password).unwrap().expose(),
        "bank-secret"
    );

    // Origin-bound and naming the right item: sealed, and the stored item is
    // untouched until the server accepts the write.
    let staged = v
        .stage_save_login(
            "https://github.com/",
            None,
            Some("ignored"),
            secret("new-gh"),
            Some(&gh),
            NOW + 1,
        )
        .unwrap();
    assert_eq!(staged.item_id, gh);
    assert_eq!(
        v.reveal(&gh, SecretField::Password).unwrap().expose(),
        "gh-secret"
    );

    v.commit_write(staged.write, 9).unwrap();
    let item = v.get_item(&gh).unwrap();
    // The browser cannot rename the login or change its username; only the
    // password is replaced.
    assert_eq!(item.username.as_deref(), Some("octo"));
    assert!(item.has_totp);
    assert_eq!(
        v.reveal(&gh, SecretField::Password).unwrap().expose(),
        "new-gh"
    );
    assert_eq!(v.password_history(&gh).unwrap().len(), 1);
}

/// Password-history bounding and skip-if-unchanged is a `stage_update`/
/// `commit_write` behavior (see `build_item`), not a `save_login` one;
/// `save_login` always refuses (see `save_login_*` tests above), so this
/// drives the write path it would otherwise have used.
#[test]
fn password_history_is_bounded_and_skips_unchanged() {
    let (mut v, gh) = github_vault();
    let existing = v.get_item(&gh).unwrap();
    let update_password = |v: &mut havenkeys_core::vault::VaultService, pw: &str, now: i64| {
        let input = ItemInput {
            item_type: ItemType::Login,
            title: existing.title.clone(),
            username: existing.username.clone(),
            urls: existing.urls.clone(),
            password: SecretUpdate::Set(secret(pw)),
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
        };
        let staged = v.stage_update(&gh, input, now).unwrap();
        v.commit_write(staged, now).unwrap();
    };
    for i in 0..8 {
        update_password(&mut v, &format!("pw-{i}"), NOW + i);
    }
    // Saving the same password again adds nothing.
    update_password(&mut v, "pw-7", NOW + 100);

    let history = v.password_history(&gh).unwrap();
    assert_eq!(history.len(), havenkeys_core::model::MAX_PASSWORD_HISTORY);
    assert_eq!(history[0], NOW + 7);
    assert_eq!(v.reveal_previous_password(&gh, 0).unwrap().expose(), "pw-6");
}

#[test]
fn save_login_refused_while_locked() {
    let (mut v, gh) = github_vault();
    v.lock();
    assert_eq!(
        v.stage_save_login(
            "https://github.com/",
            None,
            None,
            secret("x"),
            Some(&gh),
            NOW
        )
        .err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.check_login("https://github.com/", None, None, &secret("x"))
            .err(),
        Some(Error::Locked)
    );
}

/// A staged save writes nothing: the server has to accept it first
/// (spec 2026-09-20 §8.4). Neither the store nor the overview cache moves
/// until `commit_write` runs.
#[test]
fn a_staged_save_touches_nothing_until_it_is_committed() {
    let (mut v, gh) = github_vault();
    let before = v.list_items().unwrap().len();

    let created = v
        .stage_save_login(
            "https://github.com/",
            None,
            Some("someone-new"),
            secret("brand-new-pw"),
            None,
            NOW,
        )
        .unwrap();
    let updated = v
        .stage_save_login(
            "https://github.com/",
            None,
            None,
            secret("also-new"),
            Some(&gh),
            NOW,
        )
        .unwrap();
    assert_eq!(updated.item_id, gh);

    // Nothing was created...
    assert_eq!(v.list_items().unwrap().len(), before);
    // ...and the existing item is untouched.
    let item = v.get_item(&gh).unwrap();
    assert_eq!(item.username.as_deref(), Some("octo"));
    assert_eq!(
        v.reveal(&gh, SecretField::Password).unwrap().expose(),
        "gh-secret"
    );
    assert!(v.password_history(&gh).unwrap().is_empty());

    // A lock between staging and committing discards both: the blobs were
    // sealed under a session that no longer exists.
    v.lock();
    assert_eq!(v.commit_write(created.write, 1).err(), Some(Error::Locked));
    assert_eq!(v.commit_write(updated.write, 2).err(), Some(Error::Locked));
}
