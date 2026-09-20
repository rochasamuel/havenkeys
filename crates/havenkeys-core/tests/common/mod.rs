#![allow(dead_code)]

use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::store::{AccountRecord, Store};
use havenkeys_core::vault::{prepare_new_account_vault, prepare_new_vault, VaultService};
use havenkeys_core::SecretString;
use std::path::Path;
use uuid::Uuid;

pub const PASSWORD: &str = "correct horse battery staple";
pub const NOW: i64 = 1_700_000_000_000;

/// Cheapest parameters the validator accepts, to keep tests fast.
pub fn fast_kdf() -> KdfParams {
    KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap()
}

pub fn secret(s: &str) -> SecretString {
    SecretString::from(s)
}

pub fn new_vault_in(store: Store) -> VaultService {
    let mut v = VaultService::new(store);
    v.create_vault(prepare_new_vault(&secret(PASSWORD), fast_kdf(), NOW).unwrap())
        .unwrap();
    v
}

pub fn new_vault() -> VaultService {
    new_vault_in(Store::open_in_memory().unwrap())
}

pub fn open_file(path: &Path) -> VaultService {
    VaultService::new(Store::open(path).unwrap())
}

pub fn login(title: &str, username: &str, password: &str, url: &str) -> ItemInput {
    ItemInput {
        item_type: ItemType::Login,
        title: title.into(),
        username: Some(username.into()),
        urls: vec![UrlRule {
            url: url.into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(secret(password)),
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
    }
}

pub fn account() -> AccountRef {
    AccountRef::new(
        Uuid::from_u128(0x5eed),
        NormalizedEmail::parse("user@example.com").unwrap(),
    )
}

pub fn account_record() -> AccountRecord {
    AccountRecord {
        account_id: account().id,
        email: "user@example.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    }
}

/// An activated account vault, unlocked, with its Secret Key and its
/// account record (an account vault never exists without one).
pub fn activated_vault() -> (VaultService, SecretKey) {
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let sk = made.secret_key;
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault
        .create_account_vault(made.prepared, &account_record())
        .unwrap();
    // create_account_vault leaves the vault unlocked; lock it first so the
    // account-bound unlock path is genuinely exercised, matching sign-in on
    // a real second device.
    vault.lock();
    vault
        .unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    (vault, sk)
}

/// A second device on the same account, signed in from the first one's
/// header and ready to sync.
pub fn second_device(first: &VaultService, sk: &SecretKey) -> VaultService {
    let header = first.encode_account_header().unwrap();
    let (prepared, _auth) =
        havenkeys_core::sync::prepare_sign_in(&header, &secret(PASSWORD), sk, &account()).unwrap();
    let mut b = VaultService::new(Store::open_in_memory().unwrap());
    b.create_account_vault(prepared, &account_record()).unwrap();
    // create_account_vault leaves the vault unlocked; lock it so the
    // account unlock path is the one actually exercised.
    b.lock();
    b.unlock_for_account(&secret(PASSWORD), sk, &account())
        .unwrap();
    b
}

pub fn note(title: &str, content: &str) -> ItemInput {
    ItemInput {
        item_type: ItemType::SecureNote,
        title: title.into(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Set(secret(content)),
    }
}
