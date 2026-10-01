//! Test support for other crates: an unlocked account vault without a
//! server. Never compiled into an app.

use crate::client::HavenClient;
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::AccountRecord;
use havenkeys_core::vault::prepare_new_account_vault;
use havenkeys_core::SecretString;
use uuid::Uuid;

pub const ACCOUNT: Uuid = Uuid::from_u128(0x7e57);

/// Create an account vault on `client`, unlocked, with its Secret Key kept
/// on the device. The server URL points nowhere: the device stays offline.
pub fn seed_account_vault(client: &HavenClient, password: &str) -> SecretKey {
    let account = AccountRef::new(ACCOUNT, NormalizedEmail::parse("user@example.com").unwrap());
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
    let made = prepare_new_account_vault(
        &SecretString::from(password),
        &account,
        kdf,
        crate::now_ms(),
    )
    .unwrap();
    let record = AccountRecord {
        account_id: ACCOUNT,
        email: "user@example.com".into(),
        server_url: "https://127.0.0.1:9".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    };
    client.create_vault(made.prepared, &record).unwrap();
    client
        .device()
        .unwrap()
        .set_secret_key(ACCOUNT, &made.secret_key)
        .unwrap();
    made.secret_key
}
