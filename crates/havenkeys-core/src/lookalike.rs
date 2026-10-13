//! Look-alike sites (spec 2026-10-13-lookalike-warning §3): a page whose
//! registrable domain *looks* like a saved site's but is a different
//! domain, such as `pаypal.com` (Cyrillic "а") or `g00gle.com`.
//!
//! Only ever used to add a warning. Matching and filling never read this
//! module: a look-alike still matches nothing (origin.rs).
//!
//! Not detected, by design: typos (`paypall.com`), the same name on another
//! suffix (`paypal.co.uk`), names that contain a brand (`paypal-login.com`).
#![cfg_attr(not(test), allow(dead_code))]

use crate::origin::{host_key, registrable_domain_of, PageUrl};
use std::collections::BTreeSet;
use url::Url;

/// Two registrable domains look alike when their keys are equal. The digit
/// map runs before the skeleton so that a digit and the letter it imitates
/// reach the same prototype (`g00gle` → `google`).
pub(crate) fn key(domain: &str) -> String {
    let (unicode, decoded) = idna::domain_to_unicode(domain);
    let text = if decoded.is_ok() {
        unicode
    } else {
        domain.to_owned()
    };
    let mapped: String = text
        .chars()
        .map(|c| match c {
            '0' => 'o',
            '1' => 'l',
            '3' => 'e',
            '5' => 's',
            c => c,
        })
        .collect();
    unicode_security::skeleton(&mapped)
        .collect::<String>()
        .to_lowercase()
}

/// The page's registrable domain under a known public suffix. `None` for IP
/// addresses, `localhost`, unknown suffixes and public suffixes themselves.
pub(crate) fn page_site(page: &PageUrl) -> Option<String> {
    registrable_domain_of(&host_key(page.url())?)
}

/// The registrable domain of a saved http(s) website rule, if it has one.
pub(crate) fn rule_site(rule_url: &str) -> Option<String> {
    let url = Url::parse(rule_url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    registrable_domain_of(&host_key(&url)?)
}

/// The saved site `page_site` imitates: the first (sorted, so answers are
/// stable) saved site that is a different domain with the same key.
pub(crate) fn imitated(page_site: &str, saved: &BTreeSet<String>) -> Option<String> {
    let k = key(page_site);
    saved
        .iter()
        .find(|s| s.as_str() != page_site && key(s) == k)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ASCII (punycode) host `url` makes of `raw`: never hand-written.
    fn ascii(raw: &str) -> String {
        Url::parse(&format!("https://{raw}/"))
            .unwrap()
            .host_str()
            .unwrap()
            .to_owned()
    }

    fn set(sites: &[&str]) -> BTreeSet<String> {
        sites.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn confusables_and_digits_share_a_key() {
        let cyrillic = ascii("p\u{0430}ypal.com");
        assert!(cyrillic.starts_with("xn--"), "{cyrillic}");
        assert_eq!(key(&cyrillic), key("paypal.com"));
        assert_eq!(key("g00gle.com"), key("google.com"));
        assert_eq!(key("rnicrosoft.com"), key("microsoft.com"));
        assert_eq!(key("paypa1.com"), key("paypal.com"));
    }

    #[test]
    fn other_suffixes_and_unrelated_names_differ() {
        assert_ne!(key("paypal.co.uk"), key("paypal.com"));
        assert_ne!(key("paypal-login.com"), key("paypal.com"));
        assert_ne!(key("paypall.com"), key("paypal.com"));
        assert_ne!(key("example.org"), key("paypal.com"));
    }

    #[test]
    fn undecodable_punycode_is_kept_as_is() {
        // Not valid punycode: the key is still computed, from the ASCII text.
        // (The skeleton itself spells "m" as "rn", hence "corn".)
        assert_eq!(key("xn--invalid-.com"), "xn--invalid-.corn");
    }

    #[test]
    fn imitated_needs_a_different_domain_with_the_same_key() {
        let saved = set(&["github.com", "paypal.com"]);
        assert_eq!(
            imitated("paypa1.com", &saved).as_deref(),
            Some("paypal.com")
        );
        assert_eq!(imitated("paypal.com", &saved), None);
        assert_eq!(imitated("example.org", &saved), None);
    }

    #[test]
    fn imitated_picks_the_first_in_sorted_order() {
        let saved = set(&["paypal.com", "paypa1.com"]);
        let page = ascii("p\u{0430}ypal.com");
        assert_eq!(imitated(&page, &saved).as_deref(), Some("paypa1.com"));
        assert_eq!(imitated(&page, &saved), imitated(&page, &saved));
    }

    #[test]
    fn rule_and_page_sites_are_registrable_domains_only() {
        assert_eq!(
            rule_site("https://login.paypal.com/x").as_deref(),
            Some("paypal.com")
        );
        assert_eq!(rule_site("http://192.168.1.1/"), None);
        assert_eq!(rule_site("http://localhost:8080/"), None);
        assert_eq!(rule_site("https://github.io/"), None);
        assert_eq!(rule_site("androidapp://com.github.android"), None);
        assert_eq!(rule_site("not a url"), None);
        let page = PageUrl::parse("https://www.g00gle.com/login").unwrap();
        assert_eq!(page_site(&page).as_deref(), Some("g00gle.com"));
        assert_eq!(
            page_site(&PageUrl::parse("https://10.0.0.1/").unwrap()),
            None
        );
    }

    /// Seeded xorshift: random Unicode never panics, the key is a pure
    /// function of its input, and it is lowercase.
    #[test]
    fn fuzz_key_is_total_and_deterministic() {
        let mut s: u64 = 0x5eed_100c_a11e | 1;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..5_000 {
            let len = (next() % 24) as usize;
            let text: String = (0..len)
                .filter_map(|_| char::from_u32((next() % 0x3_0000) as u32))
                .collect();
            let a = key(&text);
            assert_eq!(a, key(&text));
            assert_eq!(a, a.to_lowercase());
            // As a host, too: whatever `url` accepts reaches a key.
            if let Some(page) = PageUrl::parse(&format!("https://{text}.com/")) {
                if let Some(site) = page_site(&page) {
                    let k = key(&site);
                    assert!(!k.is_empty());
                    assert_eq!(k, key(&site));
                }
            }
        }
    }
}
