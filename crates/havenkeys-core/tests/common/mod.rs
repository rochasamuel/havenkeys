#![allow(dead_code)]

use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::store::Store;
use havenkeys_core::vault::{prepare_new_vault, VaultService};
use havenkeys_core::SecretString;
use std::path::Path;

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
