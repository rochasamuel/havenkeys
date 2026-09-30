//! Staged writes: the server accepts them, the replica records them.

mod common;

use common::{account, activated_vault, login, secret, NOW, PASSWORD};
use havenkeys_core::model::SecretField;

#[test]
fn a_staged_create_is_not_visible_until_it_is_committed() {
    let (mut vault, _sk) = activated_vault();
    let staged = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();

    // Nothing is stored yet: the server has not seen it.
    assert!(vault.list_items().unwrap().is_empty());
    assert_eq!(staged.base_revision, None);

    let overview = vault.commit_write(staged, 12).unwrap().unwrap();
    assert_eq!(vault.list_items().unwrap().len(), 1);
    assert_eq!(
        vault
            .reveal(&overview.id, SecretField::Password)
            .unwrap()
            .expose(),
        "pw"
    );
}

#[test]
fn an_update_stages_the_revision_it_last_saw() {
    let (mut vault, _sk) = activated_vault();
    let created = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let id = created.item_id;
    vault.commit_write(created, 12).unwrap();

    let staged = vault
        .stage_update(&id, login("GitHub", "me", "pw2", "github.com"), NOW + 1)
        .unwrap();
    assert_eq!(staged.base_revision, Some(12));
}

#[test]
fn a_staged_delete_carries_no_blobs() {
    let (mut vault, _sk) = activated_vault();
    let created = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    let id = created.item_id;
    vault.commit_write(created, 12).unwrap();

    let staged = vault.stage_delete(&id).unwrap();
    assert!(staged.overview.is_none() && staged.details.is_none());
    assert_eq!(staged.base_revision, Some(12));

    assert!(vault.commit_write(staged, 13).unwrap().is_none());
    assert!(vault.list_items().unwrap().is_empty());
}

#[test]
fn a_write_staged_before_a_lock_is_discarded() {
    let (mut vault, sk) = activated_vault();
    let staged = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    vault.lock();
    vault
        .unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();

    // The blobs were sealed under the previous session; the server may even
    // have accepted them, but this device must not record them blindly.
    assert!(vault.commit_write(staged, 12).is_err());
    assert!(vault.list_items().unwrap().is_empty());
}

#[test]
fn staging_a_write_while_locked_is_refused() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .is_err());
}

// ------------------------------------------------------------------- import

/// An import is a batch of ordinary staged writes: nothing is stored until
/// the server has accepted each one.
#[test]
fn a_staged_import_seals_new_items_and_skips_ones_already_here() {
    use havenkeys_core::import::{ImportReport, ImportedItem};

    let (mut vault, _sk) = activated_vault();
    let existing = vault
        .stage_create(login("GitHub", "me", "pw", "github.com"), NOW)
        .unwrap();
    vault.commit_write(existing, 1).unwrap();

    let staged = vault
        .stage_import(
            vec![
                // The same login as above: already here.
                ImportedItem {
                    input: login("GitHub", "me", "other-pw", "github.com"),
                    created_at: None,
                    updated_at: None,
                },
                ImportedItem {
                    input: login("Fastmail", "me", "pw2", "fastmail.com"),
                    created_at: Some(1_600_000_000_000),
                    updated_at: Some(1_600_000_500_000),
                },
                ImportedItem {
                    input: common::note("Recovery codes", "1234 5678"),
                    created_at: None,
                    updated_at: None,
                },
            ],
            ImportReport::default(),
            NOW,
        )
        .unwrap();

    assert_eq!(staged.report.skipped_duplicates, 1);
    assert_eq!(staged.report.logins, 1);
    assert_eq!(staged.report.secure_notes, 1);
    assert_eq!(staged.report.imported, 2);
    assert_eq!(staged.writes.len(), 2);
    // Still only the original item: an import that is never sent stores
    // nothing.
    assert_eq!(vault.list_items().unwrap().len(), 1);

    let mut revision = 1;
    for write in staged.writes {
        revision += 1;
        vault.commit_write(write, revision).unwrap();
    }
    let titles: Vec<String> = vault
        .list_items()
        .unwrap()
        .iter()
        .map(|i| i.title.clone())
        .collect();
    assert_eq!(titles.len(), 3);
    assert!(titles.iter().any(|t| t == "Fastmail"));
    assert!(titles.iter().any(|t| t == "Recovery codes"));

    // The imported login keeps the timestamps the export carried.
    let items = vault.list_items().unwrap();
    let fastmail = items.iter().find(|i| i.title == "Fastmail").unwrap();
    assert_eq!(fastmail.created_at, 1_600_000_000_000);
    assert_eq!(fastmail.updated_at, 1_600_000_500_000);
}

