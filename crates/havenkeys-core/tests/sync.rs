//! Secret Key and folder sync (docs/sync.md, docs/crypto.md).
//!
//! Two or three "devices" share one temporary folder, exactly as two
//! computers share a OneDrive/Dropbox folder.

mod common;

use common::*;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::model::SecretField;
use havenkeys_core::store::{KeyScheme, Store};
use havenkeys_core::sync::{folder, prepare_join, SyncInput};
use havenkeys_core::vault::{prepare_new_vault_with_secret_key, VaultService};
use havenkeys_core::Error;
use std::path::Path;
use uuid::Uuid;

struct Device {
    id: Uuid,
    vault: VaultService,
}

fn sk_vault(sk: &SecretKey) -> VaultService {
    let mut v = VaultService::new(Store::open_in_memory().unwrap());
    v.create_vault(
        prepare_new_vault_with_secret_key(&secret(PASSWORD), sk, fast_kdf(), NOW).unwrap(),
    )
    .unwrap();
    v
}

fn dir_of(root: &Path, v: &VaultService) -> std::path::PathBuf {
    folder::vault_dir(root, v.vault_id().unwrap().unwrap())
}

/// One full sync round for a device: read the folder, merge, write back.
fn sync(root: &Path, d: &mut Device, now: i64) -> havenkeys_core::sync::SyncReport {
    let dir = dir_of(root, &d.vault);
    let input = folder::read(&dir, d.id).unwrap();
    let out = d.vault.sync(d.id, input, now).unwrap();
    folder::write(&dir, d.id, out.header.as_deref(), &out.snapshot).unwrap();
    out.report
}

fn join(root: &Path, vault_id: Uuid, password: &str, sk: &SecretKey) -> Result<Device, Error> {
    let header = folder::read_header(&folder::vault_dir(root, vault_id)).unwrap();
    let prepared = prepare_join(&header, &secret(password), sk)?;
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(prepared)?;
    Ok(Device {
        id: Uuid::new_v4(),
        vault,
    })
}

fn titles(v: &VaultService) -> Vec<String> {
    v.list_items()
        .unwrap()
        .into_iter()
        .map(|i| i.title.clone())
        .collect()
}

fn setup() -> (tempfile::TempDir, SecretKey, Device, Device) {
    let root = tempfile::tempdir().unwrap();
    let sk = SecretKey::generate().unwrap();
    let mut a = Device {
        id: Uuid::new_v4(),
        vault: sk_vault(&sk),
    };
    a.vault
        .create_item(login("GitHub", "octo", "gh-pw", "github.com"), NOW)
        .unwrap();
    sync(root.path(), &mut a, NOW);
    let sk_again = SecretKey::parse(sk.to_text().expose()).unwrap();
    let mut b = join(
        root.path(),
        a.vault.vault_id().unwrap().unwrap(),
        PASSWORD,
        &sk_again,
    )
    .unwrap();
    sync(root.path(), &mut b, NOW);
    (root, sk, a, b)
}

// ------------------------------------------------------------------ Secret Key

#[test]
fn secret_key_is_required_and_checked() {
    let sk = SecretKey::generate().unwrap();
    let mut v = sk_vault(&sk);
    v.lock();
    let ticket = v.begin_unlock().unwrap();
    assert!(ticket.needs_secret_key());
    // Password alone: refused before any key derivation.
    let r = ticket.derive(&secret(PASSWORD));
    assert_eq!(r.as_ref().err(), Some(&Error::SecretKeyRequired));
    assert!(v.finish_unlock(ticket, r).is_err());

    let other = SecretKey::generate().unwrap();
    let ticket = v.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&other));
    assert_eq!(v.finish_unlock(ticket, r), Err(Error::UnlockFailed));

    let ticket = v.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret("wrong password!"), Some(&sk));
    assert_eq!(v.finish_unlock(ticket, r), Err(Error::UnlockFailed));

    let ticket = v.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&sk));
    v.finish_unlock(ticket, r).unwrap();
    assert!(v.is_unlocked());
}

