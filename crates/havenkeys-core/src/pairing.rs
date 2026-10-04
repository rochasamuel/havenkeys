//! Signing in a new device from an unlocked one (spec
//! 2026-10-03-phone-approved-sign-in). The new device's key pair, the
//! `havenkeys://pair/v1` link that carries its public key from its screen to
//! the phone's camera, and the envelope the phone seals to that key with
//! HPKE (RFC 9180). The server relays the envelope and cannot open it.

use crate::crypto::fill_random;
use crate::crypto::keys::Key256;
use crate::crypto::secret_key::SecretKey;
use crate::error::{Error, Result};
use data_encoding::BASE64URL_NOPAD;
use hpke::aead::AesGcm256;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::{Deserializable, Kem as _, OpModeR, OpModeS, Serializable};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroizing;

type Kem = X25519HkdfSha256;
type Kdf = HkdfSha256;
type Aead = AesGcm256;

const LINK_PREFIX: &str = "havenkeys://pair/v1?";
const MAX_LINK_LEN: usize = 512;
pub const MAX_ENVELOPE_LEN: usize = 4096;
const ENVELOPE_V1: u8 = 1;
/// RFC 9180 identifiers: KEM 0x0020, KDF 0x0001, AEAD 0x0002.
const SUITE: [u8; 6] = [0x00, 0x20, 0x00, 0x01, 0x00, 0x02];
const ENC_LEN: usize = 32;
const HEADER_LEN: usize = 1 + SUITE.len() + ENC_LEN;
const TAG_LEN: usize = 16;
const PAYLOAD_V1: u8 = 1;
const MAX_EMAIL_LEN: usize = 320;
const MAX_SECRET_KEY_TEXT: usize = 64;
const PAIRING_ID_CHARS: usize = 22;

/// What the QR code says.
pub struct PairingLink {
    pub server_url: String,
    pub pairing_id: String,
    pub public_key: [u8; 32],
}

/// 16 random bytes in base64url: anything else is refused before it is
/// put in a request path.
pub fn valid_pairing_id(id: &str) -> bool {
    id.len() == PAIRING_ID_CHARS
        && BASE64URL_NOPAD
            .decode(id.as_bytes())
            .is_ok_and(|b| b.len() == 16)
}

impl PairingLink {
    pub fn to_text(&self) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("server", &self.server_url)
            .append_pair("id", &self.pairing_id)
            .append_pair("pk", &BASE64URL_NOPAD.encode(&self.public_key))
            .finish();
        format!("{LINK_PREFIX}{query}")
    }

    /// Exactly `server`, `id` and `pk`, once each; anything else is not a
    /// pairing link.
    pub fn parse(text: &str) -> Result<Self> {
        const BAD: Error = Error::InvalidInput("that is not a HavenKeys sign-in code");
        if text.len() > MAX_LINK_LEN {
            return Err(BAD);
        }
        let query = text.strip_prefix(LINK_PREFIX).ok_or(BAD)?;
        let (mut server, mut id, mut pk) = (None, None, None);
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            let slot = match key.as_ref() {
                "server" => &mut server,
                "id" => &mut id,
                "pk" => &mut pk,
                _ => return Err(BAD),
            };
            if slot.replace(value.into_owned()).is_some() {
                return Err(BAD);
            }
        }
        let (server, id, pk) = (server.ok_or(BAD)?, id.ok_or(BAD)?, pk.ok_or(BAD)?);
        let parsed = url::Url::parse(&server).map_err(|_| BAD)?;
        if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
            return Err(BAD);
        }
        if !valid_pairing_id(&id) {
            return Err(BAD);
        }
        let public_key: [u8; 32] = BASE64URL_NOPAD
            .decode(pk.as_bytes())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or(BAD)?;
        Ok(Self {
            server_url: server,
            pairing_id: id,
            public_key,
        })
    }
}

/// The new device's HPKE key pair, in memory only, for one pairing.
pub struct PairingKeys {
    secret: <Kem as hpke::Kem>::PrivateKey,
    public: [u8; 32],
}

