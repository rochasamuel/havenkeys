//! Dry-run a 1Password import: parse a `.1pux` file and import it into a
//! throwaway in-memory vault, printing only the resulting counts.
//!
//! `cargo run --release -p havenkeys-core --example import_dry_run -- <file.1pux>`
//!
//! Nothing is written to disk and no item content is printed.

use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::import::onepux;
use havenkeys_core::store::Store;
use havenkeys_core::vault::{prepare_new_vault, VaultService};
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
    let mut vault = VaultService::new(Store::open_in_memory().expect("store"));
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).expect("kdf");
    vault
        .create_vault(prepare_new_vault(&"dry-run-password".into(), kdf, 0).expect("vault"))
        .expect("create");
    let report = vault
        .import_items(parsed.items, parsed.report, 0)
        .expect("import");
    println!("{}", serde_json::to_string_pretty(&report).expect("json"));
}
