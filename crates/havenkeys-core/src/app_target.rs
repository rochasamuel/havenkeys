//! Who is asking Android to fill, and what kind of target that is (spec
//! 2026-10-01-android-app §7.2).
//!
//! A browser on Google's privileged list (`data/android-browsers.json`, the
//! list Credential Manager uses; release signatures only) is trusted to
//! report the page's domain and scheme. Every other caller — WebViews inside
//! apps included, since an app controls its WebView — is an app, identified
//! only by its package name and signing certificates, which Android reports.

use crate::error::{Error, Result};
use crate::origin::PageUrl;
use serde::Deserialize;
use std::sync::OnceLock;

pub const CERT_LEN: usize = 32;
const MAX_PACKAGE_LEN: usize = 255;
const MAX_SIGNERS: usize = 16;
const MAX_DOMAIN_LEN: usize = 260;
const BROWSERS_JSON: &str = include_str!("../data/android-browsers.json");

#[derive(Clone, PartialEq, Eq)]
pub struct AppIdentity {
    package: String,
    certs: Vec<[u8; CERT_LEN]>,
}

impl std::fmt::Debug for AppIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppIdentity")
            .field("package", &self.package)
            .finish_non_exhaustive()
    }
}

impl AppIdentity {
    pub fn new(package: &str, certs: &[Vec<u8>]) -> Result<Self> {
        if !valid_package(package) || certs.is_empty() || certs.len() > MAX_SIGNERS {
            return Err(Error::Denied);
        }
        let certs = certs
            .iter()
            .map(|c| <[u8; CERT_LEN]>::try_from(c.as_slice()).map_err(|_| Error::Denied))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            package: package.to_owned(),
            certs,
        })
    }

    pub fn package(&self) -> &str {
        &self.package
    }

    pub fn certs(&self) -> &[[u8; CERT_LEN]] {
        &self.certs
    }

    pub fn signed_by(&self, cert: &[u8; CERT_LEN]) -> bool {
        self.certs.iter().any(|c| c == cert)
    }
}

