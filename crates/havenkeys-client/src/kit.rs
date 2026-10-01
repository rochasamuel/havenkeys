//! The Emergency Kit code (`havenkeys://kit/v2?account=…&email=…&key=…&server=…`),
//! scanned or pasted on a new device. Everything is checked here; the Secret
//! Key goes straight into a `SecretString`.

use crate::error::{ClientError, ClientResult};
use havenkeys_core::account::NormalizedEmail;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::SecretString;
use uuid::Uuid;

const MAX_KIT_LEN: usize = 2048;

pub struct KitFields {
    pub account_id: Uuid,
    pub email: String,
    pub server_url: String,
    pub secret_key: SecretString,
}

impl std::fmt::Debug for KitFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KitFields(<redacted>)")
    }
}

pub fn parse_kit(text: &str) -> ClientResult<KitFields> {
    let text = text.trim();
    if text.len() > MAX_KIT_LEN {
        return Err(ClientError::invalid_kit());
    }
    let url = url::Url::parse(text).map_err(|_| ClientError::invalid_kit())?;
    if url.scheme() != "havenkeys" || url.host_str() != Some("kit") || url.path() != "/v2" {
        return Err(ClientError::invalid_kit());
    }
    let (mut account, mut email, mut key, mut server) = (None, None, None, None);
    for (name, value) in url.query_pairs() {
        let slot = match name.as_ref() {
            "account" => &mut account,
            "email" => &mut email,
            "key" => &mut key,
            "server" => &mut server,
            _ => return Err(ClientError::invalid_kit()),
        };
        // Each field exactly once: a second value is a code someone edited.
        if slot.replace(value.into_owned()).is_some() {
            return Err(ClientError::invalid_kit());
        }
    }
    let (Some(account), Some(email), Some(key), Some(server)) = (account, email, key, server)
    else {
        return Err(ClientError::invalid_kit());
    };
    let secret_key = SecretString::new(key);
    SecretKey::parse(secret_key.expose()).map_err(|_| ClientError::invalid_kit())?;
    NormalizedEmail::parse(&email).map_err(|_| ClientError::invalid_kit())?;
    Ok(KitFields {
        account_id: account.parse().map_err(|_| ClientError::invalid_kit())?,
        email,
        server_url: server,
        secret_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use havenkeys_core::crypto::secret_key::SecretKey;

    fn key() -> String {
        SecretKey::generate().unwrap().to_text().expose().to_owned()
    }

    /// Built as the desktop's `emergency_kit.rs` builds it (it leaves
    /// `-._~` unescaped; this escapes them too, which decodes the same).
    fn kit(email: &str, server: &str, key: &str) -> String {
        use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
        format!(
            "havenkeys://kit/v2?account={}&email={}&key={}&server={}",
            Uuid::from_u128(5),
            utf8_percent_encode(email, NON_ALPHANUMERIC),
            key,
            utf8_percent_encode(server, NON_ALPHANUMERIC),
        )
    }

    #[test]
    fn a_desktop_kit_parses() {
        let k = key();
        let f = parse_kit(&kit(
            "first+tag@example.com",
            "https://vault.example.com:8443",
            &k,
        ))
        .unwrap();
        assert_eq!(f.account_id, Uuid::from_u128(5));
        assert_eq!(f.email, "first+tag@example.com");
        assert_eq!(f.server_url, "https://vault.example.com:8443");
        assert_eq!(f.secret_key.expose(), k);
    }

    #[test]
    fn anything_else_is_refused() {
        let k = key();
        let good = kit("a@example.com", "https://v.example.com", &k);
        for bad in [
            good.replace("havenkeys://", "https://"),
            good.replace("/v2?", "/v1?"),
            good.replace("kit/", "kat/"),
            good.replace(&format!("&key={k}"), ""),
            format!("{good}&key={k}"),
            good.replace(&k, "not-a-key"),
            good.replace("account=", "account=nope"),
            format!("{good}{}", "x".repeat(2048)),
            String::new(),
        ] {
            assert_eq!(parse_kit(&bad).unwrap_err().code, "invalid_kit", "{bad}");
        }
    }

    #[test]
    fn a_kit_prints_no_secret_key() {
        let f = parse_kit(&kit("a@example.com", "https://v.example.com", &key())).unwrap();
        assert!(!format!("{f:?}").contains(f.secret_key.expose()));
    }
}
