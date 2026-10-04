# Signing In a New Desktop From the Phone — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new desktop shows a QR code; the user's unlocked phone scans it, shows the device's name and location, and on Allow (behind biometrics) the desktop opens the vault without the Emergency Kit or the master password.

**Architecture:** The desktop opens a pairing request on the user's server and shows `havenkeys://pair/v1?server&id&pk`. The phone (with its own session) reads the request's details and, on Allow, seals {account id, vault id, email, Secret Key, vault key} to the desktop's X25519 public key with HPKE, inside the Rust core. The server registers the desktop's device and issues its session; the desktop claims both with a secret that never appears in the QR, opens the envelope, verifies the vault header with the vault key and creates its local vault exactly as a sign-in does.

**Tech Stack:** Rust (havenkeys-core, -sync-client, -client, -server, -mobile), `hpke` 0.14 (RustCrypto, RFC 9180), `maxminddb` 0.32, axum + tokio-postgres, Tauri 2 + React/TypeScript, Kotlin/Compose via uniffi.

**Spec:** `docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md`

## Global Constraints

- HPKE base mode, suite DHKEM(X25519, HKDF-SHA256) / HKDF-SHA256 / AES-256-GCM, via `hpke` 0.14 only. No hand-assembled construction (CLAUDE.md §5, §6).
- HPKE `info` = `"havenkeys/pair/v1" ‖ 0x00 ‖ server_url ‖ 0x00 ‖ pairing_id`.
- Envelope wire bytes: `0x01 ‖ 00 20 00 01 00 02 ‖ enc (32) ‖ ciphertext`; at most `4096` bytes (`MAX_PAIRING_ENVELOPE_BYTES`).
- Pairing lifetime `120` seconds; rows older than `10` minutes are deleted on each create.
- Per IP: at most `10` creates per 10 minutes and `3` pending at once; over that, `429 rate_limited`.
- `pairing_id`: 16 random bytes, base64url without padding (22 characters).
- `claim_secret`: 32 random bytes; the server stores only its SHA-256.
- Desktop polls claim every `2` seconds.
- Allow on the phone requires `BiometricGate.verifyUser` (biometrics or device credential).
- The phone approves only when unlocked and online, and only a link whose server equals its account's `server_url`.
- Vault key, Secret Key, envelope plaintext, claim secret and session token: `Zeroizing`/`SecretString`, never logged, never in `Debug`, never in an error message (CLAUDE.md §39, §40).
- User-facing errors (EN / pt-BR), fixed strings only:
  - `pairing_gone`: "This code has expired. Ask the new device for a new one." / "Este código expirou. Peça um novo no novo dispositivo."
  - `pairing_other_server`: "This code is for another server." / "Este código é de outro servidor."
  - `pairing_failed`: "The sign-in could not be completed. Ask for a new code." / "Não foi possível concluir a entrada. Peça um novo código."
  - desktop denied: "The sign-in was denied on your phone." / "A entrada foi recusada no seu celular."
- Commit messages: no `Co-Authored-By` trailer (repository rule).
- Postgres-backed tests run with `scripts/test-server.sh` (starts `havenkeys-test-pg` on port 5433), or with `HAVENKEYS_TEST_DATABASE_URL` set.
- Android commands need `export JAVA_HOME=/home/sams/.local/opt/jdk-17.0.20.1+1`; the flavour task names are `…GithubDebug…`.

## Review Focus