impl PairingKeys {
    pub fn generate() -> Self {
        let (secret, public_key) = Kem::gen_keypair();
        let mut public = [0u8; 32];
        public.copy_from_slice(&public_key.to_bytes());
        Self { secret, public }
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.public
    }

    /// Open an envelope sealed to this key for this pairing on this server.
    pub fn open(
        &self,
        server_url: &str,
        pairing_id: &str,
        envelope: &[u8],
    ) -> Result<PairingPayload> {
        if envelope.len() > MAX_ENVELOPE_LEN || envelope.len() < HEADER_LEN + TAG_LEN {
            return Err(Error::Corrupted);
        }
        if envelope[0] != ENVELOPE_V1 || envelope[1..1 + SUITE.len()] != SUITE {
            return Err(Error::UnsupportedVersion);
        }
        let enc =
            <Kem as hpke::Kem>::EncappedKey::from_bytes(&envelope[1 + SUITE.len()..HEADER_LEN])
                .map_err(|_| Error::Corrupted)?;
        let plain = Zeroizing::new(
            hpke::single_shot_open::<Aead, Kdf, Kem>(
                &OpModeR::Base,
                &self.secret,
                &enc,
                &info(server_url, pairing_id),
                &envelope[HEADER_LEN..],
                &[],
            )
            .map_err(|_| Error::Decryption)?,
        );
        PairingPayload::decode(&plain)
    }
}

impl fmt::Debug for PairingKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingKeys(<redacted>)")
    }
}

/// The secret the new device claims its session with. Never in the QR code.
pub struct ClaimSecret(Zeroizing<[u8; 32]>);

impl ClaimSecret {
    pub fn generate() -> Result<Self> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        fill_random(bytes.as_mut())?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// What the server stores and compares.
    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.0.as_ref()).into()
    }
}

impl fmt::Debug for ClaimSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClaimSecret(<redacted>)")
    }
}

/// What the approving device hands the new one.
pub struct PairingPayload {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub email: String,
    pub secret_key: SecretKey,
    // Read by VaultService::seal_pairing / the new device (later tasks).
    #[allow(dead_code)]
    pub(crate) vault_key: Key256,
}

impl fmt::Debug for PairingPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingPayload(<redacted>)")
    }
}

impl PairingPayload {
    /// `1 ‖ account (16) ‖ vault (16) ‖ vault key (32) ‖ len u8 ‖ Secret Key
    /// text ‖ len u16 BE ‖ email`.
    // Consumed by VaultService::seal_pairing (a later task).
    #[allow(dead_code)]
    fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        let key = self.secret_key.to_text();
        let key = key.expose().as_bytes();
        let email = self.email.as_bytes();
        if key.len() > MAX_SECRET_KEY_TEXT || email.is_empty() || email.len() > MAX_EMAIL_LEN {
            return Err(Error::InvalidInput("the account cannot be sent"));
        }
        let mut out = Zeroizing::new(Vec::with_capacity(
            1 + 16 + 16 + 32 + 1 + key.len() + 2 + email.len(),
        ));
        out.push(PAYLOAD_V1);
        out.extend_from_slice(self.account_id.as_bytes());
        out.extend_from_slice(self.vault_id.as_bytes());
        out.extend_from_slice(self.vault_key.as_bytes());
        out.push(key.len() as u8);
        out.extend_from_slice(key);
        out.extend_from_slice(&(email.len() as u16).to_be_bytes());
        out.extend_from_slice(email);
        Ok(out)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        const BAD: Error = Error::Corrupted;
        let mut rest = bytes;
        let mut take = |n: usize| -> Result<&[u8]> {
            if rest.len() < n {
                return Err(BAD);
            }
            let (head, tail) = rest.split_at(n);
            rest = tail;
            Ok(head)
        };
        if take(1)?[0] != PAYLOAD_V1 {
            return Err(Error::UnsupportedVersion);
        }
        let account_id = Uuid::from_slice(take(16)?).map_err(|_| BAD)?;
        let vault_id = Uuid::from_slice(take(16)?).map_err(|_| BAD)?;
        let mut key = [0u8; 32];
        key.copy_from_slice(take(32)?);
        let vault_key = Key256::from_bytes(key);
        let key_len = take(1)?[0] as usize;
        if key_len > MAX_SECRET_KEY_TEXT {
            return Err(BAD);
        }
        let key_text = std::str::from_utf8(take(key_len)?).map_err(|_| BAD)?;
        let secret_key = SecretKey::parse(key_text).map_err(|_| BAD)?;
        let email_len = u16::from_be_bytes(take(2)?.try_into().map_err(|_| BAD)?) as usize;
        if email_len == 0 || email_len > MAX_EMAIL_LEN {
            return Err(BAD);
        }
        let email = std::str::from_utf8(take(email_len)?)
            .map_err(|_| BAD)?
            .to_owned();
        if !rest.is_empty() {
            return Err(BAD);
        }
        Ok(Self {
            account_id,
            vault_id,
            email,
            secret_key,
            vault_key,
        })
    }
}

