//! Wrapper for secret strings: zeroized on drop, never printed.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use zeroize::Zeroizing;

/// A UTF-8 secret (password, TOTP secret, note body).
///
/// * Memory is overwritten when the value is dropped.
/// * `Debug` never shows the contents.
/// * Deliberately does not implement `Display`.
///
/// Copies made by other code (serde buffers, IPC, the WebView) are outside our
/// control; see docs/security-model.md §Memory.
#[derive(Clone, Default)]
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Length in characters (not bytes).
    pub fn char_len(&self) -> usize {
        self.0.chars().count()
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value.to_owned())
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(<redacted>)")
    }
}

impl Serialize for SecretString {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer).map(SecretString::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let s = SecretString::from("hunter2");
        let printed = format!("{s:?}");
        assert!(!printed.contains("hunter2"));
        assert!(printed.contains("redacted"));
    }

    #[test]
    fn serde_round_trip() {
        let s = SecretString::from("p@ss");
        let json = serde_json::to_string(&s).unwrap();
        let back: SecretString = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "p@ss");
    }
}