#[test]
fn upgrading_a_password_only_vault_keeps_items() {
    let mut v = new_vault(); // key scheme 1
    let id = v
        .create_item(login("Bank", "me", "bank-pw", "bank.com"), NOW)
        .unwrap()
        .id;
    let sk = SecretKey::generate().unwrap();
    let ticket = v.begin_rekey().unwrap();
    assert_eq!(ticket.key_scheme(), KeyScheme::PasswordOnly);
    let r = ticket.derive_secret_key_upgrade(&secret(PASSWORD), &sk, fast_kdf());
    v.commit_rekey(ticket, r).unwrap();
    assert_eq!(
        v.key_scheme().unwrap(),
        Some(KeyScheme::PasswordAndSecretKey)
    );

    v.lock();
    assert_eq!(v.unlock(&secret(PASSWORD)), Err(Error::SecretKeyRequired));
    let ticket = v.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&sk));
    v.finish_unlock(ticket, r).unwrap();
    assert_eq!(
        v.reveal(&id, SecretField::Password).unwrap().expose(),
        "bank-pw"
    );

    // No second upgrade.
    let ticket = v.begin_rekey().unwrap();
    assert!(ticket
        .derive_secret_key_upgrade(&secret(PASSWORD), &sk, fast_kdf())
        .is_err());
}

#[test]
fn password_change_keeps_the_secret_key() {
    let sk = SecretKey::generate().unwrap();
    let mut v = sk_vault(&sk);
    let ticket = v.begin_rekey().unwrap();
    let r = ticket.derive_with_secret_key(
        &secret(PASSWORD),
        &secret("a new master pw"),
        fast_kdf(),
        Some(&sk),
    );
    v.commit_rekey(ticket, r).unwrap();
    v.lock();
    let ticket = v.begin_unlock().unwrap();
    assert!(ticket.needs_secret_key());
    let r = ticket.derive_with_secret_key(&secret("a new master pw"), Some(&sk));
    v.finish_unlock(ticket, r).unwrap();
}

// ------------------------------------------------------------------ sync

#[test]
fn a_new_device_joins_with_password_and_secret_key_only() {
    let (root, sk, a, b) = setup();
    assert_eq!(titles(&b.vault), vec!["GitHub"]);
    let id = b.vault.list_items().unwrap()[0].id;
    assert_eq!(
        b.vault.reveal(&id, SecretField::Password).unwrap().expose(),
        "gh-pw"
    );

    let vault_id = a.vault.vault_id().unwrap().unwrap();
    assert_eq!(folder::list_vaults(root.path()), vec![vault_id]);
    // The folder alone plus the password is not enough.
    let wrong_sk = SecretKey::generate().unwrap();
    assert_eq!(
        join(root.path(), vault_id, PASSWORD, &wrong_sk).err(),
        Some(Error::UnlockFailed)
    );
    assert_eq!(
        join(root.path(), vault_id, "not the password", &sk).err(),
        Some(Error::UnlockFailed)
    );
}