1. **A desktop that never claims** (closed, crashed) after the phone approved: the issued session must not outlive the pairing row by much, and the device appears in Devices where it can be revoked. Test: Task 3 `an_approved_pairing_never_claimed_is_gone_after_ten_minutes` (forces `created_at` back and runs create).
2. **The phone's clock or network dies between Allow and approve**: the phone must show `pairing_failed` or `offline`, not hang; a retried Allow on the same link after a success gets `pairing_gone`. Test: Task 6 `approving_twice_is_refused_the_second_time`.
3. **The desktop's server field typed with a trailing slash or spaces** while the phone's account URL has none: links must compare equal after the same normalisation `sign_in` uses. Test: Task 6 `a_trailing_slash_on_the_desktop_still_matches_the_phone`.
4. **A QR that is not a pairing link** (a TOTP code, a kit, a URL) scanned on the phone's pairing screen: dropped silently, scanning continues. Test: Task 8 `scan_pairing_keeps_only_pair_links`.
5. **The desktop already has a vault** (someone clicks the phone option on an existing install): `start_pairing` refuses with `vault_exists` before contacting the server. Test: Task 6 `pairing_refuses_a_device_that_already_has_a_vault`.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-core/src/pairing.rs` (new) | Link build/parse, key pair, claim secret, payload encoding, HPKE seal/open |
| `crates/havenkeys-core/src/vault.rs` | Session keeps the vault key; `seal_pairing` |
| `crates/havenkeys-core/src/sync.rs` | `prepare_paired_sign_in` |
| `crates/havenkeys-server/migrations/0002_pairings.sql` (new) | `pairings` table, `devices.approved_by` |
| `crates/havenkeys-server/src/routes/pairings.rs` (new) | The five routes |
| `crates/havenkeys-server/src/locate.rs` (new) | Optional IP → "City, CC" |
| `crates/havenkeys-sync-client/src/client.rs`, `wire.rs` | Typed pairing calls |
| `crates/havenkeys-client/src/pairing.rs` (new) | Desktop start/poll/finish/cancel; phone details/approve/deny |
| `apps/desktop/src-tauri/src/pairing.rs` (new) | Tauri commands |
| `apps/desktop/src/views/PhoneSignInPanel.tsx` (new), `src/lib/pairing.ts` (new) | Panel and its pure helpers |
| `crates/havenkeys-mobile/src/pairing.rs` (new) | uniffi exports |
| `apps/android/.../ui/pairing/*` (new) | Scan screen, confirmation sheet, ViewModel |

---

### Task 1: Core pairing primitives

**Files:**
- Create: `crates/havenkeys-core/src/pairing.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `pub mod pairing;` after `pub mod origin;`)
- Modify: `crates/havenkeys-core/Cargo.toml` (dependency)

**Interfaces:**
- Produces:
  - `pub struct PairingLink { pub server_url: String, pub pairing_id: String, pub public_key: [u8; 32] }` with `parse(&str) -> Result<Self>` and `to_text(&self) -> String`
  - `pub struct PairingKeys` with `generate() -> Self`, `public_key(&self) -> [u8; 32]`, `open(&self, server_url: &str, pairing_id: &str, envelope: &[u8]) -> Result<PairingPayload>`
  - `pub struct ClaimSecret` with `generate() -> Result<Self>`, `as_bytes(&self) -> &[u8; 32]`, `hash(&self) -> [u8; 32]`
  - `pub struct PairingPayload { pub account_id: Uuid, pub vault_id: Uuid, pub email: String, pub secret_key: SecretKey, pub(crate) vault_key: Key256 }`
  - `pub(crate) fn seal(link: &PairingLink, payload: &PairingPayload) -> Result<Vec<u8>>`
  - `pub fn valid_pairing_id(id: &str) -> bool`, `pub const MAX_ENVELOPE_LEN: usize = 4096`

- [ ] **Step 1: Dependency gate (CLAUDE.md §48)**

Add to `crates/havenkeys-core/Cargo.toml` under `# --- cryptography (RustCrypto) ---`:

```toml
# HPKE (RFC 9180) for the pairing envelope: X25519, HKDF-SHA256, AES-256-GCM.
# rozbb/rust-hpke; NCC Group audit (2021); MIT/Apache-2.0.
hpke = { version = "0.14", default-features = false, features = ["alloc", "getrandom", "x25519", "aes"] }
```

Run: `cargo tree -p havenkeys-core -i hpke && cargo deny check 2>&1 | tail -5 && cargo audit 2>&1 | tail -5`
Expected: `hpke` resolves against the existing `aes-gcm 0.11` / `hkdf 0.13`; `deny` and `audit` report no new failure. If either fails, STOP and report: do not substitute another construction.

- [ ] **Step 2: Write the failing tests** (bottom of the new `pairing.rs`)

```rust
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
        assert!(keys.open(SERVER, "BBBBBBBBBBBBBBBBBBBBBB", &sealed).is_err());
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
        assert_eq!(keys.open(SERVER, ID, &sealed).err(), Some(Error::UnsupportedVersion));
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
        assert!(keys.open(SERVER, ID, &vec![1u8; MAX_ENVELOPE_LEN + 1]).is_err());
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
        assert_ne!(ClaimSecret::generate().unwrap().as_bytes(), secret.as_bytes());
    }

    #[test]
    fn pairing_ids_are_22_base64url_characters() {
        assert!(valid_pairing_id(ID));
        assert!(!valid_pairing_id("AAAA"));
        assert!(!valid_pairing_id("AAAAAAAAAAAAAAAAAAAAA/"));
        assert!(!valid_pairing_id("../../../../../v1/sync"));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core pairing 2>&1 | tail -5`
Expected: compile errors (`PairingLink` not found).

- [ ] **Step 4: Implement `pairing.rs`** (above the tests)

```rust
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
    pub fn open(&self, server_url: &str, pairing_id: &str, envelope: &[u8]) -> Result<PairingPayload> {
        if envelope.len() > MAX_ENVELOPE_LEN || envelope.len() < HEADER_LEN + TAG_LEN {
            return Err(Error::Corrupted);
        }
        if envelope[0] != ENVELOPE_V1 || envelope[1..1 + SUITE.len()] != SUITE {
            return Err(Error::UnsupportedVersion);
        }
        let enc = <Kem as hpke::Kem>::EncappedKey::from_bytes(&envelope[1 + SUITE.len()..HEADER_LEN])
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
    fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        let key = self.secret_key.to_text();
        let key = key.expose().as_bytes();
        let email = self.email.as_bytes();
        if key.len() > MAX_SECRET_KEY_TEXT || email.is_empty() || email.len() > MAX_EMAIL_LEN {
            return Err(Error::InvalidInput("the account cannot be sent"));
        }
        let mut out = Zeroizing::new(Vec::with_capacity(1 + 16 + 16 + 32 + 1 + key.len() + 2 + email.len()));
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
        let email = std::str::from_utf8(take(email_len)?).map_err(|_| BAD)?.to_owned();
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
```

If `hpke::Kem::gen_keypair` in 0.14 requires an RNG argument, use `Kem::gen_keypair_with_rng(&mut rand::rngs::SysRng)` with the `rand 0.10` the core already depends on, and keep the rest unchanged. If `PairingPayload::decode`'s closure borrows `rest` mutably while the function still reads it, replace the closure with a small `struct Reader<'a>(&'a [u8])` with `fn take(&mut self, n) -> Result<&'a [u8]>`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p havenkeys-core pairing 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: `test result: ok. 9 passed`.

- [ ] **Step 6: Lint and commit**

```bash
cargo fmt -p havenkeys-core && cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/Cargo.toml Cargo.lock crates/havenkeys-core/src/lib.rs crates/havenkeys-core/src/pairing.rs
git commit -m "feat(core): pairing link, claim secret and HPKE envelope for signing in a new device"
```

---

### Task 2: Core vault — keep the vault key, seal from the session, sign in from a payload

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (`Session` struct at ~line 70, `create_account_vault` ~731, `open_session` ~826, new method)
- Modify: `crates/havenkeys-core/src/sync.rs` (new `prepare_paired_sign_in` after `prepare_sign_in`)

**Interfaces:**
- Consumes: `PairingLink`, `PairingPayload`, `pairing::seal` (Task 1)
- Produces:
  - `VaultService::seal_pairing(&self, link: &PairingLink, account: &AccountRecord, secret_key: SecretKey) -> Result<Vec<u8>>` (refuses `Locked` when locked)
  - `pub struct PairedSignIn { pub prepared: PreparedVault, pub account: AccountRef, pub secret_key: SecretKey }`
  - `pub fn prepare_paired_sign_in(header: &[u8], payload: PairingPayload) -> Result<PairedSignIn>` (in `sync.rs`)

- [ ] **Step 1: Write the failing tests**

In `vault.rs`'s test module, add (reuse the module's helpers that build an account vault; the test module already has one that creates an account vault, look for `create_account_vault(` in it and copy its setup into `account_vault()` below if no helper exists):

```rust
    #[test]
    fn an_unlocked_vault_seals_its_keys_for_a_new_device_and_a_locked_one_does_not() {
        let (mut v, record, prepared_header) = account_vault_with_header();
        let keys = crate::pairing::PairingKeys::generate();
        let link = crate::pairing::PairingLink {
            server_url: record.server_url.clone(),
            pairing_id: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            public_key: keys.public_key(),
        };
        let sk = SecretKey::generate().unwrap();
        let sealed = v.seal_pairing(&link, &record, sk).unwrap();
        let payload = keys.open(&link.server_url, &link.pairing_id, &sealed).unwrap();
        assert_eq!(payload.account_id, record.account_id);
        assert_eq!(payload.email, record.email);

        // The payload signs a new device in against the same header.
        let paired = crate::sync::prepare_paired_sign_in(&prepared_header, payload).unwrap();
        assert_eq!(paired.account.id, record.account_id);

        v.lock();
        assert_eq!(
            v.seal_pairing(&link, &record, SecretKey::generate().unwrap()).err(),
            Some(Error::Locked)
        );
    }
```

`account_vault_with_header()` returns an unlocked `VaultService` created with `create_account_vault`, its `AccountRecord`, and the header bytes from `encode_header_for(&prepared)` taken before creation. Write it in the test module from the existing account-vault test setup (`prepare_new_account_vault` → `encode_header_for` → `create_account_vault`).

In `sync.rs`'s test module add:

```rust
    #[test]
    fn a_paired_sign_in_refuses_a_header_for_another_vault_or_a_wrong_key() {
        let (prepared, header, record) = new_account_header();
        let good = || crate::pairing::PairingPayload {
            account_id: record.account_id,
            vault_id: prepared.vault_id(),
            email: record.email.clone(),
            secret_key: SecretKey::generate().unwrap(),
            vault_key: Key256::from_bytes(*prepared.vault_key.as_bytes()),
        };
        assert!(prepare_paired_sign_in(&header, good()).is_ok());

        let mut other_vault = good();
        other_vault.vault_id = Uuid::new_v4();
        assert_eq!(prepare_paired_sign_in(&header, other_vault).err(), Some(Error::Corrupted));

        let mut wrong_key = good();
        wrong_key.vault_key = Key256::from_bytes([1u8; 32]);
        assert_eq!(prepare_paired_sign_in(&header, wrong_key).err(), Some(Error::Corrupted));

        let mut bad_email = good();
        bad_email.email = "not an email".into();
        assert!(prepare_paired_sign_in(&header, bad_email).is_err());

        assert!(prepare_paired_sign_in(b"{}", good()).is_err());
    }
```

`new_account_header()` builds a `PreparedVault` with `prepare_new_account_vault` (look at its existing callers in `sync.rs` tests or `vault.rs` tests for the arguments), the header bytes with `encode_header_for`, and a matching `AccountRecord`. `PreparedVault::vault_id()` exists (vault.rs ~431); if it does not, read `prepared.header.vault_id`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p havenkeys-core pair 2>&1 | tail -5`
Expected: compile errors (`seal_pairing`, `prepare_paired_sign_in` not found).

- [ ] **Step 3: Keep the vault key in the session**

In `vault.rs`, `struct Session`, add after `data_key`:

```rust
    /// The vault key itself, for one purpose: sealing it to a new device the
    /// user approves (`seal_pairing`). Zeroized with the session on lock, like
    /// `data_key`, and never returned by any method.
    pub(crate) vault_key: Key256,
```

In `open_session`, add to the returned `Session`: `vault_key: Key256::from_bytes(*vault_key.as_bytes()),`.
In `create_account_vault`, build the session with `vault_key: prepared.vault_key,` moved in last (compute `data_key` and `identity_id` from `&prepared.vault_key` into locals first, then move it).

- [ ] **Step 4: Add `seal_pairing`** (in `impl VaultService`, next to `create_account_vault`)

```rust
    /// Seal this vault's keys for a new device the user approved by
    /// scanning its code (spec 2026-10-03-phone-approved-sign-in §4). Only
    /// while unlocked; the vault key never leaves the core.
    pub fn seal_pairing(
        &self,
        link: &crate::pairing::PairingLink,
        account: &AccountRecord,
        secret_key: SecretKey,
    ) -> Result<Vec<u8>> {
        let session = self.session()?;
        let payload = crate::pairing::PairingPayload {
            account_id: account.account_id,
            vault_id: session.vault_id,
            email: account.email.clone(),
            secret_key,
            vault_key: Key256::from_bytes(*session.vault_key.as_bytes()),
        };
        crate::pairing::seal(link, &payload)
    }
```

(`self.session()` already returns `Err(Error::Locked)` when locked; confirm by reading it. Import `SecretKey` from `crate::crypto::secret_key` if `vault.rs` does not yet.)

- [ ] **Step 5: Add `prepare_paired_sign_in`** (in `sync.rs`, right after `prepare_sign_in`)

```rust
/// What a device signing in from another device's approval needs.
pub struct PairedSignIn {
    pub prepared: PreparedVault,
    pub account: AccountRef,
    pub secret_key: SecretKey,
}

/// Sign in from an approving device's envelope instead of the master
/// password: the vault key comes from the payload, and the header the server
/// serves must be this vault's and carry an attestation that key made.
/// Refuses any key scheme below 3, as `prepare_sign_in` does.
pub fn prepare_paired_sign_in(
    header: &[u8],
    payload: crate::pairing::PairingPayload,
) -> Result<PairedSignIn> {
    let file = parse_header(header)?;
    if file.body.key_scheme != KeyScheme::AccountBound {
        return Err(Error::UnsupportedVersion);
    }
    let record = body_to_record(&file.body)?;
    if record.vault_id != payload.vault_id {
        return Err(Error::Corrupted);
    }
    if !verify_header(&derive_data_key(&payload.vault_key)?, &file) {
        return Err(Error::Corrupted);
    }
    let email = NormalizedEmail::parse(&payload.email)?;
    Ok(PairedSignIn {
        account: AccountRef::new(payload.account_id, email),
        secret_key: payload.secret_key,
        prepared: PreparedVault {
            header: record,
            vault_key: payload.vault_key,
        },
    })
}
```

Add any missing imports (`NormalizedEmail` from `crate::account`).

- [ ] **Step 6: Run the tests**

Run: `cargo test -p havenkeys-core 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: every result line `ok`.

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt -p havenkeys-core && cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/vault.rs crates/havenkeys-core/src/sync.rs
git commit -m "feat(core): an unlocked vault seals its keys for a device the user approved; sign in from that envelope"
```

---

### Task 3: Server — the pairings table and routes

**Files:**
- Create: `crates/havenkeys-server/migrations/0002_pairings.sql`
- Create: `crates/havenkeys-server/src/routes/pairings.rs`
- Create: `crates/havenkeys-server/tests/pairings.rs`
- Modify: `crates/havenkeys-server/src/db.rs` (`MIGRATIONS`)
- Modify: `crates/havenkeys-server/src/routes/mod.rs` (module + routes)
- Modify: `crates/havenkeys-server/src/routes/auth.rs` (`register_device`, `clean_device_name` become `pub(crate)` and generic over the client)
- Modify: `crates/havenkeys-server/src/auth/mod.rs` (`issue_token` generic over the client)
- Modify: `crates/havenkeys-server/src/routes/devices.rs` (list returns `approvedBy`)
- Modify: `crates/havenkeys-server/src/limits.rs` (constants)

**Interfaces:**
- Produces (HTTP, camelCase JSON):
  - `POST /v1/pairings` `{deviceId, deviceName, publicKey (b64url 32), claimHash (b64url 32)}` → `200 {pairingId, expiresAt}`; `429` over the limits
  - `GET /v1/pairings/{id}` (Bearer) → `200 {deviceName, ip, location|null, createdAt, expiresAt}`; `404` otherwise
  - `POST /v1/pairings/{id}/approve` (Bearer) `{envelope (b64)}` → `204`; `404` gone; `400` device cap
  - `POST /v1/pairings/{id}/deny` (Bearer) → `204`; `404` gone
  - `POST /v1/pairings/{id}/claim` `{claimSecret (b64url 32)}` → `200 {state:"waiting"} | {state:"denied"} | {state:"approved", token, expiresAt, accountId, vaultId, envelope}`; `404` gone or wrong secret; `429` blocked IP
  - Devices list items gain `"approvedBy": uuid|null`

- [ ] **Step 1: Write the migration**

`crates/havenkeys-server/migrations/0002_pairings.sql`:

```sql
-- Signing in a new device from an approving one
-- (docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md §5).
CREATE TABLE pairings (
  id               TEXT PRIMARY KEY,
  state            TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'denied', 'claimed')),
  device_id        UUID NOT NULL,
  device_name      TEXT NOT NULL,
  public_key       BYTEA NOT NULL CHECK (octet_length(public_key) = 32),
  claim_hash       BYTEA NOT NULL CHECK (octet_length(claim_hash) = 32),
  ip               TEXT NOT NULL,
  location         TEXT,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at       TIMESTAMPTZ NOT NULL,
  account_id       UUID REFERENCES accounts(id) ON DELETE CASCADE,
  envelope         BYTEA,
  token            TEXT,
  token_expires_at TIMESTAMPTZ
);
CREATE INDEX pairings_by_ip ON pairings (ip);
CREATE INDEX pairings_by_created ON pairings (created_at);

ALTER TABLE devices ADD COLUMN approved_by UUID;
```

In `db.rs`:

```rust
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../migrations/0001_init.sql")),
    ("0002_pairings", include_str!("../migrations/0002_pairings.sql")),
];
```

In `limits.rs` append:

```rust
/// A pairing's QR code is good for this long.
pub const PAIRING_TTL_SECONDS: f64 = 120.0;

/// Every pairing older than this is deleted when a new one is created.
pub const PAIRING_MAX_AGE_MINUTES: i32 = 10;

/// Pairings one address may create in `PAIRING_MAX_AGE_MINUTES`.
pub const PAIRINGS_PER_IP: i64 = 10;

/// Pairings one address may have waiting at once.
pub const PENDING_PAIRINGS_PER_IP: i64 = 3;

/// The largest envelope an approving device may send.
pub const MAX_PAIRING_ENVELOPE_BYTES: usize = 4096;
```

- [ ] **Step 2: Write the failing integration tests**

`crates/havenkeys-server/tests/pairings.rs`:

```rust
mod support;

use data_encoding::{BASE64, BASE64URL_NOPAD};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::{Sess, TestServer};
use uuid::Uuid;

const SECRET: [u8; 32] = [7u8; 32];

fn create_body(device_id: Uuid) -> Value {
    json!({
        "deviceId": device_id,
        "deviceName": "Desktop · Linux",
        "publicKey": BASE64URL_NOPAD.encode(&[3u8; 32]),
        "claimHash": BASE64URL_NOPAD.encode(&Sha256::digest(SECRET)),
    })
}

async fn create(server: &TestServer, device_id: Uuid) -> String {
    let res = server.post("/v1/pairings").json(&create_body(device_id)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["expiresAt"].is_string());
    body["pairingId"].as_str().unwrap().to_string()
}

async fn claim(server: &TestServer, id: &str, secret: [u8; 32]) -> (u16, Value) {
    let res = server
        .post(&format!("/v1/pairings/{id}/claim"))
        .json(&json!({ "claimSecret": BASE64URL_NOPAD.encode(&secret) }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn approve(server: &TestServer, id: &str, phone: &Sess) -> u16 {
    server
        .post_as(&format!("/v1/pairings/{id}/approve"), phone)
        .json(&json!({ "envelope": BASE64.encode(&[1u8; 200]) }))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn an_approved_pairing_is_claimed_once_with_a_working_session() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let desktop = Uuid::new_v4();
    let id = create(&server, desktop).await;

    assert_eq!(claim(&server, &id, SECRET).await.1["state"], "waiting");

    let details: Value = server
        .get_as(&format!("/v1/pairings/{id}"), &phone)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(details["deviceName"], "Desktop · Linux");
    assert_eq!(details["ip"], "127.0.0.1");
    assert!(details["location"].is_null());

    assert_eq!(approve(&server, &id, &phone).await, 204);
    let (status, body) = claim(&server, &id, SECRET).await;
    assert_eq!(status, 200);
    assert_eq!(body["state"], "approved");
    assert_eq!(body["accountId"], json!(phone.account_id));
    assert_eq!(body["vaultId"], json!(phone.vault_id));
    assert_eq!(BASE64.decode(body["envelope"].as_str().unwrap().as_bytes()).unwrap(), vec![1u8; 200]);

    // The token is the desktop's session, and the device says who approved it.
    let token = body["token"].as_str().unwrap();
    let devices: Value = server.get("/v1/devices").bearer_auth(token).send().await.unwrap().json().await.unwrap();
    let me = devices.as_array().unwrap().iter().find(|d| d["current"] == true).unwrap();
    assert_eq!(me["id"], json!(desktop));
    assert_eq!(me["approvedBy"], json!(phone.device_id));

    // Single use.
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn a_wrong_secret_claims_nothing_and_counts_against_the_address() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(approve(&server, &id, &phone).await, 204);
    for _ in 0..5 {
        assert_eq!(claim(&server, &id, [8u8; 32]).await.0, 404);
    }
    // Blocked now, even with the right secret.
    assert_eq!(claim(&server, &id, SECRET).await.0, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn another_account_cannot_see_approve_or_deny_a_pairing_bound_to_the_first() {
    let server = TestServer::start().await;
    let (_, ana) = support::signed_in(&server, "ana@example.com").await;
    let (_, bob) = support::signed_in(&server, "bob@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(server.get_as(&format!("/v1/pairings/{id}"), &ana).send().await.unwrap().status(), 200);
    assert_eq!(server.get_as(&format!("/v1/pairings/{id}"), &bob).send().await.unwrap().status(), 404);
    assert_eq!(approve(&server, &id, &bob).await, 404);
    assert_eq!(server.post_as(&format!("/v1/pairings/{id}/deny"), &bob).send().await.unwrap().status(), 404);
    assert_eq!(approve(&server, &id, &ana).await, 204);
    server.cleanup().await;
}

#[tokio::test]
async fn a_denied_pairing_says_so_once() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(server.post_as(&format!("/v1/pairings/{id}/deny"), &phone).send().await.unwrap().status(), 204);
    assert_eq!(approve(&server, &id, &phone).await, 404);
    assert_eq!(claim(&server, &id, SECRET).await.1["state"], "denied");
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_pairing_is_gone() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    server.db().await
        .execute("UPDATE pairings SET expires_at = now() - interval '1 second' WHERE id = $1", &[&id])
        .await.unwrap();
    assert_eq!(server.get_as(&format!("/v1/pairings/{id}"), &phone).send().await.unwrap().status(), 404);
    assert_eq!(approve(&server, &id, &phone).await, 404);
    assert_eq!(claim(&server, &id, SECRET).await.0, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn an_approved_pairing_never_claimed_is_gone_after_ten_minutes() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    let id = create(&server, Uuid::new_v4()).await;
    assert_eq!(approve(&server, &id, &phone).await, 204);
    server.db().await
        .execute("UPDATE pairings SET created_at = now() - interval '11 minutes' WHERE id = $1", &[&id])
        .await.unwrap();
    create(&server, Uuid::new_v4()).await;
    let left: i64 = server.db().await
        .query_one("SELECT count(*) FROM pairings WHERE id = $1", &[&id]).await.unwrap().get(0);
    assert_eq!(left, 0, "the envelope and token do not outlive the row's ten minutes");
    server.cleanup().await;
}

#[tokio::test]
async fn one_address_may_hold_three_pending_and_create_ten_per_window() {
    let server = TestServer::start().await;
    for _ in 0..3 {
        create(&server, Uuid::new_v4()).await;
    }
    let res = server.post("/v1/pairings").json(&create_body(Uuid::new_v4())).send().await.unwrap();
    assert_eq!(res.status(), 429);
    server.db().await.execute("UPDATE pairings SET state = 'claimed'", &[]).await.unwrap();
    for _ in 0..7 {
        create(&server, Uuid::new_v4()).await;
        server.db().await.execute("UPDATE pairings SET state = 'claimed'", &[]).await.unwrap();
    }
    let res = server.post("/v1/pairings").json(&create_body(Uuid::new_v4())).send().await.unwrap();
    assert_eq!(res.status(), 429);
    server.cleanup().await;
}

#[tokio::test]
async fn malformed_requests_are_refused() {
    let server = TestServer::start().await;
    let (_, phone) = support::signed_in(&server, "ana@example.com").await;
    for body in [
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "D", "publicKey": "AA", "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]) }),
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "", "publicKey": BASE64URL_NOPAD.encode(&[0u8; 32]), "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]) }),
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "D\u{7}", "publicKey": BASE64URL_NOPAD.encode(&[0u8; 32]), "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]) }),
        json!({ "deviceId": Uuid::new_v4(), "deviceName": "D", "publicKey": BASE64URL_NOPAD.encode(&[0u8; 32]), "claimHash": BASE64URL_NOPAD.encode(&[0u8; 32]), "accountId": Uuid::new_v4() }),
    ] {
        assert_eq!(server.post("/v1/pairings").json(&body).send().await.unwrap().status(), 400);
    }
    let id = create(&server, Uuid::new_v4()).await;
    let too_big = json!({ "envelope": BASE64.encode(&vec![0u8; 4097]) });
    assert_eq!(server.post_as(&format!("/v1/pairings/{id}/approve"), &phone).json(&too_big).send().await.unwrap().status(), 400);
    // The approving device cannot approve itself in.
    let own = create(&server, phone.device_id).await;
    assert_eq!(approve(&server, &own, &phone).await, 400);
    server.cleanup().await;
}

#[tokio::test]
async fn a_revoked_device_id_cannot_be_approved_in() {
    let server = TestServer::start().await;
    let (account, phone) = support::signed_in(&server, "ana@example.com").await;
    let old = support::login(&server, &account, "Old laptop").await;
    assert_eq!(server.delete_as(&format!("/v1/devices/{}", old.device_id), &phone).send().await.unwrap().status(), 204);
    let id = create(&server, old.device_id).await;
    assert_eq!(approve(&server, &id, &phone).await, 401);
    server.cleanup().await;
}
```

Add `sha2 = "0.10"` to the server's `[dev-dependencies]` only if the tests cannot use the existing `sha2` dependency (they can: `sha2` is already a normal dependency, visible to integration tests).

- [ ] **Step 3: Run them to verify they fail**

Run: `scripts/test-server.sh --test pairings 2>&1 | grep -E "test result|FAILED" | head`
Expected: failures with status 404/405 (routes missing).

- [ ] **Step 4: Make the shared helpers reusable**

In `routes/auth.rs`:
- `fn clean_device_name` → `pub(crate) fn clean_device_name`.
- `async fn register_device(db: &Object, …)` → `pub(crate) async fn register_device(db: &impl deadpool_postgres::GenericClient, …)`; its body is unchanged.

In `auth/mod.rs`:
- `pub async fn issue_token(db: &Object, …)` → `pub async fn issue_token(db: &impl deadpool_postgres::GenericClient, …)`.

Run: `cargo check -p havenkeys-server` (callers passing `&Object` still compile).

- [ ] **Step 5: Implement `routes/pairings.rs`**

```rust
//! Signing in a new device from an approving one
//! (spec 2026-10-03-phone-approved-sign-in §3, §5).
//!
//! The server relays an envelope it cannot open and issues the new device's
//! session when a signed-in device of the same account approves. Unknown,
//! expired, used, denied and other-account pairings all answer 404.

use crate::auth::{self, rate_limit, Session};
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::{
    MAX_PAIRING_ENVELOPE_BYTES, PAIRINGS_PER_IP, PAIRING_MAX_AGE_MINUTES, PAIRING_TTL_SECONDS,
    PENDING_PAIRINGS_PER_IP,
};
use crate::routes::auth::{clean_device_name, client_ip, register_device};
use crate::routes::AppState;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use data_encoding::{BASE64, BASE64URL_NOPAD};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRequest {
    device_id: Uuid,
    device_name: String,
    public_key: String,
    claim_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApproveRequest {
    envelope: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimRequest {
    claim_secret: String,
}

fn fixed32(raw: &str, bad: &'static str) -> Result<[u8; 32], ApiError> {
    if raw.len() > 64 {
        return Err(ApiError::InvalidRequest(bad));
    }
    BASE64URL_NOPAD
        .decode(raw.as_bytes())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(ApiError::InvalidRequest(bad))
}

/// 16 random bytes; a path segment that is anything else is simply gone.
fn pairing_id(raw: &str) -> Result<String, ApiError> {
    let ok = raw.len() == 22
        && BASE64URL_NOPAD
            .decode(raw.as_bytes())
            .is_ok_and(|b| b.len() == 16);
    if ok {
        Ok(raw.to_string())
    } else {
        Err(ApiError::NotFound)
    }
}

pub async fn create(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<CreateRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let device_name = clean_device_name(&req.device_name)?;
    let public_key = fixed32(&req.public_key, "publicKey is not valid")?;
    let claim_hash = fixed32(&req.claim_hash, "claimHash is not valid")?;
    let ip = client_ip(&state, &headers, peer);
    let db = state.pool.get().await?;

    // No periodic task: old rows go here, whatever their state.
    db.execute(
        "DELETE FROM pairings WHERE created_at < now() - make_interval(mins => $1)",
        &[&PAIRING_MAX_AGE_MINUTES],
    )
    .await?;
    let counts = db
        .query_one(
            "SELECT count(*),
                    count(*) FILTER (WHERE state = 'pending' AND expires_at > now())
               FROM pairings WHERE ip = $1",
            &[&ip],
        )
        .await?;
    let (total, pending): (i64, i64) = (counts.get(0), counts.get(1));
    if total >= PAIRINGS_PER_IP || pending >= PENDING_PAIRINGS_PER_IP {
        return Err(ApiError::RateLimited);
    }

    let mut raw = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut raw);
    let id = BASE64URL_NOPAD.encode(&raw);
    let location = state.locator.as_ref().and_then(|l| l.locate(&ip));
    let row = db
        .query_one(
            "INSERT INTO pairings
               (id, state, device_id, device_name, public_key, claim_hash, ip, location, expires_at)
             VALUES ($1, 'pending', $2, $3, $4, $5, $6, $7, now() + make_interval(secs => $8))
             RETURNING expires_at",
            &[
                &id,
                &req.device_id,
                &device_name,
                &public_key.as_slice(),
                &claim_hash.as_slice(),
                &ip,
                &location,
                &PAIRING_TTL_SECONDS,
            ],
        )
        .await?;
    let expires: chrono::DateTime<chrono::Utc> = row.get(0);
    tracing::info!(outcome = "created", "pairing");
    Ok(axum::Json(serde_json::json!({
        "pairingId": id,
        "expiresAt": expires.to_rfc3339(),
    })))
}

pub async fn details(
    State(state): State<AppState>,
    session: Session,
    Path(raw): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let id = pairing_id(&raw)?;
    let db = state.pool.get().await?;
    // Reading binds the pairing to this account: no other account can see,
    // approve or deny it afterwards.
    let row = db
        .query_opt(
            "UPDATE pairings SET account_id = $2
              WHERE id = $1 AND state = 'pending' AND expires_at > now()
                AND (account_id IS NULL OR account_id = $2)
             RETURNING device_name, ip, location, created_at, expires_at",
            &[&id, &session.account_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let created: chrono::DateTime<chrono::Utc> = row.get(3);
    let expires: chrono::DateTime<chrono::Utc> = row.get(4);
    Ok(axum::Json(serde_json::json!({
        "deviceName": row.get::<_, String>(0),
        "ip": row.get::<_, String>(1),
        "location": row.get::<_, Option<String>>(2),
        "createdAt": created.to_rfc3339(),
        "expiresAt": expires.to_rfc3339(),
    })))
}

pub async fn approve(
    State(state): State<AppState>,
    session: Session,
    Path(raw): Path<String>,
    Json(req): Json<ApproveRequest>,
) -> Result<StatusCode, ApiError> {
    let id = pairing_id(&raw)?;
    const BAD: ApiError = ApiError::InvalidRequest("envelope is not valid");
    if req.envelope.len() > MAX_PAIRING_ENVELOPE_BYTES / 3 * 4 + 4 {
        return Err(BAD);
    }
    let envelope = BASE64.decode(req.envelope.as_bytes()).map_err(|_| BAD)?;
    if envelope.is_empty() || envelope.len() > MAX_PAIRING_ENVELOPE_BYTES {
        return Err(BAD);
    }

    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT device_id, device_name FROM pairings
              WHERE id = $1 AND state = 'pending' AND expires_at > now()
                AND (account_id IS NULL OR account_id = $2)
              FOR UPDATE",
            &[&id, &session.account_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let device_id: Uuid = row.get(0);
    let device_name: String = row.get(1);
    if device_id == session.device_id {
        return Err(ApiError::InvalidRequest("a device cannot approve itself"));
    }
    register_device(&tx, session.account_id, device_id, &device_name).await?;
    tx.execute(
        "UPDATE devices SET approved_by = $2 WHERE id = $1",
        &[&device_id, &session.device_id],
    )
    .await?;
    tx.execute("DELETE FROM sessions WHERE device_id = $1", &[&device_id])
        .await?;
    let (token, expires) = auth::issue_token(&tx, session.account_id, device_id).await?;
    tx.execute(
        "UPDATE pairings
            SET state = 'approved', account_id = $2, envelope = $3,
                token = $4, token_expires_at = $5
          WHERE id = $1",
        &[&id, &session.account_id, &envelope, &token.as_str(), &expires],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(account_id = %session.account_id, device_id = %device_id, outcome = "approved", "pairing");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn deny(
    State(state): State<AppState>,
    session: Session,
    Path(raw): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = pairing_id(&raw)?;
    let db = state.pool.get().await?;
    let changed = db
        .execute(
            "UPDATE pairings SET state = 'denied', account_id = $2
              WHERE id = $1 AND state = 'pending' AND expires_at > now()
                AND (account_id IS NULL OR account_id = $2)",
            &[&id, &session.account_id],
        )
        .await?;
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    tracing::info!(account_id = %session.account_id, outcome = "denied", "pairing");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn claim(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(raw): Path<String>,
    Json(req): Json<ClaimRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let id = pairing_id(&raw)?;
    let secret = fixed32(&req.claim_secret, "claimSecret is not valid")?;
    let ip_key = rate_limit::ip_key(&client_ip(&state, &headers, peer));
    let db = state.pool.get().await?;
    rate_limit::check(&db, &ip_key).await?;

    let row = db
        .query_opt(
            "SELECT state, claim_hash, expires_at > now(), account_id, envelope, token, token_expires_at
               FROM pairings WHERE id = $1",
            &[&id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let stored: Vec<u8> = row.get(1);
    let given = Sha256::digest(secret);
    if !bool::from(stored.as_slice().ct_eq(given.as_slice())) {
        rate_limit::record_failure(&db, &ip_key).await?;
        return Err(ApiError::NotFound);
    }
    let live: bool = row.get(2);
    match row.get::<_, String>(0).as_str() {
        "pending" if live => Ok(axum::Json(serde_json::json!({ "state": "waiting" }))),
        "denied" => {
            db.execute("UPDATE pairings SET state = 'claimed' WHERE id = $1", &[&id])
                .await?;
            Ok(axum::Json(serde_json::json!({ "state": "denied" })))
        }
        "approved" => {
            let account_id: Uuid = row.get(3);
            let envelope: Vec<u8> = row.get(4);
            let token: String = row.get(5);
            let expires: chrono::DateTime<chrono::Utc> = row.get(6);
            let vault_id: Uuid = db
                .query_one("SELECT id FROM vaults WHERE account_id = $1", &[&account_id])
                .await?
                .get(0);
            // Single use: the token and envelope leave the database here.
            db.execute(
                "UPDATE pairings SET state = 'claimed', envelope = NULL, token = NULL
                  WHERE id = $1 AND state = 'approved'",
                &[&id],
            )
            .await?;
            tracing::info!(account_id = %account_id, outcome = "claimed", "pairing");
            Ok(axum::Json(serde_json::json!({
                "state": "approved",
                "token": token,
                "expiresAt": expires.to_rfc3339(),
                "accountId": account_id,
                "vaultId": vault_id,
                "envelope": BASE64.encode(&envelope),
            })))
        }
        _ => Err(ApiError::NotFound),
    }
}
```

`AppState.locator` arrives in Task 4. For this task, add the field now as `pub locator: Option<std::sync::Arc<crate::locate::Locator>>` together with an empty `locate.rs` holding `pub struct Locator; impl Locator { pub fn locate(&self, _ip: &str) -> Option<String> { None } }`, and set `locator: None` in every `AppState { … }` literal:
`crates/havenkeys-server/src/main.rs`, `crates/havenkeys-server/tests/support/mod.rs`, `crates/havenkeys-client/tests/round_trip.rs`, `crates/havenkeys-sync-client/tests/round_trip.rs`, `crates/havenkeys-mobile/tests/round_trip.rs`. Add `pub mod locate;` to `crates/havenkeys-server/src/lib.rs`.

In `routes/mod.rs`, add `pub mod pairings;` and, after the devices routes:

```rust
        .route("/v1/pairings", post(pairings::create))
        .route("/v1/pairings/{id}", get(pairings::details))
        .route("/v1/pairings/{id}/approve", post(pairings::approve))
        .route("/v1/pairings/{id}/deny", post(pairings::deny))
        .route("/v1/pairings/{id}/claim", post(pairings::claim))
```

In `routes/devices.rs` `list`, select `approved_by` as the fifth column and add `"approvedBy": row.get::<_, Option<Uuid>>(4),` to each item.

- [ ] **Step 6: Run the tests**

Run: `scripts/test-server.sh 2>&1 | grep -E "test result|FAILED|panicked" | head -30`
Expected: every result `ok`, including the 9 new pairing tests and the existing `no_logging` test (the new `tracing::info!` lines carry no secret).

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt --all && cargo clippy -p havenkeys-server --all-targets -- -D warnings
git add crates/havenkeys-server crates/havenkeys-client/tests/round_trip.rs crates/havenkeys-sync-client/tests/round_trip.rs crates/havenkeys-mobile/tests/round_trip.rs
git commit -m "feat(server): pairings: create, details, approve, deny and single-use claim for signing in a new device"
```

---

### Task 4: Server — optional location from a local IP database

**Files:**
- Modify: `crates/havenkeys-server/src/locate.rs` (replace the Task 3 stub)
- Modify: `crates/havenkeys-server/Cargo.toml`, `src/config.rs`, `src/main.rs`
- Modify: `docs/` deployment notes for the server (find with `grep -rln "HAVENKEYS_TRUST_FORWARDED_FOR" docs README.md`)

**Interfaces:**
- Produces: `Locator::open(path: &Path) -> Result<Locator, String>`, `Locator::locate(&self, ip: &str) -> Option<String>` ("City, CC", or "CC", or `None`); `Config.geoip_database: Option<PathBuf>` from `HAVENKEYS_GEOIP_DATABASE`.

- [ ] **Step 1: Write the failing tests** (in `locate.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_database_is_an_error_without_the_path_in_it() {
        let err = Locator::open(Path::new("/nonexistent/secret-dir/db.mmdb")).unwrap_err();
        assert!(!err.contains("secret-dir"));
    }

    #[test]
    fn the_label_prefers_city_and_country_then_country() {
        assert_eq!(label(Some("São Paulo"), Some("BR")).as_deref(), Some("São Paulo, BR"));
        assert_eq!(label(None, Some("BR")).as_deref(), Some("BR"));
        assert_eq!(label(Some("Nowhere"), None), None);
        assert_eq!(label(Some("A\u{7}"), Some("BR")).as_deref(), Some("BR"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-server --lib locate 2>&1 | tail -3`
Expected: compile errors (`open`, `label` not found).

- [ ] **Step 3: Implement**

`Cargo.toml` `[dependencies]`: `maxminddb = "0.32"   # ISC; local IP → city lookups, no network`.

`locate.rs`:

```rust
//! Where a request came from, roughly, for the pairing confirmation
//! (spec 2026-10-03-phone-approved-sign-in §5.3). Read from a local MaxMind
//! format database (DB-IP Lite or GeoLite2) the operator downloads; no third
//! party is ever called. Without one, nothing is located.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::Path;

pub struct Locator(maxminddb::Reader<Vec<u8>>);

#[derive(Deserialize)]
struct Record {
    #[serde(default)]
    city: Option<Named>,
    #[serde(default)]
    country: Option<Country>,
}

#[derive(Deserialize)]
struct Named {
    #[serde(default)]
    names: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Country {
    #[serde(default)]
    iso_code: Option<String>,
}

impl Locator {
    pub fn open(path: &Path) -> Result<Self, String> {
        maxminddb::Reader::open_readfile(path)
            .map(Self)
            .map_err(|_| "the IP location database could not be read".to_string())
    }

    pub fn locate(&self, ip: &str) -> Option<String> {
        let addr: IpAddr = ip.parse().ok()?;
        let record: Record = self.0.lookup(addr).ok()?.decode().ok()??;
        let city = record.city.and_then(|c| c.names.get("en").cloned());
        let country = record.country.and_then(|c| c.iso_code);
        label(city.as_deref(), country.as_deref())
    }
}

/// "City, CC", or "CC". Text from the database is checked like any input:
/// a value with control characters or of absurd length is dropped.
fn label(city: Option<&str>, country: Option<&str>) -> Option<String> {
    let clean = |s: &str| (!s.is_empty() && s.chars().count() <= 64 && !s.chars().any(char::is_control)).then(|| s.to_string());
    let country = clean(country?)?;
    match city.and_then(clean) {
        Some(city) => Some(format!("{city}, {country}")),
        None => Some(country),
    }
}
```

`config.rs`: add `pub geoip_database: Option<std::path::PathBuf>,` documented as "A MaxMind-format city database for the pairing confirmation's location. Optional; without it the phone sees the IP only." Set it from `std::env::var("HAVENKEYS_GEOIP_DATABASE").ok().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).map(Into::into)`.

`main.rs`, where `AppState` is built:

```rust
        locator: match &config.geoip_database {
            Some(path) => match havenkeys_server::locate::Locator::open(path) {
                Ok(locator) => Some(std::sync::Arc::new(locator)),
                Err(why) => {
                    tracing::warn!(reason = why.as_str(), "IP location disabled");
                    None
                }
            },
            None => None,
        },
```

(Use the crate path `main.rs` already uses for its other modules.)

Document `HAVENKEYS_GEOIP_DATABASE` next to `HAVENKEYS_TRUST_FORWARDED_FOR` in the server's deployment docs: what it is, that DB-IP "IP to City Lite" (CC BY 4.0, attribution required in the deployment) or MaxMind GeoLite2 City works, and that it is optional.

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-server --lib 2>&1 | grep "test result"` and `scripts/test-server.sh --test pairings 2>&1 | grep "test result"`
Expected: `ok` for both.

- [ ] **Step 5: Lint and commit**

```bash
cargo fmt --all && cargo clippy -p havenkeys-server --all-targets -- -D warnings && cargo deny check 2>&1 | tail -3
git add crates/havenkeys-server Cargo.lock docs README.md
git commit -m "feat(server): optional local IP location for the pairing confirmation (HAVENKEYS_GEOIP_DATABASE)"
```

---

### Task 5: Sync-client — typed pairing calls

**Files:**
- Modify: `crates/havenkeys-sync-client/src/wire.rs`, `src/client.rs`, `src/lib.rs` (re-exports if the crate re-exports client types)

**Interfaces:**
- Consumes: the HTTP API of Task 3.
- Produces (on `SyncClient<T>`):
  - `create_pairing(&self, device_id: Uuid, device_name: &str, public_key: &[u8; 32], claim_hash: &[u8; 32]) -> Result<CreatedPairing>`; `pub struct CreatedPairing { pub pairing_id: String, pub expires_at: String }`
  - `pairing_details(&self, session: &Session, pairing_id: &str) -> Result<PairingDetails>`; `pub struct PairingDetails { pub device_name: String, pub ip: String, pub location: Option<String>, pub created_at: String, pub expires_at: String }`
  - `approve_pairing(&self, session: &Session, pairing_id: &str, envelope: &[u8]) -> Result<()>`
  - `deny_pairing(&self, session: &Session, pairing_id: &str) -> Result<()>`
  - `claim_pairing(&self, pairing_id: &str, claim_secret: &[u8; 32]) -> Result<Claim>`; `pub enum Claim { Waiting, Denied, Approved { session: Session, envelope: Vec<u8> } }`
  - `Device` (the devices list) gains `pub approved_by: Option<Uuid>`
  - A pairing that is gone maps to `SyncError::Refused("not found")` (existing 404 mapping).

- [ ] **Step 1: Write the failing tests** (in `wire.rs` test module; create `#[cfg(test)] mod tests` if absent)

```rust
#[cfg(test)]
mod pairing_tests {
    use super::*;

    #[test]
    fn a_claim_answer_parses_each_state_and_refuses_unknown_ones() {
        let waiting: ClaimDto = parse(br#"{"state":"waiting"}"#).unwrap();
        assert!(matches!(waiting, ClaimDto::Waiting));
        let denied: ClaimDto = parse(br#"{"state":"denied"}"#).unwrap();
        assert!(matches!(denied, ClaimDto::Denied));
        let approved: ClaimDto = parse(
            br#"{"state":"approved","token":"t","expiresAt":"2099-01-01T00:00:00Z",
                 "accountId":"00000000-0000-0000-0000-000000000001",
                 "vaultId":"00000000-0000-0000-0000-000000000002","envelope":"AQID"}"#,
        )
        .unwrap();
        assert!(matches!(approved, ClaimDto::Approved { .. }));
        assert!(parse::<ClaimDto>(br#"{"state":"approved"}"#).is_err());
        assert!(parse::<ClaimDto>(br#"{"state":"granted"}"#).is_err());
    }
}
```

In `client.rs` add a test module:

```rust
#[cfg(test)]
mod pairing_tests {
    use super::*;

    #[test]
    fn a_pairing_id_that_could_change_the_path_is_refused_before_sending() {
        assert!(pairing_path("AAAAAAAAAAAAAAAAAAAAAA", "claim").is_ok());
        for bad in ["", "../devices", "AAAAAAAAAAAAAAAAAAAAA/", "A".repeat(23).as_str()] {
            assert!(pairing_path(bad, "claim").is_err());
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-sync-client pairing 2>&1 | tail -3`
Expected: compile errors.

- [ ] **Step 3: Implement the DTOs** (`wire.rs`)

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePairingBody<'a> {
    pub device_id: Uuid,
    pub device_name: &'a str,
    pub public_key: String,
    pub claim_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedPairingDto {
    pub pairing_id: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingDetailsDto {
    pub device_name: String,
    pub ip: String,
    #[serde(default)]
    pub location: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovePairingBody {
    pub envelope: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimPairingBody {
    pub claim_secret: String,
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ClaimDto {
    Waiting,
    Denied,
    #[serde(rename_all = "camelCase")]
    Approved {
        token: String,
        expires_at: String,
        account_id: Uuid,
        vault_id: Uuid,
        envelope: String,
    },
}
```

(Match the existing `use serde::{Deserialize, Serialize}` imports at the top of `wire.rs`.)

- [ ] **Step 4: Implement the calls** (`client.rs`, inside `impl<T: Transport> SyncClient<T>` before the private `get`)

```rust
    /// Open a pairing request for this (new) device. No session.
    pub async fn create_pairing(
        &self,
        device_id: Uuid,
        device_name: &str,
        public_key: &[u8; 32],
        claim_hash: &[u8; 32],
    ) -> Result<CreatedPairing> {
        let body = wire::CreatePairingBody {
            device_id,
            device_name,
            public_key: BASE64URL_NOPAD.encode(public_key),
            claim_hash: BASE64URL_NOPAD.encode(claim_hash),
        };
        let dto: wire::CreatedPairingDto = expect_ok(self.post("/v1/pairings", None, &body).await?)?;
        if !havenkeys_core::pairing::valid_pairing_id(&dto.pairing_id) || dto.expires_at.len() > 64 {
            return Err(SyncError::Protocol("pairing"));
        }
        Ok(CreatedPairing {
            pairing_id: dto.pairing_id,
            expires_at: dto.expires_at,
        })
    }

    /// What the server says about a pairing. Shown to the user, never trusted
    /// for anything else.
    pub async fn pairing_details(&self, session: &Session, pairing_id: &str) -> Result<PairingDetails> {
        let path = pairing_path(pairing_id, "")?;
        let dto: wire::PairingDetailsDto = expect_ok(self.get(&path, Some(session)).await?)?;
        let short = |s: &str| s.chars().count() <= 64;
        if !short(&dto.device_name)
            || !short(&dto.ip)
            || !dto.location.as_deref().is_none_or(short)
            || dto.created_at.len() > 64
            || dto.expires_at.len() > 64
        {
            return Err(SyncError::Protocol("pairing"));
        }
        Ok(PairingDetails {
            device_name: dto.device_name,
            ip: dto.ip,
            location: dto.location,
            created_at: dto.created_at,
            expires_at: dto.expires_at,
        })
    }

    pub async fn approve_pairing(&self, session: &Session, pairing_id: &str, envelope: &[u8]) -> Result<()> {
        let path = pairing_path(pairing_id, "/approve")?;
        let body = wire::ApprovePairingBody {
            envelope: BASE64.encode(envelope),
        };
        let response = self.post(&path, Some(session), &body).await?;
        match response.status {
            200 | 204 => Ok(()),
            _ => Err(error_for(response.status)),
        }
    }

    pub async fn deny_pairing(&self, session: &Session, pairing_id: &str) -> Result<()> {
        let path = pairing_path(pairing_id, "/deny")?;
        let response = self.send(Method::Post, &path, Some(session), None).await?;
        match response.status {
            200 | 204 => Ok(()),
            _ => Err(error_for(response.status)),
        }
    }

    /// Ask whether the pairing was approved. The session comes from here,
    /// but the caller checks its account and vault against the envelope.
    pub async fn claim_pairing(&self, pairing_id: &str, claim_secret: &[u8; 32]) -> Result<Claim> {
        let path = pairing_path(pairing_id, "/claim")?;
        let body = wire::ClaimPairingBody {
            claim_secret: BASE64URL_NOPAD.encode(claim_secret),
        };
        let dto: wire::ClaimDto = expect_ok(self.post(&path, None, &body).await?)?;
        Ok(match dto {
            wire::ClaimDto::Waiting => Claim::Waiting,
            wire::ClaimDto::Denied => Claim::Denied,
            wire::ClaimDto::Approved { token, expires_at, account_id, vault_id, envelope } => {
                if token.is_empty() || token.len() > 128 || expires_at.len() > 64 {
                    return Err(SyncError::Protocol("token"));
                }
                if envelope.len() > havenkeys_core::pairing::MAX_ENVELOPE_LEN / 3 * 4 + 4 {
                    return Err(SyncError::TooLarge);
                }
                let envelope = BASE64
                    .decode(envelope.as_bytes())
                    .map_err(|_| SyncError::Protocol("envelope"))?;
                Claim::Approved {
                    session: Session::new(token, expires_at, account_id, vault_id),
                    envelope,
                }
            }
        })
    }
```

At module level:

```rust
pub struct CreatedPairing {
    pub pairing_id: String,
    pub expires_at: String,
}

#[derive(Clone, Debug)]
pub struct PairingDetails {
    pub device_name: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

pub enum Claim {
    Waiting,
    Denied,
    Approved { session: Session, envelope: Vec<u8> },
}

impl std::fmt::Debug for Claim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Claim::Waiting => "Claim::Waiting",
            Claim::Denied => "Claim::Denied",
            Claim::Approved { .. } => "Claim::Approved(<redacted>)",
        })
    }
}

/// `/v1/pairings/{id}{suffix}`, only for an id that cannot change the path.
fn pairing_path(pairing_id: &str, suffix: &str) -> Result<String> {
    if !havenkeys_core::pairing::valid_pairing_id(pairing_id) {
        return Err(SyncError::Refused("not found"));
    }
    Ok(format!("/v1/pairings/{pairing_id}{suffix}"))
}
```

Add `use data_encoding::BASE64URL_NOPAD;` next to the existing `BASE64` import.

- [ ] **Step 4b: Carry `approvedBy` through the devices list**

In `wire.rs` `DeviceDto` add `#[serde(default)] pub approved_by: Option<Uuid>,`; in `client.rs`'s `Device` struct add `pub approved_by: Option<Uuid>,` and set it in `devices()`. Then follow the field outward so every layer compiles and carries it:
- `crates/havenkeys-client/src/account.rs` `DeviceEntry` gains `pub approved_by: Option<Uuid>` (set in `list_devices`);
- `crates/havenkeys-mobile/src/account.rs` `DeviceInfo` gains `pub approved_by: Option<String>` (`d.approved_by.map(|u| u.to_string())`);
- the desktop's `DeviceEntry` serialisation needs nothing more (serde), and `apps/desktop/src/lib/types.ts`'s device type gains `approvedBy?: string | null`.
The two screens use it in Tasks 7 and 9.

- [ ] **Step 5: Run tests and lint**

Run: `cargo test -p havenkeys-sync-client --lib 2>&1 | grep "test result"` then `cargo clippy -p havenkeys-sync-client --all-targets -- -D warnings`
Expected: `ok`, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-sync-client
git commit -m "feat(sync-client): typed pairing calls with path-safe ids and bounded answers"
```

---

### Task 6: Client — the desktop's and the phone's sides

**Files:**
- Create: `crates/havenkeys-client/src/pairing.rs`
- Modify: `crates/havenkeys-client/src/lib.rs` (`mod pairing;` and `pub use pairing::{PairingPoll, PairingRequest, PairingStart};`)
- Modify: `crates/havenkeys-client/src/client.rs` (field `pub(crate) pairing: Mutex<Option<PendingPairing>>`, initialised `Mutex::new(None)`; `lock()` leaves it alone — a locked vault cannot exist during pairing)
- Modify: `crates/havenkeys-client/src/account.rs` (extract the tail of `sign_in` into `go_online_after_sign_in`; make `new_account_record` `pub(crate)`)
- Modify: `crates/havenkeys-client/src/error.rs` (three constructors)
- Create: `crates/havenkeys-client/tests/pairing.rs`

**Interfaces:**
- Consumes: Tasks 1, 2, 5.
- Produces (on `HavenClient`):
  - `pub async fn start_pairing(&self, server_url: String, device_name: &str) -> ClientResult<PairingStart>`; `pub struct PairingStart { pub link: String, pub expires_at: String }`
  - `pub async fn poll_pairing(self: &Arc<Self>) -> ClientResult<PairingPoll>`; `pub enum PairingPoll { Waiting, Denied, Expired, Approved(VaultStatus) }`
  - `pub fn cancel_pairing(&self)`
  - `pub async fn pairing_request(&self, link: &str) -> ClientResult<PairingRequest>`; `pub struct PairingRequest { pub link: String, pub device_name: String, pub ip: String, pub location: Option<String>, pub created_at: String }`
  - `pub async fn approve_pairing(&self, link: &str) -> ClientResult<()>`
  - `pub async fn deny_pairing(&self, link: &str) -> ClientResult<()>`
  - `ClientError::pairing_gone()`, `pairing_other_server()`, `pairing_failed()` with codes `pairing_gone`, `pairing_other_server`, `pairing_failed`

- [ ] **Step 1: Write the failing integration tests**

`crates/havenkeys-client/tests/pairing.rs` — copy the `Server` struct, `exec`, and the client-construction helper from `tests/round_trip.rs` (the `Server::start`, `invite`, `cleanup` functions and whatever helper builds a `HavenClient` in a temp dir with `MemoryKeyStore`), then:

```rust
#[tokio::test]
async fn a_phone_signs_a_new_desktop_in_without_the_password() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    let invite = server.invite("ana@example.com").await;
    phone.activate(invite, SecretString::from(PASSWORD)).await.unwrap();
    until(|| phone.is_online()).await;
    let item = phone.vault().unwrap().stage_create(login("GitHub"), havenkeys_client::now_ms()).unwrap();
    phone.push(item).await.unwrap();

    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop.start_pairing(format!("{}/", server.base), "Desktop · Linux").await.unwrap();
    assert!(matches!(desktop.poll_pairing().await.unwrap(), havenkeys_client::PairingPoll::Waiting));

    let request = phone.pairing_request(&start.link).await.unwrap();
    assert_eq!(request.device_name, "Desktop · Linux");
    phone.approve_pairing(&start.link).await.unwrap();

    let status = match desktop.poll_pairing().await.unwrap() {
        havenkeys_client::PairingPoll::Approved(status) => status,
        other => panic!("not approved: {other:?}"),
    };
    assert!(status.unlocked);
    until(|| desktop.is_online()).await;
    desktop.sync_now().await.unwrap();
    let titles: Vec<String> = desktop.vault().unwrap().list_items().unwrap().iter().map(|o| o.title.clone()).collect();
    assert!(titles.contains(&"GitHub".to_string()));

    // From now on an ordinary device: the master password unlocks it.
    desktop.lock("user");
    desktop.unlock(SecretString::from(PASSWORD), None).await.unwrap();
    assert!(desktop.require_unlocked().is_ok());

    // Approving the same code again finds nothing.
    assert_eq!(phone.approve_pairing(&start.link).await.unwrap_err().code, "pairing_gone");
    server.cleanup().await;
}

#[tokio::test]
async fn a_denied_or_expired_pairing_leaves_the_desktop_empty() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone.activate(server.invite("ana@example.com").await, SecretString::from(PASSWORD)).await.unwrap();
    until(|| phone.is_online()).await;

    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop.start_pairing(server.base.clone(), "Desktop").await.unwrap();
    phone.deny_pairing(&start.link).await.unwrap();
    assert!(matches!(desktop.poll_pairing().await.unwrap(), havenkeys_client::PairingPoll::Denied));
    assert!(desktop.vault().unwrap().account().unwrap().is_none());

    let start = desktop.start_pairing(server.base.clone(), "Desktop").await.unwrap();
    let id = havenkeys_core::pairing::PairingLink::parse(&start.link).unwrap().pairing_id;
    exec_on(&server, &format!("UPDATE pairings SET expires_at = now() - interval '1 second' WHERE id = '{id}'")).await;
    assert!(matches!(desktop.poll_pairing().await.unwrap(), havenkeys_client::PairingPoll::Expired));
    assert_eq!(phone.approve_pairing(&start.link).await.unwrap_err().code, "pairing_gone");
    assert!(desktop.vault().unwrap().account().unwrap().is_none());
    server.cleanup().await;
}

#[tokio::test]
async fn a_code_for_another_server_is_refused_by_the_phone() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone.activate(server.invite("ana@example.com").await, SecretString::from(PASSWORD)).await.unwrap();
    until(|| phone.is_online()).await;
    let keys = havenkeys_core::pairing::PairingKeys::generate();
    let link = havenkeys_core::pairing::PairingLink {
        server_url: "https://elsewhere.example.com".into(),
        pairing_id: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        public_key: keys.public_key(),
    }
    .to_text();
    assert_eq!(phone.pairing_request(&link).await.unwrap_err().code, "pairing_other_server");
    assert_eq!(phone.approve_pairing(&link).await.unwrap_err().code, "pairing_other_server");
    server.cleanup().await;
}

#[tokio::test]
async fn a_trailing_slash_on_the_desktop_still_matches_the_phone() {
    // Covered by `a_phone_signs_a_new_desktop_in_without_the_password`, which
    // starts with `format!("{}/", server.base)`; this test pins the locked case.
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone.activate(server.invite("ana@example.com").await, SecretString::from(PASSWORD)).await.unwrap();
    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop.start_pairing(format!(" {}/ ", server.base), "Desktop").await.unwrap();
    phone.lock("user");
    assert_eq!(phone.approve_pairing(&start.link).await.unwrap_err().code, "locked");
    server.cleanup().await;
}

#[tokio::test]
async fn pairing_refuses_a_device_that_already_has_a_vault() {
    let server = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let phone = client_in(dir.path());
    phone.activate(server.invite("ana@example.com").await, SecretString::from(PASSWORD)).await.unwrap();
    assert_eq!(
        phone.start_pairing(server.base.clone(), "Desktop").await.unwrap_err().code,
        "vault_exists"
    );
    server.cleanup().await;
}

#[tokio::test]
async fn approving_twice_is_refused_the_second_time() {
    let server = Server::start().await;
    let phone_dir = tempfile::tempdir().unwrap();
    let phone = client_in(phone_dir.path());
    phone.activate(server.invite("ana@example.com").await, SecretString::from(PASSWORD)).await.unwrap();
    until(|| phone.is_online()).await;
    let desk_dir = tempfile::tempdir().unwrap();
    let desktop = client_in(desk_dir.path());
    let start = desktop.start_pairing(server.base.clone(), "Desktop").await.unwrap();
    phone.approve_pairing(&start.link).await.unwrap();
    assert_eq!(phone.approve_pairing(&start.link).await.unwrap_err().code, "pairing_gone");
    server.cleanup().await;
}
```

`exec_on(&server, sql)` runs SQL on the test database (`server.pool.get().await.unwrap().batch_execute(sql)`). `until` and `login(title)` are copied from `round_trip.rs`. If `VaultStatus` has no `unlocked` field, use the field `round_trip.rs` checks after `activate`. Check `activate`'s exact signature in `account.rs:127` and adjust the call. Check the `vault_exists` error code by reading `ClientError`'s mapping of `havenkeys_core::Error::VaultExists`.

- [ ] **Step 2: Run to verify failure**

Run: `scripts/test-server.sh -p havenkeys-client --test pairing 2>&1 | tail -5` (the script passes extra args to `cargo test`; if `-p` conflicts with its own `-p` list, run `HAVENKEYS_TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable cargo test -p havenkeys-client --test pairing` after the script has started the container once)
Expected: compile errors.

- [ ] **Step 3: Error constructors** (`error.rs`, next to `sign_in_failed`)

```rust
    /// The code expired, was used, or was denied. One message for all.
    pub fn pairing_gone() -> Self {
        Self::fixed("pairing_gone", "This code has expired. Ask the new device for a new one.")
    }

    /// A code from a device signing in to a different server.
    pub fn pairing_other_server() -> Self {
        Self::fixed("pairing_other_server", "This code is for another server.")
    }

    /// The approval arrived but did not open or did not match this account.
    pub fn pairing_failed() -> Self {
        Self::fixed("pairing_failed", "The sign-in could not be completed. Ask for a new code.")
    }
```

- [ ] **Step 4: Share the end of `sign_in`** (`account.rs`)

Replace everything in `sign_in` from `let (status, online) = {` to the final `Ok(status)` with `self.go_online_after_sign_in(session)`, and add:

```rust
    /// The new vault exists and is open: go online with `session` and catch
    /// up in the background. A lock that landed meanwhile wins; the session
    /// is dropped unused and the locked status is reported.
    pub(crate) fn go_online_after_sign_in(self: &Arc<Self>, session: Session) -> ClientResult<VaultStatus> {
        let (status, online) = {
            let vault = self.vault()?;
            let online = vault.is_unlocked();
            if online {
                self.set_online(session);
            }
            (vault.status()?, online)
        };
        if !online {
            return Ok(status);
        }
        self.events.connectivity(true);
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let _ = client.sync_now().await;
        });
        Ok(status)
    }
```

Run `cargo test -p havenkeys-client --lib` — unchanged behaviour, still `ok`.

- [ ] **Step 5: Implement `pairing.rs`**

```rust
//! Signing in a new device from an approving one
//! (spec 2026-10-03-phone-approved-sign-in). The desktop side starts a
//! pairing, polls for the answer and builds its vault from the envelope; the
//! phone side reads a scanned code, and approves or denies it.

use crate::account::new_account_record;
use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use havenkeys_core::pairing::{ClaimSecret, PairingKeys, PairingLink};
use havenkeys_core::sync::prepare_paired_sign_in;
use havenkeys_core::vault::VaultStatus;
use havenkeys_sync_client::client::Claim;
use havenkeys_sync_client::SyncError;
use std::sync::Arc;

/// What the desktop shows: the QR code's text and when it stops working.
pub struct PairingStart {
    pub link: String,
    pub expires_at: String,
}

#[derive(Debug)]
pub enum PairingPoll {
    Waiting,
    Denied,
    Expired,
    Approved(VaultStatus),
}

/// What the phone shows before Allow. From the server: shown, not trusted.
pub struct PairingRequest {
    pub link: String,
    pub device_name: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: String,
}

/// The desktop's half of one pairing, in memory only.
pub(crate) struct PendingPairing {
    keys: PairingKeys,
    claim: ClaimSecret,
    server_url: String,
    pairing_id: String,
}

fn normalize(server_url: &str) -> String {
    server_url.trim().trim_end_matches('/').to_string()
}

impl HavenClient {
    pub async fn start_pairing(&self, server_url: String, device_name: &str) -> ClientResult<PairingStart> {
        if self.vault()?.key_scheme()?.is_some() {
            return Err(havenkeys_core::Error::VaultExists.into());
        }
        let server_url = normalize(&server_url);
        let server = self.server_for(&server_url)?;
        let keys = PairingKeys::generate();
        let claim = ClaimSecret::generate()?;
        let created = server
            .create_pairing(self.device_id()?, device_name, &keys.public_key(), &claim.hash())
            .await?;
        let link = PairingLink {
            server_url: server_url.clone(),
            pairing_id: created.pairing_id.clone(),
            public_key: keys.public_key(),
        }
        .to_text();
        *self.pairing.lock().map_err(|_| ClientError::internal())? = Some(PendingPairing {
            keys,
            claim,
            server_url,
            pairing_id: created.pairing_id,
        });
        Ok(PairingStart {
            link,
            expires_at: created.expires_at,
        })
    }

    pub fn cancel_pairing(&self) {
        if let Ok(mut p) = self.pairing.lock() {
            *p = None;
        }
    }

    /// One claim attempt. Any answer but `Waiting` ends the pairing.
    pub async fn poll_pairing(self: &Arc<Self>) -> ClientResult<PairingPoll> {
        let (server_url, pairing_id, secret) = {
            let guard = self.pairing.lock().map_err(|_| ClientError::internal())?;
            let p = guard.as_ref().ok_or_else(ClientError::pairing_gone)?;
            (p.server_url.clone(), p.pairing_id.clone(), *p.claim.as_bytes())
        };
        let secret = zeroize::Zeroizing::new(secret);
        let server = self.server_for(&server_url)?;
        let answer = server.claim_pairing(&pairing_id, &secret).await;
        let (session, envelope) = match answer {
            Ok(Claim::Waiting) => return Ok(PairingPoll::Waiting),
            Ok(Claim::Denied) => {
                self.cancel_pairing();
                return Ok(PairingPoll::Denied);
            }
            Err(SyncError::Refused(_)) => {
                self.cancel_pairing();
                return Ok(PairingPoll::Expired);
            }
            Err(e) => return Err(e.into()),
            Ok(Claim::Approved { session, envelope }) => (session, envelope),
        };
        let pending = self
            .pairing
            .lock()
            .map_err(|_| ClientError::internal())?
            .take()
            .ok_or_else(ClientError::pairing_gone)?;
        let status = self.finish_pairing(pending, session, envelope).await?;
        Ok(PairingPoll::Approved(status))
    }

    async fn finish_pairing(
        self: &Arc<Self>,
        pending: PendingPairing,
        session: havenkeys_sync_client::Session,
        envelope: Vec<u8>,
    ) -> ClientResult<VaultStatus> {
        let failed = |_| ClientError::pairing_failed();
        let payload = pending
            .keys
            .open(&pending.server_url, &pending.pairing_id, &envelope)
            .map_err(failed)?;
        // The server named the account and vault; the envelope proves them.
        if payload.account_id != session.account_id || payload.vault_id != session.vault_id {
            return Err(ClientError::pairing_failed());
        }
        let server = self.server_for(&pending.server_url)?;
        let header = server.header(&session).await.map_err(|_| ClientError::pairing_failed())?;
        let paired = prepare_paired_sign_in(&header.bytes, payload).map_err(failed)?;
        let revision = paired.prepared.header_revision() as i64;
        let record = new_account_record(&paired.account, pending.server_url.clone(), revision);
        self.create_vault(paired.prepared, &record)?;
        self.device()?
            .set_secret_key(paired.account.id, &paired.secret_key)
            .map_err(|_| ClientError::file())?;
        self.go_online_after_sign_in(session)
    }

    /// The phone: parse a scanned code and ask the server about it.
    pub async fn pairing_request(&self, link: &str) -> ClientResult<PairingRequest> {
        let link = self.pairing_link_for_this_account(link)?;
        let (session, server) = (self.session()?, self.server()?);
        let details = server
            .pairing_details(&session, &link.pairing_id)
            .await
            .map_err(|e| self.pairing_error(e))?;
        Ok(PairingRequest {
            link: link.to_text(),
            device_name: details.device_name,
            ip: details.ip,
            location: details.location,
            created_at: details.created_at,
        })
    }

    /// The phone: seal this vault's keys to the code's device and approve.
    pub async fn approve_pairing(&self, link: &str) -> ClientResult<()> {
        let link = self.pairing_link_for_this_account(link)?;
        let envelope = {
            let vault = self.vault()?;
            let account = vault.account()?.ok_or(havenkeys_core::Error::NoVault)?;
            let secret_key = self
                .device()?
                .secret_key(account.account_id)
                .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
            vault.seal_pairing(&link, &account, secret_key)?
        };
        let (session, server) = (self.session()?, self.server()?);
        server
            .approve_pairing(&session, &link.pairing_id, &envelope)
            .await
            .map_err(|e| self.pairing_error(e))
    }

    pub async fn deny_pairing(&self, link: &str) -> ClientResult<()> {
        let link = self.pairing_link_for_this_account(link)?;
        let (session, server) = (self.session()?, self.server()?);
        server
            .deny_pairing(&session, &link.pairing_id)
            .await
            .map_err(|e| self.pairing_error(e))
    }

    /// Unlocked, online, and a code for this account's server.
    fn pairing_link_for_this_account(&self, link: &str) -> ClientResult<PairingLink> {
        self.require_unlocked()?;
        let link = PairingLink::parse(link)?;
        let ours = self.vault()?.account()?.ok_or(havenkeys_core::Error::NoVault)?.server_url;
        if normalize(&link.server_url) != normalize(&ours) {
            return Err(ClientError::pairing_other_server());
        }
        self.require_online()?;
        Ok(link)
    }

    /// A gone pairing is `pairing_gone`; a refused session or no answer go
    /// through the usual handling.
    fn pairing_error(&self, e: SyncError) -> ClientError {
        match e {
            SyncError::Refused(_) => ClientError::pairing_gone(),
            other => self.failed(other),
        }
    }
}
```

Adjust names to what exists: `PreparedVault::header_revision()` (used in `sign_in`), `havenkeys_sync_client::Session` import path (see `client.rs` line 8), `ClientError::file()` (used in `sign_in`). `self.device()?` is a `MutexGuard`: in `approve_pairing` it is taken while the vault guard is held, which is the same order `account_value` uses (vault, then device) — keep it.

- [ ] **Step 6: Run the tests**

Run: `scripts/test-server.sh 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: all `ok`, including `tests/pairing.rs`.

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt --all && cargo clippy -p havenkeys-client --all-targets -- -D warnings
git add crates/havenkeys-client
git commit -m "feat(client): start, poll and finish a pairing on the new device; read, approve and deny it on the phone"
```

---

### Task 7: Desktop — commands and the "Sign in with your phone" panel

**Files:**
- Create: `apps/desktop/src-tauri/src/pairing.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (`mod pairing;`, register 3 commands)
- Modify: `apps/desktop/src-tauri/build.rs` (`COMMANDS`), `apps/desktop/src-tauri/capabilities/main.json` (`allow-pairing-start`, `allow-pairing-poll`, `allow-pairing-cancel`)
- Create: `apps/desktop/src/lib/pairing.ts`, `apps/desktop/src/lib/pairing.test.ts`
- Create: `apps/desktop/src/views/PhoneSignInPanel.tsx`
- Modify: `apps/desktop/src/views/WelcomeScreen.tsx`, `src/lib/api.ts`, `src/lib/types.ts`, `src/i18n/en.ts`, `src/i18n/pt-BR.ts`, `src/i18n/errors.ts`, `src/styles.css`

**Interfaces:**
- Consumes: Task 6 (`start_pairing`, `poll_pairing`, `cancel_pairing`).
- Produces:
  - Tauri `pairing_start(serverUrl: string) -> { qrSize: number, qrModules: boolean[], expiresAt: string }`
  - Tauri `pairing_poll() -> { state: "waiting" | "denied" | "expired" } | { state: "approved", status: VaultStatus }`
  - Tauri `pairing_cancel() -> void`
  - TS `secondsLeft(expiresAt: string, now: number): number`, `nextStep(state: PollState["state"]): "poll" | "denied" | "expired" | "done"`

- [ ] **Step 1: Write the failing TS tests** (`src/lib/pairing.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { nextStep, secondsLeft } from "./pairing";

describe("secondsLeft", () => {
  it("counts down to zero and never below", () => {
    const at = Date.parse("2026-10-03T12:02:00Z");
    expect(secondsLeft("2026-10-03T12:02:00Z", at - 90_500)).toBe(91);
    expect(secondsLeft("2026-10-03T12:02:00Z", at + 5_000)).toBe(0);
  });
  it("treats an unreadable time as expired", () => {
    expect(secondsLeft("not a date", Date.now())).toBe(0);
  });
});

describe("nextStep", () => {
  it("keeps polling only while waiting", () => {
    expect(nextStep("waiting")).toBe("poll");
    expect(nextStep("denied")).toBe("denied");
    expect(nextStep("expired")).toBe("expired");
    expect(nextStep("approved")).toBe("done");
  });
});
```

Run: `cd apps/desktop && npx vitest run src/lib/pairing.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 2: Implement `src/lib/pairing.ts`**

```ts
/** Whole seconds until `expiresAt`, never negative; unreadable means expired. */
export function secondsLeft(expiresAt: string, now: number): number {
  const at = Date.parse(expiresAt);
  if (Number.isNaN(at)) return 0;
  return Math.max(0, Math.ceil((at - now) / 1000));
}

