//! Dry-run an import: parse an export file and print the resulting counts.
//! Storing the parsed items needs a server to accept the write
//! (spec 2026-09-20 §8.4); this only exercises the parser.
//!
//! `cargo run --release -p havenkeys-core --example import_dry_run -- <source> <file>`
//!
//! `<source>` is one of onePassword, bitwardenJson, bitwardenCsv, chrome,
//! firefox, keePassXc, lastPass. Nothing is written to disk and no item
//! content is printed.

use havenkeys_core::import::{self, ImportSource};
use zeroize::Zeroizing;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(source), Some(path)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: import_dry_run <source> <file>");
        std::process::exit(2);
    };
    let Ok(source) = serde_json::from_value::<ImportSource>(serde_json::json!(source)) else {
        eprintln!("unknown source");
        std::process::exit(2);
    };
    let bytes = Zeroizing::new(std::fs::read(path).expect("could not read the file"));
    let parsed = match import::parse(source, &bytes) {
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
