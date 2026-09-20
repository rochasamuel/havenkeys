//! Deterministic fuzzing of the core's untrusted-input parsers
//! (CLAUDE.md §47): the encrypted blob format, TOTP input, website rules and
//! page URLs, domain matching, and item input from the UI.
//!
//! Seeded xorshift, so failures reproduce, and fast enough for every
//! `cargo test`. Each target mixes random bytes with mutations of valid
//! inputs, and checks invariants, not just "no panic".

mod common;

use common::*;
use havenkeys_core::crypto::blob::{self, BlobContext, Purpose};
use havenkeys_core::crypto::keys::Key256;
use havenkeys_core::model::{normalize_url, ItemInput, MatchType, UrlRule};
use havenkeys_core::origin::{match_rule, MatchStrength, PageUrl};
use havenkeys_core::totp::{generate, parse_totp_input};
use uuid::Uuid;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
    fn bytes(&mut self, max: usize) -> Vec<u8> {
        (0..self.below(max + 1))
            .map(|_| self.next() as u8)
            .collect()
    }
}

/// Flip, insert, delete, duplicate or truncate at random positions.
fn mutate(rng: &mut Rng, input: &[u8]) -> Vec<u8> {
    let mut v = input.to_vec();
    for _ in 0..=rng.below(4) {
        let pos = if v.is_empty() { 0 } else { rng.below(v.len()) };
        match rng.below(5) {
            0 if !v.is_empty() => v[pos] ^= 1 << rng.below(8),
            1 => v.insert(pos, rng.next() as u8),
            2 if !v.is_empty() => {
                v.remove(pos);
            }
            3 if !v.is_empty() => {
                let end = (pos + rng.below(8)).min(v.len());
                let chunk = v[pos..end].to_vec();
                v.splice(pos..pos, chunk);
            }
            _ => v.truncate(pos),
        }
    }
    v
}

fn mutate_str(rng: &mut Rng, s: &str) -> String {
    String::from_utf8_lossy(&mutate(rng, s.as_bytes())).into_owned()
}

// ------------------------------------------------------------------ blobs

/// A4 at scale: no mutation of a sealed blob ever decrypts, and garbage
/// never panics the parser.
#[test]
fn fuzz_blob_parse_and_open() {
    let mut rng = Rng::new(0xB10B);
    let key = Key256::from_bytes([7; 32]);
    let ctx = BlobContext::item(Purpose::ItemDetails, Uuid::from_u128(1), Uuid::from_u128(2));
    let sealed: Vec<Vec<u8>> = [
        &b""[..],
        b"x",
        b"{\"type\":\"login\",\"password\":\"hunter2\"}",
        &[0u8; 300],
    ]
    .iter()
    .map(|p| blob::seal(&key, &ctx, p).unwrap())
    .collect();
    for s in &sealed {
        assert!(blob::open(&key, &ctx, s).is_ok());
    }
    for i in 0..20_000 {
        let input = if i % 3 == 0 {
            rng.bytes(96)
        } else {
            let original = rng.pick(&sealed).clone();
            let m = mutate(&mut rng, &original);
            if m == original {
                continue;
            }
            m
        };
        let _ = blob::parse(&input);
        assert!(
            blob::open(&key, &ctx, &input).is_err(),
            "mutated blob opened (case {i})"
        );
    }
}

// ------------------------------------------------------------------ TOTP

#[test]
fn fuzz_totp_input() {
    let mut rng = Rng::new(0x7071);
    let seeds = [
        "otpauth://totp/GitHub:octo?secret=JBSWY3DPEHPK3PXP&issuer=GitHub",
        "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=SHA512&digits=8&period=60",
        "JBSWY3DPEHPK3PXP",
        "jbsw y3dp ehpk 3pxp",
        "otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=1",
    ];
    for i in 0..20_000 {
        let input = if i % 4 == 0 {
            String::from_utf8_lossy(&rng.bytes(80)).into_owned()
        } else {
            let seed = *rng.pick(&seeds);
            mutate_str(&mut rng, seed)
        };
        // Anything accepted must be a usable, valid configuration.
        if let Ok(cfg) = parse_totp_input(&input) {
            cfg.validate().unwrap();
            let code = generate(&cfg, 1_700_000_000).unwrap();
            assert!(code.code.expose().bytes().all(|b| b.is_ascii_digit()));
            assert_eq!(code.code.expose().len() as u32, cfg.digits);
        }
    }
}