export type PollState = "waiting" | "denied" | "expired" | "approved";

/** What the panel does after one claim answer. */
export function nextStep(state: PollState): "poll" | "denied" | "expired" | "done" {
  switch (state) {
    case "waiting":
      return "poll";
    case "approved":
      return "done";
    default:
      return state;
  }
}
```

Run the test again. Expected: PASS.

- [ ] **Step 3: Tauri commands** (`src-tauri/src/pairing.rs`)

```rust
//! Signing this computer in from the phone
//! (spec 2026-10-03-phone-approved-sign-in §3.1, §3.4). The pairing's keys
//! stay in `havenkeys_client`; the renderer gets the QR code's modules, the
//! time left and the outcome.

use crate::state::{AppState, CmdResult};
use havenkeys_client::PairingPoll;
use havenkeys_core::vault::VaultStatus;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCode {
    qr_size: usize,
    /// `qr_size` × `qr_size` modules, row-major, true = dark.
    qr_modules: Vec<bool>,
    expires_at: String,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum PairingState {
    Waiting,
    Denied,
    Expired,
    Approved { status: VaultStatus },
}

/// "Desktop · Linux": the name the phone shows before Allow.
fn device_label() -> String {
    let os = match std::env::consts::OS {
        "linux" => "Linux",
        "windows" => "Windows",
        "macos" => "macOS",
        other => other,
    };
    format!("Desktop · {os}")
}

#[tauri::command]
pub async fn pairing_start(app: AppHandle, server_url: String) -> CmdResult<PairingCode> {
    let client = app.state::<AppState>().client().clone();
    let start = client.start_pairing(server_url, &device_label()).await?;
    let code = QrCode::with_error_correction_level(start.link.as_bytes(), EcLevel::M)
        .map_err(|_| crate::state::CmdError::internal())?;
    Ok(PairingCode {
        qr_size: code.width(),
        qr_modules: code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect(),
        expires_at: start.expires_at,
    })
}

#[tauri::command]
pub async fn pairing_poll(app: AppHandle) -> CmdResult<PairingState> {
    let client = app.state::<AppState>().client().clone();
    Ok(match client.poll_pairing().await? {
        PairingPoll::Waiting => PairingState::Waiting,
        PairingPoll::Denied => PairingState::Denied,
        PairingPoll::Expired => PairingState::Expired,
        PairingPoll::Approved(status) => PairingState::Approved { status },
    })
}

#[tauri::command]
pub fn pairing_cancel(app: AppHandle) {
    app.state::<AppState>().client().cancel_pairing();
}
```

Check `emergency_kit.rs` (~line 60) for how it maps `QrCode` errors and modules, and mirror it exactly. `VaultStatus` must be `Serialize` (it already crosses to the renderer from `sign_in`).

Register in `lib.rs` `invoke_handler`, after `account::sign_in,`: `pairing::pairing_start, pairing::pairing_poll, pairing::pairing_cancel,`. Add `"pairing_start", "pairing_poll", "pairing_cancel",` to `build.rs`'s `COMMANDS` after `"sign_in",`, and `"allow-pairing-start", "allow-pairing-poll", "allow-pairing-cancel",` to `capabilities/main.json` after `"allow-sign-in",`.

Run: `cargo clippy -p havenkeys-desktop --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: API, types, i18n**

`src/lib/types.ts`:

```ts
export interface PairingCode {
  qrSize: number;
  qrModules: boolean[];
  expiresAt: string;
}

export type PairingState =
  | { state: "waiting" | "denied" | "expired" }
  | { state: "approved"; status: VaultStatus };
```

`src/lib/api.ts`, next to `signIn`:

```ts
  pairingStart: (serverUrl: string) => call<PairingCode>("pairing_start", { serverUrl: serverUrl.trim() }),
  pairingPoll: () => call<PairingState>("pairing_poll"),
  pairingCancel: () => call<void>("pairing_cancel"),
```

`src/i18n/en.ts`, inside `welcome`:

```ts
    tabPhone: "Use your phone",
    subPhone: "Approve this computer from HavenKeys on your phone.",
    phoneServer: "Your server",
    phoneShowCode: "Show code",
    phoneStarting: "Preparing…",
    phoneScan: "On your phone, open HavenKeys → Settings → Sign in a new device, and scan this code.",
    phoneQrLabel: "Sign-in code for your phone",
    phoneExpiresIn: (s: number) => `Expires in ${s}s`,
    phoneExpired: "This code has expired.",
    phoneNewCode: "New code",
    phoneDenied: "The sign-in was denied on your phone.",
    phoneUseKit: "Use the Emergency Kit instead",
```

`src/i18n/pt-BR.ts`, same keys:

```ts
    tabPhone: "Usar o celular",
    subPhone: "Aprove este computador pelo HavenKeys no seu celular.",
    phoneServer: "Seu servidor",
    phoneShowCode: "Mostrar código",
    phoneStarting: "Preparando…",
    phoneScan: "No celular, abra o HavenKeys → Ajustes → Entrar em um novo dispositivo e escaneie este código.",
    phoneQrLabel: "Código de entrada para o seu celular",
    phoneExpiresIn: (s: number) => `Expira em ${s}s`,
    phoneExpired: "Este código expirou.",
    phoneNewCode: "Novo código",
    phoneDenied: "A entrada foi recusada no seu celular.",
    phoneUseKit: "Usar o Emergency Kit",
```

`src/i18n/errors.ts`: map `pairing_gone`, `pairing_other_server`, `pairing_failed` to the Global Constraints strings, following the file's existing code→message pattern (add the keys to both locales' error tables).

