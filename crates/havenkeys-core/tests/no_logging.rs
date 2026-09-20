//! Logging policy guard: the security core must never print or log.

use std::path::Path;

fn visit(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            visit(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
}

#[test]
fn core_source_has_no_print_or_log_macros() {
    let mut files = Vec::new();
    visit(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    let forbidden = [
        "println!",
        "eprintln!",
        "print!(",
        "eprint!(",
        "dbg!",
        "log::",
        "tracing::",
    ];
    for (name, src) in files {
        for pattern in forbidden {
            assert!(!src.contains(pattern), "{name} contains `{pattern}`");
        }
    }
}

/// `AuthKey` is the one new secret-bearing type in this plan (CLAUDE.md
/// §9, §40): its `Debug` must never print the key material, so an accidental
/// `{:?}` in a log line stays safe.
#[test]
fn auth_key_debug_never_leaks_material() {
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::KdfParams;
    use havenkeys_core::crypto::secret_key::SecretKey;
    use havenkeys_core::vault::derive_auth_key;
    use havenkeys_core::SecretString;

    let account = AccountRef::new(
        uuid::Uuid::from_u128(3),
        NormalizedEmail::parse("user@example.com").unwrap(),
    );
    let sk = SecretKey::generate().unwrap();
    let kdf = KdfParams::with_cost(
        havenkeys_core::crypto::kdf::MIN_MEMORY_KIB,
        havenkeys_core::crypto::kdf::MIN_ITERATIONS,
        1,
    )
    .unwrap();
    let auth = derive_auth_key(
        &SecretString::from("correct horse battery staple"),
        &sk,
        &kdf,
        &account,
    )
    .unwrap();
    let printed = format!("{auth:?}");
    assert_eq!(printed, "AuthKey(<redacted>)");
    assert!(!printed.contains(auth.to_base64().as_str()));
}
