//! Where to change a login's password (spec
//! `2026-11-14-change-password-url-design.md`).
//!
//! No network here. This module decides, from a login's saved websites:
//!
//! 1. the site's change-password page from a curated table compiled in from
//!    `data/change-password-urls.json` (Apple's Password Manager Resources,
//!    MIT; refreshed by `scripts/update-change-password-urls.mjs` and reviewed
//!    like code, never fetched);
//! 2. otherwise the origin the desktop may probe for the W3C
//!    `/.well-known/change-password` URL, on an explicit click only, plus the
//!    rules that probe must follow (https, public DNS names, public
//!    addresses);
//! 3. otherwise the login's own website.
//!
//! Matching is by DNS labels, never string suffixes, and stops at the host's
//! registrable domain (Public Suffix List), as in `health/directory.rs`:
//! `evilgithub.com` and `github.com.evil.com` never match `github.com`.

use crate::model::{normalize_url, ItemOverview, ItemType};
use crate::origin::{host_key, registrable_domain_of};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::OnceLock;
use url::{Host, Url};

/// The W3C "A Well-Known URL for Changing Passwords" path.
pub const WELL_KNOWN_PATH: &str = "/.well-known/change-password";

/// Asked first: a site that answers 2xx here answers 2xx to anything, so
/// its answer for [`WELL_KNOWN_PATH`] means nothing (W3C spec §4).
pub const NOT_EXIST_PATH: &str =
    "/.well-known/resource-that-should-not-exist-whose-status-code-should-not-be-200";

/// At most this many redirects per probe request.
pub const MAX_REDIRECTS: usize = 5;

const MAX_ENTRIES: usize = 20_000;
const MAX_URL_LEN: usize = 2048;

/// What to open when the user asks to change a login's password. Every
/// address in it is https (the curated page, the probe origin) or a saved
/// website after the same check as saving (http or https).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangePasswordPlan {
    /// The curated change-password page for one of the login's websites.
    pub curated: Option<String>,
    /// `https://<host>/` of the first website the desktop may probe.
    pub probe_origin: Option<Url>,
    /// The login's first website, normalized: the fallback.
    pub website: String,
}

/// The plan for `item`, or `None` when it is not a login or has no http(s)
/// website.
pub fn plan(item: &ItemOverview) -> Option<ChangePasswordPlan> {
    if item.item_type != ItemType::Login {
        return None;
    }
    let websites: Vec<String> = item
        .urls
        .iter()
        .filter_map(|r| normalize_url(&r.url).ok())
        .collect();
    let website = websites.first()?.clone();
    Some(ChangePasswordPlan {
        curated: websites
            .iter()
            .find_map(|w| curated_url(w).map(str::to_owned)),
        probe_origin: websites.iter().find_map(|w| probe_origin(w)),
        website,
    })
}

/// The curated change-password page for `website` (a saved address): its
/// exact host first, then each parent domain down to its registrable domain.
pub fn curated_url(website: &str) -> Option<&'static str> {
    let url = Url::parse(website).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let Some(Host::Domain(_)) = url.host() else {
        return None;
    };
    let host = host_key(&url)?.to_ascii_lowercase();
    table().lookup(&host)
}

/// `https://<host>/` when `website`'s host may be probed: a DNS name under a
/// known public suffix (never an IP address, `localhost`, a single label or
/// a private suffix such as `.local` or `.internal`), on the default port.
/// The scheme of the saved address does not matter: the probe is https only.
pub fn probe_origin(website: &str) -> Option<Url> {
    let url = Url::parse(website).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.port().is_some() {
        return None;
    }
    let host = public_dns_host(&url)?;
    Url::parse(&format!("https://{host}/")).ok()
}

/// Whether the probe may request `url`, including every redirect it is
/// sent: https, a public DNS name ([`probe_origin`]'s rules), the default
/// port, no credentials.
pub fn probe_may_request(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && public_dns_host(url).is_some()
}