If the TypeScript type of `en.ts` is inferred and `pt-BR.ts` is checked against it, `npx tsc --noEmit` will flag a missing key — fix until clean.

- [ ] **Step 5: The panel** (`src/views/PhoneSignInPanel.tsx`)

```tsx
import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import { nextStep, secondsLeft } from "../lib/pairing";
import type { PairingCode, VaultStatus } from "../lib/types";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

const POLL_MS = 2000;

type Phase =
  | { kind: "server" }
  | { kind: "code"; code: PairingCode }
  | { kind: "expired" }
  | { kind: "denied" };

/** The server, then the code, then the outcome. Keys never reach React. */
export function PhoneSignInPanel({
  onSignedIn,
  onUseKit,
}: {
  onSignedIn: (s: VaultStatus) => void;
  onUseKit: () => void;
}) {
  const { t } = useI18n();
  const [server, setServer] = useState("");
  const [phase, setPhase] = useState<Phase>({ kind: "server" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => first.current?.focus(), []);
  // Leaving the panel ends the pairing on this side.
  useEffect(() => () => void api.pairingCancel().catch(() => {}), []);

  useEffect(() => {
    if (phase.kind !== "code") return;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const answer = await api.pairingPoll();
        if (stopped) return;
        const step = nextStep(answer.state);
        if (step === "done" && answer.state === "approved") onSignedIn(answer.status);
        else if (step === "denied") setPhase({ kind: "denied" });
        else if (step === "expired") setPhase({ kind: "expired" });
        else timer = setTimeout(poll, POLL_MS);
      } catch (err) {
        if (stopped) return;
        setError(errorMessage(err, t, t.welcome.signInFailed));
        timer = setTimeout(poll, POLL_MS);
      }
    };
    timer = setTimeout(poll, POLL_MS);
    return () => {
      stopped = true;
      clearInterval(tick);
      clearTimeout(timer);
    };
    // `t` is left out: a language change must not restart the pairing.
  }, [phase, onSignedIn]);

  const left = phase.kind === "code" ? secondsLeft(phase.code.expiresAt, now) : 0;
  useEffect(() => {
    if (phase.kind === "code" && left === 0) setPhase({ kind: "expired" });
  }, [phase, left]);

  async function start(e?: FormEvent) {
    e?.preventDefault();
    if (busy || !server.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const code = await api.pairingStart(server);
      setNow(Date.now());
      setPhase({ kind: "code", code });
    } catch (err) {
      setError(errorMessage(err, t, t.welcome.signInFailed));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="welcome-form">
      {phase.kind === "server" && (
        <form onSubmit={start} noValidate>
          <label className="wf">
            <span className="wf-label">{t.welcome.phoneServer}</span>
            <input
              ref={first}
              className="wf-input"
              value={server}
              onChange={(e) => setServer(e.target.value)}
              placeholder="https://vault.example.com"
              disabled={busy}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
            />
            <span className="wf-hint">{t.welcome.serverHint}</span>
          </label>
          <button className="btn btn-primary btn-lg" type="submit" disabled={busy || !server.trim()}>
            {busy ? t.welcome.phoneStarting : t.welcome.phoneShowCode}
          </button>
        </form>
      )}

      {phase.kind === "code" && (
        <div className="pair-code">
          <PairQr size={phase.code.qrSize} modules={phase.code.qrModules} label={t.welcome.phoneQrLabel} />
          <p className="pair-help">{t.welcome.phoneScan}</p>
          <p className="pair-left" aria-live="polite">
            {t.welcome.phoneExpiresIn(left)}
          </p>
        </div>
      )}

      {(phase.kind === "expired" || phase.kind === "denied") && (
        <div className="pair-code">
          <p role="alert">{phase.kind === "expired" ? t.welcome.phoneExpired : t.welcome.phoneDenied}</p>
          <button className="btn btn-primary" type="button" onClick={() => void start()} disabled={busy}>
            {t.welcome.phoneNewCode}
          </button>
        </div>
      )}

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <button className="btn btn-quiet" type="button" onClick={onUseKit}>
        {t.welcome.phoneUseKit}
      </button>
    </div>
  );
}

function PairQr({ size, modules, label }: { size: number; modules: boolean[]; label: string }) {
  const quiet = 4;
  const box = size + quiet * 2;
  return (
    <svg className="pair-qr" viewBox={`0 0 ${box} ${box}`} role="img" aria-label={label} shapeRendering="crispEdges">
      <rect width={box} height={box} fill="#fff" />
      {modules.map((dark, i) =>
        dark ? <rect key={i} x={(i % size) + quiet} y={Math.floor(i / size) + quiet} width={1} height={1} fill="#000" /> : null,
      )}
    </svg>
  );
}
```

