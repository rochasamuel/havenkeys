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
        })
    }
}