// ------------------------------------------------------------------ URLs and matching

const URL_SEEDS: &[&str] = &[
    "https://github.com/login",
    "https://user:pw@github.com:443/a/b?c=d#e",
    "http://192.168.1.1:8080/",
    "https://[::1]/",
    "https://xn--gthub-n4a.com/",
    "https://gist.github.com./x",
    "https://bank.co.uk/",
    "https://alice.github.io/",
    "file:///etc/passwd",
    "javascript:alert(1)",
    "https://github.com%2eevil.com/",
    "https://github.com\\@evil.com/",
];

/// Website rules and page URLs from anywhere (UI, imports, the browser)
/// never panic the parser or the matcher.
#[test]
fn fuzz_url_parsing_and_matching() {
    let mut rng = Rng::new(0x0421);
    for i in 0..20_000 {
        let a = if i % 5 == 0 {
            String::from_utf8_lossy(&rng.bytes(60)).into_owned()
        } else {
            let seed = *rng.pick(URL_SEEDS);
            mutate_str(&mut rng, seed)
        };
        let seed = *rng.pick(URL_SEEDS);
        let b = mutate_str(&mut rng, seed);
        // Anything normalize_url accepts is an http(s) URL without credentials.
        if let Ok(n) = normalize_url(&a) {
            let u = url::Url::parse(&n).unwrap();
            assert!(matches!(u.scheme(), "http" | "https"), "{n}");
            assert!(u.username().is_empty() && u.password().is_none(), "{n}");
        }
        if let Some(page) = PageUrl::parse(&b) {
            for match_type in [MatchType::Exact, MatchType::Origin, MatchType::Domain] {
                let _ = match_rule(
                    &UrlRule {
                        url: a.clone(),
                        match_type,
                    },
                    &page,
                );
            }
        }
    }
}

/// The phishing properties of CLAUDE.md §23–24, checked on generated
/// look-alikes of many real sites instead of a hand-written list: none of
/// them may ever match, while real subdomains must.
#[test]
fn fuzz_lookalike_hosts_never_match() {
    // (host of the saved rule, its registrable domain)
    let sites = [
        ("github.com", "github.com"),
        ("google.com", "google.com"),
        ("bank.co.uk", "bank.co.uk"),
        ("itau.com.br", "itau.com.br"),
        ("example.org", "example.org"),
        ("paypal.com", "paypal.com"),
        ("login.microsoftonline.com", "microsoftonline.com"),
    ];
    let evil = [
        "evil.com",
        "attacker.net",
        "co.uk",
        "com.br",
        "github.io",
        "example",
    ];
    let mut rng = Rng::new(0xFEED);
    for _ in 0..5_000 {
        let (site, registrable) = *rng.pick(&sites);
        let bad = *rng.pick(&evil);
        let label = format!("x{}", rng.below(1000));
        let lookalikes = [
            format!("{site}.{bad}"), // github.com.evil.com
            // Glued onto the registrable domain: glued onto a subdomain
            // it would be a real subdomain of the site.
            format!("{bad}{registrable}"),   // evil.comgithub.com
            format!("{label}{registrable}"), // x12github.com
            format!("{}-login.{bad}", site.replace('.', "-")),
            format!("{site}-{label}.{bad}"),
            format!("{bad}/{site}"), // path, not host
            format!("{bad}?{site}"),
            format!("{site}@{bad}"), // userinfo trick
            format!("{bad}#{site}"),
        ];
        for rule_type in [MatchType::Exact, MatchType::Origin, MatchType::Domain] {
            let rule = UrlRule {
                url: format!("https://{site}/"),
                match_type: rule_type,
            };
            for host in &lookalikes {
                let page = format!("https://{host}/");
                if let Some(p) = PageUrl::parse(&page) {
                    assert_eq!(match_rule(&rule, &p), None, "{page} matched {site}");
                }
                // A downgrade of the real site never matches an https rule.
                let downgrade = PageUrl::parse(&format!("http://{site}/")).unwrap();
                assert_eq!(match_rule(&rule, &downgrade), None);
            }
        }
        // Real subdomains match a whole-site rule, and only as "same site".
        let rule = UrlRule {
            url: format!("https://{site}/"),
            match_type: MatchType::Domain,
        };
        let sub = PageUrl::parse(&format!("https://{label}.{site}/")).unwrap();
        assert_eq!(
            match_rule(&rule, &sub),
            Some(MatchStrength::SameSite),
            "{label}.{site}"
        );
        let origin_rule = UrlRule {
            match_type: MatchType::Origin,
            ..rule
        };
        assert_eq!(match_rule(&origin_rule, &sub), None);
    }
}