Before writing `PairQr`, read `components/EmergencyKit.tsx`'s `QrCode` (lines 10-30): if it is exported or can be exported with an `ariaLabel` prop, reuse it instead of `PairQr`. Use the button class names the codebase already has (check `styles.css` for `btn-quiet` or the equivalent secondary style).

`styles.css`: add `.pair-code { display: grid; justify-items: center; gap: 12px; }`, `.pair-qr { width: 220px; height: 220px; border-radius: var(--r-row, 8px); }`, `.pair-help, .pair-left { text-align: center; color: var(--muted); }` using the variables the file defines for muted text and radii.

- [ ] **Step 6: Wire it into the Welcome screen**

In `WelcomeScreen.tsx`: `type Path = "invite" | "signin" | "phone";`, a third segmented tab (`t.welcome.tabPhone`, same markup as the other two), the subtitle `path === "phone" ? t.welcome.subPhone : …`, and the body:

```tsx
        {path === "invite" && <InvitePanel onActivated={onActivated} />}
        {path === "signin" && <SignInPanel onSignedIn={onSignedIn} />}
        {path === "phone" && <PhoneSignInPanel onSignedIn={onSignedIn} onUseKit={() => setPath("signin")} />}
```

- [ ] **Step 6b: "Approved by" in the desktop's Devices list**