/// Android package name grammar: two or more dot-separated segments, each
/// starting with a letter, then letters, digits or underscores.
pub(crate) fn valid_package(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= MAX_PACKAGE_LEN
        && p.split('.').count() >= 2
        && p.split('.').all(|seg| {
            let mut chars = seg.chars();
            matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

pub enum FillTarget {
    /// A privileged browser showing this page.
    Browser {
        page_url: String,
    },
    App(AppIdentity),
}

pub fn classify(
    own_package: &str,
    package: &str,
    certs: &[Vec<u8>],
    web_domain: Option<&str>,
    web_scheme: Option<&str>,
) -> Result<FillTarget> {
    let app = AppIdentity::new(package, certs)?;
    if app.package == own_package {
        return Err(Error::Denied);
    }
    if !app
        .certs
        .iter()
        .any(|c| is_privileged_browser(&app.package, c))
    {
        return Ok(FillTarget::App(app));
    }
    // A browser's own screens (address bar, settings) carry no page.
    let domain = web_domain.ok_or(Error::Denied)?;
    // Without a scheme an http page could pass for https; refuse rather
    // than guess.
    let scheme = match web_scheme {
        Some("https") => "https",
        Some("http") => "http",
        _ => return Err(Error::Denied),
    };
    let page_url = page_url_for(scheme, domain).ok_or(Error::Denied)?;
    Ok(FillTarget::Browser { page_url })
}

/// The domain is the browser's report of the page host (sometimes with a
/// port). Anything that would change how the URL parses is refused rather
/// than trimmed.
fn page_url_for(scheme: &str, domain: &str) -> Option<String> {
    if domain.is_empty()
        || domain.len() > MAX_DOMAIN_LEN
        || domain.contains(['/', '\\', '@', '?', '#', ' '])
    {
        return None;
    }
    let raw = format!("{scheme}://{domain}/");
    PageUrl::parse(&raw)?;
    Some(raw)
}

pub fn parse_fingerprint(text: &str) -> Option<[u8; CERT_LEN]> {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() != CERT_LEN {
        return None;
    }
    let mut out = [0u8; CERT_LEN];
    for (byte, part) in out.iter_mut().zip(parts) {
        if part.len() != 2 {
            return None;
        }
        *byte = u8::from_str_radix(part, 16).ok()?;
    }
    Some(out)
}

#[derive(Deserialize)]
struct BrowserList {
    apps: Vec<ListedApp>,
}

#[derive(Deserialize)]
struct ListedApp {
    #[serde(rename = "type")]
    kind: String,
    info: ListedInfo,
}

#[derive(Deserialize)]
struct ListedInfo {
    package_name: String,
    signatures: Vec<ListedSignature>,
}

#[derive(Deserialize)]
struct ListedSignature {
    build: String,
    cert_fingerprint_sha256: String,
}

/// (package, release certificate) pairs. A list that does not parse is
/// empty — every caller is then an app, which is the safe side — and the
/// tests catch it before release.
fn browsers() -> &'static [(String, [u8; CERT_LEN])] {
    static LIST: OnceLock<Vec<(String, [u8; CERT_LEN])>> = OnceLock::new();
    LIST.get_or_init(|| {
        let Ok(list) = serde_json::from_str::<BrowserList>(BROWSERS_JSON) else {
            return Vec::new();
        };
        list.apps
            .into_iter()
            .filter(|a| a.kind == "android")
            .flat_map(|a| {
                let package = a.info.package_name;
                a.info
                    .signatures
                    .into_iter()
                    .filter(|s| s.build == "release")
                    .filter_map(|s| parse_fingerprint(&s.cert_fingerprint_sha256))
                    .map(move |c| (package.clone(), c))
                    .collect::<Vec<_>>()
            })
            .collect()
    })
}

pub fn is_privileged_browser(package: &str, cert: &[u8; CERT_LEN]) -> bool {
    browsers().iter().any(|(p, c)| p == package && c == cert)
}

/// The vendored list, for Android's `CallingAppInfo.getOrigin`. Rust checks
/// the caller against the same list again; Android's check is not relied on.
pub fn privileged_browsers_json() -> &'static str {
    BROWSERS_JSON
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "net.havenkeys.android";
    const CHROME_RELEASE: &str =
        "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";
    const CHROME_USERDEBUG: &str =
        "19:75:B2:F1:71:77:BC:89:A5:DF:F3:1F:9E:64:A6:CA:E2:81:A5:3D:C1:D1:D5:9B:1D:14:7F:E1:C8:2A:FA:00";
    const FIREFOX: &str =
        "A7:8B:62:A5:16:5B:44:94:B2:FE:AD:9E:76:A2:80:D2:2D:93:7F:EE:62:51:AE:CE:59:94:46:B2:EA:31:9B:04";

    fn cert(fp: &str) -> Vec<u8> {
        parse_fingerprint(fp).unwrap().to_vec()
    }

    #[test]
    fn the_vendored_list_has_the_major_browsers() {
        assert!(is_privileged_browser(
            "com.android.chrome",
            &parse_fingerprint(CHROME_RELEASE).unwrap()
        ));
        assert!(is_privileged_browser(
            "org.mozilla.firefox",
            &parse_fingerprint(FIREFOX).unwrap()
        ));
        assert!(browsers().len() > 20);
    }

    #[test]
    fn only_release_signatures_count() {
        assert!(!is_privileged_browser(
            "com.android.chrome",
            &parse_fingerprint(CHROME_USERDEBUG).unwrap()
        ));
    }

    #[test]
    fn chrome_reporting_an_https_page_is_a_browser_target() {
        let t = classify(
            OWN,
            "com.android.chrome",
            &[cert(CHROME_RELEASE)],
            Some("github.com"),
            Some("https"),
        )
        .unwrap();
        assert!(matches!(t, FillTarget::Browser { page_url } if page_url == "https://github.com/"));
    }

    #[test]
    fn an_http_page_stays_http() {
        let t = classify(
            OWN,
            "com.android.chrome",
            &[cert(CHROME_RELEASE)],
            Some("example.com"),
            Some("http"),
        )
        .unwrap();
        assert!(matches!(t, FillTarget::Browser { page_url } if page_url == "http://example.com/"));
    }

    #[test]
    fn a_browser_without_a_scheme_gets_nothing() {
        let r = classify(
            OWN,
            "com.android.chrome",
            &[cert(CHROME_RELEASE)],
            Some("github.com"),
            None,
        );
        assert_eq!(r.err(), Some(Error::Denied));
        let r = classify(
            OWN,
            "com.android.chrome",
            &[cert(CHROME_RELEASE)],
            Some("github.com"),
            Some("ftp"),
        );
        assert_eq!(r.err(), Some(Error::Denied));
    }

    #[test]
    fn a_browser_screen_without_a_page_gets_nothing() {
        let r = classify(
            OWN,
            "com.android.chrome",
            &[cert(CHROME_RELEASE)],
            None,
            Some("https"),
        );
        assert_eq!(r.err(), Some(Error::Denied));
    }

    #[test]
    fn a_domain_that_would_change_the_url_is_refused() {
        for domain in [
            "github.com/evil",
            "user@github.com",
            "github.com?x",
            "github.com#x",
            "",
            "git hub.com",
            "github.com\\x",
        ] {
            let r = classify(
                OWN,
                "com.android.chrome",
                &[cert(CHROME_RELEASE)],
                Some(domain),
                Some("https"),
            );
            assert_eq!(r.err(), Some(Error::Denied), "{domain}");
        }
    }

    #[test]
    fn chrome_signed_by_someone_else_is_an_app() {
        let t = classify(
            OWN,
            "com.android.chrome",
            &[vec![7; 32]],
            Some("github.com"),
            Some("https"),
        )
        .unwrap();
        assert!(matches!(t, FillTarget::App(a) if a.package() == "com.android.chrome"));
    }

    #[test]
    fn an_app_claiming_a_web_domain_is_still_an_app() {
        let t = classify(
            OWN,
            "com.example.webviewapp",
            &[vec![1; 32]],
            Some("github.com"),
            Some("https"),
        )
        .unwrap();
        assert!(matches!(t, FillTarget::App(_)));
    }

    #[test]
    fn havenkeys_never_fills_itself() {
        assert_eq!(
            classify(OWN, OWN, &[vec![1; 32]], None, None).err(),
            Some(Error::Denied)
        );
    }

    #[test]
    fn bad_identities_are_refused() {
        for (pkg, certs) in [
            ("", vec![vec![1u8; 32]]),
            ("nodots", vec![vec![1; 32]]),
            ("com..x", vec![vec![1; 32]]),
            ("com.1x", vec![vec![1; 32]]),
            ("com.x", vec![]),
            ("com.x", vec![vec![1; 31]]),
        ] {
            assert!(AppIdentity::new(pkg, &certs).is_err(), "{pkg}");
        }
        assert!(AppIdentity::new(&format!("com.{}", "a".repeat(300)), &[vec![1; 32]]).is_err());
        assert!(AppIdentity::new("com.x", &vec![vec![1; 32]; 17]).is_err());
    }

    #[test]
    fn fingerprints_parse_strictly() {
        assert!(parse_fingerprint(CHROME_RELEASE).is_some());
        assert!(parse_fingerprint(&CHROME_RELEASE.to_lowercase()).is_some());
        assert!(parse_fingerprint(&CHROME_RELEASE.replace(':', "")).is_none());
        assert!(parse_fingerprint(&CHROME_RELEASE[3..]).is_none());
        assert!(parse_fingerprint("ZZ").is_none());
    }

    #[test]
    fn the_allowlist_handed_to_android_is_the_vendored_list() {
        let json: serde_json::Value = serde_json::from_str(privileged_browsers_json()).unwrap();
        assert!(json["apps"].as_array().unwrap().len() > 20);
    }
}
