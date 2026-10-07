//! Vault health (spec 2026-10-07-vault-health): which logins have a weak,
//! reused or old password, an http website, a duplicate, or a site that
//! offers passkeys or one-time codes the login does not use. Computed from
//! the unlocked vault; the report holds item IDs and check kinds only.

pub(crate) mod directory;
mod vault;

pub use vault::HEALTH_CACHE_MS;

use crate::secret::SecretString;
use directory::{passkey_sites, twofactor_sites};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroize;

/// A password unchanged for longer than this is "old".
pub const OLD_AFTER_MS: i64 = 365 * 24 * 3600 * 1000;
/// zxcvbn scores below this are "weak" (0, 1, 2 of 0–4).
const STRONG_ENOUGH: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheck {
    Weak,
    Reused,
    Old,
    Passkey,
    TwoFactor,
    Insecure,
    Duplicate,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCounts {
    pub weak: u32,
    pub reused: u32,
    pub old: u32,
    pub passkey: u32,
    pub two_factor: u32,
    pub insecure: u32,
    pub duplicate: u32,
}

impl HealthCounts {
    fn add(&mut self, check: HealthCheck) {
        let slot = match check {
            HealthCheck::Weak => &mut self.weak,
            HealthCheck::Reused => &mut self.reused,
            HealthCheck::Old => &mut self.old,
            HealthCheck::Passkey => &mut self.passkey,
            HealthCheck::TwoFactor => &mut self.two_factor,
            HealthCheck::Insecure => &mut self.insecure,
            HealthCheck::Duplicate => &mut self.duplicate,
        };
        *slot += 1;
    }
}

/// One login's problems. IDs and kinds only (spec §5.1).
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssue {
    pub item_id: Uuid,
    pub checks: Vec<HealthCheck>,
    pub reused_group: Option<u32>,
    pub duplicate_group: Option<u32>,
    /// The issue's Passkey or TwoFactor check has an https setup guide in
    /// the directory for this login's own site (`health_help_url` succeeds).
    pub help: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDismissed {
    pub item_id: Uuid,
    pub checks: Vec<HealthCheck>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub computed_at: i64,
    pub counts: HealthCounts,
    pub issues: Vec<HealthIssue>,
    pub dismissed: Vec<HealthDismissed>,
}

impl fmt::Debug for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthReport")
            .field("counts", &self.counts)
            .field("issues", &self.issues.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for HealthIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthIssue")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for HealthDismissed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthDismissed")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

/// What `compute` needs to know about one login, read under the vault
/// lock. Holds the password; dropped (zeroized) as soon as the report is built.
pub(crate) struct LoginFacts {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) username: Option<String>,
    /// (scheme, host) of each http(s) website rule.
    pub(crate) hosts: Vec<(String, String)>,
    pub(crate) password: Option<SecretString>,
    pub(crate) last_changed: i64,
    pub(crate) has_totp: bool,
    pub(crate) has_passkey: bool,
    pub(crate) ignored: Vec<HealthCheck>,
    /// `vault::dedupe_key_parts` digest (no password in it).
    pub(crate) dedupe: [u8; 32],
}

impl Drop for LoginFacts {
    fn drop(&mut self) {
        self.title.zeroize();
        self.username.zeroize();
        for (_, h) in &mut self.hosts {
            h.zeroize();
        }
    }
}

/// The logins as they were when the snapshot was taken, and which session
/// state (`generation`, `epoch`) that was, so a stale report is not cached.
pub struct HealthSnapshot {
    pub(crate) logins: Vec<LoginFacts>,
    pub(crate) generation: u64,
    pub(crate) epoch: u64,
}

impl fmt::Debug for HealthSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthSnapshot")
            .field("logins", &self.logins.len())
            .finish_non_exhaustive()
    }
}

fn is_weak(f: &LoginFacts, password: &str) -> bool {
    let mut inputs: Vec<&str> = vec![f.title.as_str()];
    if let Some(u) = f.username.as_deref() {
        inputs.push(u);
    }
    u8::from(zxcvbn::zxcvbn(password, &inputs).score()) < STRONG_ENOUGH
}