In the desktop view that lists devices (`grep -rln "listDevices" apps/desktop/src/views`), under a device's name, when `approvedBy` is set and another device in the same list has that id, show `t.devices.approvedBy(name)` ("Approved by {name}" / "Aprovado por {name}"); add the key to both locales. No new command.

- [ ] **Step 7: Check and commit**

Run: `cd apps/desktop && npx tsc --noEmit && npx vitest run && npx eslint src --max-warnings 0 2>/dev/null || npm run lint`
Expected: no type errors; all vitest suites pass; lint clean (use whichever lint script `package.json` defines).

```bash
git add apps/desktop
git commit -m "feat(desktop): sign this computer in with your phone (QR code, countdown, new code, denied)"
```

---

### Task 8: Mobile — uniffi exports

**Files:**
- Create: `crates/havenkeys-mobile/src/pairing.rs`
- Modify: `crates/havenkeys-mobile/src/lib.rs` (`mod pairing;`, `pub use pairing::PairingRequestView;`)
- Modify: `crates/havenkeys-mobile/tests/round_trip.rs` (approval test)
- Regenerate: `apps/android/app/src/main/kotlin/uniffi/havenkeys_mobile/havenkeys_mobile.kt`

**Interfaces:**
- Consumes: Task 6 phone side; `LumaFrame`, `qr::decode`.
- Produces (on `MobileVault`, exported):
  - `scan_pairing(frame: LumaFrame) -> MobileResult<Option<String>>` — the link text if the frame holds a `havenkeys://pair/v1` link, else `None`
  - `pairing_request(link: String) -> MobileResult<PairingRequestView>`; `#[derive(uniffi::Record)] pub struct PairingRequestView { pub link: String, pub device_name: String, pub ip: String, pub location: Option<String>, pub created_at: String }`
  - `approve_pairing(link: String) -> MobileResult<()>`, `deny_pairing(link: String) -> MobileResult<()>`

