//! Email normalization, restated from `havenkeys-core::account`.
//!
//! The rules must match the client's exactly: the normalized address is part
//! of the client's key derivation (docs/crypto.md, key scheme 3) and the
//! server's account identity. Two spellings that normalize differently on the
//! two sides would look like two accounts. The server does not depend on the
//! client crate — it must be deployable without it — so the rules live in
//! both places and `tests/email.rs` pins them together.

use unicode_normalization::UnicodeNormalization;

/// Longest address accepted, from the SMTP path limit (RFC 5321 §4.5.3.1.3).
const MAX_EMAIL_LEN: usize = 254;

pub fn normalize(raw: &str) -> Result<String, &'static str> {
    const BAD: &str = "email address is not valid";
    let normalized: String = raw.trim().nfc().collect::<String>().to_lowercase();
    if normalized.is_empty() || normalized.len() > MAX_EMAIL_LEN {
        return Err(BAD);
    }
    if normalized.chars().any(char::is_whitespace) {
        return Err(BAD);
    }
    let mut parts = normalized.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if local.is_empty() || domain.is_empty() || parts.next().is_some() {
        return Err(BAD);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_lowercases_and_composes() {
        assert_eq!(normalize("  User@Example.COM ").unwrap(), "user@example.com");
        assert_eq!(
            normalize("jose\u{0301}@example.com").unwrap(),
            normalize("jos\u{00e9}@example.com").unwrap()
        );
    }

    #[test]
    fn refuses_what_the_core_refuses() {
        for bad in [
            "",
            "   ",
            "no-at-sign",
            "@example.com",
            "user@",
            "user@@example.com",
            "user name@example.com",
            "user\n@example.com",
        ] {
            assert!(normalize(bad).is_err(), "should have refused {bad:?}");
        }
        assert!(normalize(&format!("{}@example.com", "a".repeat(MAX_EMAIL_LEN))).is_err());
    }
}
