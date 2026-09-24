//! Domain matching: does a saved website rule apply to the page being filled?
//!
//! This is the security boundary that stops credentials for `github.com`
//! from being offered on `github.com.evil.com`. It never does string
//! prefix/suffix matching on raw URLs: URLs are parsed with the WHATWG `url`
//! parser (which also converts IDNs to punycode) and registrable domains come
//! from the Public Suffix List (`psl`, compiled in — no network).
//!
//! Rules (see docs/autofill.md):
//!
//! * Only `http`/`https` pages can match.
//! * Scheme: an `https` rule never matches an `http` page (no downgrade). An
//!   `http` rule matches `http` or `https` pages (upgrade is allowed).
//! * Port must be equal (after applying scheme defaults).
//! * `Exact`: same host and same path. Query and fragment are ignored.
//! * `Origin`: same host.
//! * `Domain`: same registrable domain (eTLD+1), so `login.example.com`
//!   matches a rule for `example.com` and vice versa. If either host has no
//!   registrable domain under a *known* public suffix (IP addresses,
//!   `localhost`, unknown TLDs, or a host that is itself a public suffix such
//!   as `github.io`), `Domain` falls back to same-host.

use crate::model::{ItemOverview, MatchType, UrlRule};
use serde::Serialize;
use url::{Host, Url};

/// How closely a page matched, strongest first. Used to rank suggestions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStrength {
    /// Same host and path (an `Exact` rule).
    ExactUrl,
    /// Same host.
    SameHost,
    /// Different host under the same registrable domain (`Domain` rule only).
    SameSite,
}

/// A page URL that has passed the basic checks.
pub struct PageUrl {
    url: Url,
}

impl PageUrl {
    /// Parse a page URL. Anything that is not an absolute http(s) URL with a
    /// host is rejected: `about:blank`, `data:`, `file:`, `javascript:`,
    /// `blob:`, extension pages, etc. never match anything.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.len() > crate::model::MAX_URL_LEN * 4 {
            return None;
        }
        let url = Url::parse(raw).ok()?;
        if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
            return None;
        }
        Some(Self { url })
    }

    /// A title for a login saved from this page (the host without a leading
    /// `www.`) and the page's origin, used as the new login's website.
    pub fn title_and_origin(&self) -> Option<(String, String)> {
        let host = host_key(&self.url)?;
        let title = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        let origin = self.url.origin().ascii_serialization();
        (origin != "null").then_some((title, origin))
    }

    pub(crate) fn url(&self) -> &Url {
        &self.url
    }
}

/// Host with a single trailing dot removed (`example.com.` is the same DNS
/// name as `example.com`). IP hosts are returned in canonical form.
pub(crate) fn host_key(url: &Url) -> Option<String> {
    match url.host()? {
        Host::Domain(d) => Some(d.strip_suffix('.').unwrap_or(d).to_owned()),
        Host::Ipv4(ip) => Some(ip.to_string()),
        Host::Ipv6(ip) => Some(format!("[{ip}]")),
    }
}

/// Registrable domain (eTLD+1) under a known public suffix, or `None`.
fn registrable_domain(url: &Url) -> Option<String> {
    let Host::Domain(d) = url.host()? else {
        return None;
    };
    registrable_domain_of(d.strip_suffix('.').unwrap_or(d))
}

/// [`registrable_domain`] for a host name that is already normalized.
pub(crate) fn registrable_domain_of(host: &str) -> Option<String> {
    let domain = psl::domain(host.as_bytes())?;
    if !domain.suffix().is_known() {
        return None;
    }
    std::str::from_utf8(domain.as_bytes())
        .ok()
        .map(str::to_owned)
}

fn scheme_allowed(rule: &Url, page: &Url) -> bool {
    // Never downgrade https → http; allow http → https.
    matches!(
        (rule.scheme(), page.scheme()),
        ("https", "https") | ("http", "http") | ("http", "https")
    )
}

fn port_matches(rule: &Url, page: &Url) -> bool {
    // An http rule upgraded to https compares the page against the default
    // https port unless the rule named a port explicitly.
    let rule_port = match (rule.port(), rule.scheme(), page.scheme()) {
        (None, "http", "https") => Some(443),
        _ => rule.port_or_known_default(),
    };
    rule_port == page.port_or_known_default()
}