#[test]
fn nothing_in_the_folder_is_plaintext() {
    let (root, _sk, a, _b) = setup();
    let dir = dir_of(root.path(), &a.vault);
    let mut blob = Vec::new();
    for entry in walk(&dir) {
        blob.extend(std::fs::read(entry).unwrap());
    }
    let text = String::from_utf8_lossy(&blob);
    for secret in ["gh-pw", "octo", "GitHub", "github.com"] {
        assert!(!text.contains(secret), "{secret} found in the sync folder");
    }
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn edits_and_deletes_flow_both_ways() {
    let (root, _sk, mut a, mut b) = setup();
    let gh = a.vault.list_items().unwrap()[0].id;

    // B edits, A picks it up.
    let mut input = login("GitHub (work)", "octo", "gh-pw-2", "github.com");
    input.title = "GitHub (work)".into();
    b.vault.update_item(&gh, input, NOW + 10).unwrap();
    b.vault
        .create_item(login("Mail", "me", "mail-pw", "mail.com"), NOW + 11)
        .unwrap();
    sync(root.path(), &mut b, NOW + 12);
    let r = sync(root.path(), &mut a, NOW + 13);
    assert_eq!((r.added, r.updated, r.deleted), (1, 1, 0));
    assert_eq!(titles(&a.vault), vec!["GitHub (work)", "Mail"]);
    assert_eq!(
        a.vault.reveal(&gh, SecretField::Password).unwrap().expose(),
        "gh-pw-2"
    );

    // A deletes, B loses it.
    a.vault.delete_item(&gh, NOW + 20).unwrap();
    sync(root.path(), &mut a, NOW + 21);
    let r = sync(root.path(), &mut b, NOW + 22);
    assert_eq!(r.deleted, 1);
    assert_eq!(titles(&b.vault), vec!["Mail"]);

    // Stable: another round changes nothing.
    let r = sync(root.path(), &mut a, NOW + 30);
    assert_eq!((r.added, r.updated, r.deleted), (0, 0, 0));
}

#[test]
fn newest_edit_wins_and_an_edit_after_delete_survives() {
    let (root, _sk, mut a, mut b) = setup();
    let gh = a.vault.list_items().unwrap()[0].id;
    // Concurrent edits: B's is later.
    a.vault
        .update_item(&gh, login("From A", "octo", "a", "github.com"), NOW + 5)
        .unwrap();
    b.vault
        .update_item(&gh, login("From B", "octo", "b", "github.com"), NOW + 6)
        .unwrap();
    sync(root.path(), &mut a, NOW + 7);
    sync(root.path(), &mut b, NOW + 7);
    sync(root.path(), &mut a, NOW + 8);
    assert_eq!(titles(&a.vault), vec!["From B"]);
    assert_eq!(titles(&b.vault), vec!["From B"]);

    // A deletes, but B edited afterwards: the edit wins everywhere.
    a.vault.delete_item(&gh, NOW + 10).unwrap();
    b.vault
        .update_item(&gh, login("Kept", "octo", "k", "github.com"), NOW + 11)
        .unwrap();
    sync(root.path(), &mut a, NOW + 12);
    sync(root.path(), &mut b, NOW + 12);
    sync(root.path(), &mut a, NOW + 13);
    assert_eq!(titles(&a.vault), vec!["Kept"]);
    assert_eq!(titles(&b.vault), vec!["Kept"]);
}

/// Replaying an old snapshot (a rollback attempt through the sync service)
/// cannot bring back old values.
#[test]
fn replayed_old_snapshots_do_not_roll_back() {
    let (root, _sk, mut a, mut b) = setup();
    let gh = a.vault.list_items().unwrap()[0].id;
    let b_file = dir_of(root.path(), &b.vault)
        .join("devices")
        .join(format!("{}.hks", b.id));
    let old = std::fs::read(&b_file).unwrap();

    b.vault
        .update_item(&gh, login("New", "octo", "new-pw", "github.com"), NOW + 5)
        .unwrap();
    sync(root.path(), &mut b, NOW + 6);
    sync(root.path(), &mut a, NOW + 7);
    assert_eq!(titles(&a.vault), vec!["New"]);

    std::fs::write(&b_file, old).unwrap(); // attacker restores B's old file
    let r = sync(root.path(), &mut a, NOW + 8);
    assert_eq!((r.added, r.updated), (0, 0));
    assert_eq!(
        a.vault.reveal(&gh, SecretField::Password).unwrap().expose(),
        "new-pw"
    );
}

/// A4 for the folder: tampered, foreign or garbage files are ignored.
#[test]
fn tampered_and_foreign_files_are_ignored() {
    let (root, _sk, mut a, b) = setup();
    let devices = dir_of(root.path(), &a.vault).join("devices");
    let b_file = devices.join(format!("{}.hks", b.id));
    let mut bytes = std::fs::read(&b_file).unwrap();
    let n = bytes.len();
    bytes[n - 5] ^= 1;
    std::fs::write(&b_file, &bytes).unwrap();
    // B's valid file copied under another device's name: bound to B's ID.
    let mut c = Device {
        id: Uuid::new_v4(),
        vault: sk_vault(&SecretKey::generate().unwrap()),
    };
    c.vault
        .create_item(login("Evil", "x", "y", "evil.com"), NOW)
        .unwrap();
    let foreign = c
        .vault
        .sync(c.id, SyncInput::default(), NOW)
        .unwrap()
        .snapshot;
    std::fs::write(devices.join(format!("{}.hks", Uuid::new_v4())), foreign).unwrap();
    std::fs::write(devices.join("not-a-uuid.hks"), b"junk").unwrap();
    std::fs::write(devices.join(format!("{}.hks", Uuid::new_v4())), b"junk").unwrap();

    let r = sync(root.path(), &mut a, NOW + 1);
    assert_eq!(r.unreadable_devices, 3);
    assert_eq!(titles(&a.vault), vec!["GitHub"]);
}

#[test]
fn a_forged_header_is_rejected_and_repaired() {
    let (root, sk, mut a, _b) = setup();
    let dir = dir_of(root.path(), &a.vault);
    let original = std::fs::read(dir.join("header.json")).unwrap();
    // Raise the revision without being able to re-attest it.
    let forged = String::from_utf8(original.clone())
        .unwrap()
        .replace("\"revision\": 0", "\"revision\": 99");
    assert_ne!(forged.as_bytes(), original.as_slice());
    std::fs::write(dir.join("header.json"), &forged).unwrap();
    let r = sync(root.path(), &mut a, NOW + 1);
    assert!(r.header_rejected && !r.header_adopted);
    // Repaired: a freshly attested header at the real revision (new nonce,
    // so not byte-identical), which a new device can join with.
    let repaired = String::from_utf8(std::fs::read(dir.join("header.json")).unwrap()).unwrap();
    assert!(repaired.contains("\"revision\": 0"));
    let vault_id = a.vault.vault_id().unwrap().unwrap();
    assert!(join(root.path(), vault_id, PASSWORD, &sk).is_ok());
    // A new device refuses the forged header.
    std::fs::write(dir.join("header.json"), &forged).unwrap();
    assert!(join(root.path(), vault_id, PASSWORD, &sk).is_err());
}

/// A master password change on one device reaches the others through the
/// header; they need the new password at their next unlock.
#[test]
fn password_change_propagates() {
    let (root, sk, mut a, mut b) = setup();
    let ticket = a.vault.begin_rekey().unwrap();
    let r = ticket.derive_with_secret_key(
        &secret(PASSWORD),
        &secret("rotated password"),
        fast_kdf(),
        Some(&sk),
    );
    a.vault.commit_rekey(ticket, r).unwrap();
    sync(root.path(), &mut a, NOW + 1);
    let r = sync(root.path(), &mut b, NOW + 2);
    assert!(r.header_adopted);

    b.vault.lock();
    let ticket = b.vault.begin_unlock().unwrap();
    let old = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&sk));
    assert_eq!(b.vault.finish_unlock(ticket, old), Err(Error::UnlockFailed));
    let ticket = b.vault.begin_unlock().unwrap();
    let new = ticket.derive_with_secret_key(&secret("rotated password"), Some(&sk));
    b.vault.finish_unlock(ticket, new).unwrap();
    assert_eq!(titles(&b.vault), vec!["GitHub"]);
}

#[test]
fn sync_needs_a_secret_key_and_the_same_vault() {
    let mut v = new_vault();
    assert!(v.sync(Uuid::new_v4(), SyncInput::default(), NOW).is_err());

    let (root, _sk, a, _b) = setup();
    let other = Device {
        id: Uuid::new_v4(),
        vault: sk_vault(&SecretKey::generate().unwrap()),
    };
    // Point the other vault at A's folder.
    let mut input = folder::read(&dir_of(root.path(), &a.vault), other.id).unwrap();
    input.devices.clear();
    let mut other = other;
    assert!(other.vault.sync(other.id, input, NOW).is_err());
}

#[test]
fn sync_refused_while_locked() {
    let (_root, _sk, mut a, _b) = setup();
    a.vault.lock();
    assert!(a.vault.sync(a.id, SyncInput::default(), NOW).is_err());
}