/// Whether the probe may connect to `ip`: a globally routed unicast
/// address. Loopback, private, link-local, carrier-grade NAT, documentation,
/// benchmarking, multicast, broadcast and reserved ranges are refused, so a
/// name that resolves (or rebinds) to the user's own network gets nothing.
pub fn is_public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let s = v6.segments();
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                // Unique local fc00::/7, link-local fe80::/10.
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                // Site-local fec0::/10 (deprecated).
                || (s[0] & 0xffc0) == 0xfec0
                // Documentation 2001:db8::/32.
                || (s[0] == 0x2001 && s[1] == 0x0db8)
                // IPv4-compatible ::/96 and NAT64 64:ff9b::/96 carry an
                // IPv4 address that may be private.
                || (s[0..6] == [0; 6])
                || (s[0] == 0x0064 && s[1] == 0xff9b)
                // 6to4 2002::/16 embeds an IPv4 address too.
                || s[0] == 0x2002)
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        // Carrier-grade NAT 100.64.0.0/10.
        || (a == 100 && (b & 0xc0) == 64)
        // IETF protocol assignments 192.0.0.0/24.
        || (a == 192 && b == 0 && c == 0)
        // Benchmarking 198.18.0.0/15.
        || (a == 198 && (b & 0xfe) == 18)
        // Reserved 240.0.0.0/4.
        || a >= 240)
}

/// The URL's host when it is a DNS name with a registrable domain under a
/// known public suffix.
fn public_dns_host(url: &Url) -> Option<String> {
    let Some(Host::Domain(_)) = url.host() else {
        return None;
    };
    let host = host_key(url)?.to_ascii_lowercase();
    registrable_domain_of(&host)?;
    Some(host)
}

// ------------------------------------------------------------------ table

struct Table {
    by_domain: HashMap<String, String>,
}

impl Table {
    /// Parse the table, dropping (never panicking on) an entry whose key is
    /// not a lowercase DNS name or whose page is not an https URL with a
    /// host and no credentials.
    fn parse(json: &str) -> (Self, usize) {
        let mut table = Table {
            by_domain: HashMap::new(),
        };
        let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json)
        else {
            return (table, 0);
        };
        let mut dropped = 0;
        for (key, value) in map.into_iter().take(MAX_ENTRIES) {
            match (clean_domain(&key), value.as_str().and_then(clean_page)) {
                (Some(d), Some(p)) => {
                    table.by_domain.insert(d, p);
                }
                _ => dropped += 1,
            }
        }
        (table, dropped)
    }

    /// The exact host first (so a key that is itself a public suffix, such
    /// as `gov.br`, matches only that host), then each parent domain down to
    /// the host's registrable domain.
    fn lookup(&self, host: &str) -> Option<&str> {
        if let Some(page) = self.by_domain.get(host) {
            return Some(page);
        }
        let site = registrable_domain_of(host)?;
        let mut h = host;
        loop {
            if let Some(page) = self.by_domain.get(h) {
                return Some(page);
            }
            if h.len() <= site.len() {
                return None;
            }
            h = h.split_once('.')?.1;
        }
    }
}

/// A lowercase DNS name exactly as the URL parser writes it.
fn clean_domain(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > 253 || !s.contains('.') || s.ends_with('.') {
        return None;
    }
    let url = Url::parse(&format!("https://{s}/")).ok()?;
    match url.host() {
        Some(Host::Domain(d)) if d == s => Some(s.to_owned()),
        _ => None,
    }
}

