//! Export, encrypted backup and restore (spec 2026-10-05-export).

mod common;

use common::*;
use havenkeys_core::export;
use havenkeys_core::Error;

#[test]
fn the_master_password_is_checked_against_this_vault() {
    let (vault, sk) = activated_vault();
    let check = vault.begin_password_check().unwrap();
    check.verify(&secret(PASSWORD), &sk).unwrap();
    assert!(matches!(
        check.verify(&secret("not the password"), &sk),
        Err(Error::UnlockFailed)
    ));
}

#[test]
fn a_locked_vault_gives_no_password_check() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(matches!(vault.begin_password_check(), Err(Error::Locked)));
}

#[test]
fn backup_password_rules() {
    let master = secret(PASSWORD);
    export::check_backup_password(&secret("another long passphrase"), &master).unwrap();
    assert!(matches!(
        export::check_backup_password(&secret("short"), &master),
        Err(Error::InvalidInput(
            "backup password must be at least 10 characters"
        ))
    ));
    assert!(matches!(
        export::check_backup_password(&secret(PASSWORD), &master),
        Err(Error::InvalidInput(
            "backup password must differ from the master password"
        ))
    ));
    let long = "x".repeat(10_000);
    assert!(matches!(
        export::check_backup_password(&secret(&long), &master),
        Err(Error::InvalidInput("backup password is too long"))
    ));
}

use havenkeys_core::export::{ExportFormat, ExportSummary};
use havenkeys_core::vault::VaultService;

fn commit_all(vault: &mut VaultService, inputs: Vec<havenkeys_core::model::ItemInput>) {
    let mut rev = 0;
    for input in inputs {
        rev += 1;
        let w = vault.stage_create(input, NOW).unwrap();
        vault.commit_write(w, rev).unwrap();
    }
}

#[test]
fn summary_counts_what_each_format_leaves_out() {
    let (mut vault, _sk) = activated_vault();
    commit_all(
        &mut vault,
        vec![
            login("GitHub", "octo", "pw1", "https://github.com"),
            login("Bank", "me", "pw2", "https://bank.example"),
            note("Recovery codes", "1111 2222"),
        ],
    );
    let csv = export::summarize(&vault, ExportFormat::Csv).unwrap();
    assert_eq!(
        csv,
        ExportSummary {
            logins: 2,
            secure_notes: 1,
            items_left_out: 1,
            ..Default::default()
        }
    );
    let backup = export::summarize(&vault, ExportFormat::Backup).unwrap();
    assert_eq!((backup.logins, backup.secure_notes, backup.items_left_out), (2, 1, 0));
}

#[test]
fn the_backup_payload_holds_every_item() {
    let (mut vault, _sk) = activated_vault();
    commit_all(&mut vault, vec![login("GitHub", "octo", "pw1", "https://github.com")]);
    let rendered = export::render(&vault, ExportFormat::Backup, NOW).unwrap();
    let text = std::str::from_utf8(&rendered.bytes).unwrap();
    assert!(text.contains("\"version\":1"));
    assert!(text.contains("pw1"));
    assert_eq!(rendered.summary.logins, 1);
}

#[test]
fn a_locked_vault_exports_nothing() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(matches!(export::render(&vault, ExportFormat::Csv, NOW), Err(Error::Locked)));
    assert!(matches!(export::summarize(&vault, ExportFormat::Csv), Err(Error::Locked)));
}

#[test]
fn default_file_names() {
    assert_eq!(
        export::default_file_name(ExportFormat::Backup, NOW),
        "havenkeys-export-2023-11-14.hkbackup"
    );
    assert_eq!(export::default_file_name(ExportFormat::Csv, NOW), "havenkeys-export-2023-11-14.csv");
}