/// Groups of two or more logins sharing a key, numbered by each group's
/// smallest item ID. Returns login index → group number.
fn groups<K: std::hash::Hash + Eq>(
    keys: impl Iterator<Item = (usize, K)>,
    ids: &[Uuid],
) -> HashMap<usize, u32> {
    let mut by_key: HashMap<K, Vec<usize>> = HashMap::new();
    for (i, k) in keys {
        by_key.entry(k).or_default().push(i);
    }
    let mut multi: Vec<Vec<usize>> = by_key.into_values().filter(|v| v.len() > 1).collect();
    multi.sort_by_key(|v| v.iter().map(|&i| ids[i]).min());
    let mut out = HashMap::new();
    for (g, members) in multi.into_iter().enumerate() {
        for i in members {
            out.insert(i, g as u32);
        }
    }
    out
}

pub fn compute(snapshot: &HealthSnapshot, now_ms: i64) -> HealthReport {
    let logins = &snapshot.logins;
    let ids: Vec<Uuid> = logins.iter().map(|f| f.id).collect();
    let reused = groups(
        logins.iter().enumerate().filter_map(|(i, f)| {
            let p = f.password.as_ref()?.expose();
            (!p.is_empty()).then_some((i, p))
        }),
        &ids,
    );
    let duplicate = groups(logins.iter().enumerate().map(|(i, f)| (i, f.dedupe)), &ids);

    let mut counts = HealthCounts::default();
    let mut issues = Vec::new();
    let mut dismissed = Vec::new();
    // BTreeMap: one deterministic order (by item ID) for the UI.
    let mut ordered: BTreeMap<Uuid, usize> = BTreeMap::new();
    for (i, f) in logins.iter().enumerate() {
        ordered.insert(f.id, i);
    }
    for (_, i) in ordered {
        let f = &logins[i];
        let mut found = Vec::new();
        let password = f
            .password
            .as_ref()
            .map(|p| p.expose())
            .filter(|p| !p.is_empty());
        if let Some(p) = password {
            if is_weak(f, p) {
                found.push(HealthCheck::Weak);
            }
            if reused.contains_key(&i) {
                found.push(HealthCheck::Reused);
            }
            if now_ms - f.last_changed > OLD_AFTER_MS {
                found.push(HealthCheck::Old);
            }
        }
        let hosts = || f.hosts.iter().map(|(_, h)| h.as_str());
        let passkey = !f.has_passkey && hosts().any(|h| passkey_sites().lookup(h).is_some());
        if passkey {
            found.push(HealthCheck::Passkey);
        } else if !f.has_totp && hosts().any(|h| twofactor_sites().lookup(h).is_some()) {
            found.push(HealthCheck::TwoFactor);
        }
        if f.hosts
            .iter()
            .any(|(s, h)| s == "http" && crate::origin::registrable_domain_of(h).is_some())
        {
            found.push(HealthCheck::Insecure);
        }
        if duplicate.contains_key(&i) {
            found.push(HealthCheck::Duplicate);
        }
        let (gone, kept): (Vec<_>, Vec<_>) = found.into_iter().partition(|c| f.ignored.contains(c));
        for c in &kept {
            counts.add(*c);
        }
        if !gone.is_empty() {
            dismissed.push(HealthDismissed {
                item_id: f.id,
                checks: gone,
            });
        }
        if !kept.is_empty() {
            let has = |c| kept.contains(&c);
            let dir = if has(HealthCheck::Passkey) {
                Some(passkey_sites())
            } else if has(HealthCheck::TwoFactor) {
                Some(twofactor_sites())
            } else {
                None
            };
            issues.push(HealthIssue {
                help: dir.is_some_and(|d| d.help_for(hosts()).is_some()),
                item_id: f.id,
                reused_group: has(HealthCheck::Reused).then(|| reused[&i]),
                duplicate_group: has(HealthCheck::Duplicate).then(|| duplicate[&i]),
                checks: kept,
            });
        }
    }
    HealthReport {
        computed_at: now_ms,
        counts,
        issues,
        dismissed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretString;
    use uuid::Uuid;

    const NOW: i64 = 1_800_000_000_000;
    const STRONG: &str = "q7$Vt!m2Lz#9pWx4Rk@e";

    fn facts(n: u128, url: &str, password: Option<&str>) -> LoginFacts {
        LoginFacts {
            id: Uuid::from_u128(n),
            title: format!("Login {n}"),
            username: Some(format!("user{n}")),
            hosts: directory::rule_host(url).into_iter().collect(),
            password: password.map(SecretString::from),
            last_changed: NOW,
            has_totp: false,
            has_passkey: false,
            ignored: Vec::new(),
            dedupe: [n as u8; 32],
        }
    }

    fn report(logins: Vec<LoginFacts>) -> HealthReport {
        compute(
            &HealthSnapshot {
                logins,
                generation: 0,
                epoch: 0,
            },
            NOW,
        )
    }

    fn checks(r: &HealthReport, n: u128) -> Vec<HealthCheck> {
        r.issues
            .iter()
            .find(|i| i.item_id == Uuid::from_u128(n))
            .map(|i| i.checks.clone())
            .unwrap_or_default()
    }

    #[test]
    fn weak_passwords_are_flagged_and_strong_ones_are_not() {
        let r = report(vec![
            facts(1, "https://unlisted-site.example", Some("password1")),
            facts(2, "https://unlisted-site.example", Some(STRONG)),
        ]);
        assert!(checks(&r, 1).contains(&HealthCheck::Weak));
        assert!(!checks(&r, 2).contains(&HealthCheck::Weak));
        assert_eq!(r.counts.weak, 1);
    }

    #[test]
    fn a_password_built_from_the_title_is_weak() {
        let mut f = facts(1, "https://unlisted-site.example", Some("Zephyrwind2024"));
        f.title = "Zephyrwind".into();
        assert!(checks(&report(vec![f]), 1).contains(&HealthCheck::Weak));
    }

    #[test]
    fn reused_passwords_form_groups_ordered_by_item_id() {
        let r = report(vec![
            facts(9, "https://a.example", Some(STRONG)),
            facts(3, "https://b.example", Some(STRONG)),
            facts(5, "https://c.example", Some("another-Strong-pass-77!")),
            facts(1, "https://d.example", Some("another-Strong-pass-77!")),
            facts(7, "https://e.example", Some("unique-Strong-pass-99?")),
        ]);
        let g = |n| {
            r.issues
                .iter()
                .find(|i| i.item_id == Uuid::from_u128(n))
                .and_then(|i| i.reused_group)
        };
        assert_eq!(g(1), Some(0));
        assert_eq!(g(5), Some(0));
        assert_eq!(g(3), Some(1));
        assert_eq!(g(9), Some(1));
        assert_eq!(g(7), None);
        assert_eq!(r.counts.reused, 4);
    }

    #[test]
    fn empty_and_missing_passwords_are_not_reused() {
        let r = report(vec![
            facts(1, "https://a.example", Some("")),
            facts(2, "https://b.example", Some("")),
            facts(3, "https://c.example", None),
            facts(4, "https://d.example", None),
        ]);
        assert_eq!(r.counts.reused, 0);
    }

    #[test]
    fn old_is_strictly_more_than_a_year() {
        let mut exactly = facts(1, "https://a.example", Some(STRONG));
        exactly.last_changed = NOW - OLD_AFTER_MS;
        let mut older = facts(2, "https://b.example", Some("other-Strong-pass-12#"));
        older.last_changed = NOW - OLD_AFTER_MS - 1;
        let mut no_password = facts(3, "https://c.example", None);
        no_password.last_changed = 0;
        let r = report(vec![exactly, older, no_password]);
        assert!(!checks(&r, 1).contains(&HealthCheck::Old));
        assert!(checks(&r, 2).contains(&HealthCheck::Old));
        assert!(!checks(&r, 3).contains(&HealthCheck::Old));
    }

    #[test]
    fn passkey_sites_win_over_two_factor() {
        // github.com is in both bundled lists.
        let r = report(vec![facts(1, "https://github.com", Some(STRONG))]);
        assert!(checks(&r, 1).contains(&HealthCheck::Passkey));
        assert!(!checks(&r, 1).contains(&HealthCheck::TwoFactor));
    }

    #[test]
    fn a_login_with_a_passkey_gets_the_two_factor_suggestion_instead() {
        let mut f = facts(1, "https://github.com", Some(STRONG));
        f.has_passkey = true;
        let r = report(vec![f]);
        assert!(!checks(&r, 1).contains(&HealthCheck::Passkey));
        assert!(checks(&r, 1).contains(&HealthCheck::TwoFactor));
    }

    fn help(r: &HealthReport, n: u128) -> bool {
        r.issues
            .iter()
            .find(|i| i.item_id == Uuid::from_u128(n))
            .is_some_and(|i| i.help)
    }

    #[test]
    fn help_says_whether_the_site_publishes_a_setup_guide() {
        // github.com has help links in both lists; airasia.com (passkey) and
        // ngpvan.com (two-factor) are listed with `help: null`.
        let mut totp_only = facts(4, "https://github.com", Some(STRONG));
        totp_only.has_passkey = true;
        let r = report(vec![
            facts(1, "https://github.com", Some(STRONG)),
            facts(2, "https://airasia.com", Some(STRONG)),
            facts(3, "https://ngpvan.com", Some(STRONG)),
            totp_only,
            facts(5, "http://example.com", Some(STRONG)),
        ]);
        assert!(checks(&r, 2).contains(&HealthCheck::Passkey));
        assert!(checks(&r, 3).contains(&HealthCheck::TwoFactor));
        assert!(help(&r, 1), "passkey, github.com");
        assert!(!help(&r, 2), "passkey, no help link");
        assert!(!help(&r, 3), "two-factor, no help link");
        assert!(help(&r, 4), "two-factor, github.com");
        assert!(!help(&r, 5), "no site check");
    }

    #[test]
    fn lookalike_hosts_get_no_site_suggestion() {
        for url in [
            "https://evilgithub.com",
            "https://github.com.evil.com",
            "https://github-login.example.com",
        ] {
            let r = report(vec![facts(1, url, Some(STRONG))]);
            assert!(!checks(&r, 1).contains(&HealthCheck::Passkey), "{url}");
            assert!(!checks(&r, 1).contains(&HealthCheck::TwoFactor), "{url}");
        }
    }

    #[test]
    fn a_login_with_totp_is_not_asked_for_two_factor() {
        let mut f = facts(1, "https://github.com", Some(STRONG));
        f.has_passkey = true;
        f.has_totp = true;
        assert!(checks(&report(vec![f]), 1).is_empty());
    }

    #[test]
    fn http_websites_are_unsecured_except_local_devices() {
        let flagged = report(vec![facts(1, "http://example.com", Some(STRONG))]);
        assert!(checks(&flagged, 1).contains(&HealthCheck::Insecure));
        for url in [
            "https://example.com",
            "http://localhost:8080",
            "http://192.168.0.1",
            "http://router.lan",
        ] {
            let r = report(vec![facts(1, url, Some(STRONG))]);
            assert!(!checks(&r, 1).contains(&HealthCheck::Insecure), "{url}");
        }
    }

    #[test]
    fn duplicates_group_by_digest_regardless_of_password() {
        let mut a = facts(1, "https://a.example", Some(STRONG));
        let mut b = facts(2, "https://a.example", Some("different-Strong-pass-3$"));
        a.dedupe = [7; 32];
        b.dedupe = [7; 32];
        let c = facts(3, "https://a.example", Some("third-Strong-pass-4%"));
        let r = report(vec![a, b, c]);
        assert!(checks(&r, 1).contains(&HealthCheck::Duplicate));
        assert!(checks(&r, 2).contains(&HealthCheck::Duplicate));
        assert!(!checks(&r, 3).contains(&HealthCheck::Duplicate));
        assert_eq!(r.counts.duplicate, 2);
    }

    #[test]
    fn dismissed_checks_move_out_of_issues_and_counts() {
        let mut f = facts(1, "http://example.com", Some("password1"));
        f.ignored = vec![HealthCheck::Weak];
        let r = report(vec![f]);
        assert_eq!(checks(&r, 1), vec![HealthCheck::Insecure]);
        assert_eq!(r.counts.weak, 0);
        assert_eq!(r.dismissed.len(), 1);
        assert_eq!(r.dismissed[0].checks, vec![HealthCheck::Weak]);
    }

    #[test]
    fn a_login_with_nothing_wrong_is_not_listed() {
        let r = report(vec![facts(
            1,
            "https://unlisted-site.example",
            Some(STRONG),
        )]);
        assert!(r.issues.is_empty());
        assert!(r.dismissed.is_empty());
    }

    #[test]
    fn long_and_unicode_passwords_do_not_panic() {
        let long = "a".repeat(10_000);
        let r = report(vec![
            facts(1, "https://a.example", Some(&long)),
            facts(2, "https://b.example", Some("🔐🔐🔐ção-Ünïcode-ŝtrong-✓")),
        ]);
        assert_eq!(r.computed_at, NOW);
    }

    #[test]
    fn the_serialized_report_never_contains_a_password() {
        let pw = "Leaky-Secret-Pass-0042";
        let r = report(vec![
            facts(1, "https://github.com", Some(pw)),
            facts(2, "https://github.com", Some(pw)),
        ]);
        let json = serde_json::to_string(&r).unwrap();
        for window in pw.as_bytes().windows(4) {
            assert!(
                !json.contains(std::str::from_utf8(window).unwrap()),
                "leaked {:?}",
                window
            );
        }
        assert!(!format!("{r:?}").contains("Leaky"));
    }
}
