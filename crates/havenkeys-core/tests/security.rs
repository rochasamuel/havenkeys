//! Security regression tests (docs/threat-model.md §5).

mod common;

use common::*;
use havenkeys_core::model::{ItemInput, ItemType, SecretField, SecretUpdate, Settings};
use havenkeys_core::sso::SsoProvider;
use havenkeys_core::vault::{ProviderLogin, SaveAction, SaveTarget, VaultState};
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
        |user: Option<&str>, pw: &str| v.check_login(page, None, user, &secret(pw), None).unwrap();
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
            &secret("gh-secret"),
            None
        )
        .unwrap(),
        SaveAction::Add
    );
    assert!(v
        .check_login(page, None, Some("octo"), &secret(""), None)
        .is_err());
}

/// A change-password form usually has no username field: the current
/// password the user entered picks the login to update.
#[test]
fn check_login_finds_the_changed_login_by_its_current_password() {
    let (mut v, gh) = github_vault();
    let page = "https://github.com/settings/security";
    let check =
        |v: &havenkeys_core::vault::VaultService, user: Option<&str>, new: &str, current: &str| {
            v.check_login(page, None, user, &secret(new), Some(&secret(current)))
                .unwrap()
        };
    assert_eq!(
        check(&v, None, "new-secret", "gh-secret"),
        SaveAction::Update(gh)
    );
    assert_eq!(
        check(&v, Some("OCTO"), "new-secret", "gh-secret"),
        SaveAction::Update(gh)
    );
    // The new password is already saved: nothing to offer.
    assert_eq!(
        check(&v, None, "gh-secret", "gh-secret"),
        SaveAction::Unchanged
    );
    // A current password that is not saved for this page updates nothing.
    assert_eq!(check(&v, None, "new-secret", "wrong"), SaveAction::Add);
    assert_eq!(
        check(&v, None, "new-secret", "bank-secret"),
        SaveAction::Add
    );
    // Another user's form never updates octo's login.
    assert_eq!(
        check(&v, Some("someone-else"), "new-secret", "gh-secret"),
        SaveAction::Add
    );
    // Another site never updates the github.com login.
    assert_eq!(
        v.check_login(
            "https://evil.com/",
            None,
            None,
            &secret("new-secret"),
            Some(&secret("gh-secret"))
        )
        .unwrap(),
        SaveAction::Add
    );
    // Two logins for the page share the current password: too ambiguous to pick one.
    let staged = v
        .stage_create(
            login("GitHub work", "octo-work", "gh-secret", "github.com"),
            NOW,
        )
        .unwrap();
    v.commit_write(staged, 3).unwrap();
    assert_eq!(check(&v, None, "new-secret", "gh-secret"), SaveAction::Add);
    // With the username given, the ambiguity is gone.
    assert_eq!(
        check(&v, Some("octo"), "new-secret", "gh-secret"),
        SaveAction::Update(gh)
    );
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
            SaveTarget::New { title: None },
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
        v.stage_save_login(
            "file:///etc/passwd",
            None,
            None,
            secret("x"),
            SaveTarget::New { title: None },
            NOW
        )
        .err(),
        Some(Error::Denied)
    );
}

/// The title the user gave in the save prompt; without one, the host.
#[test]
fn save_login_takes_the_prompts_title() {
    let (mut v, _) = github_vault();
    let page = "https://www.example.com/signin";
    let title_of = |v: &mut havenkeys_core::vault::VaultService, title: Option<&str>| {
        let staged = v
            .stage_save_login(
                page,
                None,
                Some("me"),
                secret("pw"),
                SaveTarget::New { title },
                NOW,
            )
            .unwrap();
        let id = staged.item_id;
        v.commit_write(staged.write, 7).unwrap();
        v.find_matches(page, None)
            .unwrap()
            .into_iter()
            .find(|m| m.id == id)
            .unwrap()
            .title
    };
    assert_eq!(
        title_of(&mut v, Some("  Example — work ")),
        "Example — work"
    );
    assert_eq!(title_of(&mut v, None), "example.com");
    let long = "x".repeat(257);
    for bad in ["", "   ", "a\nb", long.as_str()] {
        assert!(matches!(
            v.stage_save_login(
                page,
                None,
                None,
                secret("pw"),
                SaveTarget::New { title: Some(bad) },
                NOW
            ),
            Err(Error::InvalidInput(_))
        ));
    }
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
            SaveTarget::Update(&bank),
            NOW
        )
        .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_save_login(
            "https://evil.com/",
            None,
            None,
            secret("x"),
            SaveTarget::Update(&gh),
            NOW
        )
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
            SaveTarget::Update(&gh),
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
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: None,
            sections: None,
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
            SaveTarget::Update(&gh),
            NOW
        )
        .err(),
        Some(Error::Locked)
    );
    assert_eq!(
        v.check_login("https://github.com/", None, None, &secret("x"), None)
            .err(),
        Some(Error::Locked)
    );
}

