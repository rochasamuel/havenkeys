//! Passwords a site accepts (spec 2026-11-14-password-rules-generator).
//!
//! A sign-up page may declare what its passwords must look like, in the
//! `passwordrules` attribute Apple proposed for HTML
//! (`minlength: 8; maxlength: 16; required: lower, upper; required: digit;
//! allowed: [-().&@?'#,/"+]; max-consecutive: 2;`). Many sites do not, so a
//! reviewed, compiled table of Apple's "password rules" quirks
//! (`data/password-rules.json`) fills in for them. The field's own
//! `minlength`/`maxlength` always bound the length too.
//!
//! Sources, highest priority first: the field's `passwordrules`, the table
//! entry for the page's host or else its registrable domain, then the plain
//! policy. A source that does not parse, or asks for something the generator
//! cannot meet, is skipped for the next; the plain policy never fails where
//! [`generator::generate`] succeeds.
//!
//! Every value here comes from the page (or its URL) and is untrusted. That
//! is acceptable because a page's rules only shape the one password the same
//! page then receives: a hostile page can make its own password weaker,
//! never another site's, and never read anything. The length never drops
//! below [`RULES_MIN_LENGTH`], and below the usual [`generator::MIN_LENGTH`]
//! only when the page's own maximum length demands it.
//!
//! Parsing is the `password-rules-parser` crate (1Password's). We generate
//! ASCII only: `unicode` reads as ASCII printable (as that crate does), and
//! the space character is never generated (many sites trim it).

use crate::error::Result;
use crate::generator::{self, GeneratedPassword, GeneratorOptions, MAX_LENGTH, MIN_LENGTH};
use crate::origin::{host_key, registrable_domain_of, PageUrl};
use password_rules_parser::{parse_password_rules, CharacterClass, PasswordRules};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The shortest password generated for a page whose own `maxlength` (or
/// rules) demand fewer than [`MIN_LENGTH`] characters. Below this, the
/// page's limits are ignored and the plain policy applies.
pub const RULES_MIN_LENGTH: usize = 4;

/// A rules string longer than this is not looked at (the protocol bound).
pub const MAX_RULES_BYTES: usize = 1024;

const TABLE_JSON: &str = include_str!("../data/password-rules.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    rules: HashMap<String, String>,
}

fn table() -> &'static HashMap<String, String> {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str::<Table>(TABLE_JSON)
            .map(|t| t.rules)
            .unwrap_or_default()
    })
}

/// What the password field said about itself. All page-controlled.
#[derive(Clone, Copy, Debug, Default)]
pub struct FieldConstraints<'a> {
    /// The field's `passwordrules` attribute.
    pub rules: Option<&'a str>,
    /// The field's `minlength`.
    pub min_length: Option<u32>,
    /// The field's `maxlength`.
    pub max_length: Option<u32>,
}

/// Which rules a password followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSource {
    /// The field's own `passwordrules` attribute.
    Field,
    /// The bundled table's entry for the site.
    Site,
}

/// A password for a page, and which rules it followed (`None`: the plain
/// policy, perhaps shortened or lengthened by the field's min/maxlength).
#[derive(Debug)]
pub struct PagePassword {
    pub generated: GeneratedPassword,
    pub followed: Option<RuleSource>,
}

/// The table's rules for a page: its host, else its registrable domain.
pub fn site_rules(page_url: &str) -> Option<&'static str> {
    let page = PageUrl::parse(page_url)?;
    let host = host_key(page.url())?;
    let t = table();
    t.get(&host)
        .or_else(|| registrable_domain_of(&host).and_then(|d| t.get(&d)))
        .map(String::as_str)
}

/// A rules-following plan the generator can meet.
struct Plan {
    required: Vec<Vec<u8>>,
    alphabet: Vec<u8>,
    length: usize,
    max_consecutive: Option<usize>,
}

/// Characters we generate from a class: printable ASCII but space.
fn chars_of(classes: &[CharacterClass]) -> Vec<u8> {
    let mut seen = [false; 128];
    for class in classes {
        for c in class.chars() {
            if c.is_ascii_graphic() {
                seen[c as usize] = true;
            }
        }
    }
    (0u8..128).filter(|c| seen[*c as usize]).collect()
}

/// `sets` without ambiguous characters, unless that empties one of them.
fn without_ambiguous(sets: &[Vec<u8>]) -> Option<Vec<Vec<u8>>> {
    let filtered: Vec<Vec<u8>> = sets
        .iter()
        .map(|s| {
            s.iter()
                .copied()
                .filter(|c| !generator::is_ambiguous(*c))
                .collect()
        })
        .collect();
    filtered
        .iter()
        .all(|s: &Vec<u8>| !s.is_empty())
        .then_some(filtered)
}