/// One login's details no longer open: export skips it, counts it in
/// `unreadable`, and still exports the rest.
#[test]
fn an_unreadable_item_is_counted_not_fatal() {
    use rusqlite::{params, Connection};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let (damaged, sk) = {
        let (mut v, sk) = activated_vault_at(&path);
        let a = v.stage_create(login("Fine", "a", "pw1", "https://a.example"), NOW).unwrap();
        v.commit_write(a, 1).unwrap();
        let b = v.stage_create(login("Damaged", "b", "pw2", "https://b.example"), NOW + 1).unwrap();
        (v.commit_write(b, 2).unwrap().unwrap().id, sk)
    };
    let c = Connection::open(&path).unwrap();
    let mut blob: Vec<u8> = c
        .query_row("SELECT details FROM items WHERE id = ?1", params![damaged.to_string()], |r| r.get(0))
        .unwrap();
    let last = blob.len() - 1;
    blob[last] ^= 0x01;
    c.execute("UPDATE items SET details = ?1 WHERE id = ?2", params![blob, damaged.to_string()])
        .unwrap();
    drop(c);

    let mut v = open_file(&path);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account()).unwrap();
    let s = export::summarize(&v, ExportFormat::Csv).unwrap();
    assert_eq!((s.logins, s.unreadable), (1, 1));
}

use havenkeys_core::export::backup::{open_backup, seal_backup, HEADER_LEN};

fn backup_of(vault: &VaultService, password: &str) -> Vec<u8> {
    let payload = export::render(vault, ExportFormat::Backup, NOW).unwrap();
    seal_backup(&payload.bytes, &secret(password), &fast_kdf()).unwrap()
}

const BACKUP_PW: &str = "a separate backup passphrase";

#[test]
fn a_backup_opens_with_its_password_only() {
    let (mut vault, _sk) = activated_vault();
    commit_all(&mut vault, vec![login("GitHub", "octo", "pw1", "https://github.com")]);
    let file = backup_of(&vault, BACKUP_PW);
    assert_eq!(&file[..8], b"HKBACKUP");
    // Nothing readable in the file.
    assert!(!file.windows(3).any(|w| w == b"pw1"));
    assert!(!file.windows(6).any(|w| w == b"GitHub"));
    let items = open_backup(&file, &secret(BACKUP_PW)).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].overview.title, "GitHub");
    assert!(matches!(
        open_backup(&file, &secret("wrong backup passphrase")),
        Err(Error::InvalidInput("wrong backup password, or the file is damaged"))
    ));
}

#[test]
fn every_tampered_byte_is_refused() {
    let (vault, _sk) = activated_vault();
    let file = backup_of(&vault, BACKUP_PW);
    // Version (8), KDF algorithm (9), each KDF parameter, the salt and the blob.
    for pos in [8, 9, 10, 14, 18, 22, 37, HEADER_LEN, HEADER_LEN + 2, file.len() - 1] {
        let mut bad = file.clone();
        bad[pos] ^= 0x01;
        assert!(open_backup(&bad, &secret(BACKUP_PW)).is_err(), "byte {pos}");
    }
}

#[test]
fn malformed_backups_are_refused_with_a_fixed_message() {
    let (vault, _sk) = activated_vault();
    let file = backup_of(&vault, BACKUP_PW);
    let not_ours = Error::InvalidInput("not a HavenKeys backup file");
    for bad in [&b""[..], b"HKBACKU", &file[..HEADER_LEN], b"PK\x03\x04 a zip file......................................"] {
        assert_eq!(
            open_backup(bad, &secret(BACKUP_PW)).unwrap_err().to_string(),
            not_ours.to_string()
        );
    }
    let mut newer = file.clone();
    newer[8] = 2;
    assert!(matches!(
        open_backup(&newer, &secret(BACKUP_PW)),
        Err(Error::InvalidInput("this backup was made by a newer version of HavenKeys"))
    ));
    // KDF parameters outside the accepted range (memory = 1 GiB + 1 KiB).
    let mut greedy = file.clone();
    greedy[10..14].copy_from_slice(&(1024 * 1024 + 1u32).to_le_bytes());
    assert!(matches!(
        open_backup(&greedy, &secret(BACKUP_PW)),
        Err(Error::InvalidInput("not a HavenKeys backup file"))
    ));
}

