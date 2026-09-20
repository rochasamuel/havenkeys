//! Dry-run a 1Password import: parse a `.1pux` file and import it into a
//! throwaway in-memory vault, printing only the resulting counts.
//!
//! `cargo run --release -p havenkeys-core --example import_dry_run -- <file.1pux>`
//!
//! Nothing is written to disk and no item content is printed.

use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::import::onepux;
use havenkeys_core::store::{AccountRecord, Store};
use havenkeys_core::vault::{prepare_new_account_vault, VaultService};
use uuid::Uuid;
use zeroize::Zeroizing;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: import_dry_run <file.1pux>");
        std::process::exit(2);
    };
    let bytes = Zeroizing::new(std::fs::read(&path).expect("could not read the file"));
    let parsed = match onepux::parse(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse failed: {e}");
            std::process::exit(1);
        }
    };
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).expect("kdf");
    // Every vault is account-bound now; this dry run makes up a throwaway
    // account since nothing here ever leaves this process.
    let account = AccountRef::new(
        Uuid::nil(),
        NormalizedEmail::parse("dry-run@example.invalid").expect("email"),
    );
    let made =
        prepare_new_account_vault(&"dry-run-password".into(), &account, kdf, 0).expect("vault");
    let mut vault = VaultService::new(Store::open_in_memory().expect("store"));
    vault
        .create_account_vault(
            made.prepared,
            &AccountRecord {
                account_id: account.id,
                email: "dry-run@example.invalid".into(),
                server_url: "https://example.invalid".into(),
                server_cursor: 0,
                max_header_rev: 0,
                last_synced_at: None,
            },
        )
        .expect("create");
    let report = vault
        .import_items(parsed.items, parsed.report, 0)
        .expect("import");
    println!("{}", serde_json::to_string_pretty(&report).expect("json"));
}
