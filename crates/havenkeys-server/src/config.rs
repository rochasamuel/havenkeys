//! Configuration from the environment.

use data_encoding::BASE64;
use zeroize::Zeroizing;

pub struct Config {
    pub database_url: Zeroizing<String>,
    pub server_secret: [u8; 32],
    pub port: u16,
    /// Exactly one browser origin, or none at all. There is no wildcard.
    pub cors_origin: Option<String>,
    /// Whether to believe the left-most `X-Forwarded-For` entry. Off unless
    /// the service is known to sit behind a proxy that sets it, because a
    /// client-supplied header would otherwise defeat per-IP rate limiting.
    pub trust_forwarded_for: bool,
    /// A MaxMind-format city database for the pairing confirmation's
    /// location. Optional; without it the phone sees the IP only.
    pub geoip_database: Option<std::path::PathBuf>,
}

impl Config {
    /// Reads the environment. Fails loudly rather than inventing a default
    /// secret: a predictable `SERVER_SECRET` would make the `auth/params`
    /// salts guessable and bring account enumeration back.
    pub fn from_env() -> Result<Self, String> {
        let database_url = Zeroizing::new(
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_string())?,
        );
        let raw = Zeroizing::new(
            std::env::var("SERVER_SECRET")
                .map_err(|_| "SERVER_SECRET is not set (32 random bytes, base64)".to_string())?,
        );
        let decoded = Zeroizing::new(
            BASE64
                .decode(raw.trim().as_bytes())
                .map_err(|_| "SERVER_SECRET is not valid base64".to_string())?,
        );
        let server_secret: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| "SERVER_SECRET must decode to exactly 32 bytes".to_string())?;
        let port = match std::env::var("PORT") {
            Ok(p) => p
                .trim()
                .parse()
                .map_err(|_| "PORT is not a number".to_string())?,
            Err(_) => 8080,
        };
        Ok(Self {
            database_url,
            server_secret,
            port,
            cors_origin: std::env::var("HAVENKEYS_CORS_ORIGIN")
                .ok()
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty()),
            trust_forwarded_for: matches!(
                std::env::var("HAVENKEYS_TRUST_FORWARDED_FOR").as_deref(),
                Ok("1") | Ok("true")
            ),
            geoip_database: std::env::var("HAVENKEYS_GEOIP_DATABASE")
                .ok()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .map(Into::into),
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
    use super::check_public_url;

    #[test]
    fn a_plain_http_server_url_is_refused() {
        assert!(check_public_url("http://vault.example.com").is_err());
        assert_eq!(
            check_public_url("https://vault.example.com/").unwrap(),
            "https://vault.example.com"
        );
        assert!(check_public_url("http://localhost:8080").is_ok());
    }
}