#[test]
fn a_payload_with_unknown_fields_is_refused() {
    let payload = br#"{"version":1,"exportedAt":0,"items":[],"extra":true}"#;
    let file = seal_backup(payload, &secret(BACKUP_PW), &fast_kdf()).unwrap();
    assert!(open_backup(&file, &secret(BACKUP_PW)).is_err());
    let newer = br#"{"version":2,"exportedAt":0,"items":[]}"#;
    let file = seal_backup(newer, &secret(BACKUP_PW), &fast_kdf()).unwrap();
    assert!(matches!(
        open_backup(&file, &secret(BACKUP_PW)),
        Err(Error::InvalidInput("this backup was made by a newer version of HavenKeys"))
    ));
}

#[test]
fn the_size_limit_is_enforced_on_both_sides() {
    use havenkeys_core::crypto::blob::MIN_BLOB_LEN;
    use havenkeys_core::export::backup::MAX_BACKUP_BYTES;
    let largest = MAX_BACKUP_BYTES as usize - HEADER_LEN - MIN_BLOB_LEN;
    let file = seal_backup(&vec![b'x'; largest], &secret(BACKUP_PW), &fast_kdf()).unwrap();
    assert!(file.len() as u64 <= MAX_BACKUP_BYTES);
    assert!(matches!(
        seal_backup(&vec![b'x'; largest + 1], &secret(BACKUP_PW), &fast_kdf()),
        Err(Error::InvalidInput("backup file is too large"))
    ));
    assert!(matches!(
        open_backup(&vec![0u8; MAX_BACKUP_BYTES as usize + 1], &secret(BACKUP_PW)),
        Err(Error::InvalidInput("backup file is too large"))
    ));
}

use havenkeys_core::passkey::PasskeyCreate;

const GH: &str = "https://github.com/login";

fn passkey_req(handle: &[u8]) -> PasskeyCreate<'_> {
    PasskeyCreate {
        rp_id: "github.com",
        page_url: GH,
        top_url: None,
        challenge: &[7; 32],
        user_handle: handle,
        user_name: "octo",
        display_name: None,
        item_id: None,
        conditional: false,
    }
}

/// Stage-and-commit every restore write; returns the report.
fn restore_into(
    vault: &mut VaultService,
    file: &[u8],
    base: i64,
) -> havenkeys_core::import::ImportReport {
    let items = open_backup(file, &secret(BACKUP_PW)).unwrap();
    let staged = vault.stage_restore(items, NOW).unwrap();
    let mut rev = base;
    for w in staged.writes {
        rev += 1;
        vault.commit_write(w, rev).unwrap();
    }
    staged.report
}

#[test]
fn a_backup_restores_into_a_new_account_with_working_passkeys() {
    let (mut a, _) = activated_vault();
    commit_all(&mut a, vec![note("Recovery", "1111"), login("Bank", "me", "pw", "https://bank.example")]);
    let staged = a.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    let pk_item = staged.item_id;
    let cred = staged.registration.credential_id.clone();
    a.commit_write(staged.write, 100).unwrap();
    let file = backup_of(&a, BACKUP_PW);

    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &file, 0);
    assert_eq!((report.imported, report.logins, report.secure_notes, report.failed), (3, 2, 1, 0));
    assert!(b.get_item(&pk_item).unwrap().has_passkey);
    b.passkey_assert(&pk_item, &cred, "github.com", GH, None, &[3; 32]).unwrap();
}

#[test]
fn restoring_twice_adds_nothing_and_overwrites_nothing() {
    let (mut a, _) = activated_vault();
    commit_all(&mut a, vec![login("Bank", "me", "pw", "https://bank.example")]);
    let file = backup_of(&a, BACKUP_PW);
    let (mut b, _) = activated_vault();
    assert_eq!(restore_into(&mut b, &file, 0).imported, 1);
    let second = restore_into(&mut b, &file, 10);
    assert_eq!((second.imported, second.skipped_existing), (0, 1));
    assert_eq!(b.list_items().unwrap().len(), 1);
}

