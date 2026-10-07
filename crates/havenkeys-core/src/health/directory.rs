//! The two site lists Vault health checks logins against, compiled in from
//! `crates/havenkeys-core/data/` (2factorauth data, refreshed by scripts and
//! reviewed like code; never fetched). Third-party text: every entry is
//! validated, and one that fails is dropped, never a panic.
//!
//! Matching is by DNS labels, never string suffixes: a login host matches a
//! listed domain when it is that domain or a subdomain of it, and the walk
//! stops at the host's registrable domain (Public Suffix List), so
//! `evilgithub.com` and `github.com.evil.com` never match `github.com`.

use crate::origin::{host_key, registrable_domain_of};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use url::Url;

const MAX_SITES: usize = 20_000;
const MAX_NAME: usize = 100;

pub(crate) struct Site {
    pub(crate) help: Option<String>,
}

pub(crate) struct Directory {
    sites: Vec<Site>,
    by_domain: HashMap<String, usize>,
}

#[derive(Deserialize)]
struct RawSite {
    name: serde_json::Value,
    domains: serde_json::Value,
    #[serde(default)]
    passwordless: Option<bool>,
    #[serde(default)]
    help: serde_json::Value,
}

/// A lowercase DNS name as the URL parser would write it, or `None`.
fn clean_host(v: &serde_json::Value) -> Option<String> {
    let s = v.as_str()?;
    if s.is_empty() || s.len() > 253 || !s.contains('.') || s.ends_with('.') {
        return None;
    }
    let url = Url::parse(&format!("https://{s}/")).ok()?;
    let host = host_key(&url)?;
    (host == s).then_some(host)
}

fn clean_help(v: &serde_json::Value) -> Option<String> {
    let url = Url::parse(v.as_str()?).ok()?;
    (url.scheme() == "https" && url.host().is_some()).then(|| url.to_string())
}

fn clean_name(v: &serde_json::Value) -> bool {
    v.as_str().is_some_and(|n| {
        let n = n.trim();
        !n.is_empty() && n.chars().count() <= MAX_NAME && !n.chars().any(char::is_control)
    })
}

impl Directory {
    /// `require_passwordless`: keep only sites where a passkey replaces the
    /// password (the passkey list); the TOTP list passes `false`.
    pub(crate) fn parse(json: &str, require_passwordless: bool) -> Self {
        let mut dir = Directory {
            sites: Vec::new(),
            by_domain: HashMap::new(),
        };
        let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
            return dir;
        };
        for entry in entries.into_iter().take(MAX_SITES) {
            let Ok(raw) = serde_json::from_value::<RawSite>(entry) else {
                continue;
            };
            if !clean_name(&raw.name) {
                continue;
            }
            if require_passwordless && raw.passwordless != Some(true) {
                continue;
            }
            let Some(list) = raw.domains.as_array() else {
                continue;
            };
            let domains: Vec<String> = list.iter().filter_map(clean_host).collect();
            if domains.is_empty() {
                continue;
            }
            let idx = dir.sites.len();
            dir.sites.push(Site {
                help: clean_help(&raw.help),
            });
            for d in domains {
                dir.by_domain.entry(d).or_insert(idx);
            }
        }
        dir
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.sites.len()
    }

    /// The site listing `host` or one of its parent domains, down to (and
    /// including) the host's registrable domain. A host without one (an IP
    /// address, `localhost`, an unknown suffix) matches nothing.
    pub(crate) fn lookup(&self, host: &str) -> Option<&Site> {
        let site = registrable_domain_of(host)?;
        let mut h = host;
        loop {
            if let Some(&i) = self.by_domain.get(h) {
                return self.sites.get(i);
            }
            if h.len() <= site.len() {
                return None;
            }
            h = h.split_once('.')?.1;
        }
    }
}

impl Directory {
    /// The https help page of the first of `hosts` this list has one for.
    /// Shared by the report's `help` flag and `health_help_url`, so the two
    /// never disagree.
    pub(crate) fn help_for<'a>(&self, hosts: impl IntoIterator<Item = &'a str>) -> Option<&str> {
        hosts
            .into_iter()
            .find_map(|h| self.lookup(h).and_then(|s| s.help.as_deref()))
    }
}

/// Scheme and normalized host of a saved website rule; only http(s).
pub(crate) fn rule_host(url: &str) -> Option<(String, String)> {
    let url = Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    Some((
        url.scheme().to_owned(),
        host_key(&url)?.to_ascii_lowercase(),
    ))
}

pub(crate) fn passkey_sites() -> &'static Directory {
    static DIR: OnceLock<Directory> = OnceLock::new();
    DIR.get_or_init(|| Directory::parse(include_str!("../../data/passkey-sites.json"), true))
}