/// The length range `[lo, hi]` both bounds allow, or `None`. At least
/// [`MIN_LENGTH`] unless `hi` is lower, never below [`RULES_MIN_LENGTH`],
/// never above [`MAX_LENGTH`].
fn length_range(min: Option<u32>, max: Option<u32>) -> Option<(usize, usize)> {
    let hi = max.map_or(MAX_LENGTH, |m| (m as usize).min(MAX_LENGTH));
    let lo = (min.unwrap_or(0) as usize).max(MIN_LENGTH.min(hi));
    (hi >= RULES_MIN_LENGTH && lo <= hi).then_some((lo, hi))
}

fn tighter(a: Option<u32>, b: Option<u32>, pick: fn(u32, u32) -> u32) -> Option<u32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(pick(x, y)),
        (x, y) => x.or(y),
    }
}

fn plan(policy: &GeneratorOptions, rules: &str, field: &FieldConstraints) -> Option<Plan> {
    if rules.len() > MAX_RULES_BYTES {
        return None;
    }
    let parsed: PasswordRules = parse_password_rules(rules, true).ok()?;
    let min = tighter(parsed.min_length, field.min_length, u32::max);
    let max = tighter(parsed.max_length, field.max_length, u32::min);
    let (lo, hi) = length_range(min, max)?;
    let length = policy.length.clamp(lo, hi);

    let mut required: Vec<Vec<u8>> = parsed.required.iter().map(|g| chars_of(g)).collect();
    if required.iter().any(Vec::is_empty) || required.len() > length {
        return None;
    }
    let mut every: Vec<CharacterClass> = parsed.allowed.clone();
    every.extend(parsed.required.iter().flatten().cloned());
    let mut alphabet = chars_of(&every);
    if alphabet.is_empty() {
        return None;
    }
    if policy.avoid_ambiguous {
        let mut sets = required.clone();
        sets.push(alphabet.clone());
        if let Some(mut f) = without_ambiguous(&sets) {
            alphabet = f.pop()?;
            required = f;
        }
    }
    let max_consecutive = parsed
        .max_consecutive
        .map(|n| n as usize)
        .filter(|n| *n > 0 && *n < length);
    if max_consecutive.is_some() && alphabet.len() < 2 {
        return None;
    }
    Some(Plan {
        required,
        alphabet,
        length,
        max_consecutive,
    })
}

/// The rules a page's password would follow, in priority order: the
/// field's, then the site's. Skips a source with no workable plan.
fn rules_plan(
    policy: &GeneratorOptions,
    page_url: &str,
    field: &FieldConstraints,
) -> Option<(Plan, RuleSource)> {
    let field_rules = field
        .rules
        .filter(|r| !r.trim().is_empty())
        .and_then(|r| plan(policy, r, field))
        .map(|p| (p, RuleSource::Field));
    field_rules.or_else(|| {
        site_rules(page_url)
            .and_then(|r| plan(policy, r, field))
            .map(|p| (p, RuleSource::Site))
    })
}

/// Which source [`generate_for_page`] would follow, without generating.
/// The menu says so before the user picks.
pub fn followed_for_page(
    policy: &GeneratorOptions,
    page_url: &str,
    field: &FieldConstraints,
) -> Result<Option<RuleSource>> {
    policy.validate()?;
    Ok(rules_plan(policy, page_url, field).map(|(_, s)| s))
}

/// A password for the field on `page_url`: following the field's rules,
/// else the site's, else the plain `policy` bounded by the field's
/// min/maxlength. Fails only where [`generator::generate`] fails (an
/// invalid policy).
pub fn generate_for_page(
    policy: &GeneratorOptions,
    page_url: &str,
    field: &FieldConstraints,
) -> Result<PagePassword> {
    policy.validate()?;
    if let Some((p, source)) = rules_plan(policy, page_url, field) {
        if let Ok(generated) =
            generator::build(&p.required, &p.alphabet, p.length, p.max_consecutive)
        {
            return Ok(PagePassword {
                generated,
                followed: Some(source),
            });
        }
    }
    Ok(PagePassword {
        generated: plain(policy, field)?,
        followed: None,
    })
}