- [ ] **Step 1: Write the failing tests**

In `pairing.rs`'s test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::onboarding::LumaFrame;
    use crate::vault::tests::unlocked;

    fn frame(text: &str) -> LumaFrame {
        let (bytes, width, height) = crate::qr::tests::frame_of(text);
        LumaFrame { width, height, bytes }
    }

    #[test]
    fn scan_pairing_keeps_only_pair_links() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let keys = havenkeys_core::pairing::PairingKeys::generate();
        let link = havenkeys_core::pairing::PairingLink {
            server_url: "https://vault.example.com".into(),
            pairing_id: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            public_key: keys.public_key(),
        }
        .to_text();
        assert_eq!(v.scan_pairing(frame(&link)).unwrap().as_deref(), Some(link.as_str()));
        assert_eq!(v.scan_pairing(frame("otpauth://totp/A?secret=JBSWY3DPEHPK3PXP")).unwrap(), None);
        assert_eq!(v.scan_pairing(frame("havenkeys://kit/v2?x=1")).unwrap(), None);
        assert_eq!(v.scan_pairing(frame("https://example.com")).unwrap(), None);
    }

    #[test]
    fn a_locked_vault_scans_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        v.lock();
        assert!(v.scan_pairing(frame("https://example.com")).is_err());
    }
}
```

In `tests/round_trip.rs` (it already runs a real server and a phone), add a test: a phone `MobileVault` signed in through the existing helpers; a desktop `HavenClient` built the way `crates/havenkeys-client/tests/pairing.rs` builds one (copy that helper); `desktop.start_pairing(server.base, "Desktop")`; `phone.pairing_request(link)` returns `device_name == "Desktop"`; `phone.approve_pairing(link)`; `desktop.poll_pairing()` is `Approved`; then `phone.approve_pairing(link)` errors with code `pairing_gone` (use the file's `code(e)` helper). If adding `havenkeys-client` as a dev-dependency of `havenkeys-mobile` is needed, it is already a normal dependency.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-mobile pairing 2>&1 | tail -3`
Expected: compile errors.

- [ ] **Step 3: Implement** (`pairing.rs`)

```rust
//! The phone's side of signing a new device in
//! (spec 2026-10-03-phone-approved-sign-in §3.2). Kotlin scans and shows;
//! Rust reads the code, talks to the server and seals the keys.

use crate::error::MobileResult;
use crate::onboarding::LumaFrame;
use crate::qr;
use crate::vault::MobileVault;
use havenkeys_core::pairing::PairingLink;

#[derive(uniffi::Record)]
pub struct PairingRequestView {
    /// The code as scanned, for `approve_pairing` / `deny_pairing`.
    pub link: String,
    pub device_name: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: String,
}

#[uniffi::export]
impl MobileVault {
    /// A camera frame on the "Sign in a new device" screen. Only a
    /// `havenkeys://pair/v1` link comes back; any other code is dropped.
    pub fn scan_pairing(&self, frame: LumaFrame) -> MobileResult<Option<String>> {
        self.unlocked()?;
        let Some(text) = qr::decode(frame.bytes, frame.width, frame.height) else {
            return Ok(None);
        };
        Ok(PairingLink::parse(&text).ok().map(|_| text.to_string()))
    }

    pub fn pairing_request(&self, link: String) -> MobileResult<PairingRequestView> {
        self.touch();
        let client = self.client.clone();
        let r = self.block_on(async move { client.pairing_request(&link).await })?;
        Ok(PairingRequestView {
            link: r.link,
            device_name: r.device_name,
            ip: r.ip,
            location: r.location,
            created_at: r.created_at,
        })
    }

    /// Kotlin asks for biometrics before calling this.
    pub fn approve_pairing(&self, link: String) -> MobileResult<()> {
        self.touch();
        let client = self.client.clone();
        Ok(self.block_on(async move { client.approve_pairing(&link).await })?)
    }

    pub fn deny_pairing(&self, link: String) -> MobileResult<()> {
        self.touch();
        let client = self.client.clone();
        Ok(self.block_on(async move { client.deny_pairing(&link).await })?)
    }
}
```

Match `block_on`'s actual signature (see `sync_now` in `account.rs`: `self.block_on(client.sync_now())`), and the name of the activity-reset call (`touch` or `record_activity`; see `vault.rs`). `qr::decode` returns `Zeroizing<String>`; `.to_string()` copies the (non-secret) link.

- [ ] **Step 4: Run tests, lint, regenerate bindings**

```bash
cargo test -p havenkeys-mobile 2>&1 | grep "test result"
scripts/test-server.sh 2>&1 | grep -E "test result|FAILED"   # runs server/sync-client/client
HAVENKEYS_TEST_DATABASE_URL='postgres://postgres:postgres@localhost:5433/postgres?sslmode=disable' cargo test -p havenkeys-mobile --test round_trip 2>&1 | grep "test result"
cargo clippy -p havenkeys-mobile --all-targets -- -D warnings
ANDROID_NDK_HOME=/home/sams/Android/Sdk/ndk/30.0.16248370 scripts/build-android.sh
```
Expected: all `ok`; the bindings file gains `scanPairing`, `pairingRequest`, `approvePairing`, `denyPairing`, `PairingRequestView`.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-mobile apps/android/app/src/main/kotlin/uniffi
git commit -m "feat(mobile): scan, read, approve and deny a new device's sign-in code"
```

---

### Task 9: Android — "Sign in a new device"

**Files:**
- Create: `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/pairing/PairingViewModel.kt`
- Create: `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/pairing/PairingScreen.kt`
- Modify: `data/AccountRepository.kt` (4 calls), `ui/nav/Routes.kt` (`PAIRING = "pairing"`), `ui/nav/NavMotion.kt` (`PUSHED` gains `Routes.PAIRING`), `ui/nav/HavenNavHost.kt` (composable), `ui/nav/NavServices.kt` (`verifyUser`), `ui/nav/ShellDestinations.kt` + `ui/settings/SettingsNavigation.kt` (`onPairing`), `ui/settings/SettingsScreen.kt` (row), `ui/components/ErrorText.kt` (3 codes), `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Modify: `app/src/test/kotlin/net/havenkeys/android/fakes/FakeRepositories.kt`
- Create: `app/src/test/kotlin/net/havenkeys/android/ui/pairing/PairingViewModelTest.kt`, `PairingScreenTest.kt`

**Interfaces:**
- Consumes: Task 8 bindings.
- Produces:
  - `AccountRepository.scanPairing(frame: LumaFrame): Outcome<String?>`, `pairingRequest(link: String): Outcome<PairingRequestView>`, `approvePairing(link: String): Outcome<Unit>`, `denyPairing(link: String): Outcome<Unit>`
  - `PairingUiState(stage: Stage = Stage.SCANNING, request: PairingRequestView? = null, busy: Boolean = false, errorCode: String? = null)` with `enum class Stage { SCANNING, CONFIRM, DONE, DENIED }`
  - `NavServices.verifyUser: suspend (title: String, subtitle: String) -> Boolean`

- [ ] **Step 1: Strings**

`res/values/strings.xml`:

```xml
    <string name="settings_pairing">Sign in a new device</string>
    <string name="pairing_title">Sign in a new device</string>
    <string name="pairing_scan_sub">On the new computer, choose “Use your phone” and scan the code it shows.</string>
    <string name="pairing_camera_needed">Allow the camera to scan the new device’s code.</string>
    <string name="pairing_camera_unavailable">The camera could not be opened.</string>
    <string name="pairing_confirm_title">Sign in a new device?</string>
    <string name="pairing_confirm_where">%1$s · %2$s</string>
    <string name="pairing_confirm_warning">Allow only a device you are using right now. It will be able to open your whole vault.</string>
    <string name="pairing_allow">Allow</string>
    <string name="pairing_deny">Deny</string>
    <string name="pairing_verify_title">Approve the new device</string>
    <string name="pairing_done">The new device is signed in.</string>
    <string name="pairing_denied">The sign-in was denied.</string>
    <string name="error_pairing_gone">This code has expired. Ask the new device for a new one.</string>
    <string name="error_pairing_other_server">This code is for another server.</string>
    <string name="error_pairing_failed">The sign-in could not be completed. Ask for a new code.</string>
```

`res/values-pt-rBR/strings.xml`:

```xml
    <string name="settings_pairing">Entrar em um novo dispositivo</string>
    <string name="pairing_title">Entrar em um novo dispositivo</string>
    <string name="pairing_scan_sub">No novo computador, escolha “Usar o celular” e escaneie o código que ele mostra.</string>
    <string name="pairing_camera_needed">Permita o uso da câmera para escanear o código do novo dispositivo.</string>
    <string name="pairing_camera_unavailable">Não foi possível abrir a câmera.</string>
    <string name="pairing_confirm_title">Entrar em um novo dispositivo?</string>
    <string name="pairing_confirm_where">%1$s · %2$s</string>
    <string name="pairing_confirm_warning">Permita só um dispositivo que você está usando agora. Ele poderá abrir todo o seu cofre.</string>
    <string name="pairing_allow">Permitir</string>
    <string name="pairing_deny">Recusar</string>
    <string name="pairing_verify_title">Aprovar o novo dispositivo</string>
    <string name="pairing_done">O novo dispositivo entrou.</string>
    <string name="pairing_denied">A entrada foi recusada.</string>
    <string name="error_pairing_gone">Este código expirou. Peça um novo no novo dispositivo.</string>
    <string name="error_pairing_other_server">Este código é de outro servidor.</string>
    <string name="error_pairing_failed">Não foi possível concluir a entrada. Peça um novo código.</string>
```

`ErrorText.kt`: add `"pairing_gone" to R.string.error_pairing_gone, "pairing_other_server" to R.string.error_pairing_other_server, "pairing_failed" to R.string.error_pairing_failed,`.

- [ ] **Step 2: Repository**

`AccountRepository` interface:

```kotlin
    /** A `havenkeys://pair/v1` link in [frame], or null; the frame's pixels are wiped after. */
    suspend fun scanPairing(frame: LumaFrame): Outcome<String?>
    suspend fun pairingRequest(link: String): Outcome<PairingRequestView>
    /** Call only after the user passed the biometric check. */
    suspend fun approvePairing(link: String): Outcome<Unit>
    suspend fun denyPairing(link: String): Outcome<Unit>
```

`RustAccountRepository`:

```kotlin
    override suspend fun scanPairing(frame: LumaFrame) = rust {
        try {
            vault.scanPairing(frame)
        } finally {
            frame.bytes.fill(0)
        }
    }
    override suspend fun pairingRequest(link: String) = rust { vault.pairingRequest(link) }
    override suspend fun approvePairing(link: String) = rust { vault.approvePairing(link) }
    override suspend fun denyPairing(link: String) = rust { vault.denyPairing(link) }