fn google_login(title: &str, account: Option<&str>, url: &str) -> havenkeys_core::model::ItemInput {
    let mut input = login(title, "", "", url);
    input.username = None;
    input.password = havenkeys_core::model::SecretUpdate::Keep;
    input.sign_in_with = Some(havenkeys_core::sso::SignInWith {
        provider: havenkeys_core::sso::SsoProvider::Google,
        account: account.map(str::to_owned),
    });
    input
}

#[test]
fn sign_in_with_round_trips_and_is_searchable() {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(
            google_login("Typeform", Some(" me@gmail.com "), "typeform.com"),
            NOW,
        )
        .unwrap();
    let ov = v.commit_write(staged, 5).unwrap().unwrap();
    let s = ov.sign_in_with.as_ref().unwrap();
    assert_eq!(s.account.as_deref(), Some("me@gmail.com"));
    assert!(!ov.has_password);
    assert_eq!(v.search("me@gmail").unwrap().len(), 1);
    assert_eq!(v.search("google").unwrap().len(), 1);
}

#[test]
fn password_update_keeps_sign_in_with() {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(
            google_login("Typeform", Some("me@gmail.com"), "typeform.com"),
            NOW,
        )
        .unwrap();
    let id = v.commit_write(staged, 5).unwrap().unwrap().id;
    let staged = v
        .stage_save_login(
            "https://typeform.com/",
            None,
            None,
            secret("also-a-password"),
            SaveTarget::Update(&id),
            NOW,
        )
        .unwrap();
    let ov = v.commit_write(staged.write, 6).unwrap().unwrap();
    assert!(ov.has_password);
    assert_eq!(
        ov.sign_in_with.as_ref().unwrap().account.as_deref(),
        Some("me@gmail.com")
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
            SaveTarget::New { title: None },
            NOW,
        )
        .unwrap();
    let updated = v
        .stage_save_login(
            "https://github.com/",
            None,
            None,
            secret("also-new"),
            SaveTarget::Update(&gh),
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

/// Automatic sign-in needs both the vault setting and the login's switch;
/// an edit that does not mention the switch keeps it.
#[test]
fn auto_sign_in_needs_the_setting_and_the_login_switch() {
    let (mut v, gh) = github_vault();
    assert!(v.auto_sign_in_for(&gh).unwrap(), "both default on");

    let edit = |title: &str, switch: Option<bool>| {
        let mut input = login(title, "octo", "gh-secret", "github.com");
        input.password = SecretUpdate::Keep;
        input.auto_sign_in = switch;
        input
    };
    let staged = v
        .stage_update(&gh, edit("GitHub", Some(false)), NOW)
        .unwrap();
    v.commit_write(staged, 3).unwrap();
    assert!(!v.auto_sign_in_for(&gh).unwrap());

    let staged = v
        .stage_update(&gh, edit("GitHub (work)", None), NOW)
        .unwrap();
    v.commit_write(staged, 4).unwrap();
    assert!(!v.auto_sign_in_for(&gh).unwrap(), "None keeps the switch");

    let staged = v
        .stage_update(&gh, edit("GitHub", Some(true)), NOW)
        .unwrap();
    v.commit_write(staged, 5).unwrap();
    assert!(v.auto_sign_in_for(&gh).unwrap());

    v.update_settings(Settings {
        auto_sign_in: false,
        ..v.settings().unwrap()
    })
    .unwrap();
    assert!(!v.auto_sign_in_for(&gh).unwrap(), "global off wins");

    assert_eq!(
        v.auto_sign_in_for(&Uuid::new_v4()).err(),
        Some(Error::NotFound)
    );
    v.lock();
    assert_eq!(v.auto_sign_in_for(&gh).err(), Some(Error::Locked));
}

fn sso_vault() -> (havenkeys_core::vault::VaultService, Uuid) {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(
            google_login("Typeform", Some("me@gmail.com"), "typeform.com"),
            NOW,
        )
        .unwrap();
    let id = v.commit_write(staged, 5).unwrap().unwrap().id;
    (v, id)
}

#[test]
fn start_sso_is_origin_bound_and_returns_no_secret() {
    let (mut v, id) = sso_vault();
    let s = v
        .start_sso_for_page(&id, "https://admin.typeform.com/login", None)
        .unwrap();
    assert_eq!(s.provider, SsoProvider::Google);
    assert_eq!(s.account.as_deref(), Some("me@gmail.com"));
    assert!(s.auto_choose);
    for page in [
        "https://evil.com/",
        "https://typeform.com.evil.com/",
        "http://typeform.com/",
        "javascript:x",
    ] {
        assert_eq!(
            v.start_sso_for_page(&id, page, None).err(),
            Some(Error::Denied),
            "{page}"
        );
    }
    // A frame of typeform.com embedded in evil.com gets nothing.
    assert_eq!(
        v.start_sso_for_page(&id, "https://typeform.com/", Some("https://evil.com/"))
            .err(),
        Some(Error::Denied)
    );
    // A login without sign_in_with is not an SSO item.
    let gh = v.find_matches("https://github.com/", None).unwrap()[0].id;
    assert_eq!(
        v.start_sso_for_page(&gh, "https://github.com/", None).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.start_sso_for_page(&Uuid::new_v4(), "https://typeform.com/", None)
            .err(),
        Some(Error::NotFound)
    );
    v.lock();
    assert_eq!(
        v.start_sso_for_page(&id, "https://typeform.com/", None)
            .err(),
        Some(Error::Locked)
    );
}

#[test]
fn start_sso_auto_choose_follows_both_switches() {
    let (mut v, id) = sso_vault();
    let mut settings = v.settings().unwrap();
    settings.auto_sign_in = false;
    v.update_settings(settings).unwrap();
    assert!(
        !v.start_sso_for_page(&id, "https://typeform.com/", None)
            .unwrap()
            .auto_choose
    );
}

#[test]
fn find_matches_reports_provider_and_account() {
    let (v, id) = sso_vault();
    let m = v.find_matches("https://typeform.com/", None).unwrap();
    let s = m.iter().find(|s| s.id == id).unwrap();
    assert_eq!(s.provider, Some(SsoProvider::Google));
    assert_eq!(s.account.as_deref(), Some("me@gmail.com"));
    assert_eq!(s.username, None);
}

#[test]
fn check_sso_actions() {
    let (mut v, id) = sso_vault();
    let page = "https://typeform.com/";
    let check =
        |v: &havenkeys_core::vault::VaultService, p, a| v.check_sso(page, None, p, a).unwrap();
    assert_eq!(
        check(&v, SsoProvider::Google, Some("ME@gmail.com")),
        SaveAction::Unchanged
    );
    assert_eq!(check(&v, SsoProvider::Google, None), SaveAction::Unchanged);
    assert_eq!(
        check(&v, SsoProvider::Google, Some("other@gmail.com")),
        SaveAction::Add
    );
    assert_eq!(check(&v, SsoProvider::Github, None), SaveAction::Add);
    // An imported login with the provider but no account is offered the account.
    let staged = v
        .stage_create(google_login("Notion", None, "notion.so"), NOW)
        .unwrap();
    let notion = v.commit_write(staged, 6).unwrap().unwrap().id;
    assert_eq!(
        v.check_sso(
            "https://notion.so/",
            None,
            SsoProvider::Google,
            Some("me@gmail.com")
        )
        .unwrap(),
        SaveAction::Update(notion)
    );
    let _ = id;
}

#[test]
fn provider_accounts_are_the_usernames_the_vault_matches_to_the_provider() {
    let (mut v, _) = sso_vault();
    for (rev, input) in (7..).zip([
        login("Google", " Me@Gmail.com ", "pw", "google.com"),
        login(
            "Google (work)",
            "srocha@callix.com.br",
            "pw",
            "https://accounts.google.com",
        ),
        login("Google again", "me@gmail.com", "pw", "google.com"),
        login("Google, no username", "", "pw", "google.com"),
        login("Look-alike", "evil@x.com", "pw", "google.com.evil.com"),
    ]) {
        let staged = v.stage_create(input, NOW).unwrap();
        v.commit_write(staged, rev).unwrap();
    }
    // Sorted, trimmed, lowercased, deduplicated; the Typeform "Sign in with"
    // login (not saved for Google's page) and the look-alike are not included.
    assert_eq!(
        v.provider_accounts(SsoProvider::Google).unwrap(),
        vec!["me@gmail.com".to_owned(), "srocha@callix.com.br".to_owned()]
    );
    assert!(v.provider_accounts(SsoProvider::Apple).unwrap().is_empty());
    v.lock();
    assert_eq!(
        v.provider_accounts(SsoProvider::Google).err(),
        Some(Error::Locked)
    );
}

#[test]
fn sso_accounts_are_the_logins_the_vault_matches_to_the_provider() {
    let (mut v, _) = sso_vault();
    for (rev, input) in (7..).zip([
        login("Google", " Me@Gmail.com ", "pw", "google.com"),
        login(
            "Google (work)",
            "srocha@callix.com.br",
            "pw",
            "https://accounts.google.com",
        ),
        login("Google, no username", "", "pw", "google.com"),
        login("Look-alike", "evil@x.com", "pw", "google.com.evil.com"),
        login("Discord", "samuel", "pw", "discord.com"),
    ]) {
        let staged = v.stage_create(input, NOW).unwrap();
        v.commit_write(staged, rev).unwrap();
    }
    let google = v.sso_accounts(SsoProvider::Google).unwrap();
    let shown: Vec<(&str, &str)> = google
        .iter()
        .map(|a| (a.title.as_str(), a.username.as_str()))
        .collect();
    // Sorted by title; username trimmed but its case kept; no empty username,
    // no look-alike, no Typeform "Sign in with" login (saved for typeform.com).
    assert_eq!(
        shown,
        [
            ("Google", "Me@Gmail.com"),
            ("Google (work)", "srocha@callix.com.br")
        ]
    );
    assert_eq!(v.sso_accounts(SsoProvider::Discord).unwrap().len(), 1);
    assert!(v.sso_accounts(SsoProvider::Gitlab).unwrap().is_empty());
    // The save prompt's account list is built on the same rule.
    assert_eq!(
        v.provider_accounts(SsoProvider::Google).unwrap(),
        vec!["me@gmail.com".to_owned(), "srocha@callix.com.br".to_owned()]
    );
    // Never a secret, never the username in Debug output.
    let json = serde_json::to_string(&google[0]).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, ["id", "title", "username"]);
    assert!(!format!("{:?}", google[0]).contains("Gmail"));
    v.lock();
    assert_eq!(
        v.sso_accounts(SsoProvider::Google).err(),
        Some(Error::Locked)
    );
}

#[test]
fn sso_accounts_are_capped() {
    let (mut v, _) = sso_vault();
    for i in 0..(havenkeys_core::vault::MAX_SSO_PICKER_ACCOUNTS + 5) {
        let staged = v
            .stage_create(
                login(
                    &format!("G{i:03}"),
                    &format!("u{i}@gmail.com"),
                    "pw",
                    "google.com",
                ),
                NOW,
            )
            .unwrap();
        v.commit_write(staged, 10 + i as i64).unwrap();
    }
    assert_eq!(
        v.sso_accounts(SsoProvider::Google).unwrap().len(),
        havenkeys_core::vault::MAX_SSO_PICKER_ACCOUNTS
    );
}

#[test]
fn provider_login_is_the_one_login_a_run_would_use() {
    let (mut v, typeform) = sso_vault(); // Typeform signs in with Google as me@gmail.com
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::None);

    let staged = v
        .stage_create(login("Google", " ME@gmail.com ", "pw", "google.com"), NOW)
        .unwrap();
    let google = v.commit_write(staged, 7).unwrap().unwrap().id;
    assert_eq!(
        v.provider_login(&typeform).unwrap(),
        ProviderLogin::One(google)
    );

    let staged = v
        .stage_create(
            login(
                "Google 2",
                "me@gmail.com",
                "pw",
                "https://accounts.google.com",
            ),
            NOW,
        )
        .unwrap();
    let second = v.commit_write(staged, 8).unwrap().unwrap().id;
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::Several);

    // Deleting the extra one makes the link unambiguous again; deleting the
    // last leaves no link.
    let staged = v.stage_delete(&second).unwrap();
    v.commit_write(staged, 9).unwrap();
    assert_eq!(
        v.provider_login(&typeform).unwrap(),
        ProviderLogin::One(google)
    );
    let staged = v.stage_delete(&google).unwrap();
    v.commit_write(staged, 10).unwrap();
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::None);

    // A login without "Sign in with" has no provider login.
    let plain = plain_login(&mut v);
    assert_eq!(v.provider_login(&plain).unwrap(), ProviderLogin::None);
    assert_eq!(
        v.provider_login(&Uuid::new_v4()).err(),
        Some(Error::NotFound)
    );
    assert_eq!(
        serde_json::to_string(&ProviderLogin::One(typeform)).unwrap(),
        format!(r#"{{"kind":"one","id":"{typeform}"}}"#)
    );
    assert_eq!(
        serde_json::to_string(&ProviderLogin::Several).unwrap(),
        r#"{"kind":"several"}"#
    );
    v.lock();
    assert_eq!(v.provider_login(&typeform).err(), Some(Error::Locked));
}

