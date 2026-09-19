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