#[test]
fn the_identity_restores_under_this_vaults_id_or_is_skipped() {
    let (mut a, _) = activated_vault();
    let w = a.stage_identity_if_missing("me@example.com", NOW).unwrap().unwrap();
    a.commit_write(w, 1).unwrap();
    let file = backup_of(&a, BACKUP_PW);

    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &file, 0);
    assert_eq!(report.identities, 1);
    assert!(b.get_item(&b.identity_item_id().unwrap()).is_ok());
    let again = restore_into(&mut b, &file, 10);
    assert_eq!((again.identities, again.skipped_existing), (0, 1));
}

use havenkeys_core::import::{self, ImportSource};

#[test]
fn bitwarden_json_reimports_and_never_carries_passkeys() {
    let (mut v, _) = activated_vault();
    let mut gh = login("GitHub", "octo", "pw1", "https://github.com");
    gh.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"));
    commit_all(&mut v, vec![gh, note("Recovery", "1111 2222")]);
    let staged = v.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    v.commit_write(staged.write, 50).unwrap();

    let out = export::render(&v, ExportFormat::BitwardenJson, NOW).unwrap();
    assert_eq!(out.summary.passkeys_left_out, 1);
    let text = std::str::from_utf8(&out.bytes).unwrap();
    assert!(text.contains("\"fido2Credentials\":[]"));
    assert!(!text.contains("privateKey") && !text.contains("keyValue"));

    let parsed = import::parse(ImportSource::BitwardenJson, &out.bytes).unwrap();
    let (mut fresh, _) = activated_vault();
    let staged = fresh.stage_import(parsed.items, parsed.report, NOW).unwrap();
    assert_eq!(staged.report.failed, 0);
    assert_eq!((staged.report.logins, staged.report.secure_notes), (2, 1));
    let mut rev = 0;
    for w in staged.writes {
        rev += 1;
        fresh.commit_write(w, rev).unwrap();
    }
    let github = fresh
        .list_items()
        .unwrap()
        .into_iter()
        .find(|i| i.title == "GitHub")
        .unwrap();
    assert!(github.has_totp);
    assert_eq!(github.username.as_deref(), Some("octo"));
}

/// No plaintext export contains a passkey's private key in any encoding.
#[test]
#[ignore = "enabled in Task 7 when the CSV writer exists"]
fn plaintext_exports_never_contain_passkey_key_material() {
    use data_encoding::{BASE64, BASE64URL, BASE64URL_NOPAD, BASE64_NOPAD, HEXLOWER};
    let (mut v, _) = activated_vault();
    let staged = v.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    v.commit_write(staged.write, 1).unwrap();
    // The raw key, read from the backup payload (the only place it may be).
    let payload = export::render(&v, ExportFormat::Backup, NOW).unwrap();
    let items: serde_json::Value = serde_json::from_slice(&payload.bytes).unwrap();
    let key_text = items["items"][0]["details"]["passkeys"][0]["privateKey"]
        .as_str()
        .unwrap()
        .to_owned();
    let raw = BASE64URL_NOPAD
        .decode(key_text.as_bytes())
        .or_else(|_| BASE64.decode(key_text.as_bytes()))
        .unwrap();
    let encodings = [
        BASE64.encode(&raw),
        BASE64_NOPAD.encode(&raw),
        BASE64URL.encode(&raw),
        BASE64URL_NOPAD.encode(&raw),
        HEXLOWER.encode(&raw),
    ];
    for format in [ExportFormat::BitwardenJson, ExportFormat::Csv] {
        let out = export::render(&v, format, NOW).unwrap();
        assert!(!out.bytes.windows(raw.len()).any(|w| w == raw.as_slice()));
        let text = String::from_utf8_lossy(&out.bytes);
        for e in &encodings {
            assert!(!text.contains(e.as_str()), "{format:?}");
        }
    }
}