// ------------------------------------------------------------------ item input

/// Item JSON from the UI (or a compromised renderer) never panics the core,
/// and whatever is stored reads back.
#[test]
fn fuzz_item_input() {
    let mut rng = Rng::new(0x17E4);
    let seeds = [
        r#"{"itemType":"login","title":"GitHub","username":"octo","urls":[{"url":"github.com","matchType":"domain"}],"password":{"op":"set","value":"pw"},"totp":{"op":"set","value":"JBSWY3DPEHPK3PXP"}}"#,
        r#"{"itemType":"secure_note","title":"Note","content":{"op":"set","value":"body"}}"#,
        r#"{"itemType":"login","title":"x","urls":[{"url":"http://[::1]:8080/p","matchType":"exact"}],"notes":{"op":"clear"}}"#,
    ];
    let (mut v, _sk) = activated_vault();
    let mut stored = 0;
    for i in 0..3_000 {
        let seed = *rng.pick(&seeds);
        let json = mutate(&mut rng, seed.as_bytes());
        let Ok(input) = serde_json::from_slice::<ItemInput>(&json) else {
            continue;
        };
        if let Ok(item) = v.create_item(input, NOW + i) {
            stored += 1;
            assert_eq!(v.get_item(&item.id).unwrap().title, item.title);
        }
    }
    assert!(stored > 0, "the fuzzer should also produce valid items");
    assert_eq!(v.list_items().unwrap().len(), stored);
}

// ------------------------------------------------------------ server input

/// Random bytes in the overview and details slots must never panic, never
/// produce an item, and must be counted as skipped. Everything a sync
/// server sends is untrusted input reaching `check_item_bytes`, which
/// decrypts under this vault's data key.
#[test]
fn fuzz_remote_changes() {
    let (mut vault, _sk) = activated_vault();
    let mut rng = Rng::new(0x5EED_5EED);
    for _ in 0..2000 {
        let change = havenkeys_core::sync::RemoteChange {
            item_id: Uuid::from_u128(u128::from(rng.next())),
            revision: 1,
            overview: Some(rng.bytes(512)),
            details: Some(rng.bytes(512)),
            deleted: false,
        };
        let report = vault.apply_remote_changes(1, vec![change], NOW).unwrap();
        assert_eq!(report.added, 0);
        assert_eq!(report.updated, 0);
        assert_eq!(report.skipped_items, 1);
    }
    assert_eq!(vault.list_items().unwrap().len(), 0);
}

/// Random bytes as a server header must be rejected, never adopted.
#[test]
fn fuzz_account_headers() {
    let (mut vault, _sk) = activated_vault();
    let mut rng = Rng::new(0x1234_5678);
    for _ in 0..2000 {
        let bytes = rng.bytes(1024);
        // Either an error or a refusal; never an adoption, never a panic.
        assert!(matches!(
            vault.adopt_account_header(&bytes),
            Err(_) | Ok(false)
        ));
    }
}