/// A re-import that now knows about `sign_in_with` (the old importer wrote
/// "Sign in with Google" as a free-text note instead) fills it in on the
/// matching login already in the vault rather than treating it as a
/// duplicate, and clears notes only when they are exactly that old line.
#[test]
fn a_staged_import_upgrades_a_matching_login_with_sign_in_with() {
    use havenkeys_core::import::{ImportReport, ImportedItem};
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use havenkeys_core::sso::{SignInWith, SsoProvider};

    let (mut vault, _sk) = activated_vault();

    let existing_login = |title: &str, url: &str, notes: &str| ItemInput {
        item_type: ItemType::Login,
        title: title.into(),
        username: None,
        urls: vec![UrlRule {
            url: url.into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Set(secret(notes)),
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: None,
    };
    let imported = |title: &str, url: &str| ImportedItem {
        input: ItemInput {
            item_type: ItemType::Login,
            title: title.into(),
            username: None,
            urls: vec![UrlRule {
                url: url.into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Keep,
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: Some(SignInWith {
                provider: SsoProvider::Google,
                account: None,
            }),
            identity: None,
            card: None,
        },
        created_at: None,
        updated_at: None,
    };

    // The old importer's exact note: cleared once sign_in_with takes over.
    let a = vault
        .stage_create(
            existing_login("Typeform", "https://typeform.com", "Sign in with Google"),
            NOW,
        )
        .unwrap();
    let a_id = a.item_id;
    vault.commit_write(a, 1).unwrap();

    let staged = vault
        .stage_import(
            vec![imported("Typeform", "https://typeform.com")],
            ImportReport::default(),
            NOW,
        )
        .unwrap();
    assert_eq!(staged.report.sso_upgraded, 1);
    assert_eq!(staged.report.skipped_duplicates, 0);
    assert_eq!(staged.report.imported, 0);
    assert_eq!(staged.writes.len(), 1);

    let mut revision = 1;
    for write in staged.writes {
        revision += 1;
        vault.commit_write(write, revision).unwrap();
    }
    let item = vault.get_item(&a_id).unwrap();
    assert_eq!(
        item.sign_in_with.as_ref().map(|s| s.provider),
        Some(SsoProvider::Google)
    );
    assert!(!item.has_notes, "the old importer's exact line is cleared");

    // A note that isn't exactly that line is left alone.
    let b = vault
        .stage_create(
            existing_login("Notion", "https://notion.so", "Sign in with Google\nmore"),
            NOW,
        )
        .unwrap();
    let b_id = b.item_id;
    vault.commit_write(b, revision).unwrap();
    revision += 1;

    let staged = vault
        .stage_import(
            vec![imported("Notion", "https://notion.so")],
            ImportReport::default(),
            NOW,
        )
        .unwrap();
    assert_eq!(staged.report.sso_upgraded, 1);
    for write in staged.writes {
        revision += 1;
        vault.commit_write(write, revision).unwrap();
    }
    let item = vault.get_item(&b_id).unwrap();
    assert_eq!(
        item.sign_in_with.as_ref().map(|s| s.provider),
        Some(SsoProvider::Google)
    );
    assert!(item.has_notes, "notes that don't match exactly are kept");
}

/// A locked vault cannot seal anything, import included.
#[test]
fn a_staged_import_needs_an_unlocked_vault() {
    use havenkeys_core::import::ImportReport;

    let (mut vault, _sk) = activated_vault();
    vault.lock();
    let err = vault
        .stage_import(vec![], ImportReport::default(), NOW)
        .unwrap_err();
    assert_eq!(err.code(), "locked");
}

/// Pins how an update treats the main TOTP before `apply_totp` takes over
/// the inline match in `build_item` (spec 2026-09-30 §4.6).
#[test]
fn main_totp_updates_keep_clear_and_set() {
    use havenkeys_core::model::SecretUpdate;
    let (mut v, _) = activated_vault();
    let mut input = login("AWS", "root", "pw", "aws.amazon.com");
    input.totp = SecretUpdate::Set(secret("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"));
    let staged = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    assert_eq!(v.totp_code(&id, 59).unwrap().code.expose(), "287082");

    let with = |totp: SecretUpdate| {
        let mut i = login("AWS", "root", "pw", "aws.amazon.com");
        i.totp = totp;
        i
    };
    // Keep keeps.
    let staged = v
        .stage_update(&id, with(SecretUpdate::Keep), NOW + 1)
        .unwrap();
    v.commit_write(staged, 2).unwrap();
    assert_eq!(v.totp_code(&id, 59).unwrap().code.expose(), "287082");
    // A blank Set clears.
    let staged = v
        .stage_update(&id, with(SecretUpdate::Set(secret("   "))), NOW + 2)
        .unwrap();
    let ov = v.commit_write(staged, 3).unwrap().unwrap();
    assert!(!ov.has_totp);
    // A bad Set is refused without echoing it.
    let err = v
        .stage_update(
            &id,
            with(SecretUpdate::Set(secret("not-base32-SECRETVALUE!"))),
            NOW + 3,
        )
        .unwrap_err();
    assert!(!err.to_string().contains("SECRETVALUE"));
    // Set, then Clear.
    let staged = v
        .stage_update(
            &id,
            with(SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"))),
            NOW + 4,
        )
        .unwrap();
    assert!(v.commit_write(staged, 4).unwrap().unwrap().has_totp);
    let staged = v
        .stage_update(&id, with(SecretUpdate::Clear), NOW + 5)
        .unwrap();
    assert!(!v.commit_write(staged, 5).unwrap().unwrap().has_totp);
}