fn plain_login(v: &mut havenkeys_core::vault::VaultService) -> Uuid {
    let staged = v
        .stage_create(login("Plain", "plain", "pw", "plain.example"), NOW)
        .unwrap();
    v.commit_write(staged, 50).unwrap().unwrap().id
}

#[test]
fn save_sso_adds_without_a_password_and_updates_only_the_account() {
    let (mut v, _) = sso_vault();
    let staged = v
        .stage_save_sso(
            "https://www.canva.com/login",
            None,
            SsoProvider::Apple,
            Some("me@icloud.com"),
            SaveTarget::New {
                title: Some("Canva"),
            },
            NOW,
        )
        .unwrap();
    let ov = v.commit_write(staged.write, 7).unwrap().unwrap();
    assert_eq!(ov.title, "Canva");
    assert!(!ov.has_password);
    assert_eq!(ov.urls[0].url, "https://www.canva.com/");
    assert_eq!(
        ov.sign_in_with.as_ref().unwrap().provider,
        SsoProvider::Apple
    );

    let staged = v
        .stage_create(google_login("Notion", None, "notion.so"), NOW)
        .unwrap();
    let notion = v.commit_write(staged, 8).unwrap().unwrap().id;
    let staged = v
        .stage_save_sso(
            "https://notion.so/",
            None,
            SsoProvider::Google,
            Some("me@gmail.com"),
            SaveTarget::Update(&notion),
            NOW,
        )
        .unwrap();
    let ov = v.commit_write(staged.write, 9).unwrap().unwrap();
    assert_eq!(ov.title, "Notion");
    assert_eq!(
        ov.sign_in_with.as_ref().unwrap().account.as_deref(),
        Some("me@gmail.com")
    );

    // Another site's login, or a different provider, cannot be touched.
    assert_eq!(
        v.stage_save_sso(
            "https://evil.com/",
            None,
            SsoProvider::Google,
            Some("x@y.z"),
            SaveTarget::Update(&notion),
            NOW
        )
        .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_save_sso(
            "https://notion.so/",
            None,
            SsoProvider::Github,
            Some("x"),
            SaveTarget::Update(&notion),
            NOW
        )
        .err(),
        Some(Error::Denied)
    );
    assert!(matches!(
        v.stage_save_sso(
            "https://notion.so/",
            None,
            SsoProvider::Google,
            Some("a\u{7}b"),
            SaveTarget::New { title: None },
            NOW
        ),
        Err(Error::InvalidInput(_))
    ));
}

