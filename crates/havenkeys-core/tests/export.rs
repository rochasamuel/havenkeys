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
