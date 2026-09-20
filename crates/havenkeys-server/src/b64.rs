//! Base64 blobs on the wire.
//!
//! The length is checked while decoding, before the bytes are kept, so an
//! oversized blob costs one bounded decode rather than a stored allocation.
//! The server never looks inside: these are AES-256-GCM ciphertexts it has no
//! key for.

use crate::limits::MAX_BLOB_BYTES;
use data_encoding::BASE64;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, PartialEq, Eq)]
pub struct Blob(pub Vec<u8>);

impl Blob {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Blob {
    /// Never print ciphertext, not even truncated.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Blob({} bytes)", self.0.len())
    }
}

impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        // Base64 expands by 4/3; reject before decoding rather than after.
        if raw.len() > MAX_BLOB_BYTES / 3 * 4 + 4 {
            return Err(D::Error::custom("blob is too large"));
        }
        let bytes = BASE64
            .decode(raw.as_bytes())
            .map_err(|_| D::Error::custom("blob is not base64"))?;
        if bytes.len() > MAX_BLOB_BYTES {
            return Err(D::Error::custom("blob is too large"));
        }
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blob_round_trips() {
        let json = serde_json::to_string(&Blob(vec![1, 2, 3])).unwrap();
        assert_eq!(json, "\"AQID\"");
        assert_eq!(
            serde_json::from_str::<Blob>(&json).unwrap().0,
            vec![1, 2, 3]
        );
    }

    #[test]
    fn a_blob_over_the_limit_is_refused_without_being_kept() {
        let oversized = "A".repeat(MAX_BLOB_BYTES / 3 * 4 + 8);
        let json = serde_json::to_string(&oversized).unwrap();
        assert!(serde_json::from_str::<Blob>(&json).is_err());
    }

    #[test]
    fn a_blob_that_is_not_base64_is_refused() {
        assert!(serde_json::from_str::<Blob>("\"not base64!\"").is_err());
    }

    #[test]
    fn debug_never_prints_the_bytes() {
        assert_eq!(format!("{:?}", Blob(vec![7, 7, 7])), "Blob(3 bytes)");
    }
}