```

Fake (`FakeRepositories.kt`, in the fake account repository):

```kotlin
    var scannedLink: Outcome<String?> = Outcome.Ok(null)
    var request: Outcome<PairingRequestView> = Outcome.Failed("pairing_gone")
    val approved = mutableListOf<String>()
    val denied = mutableListOf<String>()
    var approveResult: Outcome<Unit> = Outcome.Ok(Unit)

    override suspend fun scanPairing(frame: LumaFrame) = scannedLink
    override suspend fun pairingRequest(link: String) = request
    override suspend fun approvePairing(link: String): Outcome<Unit> {
        approved += link
        return approveResult
    }
    override suspend fun denyPairing(link: String): Outcome<Unit> {
        denied += link
        return Outcome.Ok(Unit)
    }
```

- [ ] **Step 3: Write the failing ViewModel test**

`PairingViewModelTest.kt` (follow the coroutine test setup another ViewModel test in `app/src/test` uses, e.g. `DevicesViewModelTest` or `EditViewModelTest` — `MainDispatcherRule`/`runTest`):

```kotlin
class PairingViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val accounts = FakeAccountRepository()
    private val view = PairingRequestView("havenkeys://pair/v1?x", "Desktop · Linux", "187.1.2.3", "São Paulo, BR", "2026-10-03T12:00:00Z")
    private fun frame() = LumaFrame(1u, 1u, byteArrayOf(1))

    @Test
    fun aScannedCodeShowsTheRequest() = runTest {
        accounts.scannedLink = Outcome.Ok("havenkeys://pair/v1?x")
        accounts.request = Outcome.Ok(view)
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame())
        advanceUntilIdle()
        assertEquals(PairingUiState.Stage.CONFIRM, vm.state.value.stage)
        assertEquals("Desktop · Linux", vm.state.value.request?.deviceName)
    }

    @Test
    fun allowWithoutTheBiometricCheckApprovesNothing() = runTest {
        accounts.scannedLink = Outcome.Ok("havenkeys://pair/v1?x")
        accounts.request = Outcome.Ok(view)
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame()); advanceUntilIdle()
        vm.allow(verify = { false }); advanceUntilIdle()
        assertTrue(accounts.approved.isEmpty())
        assertEquals(PairingUiState.Stage.CONFIRM, vm.state.value.stage)
        vm.allow(verify = { true }); advanceUntilIdle()
        assertEquals(listOf("havenkeys://pair/v1?x"), accounts.approved)
        assertEquals(PairingUiState.Stage.DONE, vm.state.value.stage)
    }

    @Test
    fun denyDenies() = runTest {
        accounts.scannedLink = Outcome.Ok("havenkeys://pair/v1?x")
        accounts.request = Outcome.Ok(view)
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame()); advanceUntilIdle()
        vm.deny(); advanceUntilIdle()
        assertEquals(listOf("havenkeys://pair/v1?x"), accounts.denied)
        assertEquals(PairingUiState.Stage.DENIED, vm.state.value.stage)
    }

    @Test
    fun aGoneCodeSaysSoAndScansAgain() = runTest {
        accounts.scannedLink = Outcome.Ok("havenkeys://pair/v1?x")
        accounts.request = Outcome.Failed("pairing_gone")
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame()); advanceUntilIdle()
        assertEquals(PairingUiState.Stage.SCANNING, vm.state.value.stage)
        assertEquals("pairing_gone", vm.state.value.errorCode)
    }

    @Test
    fun aFrameWithNoPairingCodeIsDroppedQuietly() = runTest {
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame()); advanceUntilIdle()
        assertEquals(PairingUiState(), vm.state.value)
    }
}
```

Run: `cd apps/android && ./gradlew :app:testGithubDebugUnitTest --tests '*PairingViewModelTest*' -q`
Expected: compile failure.

- [ ] **Step 4: Implement the ViewModel**

```kotlin
package net.havenkeys.android.ui.pairing

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.PairingRequestView

/** The code and the request are not secrets; nothing secret passes through here. */
data class PairingUiState(
    val stage: Stage = Stage.SCANNING,
    val request: PairingRequestView? = null,
    val busy: Boolean = false,
    val errorCode: String? = null,
) {
    enum class Stage { SCANNING, CONFIRM, DONE, DENIED }
}

/**
 * Settings → Sign in a new device (spec 2026-10-03-phone-approved-sign-in
 * §3.2): scan, show who is asking, and approve only after the biometric
 * check passes.
 */
class PairingViewModel(private val accounts: AccountRepository) : ViewModel() {
    private val _state = MutableStateFlow(PairingUiState())
    val state: StateFlow<PairingUiState> = _state.asStateFlow()
    private val decoding = AtomicBoolean(false)

    fun onFrame(frame: LumaFrame) {
        if (_state.value.stage != PairingUiState.Stage.SCANNING || !decoding.compareAndSet(false, true)) {
            frame.bytes.fill(0)
            return
        }
        viewModelScope.launch {
            try {
                val link = (accounts.scanPairing(frame) as? Outcome.Ok)?.value ?: return@launch
                when (val r = accounts.pairingRequest(link)) {
                    is Outcome.Ok -> _state.value = PairingUiState(PairingUiState.Stage.CONFIRM, r.value)
                    is Outcome.Failed -> _state.update { it.copy(errorCode = r.code) }
                }
            } finally {
                decoding.set(false)
            }
        }
    }

    /** [verify] is the biometric prompt; false (cancelled, failed) approves nothing. */
    fun allow(verify: suspend () -> Boolean) {
        val request = _state.value.request ?: return
        if (_state.value.busy) return
        _state.update { it.copy(busy = true, errorCode = null) }
        viewModelScope.launch {
            if (!verify()) {
                _state.update { it.copy(busy = false) }
                return@launch
            }
            when (val r = accounts.approvePairing(request.link)) {
                is Outcome.Ok -> _state.value = PairingUiState(PairingUiState.Stage.DONE, request)
                is Outcome.Failed -> _state.update { it.copy(busy = false, errorCode = r.code) }
            }
        }
    }

    fun deny() {
        val request = _state.value.request ?: return
        if (_state.value.busy) return
        _state.update { it.copy(busy = true) }
        viewModelScope.launch {
            accounts.denyPairing(request.link)
            _state.value = PairingUiState(PairingUiState.Stage.DENIED, request)
        }
    }

    /** Back to the camera after an error or a finished approval. */
    fun scanAgain() {
        _state.value = PairingUiState()
    }
}
```

Run the test again. Expected: PASS (5 tests).

- [ ] **Step 5: The screen**

`PairingScreen.kt`: a `HavenScaffold` with `ScreenBar(onBack, online, onLock)` titled `pairing_title`. Content by stage:
- `SCANNING`: `HavenText(pairing_scan_sub)` muted, then `KitScanner(onFrame = viewModel::onFrame, modifier = Modifier.fillMaxWidth().aspectRatio(1f).clip(HavenShape.group), cameraNeeded = stringResource(R.string.pairing_camera_needed), cameraUnavailable = stringResource(R.string.pairing_camera_unavailable))`; under it `ErrorLine(code)` when `errorCode != null`. If `!online`, show `ErrorLine("offline")` instead of the camera.
- `CONFIRM`: `HavenSheet(onDismiss = viewModel::deny, title = stringResource(R.string.pairing_confirm_title))` containing an `InsetGroup` with one `GroupRow { GroupRowText(request.deviceName, stringResource(R.string.pairing_confirm_where, request.location ?: request.ip, relativeTime(request.createdAt))) }`, a muted `pairing_confirm_warning`, the error line, and two `HavenButton`s: Deny (`ButtonStyle.Quiet`, `viewModel::deny`) and Allow (primary, `enabled = !busy`, `onClick = { viewModel.allow { verifyUser(title, subtitle = request.deviceName) } }`).
- `DONE` / `DENIED`: a centred line (`pairing_done` / `pairing_denied`) and a button "Done" that calls `onBack`.

Show location and IP both when location exists: `"${location} · ${ip}"` as the subtitle's first part; `relativeTime` reuses the helper in `DevicesScreen.kt` (`DateUtils.getRelativeTimeSpanString`) after parsing the RFC 3339 string with `java.time.Instant.parse` (catch `DateTimeParseException` → show nothing).

Signature: `fun PairingScreen(viewModel: PairingViewModel, online: Boolean, verifyUser: suspend (String, String) -> Boolean, onBack: () -> Unit, onLock: () -> Unit, modifier: Modifier = Modifier)`.

`PairingScreenTest.kt` (Robolectric + `rule.setKit`, like `ItemsScreensTest`): with the fake set to CONFIRM state, assert the device name and "São Paulo, BR · 187.1.2.3" are shown; click "Allow" with `verifyUser = { _, _ -> false }` → `accounts.approved` is empty; click "Deny" → `accounts.denied` has the link. The camera is not composed in tests: drive the ViewModel to CONFIRM first via `onFrame` with the fake returning a link.

- [ ] **Step 6: Navigation and Settings**

- `Routes.PAIRING = "pairing"`; add to `PUSHED` in `NavMotion.kt`.
- `NavServices`: add `val verifyUser: suspend (title: String, subtitle: String) -> Boolean`; in `rememberNavServices` set it to `{ title, subtitle -> container.biometricGate.canVerifyUser(activity) && container.biometricGate.verifyUser(activity, title, subtitle) }`. Update the test builder of `NavServices` (search `NavServices(` in `app/src/test`) with `verifyUser = { _, _ -> true }`.
- `HavenNavHost.kt`, after `Routes.DEVICES`:

```kotlin
    composable(Routes.PAIRING) {
        val online by services.events.online.collectAsStateWithLifecycle()
        val title = stringResource(R.string.pairing_verify_title)
        PairingScreen(
            viewModel = viewModel { PairingViewModel(services.accounts) },
            online = online,
            verifyUser = { _, subtitle -> services.verifyUser(title, subtitle) },
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
```

- `SettingsNavigation(val onDevices: () -> Unit, val onAutofillSetup: () -> Unit, val onPairing: () -> Unit)`; `ShellDestinations.kt` passes `onPairing = { navController.pushOnce(Routes.PAIRING) }`; fix other constructors of `SettingsNavigation` in tests.
- `SettingsScreen.kt` `AccountGroup`, right after the Devices row:

```kotlin
        row {
            GroupRow(onClick = navigation.onPairing, icon = HavenIcon.Qr, chevron = true) {
                GroupRowText(stringResource(R.string.settings_pairing), offline)
            }
        }
```

(If the Devices row has no icon/chevron, match it instead.) A locked vault never reaches Settings, so no lock check is needed here; offline is shown and the screen shows the offline line.

- [ ] **Step 6b: "Approved by" in the phone's Devices list**

In `DevicesScreen.kt` `DeviceRow`, when `device.approvedBy` matches another listed device's `id`, add a muted line `stringResource(R.string.devices_approved_by, thatDevice.name)` under the name. Strings: `<string name="devices_approved_by">Approved by %1$s</string>` / `<string name="devices_approved_by">Aprovado por %1$s</string>`. Add a case to the existing Devices screen test (fake list with two devices, one `approvedBy` the other) asserting the line is shown. Update every `DeviceInfo(…)` constructor in tests and fakes with the new argument.

- [ ] **Step 7: Run everything and commit**

```bash
cd apps/android && export JAVA_HOME=/home/sams/.local/opt/jdk-17.0.20.1+1
./gradlew :app:testGithubDebugUnitTest :app:detekt :app:lintGithubDebug --continue 2>&1 | grep -E "FAILED|tests completed|BUILD|\.kt:[0-9]+:" | grep -v "^w:"
```
Expected: `BUILD SUCCESSFUL`.

```bash
git add apps/android
git commit -m "feat(android): Settings → Sign in a new device: scan, confirm who is asking, approve behind biometrics"
```

---

### Task 10: Documentation, audits and the security review

**Files:**
- Modify: `CLAUDE.md` (amendment note), `docs/threat-model.md`, `docs/security-model.md`, `docs/crypto.md`, `docs/architecture.md`, `docs/security-review.md`, the spec's `Status:` line
- Modify: the Android spec's onboarding section and the desktop/README sign-in description, wherever they list the ways to add a device (`grep -rn "Emergency Kit" docs README.md | grep -i "sign in\|new device"`)

- [ ] **Step 1: CLAUDE.md amendment** — after the 2026-10-03 activity-record note, add:

```markdown
> Amended on 2026-10-03 by
> `docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md`: a
> new desktop may be signed in by scanning its QR code with the unlocked
> phone and tapping Allow (behind biometrics). The phone seals the vault key
> and Secret Key to the desktop's one-time key with HPKE; the server relays
> ciphertext and issues the desktop's session. The master password is not
> typed for that first sign-in; every later unlock needs it as usual.
```

- [ ] **Step 2: Threat and security model**

- `docs/threat-model.md`: a "Phone-approved sign-in" section with the spec's §2 trust decision and §7 table, verbatim in substance, including the residual social-engineering risk.
- `docs/security-model.md`: the five endpoints (auth, limits, what each returns), the `pairings` table and what it holds for how long (token and envelope until claim, at most 10 minutes), `devices.approved_by`, `HAVENKEYS_GEOIP_DATABASE`, and that the core session now holds the vault key while unlocked (and why).
- `docs/crypto.md`: the HPKE suite, the `info` string, the envelope byte layout and the payload layout from Task 1, and the claim secret.
- `docs/architecture.md`: the flow diagram from spec §3 in text.

- [ ] **Step 3: Audits**

```bash
cargo audit 2>&1 | tail -5
cargo deny check 2>&1 | tail -5
cd apps/desktop && npm audit --omit=dev 2>&1 | tail -3
```
Expected: no new advisories or licence failures. Record the results in Step 4.

- [ ] **Step 4: Security review** — append a dated section to `docs/security-review.md` in its existing format (finding, severity, component, attack, mitigation, residual) covering at least:
  - an attacker-shown QR code approved by a deceived user (residual, documented);
  - a malicious server substituting the public key (not possible: key travels by camera) or lying about name/location (possible; harmless without the private key);
  - `pairings.token` held in plaintext until claim (bounded to 10 minutes; a database reader in that window gains a session but no key material);
  - the vault key now in the unlocked session (same exposure class as the data key);
  - envelope and payload parsers (deterministic garbage tests in Task 1);
  - secret logging (`no_logging` test passes; `Debug` impls redact).

- [ ] **Step 5: Spec status** — change the spec's `Status: proposed, 2026-10-03.` to `Status: accepted, 2026-10-03; implemented by docs/superpowers/plans/2026-10-03-phone-approved-sign-in.md.`

- [ ] **Step 6: Final full check and commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
scripts/test-server.sh 2>&1 | grep -E "test result|FAILED" | sort | uniq -c
cargo test --workspace --exclude havenkeys-server --exclude havenkeys-client --exclude havenkeys-sync-client 2>&1 | grep -E "test result|FAILED" | sort | uniq -c
git add CLAUDE.md docs README.md
git commit -m "docs: phone-approved sign-in in the threat and security models, crypto notes and the security review"
```
Expected: no `FAILED`; every `test result` line `ok`.