/// Does `rule` apply to `page`? Returns how strongly, or `None`.
pub fn match_rule(rule: &UrlRule, page: &PageUrl) -> Option<MatchStrength> {
    let rule_url = Url::parse(&rule.url).ok()?;
    let page = &page.url;
    if !matches!(rule_url.scheme(), "http" | "https") {
        return None;
    }
    if !scheme_allowed(&rule_url, page) || !port_matches(&rule_url, page) {
        return None;
    }
    let rule_host = host_key(&rule_url)?;
    let page_host = host_key(page)?;
    let same_host = rule_host == page_host;

    match rule.match_type {
        MatchType::Exact => {
            (same_host && rule_url.path() == page.path()).then_some(MatchStrength::ExactUrl)
        }
        MatchType::Origin => same_host.then_some(MatchStrength::SameHost),
        MatchType::Domain => {
            if same_host {
                return Some(MatchStrength::SameHost);
            }
            let rule_site = registrable_domain(&rule_url)?;
            let page_site = registrable_domain(page)?;
            (rule_site == page_site).then_some(MatchStrength::SameSite)
        }
    }
}

/// Best match of any of the item's website rules.
pub fn match_item(item: &ItemOverview, page: &PageUrl) -> Option<MatchStrength> {
    item.urls.iter().filter_map(|r| match_rule(r, page)).min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::normalize_url;

    fn rule(url: &str, match_type: MatchType) -> UrlRule {
        UrlRule {
            url: normalize_url(url).unwrap(),
            match_type,
        }
    }

    fn m(rule_url: &str, match_type: MatchType, page: &str) -> Option<MatchStrength> {
        match_rule(&rule(rule_url, match_type), &PageUrl::parse(page)?)
    }

    use MatchStrength::*;
    use MatchType::*;

    #[test]
    fn claude_md_domain_cases() {
        // Saved item https://example.com (domain rule).
        let r = "https://example.com";
        assert_eq!(m(r, Domain, "https://example.com/login"), Some(SameHost));
        assert_eq!(m(r, Domain, "https://www.example.com/"), Some(SameSite));
        assert_eq!(m(r, Domain, "https://login.example.com/"), Some(SameSite));
        assert_eq!(m(r, Domain, "https://evil-example.com/"), None);
        assert_eq!(m(r, Domain, "https://example.com.evil.com/"), None);
        assert_eq!(m(r, Domain, "https://evil.com/"), None);
        assert_eq!(m(r, Domain, "https://notexample.com/"), None);
        assert_eq!(m(r, Domain, "https://example.co/"), None);
    }

    #[test]
    fn phishing_shapes_never_match() {
        let r = "https://github.com";
        for page in [
            "https://github.com.evil.com/login",
            "https://github-login.example.com/",
            "https://evilgithub.com/",
            "https://github.co/",
            "https://evil.com/github.com",
            "https://evil.com/?next=https://github.com",
            "https://evil.com/#github.com",
            "https://github.com@evil.com/",
            "https://evil.com\\@github.com/",
            // IDN homograph: Cyrillic "і" → different punycode host.
            "https://gіthub.com/",
        ] {
            for t in [Domain, Origin, Exact] {
                assert_eq!(m(r, t, page), None, "{page} matched {t:?}");
            }
        }
    }

    #[test]
    fn subdomain_rule_matches_its_site() {
        let r = "https://accounts.google.com";
        assert_eq!(
            m(r, Domain, "https://accounts.google.com/signin"),
            Some(SameHost)
        );
        assert_eq!(m(r, Domain, "https://mail.google.com/"), Some(SameSite));
        assert_eq!(m(r, Domain, "https://google.com/"), Some(SameSite));
        assert_eq!(m(r, Origin, "https://mail.google.com/"), None);
    }

    #[test]
    fn public_suffix_boundaries() {
        // Each github.io / co.uk registration is its own site.
        assert_eq!(
            m("https://alice.github.io", Domain, "https://bob.github.io/"),
            None
        );
        assert_eq!(
            m(
                "https://alice.github.io",
                Domain,
                "https://www.alice.github.io/"
            ),
            Some(SameSite)
        );
        assert_eq!(m("https://bank.co.uk", Domain, "https://evil.co.uk/"), None);
        assert_eq!(
            m("https://bank.co.uk", Domain, "https://login.bank.co.uk/"),
            Some(SameSite)
        );
        // A rule that is itself a public suffix only matches that exact host.
        assert_eq!(
            m("https://github.io", Domain, "https://alice.github.io/"),
            None
        );
        assert_eq!(
            m("https://github.io", Domain, "https://github.io/"),
            Some(SameHost)
        );
    }

    #[test]
    fn ips_localhost_and_unknown_tlds_are_host_only() {
        assert_eq!(
            m("http://192.168.1.1", Domain, "http://192.168.1.1/admin"),
            Some(SameHost)
        );
        assert_eq!(m("http://192.168.1.1", Domain, "http://192.168.1.2/"), None);
        assert_eq!(
            m("http://localhost:8080", Domain, "http://localhost:8080/x"),
            Some(SameHost)
        );
        assert_eq!(
            m(
                "http://localhost:8080",
                Domain,
                "http://app.localhost:8080/"
            ),
            None
        );
        assert_eq!(
            m("https://nas.internal", Domain, "https://evil.internal/"),
            None
        );
        assert_eq!(
            m("http://[::1]:3000", Domain, "http://[::1]:3000/"),
            Some(SameHost)
        );
    }

    #[test]
    fn scheme_rules() {
        // No downgrade.
        assert_eq!(
            m("https://example.com", Domain, "http://example.com/"),
            None
        );
        assert_eq!(
            m("https://example.com", Origin, "http://example.com/"),
            None
        );
        // Upgrade allowed.
        assert_eq!(
            m("http://example.com", Origin, "https://example.com/"),
            Some(SameHost)
        );
        assert_eq!(
            m("http://example.com", Domain, "http://example.com/"),
            Some(SameHost)
        );
        // Non-web pages never match.
        for page in [
            "javascript:alert(1)",
            "data:text/html,<form>",
            "file:///C:/Users/x/login.html",
            "about:blank",
            "blob:https://example.com/abc",
            "chrome-extension://abc/popup.html",
            "ftp://example.com/",
            "",
            "example.com",
        ] {
            assert_eq!(m("https://example.com", Domain, page), None, "{page}");
        }
    }

    #[test]
    fn port_rules() {
        assert_eq!(
            m("https://example.com", Origin, "https://example.com:443/"),
            Some(SameHost)
        );
        assert_eq!(
            m("https://example.com", Origin, "https://example.com:8443/"),
            None
        );
        assert_eq!(
            m("https://example.com:8443", Domain, "https://example.com/"),
            None
        );
        assert_eq!(
            m("http://example.com", Origin, "https://example.com:443/"),
            Some(SameHost)
        );
        assert_eq!(
            m(
                "http://example.com:8080",
                Origin,
                "https://example.com:8080/"
            ),
            Some(SameHost)
        );
    }

    #[test]
    fn exact_rules_compare_path_only() {
        let r = "https://example.com/login";
        assert_eq!(
            m(r, Exact, "https://example.com/login?next=/home#top"),
            Some(ExactUrl)
        );
        assert_eq!(m(r, Exact, "https://example.com/login/other"), None);
        assert_eq!(m(r, Exact, "https://example.com/"), None);
        assert_eq!(m(r, Exact, "https://www.example.com/login"), None);
    }

    #[test]
    fn normalization() {
        assert_eq!(
            m("https://Example.COM", Origin, "https://EXAMPLE.com./x"),
            Some(SameHost)
        );
        assert_eq!(
            m("https://bücher.de", Domain, "https://www.xn--bcher-kva.de/"),
            Some(SameSite)
        );
    }

    #[test]
    fn best_rule_wins() {
        let item = ItemOverview {
            id: uuid::Uuid::nil(),
            item_type: crate::model::ItemType::Login,
            title: "x".into(),
            username: None,
            urls: vec![
                rule("https://example.com", Domain),
                rule("https://app.example.com/login", Exact),
            ],
            has_password: true,
            has_totp: false,
            has_notes: false,
            has_passkey: false,
            created_at: 0,
            updated_at: 0,
        };
        let page = PageUrl::parse("https://app.example.com/login").unwrap();
        assert_eq!(match_item(&item, &page), Some(ExactUrl));
        let page = PageUrl::parse("https://other.example.com/").unwrap();
        assert_eq!(match_item(&item, &page), Some(SameSite));
        let page = PageUrl::parse("https://example.org/").unwrap();
        assert_eq!(match_item(&item, &page), None);
    }

    #[test]
    fn never_panics_on_odd_input() {
        let rules = [
            rule("https://example.com", Domain),
            UrlRule {
                url: "not a url".into(),
                match_type: Domain,
            },
        ];
        let pages = [
            "https://",
            "https://.",
            "https://..example.com",
            "https://example..com",
            "https://-a-.com",
            "https://a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.example.com",
            "https://[::ffff:127.0.0.1]/",
            "https://0x7f.1/",
            "https://%65xample.com/",
            "https://xn--/",
        ];
        for p in pages {
            if let Some(page) = PageUrl::parse(p) {
                for r in &rules {
                    let _ = match_rule(r, &page);
                }
            }
        }
        // Percent-encoded host is decoded by the parser: still the same host.
        assert_eq!(
            m("https://example.com", Origin, "https://%65xample.com/"),
            Some(SameHost)
        );
        assert!(PageUrl::parse(&format!("https://a.com/{}", "x".repeat(20_000))).is_none());
    }
}
