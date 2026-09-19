//! TOTP (RFC 6238) code generation and `otpauth://` parsing.

use crate::error::{Error, Result};
use crate::secret::SecretString;
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;
use zeroize::Zeroizing;

pub const MIN_SECRET_BYTES: usize = 10; // 80 bits; common real-world minimum
pub const MAX_SECRET_BYTES: usize = 128;
pub const MIN_PERIOD: u32 = 1;
pub const MAX_PERIOD: u32 = 300;
const MAX_URI_LEN: usize = 2048;
const MAX_LABEL_LEN: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TotpAlgorithm {
    #[serde(rename = "SHA1")]
    Sha1,
    #[serde(rename = "SHA256")]
    Sha256,
    #[serde(rename = "SHA512")]
    Sha512,
}

/// Validated TOTP configuration. `secret` is normalized Base32 (upper-case,
/// no padding, no whitespace).
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TotpConfig {
    pub secret: SecretString,
    pub algorithm: TotpAlgorithm,
    pub digits: u32,
    pub period: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl fmt::Debug for TotpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TotpConfig")
            .field("algorithm", &self.algorithm)
            .field("digits", &self.digits)
            .field("period", &self.period)
            .finish_non_exhaustive()
    }
}

impl TotpConfig {
    pub fn validate(&self) -> Result<()> {
        if !(self.digits == 6 || self.digits == 8) {
            return Err(Error::InvalidInput("TOTP digits must be 6 or 8"));
        }
        if !(MIN_PERIOD..=MAX_PERIOD).contains(&self.period) {
            return Err(Error::InvalidInput("TOTP period out of range"));
        }
        decode_secret(self.secret.expose()).map(|_| ())
    }
}

/// A generated one-time code plus timing information for the UI.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: SecretString,
    pub period: u32,
    pub seconds_remaining: u32,
}

impl fmt::Debug for TotpCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TotpCode(<redacted>)")
    }
}

fn normalize_base32(input: &str) -> Zeroizing<String> {
    Zeroizing::new(
        input
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
            .map(|c| c.to_ascii_uppercase())
            .collect(),
    )
}

fn decode_secret(normalized: &str) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = BASE32_NOPAD
        .decode(normalized.as_bytes())
        .map_err(|_| Error::InvalidInput("TOTP secret is not valid Base32"))?;
    let bytes = Zeroizing::new(bytes);
    if bytes.len() < MIN_SECRET_BYTES || bytes.len() > MAX_SECRET_BYTES {
        return Err(Error::InvalidInput("TOTP secret has an invalid length"));
    }
    Ok(bytes)
}

/// Parse either an `otpauth://totp/...` URI or a bare Base32 secret
/// (which gets the standard defaults: SHA-1, 6 digits, 30 s).
pub fn parse_totp_input(input: &str) -> Result<TotpConfig> {
    let trimmed = input.trim();
    if trimmed.len() > MAX_URI_LEN {
        return Err(Error::InvalidInput("TOTP input too long"));
    }
    if trimmed.to_ascii_lowercase().starts_with("otpauth:") {
        return parse_otpauth(trimmed);
    }
    let secret = normalize_base32(trimmed);
    decode_secret(&secret)?;
    Ok(TotpConfig {
        secret: SecretString::new(secret.to_string()),
        algorithm: TotpAlgorithm::Sha1,
        digits: 6,
        period: 30,
        issuer: None,
        account: None,
    })
}

