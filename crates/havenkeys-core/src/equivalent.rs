//! Equivalent domains: registrable domains that one company owns and that
//! sign in with the same account (spec 2026-10-13-autofill-ranking-
//! equivalent-domains §4). A compiled, reviewed table; never fetched. A
//! table that does not parse reads as empty, so it can only narrow.

use crate::origin::registrable_domain_of;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const TABLE_JSON: &str = include_str!("../data/equivalent-domains.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    groups: Vec<Group>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    note: String,
    domains: Vec<String>,
}

/// Domain → group index.
fn index() -> &'static HashMap<String, usize> {
    static INDEX: OnceLock<HashMap<String, usize>> = OnceLock::new();
    INDEX.get_or_init(|| build(TABLE_JSON).unwrap_or_default())
}

/// The index, or `None` if the table is malformed in any way the checks
/// below forbid. Used at runtime (malformed → empty) and by the tests
/// (malformed → failure).
fn build(json: &str) -> Option<HashMap<String, usize>> {
    let table: Table = serde_json::from_str(json).ok()?;
    let mut index = HashMap::new();
    for (i, g) in table.groups.iter().enumerate() {
        if g.note.trim().is_empty() || g.domains.len() < 2 {
            return None;
        }
        for d in &g.domains {
            if !d.is_ascii() || *d != d.to_ascii_lowercase() {
                return None;
            }
            let canonical = registrable_domain_of(d).filter(|r| r == d)?;
            if index.insert(canonical, i).is_some() {
                return None;
            }
        }
    }
    Some(index)
}

/// Both are registrable domains in the same group, and differ.
pub(crate) fn same_group(a: &str, b: &str) -> bool {
    a != b && matches!((index().get(a), index().get(b)), (Some(x), Some(y)) if x == y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_table_passes_every_check() {
        let index = build(TABLE_JSON).expect("equivalent-domains.json breaks a table rule");
        assert!(!index.is_empty());
    }

    #[test]
    fn entries_are_lowercase_ascii_registrable_domains() {
        let table: Table = serde_json::from_str(TABLE_JSON).unwrap();
        for d in table.groups.iter().flat_map(|g| &g.domains) {
            assert!(d.is_ascii() && *d == d.to_lowercase(), "{d}");
        }
    }

    #[test]
    fn malformed_tables_are_rejected() {
        let one = r#"{"groups":[{"note":"x","domains":["apple.com"]}]}"#;
        let no_note = r#"{"groups":[{"note":" ","domains":["apple.com","icloud.com"]}]}"#;
        let sub = r#"{"groups":[{"note":"x","domains":["www.apple.com","icloud.com"]}]}"#;
        let suffix = r#"{"groups":[{"note":"x","domains":["github.io","icloud.com"]}]}"#;
        let twice = r#"{"groups":[{"note":"x","domains":["apple.com","icloud.com"]},
                                 {"note":"y","domains":["apple.com","me.com"]}]}"#;
        let upper = r#"{"groups":[{"note":"x","domains":["Apple.com","icloud.com"]}]}"#;
        let extra = r#"{"groups":[],"more":1}"#;
        for bad in [one, no_note, sub, suffix, twice, upper, extra, "not json"] {
            assert!(build(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn same_group_is_exact_and_symmetric() {
        assert!(same_group("apple.com", "icloud.com"));
        assert!(same_group("icloud.com", "apple.com"));
        assert!(!same_group("apple.com", "apple.com"));
        assert!(!same_group("apple.com", "google.com"));
        assert!(!same_group("icloud.com.evil.com", "apple.com"));
        assert!(!same_group("evil-icloud.com", "apple.com"));
        assert!(!same_group("unknown.example", "apple.com"));
    }
}