/// The plain policy, its length moved into the field's min/maxlength when
/// that range is workable (else exactly as [`generator::generate`]).
fn plain(policy: &GeneratorOptions, field: &FieldConstraints) -> Result<GeneratedPassword> {
    let classes = generator::policy_classes(policy);
    let length = match length_range(field.min_length, field.max_length) {
        Some((lo, hi)) if hi >= classes.len() => policy.length.clamp(lo, hi),
        _ => policy.length,
    };
    if length == policy.length {
        return generator::generate(policy);
    }
    let alphabet = classes.concat();
    generator::build(&classes, &alphabet, length, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://signup.example.org/new";

    fn policy(length: usize) -> GeneratorOptions {
        GeneratorOptions {
            length,
            ..Default::default()
        }
    }

    fn field(rules: &str) -> FieldConstraints<'_> {
        FieldConstraints {
            rules: Some(rules),
            ..Default::default()
        }
    }

    fn gen(p: &GeneratorOptions, f: &FieldConstraints) -> (String, Option<RuleSource>) {
        let r = generate_for_page(p, URL, f).unwrap();
        (r.generated.password.expose().to_owned(), r.followed)
    }

    fn many(p: &GeneratorOptions, f: &FieldConstraints, check: impl Fn(&str)) {
        for _ in 0..300 {
            let (pw, followed) = gen(p, f);
            assert_eq!(followed, Some(RuleSource::Field));
            check(&pw);
        }
    }

    #[test]
    fn every_bundled_rule_parses() {
        let t: Table = serde_json::from_str(TABLE_JSON).expect("password-rules.json shape");
        assert!(t.rules.len() > 400);
        for (domain, rules) in &t.rules {
            assert!(
                domain.is_ascii() && *domain == domain.to_ascii_lowercase(),
                "{domain}"
            );
            assert!(
                PageUrl::parse(&format!("https://{domain}/")).is_some(),
                "{domain}"
            );
            assert!(rules.len() <= MAX_RULES_BYTES, "{domain}");
            assert!(
                parse_password_rules(rules, true).is_ok(),
                "{domain}: {rules}"
            );
        }
    }

    #[test]
    fn every_bundled_rule_is_followed() {
        // A rule we cannot meet would fall back to the plain policy; the
        // shipped table has none, so a parser or generator change shows up.
        let t: Table = serde_json::from_str(TABLE_JSON).unwrap();
        let mut unmet = Vec::new();
        for (domain, rules) in &t.rules {
            let url = format!("https://{domain}/");
            let r = generate_for_page(&policy(24), &url, &FieldConstraints::default()).unwrap();
            match r.followed {
                Some(RuleSource::Site) => meets(rules, r.generated.password.expose()),
                _ => unmet.push(format!("{domain}: {rules}")),
            }
        }
        assert!(unmet.is_empty(), "{unmet:#?}");
    }

    /// `pw` keeps every rule in `rules` (as the parser reads them).
    fn meets(rules: &str, pw: &str) {
        let p = parse_password_rules(rules, true).unwrap();
        let n = pw.len() as u32;
        assert!(p.min_length.is_none_or(|m| n >= m.min(128)), "{rules}");
        assert!(p.max_length.is_none_or(|m| n <= m), "{rules}");
        let mut all = p.allowed.clone();
        all.extend(p.required.iter().flatten().cloned());
        let allowed = chars_of(&all);
        assert!(pw.bytes().all(|c| allowed.contains(&c)), "{rules}");
        for group in &p.required {
            let set = chars_of(group);
            assert!(pw.bytes().any(|c| set.contains(&c)), "{rules}");
        }
        if let Some(m) = p.max_consecutive.filter(|m| *m > 0) {
            let m = m as usize;
            assert!(
                !pw.as_bytes()
                    .windows(m + 1)
                    .any(|w| w.iter().all(|c| *c == w[0])),
                "{rules}"
            );
        }
    }

    #[test]
    fn each_class_alone() {
        let only = |rules: &str, ok: fn(u8) -> bool| {
            many(&policy(20), &field(rules), |pw| {
                assert_eq!(pw.len(), 20);
                assert!(pw.bytes().all(ok), "{rules}");
            });
        };
        only("required: upper;", |c| c.is_ascii_uppercase());
        only("required: lower;", |c| c.is_ascii_lowercase());
        only("required: digit;", |c| c.is_ascii_digit());
        only("required: special;", |c| c.is_ascii_punctuation());
        only("allowed: ascii-printable;", |c| c.is_ascii_graphic());
        // unicode reads as ASCII printable; never a space.
        only("allowed: unicode;", |c| c.is_ascii_graphic());
    }

    #[test]
    fn required_classes_always_appear() {
        many(
            &policy(8),
            &field("required: lower; required: upper; required: digit; required: special;"),
            |pw| {
                assert!(pw.bytes().any(|c| c.is_ascii_lowercase()));
                assert!(pw.bytes().any(|c| c.is_ascii_uppercase()));
                assert!(pw.bytes().any(|c| c.is_ascii_digit()));
                assert!(pw.bytes().any(|c| c.is_ascii_punctuation()));
            },
        );
    }

    #[test]
    fn a_required_group_needs_one_of_its_union() {
        // "required: lower, upper" alone: letters only, at least one.
        many(
            &policy(16),
            &field("required: lower, upper; required: digit;"),
            |pw| {
                assert!(pw.bytes().all(|c| c.is_ascii_alphanumeric()));
                assert!(pw.bytes().any(|c| c.is_ascii_alphabetic()));
                assert!(pw.bytes().any(|c| c.is_ascii_digit()));
            },
        );
    }

    #[test]
    fn custom_classes() {
        many(
            &policy(24),
            &field("required: digit; allowed: lower, [-().&@?'#,/\"+];"),
            |pw| {
                assert!(pw.bytes().all(|c| c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || b"-().&@?'#,/\"+".contains(&c)));
                assert!(pw.bytes().any(|c| c.is_ascii_digit()));
            },
        );
        // A custom required class: one of its characters every time.
        many(
            &policy(12),
            &field("allowed: lower; required: [!#$];"),
            |pw| {
                assert!(pw.bytes().any(|c| b"!#$".contains(&c)));
                assert!(pw
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || b"!#$".contains(&c)));
            },
        );
        // A custom class of nothing we generate (only a space): no plan.
        let (_, followed) = gen(&policy(12), &field("required: [ ];"));
        assert_eq!(followed, None);
    }

    #[test]
    fn max_consecutive_is_enforced() {
        many(
            &policy(64),
            &field("allowed: [ab]; max-consecutive: 1;"),
            |pw| {
                assert!(!pw.as_bytes().windows(2).any(|w| w[0] == w[1]), "{pw}");
            },
        );
        many(
            &policy(128),
            &field("allowed: digit; max-consecutive: 2;"),
            |pw| {
                assert!(!pw
                    .as_bytes()
                    .windows(3)
                    .any(|w| w[0] == w[1] && w[1] == w[2]));
            },
        );
        // One character and a run limit: cannot be met.
        let (_, followed) = gen(&policy(12), &field("allowed: [a]; max-consecutive: 2;"));
        assert_eq!(followed, None);
    }

    #[test]
    fn length_is_the_policy_clamped_into_the_rules_and_field() {
        let (pw, f) = gen(&policy(24), &field("minlength: 6; maxlength: 16;"));
        assert_eq!((pw.len(), f), (16, Some(RuleSource::Field)));
        let (pw, _) = gen(&policy(10), &field("minlength: 20;"));
        assert_eq!(pw.len(), 20);
        // The field's own maxlength bounds the rules' range too.
        let f = FieldConstraints {
            rules: Some("minlength: 6; maxlength: 30;"),
            min_length: None,
            max_length: Some(12),
        };
        assert_eq!(gen(&policy(24), &f).0.len(), 12);
        // Above MAX_LENGTH: capped.
        assert_eq!(gen(&policy(128), &field("minlength: 100;")).0.len(), 128);
    }

    #[test]
    fn short_maxlength_relaxes_the_minimum_down_to_the_floor() {
        let (pw, f) = gen(&policy(24), &field("maxlength: 6; required: digit;"));
        assert_eq!((pw.len(), f), (6, Some(RuleSource::Field)));
        let (pw, _) = gen(&policy(24), &field("maxlength: 4; allowed: digit;"));
        assert_eq!(pw.len(), RULES_MIN_LENGTH);
        // Below the floor: the rules are not followed; the plain policy
        // (and the field's bounds, none here) apply.
        let (pw, f) = gen(&policy(24), &field("maxlength: 3; allowed: digit;"));
        assert_eq!((pw.len(), f), (24, None));
        // A minlength alone never shortens below MIN_LENGTH.
        assert_eq!(
            gen(&policy(8), &field("minlength: 2; allowed: digit;"))
                .0
                .len(),
            8
        );
    }

    #[test]
    fn unsatisfiable_or_unparsable_rules_fall_back() {
        for rules in [
            "minlength: 20; maxlength: 10;",
            "required: digit; required: upper; required: lower; required: special; maxlength: 4; required: [!]; ",
            "this is not a rule",
            "minlength: ;;;; maxlength",
            "required: [",
        ] {
            let (pw, f) = gen(&policy(24), &field(rules));
            assert_eq!(f, None, "{rules}");
            assert_eq!(pw.len(), 24, "{rules}");
        }
        let long = "required: digit; ".repeat(100);
        assert_eq!(gen(&policy(24), &field(&long)).1, None);
    }

    #[test]
    fn field_min_and_max_length_bound_the_plain_policy() {
        let f = FieldConstraints {
            rules: None,
            min_length: None,
            max_length: Some(12),
        };
        let r = generate_for_page(&policy(24), "https://no-rules.invalid/", &f).unwrap();
        assert_eq!(r.generated.password.expose().len(), 12);
        assert_eq!(r.followed, None);
        let f = FieldConstraints {
            min_length: Some(30),
            ..Default::default()
        };
        assert_eq!(
            generate_for_page(&policy(24), "https://no-rules.invalid/", &f)
                .unwrap()
                .generated
                .password
                .expose()
                .len(),
            30
        );
        // Unworkable bounds (min above max, or max below the floor) are ignored.
        for (min, max) in [(Some(20), Some(10)), (None, Some(3))] {
            let f = FieldConstraints {
                rules: None,
                min_length: min,
                max_length: max,
            };
            let r = generate_for_page(&policy(24), "https://no-rules.invalid/", &f).unwrap();
            assert_eq!(r.generated.password.expose().len(), 24);
        }
        // Four classes need four characters: a maxlength of 4 still fits.
        let f = FieldConstraints {
            max_length: Some(4),
            ..Default::default()
        };
        let r = generate_for_page(&policy(24), "https://no-rules.invalid/", &f).unwrap();
        assert_eq!(r.generated.password.expose().len(), 4);
    }

    #[test]
    fn nothing_given_is_exactly_the_policy() {
        let p = GeneratorOptions {
            length: 20,
            uppercase: false,
            lowercase: false,
            digits: true,
            symbols: false,
            avoid_ambiguous: false,
        };
        let r = generate_for_page(
            &p,
            "https://no-rules.invalid/",
            &FieldConstraints::default(),
        )
        .unwrap();
        assert_eq!(r.followed, None);
        let pw = r.generated.password.expose().to_owned();
        assert!(pw.len() == 20 && pw.bytes().all(|c| c.is_ascii_digit()));
        // An invalid policy still fails, as generate does.
        assert!(generate_for_page(&policy(7), URL, &FieldConstraints::default()).is_err());
        // A blank attribute is no rules.
        assert_eq!(gen(&policy(24), &field("  ")).1, None);
    }

    #[test]
    fn site_rules_by_host_then_registrable_domain() {
        // From the bundled table (Apple's quirks).
        assert!(site_rules("https://163.com/").is_some());
        assert!(site_rules("https://reg.163.com/signup").is_some());
        assert!(site_rules("https://163.com.evil.example/").is_none());
        assert!(site_rules("https://evil163.com/").is_none());
        assert!(site_rules("file:///163.com").is_none());
        let r = generate_for_page(
            &policy(24),
            "https://163.com/",
            &FieldConstraints::default(),
        )
        .unwrap();
        assert_eq!(r.followed, Some(RuleSource::Site));
        // 163.com: "minlength: 6; maxlength: 16;"
        assert_eq!(r.generated.password.expose().len(), 16);
        // The field's rules win over the site's.
        let r =
            generate_for_page(&policy(24), "https://163.com/", &field("allowed: digit;")).unwrap();
        assert_eq!(r.followed, Some(RuleSource::Field));
        assert!(r
            .generated
            .password
            .expose()
            .bytes()
            .all(|c| c.is_ascii_digit()));
        // Field rules that do not parse: the site's apply.
        let r = generate_for_page(&policy(24), "https://163.com/", &field("nonsense")).unwrap();
        assert_eq!(r.followed, Some(RuleSource::Site));
        assert_eq!(
            followed_for_page(
                &policy(24),
                "https://163.com/",
                &FieldConstraints::default()
            )
            .unwrap(),
            Some(RuleSource::Site)
        );
    }

    #[test]
    fn avoid_ambiguous_applies_when_it_leaves_every_class() {
        let p = GeneratorOptions {
            avoid_ambiguous: true,
            length: 64,
            ..Default::default()
        };
        many(&p, &field("required: lower, upper, digit;"), |pw| {
            assert!(!pw.bytes().any(generator::is_ambiguous));
        });
        // [0O] is all ambiguous: kept rather than emptied.
        many(&p, &field("allowed: lower; required: [0O];"), |pw| {
            assert!(pw.bytes().any(|c| c == b'0' || c == b'O'));
        });
    }
}