/// Updating a "Sign in with" login only fills in an account; saving none
/// over it would erase the one it has.
#[test]
fn save_sso_update_needs_an_account() {
    let (v, id) = sso_vault();
    for account in [None, Some(""), Some("   ")] {
        assert!(
            matches!(
                v.stage_save_sso(
                    "https://typeform.com/",
                    None,
                    SsoProvider::Google,
                    account,
                    SaveTarget::Update(&id),
                    NOW
                ),
                Err(Error::InvalidInput(_))
            ),
            "{account:?}"
        );
    }
    let item = v.get_item(&id).unwrap();
    assert_eq!(
        item.sign_in_with.as_ref().unwrap().account.as_deref(),
        Some("me@gmail.com")
    );
}

/// A re-import whose upgrade target no longer opens counts as one failure;
/// the other items are still staged.
#[test]
fn a_failing_import_upgrade_does_not_abort_the_import() {
    use havenkeys_core::import::{ImportReport, ImportedItem};
    use havenkeys_core::sso::SignInWith;

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
    let mut upgrade = login("GitHub", "octo", "pw", "github.com");
    upgrade.sign_in_with = Some(SignInWith {
        provider: SsoProvider::Google,
        account: None,
    });
    let items = vec![
        ImportedItem {
            input: upgrade,
            created_at: None,
            updated_at: None,
        },
        ImportedItem {
            input: login("Notion", "me", "pw2", "notion.so"),
            created_at: None,
            updated_at: None,
        },
    ];
    let staged = v.stage_import(items, ImportReport::default(), NOW).unwrap();
    assert_eq!(staged.report.failed, 1);
    assert_eq!(staged.report.sso_upgraded, 0);
    assert_eq!(staged.report.logins, 1);
    assert_eq!(staged.writes.len(), 1);
}