fn clean_page(s: &str) -> Option<String> {
    if s.len() > MAX_URL_LEN {
        return None;
    }
    let url = Url::parse(s).ok()?;
    let ok = url.scheme() == "https"
        && matches!(url.host(), Some(Host::Domain(_)))
        && url.username().is_empty()
        && url.password().is_none();
    ok.then(|| url.to_string())
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| Table::parse(include_str!("../data/change-password-urls.json")).0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MatchType, UrlRule};

    const BUNDLED: &str = include_str!("../data/change-password-urls.json");

    fn login(urls: &[&str]) -> ItemOverview {
        let mut item: ItemOverview = serde_json::from_value(serde_json::json!({
            "id": "6f0b7f0e-0000-4000-8000-000000000001",
            "itemType": "login",
            "title": "t",
            "hasPassword": true,
            "hasTotp": false,
            "hasNotes": false,
            "createdAt": 0,
            "updatedAt": 0
        }))
        .expect("overview");
        item.urls = urls
            .iter()
            .map(|u| UrlRule {
                url: (*u).to_owned(),
                match_type: MatchType::Domain,
            })
            .collect();
        item
    }

    #[test]
    fn bundled_table_is_valid_in_full() {
        // Every entry survives the parser: no key that is not a DNS name,
        // no page that is not https with a host.
        let raw: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(BUNDLED).expect("the bundled file is a JSON object");
        let (table, dropped) = Table::parse(BUNDLED);
        assert_eq!(
            dropped, 0,
            "an entry of change-password-urls.json is invalid"
        );
        assert_eq!(table.by_domain.len(), raw.len());
        assert!(raw.len() > 500, "the table looks truncated");
        for (key, value) in &raw {
            assert_eq!(clean_domain(key).as_deref(), Some(key.as_str()), "{key}");
            assert!(
                psl::suffix(key.as_bytes()).is_some_and(|s| s.is_known()),
                "{key} is not under a known public suffix"
            );
            let page = Url::parse(value.as_str().expect("string")).expect("url");
            assert_eq!(page.scheme(), "https", "{key}");
            assert!(matches!(page.host(), Some(Host::Domain(_))), "{key}");
        }
    }

    #[test]
    fn exact_host_then_registrable_domain() {
        let (t, _) = Table::parse(
            r#"{"example.com":"https://example.com/pw","accounts.example.com":"https://accounts.example.com/pw"}"#,
        );
        assert_eq!(t.lookup("example.com"), Some("https://example.com/pw"));
        assert_eq!(t.lookup("www.example.com"), Some("https://example.com/pw"));
        assert_eq!(
            t.lookup("a.accounts.example.com"),
            Some("https://accounts.example.com/pw")
        );
        assert_eq!(
            t.lookup("accounts.example.com"),
            Some("https://accounts.example.com/pw")
        );
    }

    #[test]
    fn lookalikes_never_match() {
        let (t, _) = Table::parse(r#"{"github.com":"https://github.com/settings/security"}"#);
        assert!(t.lookup("gist.github.com").is_some());
        for host in [
            "evilgithub.com",
            "github.com.evil.com",
            "github-login.example.com",
            "com",
        ] {
            assert_eq!(t.lookup(host), None, "{host}");
        }
    }

    #[test]
    fn a_parent_of_a_listed_subdomain_does_not_match() {
        let (t, _) = Table::parse(r#"{"accounts.deep.com":"https://accounts.deep.com/pw"}"#);
        assert_eq!(t.lookup("deep.com"), None);
    }

    #[test]
    fn a_public_suffix_key_matches_only_itself() {
        let (t, _) = Table::parse(r#"{"gov.br":"https://acesso.gov.br/pw"}"#);
        assert_eq!(t.lookup("gov.br"), Some("https://acesso.gov.br/pw"));
        assert_eq!(t.lookup("receita.gov.br"), None);
        assert_eq!(t.lookup("sso.acesso.gov.br"), None);
    }

    #[test]
    fn hostile_entries_are_dropped() {
        let (t, dropped) = Table::parse(
            r#"{
              "ok.com": "https://ok.com/pw",
              "http.com": "http://http.com/pw",
              "js.com": "javascript:alert(1)",
              "creds.com": "https://u:p@creds.com/pw",
              "ip.com": "https://127.0.0.1/pw",
              "Upper.com": "https://upper.com/pw",
              "not a host": "https://x.com/",
              "num.com": 5,
              "trailing.com.": "https://trailing.com/"
            }"#,
        );
        assert_eq!(dropped, 8);
        assert_eq!(t.lookup("ok.com"), Some("https://ok.com/pw"));
        for junk in ["", "[]", "null", "{", "\u{0}"] {
            assert!(Table::parse(junk).0.by_domain.is_empty());
        }
    }

    #[test]
    fn curated_url_uses_the_bundled_table() {
        assert_eq!(
            curated_url("https://accounts.google.com/login"),
            Some("https://myaccount.google.com/signinoptions/password")
        );
        assert!(curated_url("https://www.amazon.com/").is_some());
        assert_eq!(curated_url("https://evilgoogle.com/"), None);
        assert_eq!(curated_url("https://google.com.evil.com/"), None);
        assert_eq!(curated_url("https://127.0.0.1/"), None);
        assert_eq!(curated_url("ftp://github.com/"), None);
        assert_eq!(curated_url("not a url"), None);
    }

    #[test]
    fn probe_origin_is_https_and_public_names_only() {
        assert_eq!(
            probe_origin("http://Example.COM./login?x=1")
                .unwrap()
                .as_str(),
            "https://example.com/"
        );
        assert_eq!(
            probe_origin("https://a.b.example.co.uk/").unwrap().as_str(),
            "https://a.b.example.co.uk/"
        );
        for bad in [
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://10.0.0.8/",
            "https://localhost/",
            "https://router/",
            "https://nas.local/",
            "https://host.internal/",
            "https://example.com:8443/",
            "ftp://example.com/",
            "https://com/",
            "nonsense",
        ] {
            assert!(probe_origin(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn probe_may_request_follows_the_same_rules() {
        let ok = |s: &str| probe_may_request(&Url::parse(s).unwrap());
        assert!(ok("https://example.com/.well-known/change-password"));
        assert!(ok("https://myaccount.example.org/security"));
        assert!(!ok("http://example.com/"));
        assert!(!ok("https://example.com:444/"));
        assert!(!ok("https://u:p@example.com/"));
        assert!(!ok("https://192.168.1.1/"));
        assert!(!ok("https://localhost/"));
        assert!(!ok("https://printer.local/"));
    }

    #[test]
    fn public_addresses() {
        let public = |s: &str| is_public_address(s.parse().unwrap());
        for ok in [
            "93.184.216.34",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(public(ok), "{ok}");
        }
        for bad in [
            "127.0.0.1",
            "0.0.0.0",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "192.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "::",
            "::1",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.1.1",
            "64:ff9b::a00:1",
            "2002:c0a8:101::1",
            "::127.0.0.1",
        ] {
            assert!(!public(bad), "{bad}");
        }
    }

    #[test]
    fn plan_for_a_login() {
        let p = plan(&login(&["https://accounts.google.com/login"])).unwrap();
        assert!(p.curated.is_some());
        assert_eq!(
            p.probe_origin.unwrap().as_str(),
            "https://accounts.google.com/"
        );
        assert_eq!(p.website, "https://accounts.google.com/login");

        // The curated page comes from any of the websites, the fallback
        // from the first.
        let p = plan(&login(&[
            "https://unlisted-site.com/",
            "https://google.com/",
        ]))
        .unwrap();
        assert!(p.curated.is_some());
        assert_eq!(p.website, "https://unlisted-site.com/");

        let p = plan(&login(&[
            "http://192.168.1.1/admin",
            "https://unlisted-site.com/",
        ]))
        .unwrap();
        assert_eq!(p.curated, None);
        assert_eq!(
            p.probe_origin.unwrap().as_str(),
            "https://unlisted-site.com/"
        );
        assert_eq!(p.website, "http://192.168.1.1/admin");

        let p = plan(&login(&["http://nas.local:5000/"])).unwrap();
        assert_eq!((p.curated, p.probe_origin), (None, None));
    }

    #[test]
    fn no_plan_without_a_website_or_for_other_items() {
        assert!(plan(&login(&[])).is_none());
        let mut note = login(&["https://google.com/"]);
        note.item_type = ItemType::SecureNote;
        assert!(plan(&note).is_none());
    }
}
