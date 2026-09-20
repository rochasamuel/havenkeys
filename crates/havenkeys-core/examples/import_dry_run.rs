//! Dry-run a 1Password import: parse a `.1pux` file and print the resulting
//! counts. Storing the parsed items needs a server to accept the write
//! (spec 2026-09-20 §8.4); this only exercises the parser.
//!
//! `cargo run --release -p havenkeys-core --example import_dry_run -- <file.1pux>`
//!
//! Nothing is written to disk and no item content is printed.

use havenkeys_core::import::onepux;
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
    println!(
        "{}",
        serde_json::to_string_pretty(&parsed.report).expect("json")
    );
}