fn info(server_url: &str, pairing_id: &str) -> Vec<u8> {
    let mut out = b"havenkeys/pair/v1\0".to_vec();
    out.extend_from_slice(server_url.as_bytes());
    out.push(0);
    out.extend_from_slice(pairing_id.as_bytes());
    out
}

/// Seal `payload` to the link's key. Only `VaultService::seal_pairing`
/// calls this, so the vault key never leaves the core.
// Called by VaultService::seal_pairing (a later task).
#[allow(dead_code)]
pub(crate) fn seal(link: &PairingLink, payload: &PairingPayload) -> Result<Vec<u8>> {
    let recipient = <Kem as hpke::Kem>::PublicKey::from_bytes(&link.public_key)
        .map_err(|_| Error::InvalidInput("that is not a HavenKeys sign-in code"))?;
    let plain = payload.encode()?;
    let (enc, ciphertext) = hpke::single_shot_seal::<Aead, Kdf, Kem>(
        &OpModeS::Base,
        &recipient,
        &info(&link.server_url, &link.pairing_id),
        &plain,
        &[],
    )
    .map_err(|_| Error::Encryption)?;
    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.push(ENVELOPE_V1);
    out.extend_from_slice(&SUITE);
    out.extend_from_slice(&enc.to_bytes());
    out.extend_from_slice(&ciphertext);
    if out.len() > MAX_ENVELOPE_LEN {
        return Err(Error::Encryption);
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;

    const SERVER: &str = "https://vault.example.com";
    const ID: &str = "AAAAAAAAAAAAAAAAAAAAAA";

    fn payload() -> PairingPayload {
        PairingPayload {
            account_id: Uuid::from_u128(1),
            vault_id: Uuid::from_u128(2),
            email: "ana@example.com".into(),
            secret_key: SecretKey::generate().unwrap(),
            vault_key: Key256::from_bytes([9u8; 32]),
        }
    }

    fn link_for(keys: &PairingKeys) -> PairingLink {
        PairingLink {
            server_url: SERVER.into(),
            pairing_id: ID.into(),
            public_key: keys.public_key(),
        }
    }

    #[test]
    fn a_link_round_trips() {
        let keys = PairingKeys::generate();
        let link = link_for(&keys);
        let text = link.to_text();
        assert!(text.starts_with("havenkeys://pair/v1?"));
        let back = PairingLink::parse(&text).unwrap();
        assert_eq!(back.server_url, SERVER);
        assert_eq!(back.pairing_id, ID);
        assert_eq!(back.public_key, keys.public_key());
    }

    #[test]
    fn malformed_links_are_refused() {
        let good = link_for(&PairingKeys::generate()).to_text();
        for bad in [
            "",
            "https://vault.example.com",
            "havenkeys://kit/v2?account=x",
            "havenkeys://pair/v2?server=https://a&id=AAAAAAAAAAAAAAAAAAAAAA&pk=AA",
            &good.replace("id=AAAAAAAAAAAAAAAAAAAAAA", "id=short"),
            &good.replace("pk=", "pk=AA"),
            &format!("{good}&extra=1"),
            &format!("{good}&id=AAAAAAAAAAAAAAAAAAAAAA"),
            &good.replace("server=https", "server=ftp"),
            &"havenkeys://pair/v1?".repeat(40),
        ] {
            assert!(PairingLink::parse(bad).is_err(), "accepted: {bad}");
        }
    }

    #[test]
    fn an_envelope_opens_with_its_key_and_binding_only() {
        let keys = PairingKeys::generate();
        let link = link_for(&keys);
        let sealed = seal(&link, &payload()).unwrap();
        assert!(sealed.len() <= MAX_ENVELOPE_LEN);
        let opened = keys.open(SERVER, ID, &sealed).unwrap();
        assert_eq!(opened.account_id, Uuid::from_u128(1));
        assert_eq!(opened.vault_id, Uuid::from_u128(2));
        assert_eq!(opened.email, "ana@example.com");
        assert_eq!(opened.vault_key.as_bytes(), &[9u8; 32]);

        // Another device's key, another pairing, another server: nothing opens.
        assert!(PairingKeys::generate().open(SERVER, ID, &sealed).is_err());
        assert!(keys
            .open(SERVER, "BBBBBBBBBBBBBBBBBBBBBB", &sealed)
            .is_err());
        assert!(keys.open("https://evil.example.com", ID, &sealed).is_err());
    }

    #[test]
    fn a_tampered_envelope_never_opens() {
        let keys = PairingKeys::generate();
        let sealed = seal(&link_for(&keys), &payload()).unwrap();
        for i in 0..sealed.len() {
            let mut bad = sealed.clone();
            bad[i] ^= 0x01;
            assert!(keys.open(SERVER, ID, &bad).is_err(), "byte {i}");
        }
        assert!(keys.open(SERVER, ID, &sealed[..sealed.len() - 1]).is_err());
        let mut long = sealed.clone();
        long.push(0);
        assert!(keys.open(SERVER, ID, &long).is_err());
    }

    #[test]
    fn an_unknown_version_or_suite_is_refused_as_such() {
        let keys = PairingKeys::generate();
        let mut sealed = seal(&link_for(&keys), &payload()).unwrap();
        sealed[0] = 2;
        assert_eq!(
            keys.open(SERVER, ID, &sealed).err(),
            Some(Error::UnsupportedVersion)
        );
    }

    #[test]
    fn garbage_never_panics() {
        let keys = PairingKeys::generate();
        for len in 0..200usize {
            for first in [0u8, 1, 2, 0xFF] {
                let mut data = vec![0xA5u8; len];
                if len > 0 {
                    data[0] = first;
                }
                assert!(keys.open(SERVER, ID, &data).is_err());
                assert!(PairingPayload::decode(&data).is_err());
            }
        }
        assert!(keys
            .open(SERVER, ID, &vec![1u8; MAX_ENVELOPE_LEN + 1])
            .is_err());
    }

    #[test]
    fn nothing_secret_is_printed() {
        let shown = format!("{:?} {:?}", payload(), PairingKeys::generate());
        assert!(!shown.contains("ana@example.com"));
        assert!(!shown.contains("H1-"));
        assert!(shown.contains("redacted"));
    }

    #[test]
    fn a_claim_secret_hashes_with_sha256() {
        let secret = ClaimSecret::generate().unwrap();
        let expected: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
        assert_eq!(secret.hash(), expected);
        assert_ne!(
            ClaimSecret::generate().unwrap().as_bytes(),
            secret.as_bytes()
        );
    }

    #[test]
    fn pairing_ids_are_22_base64url_characters() {
        assert!(valid_pairing_id(ID));
        assert!(!valid_pairing_id("AAAA"));
        assert!(!valid_pairing_id("AAAAAAAAAAAAAAAAAAAAA/"));
        assert!(!valid_pairing_id("../../../../../v1/sync"));
    }
}
