//! Digital Asset Links (spec 2026-10-01-android-app §7.2): does a website
//! vouch for an Android app? Parsing, the cache and the choice of which
//! sites to ask live here; the HTTPS fetch is the mobile crate's (the core
//! has no network). Every input is untrusted: a site's file is parsed
//! statement by statement, and one bad statement voids only itself.

use crate::app_target::{parse_fingerprint, valid_package, AppIdentity, CERT_LEN};
use crate::error::{Error, Result};
use crate::origin::registrable_domain_of;
use data_encoding::HEXLOWER;
use serde::{Deserialize, Serialize};

pub const MAX_ASSET_LINKS_BYTES: usize = 128 * 1024;
pub const MAX_STATEMENTS: usize = 256;
pub const FRESH_MS: i64 = 7 * 24 * 60 * 60 * 1000;
pub const RETRY_FAILED_MS: i64 = 60 * 60 * 1000;
pub const MAX_CACHED_HOSTS: usize = 256;
pub const MAX_CANDIDATES: usize = 8;

const LOGIN_RELATIONS: [&str; 2] = [
    "delegate_permission/common.handle_all_urls",
    "delegate_permission/common.get_login_creds",
];

const GET_LOGIN_CREDS: &str = "delegate_permission/common.get_login_creds";

/// Labels too common in package names to say anything about the site.
const GENERIC_LABELS: [&str; 14] = [
    "com", "org", "net", "app", "apps", "android", "mobile", "client", "www", "beta", "debug",
    "release", "prod", "lite",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStatement {
    pub package: String,
    /// SHA-256 certificate digests, lowercase hex.
    pub certs: Vec<String>,
    /// The statement grants `common.get_login_creds`, which passkeys need
    /// (spec §8.1); `handle_all_urls` alone is enough for filling only.
    #[serde(default)]
    pub login_creds: bool,
}

#[derive(Deserialize)]
struct RawStatement {
    #[serde(default)]
    relation: Vec<String>,
    target: RawTarget,
}

#[derive(Deserialize)]
struct RawTarget {
    namespace: String,
    #[serde(default)]
    package_name: Option<String>,
    #[serde(default)]
    sha256_cert_fingerprints: Vec<String>,
}

pub fn parse(body: &[u8]) -> Result<Vec<AppStatement>> {
    if body.len() > MAX_ASSET_LINKS_BYTES {
        return Err(Error::InvalidInput("asset links file too large"));
    }
    let raw: Vec<serde_json::Value> = serde_json::from_slice(body)
        .map_err(|_| Error::InvalidInput("asset links file is not valid"))?;
    if raw.len() > MAX_STATEMENTS {
        return Err(Error::InvalidInput("asset links file too large"));
    }
    let mut out = Vec::new();
    for value in raw {
        let Ok(s) = serde_json::from_value::<RawStatement>(value) else {
            continue;
        };
        if s.target.namespace != "android_app"
            || !s
                .relation
                .iter()
                .any(|r| LOGIN_RELATIONS.contains(&r.as_str()))
        {
            continue;
        }
        let login_creds = s.relation.iter().any(|r| r == GET_LOGIN_CREDS);
        let Some(package) = s.target.package_name.filter(|p| valid_package(p)) else {
            continue;
        };
        let certs: Vec<String> = s
            .target
            .sha256_cert_fingerprints
            .iter()
            .filter_map(|f| parse_fingerprint(f))
            .map(|c| HEXLOWER.encode(&c))
            .collect();
        if !certs.is_empty() {
            out.push(AppStatement {
                package,
                certs,
                login_creds,
            });
        }
    }
    Ok(out)
}

pub fn vouches_for(statements: &[AppStatement], app: &AppIdentity) -> bool {
    statements.iter().any(|s| {
        s.package == app.package()
            && app
                .certs()
                .iter()
                .any(|c| s.certs.contains(&HEXLOWER.encode(c)))
    })
}

/// The certificate of `app` that a statement grants `get_login_creds`: the
/// one an app's passkey origin names.
pub fn login_creds_cert(statements: &[AppStatement], app: &AppIdentity) -> Option<[u8; CERT_LEN]> {
    statements
        .iter()
        .filter(|s| s.login_creds && s.package == app.package())
        .find_map(|s| {
            app.certs()
                .iter()
                .find(|c| s.certs.contains(&HEXLOWER.encode(&c[..])))
                .copied()
        })
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AssetLinksCache {
    hosts: Vec<CachedHost>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CachedHost {
    host: String,
    checked_at_ms: i64,
    /// `None`: the fetch failed (unreachable, redirect, too large, not JSON).
    statements: Option<Vec<AppStatement>>,
}

impl CachedHost {
    fn fresh(&self, now_ms: i64) -> bool {
        let limit = if self.statements.is_some() {
            FRESH_MS
        } else {
            RETRY_FAILED_MS
        };
        (0..=limit).contains(&now_ms.saturating_sub(self.checked_at_ms))
    }
}

impl AssetLinksCache {
    pub fn is_fresh(&self, host: &str, now_ms: i64) -> bool {
        self.hosts.iter().any(|h| h.host == host && h.fresh(now_ms))
    }

    /// The statements of a fresh, successful fetch of `host`.
    pub fn statements_for(&self, host: &str, now_ms: i64) -> Option<&[AppStatement]> {
        self.hosts
            .iter()
            .find(|h| h.host == host && h.fresh(now_ms))
            .and_then(|h| h.statements.as_deref())
    }

    pub fn record(&mut self, host: &str, now_ms: i64, statements: Option<Vec<AppStatement>>) {
        self.hosts.retain(|h| h.host != host);
        self.hosts.push(CachedHost {
            host: host.to_owned(),
            checked_at_ms: now_ms,
            statements,
        });
        if self.hosts.len() > MAX_CACHED_HOSTS {
            self.hosts
                .sort_by_key(|h| std::cmp::Reverse(h.checked_at_ms));
            self.hosts.truncate(MAX_CACHED_HOSTS);
        }
    }

    pub fn verified_hosts(&self, app: &AppIdentity, now_ms: i64) -> Vec<String> {
        self.hosts
            .iter()
            .filter(|h| h.fresh(now_ms))
            .filter(|h| h.statements.as_deref().is_some_and(|s| vouches_for(s, app)))
            .map(|h| h.host.clone())
            .collect()
    }
}

/// Which of the vault's sites to ask about an app: those whose registrable
/// domain's first label appears in the package name (`com.github.android` →
/// `github.com`). Only narrows the lookup; the file itself is what vouches.
pub fn candidate_hosts(package: &str, hosts: &[String]) -> Vec<String> {
    let labels: Vec<String> = package
        .split('.')
        .map(str::to_ascii_lowercase)
        .filter(|l| l.len() >= 3 && !GENERIC_LABELS.contains(&l.as_str()))
        .collect();
    let mut out = Vec::new();
    for host in hosts {
        let Some(site) = registrable_domain_of(host) else {
            continue;
        };
        let first = site.split('.').next().unwrap_or_default();
        if labels.iter().any(|l| l == first) && !out.contains(host) {
            out.push(host.clone());
            if out.len() == MAX_CANDIDATES {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_target::AppIdentity;

    const FP: &str = "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99";
    const NOW: i64 = 1_800_000_000_000;

    fn app() -> AppIdentity {
        AppIdentity::new(
            "com.github.android",
            &[crate::app_target::parse_fingerprint(FP).unwrap().to_vec()],
        )
        .unwrap()
    }

    fn file(relation: &str, package: &str) -> String {
        format!(
            r#"[{{"relation":["{relation}"],"target":{{"namespace":"android_app","package_name":"{package}","sha256_cert_fingerprints":["{FP}"]}}}}]"#
        )
    }

    #[test]
    fn a_login_creds_statement_vouches_for_its_app() {
        let s = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(vouches_for(&s, &app()));
        let s = parse(
            file(
                "delegate_permission/common.handle_all_urls",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(vouches_for(&s, &app()));
    }

    #[test]
    fn other_relations_packages_and_certificates_do_not() {
        let s = parse(
            file(
                "delegate_permission/common.use_as_origin",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(!vouches_for(&s, &app()));
        let s =
            parse(file("delegate_permission/common.get_login_creds", "com.evil.app").as_bytes())
                .unwrap();
        assert!(!vouches_for(&s, &app()));
        let other = AppIdentity::new("com.github.android", &[vec![1; 32]]).unwrap();
        let s = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(!vouches_for(&s, &other));
    }

    #[test]
    fn a_malformed_statement_does_not_void_the_others() {
        let body = format!(
            r#"[{{"relation":"not a list"}}, {{"target":{{"namespace":"web","site":"https://x"}},"relation":["delegate_permission/common.get_login_creds"]}}, {}]"#,
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android"
            )
            .trim_start_matches('[')
            .trim_end_matches(']')
        );
        assert!(vouches_for(&parse(body.as_bytes()).unwrap(), &app()));
    }

    #[test]
    fn oversized_or_invalid_files_are_errors() {
        assert!(parse(&vec![b' '; MAX_ASSET_LINKS_BYTES + 1]).is_err());
        assert!(parse(b"{}").is_err());
        assert!(parse(b"not json").is_err());
        let many = format!("[{}]", vec!["{}"; MAX_STATEMENTS + 1].join(","));
        assert!(parse(many.as_bytes()).is_err());
    }

    #[test]
    fn the_cache_keeps_files_a_week_and_failures_an_hour() {
        let mut c = AssetLinksCache::default();
        let s = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        c.record("github.com", NOW, Some(s));
        c.record("down.example", NOW, None);
        assert!(c.is_fresh("github.com", NOW + FRESH_MS));
        assert!(!c.is_fresh("github.com", NOW + FRESH_MS + 1));
        assert!(c.is_fresh("down.example", NOW + RETRY_FAILED_MS));
        assert!(!c.is_fresh("down.example", NOW + RETRY_FAILED_MS + 1));
        assert_eq!(
            c.verified_hosts(&app(), NOW + 1),
            vec!["github.com".to_string()]
        );
        assert!(c.verified_hosts(&app(), NOW + FRESH_MS + 1).is_empty());
        // A clock set back does not make an entry fresh forever.
        assert!(!c.is_fresh("github.com", NOW - 1));
    }

    #[test]
    fn the_cache_is_bounded() {
        let mut c = AssetLinksCache::default();
        for i in 0..(MAX_CACHED_HOSTS + 10) {
            c.record(&format!("h{i}.example"), NOW + i as i64, None);
        }
        assert!(!c.is_fresh("h0.example", NOW + MAX_CACHED_HOSTS as i64 + 10));
        assert!(c.is_fresh(
            &format!("h{}.example", MAX_CACHED_HOSTS + 9),
            NOW + MAX_CACHED_HOSTS as i64 + 10
        ));
    }

    #[test]
    fn candidates_come_from_the_package_name() {
        let hosts = vec![
            "github.com".into(),
            "www.github.com".into(),
            "gitlab.com".into(),
            "android.com".into(),
        ];
        assert_eq!(
            candidate_hosts("com.github.android", &hosts),
            vec!["github.com".to_string(), "www.github.com".to_string()]
        );
        assert!(candidate_hosts("com.example.app", &hosts).is_empty());
    }

    #[test]
    fn candidates_are_capped() {
        let hosts: Vec<String> = (0..20).map(|i| format!("s{i}.github.com")).collect();
        assert_eq!(
            candidate_hosts("com.github.android", &hosts).len(),
            MAX_CANDIDATES
        );
    }

    #[test]
    fn get_login_creds_is_recorded_per_statement() {
        let creds = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(creds[0].login_creds);
        let urls = parse(
            file(
                "delegate_permission/common.handle_all_urls",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(!urls[0].login_creds);
        assert_eq!(
            login_creds_cert(&creds, &app()),
            Some(crate::app_target::parse_fingerprint(FP).unwrap())
        );
        assert_eq!(
            login_creds_cert(&urls, &app()),
            None,
            "handle_all_urls alone is for filling only"
        );
    }

    #[test]
    fn login_creds_cert_needs_the_same_package_and_certificate() {
        let s = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.other.app",
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(login_creds_cert(&s, &app()), None);
        let other_cert = AppIdentity::new("com.github.android", &[vec![1; 32]]).unwrap();
        let s = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(login_creds_cert(&s, &other_cert), None);
    }

    #[test]
    fn statements_for_answers_only_fresh_successful_fetches() {
        let s = parse(
            file(
                "delegate_permission/common.get_login_creds",
                "com.github.android",
            )
            .as_bytes(),
        )
        .unwrap();
        let mut cache = AssetLinksCache::default();
        cache.record("github.com", NOW, Some(s));
        cache.record("down.example", NOW, None);
        assert_eq!(
            cache.statements_for("github.com", NOW).map(<[_]>::len),
            Some(1)
        );
        assert!(cache
            .statements_for("github.com", NOW + FRESH_MS + 1)
            .is_none());
        assert!(cache.statements_for("down.example", NOW).is_none());
        assert!(cache.statements_for("other.example", NOW).is_none());
    }

    #[test]
    fn a_cache_written_before_login_creds_existed_still_reads() {
        let old = r#"{"hosts":[{"host":"github.com","checked_at_ms":1,"statements":[{"package":"com.x.y","certs":["aa"]}]}]}"#;
        let cache: AssetLinksCache = serde_json::from_str(old).unwrap();
        assert!(!cache.statements_for("github.com", 1).unwrap()[0].login_creds);
    }
}