fn parse_otpauth(uri: &str) -> Result<TotpConfig> {
    let url = Url::parse(uri).map_err(|_| Error::InvalidInput("invalid otpauth URI"))?;
    if url.scheme() != "otpauth" {
        return Err(Error::InvalidInput("invalid otpauth URI"));
    }
    match url.host_str() {
        Some(h) if h.eq_ignore_ascii_case("totp") => {}
        Some(h) if h.eq_ignore_ascii_case("hotp") => {
            return Err(Error::InvalidInput("HOTP is not supported"))
        }
        _ => return Err(Error::InvalidInput("invalid otpauth URI")),
    }

    let mut secret: Option<Zeroizing<String>> = None;
    let mut algorithm = TotpAlgorithm::Sha1;
    let mut digits = 6u32;
    let mut period = 30u32;
    let mut issuer: Option<String> = None;

    for (key, value) in url.query_pairs() {
        match key.to_ascii_lowercase().as_str() {
            "secret" => secret = Some(normalize_base32(&value)),
            "algorithm" => {
                algorithm = match value.to_ascii_uppercase().as_str() {
                    "SHA1" => TotpAlgorithm::Sha1,
                    "SHA256" => TotpAlgorithm::Sha256,
                    "SHA512" => TotpAlgorithm::Sha512,
                    _ => return Err(Error::InvalidInput("unsupported TOTP algorithm")),
                }
            }
            "digits" => {
                digits = value
                    .parse()
                    .map_err(|_| Error::InvalidInput("invalid TOTP digits"))?
            }
            "period" => {
                period = value
                    .parse()
                    .map_err(|_| Error::InvalidInput("invalid TOTP period"))?
            }
            "issuer" => issuer = Some(truncate(&value)),
            _ => {} // e.g. "image"; ignored
        }
    }

    // Label is "/Issuer:account" or "/account", percent-encoded.
    let label = percent_decode(url.path().trim_start_matches('/'));
    let (label_issuer, account) = match label.split_once(':') {
        Some((i, a)) => (Some(truncate(i.trim())), truncate(a.trim())),
        None => (None, truncate(label.trim())),
    };

    let secret = secret.ok_or(Error::InvalidInput("otpauth URI has no secret"))?;
    let config = TotpConfig {
        secret: SecretString::new(secret.to_string()),
        algorithm,
        digits,
        period,
        issuer: issuer.or(label_issuer).filter(|s| !s.is_empty()),
        account: Some(account).filter(|s| !s.is_empty()),
    };
    config.validate()?;
    Ok(config)
}

fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .into_owned()
}

fn truncate(s: &str) -> String {
    s.chars().take(MAX_LABEL_LEN).collect()
}

fn hmac_digest(algorithm: TotpAlgorithm, key: &[u8], msg: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    fn run<M: Mac + KeyInit>(key: &[u8], msg: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let mut mac =
            <M as KeyInit>::new_from_slice(key).map_err(|_| Error::InvalidInput("TOTP key"))?;
        mac.update(msg);
        Ok(Zeroizing::new(mac.finalize().into_bytes().to_vec()))
    }
    match algorithm {
        TotpAlgorithm::Sha1 => run::<Hmac<sha1::Sha1>>(key, msg),
        TotpAlgorithm::Sha256 => run::<Hmac<sha2::Sha256>>(key, msg),
        TotpAlgorithm::Sha512 => run::<Hmac<sha2::Sha512>>(key, msg),
    }
}

/// RFC 4226 HOTP value with dynamic truncation.
fn hotp(algorithm: TotpAlgorithm, key: &[u8], counter: u64, digits: u32) -> Result<u32> {
    let digest = hmac_digest(algorithm, key, &counter.to_be_bytes())?;
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    Ok(binary % 10u32.pow(digits))
}

