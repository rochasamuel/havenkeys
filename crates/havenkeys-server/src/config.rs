//! Configuration from the environment.

use data_encoding::BASE64;
use zeroize::Zeroizing;

pub struct SmtpConfig {
    pub url: Zeroizing<String>,
    pub from: String,
}

pub struct Config {
    pub database_url: Zeroizing<String>,
    pub server_secret: [u8; 32],
    pub port: u16,
    pub cors_origin: Option<String>,
    pub trust_forwarded_for: bool,
    pub geoip_database: Option<std::path::PathBuf>,
    /// `HAVENKEYS_SIGNUP=open`. Off by default: a self-hosted server keeps
    /// admin invites only (spec 2026-10-07 §4.1).
    pub signup_open: bool,
    /// `HAVENKEYS_PUBLIC_URL`, carried in the invites signup issues.
    pub public_url: Option<String>,
    /// `SMTP_URL` and `SMTP_FROM`. Required when signup is open; useful
    /// without it for the trial notices of `admin new-account --trial`.
    pub smtp: Option<SmtpConfig>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let vars: Vec<(&str, String)> = [
            "DATABASE_URL",
            "SERVER_SECRET",
            "PORT",
            "HAVENKEYS_CORS_ORIGIN",
            "HAVENKEYS_TRUST_FORWARDED_FOR",
            "HAVENKEYS_GEOIP_DATABASE",
            "HAVENKEYS_SIGNUP",
            "HAVENKEYS_PUBLIC_URL",
            "SMTP_URL",
            "SMTP_FROM",
        ]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok().map(|v| (name, v)))
        .collect();
        Self::from_vars(&vars)
    }

    /// Reads a set of variables. Fails loudly rather than inventing a
    /// default secret: a predictable `SERVER_SECRET` would make the
    /// `auth/params` salts guessable and bring account enumeration back.
    pub fn from_vars(vars: &[(&str, String)]) -> Result<Self, String> {
        let get = |name: &str| -> Option<String> {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let database_url =
            Zeroizing::new(get("DATABASE_URL").ok_or("DATABASE_URL is not set".to_string())?);
        let raw = Zeroizing::new(
            get("SERVER_SECRET")
                .ok_or("SERVER_SECRET is not set (32 random bytes, base64)".to_string())?,
        );
        let decoded = Zeroizing::new(
            BASE64
                .decode(raw.as_bytes())
                .map_err(|_| "SERVER_SECRET is not valid base64".to_string())?,
        );
        let server_secret: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| "SERVER_SECRET must decode to exactly 32 bytes".to_string())?;
        let port = match get("PORT") {
            Some(p) => p.parse().map_err(|_| "PORT is not a number".to_string())?,
            None => 8080,
        };
        let signup_open = match get("HAVENKEYS_SIGNUP").as_deref() {
            None | Some("off") => false,
            Some("open") => true,
            Some(_) => return Err("HAVENKEYS_SIGNUP must be open or off".into()),
        };
        let smtp = match (get("SMTP_URL"), get("SMTP_FROM")) {
            (Some(url), Some(from)) => Some(SmtpConfig {
                url: Zeroizing::new(url),
                from,
            }),
            (None, None) => None,
            _ => return Err("SMTP_URL and SMTP_FROM must be set together".into()),
        };
        let public_url = get("HAVENKEYS_PUBLIC_URL")
            .map(|u| check_public_url(&u))
            .transpose()?;
        if signup_open && smtp.is_none() {
            return Err("HAVENKEYS_SIGNUP=open needs SMTP_URL and SMTP_FROM".into());
        }
        if signup_open && public_url.is_none() {
            return Err("HAVENKEYS_SIGNUP=open needs HAVENKEYS_PUBLIC_URL".into());
        }
        Ok(Self {
            database_url,
            server_secret,
            port,
            cors_origin: get("HAVENKEYS_CORS_ORIGIN"),
            trust_forwarded_for: matches!(
                get("HAVENKEYS_TRUST_FORWARDED_FOR").as_deref(),
                Some("1") | Some("true")
            ),
            geoip_database: get("HAVENKEYS_GEOIP_DATABASE").map(Into::into),
            signup_open,
            public_url,
            smtp,
        })
    }
}

/// A URL clients will be told to trust: an invite carries it and a client
/// sends its master-password proof there. HTTPS, or an explicit localhost
/// for development.
pub fn check_public_url(raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/').to_string();
    let local = url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1");
    if !url.starts_with("https://") && !local {
        return Err("the server URL must be https (or http on localhost)".into());
    }
    if url.len() > 512 {
        return Err("the server URL is too long".into());
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::{check_public_url, Config};

    #[test]
    fn a_plain_http_server_url_is_refused() {
        assert!(check_public_url("http://vault.example.com").is_err());
        assert_eq!(
            check_public_url("https://vault.example.com/").unwrap(),
            "https://vault.example.com"
        );
        assert!(check_public_url("http://localhost:8080").is_ok());
    }

    #[test]
    fn signup_open_needs_smtp_and_a_public_url() {
        let base = |signup: &str| {
            vec![
                ("DATABASE_URL", "postgres://x".to_string()),
                ("SERVER_SECRET", data_encoding::BASE64.encode(&[1u8; 32])),
                ("HAVENKEYS_SIGNUP", signup.to_string()),
            ]
        };
        let off = Config::from_vars(&base("off")).unwrap();
        assert!(!off.signup_open && off.smtp.is_none());
        assert!(Config::from_vars(&base("open")).is_err(), "no SMTP, no URL");
        let mut vars = base("open");
        vars.push(("SMTP_URL", "smtps://u:p@smtp.example.com:465".into()));
        vars.push(("SMTP_FROM", "HavenKeys <no-reply@havenkeys.net>".into()));
        assert!(Config::from_vars(&vars).is_err(), "no public URL");
        vars.push(("HAVENKEYS_PUBLIC_URL", "https://api.havenkeys.net/".into()));
        let open = Config::from_vars(&vars).unwrap();
        assert!(open.signup_open);
        assert_eq!(
            open.public_url.as_deref(),
            Some("https://api.havenkeys.net")
        );
        assert_eq!(
            open.smtp.as_ref().unwrap().from,
            "HavenKeys <no-reply@havenkeys.net>"
        );
        assert!(
            Config::from_vars(&base("maybe")).is_err(),
            "only open or off"
        );
        let mut smtp_only = base("off");
        smtp_only.push(("SMTP_URL", "smtps://u:p@smtp.example.com:465".into()));
        smtp_only.push(("SMTP_FROM", "no-reply@havenkeys.net".into()));
        assert!(
            Config::from_vars(&smtp_only).unwrap().smtp.is_some(),
            "trial mail without signup"
        );
    }
}
