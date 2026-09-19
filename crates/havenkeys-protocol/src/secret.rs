//! Secret values on the wire.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use zeroize::Zeroizing;

/// A password or TOTP code in a response: zeroized on drop, never printed.
///
/// Mirrors `havenkeys_core::SecretString`, which this crate cannot depend on.
#[derive(Clone, PartialEq, Eq)]
pub struct WireSecret(Zeroizing<String>);

impl WireSecret {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WireSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WireSecret(<redacted>)")
    }
}

impl Serialize for WireSecret {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WireSecret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self::new)
    }
}