/// Generate the code valid at `unix_seconds`.
pub fn generate(config: &TotpConfig, unix_seconds: u64) -> Result<TotpCode> {
    config.validate()?;
    let key = decode_secret(config.secret.expose())?;
    let period = u64::from(config.period);
    let counter = unix_seconds / period;
    let value = hotp(config.algorithm, &key, counter, config.digits)?;
    let code = format!("{:0width$}", value, width = config.digits as usize);
    Ok(TotpCode {
        code: SecretString::new(code),
        period: config.period,
        seconds_remaining: (period - unix_seconds % period) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(secret_ascii: &[u8], algorithm: TotpAlgorithm) -> TotpConfig {
        TotpConfig {
            secret: SecretString::new(BASE32_NOPAD.encode(secret_ascii)),
            algorithm,
            digits: 8,
            period: 30,
            issuer: None,
            account: None,
        }
    }

    // RFC 6238 Appendix B test vectors.
    #[test]
    fn rfc6238_vectors() {
        let sha1 = cfg(b"12345678901234567890", TotpAlgorithm::Sha1);
        let sha256 = cfg(b"12345678901234567890123456789012", TotpAlgorithm::Sha256);
        let sha512 = cfg(
            b"1234567890123456789012345678901234567890123456789012345678901234",
            TotpAlgorithm::Sha512,
        );
        let vectors: [(u64, &str, &str, &str); 6] = [
            (59, "94287082", "46119246", "90693936"),
            (1111111109, "07081804", "68084774", "25091201"),
            (1111111111, "14050471", "67062674", "99943326"),
            (1234567890, "89005924", "91819424", "93441116"),
            (2000000000, "69279037", "90698825", "38618901"),
            (20000000000, "65353130", "77737706", "47863826"),
        ];
        for (t, c1, c256, c512) in vectors {
            assert_eq!(generate(&sha1, t).unwrap().code.expose(), c1, "sha1 t={t}");
            assert_eq!(
                generate(&sha256, t).unwrap().code.expose(),
                c256,
                "sha256 t={t}"
            );
            assert_eq!(
                generate(&sha512, t).unwrap().code.expose(),
                c512,
                "sha512 t={t}"
            );
        }
    }

    #[test]
    fn six_digits_and_remaining_time() {
        let mut c = cfg(b"12345678901234567890", TotpAlgorithm::Sha1);
        c.digits = 6;
        let code = generate(&c, 59).unwrap();
        assert_eq!(code.code.expose(), "287082");
        assert_eq!(code.seconds_remaining, 1);
        assert_eq!(generate(&c, 60).unwrap().seconds_remaining, 30);
    }

    #[test]
    fn parses_full_otpauth_uri() {
        let c = parse_totp_input(
            "otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA256&digits=8&period=60",
        )
        .unwrap();
        assert_eq!(c.algorithm, TotpAlgorithm::Sha256);
        assert_eq!(c.digits, 8);
        assert_eq!(c.period, 60);
        assert_eq!(c.issuer.as_deref(), Some("ACME Co"));
        assert_eq!(c.account.as_deref(), Some("john.doe@email.com"));
        assert_eq!(c.secret.expose(), "HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ");
    }

    #[test]
    fn otpauth_defaults_and_label_issuer() {
        let c = parse_totp_input("otpauth://totp/GitHub:me?secret=jbswy3dpehpk3pxp").unwrap();
        assert_eq!(c.algorithm, TotpAlgorithm::Sha1);
        assert_eq!((c.digits, c.period), (6, 30));
        assert_eq!(c.issuer.as_deref(), Some("GitHub"));
        assert_eq!(c.secret.expose(), "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn bare_secret_with_spaces() {
        let c = parse_totp_input("  jbsw y3dp ehpk 3pxp ").unwrap();
        assert_eq!(c.secret.expose(), "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "",
            "not base32 !!!",
            "JBSWY3DP", // too short (5 bytes)
            "otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP&counter=1",
            "otpauth://totp/x",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=7",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=abc",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=0",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=100000",
            "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=MD5",
            "https://totp/x?secret=JBSWY3DPEHPK3PXP",
        ] {
            assert!(parse_totp_input(bad).is_err(), "accepted {bad:?}");
        }
        assert!(parse_totp_input(&"A".repeat(5000)).is_err());
    }

    #[test]
    fn debug_hides_secret_and_code() {
        let c = parse_totp_input("JBSWY3DPEHPK3PXP").unwrap();
        assert!(!format!("{c:?}").contains("JBSWY3DP"));
        let code = generate(&c, 1000).unwrap();
        assert!(!format!("{code:?}").contains(code.code.expose()));
    }
}
