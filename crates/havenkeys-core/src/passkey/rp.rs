//! Which relying-party IDs a page may use: WebAuthn's "is a registrable
//! domain suffix of or is equal to" rule, with the Public Suffix List from
//! `origin.rs`. The page URL comes from the browser (via the background
//! worker), never from page content; the `rp_id` comes from the page and is
//! untrusted.

use crate::error::{Error, Result};
use crate::origin::{host_key, registrable_domain_of, PageUrl};
use url::{Host, Url};

pub const MAX_RP_ID_BYTES: usize = 253;

/// A page's right to use one relying-party ID, from [`authorize_rp`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RpContext {
    /// Normalized: lowercase, punycode.
    pub rp_id: String,
    /// The frame's origin, as it goes into `clientDataJSON`.
    pub origin: String,
    pub cross_origin: bool,
    /// The top-level origin, only when `cross_origin`.
    pub top_origin: Option<String>,
}

/// WebAuthn requires a secure context: https, or http on localhost.
fn secure_context(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => url.host_str() == Some("localhost"),
        _ => false,
    }
}

/// Lowercase, punycode, no trailing dot, no port. `None` for anything that is
/// not a plain domain or IP address.
fn normalize_rp_id(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > MAX_RP_ID_BYTES || raw.ends_with('.') {
        return None;
    }
    match Host::parse(raw).ok()? {
        Host::Domain(d) => Some(d),
        Host::Ipv4(ip) => Some(ip.to_string()),
        Host::Ipv6(ip) => Some(format!("[{ip}]")),
    }
}

/// Same registrable domain, or the same host when either has none.
fn same_site(a: &str, b: &str) -> bool {
    match (registrable_domain_of(a), registrable_domain_of(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// May the page at `page_url` (framed in `top_url`, if any) use `rp_id`?
///
/// * the page is a secure context;
/// * `rp_id` equals the page's host, or is a parent domain of it that is
///   still inside the host's registrable domain (never a public suffix,
///   never for an IP host);
/// * a frame must be same-site with the top-level page.
pub fn authorize_rp(rp_id: &str, page_url: &str, top_url: Option<&str>) -> Result<RpContext> {
    let page = PageUrl::parse(page_url).ok_or(Error::Denied)?;
    let frame = page.url();
    if !secure_context(frame) {
        return Err(Error::Denied);
    }
    let host = host_key(frame).ok_or(Error::Denied)?;
    let rp_id = normalize_rp_id(rp_id).ok_or(Error::Denied)?;
    if rp_id != host {
        let is_domain = matches!(frame.host(), Some(Host::Domain(_)));
        let site = registrable_domain_of(&host).ok_or(Error::Denied)?;
        let parent = host
            .strip_suffix(rp_id.as_str())
            .is_some_and(|rest| rest.ends_with('.'));
        let within_site = rp_id == site || rp_id.ends_with(&format!(".{site}"));
        if !(is_domain && parent && within_site) {
            return Err(Error::Denied);
        }
    }
    let origin = frame.origin().ascii_serialization();
    let (cross_origin, top_origin) = match top_url {
        None => (false, None),
        Some(t) => {
            let top_page = PageUrl::parse(t).ok_or(Error::Denied)?;
            let top = top_page.url();
            let top_host = host_key(top).ok_or(Error::Denied)?;
            if !same_site(&host, &top_host) {
                return Err(Error::Denied);
            }
            let top_origin = top.origin().ascii_serialization();
            if top_origin == origin {
                (false, None)
            } else {
                (true, Some(top_origin))
            }
        }
    };
    Ok(RpContext {
        rp_id,
        origin,
        cross_origin,
        top_origin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(rp: &str, page: &str) -> RpContext {
        authorize_rp(rp, page, None).unwrap_or_else(|e| panic!("{rp} on {page}: {e}"))
    }

    fn denied(rp: &str, page: &str, top: Option<&str>) {
        assert_eq!(
            authorize_rp(rp, page, top).err(),
            Some(Error::Denied),
            "{rp} on {page} (top {top:?})"
        );
    }

    #[test]
    fn allowed() {
        let c = ok("github.com", "https://github.com/login");
        assert_eq!(c.rp_id, "github.com");
        assert_eq!(c.origin, "https://github.com");
        assert!(!c.cross_origin);
        assert_eq!(
            ok("github.com", "https://accounts.github.com/").rp_id,
            "github.com"
        );
        assert_eq!(
            ok("accounts.github.com", "https://accounts.github.com/").rp_id,
            "accounts.github.com"
        );
        assert_eq!(ok("GitHub.COM", "https://github.com/").rp_id, "github.com");
        assert_eq!(
            ok("bücher.de", "https://login.xn--bcher-kva.de/").rp_id,
            "xn--bcher-kva.de"
        );
        assert_eq!(
            ok("localhost", "http://localhost:8080/").origin,
            "http://localhost:8080"
        );
        assert_eq!(ok("127.0.0.1", "https://127.0.0.1/").rp_id, "127.0.0.1");
    }

    #[test]
    fn phishing_and_public_suffixes_are_denied() {
        for (rp, page) in [
            ("github.com", "https://github.com.evil.com/"),
            ("github.com", "https://evilgithub.com/"),
            ("github.com", "https://github-login.example.com/"),
            ("github.com", "https://evil.com/github.com"),
            ("com", "https://github.com/"),
            ("github.io", "https://user.github.io/"),
            ("user.github.io", "https://other.github.io/"),
            ("accounts.github.com", "https://github.com/"),
            ("hub.com", "https://github.com/"),
            ("github.com", "http://github.com/"),
            ("github.com", "file:///etc/passwd"),
            ("github.com", "not a url"),
            ("127.0.0.1", "https://127.0.0.2/"),
            ("0.1", "https://127.0.0.1/"),
            ("example.com", "https://127.0.0.1/"),
            ("internal", "https://app.internal/"),
            ("", "https://github.com/"),
            ("github.com.", "https://github.com/"),
            ("github.com:443", "https://github.com/"),
            ("git hub.com", "https://github.com/"),
        ] {
            denied(rp, page, None);
        }
        denied(
            &"a".repeat(MAX_RP_ID_BYTES + 1),
            "https://github.com/",
            None,
        );
    }

    #[test]
    fn frames_must_be_same_site_with_the_top_page() {
        denied(
            "github.com",
            "https://github.com/",
            Some("https://evil.com/"),
        );
        denied("github.com", "https://github.com/", Some("not a url"));
        let same = authorize_rp(
            "github.com",
            "https://github.com/a",
            Some("https://github.com/b"),
        )
        .unwrap();
        assert!(!same.cross_origin);
        assert_eq!(same.top_origin, None);
        let sub = authorize_rp(
            "github.com",
            "https://gist.github.com/",
            Some("https://github.com/"),
        )
        .unwrap();
        assert!(sub.cross_origin);
        assert_eq!(sub.top_origin.as_deref(), Some("https://github.com"));
    }
}
