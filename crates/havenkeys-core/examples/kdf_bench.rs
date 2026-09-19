//! Measure Argon2id unlock cost on this machine:
//! `cargo run --release -p havenkeys-core --example kdf_bench`

use havenkeys_core::crypto::kdf::{derive_master_key, KdfParams};
use havenkeys_core::SecretString;
use std::time::Instant;

fn main() {
    let password = SecretString::from("benchmark password");
    for (m, t, p) in [
        (19 * 1024, 2, 1),
        (64 * 1024, 3, 4),
        (128 * 1024, 3, 4),
        (128 * 1024, 4, 4),
        (256 * 1024, 3, 4),
    ] {
        let params = KdfParams::with_cost(m, t, p).expect("valid params");
        let start = Instant::now();
        let runs = 3;
        for _ in 0..runs {
            derive_master_key(&password, &params).expect("kdf");
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / runs as f64;
        // Only timing information is printed; no key material.
        println!("m={:>7} KiB  t={t}  p={p}  ->  {ms:>7.1} ms", m);
    }
}