pub(crate) fn twofactor_sites() -> &'static Directory {
    static DIR: OnceLock<Directory> = OnceLock::new();
    DIR.get_or_init(|| Directory::parse(include_str!("../../data/twofactor-sites.json"), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"name":"GitHub","domains":["github.com"],"passwordless":true,"mfa":true,"help":"https://docs.github.com/passkeys"},
      {"name":"MfaOnly","domains":["mfa-only.com"],"passwordless":false,"mfa":true,"help":null},
      {"name":"Bad host","domains":["not a host"],"passwordless":true,"mfa":false,"help":null},
      {"name":"Http help","domains":["plain.com"],"passwordless":true,"mfa":false,"help":"http://plain.com/help"},
      {"name":"Deep","domains":["accounts.deep.com"],"passwordless":true,"mfa":false,"help":null},
      42
    ]"#;

    fn dir() -> Directory {
        Directory::parse(SAMPLE, true)
    }

    #[test]
    fn exact_and_subdomain_hosts_match() {
        let d = dir();
        assert!(d.lookup("github.com").is_some());
        assert!(d.lookup("gist.github.com").is_some());
        assert!(d.lookup("a.b.github.com").is_some());
    }

    #[test]
    fn lookalike_hosts_never_match() {
        let d = dir();
        assert!(d.lookup("evilgithub.com").is_none());
        assert!(d.lookup("github.com.evil.com").is_none());
        assert!(d.lookup("github-login.example.com").is_none());
        assert!(d.lookup("com").is_none());
    }

    #[test]
    fn a_parent_of_a_listed_subdomain_does_not_match() {
        let d = dir();
        assert!(d.lookup("deep.com").is_none());
        assert!(d.lookup("accounts.deep.com").is_some());
    }

    #[test]
    fn passwordless_filter_and_bad_entries_are_dropped() {
        let d = dir();
        assert!(d.lookup("mfa-only.com").is_none());
        assert!(Directory::parse(SAMPLE, false)
            .lookup("mfa-only.com")
            .is_some());
        assert!(
            d.lookup("plain.com").unwrap().help.is_none(),
            "http help links are dropped"
        );
        assert_eq!(
            d.lookup("github.com").unwrap().help.as_deref(),
            Some("https://docs.github.com/passkeys")
        );
    }

    #[test]
    fn help_for_is_the_first_listed_host_with_an_https_link() {
        let d = dir();
        assert_eq!(
            d.help_for(["unlisted.example", "gist.github.com"]),
            Some("https://docs.github.com/passkeys")
        );
        assert_eq!(d.help_for(["accounts.deep.com"]), None, "listed, help null");
        assert_eq!(d.help_for(["plain.com"]), None, "listed, http help");
        assert_eq!(
            d.help_for(["accounts.deep.com", "github.com"]),
            Some("https://docs.github.com/passkeys")
        );
        assert_eq!(d.help_for(["unlisted.example"]), None);
    }

    #[test]
    fn garbage_parses_to_an_empty_directory() {
        for junk in ["", "{}", "null", "[[[", "[{\"domains\": 5}]", "\u{0}"] {
            assert!(Directory::parse(junk, false).lookup("github.com").is_none());
        }
    }

    #[test]
    fn lookup_normalizes_hosts() {
        let d = dir();
        let (_, host) = rule_host("https://GitHub.com./login").unwrap();
        assert!(d.lookup(&host).is_some());
        let (_, idn) = rule_host("https://bücher.example/").unwrap();
        assert_eq!(idn, "xn--bcher-kva.example");
    }

    #[test]
    fn rule_host_only_accepts_http_and_https() {
        assert_eq!(rule_host("http://example.com").unwrap().0, "http");
        assert!(rule_host("ftp://example.com").is_none());
        assert!(rule_host("not a url").is_none());
    }

    #[test]
    fn bundled_directories_parse() {
        assert!(passkey_sites().len() > 50);
        assert!(twofactor_sites().len() > 500);
        assert!(twofactor_sites().lookup("github.com").is_some());
    }

    #[test]
    fn hostile_entries_never_panic() {
        let cases = [
            r#"[{"name":"x","domains":["a.com"],"help":{"nested":[1,2]}}]"#,
            r#"[{"name":"\u0007bell","domains":["a.com"]}]"#,
            r#"[{"name":"x","domains":[".", "..", "a..com", "-a.com", "a.com."]}]"#,
            r#"[{"name":"x","domains":["xn--"]}]"#,
        ];
        for c in cases {
            let d = Directory::parse(c, false);
            let _ = d.lookup("a.com");
        }
        let long = format!(r#"[{{"name":"x","domains":["{}.com"]}}]"#, "a".repeat(300));
        assert!(Directory::parse(&long, false).lookup("a.com").is_none());
    }
}
