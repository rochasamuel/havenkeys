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