#[test]
fn save_sso_new_is_bound_to_the_frame_and_checks_the_title() {
    let (mut v, _) = sso_vault();
    let new = |v: &havenkeys_core::vault::VaultService, page: &str, top: Option<&str>, title| {
        v.stage_save_sso(
            page,
            top,
            SsoProvider::Google,
            Some("me@gmail.com"),
            SaveTarget::New { title },
            NOW,
        )
    };
    assert_eq!(
        new(&v, "file:///etc/passwd", None, None).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        new(&v, "https://www.example.com/", Some("not a url"), None).err(),
        Some(Error::Denied)
    );
    assert!(matches!(
        new(&v, "https://www.example.com/", None, Some("   ")),
        Err(Error::InvalidInput(_))
    ));
    let staged = new(&v, "https://www.example.com/x", None, None).unwrap();
    assert_eq!(staged.item_id, staged.write.item_id);
    let ov = v.commit_write(staged.write, 7).unwrap().unwrap();
    assert_eq!(ov.title, "example.com");
    assert_eq!(ov.username, None);
    assert!(!ov.has_password && !ov.has_totp && !ov.has_notes);
    assert!(ov.auto_sign_in);
    assert_eq!(ov.urls.len(), 1);
    assert_eq!(ov.urls[0].url, "https://www.example.com/");
    assert_eq!(
        ov.urls[0].match_type,
        havenkeys_core::model::MatchType::Domain
    );
    let sso = ov.sign_in_with.as_ref().unwrap();
    assert_eq!(sso.provider, SsoProvider::Google);
    assert_eq!(sso.account.as_deref(), Some("me@gmail.com"));
}

