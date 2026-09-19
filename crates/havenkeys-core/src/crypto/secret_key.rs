//! Secret Key: 128 random bits that, together with the master password,
//! protect the vault (docs/crypto.md, "Secret Key").
//!
//! It never leaves the user's devices except on the Emergency Kit they save.
//! A copy of the vault from a sync folder or a backup cannot be unlocked with
//! the master password alone, so offline guessing needs both.
//!
//! Text form: `H1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`
//! * `H1` names the format version;
//! * 26 Base32 (RFC 4648) characters of key, then 2 check characters (10
//!   bits of SHA-256 over a label and the key), which catch typos before the
//!   slow key derivation runs. The check is not a security feature.
//!
//! Parsing is forgiving: case, spaces and dashes are ignored, and the digits
//! people type for look-alike letters (0→O, 1→I, 8→B) are accepted.

use crate::crypto::fill_random;
use crate::error::{Error, Result};
use crate::secret::SecretString;
use data_encoding::BASE32_NOPAD;
use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::Zeroizing;

pub const SECRET_KEY_LEN: usize = 16;
const PREFIX: &str = "H1";
const KEY_CHARS: usize = 26;
const CHECK_CHARS: usize = 2;
const GROUP: usize = 4;
const CHECK_LABEL: &[u8] = b"havenkeys/secret-key/check";

pub struct SecretKey(Zeroizing<[u8; SECRET_KEY_LEN]>);

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

fn check_chars(key: &[u8; SECRET_KEY_LEN]) -> String {
    let digest = Sha256::new()
        .chain_update(CHECK_LABEL)
        .chain_update(key)
        .finalize();
    // Base32 of the digest; the first two characters carry 10 bits.
    BASE32_NOPAD.encode(&digest[..5])[..CHECK_CHARS].to_owned()
}

impl SecretKey {
    /// A fresh key from the OS CSPRNG.
    pub fn generate() -> Result<Self> {
        let mut key = Zeroizing::new([0u8; SECRET_KEY_LEN]);
        fill_random(key.as_mut())?;
        Ok(Self(key))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; SECRET_KEY_LEN] {
        &self.0
    }

    /// `H1-XXXX-…`, for the Emergency Kit and the device's own storage.
    pub fn to_text(&self) -> SecretString {
        let mut body = Zeroizing::new(BASE32_NOPAD.encode(self.0.as_ref()));
        body.push_str(&check_chars(&self.0));
        let mut out = String::with_capacity(PREFIX.len() + body.len() + body.len() / GROUP + 1);
        out.push_str(PREFIX);
        for chunk in body.as_bytes().chunks(GROUP) {
            out.push('-');
            out.extend(chunk.iter().map(|&b| b as char));
        }
        SecretString::new(out)
    }

    /// Parse a typed or stored key. Wrong length, bad characters or a failed
    /// check all give the same error.
    pub fn parse(text: &str) -> Result<Self> {
        const BAD: Error = Error::InvalidInput("that Secret Key is not valid");
        let mut clean = Zeroizing::new(String::with_capacity(40));
        for c in text.chars() {
            match c.to_ascii_uppercase() {
                ' ' | '-' | '\t' => {}
                '0' => clean.push('O'),
                '1' => clean.push('I'),
                '8' => clean.push('B'),
                c @ ('A'..='Z' | '2'..='7') => clean.push(c),
                _ => return Err(BAD),
            }
            if clean.len() > PREFIX.len() + KEY_CHARS + CHECK_CHARS {
                return Err(BAD);
            }
        }
        // "H1" may be typed with a digit that was mapped to a letter above.
        let body = clean.strip_prefix("HI").ok_or(BAD)?;
        if body.len() != KEY_CHARS + CHECK_CHARS {
            return Err(BAD);
        }
        let (key_part, check) = body.split_at(KEY_CHARS);
        let decoded = Zeroizing::new(BASE32_NOPAD.decode(key_part.as_bytes()).map_err(|_| BAD)?);
        let bytes: [u8; SECRET_KEY_LEN] = decoded.as_slice().try_into().map_err(|_| BAD)?;
        let key = Zeroizing::new(bytes);
        if check_chars(&key) != check {
            return Err(BAD);
        }
        Ok(Self(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_formats() {
        let k = SecretKey::generate().unwrap();
        let text = k.to_text();
        let t = text.expose();
        assert!(t.starts_with("H1-"), "{t}");
        assert_eq!(t.len(), 2 + 7 * 5);
        assert_eq!(SecretKey::parse(t).unwrap().as_bytes(), k.as_bytes());
        // Forgiving input.
        let sloppy = t.to_lowercase().replace('-', " ");
        assert_eq!(SecretKey::parse(&sloppy).unwrap().as_bytes(), k.as_bytes());
    }

    #[test]
    fn accepts_look_alike_digits() {
        let k = SecretKey::from_fixed([0u8; 16]);
        let t = k.to_text().expose().to_owned(); // all 'A's plus check
        let typed = t.replacen("H1", "h1", 1);
        assert!(SecretKey::parse(&typed).is_ok());
    }

    #[test]
    fn rejects_typos_and_junk() {
        let k = SecretKey::generate().unwrap();
        let t = k.to_text().expose().to_owned();
        // Change one key character.
        let mut chars: Vec<char> = t.chars().collect();
        let i = 4;
        chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
        let typo: String = chars.into_iter().collect();
        assert!(SecretKey::parse(&typo).is_err());
        for junk in [
            "",
            "H1",
            "H2-AAAA",
            "not a key",
            &format!("{t}AAAA"),
            &t[..t.len() - 1],
        ] {
            assert!(SecretKey::parse(junk).is_err(), "{junk}");
        }
    }

    #[test]
    fn debug_is_redacted() {
        let k = SecretKey::generate().unwrap();
        assert_eq!(format!("{k:?}"), "SecretKey(<redacted>)");
    }

    impl SecretKey {
        fn from_fixed(b: [u8; 16]) -> Self {
            Self(Zeroizing::new(b))
        }
    }
}
