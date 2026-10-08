//! Item tags (spec 2026-10-07-item-tags §3.2): free-form labels the user
//! puts on items to group them. Every write path normalises through here,
//! so "Staging", " staging " and "STAGING" are one tag on every device.
//! The spelling the user gave is kept; tags compare without case.

use crate::error::{Error, Result};

pub const MAX_TAGS: usize = 20;
pub const MAX_TAG_CHARS: usize = 32;

/// One tag as stored: trimmed, inner whitespace collapsed, case kept.
/// Errors are fixed strings; the input is never echoed.
pub fn normalize_one(raw: &str) -> Result<String> {
    let tag = raw.split_whitespace().collect::<Vec<_>>().join(" ");
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

/// The key two tags are compared by: the same tag whatever its case.
pub fn key(tag: &str) -> String {
    tag.to_lowercase()
}

/// An item's whole tag set: each normalised, deduplicated without case
/// (the first spelling given wins), sorted A-Z without case, at most
/// [`MAX_TAGS`].
pub fn normalize(raw: &[String]) -> Result<Vec<String>> {
    let tags = raw
        .iter()
        .map(|t| normalize_one(t))
        .collect::<Result<Vec<_>>>()?;
    let tags = dedupe_sorted(tags);
    if tags.len() > MAX_TAGS {
        return Err(Error::InvalidInput("Too many tags."));
    }
    Ok(tags)
}

/// Drop later spellings of a tag already present, then sort by
/// (key, spelling).
pub(crate) fn dedupe_sorted(tags: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = tags.into_iter().filter(|t| seen.insert(key(t))).collect();
    out.sort_by_cached_key(|t| (key(t), t.clone()));
    out
}

/// Give `tags` the spelling the rest of the vault already uses: each tag
/// whose key is in `vault` (key -> spelling) takes that spelling. Then
/// deduplicated and sorted again.
pub(crate) fn canonicalize(
    tags: Vec<String>,
    vault: &std::collections::HashMap<String, String>,
) -> Vec<String> {
    let mapped = tags
        .into_iter()
        .map(|t| vault.get(&key(&t)).cloned().unwrap_or(t))
        .collect();
    dedupe_sorted(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: &[&str]) -> Result<Vec<String>> {
        normalize(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn trims_collapses_and_keeps_case() {
        assert_eq!(
            n(&["  Staging  ", "Dev   Team"]).unwrap(),
            vec!["Dev Team", "Staging"]
        );
    }

    #[test]
    fn keeps_unicode_case() {
        assert_eq!(n(&["PRODUÇÃO"]).unwrap(), vec!["PRODUÇÃO"]);
    }

    #[test]
    fn deduplicates_case_insensitively_keeping_the_first_spelling() {
        assert_eq!(n(&["Work", "work", "api"]).unwrap(), vec!["api", "Work"]);
        assert_eq!(n(&["work", "WORK"]).unwrap(), vec!["work"]);
    }

    #[test]
    fn sorts_without_case() {
        assert_eq!(
            n(&["beta", "Alpha", "gamma"]).unwrap(),
            vec!["Alpha", "beta", "gamma"]
        );
    }

    #[test]
    fn canonicalize_takes_the_vault_spelling() {
        let vault = std::collections::HashMap::from([("work".to_string(), "Work".to_string())]);
        assert_eq!(
            canonicalize(vec!["work".into(), "api".into()], &vault),
            vec!["api", "Work"]
        );
        assert_eq!(canonicalize(vec!["Other".into()], &vault), vec!["Other"]);
    }

    #[test]
    fn keeps_a_slash_for_imported_folders() {
        assert_eq!(n(&["Work/Staging"]).unwrap(), vec!["Work/Staging"]);
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