/// Check order: a missing account beats the origin check; the lock beats
/// everything but the empty-password check in stage_save_login. An update
/// keeps everything but the account.
#[test]
fn save_check_order_is_pinned() {
    let (mut v, id) = sso_vault();
    assert!(matches!(
        v.stage_save_sso(
            "https://evil.com/",
            None,
            SsoProvider::Google,
            None,
            SaveTarget::Update(&id),
            NOW
        ),
        Err(Error::InvalidInput(_))
    ));
    let before = v.get_item(&id).unwrap();
    let staged = v
        .stage_save_sso(
            "https://typeform.com/",
            None,
            SsoProvider::Google,
            Some("new@gmail.com"),
            SaveTarget::Update(&id),
            NOW,
        )
        .unwrap();
    assert_eq!(staged.item_id, id);
    let ov = v.commit_write(staged.write, 9).unwrap().unwrap();
    assert_eq!(
        (
            ov.title.as_str(),
            ov.username.as_deref(),
            ov.urls.len(),
            ov.auto_sign_in
        ),
        (
            before.title.as_str(),
            before.username.as_deref(),
            before.urls.len(),
            before.auto_sign_in
        )
    );
    assert_eq!(
        ov.sign_in_with.as_ref().unwrap().account.as_deref(),
        Some("new@gmail.com")
    );
    v.lock();
    assert_eq!(
        v.stage_save_sso(
            "file:///x",
            None,
            SsoProvider::Google,
            Some("a"),
            SaveTarget::New { title: None },
            NOW
        )
        .err(),
        Some(Error::Locked)
    );
    assert!(matches!(
        v.stage_save_login(
            "file:///x",
            None,
            None,
            secret(""),
            SaveTarget::New { title: None },
            NOW
        ),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(
        v.stage_save_login(
            "file:///x",
            None,
            None,
            secret("x"),
            SaveTarget::New { title: None },
            NOW
        )
        .err(),
        Some(Error::Locked)
    );
}
