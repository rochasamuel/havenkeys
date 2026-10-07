//! Item tags (spec 2026-10-07-item-tags §3.2): free-form labels the user
//! puts on items to group them. Every write path normalises through here,
//! so "Staging", " staging " and "STAGING" are one tag on every device.

use crate::error::{Error, Result};

pub const MAX_TAGS: usize = 20;
pub const MAX_TAG_CHARS: usize = 32;

/// One tag as stored: trimmed, inner whitespace collapsed, lowercase.
/// Errors are fixed strings; the input is never echoed.
pub fn normalize_one(raw: &str) -> Result<String> {
    let tag = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if tag.is_empty() {
        return Err(Error::InvalidInput("Tag is empty."));
    }
    if tag.chars().count() > MAX_TAG_CHARS {
        return Err(Error::InvalidInput("Tag is too long."));
    }
    if tag.chars().any(|c| c.is_control() || c == ',') {
        return Err(Error::InvalidInput(
            "Tag contains a character that is not allowed.",
        ));
    }
    Ok(tag)
}

/// An item's whole tag set: each normalised, deduplicated, sorted, at most
/// [`MAX_TAGS`].
pub fn normalize(raw: &[String]) -> Result<Vec<String>> {
    let mut tags = raw
        .iter()
        .map(|t| normalize_one(t))
        .collect::<Result<Vec<_>>>()?;
    tags.sort();
    tags.dedup();
    if tags.len() > MAX_TAGS {
        return Err(Error::InvalidInput("Too many tags."));
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: &[&str]) -> Result<Vec<String>> {
        normalize(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn trims_collapses_and_lowercases() {
        assert_eq!(
            n(&["  Staging  ", "Dev   Team"]).unwrap(),
            vec!["dev team", "staging"]
        );
    }

    #[test]
    fn lowercases_unicode() {
        assert_eq!(n(&["PRODUÇÃO"]).unwrap(), vec!["produção"]);
    }

    #[test]
    fn deduplicates_case_insensitively_and_sorts() {
        assert_eq!(n(&["work", "Work", "api"]).unwrap(), vec!["api", "work"]);
    }

    #[test]
    fn keeps_a_slash_for_imported_folders() {
        assert_eq!(n(&["Work/Staging"]).unwrap(), vec!["work/staging"]);
    }

    #[test]
    fn refuses_empty_long_comma_and_control() {
        assert!(matches!(
            n(&["   "]),
            Err(Error::InvalidInput("Tag is empty."))
        ));
        assert!(matches!(
            n(&[&"a".repeat(33)]),
            Err(Error::InvalidInput("Tag is too long."))
        ));
        assert!(n(&[&"é".repeat(32)]).is_ok(), "32 chars, not bytes");
        assert!(matches!(
            n(&["a,b"]),
            Err(Error::InvalidInput(
                "Tag contains a character that is not allowed."
            ))
        ));
        assert!(n(&["a\u{0}b"]).is_err());
        // Newlines and tabs are whitespace: collapsed like spaces.
        assert_eq!(n(&["a\nb"]).unwrap(), vec!["a b"]);
    }

    #[test]
    fn refuses_more_than_twenty_after_deduplication() {
        let twenty: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
        assert_eq!(normalize(&twenty).unwrap().len(), 20);
        let mut dupes = twenty.clone();
        dupes.push("T0".into());
        assert_eq!(normalize(&dupes).unwrap().len(), 20);
        let mut over = twenty;
        over.push("t20".into());
        assert!(matches!(
            normalize(&over),
            Err(Error::InvalidInput("Too many tags."))
        ));
    }

    #[test]
    fn errors_never_contain_the_input() {
        let secretish = format!("hunter2{}", "x".repeat(40));
        let err = n(&[&secretish]).unwrap_err();
        assert!(!format!("{err} {err:?}").contains("hunter2"));
    }
}
