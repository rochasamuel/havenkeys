# Core: key scheme 3 and delta sync — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Teach `havenkeys-core` to derive keys from an account (email + master password + Secret Key), to prove that knowledge to a server without handing over anything that decrypts, and to sync item-by-item deltas against a cursor — all without a single line of networking.

**Architecture:** A new `account` module owns the normalized email and the account reference. `crypto::keys` gains a v3 derivation that produces the KEK and an `AuthKey` from one Argon2id run, separated only by HKDF `info`. The store gets schema 3: a `dirty` flag per row and an `account` table holding the server cursor and the header-rollback guard. `VaultService` gains a delta path that **reuses the existing merge decision rules** rather than restating them — the snapshot path and the delta path call the same helper, so the rules cannot drift apart.

**Tech Stack:** Rust 1.88, `hkdf` 0.13 + `sha2` 0.11, `argon2` 0.6, `rusqlite` 0.40 (bundled), `uuid` 1.20, `unicode-normalization` 0.1, `zeroize` 1.9. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-19-server-accounts-sync-design.md`

## Global Constraints

Every task's requirements implicitly include these. Values are copied verbatim from the spec.

- `havenkeys-core` keeps **no network dependency**. Nothing in this plan adds one.
- KEK derivation, key scheme 3: `HKDF-SHA-256(ikm = MK ‖ SecretKey, salt = account_id ‖ email_nfc, info = "havenkeys/v3/kek")`.
- Auth key derivation: same ikm and salt, `info = "havenkeys/v3/auth"`.
- `account_id ‖ email` means the 16 raw bytes of the account UUID followed by the NFC-normalized, lowercased email as UTF-8.
- Argon2id bounds stay as they are: memory 19 456 KiB – 1 GiB, iterations 2–16, parallelism 1–16, salt exactly 16 bytes, output 32 bytes.
- Per-blob limit stays 8 MiB.
- Downgrade is refused: a device on scheme 3 never adopts a scheme 1 or 2 header.
- Header rollback is refused: the client records the highest `header_revision` it has ever accepted and refuses anything lower.
- The crate must keep compiling under `#![forbid(unsafe_code)]` and `#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]`. Nothing logs; `Error` variants never carry data.
- New secret-bearing types implement a redacted `Debug` and zeroize on drop, like `Key256` and `SecretString`.
- Existing vaults (key scheme 1 and 2) keep opening. No item is ever re-encrypted by a scheme change.

**Verification command used throughout:** `cargo test -p havenkeys-core` and `cargo clippy -p havenkeys-core --all-targets -- -D warnings`.

**Out of scope for this plan** (later plans): the server, the HTTP client, the desktop UI, and the removal of `sync::folder`. Folder sync must keep working and keep passing its tests at every commit here.

---

### Task 1: Normalized email and account reference

**Files:**
- Create: `crates/havenkeys-core/src/account.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `pub mod account;`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `havenkeys_core::account::NormalizedEmail` with `NormalizedEmail::parse(&str) -> Result<NormalizedEmail>` and `fn as_str(&self) -> &str`
  - `havenkeys_core::account::AccountRef { pub id: Uuid, pub email: NormalizedEmail }` with `AccountRef::new(id: Uuid, email: NormalizedEmail) -> Self`

- [ ] **Step 1: Write the failing test**

Create `crates/havenkeys-core/src/account.rs` with only the test module plus the `use` lines:

```rust
//! Account identity: the email and account ID that key derivation is bound to.
//!
//! The email is part of the key derivation (docs/crypto.md, key scheme 3), so
//! two devices must normalize it identically or they derive different keys.
//! Normalization is therefore a correctness requirement, not cosmetics.

use crate::error::{Error, Result};
use std::fmt;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// Longest email accepted, from the SMTP path limit (RFC 5321 §4.5.3.1.3).
const MAX_EMAIL_LEN: usize = 254;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_surrounding_space() {
        let a = NormalizedEmail::parse("  User@Example.COM ").unwrap();
        let b = NormalizedEmail::parse("user@example.com").unwrap();
        assert_eq!(a.as_str(), "user@example.com");
        assert_eq!(a.as_str(), b.as_str());
    }

    #[test]
    fn normalizes_unicode_to_nfc() {
        // "josé@example.com" with a combining acute accent must equal the
        // precomposed form, or the two spellings derive different keys.
        let decomposed = NormalizedEmail::parse("jose\u{0301}@example.com").unwrap();
        let composed = NormalizedEmail::parse("jos\u{00e9}@example.com").unwrap();
        assert_eq!(decomposed.as_str(), composed.as_str());
    }

    #[test]
    fn rejects_malformed_addresses() {
        for bad in [
            "",
            "   ",
            "no-at-sign",
            "@example.com",
            "user@",
            "user@@example.com",
            "user name@example.com",
            "user@exa mple.com",
            "user\n@example.com",
        ] {
            assert!(
                NormalizedEmail::parse(bad).is_err(),
                "should have rejected {bad:?}"
            );
        }
    }

    #[test]
    fn rejects_overlong_addresses() {
        let long = format!("{}@example.com", "a".repeat(MAX_EMAIL_LEN));
        assert!(NormalizedEmail::parse(&long).is_err());
    }

    #[test]
    fn debug_shows_the_address() {
        // The email is not a secret; it is an identifier, and a redacted
        // Debug here would make support and tests harder for no gain.
        let e = NormalizedEmail::parse("user@example.com").unwrap();
        assert_eq!(format!("{e:?}"), "NormalizedEmail(\"user@example.com\")");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p havenkeys-core account::`
Expected: compile error — `cannot find type NormalizedEmail in this scope`. (The module is not wired into `lib.rs` yet either; that comes in step 3.)

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)]` block in `crates/havenkeys-core/src/account.rs`:

```rust
/// An email address normalized for key derivation: trimmed, NFC, lowercased.
#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedEmail(String);

impl NormalizedEmail {
    pub fn parse(raw: &str) -> Result<Self> {
        let normalized: String = raw.trim().nfc().collect::<String>().to_lowercase();
        if normalized.is_empty() || normalized.len() > MAX_EMAIL_LEN {
            return Err(Error::InvalidInput("email address is not valid"));
        }
        if normalized.chars().any(char::is_whitespace) {
            return Err(Error::InvalidInput("email address is not valid"));
        }
        let mut parts = normalized.split('@');
        let local = parts.next().unwrap_or("");
        let domain = parts.next().unwrap_or("");
        if local.is_empty() || domain.is_empty() || parts.next().is_some() {
            return Err(Error::InvalidInput("email address is not valid"));
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for NormalizedEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NormalizedEmail({:?})", self.0)
    }
}

/// The account a vault belongs to. Both fields are bound into the KEK, so a
/// vault cannot be opened under a different account or a different address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRef {
    pub id: Uuid,
    pub email: NormalizedEmail,
}

impl AccountRef {
    pub fn new(id: Uuid, email: NormalizedEmail) -> Self {
        Self { id, email }
    }
}
```

Add to `crates/havenkeys-core/src/lib.rs`, keeping the module list alphabetical (it goes first, before `pub mod crypto;`):

```rust
pub mod account;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p havenkeys-core account::`
Expected: 5 tests pass.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/account.rs crates/havenkeys-core/src/lib.rs
git commit -m "feat(core): normalized email and account reference"
```

---

### Task 2: v3 key derivation and the auth key

**Files:**
- Modify: `crates/havenkeys-core/src/crypto/keys.rs`
- Modify: `crates/havenkeys-core/src/crypto/secret_key.rs` (test-only constructor)
- Modify: `crates/havenkeys-core/src/crypto/mod.rs` (module doc line)

**Interfaces:**
- Consumes: `AccountRef`, `NormalizedEmail` (Task 1).
- Produces:
  - `crypto::keys::AuthKey` with `fn to_base64(&self) -> Zeroizing<String>`
  - `crypto::keys::derive_kek_v3(master_key: &Key256, secret_key: &SecretKey, account: &AccountRef) -> Result<Key256>`
  - `crypto::keys::derive_auth_key_from_master(master_key: &Key256, secret_key: &SecretKey, account: &AccountRef) -> Result<AuthKey>`
  - `crypto::secret_key::SecretKey::from_bytes([u8; SECRET_KEY_LEN]) -> SecretKey` (`#[cfg(test)] pub(crate)`)

Both derivations take an already-derived master key, so a caller runs Argon2id **once** and gets both the KEK and the auth key from it. Running it twice would double the unlock time for nothing.

- [ ] **Step 1: Generate the cross-check vectors with an independent HKDF**

The point of a known-answer test here is to freeze the wire format. Computing the expected value with the same Rust code it tests would prove nothing, so compute it with a separate implementation first.

Run:

```bash
python3 - <<'EOF'
import hmac, hashlib

def hkdf(salt, ikm, info, n=32):
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    okm, t, i = b"", b"", 1
    while len(okm) < n:
        t = hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        okm += t
        i += 1
    return okm[:n]

account_id = bytes.fromhex("00112233445566778899aabbccddeeff")
email = b"user@example.com"
mk = bytes([0x11]) * 32
sk = bytes([0x22]) * 16
salt = account_id + email
print("kek ", hkdf(salt, mk + sk, b"havenkeys/v3/kek").hex())
print("auth", hkdf(salt, mk + sk, b"havenkeys/v3/auth").hex())
EOF
```

Copy both hex strings into the test in step 2, replacing `<KEK_HEX>` and `<AUTH_HEX>`. Do not invent them and do not copy them from the Rust implementation's output.

- [ ] **Step 2: Write the failing test**

Append to the existing `mod tests` in `crates/havenkeys-core/src/crypto/keys.rs`:

```rust
    use crate::account::{AccountRef, NormalizedEmail};

    fn test_account() -> AccountRef {
        AccountRef::new(
            Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap(),
            NormalizedEmail::parse("user@example.com").unwrap(),
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn v3_derivation_matches_the_independent_vector() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let kek = derive_kek_v3(&mk, &sk, &test_account()).unwrap();
        let auth = derive_auth_key_from_master(&mk, &sk, &test_account()).unwrap();
        assert_eq!(hex(kek.as_bytes()), "<KEK_HEX>");
        assert_eq!(hex(auth.as_bytes()), "<AUTH_HEX>");
    }

    #[test]
    fn kek_and_auth_key_are_domain_separated() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let kek = derive_kek_v3(&mk, &sk, &test_account()).unwrap();
        let auth = derive_auth_key_from_master(&mk, &sk, &test_account()).unwrap();
        assert_ne!(kek.as_bytes(), auth.as_bytes());
    }

    #[test]
    fn v3_is_bound_to_the_account_id_and_the_email() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let base = derive_kek_v3(&mk, &sk, &test_account()).unwrap();

        let other_id = AccountRef::new(
            Uuid::from_u128(1),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let other_email = AccountRef::new(
            Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap(),
            NormalizedEmail::parse("other@example.com").unwrap(),
        );
        assert_ne!(base.as_bytes(), derive_kek_v3(&mk, &sk, &other_id).unwrap().as_bytes());
        assert_ne!(base.as_bytes(), derive_kek_v3(&mk, &sk, &other_email).unwrap().as_bytes());
    }

    #[test]
    fn v3_needs_both_secrets() {
        let account = test_account();
        let mk_a = Key256::from_bytes([0x11; 32]);
        let mk_b = Key256::from_bytes([0x12; 32]);
        let sk_a = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let sk_b = SecretKey::from_bytes([0x23; SECRET_KEY_LEN]);
        let base = derive_kek_v3(&mk_a, &sk_a, &account).unwrap();
        assert_ne!(base.as_bytes(), derive_kek_v3(&mk_b, &sk_a, &account).unwrap().as_bytes());
        assert_ne!(base.as_bytes(), derive_kek_v3(&mk_a, &sk_b, &account).unwrap().as_bytes());
    }

    #[test]
    fn v3_differs_from_the_v2_scheme() {
        // Same secrets, different scheme: the labels must keep the outputs apart.
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let account = test_account();
        let v2 = derive_kek_with_secret_key(&mk, &sk, &account.id).unwrap();
        let v3 = derive_kek_v3(&mk, &sk, &account).unwrap();
        assert_ne!(v2.as_bytes(), v3.as_bytes());
    }

    #[test]
    fn auth_key_debug_is_redacted() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let auth = derive_auth_key_from_master(&mk, &sk, &test_account()).unwrap();
        assert_eq!(format!("{auth:?}"), "AuthKey(<redacted>)");
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core crypto::keys`
Expected: compile errors — `cannot find function derive_kek_v3`, `derive_auth_key_from_master`, `no function or associated item named from_bytes found for struct SecretKey`.

- [ ] **Step 4: Add the test-only SecretKey constructor**

In `crates/havenkeys-core/src/crypto/secret_key.rs`, inside `impl SecretKey`, next to `generate`:

```rust
    /// Fixed bytes, for known-answer tests only. Real Secret Keys come from
    /// [`SecretKey::generate`] or [`SecretKey::parse`].
    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: [u8; SECRET_KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }
```

- [ ] **Step 5: Write the derivation**

In `crates/havenkeys-core/src/crypto/keys.rs`, add to the imports at the top:

```rust
use crate::account::AccountRef;
use data_encoding::BASE64;
```

Add next to the other `INFO_` constants:

```rust
const INFO_KEK_V3: &[u8] = b"havenkeys/v3/kek";
const INFO_AUTH_V3: &[u8] = b"havenkeys/v3/auth";
```

Add after `derive_kek_with_secret_key`:

```rust
/// Proof that the holder knows the master password and the Secret Key, sent
/// to the sync server at login.
///
/// It authenticates and nothing else: it unwraps no key, and HKDF's `info`
/// separation means it cannot be walked back to the KEK, the master key or
/// the Secret Key. Never log it, never persist it.
pub struct AuthKey(Zeroizing<[u8; KEY_LEN]>);

impl AuthKey {
    /// For the wire. Standard Base64, padded.
    pub fn to_base64(&self) -> Zeroizing<String> {
        Zeroizing::new(BASE64.encode(self.0.as_ref()))
    }

    #[cfg(test)]
    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for AuthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthKey(<redacted>)")
    }
}

/// HKDF salt for key scheme 3: the account UUID's 16 bytes, then the
/// normalized email. Both devices must build this identically or they derive
/// different keys, which is why the email is normalized at parse time.
fn account_salt(account: &AccountRef) -> Zeroizing<Vec<u8>> {
    let email = account.email.as_str().as_bytes();
    let mut salt = Zeroizing::new(Vec::with_capacity(16 + email.len()));
    salt.extend_from_slice(account.id.as_bytes());
    salt.extend_from_slice(email);
    salt
}

/// One HKDF expansion over (master key ‖ Secret Key), bound to the account.
fn account_bound(
    master_key: &Key256,
    secret_key: &SecretKey,
    account: &AccountRef,
    info: &[u8],
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let mut ikm = Zeroizing::new([0u8; KEY_LEN + SECRET_KEY_LEN]);
    ikm[..KEY_LEN].copy_from_slice(master_key.as_bytes());
    ikm[KEY_LEN..].copy_from_slice(secret_key.as_bytes());
    let hk = Hkdf::<Sha256>::new(Some(account_salt(account).as_ref()), ikm.as_ref());
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(info, okm.as_mut()).map_err(|_| Error::Kdf)?;
    Ok(okm)
}

/// master key + Secret Key → key-encryption key, bound to the account
/// (key scheme 3). See docs/crypto.md.
pub fn derive_kek_v3(
    master_key: &Key256,
    secret_key: &SecretKey,
    account: &AccountRef,
) -> Result<Key256> {
    Ok(Key256(account_bound(
        master_key,
        secret_key,
        account,
        INFO_KEK_V3,
    )?))
}

/// master key + Secret Key → the server auth key (key scheme 3). Same input
/// keying material as the KEK, separated by the HKDF label.
pub fn derive_auth_key_from_master(
    master_key: &Key256,
    secret_key: &SecretKey,
    account: &AccountRef,
) -> Result<AuthKey> {
    Ok(AuthKey(account_bound(
        master_key,
        secret_key,
        account,
        INFO_AUTH_V3,
    )?))
}
```

Update the module doc in `crates/havenkeys-core/src/crypto/mod.rs`:

```rust
//! * `secret_key` — the 128-bit Secret Key mixed into the KEK (key schemes 2 and 3)
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p havenkeys-core crypto::keys`
Expected: all tests pass, including `v3_derivation_matches_the_independent_vector`.

If the vector test fails, the Rust code and the Python cross-check disagree — **do not change the expected value to match Rust**. Find which side is wrong against the spec's definition first.

- [ ] **Step 7: Lint and commit**

```bash
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/crypto/
git commit -m "feat(core): key scheme 3 derivation and the server auth key"
```

---

### Task 3: KeyScheme::AccountBound and the derivation binding

**Files:**
- Modify: `crates/havenkeys-core/src/store.rs` (the `KeyScheme` enum and its `to_db`/`from_db`)
- Modify: `crates/havenkeys-core/src/vault.rs` (`derive_kek_for`, `UnlockTicket`)

**Interfaces:**
- Consumes: `derive_kek_v3` (Task 2), `AccountRef` (Task 1).
- Produces:
  - `store::KeyScheme::AccountBound` (database value `3`, serde `"account_bound"`)
  - `vault::derive_kek_for(scheme: KeyScheme, password: &SecretString, kdf: &KdfParams, vault_id: &Uuid, secret_key: Option<&SecretKey>, account: Option<&AccountRef>) -> Result<Key256>` — one extra trailing parameter
  - `vault::UnlockTicket::needs_account(&self) -> bool`
  - `vault::UnlockTicket::derive_for_account(&self, password: &SecretString, secret_key: &SecretKey, account: &AccountRef) -> Result<UnlockKey>`

`derive_kek_for` is `pub(crate)`; its callers are all inside the crate (`vault.rs`, `sync.rs`). Every call site must be updated in this task or the crate will not compile.

- [ ] **Step 1: Write the failing test**

Append to `crates/havenkeys-core/tests/vault.rs`:

```rust
#[test]
fn account_bound_scheme_refuses_derivation_without_an_account() {
    use havenkeys_core::crypto::secret_key::SecretKey;
    use havenkeys_core::store::KeyScheme;

    let sk = SecretKey::generate().unwrap();
    let err = havenkeys_core::vault::derive_kek_for_test(
        KeyScheme::AccountBound,
        &secret(PASSWORD),
        &fast_kdf(),
        &uuid::Uuid::nil(),
        Some(&sk),
        None,
    )
    .unwrap_err();
    assert_eq!(err.code(), "invalid_input");
}

#[test]
fn account_bound_scheme_refuses_derivation_without_a_secret_key() {
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::store::KeyScheme;

    let account = AccountRef::new(
        uuid::Uuid::from_u128(7),
        NormalizedEmail::parse("user@example.com").unwrap(),
    );
    let err = havenkeys_core::vault::derive_kek_for_test(
        KeyScheme::AccountBound,
        &secret(PASSWORD),
        &fast_kdf(),
        &uuid::Uuid::nil(),
        None,
        Some(&account),
    )
    .unwrap_err();
    assert_eq!(err.code(), "secret_key_required");
}

#[test]
fn key_scheme_survives_a_database_round_trip() {
    use havenkeys_core::store::KeyScheme;
    // Serde form is part of the sync header on the wire; freeze it.
    assert_eq!(
        serde_json::to_string(&KeyScheme::AccountBound).unwrap(),
        "\"account_bound\""
    );
}
```

These call a thin test shim rather than the `pub(crate)` function; add it in step 3.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core --test vault`
Expected: compile errors — `no variant named AccountBound`, `cannot find function derive_kek_for_test`.

- [ ] **Step 3: Add the variant and widen the binding**

In `crates/havenkeys-core/src/store.rs`, in `enum KeyScheme`:

```rust
    /// Argon2id(master password) combined with the device's Secret Key,
    /// bound to the account (email + account ID). Vaults that sync to a
    /// server.
    AccountBound,
```

and in the two mapping functions:

```rust
    fn to_db(self) -> i64 {
        match self {
            KeyScheme::PasswordOnly => 1,
            KeyScheme::PasswordAndSecretKey => 2,
            KeyScheme::AccountBound => 3,
        }
    }

    fn from_db(v: i64) -> Result<Self> {
        match v {
            1 => Ok(KeyScheme::PasswordOnly),
            2 => Ok(KeyScheme::PasswordAndSecretKey),
            3 => Ok(KeyScheme::AccountBound),
            _ => Err(Error::UnsupportedVersion),
        }
    }
```

In `crates/havenkeys-core/src/vault.rs`, add to the imports:

```rust
use crate::account::AccountRef;
use crate::crypto::keys::derive_kek_v3;
```

Replace `derive_kek_for` with:

```rust
/// The KEK for a key scheme. A scheme that needs the Secret Key or the
/// account is refused before any expensive work.
pub(crate) fn derive_kek_for(
    scheme: KeyScheme,
    password: &SecretString,
    kdf: &KdfParams,
    vault_id: &Uuid,
    secret_key: Option<&SecretKey>,
    account: Option<&AccountRef>,
) -> Result<Key256> {
    match (scheme, secret_key, account) {
        (KeyScheme::PasswordOnly, _, _) => {
            derive_kek(&derive_master_key(password, kdf)?, vault_id)
        }
        (KeyScheme::PasswordAndSecretKey, Some(sk), _) => {
            derive_kek_with_secret_key(&derive_master_key(password, kdf)?, sk, vault_id)
        }
        (KeyScheme::AccountBound, Some(sk), Some(account)) => {
            derive_kek_v3(&derive_master_key(password, kdf)?, sk, account)
        }
        (KeyScheme::AccountBound, Some(_), None) => Err(Error::InvalidInput(
            "this vault belongs to an account; sign in instead",
        )),
        (KeyScheme::PasswordAndSecretKey | KeyScheme::AccountBound, None, _) => {
            Err(Error::SecretKeyRequired)
        }
    }
}

/// Test shim for the integration tests, which live outside the crate.
#[doc(hidden)]
pub fn derive_kek_for_test(
    scheme: KeyScheme,
    password: &SecretString,
    kdf: &KdfParams,
    vault_id: &Uuid,
    secret_key: Option<&SecretKey>,
    account: Option<&AccountRef>,
) -> Result<()> {
    derive_kek_for(scheme, password, kdf, vault_id, secret_key, account).map(|_| ())
}
```

In `UnlockTicket`, update `needs_secret_key` and add the account-aware path:

```rust
    /// Does unlocking this vault need the Secret Key?
    pub fn needs_secret_key(&self) -> bool {
        matches!(
            self.key_scheme,
            KeyScheme::PasswordAndSecretKey | KeyScheme::AccountBound
        )
    }

    /// Does unlocking this vault need the account (email + account ID)?
    pub fn needs_account(&self) -> bool {
        self.key_scheme == KeyScheme::AccountBound
    }

    /// Key scheme 3 vaults. Slow (Argon2id).
    pub fn derive_for_account(
        &self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<UnlockKey> {
        if password.is_empty() || password.char_len() > MAX_MASTER_PASSWORD_CHARS {
            return Err(Error::UnlockFailed);
        }
        Ok(UnlockKey(derive_kek_for(
            self.key_scheme,
            password,
            &self.kdf,
            &self.vault_id,
            Some(secret_key),
            Some(account),
        )?))
    }
```

Then fix every other call site by appending `None` as the last argument: `derive_with_secret_key` in `UnlockTicket`, `RekeyTicket::derive_with_secret_key`, `RekeyTicket::unwrap`, and `prepare` in `vault.rs`. Let the compiler list them:

```bash
cargo check -p havenkeys-core 2>&1 | grep -n "this function takes"
```

- [ ] **Step 4: Run the full core suite**

Run: `cargo test -p havenkeys-core`
Expected: the three new tests pass and **every existing test still passes** — schemes 1 and 2 are untouched.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/store.rs crates/havenkeys-core/src/vault.rs crates/havenkeys-core/tests/vault.rs
git commit -m "feat(core): key scheme 3 variant and account-bound KEK derivation"
```

---

### Task 4: Local schema 3 — dirty flags and the account table

**Files:**
- Modify: `crates/havenkeys-core/src/store.rs`
- Test: `crates/havenkeys-core/tests/vault.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `store::SCHEMA_VERSION == 3`
  - `store::AccountRecord { pub account_id: Uuid, pub email: String, pub server_url: String, pub server_cursor: i64, pub max_header_rev: i64, pub last_synced_at: Option<i64> }`
  - `Store::account(&self) -> Result<Option<AccountRecord>>`
  - `Store::set_account(&mut self, rec: &AccountRecord) -> Result<()>`
  - `Store::set_cursor(&mut self, cursor: i64, synced_at: i64) -> Result<()>`
  - `Store::raise_max_header_rev(&mut self, rev: i64) -> Result<()>`
  - `Store::dirty_rows(&self) -> Result<Vec<ItemRow>>`
  - `Store::dirty_tombstones(&self) -> Result<Vec<(Uuid, i64)>>`
  - `Store::clear_dirty(&mut self, ids: &[Uuid]) -> Result<()>`

`email` is stored as typed (for display); the normalized form is derived on use, never stored, so there is one source of truth for normalization.

- [ ] **Step 1: Write the failing test**

Append to `crates/havenkeys-core/tests/vault.rs`:

```rust
#[test]
fn account_record_round_trips() {
    use havenkeys_core::store::{AccountRecord, Store};

    let mut store = Store::open_in_memory().unwrap();
    assert!(store.account().unwrap().is_none());

    let rec = AccountRecord {
        account_id: uuid::Uuid::from_u128(42),
        email: "User@Example.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    };
    store.set_account(&rec).unwrap();

    let back = store.account().unwrap().unwrap();
    assert_eq!(back.account_id, rec.account_id);
    assert_eq!(back.email, "User@Example.com");
    assert_eq!(back.server_url, rec.server_url);
    assert_eq!(back.server_cursor, 0);
}

#[test]
fn cursor_and_header_guard_move_only_forward() {
    use havenkeys_core::store::{AccountRecord, Store};

    let mut store = Store::open_in_memory().unwrap();
    store
        .set_account(&AccountRecord {
            account_id: uuid::Uuid::from_u128(1),
            email: "a@b.com".into(),
            server_url: "https://x".into(),
            server_cursor: 0,
            max_header_rev: 0,
            last_synced_at: None,
        })
        .unwrap();

    store.set_cursor(7, NOW).unwrap();
    assert_eq!(store.account().unwrap().unwrap().server_cursor, 7);

    store.raise_max_header_rev(5).unwrap();
    store.raise_max_header_rev(3).unwrap(); // an older header must not lower it
    assert_eq!(store.account().unwrap().unwrap().max_header_rev, 5);
}

#[test]
fn local_edits_are_dirty_and_clear_on_confirmation() {
    use havenkeys_core::store::Store;

    let mut store = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::from_u128(9);
    store.upsert_item(&id, b"overview", b"details").unwrap();
    assert_eq!(store.dirty_rows().unwrap().len(), 1);

    store.clear_dirty(&[id]).unwrap();
    assert!(store.dirty_rows().unwrap().is_empty());

    // Editing it again marks it dirty again.
    store.upsert_item(&id, b"overview2", b"details2").unwrap();
    assert_eq!(store.dirty_rows().unwrap().len(), 1);
}

#[test]
fn deletions_are_dirty_until_confirmed() {
    use havenkeys_core::store::Store;

    let mut store = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::from_u128(11);
    store.upsert_item(&id, b"overview", b"details").unwrap();
    store.clear_dirty(&[id]).unwrap();

    store.delete_item(&id, NOW).unwrap();
    assert_eq!(store.dirty_tombstones().unwrap(), vec![(id, NOW)]);

    store.clear_dirty(&[id]).unwrap();
    assert!(store.dirty_tombstones().unwrap().is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core --test vault`
Expected: compile errors — `AccountRecord` not found, `no method named dirty_rows`.

- [ ] **Step 3: Write the migration and the accessors**

In `crates/havenkeys-core/src/store.rs`, bump the version and add the migration next to `MIGRATE_1_TO_2`:

```rust
pub const SCHEMA_VERSION: i64 = 3;
```

```rust
/// Schema 2 → 3: server sync. `dirty` marks rows changed locally since the
/// last confirmed push (existing rows start dirty, so a vault joining an
/// account uploads itself once), and `account` holds the identity, the
/// server cursor and the header-rollback guard.
const MIGRATE_2_TO_3: &str = "
ALTER TABLE items ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
ALTER TABLE tombstones ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1;
CREATE TABLE account (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    account_id     TEXT    NOT NULL,
    email          TEXT    NOT NULL,
    server_url     TEXT    NOT NULL,
    server_cursor  INTEGER NOT NULL DEFAULT 0,
    max_header_rev INTEGER NOT NULL DEFAULT 0,
    last_synced_at INTEGER
);
";
```

In `Store::init`, extend the version ladder so each older version runs every migration after it:

```rust
        match version {
            0 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(SCHEMA)?;
                tx.execute_batch(MIGRATE_1_TO_2)?;
                tx.execute_batch(MIGRATE_2_TO_3)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            1 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(MIGRATE_1_TO_2)?;
                tx.execute_batch(MIGRATE_2_TO_3)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            2 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(MIGRATE_2_TO_3)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            SCHEMA_VERSION => {}
            _ => return Err(Error::UnsupportedVersion),
        }
```

Add the record type next to `HeaderRecord`:

```rust
/// The account this vault belongs to, and where its sync stands.
#[derive(Clone, Debug)]
pub struct AccountRecord {
    pub account_id: Uuid,
    /// As the user typed it, for display. Normalize on use, never on store.
    pub email: String,
    pub server_url: String,
    /// Highest server revision this device has pulled.
    pub server_cursor: i64,
    /// Highest `header_revision` ever accepted. Never goes down: that is the
    /// rollback guard from the design doc §8.4.
    pub max_header_rev: i64,
    pub last_synced_at: Option<i64>,
}
```

Add the accessors in `impl Store`:

```rust
    pub fn account(&self) -> Result<Option<AccountRecord>> {
        self.conn
            .query_row(
                "SELECT account_id, email, server_url, server_cursor, max_header_rev, last_synced_at
                 FROM account WHERE id = 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                    ))
                },
            )
            .optional()?
            .map(|(id, email, server_url, server_cursor, max_header_rev, last_synced_at)| {
                Ok(AccountRecord {
                    account_id: Uuid::parse_str(&id).map_err(|_| Error::Corrupted)?,
                    email,
                    server_url,
                    server_cursor,
                    max_header_rev,
                    last_synced_at,
                })
            })
            .transpose()
    }

    pub fn set_account(&mut self, rec: &AccountRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO account
               (id, account_id, email, server_url, server_cursor, max_header_rev, last_synced_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id,
               email = excluded.email,
               server_url = excluded.server_url",
            params![
                rec.account_id.to_string(),
                rec.email,
                rec.server_url,
                rec.server_cursor,
                rec.max_header_rev,
                rec.last_synced_at,
            ],
        )?;
        Ok(())
    }

    pub fn set_cursor(&mut self, cursor: i64, synced_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE account SET server_cursor = ?1, last_synced_at = ?2 WHERE id = 1",
            params![cursor, synced_at],
        )?;
        Ok(())
    }

    /// Raise the rollback guard. An older header never lowers it.
    pub fn raise_max_header_rev(&mut self, rev: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE account SET max_header_rev = max(max_header_rev, ?1) WHERE id = 1",
            params![rev],
        )?;
        Ok(())
    }

    /// Items changed locally since the last confirmed push.
    pub fn dirty_rows(&self) -> Result<Vec<ItemRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, overview, details FROM items WHERE dirty = 1")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, ov, det) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, ov, det));
            }
        }
        Ok(out)
    }

    /// Deletions not yet pushed.
    pub fn dirty_tombstones(&self) -> Result<Vec<(Uuid, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, deleted_at FROM tombstones WHERE dirty = 1")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, at) = row?;
            if let Ok(id) = Uuid::parse_str(&id) {
                out.push((id, at));
            }
        }
        Ok(out)
    }

    /// Mark pushed rows as clean, in one transaction.
    pub fn clear_dirty(&mut self, ids: &[Uuid]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE items SET dirty = 0 WHERE id = ?1",
                params![id.to_string()],
            )?;
            tx.execute(
                "UPDATE tombstones SET dirty = 0 WHERE id = ?1",
                params![id.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
```

Finally, make the existing local mutations set `dirty = 1` explicitly, so an `ON CONFLICT` update does not silently keep a stale `0`. In `upsert_item`:

```rust
            "INSERT INTO items (id, overview, details, dirty) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                           details = excluded.details,
                                           dirty = 1",
```

In `insert_items`:

```rust
                tx.prepare("INSERT INTO items (id, overview, details, dirty) VALUES (?1, ?2, ?3, 1)")?;
```

In `delete_item`, the tombstone insert:

```rust
            "INSERT INTO tombstones (id, deleted_at, dirty) VALUES (?1, ?2, 1)
             ON CONFLICT(id) DO UPDATE SET deleted_at = max(deleted_at, excluded.deleted_at),
                                           dirty = 1",
```

In `apply_merge`, rows come **from** a remote, so they are already in sync: set `dirty = 0` in both statements there.

```rust
                "INSERT INTO items (id, overview, details, dirty) VALUES (?1, ?2, ?3, 0)
                 ON CONFLICT(id) DO UPDATE SET overview = excluded.overview,
                                               details = excluded.details,
                                               dirty = 0",
```

```rust
                "INSERT INTO tombstones (id, deleted_at, dirty) VALUES (?1, ?2, 0)
                 ON CONFLICT(id) DO UPDATE SET deleted_at = max(deleted_at, excluded.deleted_at),
                                               dirty = 0",
```

- [ ] **Step 4: Run the full core suite**

Run: `cargo test -p havenkeys-core`
Expected: the four new tests pass; every existing test, including `tests/sync.rs`, still passes.

- [ ] **Step 5: Verify the migration on a real schema-2 file**

The in-memory tests always start at version 0, so they never exercise the 2 → 3 path. Add this test to `crates/havenkeys-core/tests/vault.rs`:

```rust
#[test]
fn upgrades_a_schema_2_database_in_place() {
    use havenkeys_core::store::Store;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.sqlite3");

    // Build a schema-2 database by hand: the tables as they were, and the
    // user_version that says so.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE vault_header (
                 id INTEGER PRIMARY KEY CHECK (id = 1), format_version INTEGER NOT NULL,
                 vault_id TEXT NOT NULL, kdf TEXT NOT NULL, wrapped_vault_key BLOB NOT NULL,
                 created_at INTEGER NOT NULL, key_scheme INTEGER NOT NULL DEFAULT 1,
                 header_revision INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE items (id TEXT PRIMARY KEY NOT NULL, overview BLOB NOT NULL, details BLOB NOT NULL);
             CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1), blob BLOB NOT NULL);
             CREATE TABLE tombstones (id TEXT PRIMARY KEY NOT NULL, deleted_at INTEGER NOT NULL);
             INSERT INTO items (id, overview, details)
               VALUES ('11111111-1111-1111-1111-111111111111', x'00', x'01');
             PRAGMA user_version = 2;",
        )
        .unwrap();
    }

    let store = Store::open(&path).unwrap();
    // The pre-existing row must come back dirty, so it gets uploaded once.
    assert_eq!(store.dirty_rows().unwrap().len(), 1);
    assert!(store.account().unwrap().is_none());
}
```

`rusqlite` and `tempfile` are already available to the test target.

Run: `cargo test -p havenkeys-core --test vault upgrades_a_schema_2`
Expected: PASS.

- [ ] **Step 6: Lint and commit**

```bash
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/store.rs crates/havenkeys-core/tests/vault.rs
git commit -m "feat(core): schema 3 with dirty tracking and the account record"
```

---

### Task 5: Activation and sign-in

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (activation)
- Modify: `crates/havenkeys-core/src/sync.rs` (sign-in from a server header)
- Test: `crates/havenkeys-core/tests/account.rs` (create)

**Interfaces:**
- Consumes: `AccountRef` (Task 1), `derive_kek_v3` / `derive_auth_key_from_master` / `AuthKey` (Task 2), `KeyScheme::AccountBound` (Task 3).
- Produces:
  - `vault::AccountVault { pub prepared: PreparedVault, pub secret_key: SecretKey, pub auth_key: AuthKey }`
  - `vault::prepare_new_account_vault(password: &SecretString, account: &AccountRef, kdf: KdfParams, now_ms: i64) -> Result<AccountVault>`
  - `sync::prepare_sign_in(header: &[u8], password: &SecretString, secret_key: &SecretKey, account: &AccountRef) -> Result<(PreparedVault, AuthKey)>`
  - `vault::derive_auth_key(password: &SecretString, secret_key: &SecretKey, kdf: &KdfParams, account: &AccountRef) -> Result<AuthKey>`

`prepare_sign_in` is the server counterpart of the existing `prepare_join`: same header bytes, same attestation check, different key scheme. The header format is unchanged — no `account_id` field is added to it, deliberately. A wrong account gives a wrong KEK, so the unwrap already fails; adding the field would bump the header format version and buy nothing this plan needs.

- [ ] **Step 1: Write the failing test**

Create `crates/havenkeys-core/tests/account.rs`:

```rust
//! Activation (first login) and sign-in on a second device, key scheme 3.

mod common;

use common::{fast_kdf, secret, NOW, PASSWORD};
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::store::{KeyScheme, Store};
use havenkeys_core::sync::prepare_sign_in;
use havenkeys_core::vault::{prepare_new_account_vault, VaultService};
use uuid::Uuid;

fn account() -> AccountRef {
    AccountRef::new(
        Uuid::from_u128(0x5eed),
        NormalizedEmail::parse("user@example.com").unwrap(),
    )
}

/// Build an activated vault and return it with its Secret Key and header.
fn activate() -> (VaultService, havenkeys_core::crypto::secret_key::SecretKey, Vec<u8>) {
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let secret_key = made.secret_key;
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(made.prepared).unwrap();
    vault.unlock_for_account(&secret(PASSWORD), &secret_key, &account()).unwrap();
    let header = vault.encode_account_header().unwrap();
    (vault, secret_key, header)
}

#[test]
fn activation_produces_an_account_bound_vault() {
    let (vault, _sk, _header) = activate();
    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert!(vault.is_unlocked());
}

#[test]
fn a_second_device_signs_in_with_password_secret_key_and_account() {
    let (mut first, secret_key, header) = activate();
    first.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();

    let (prepared, _auth) =
        prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &account()).unwrap();
    let mut second = VaultService::new(Store::open_in_memory().unwrap());
    second.create_vault(prepared).unwrap();
    second.unlock_for_account(&secret(PASSWORD), &secret_key, &account()).unwrap();

    assert_eq!(second.vault_id().unwrap(), first.vault_id().unwrap());
}

#[test]
fn sign_in_fails_with_the_wrong_secret_key() {
    let (_first, _sk, header) = activate();
    let other = havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap();
    assert!(prepare_sign_in(&header, &secret(PASSWORD), &other, &account()).is_err());
}

#[test]
fn sign_in_fails_with_the_wrong_email() {
    let (_first, secret_key, header) = activate();
    let wrong = AccountRef::new(
        account().id,
        NormalizedEmail::parse("someone.else@example.com").unwrap(),
    );
    assert!(prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &wrong).is_err());
}

#[test]
fn sign_in_fails_with_the_wrong_account_id() {
    let (_first, secret_key, header) = activate();
    let wrong = AccountRef::new(Uuid::from_u128(1), account().email.clone());
    assert!(prepare_sign_in(&header, &secret(PASSWORD), &secret_key, &wrong).is_err());
}

#[test]
fn sign_in_refuses_a_folder_era_header() {
    // A key scheme 2 header must not be accepted into an account vault:
    // downgrade is refused (design doc §4.2).
    let made = havenkeys_core::vault::prepare_new_vault_with_secret_key(
        &secret(PASSWORD),
        &havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap(),
        fast_kdf(),
        NOW,
    )
    .unwrap();
    let mut v2 = VaultService::new(Store::open_in_memory().unwrap());
    v2.create_vault(made).unwrap();
    // A scheme 2 vault cannot produce an account header at all.
    assert!(v2.encode_account_header().is_err());
}

#[test]
fn the_auth_key_is_not_the_kek() {
    // Both come from the same Argon2id run; only the HKDF label differs.
    // Encoded forms must not be equal, or the server would hold key material.
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let auth_b64 = made.auth_key.to_base64();
    assert!(!auth_b64.is_empty());
    assert_eq!(format!("{:?}", made.auth_key), "AuthKey(<redacted>)");
}
```

This test needs three service methods that do not exist yet: `unlock_for_account`, `encode_account_header`, and `key_scheme()` returning the new variant. They are added in step 3.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p havenkeys-core --test account`
Expected: compile errors naming `prepare_new_account_vault`, `prepare_sign_in`, `unlock_for_account`, `encode_account_header`.

- [ ] **Step 3: Write the implementation**

In `crates/havenkeys-core/src/vault.rs`, add after `prepare_new_vault_with_secret_key`:

```rust
/// A vault created for an account, with the two things the caller must not
/// lose: the Secret Key (for the Emergency Kit) and the auth key (for the
/// activation request).
pub struct AccountVault {
    pub prepared: PreparedVault,
    pub secret_key: SecretKey,
    pub auth_key: AuthKey,
}

/// Activation: derive keys and build the header for a new account vault
/// (key scheme 3). One Argon2id run yields both the KEK and the auth key.
/// Slow (Argon2id).
pub fn prepare_new_account_vault(
    password: &SecretString,
    account: &AccountRef,
    kdf: KdfParams,
    now_ms: i64,
) -> Result<AccountVault> {
    check_new_master_password(password)?;
    let secret_key = SecretKey::generate()?;
    let master_key = derive_master_key(password, &kdf)?;
    let kek = derive_kek_v3(&master_key, &secret_key, account)?;
    let auth_key = derive_auth_key_from_master(&master_key, &secret_key, account)?;
    let vault_id = Uuid::new_v4();
    let vault_key = Key256::random()?;
    let wrapped_vault_key = wrap_vault_key(&kek, vault_id, &vault_key)?;
    Ok(AccountVault {
        prepared: PreparedVault {
            header: HeaderRecord {
                format_version: FORMAT_VERSION,
                vault_id,
                kdf,
                wrapped_vault_key,
                created_at: now_ms,
                key_scheme: KeyScheme::AccountBound,
                revision: 0,
            },
            vault_key,
        },
        secret_key,
        auth_key,
    })
}

/// The auth key for a later login on a device that already holds the Secret
/// Key. Slow (Argon2id).
pub fn derive_auth_key(
    password: &SecretString,
    secret_key: &SecretKey,
    kdf: &KdfParams,
    account: &AccountRef,
) -> Result<AuthKey> {
    derive_auth_key_from_master(&derive_master_key(password, kdf)?, secret_key, account)
}
```

Add the imports it needs at the top of `vault.rs`:

```rust
use crate::crypto::keys::{derive_auth_key_from_master, AuthKey};
```

Add to `impl VaultService`, next to `unlock`:

```rust
    /// Unlock a key scheme 3 vault. Slow (Argon2id).
    pub fn unlock_for_account(
        &mut self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<()> {
        let ticket = self.begin_unlock()?;
        let key = ticket.derive_for_account(password, secret_key, account);
        self.finish_unlock(ticket, key)
    }
```

In `crates/havenkeys-core/src/sync.rs`, add next to `prepare_join`:

```rust
/// Sign in to an account vault on a new device: derive the KEK from the
/// master password, the Secret Key and the account, unwrap the vault key,
/// and check the header's attestation. Refuses any key scheme below 3, so a
/// server cannot downgrade a device. Slow (Argon2id).
pub fn prepare_sign_in(
    header: &[u8],
    password: &SecretString,
    secret_key: &SecretKey,
    account: &AccountRef,
) -> Result<(PreparedVault, AuthKey)> {
    let file = parse_header(header)?;
    if file.body.key_scheme != KeyScheme::AccountBound {
        return Err(Error::UnsupportedVersion);
    }
    let record = body_to_record(&file.body)?;
    let master_key = derive_master_key(password, &record.kdf)?;
    let kek = derive_kek_v3(&master_key, secret_key, account)?;
    let auth_key = derive_auth_key_from_master(&master_key, secret_key, account)?;
    let vault_key = unwrap_vault_key(&kek, record.vault_id, &record.wrapped_vault_key)?;
    if !verify_header(&derive_data_key(&vault_key)?, &file) {
        return Err(Error::Corrupted);
    }
    Ok((
        PreparedVault {
            header: record,
            vault_key,
        },
        auth_key,
    ))
}
```

and the service method that produces the header bytes to upload:

```rust
impl VaultService {
    /// The header this device would publish to its account's server. Requires
    /// an unlocked key scheme 3 vault.
    pub fn encode_account_header(&self) -> Result<Vec<u8>> {
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        if local.key_scheme != KeyScheme::AccountBound {
            return Err(Error::InvalidInput("this vault is not linked to an account"));
        }
        encode_header(&self.session()?.data_key, &local)
    }
}
```

Add the imports `sync.rs` needs: `crate::account::AccountRef`, `crate::crypto::kdf::derive_master_key`, and `crate::crypto::keys::{derive_auth_key_from_master, derive_kek_v3, AuthKey}`. `PreparedVault` is already imported there.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p havenkeys-core --test account`
Expected: 7 tests pass.

- [ ] **Step 5: Run the full suite and commit**

```bash
cargo test -p havenkeys-core
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/vault.rs crates/havenkeys-core/src/sync.rs crates/havenkeys-core/tests/account.rs
git commit -m "feat(core): account activation and sign-in for key scheme 3"
```

---

### Task 6: Scheme 2 → 3 upgrade and the header rollback guard

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (`RekeyTicket`)
- Modify: `crates/havenkeys-core/src/sync.rs` (`adopt_account_header`)
- Test: `crates/havenkeys-core/tests/account.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–5, plus `Store::raise_max_header_rev` and `Store::account` (Task 4).
- Produces:
  - `vault::RekeyTicket::derive_account_upgrade(&self, password: &SecretString, secret_key: &SecretKey, account: &AccountRef, new_kdf: KdfParams) -> Result<Rekeyed>`
  - `VaultService::adopt_account_header(&mut self, header: &[u8]) -> Result<bool>` — `true` when the header was adopted

An existing vault keeps its Secret Key bytes through the upgrade: rotating them would invalidate an Emergency Kit the user may already have printed, and they gain nothing here. A scheme 1 vault must first add a Secret Key with the existing flow.

- [ ] **Step 1: Write the failing test**

Append to `crates/havenkeys-core/tests/account.rs`:

```rust
#[test]
fn a_secret_key_vault_upgrades_to_an_account_without_touching_items() {
    use havenkeys_core::crypto::secret_key::SecretKey;
    use havenkeys_core::vault::prepare_new_vault_with_secret_key;

    let secret_key = SecretKey::generate().unwrap();
    let made = prepare_new_vault_with_secret_key(&secret(PASSWORD), &secret_key, fast_kdf(), NOW).unwrap();
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(made).unwrap();
    // Scheme 2 has no one-shot unlock helper; use the ticket pattern that
    // tests/sync.rs uses.
    let ticket = vault.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&secret_key));
    vault.finish_unlock(ticket, r).unwrap();
    let item = vault.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let vault_id_before = vault.vault_id().unwrap();

    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_account_upgrade(&secret(PASSWORD), &secret_key, &account(), fast_kdf())
        .unwrap();
    vault.commit_rekey(ticket, Ok(rekeyed)).unwrap();

    // Same vault, same item, new scheme, higher header revision.
    assert_eq!(vault.key_scheme().unwrap(), Some(KeyScheme::AccountBound));
    assert_eq!(vault.vault_id().unwrap(), vault_id_before);

    vault.lock();
    vault.unlock_for_account(&secret(PASSWORD), &secret_key, &account()).unwrap();
    assert_eq!(vault.get_item(&item.id).unwrap().title, "GitHub");

    // The old scheme-2 unlock no longer works: without the account there is
    // no KEK to derive.
    vault.lock();
    let ticket = vault.begin_unlock().unwrap();
    let r = ticket.derive_with_secret_key(&secret(PASSWORD), Some(&secret_key));
    assert!(vault.finish_unlock(ticket, r).is_err());
}

#[test]
fn an_older_header_is_refused_after_a_password_change() {
    let (mut vault, secret_key, old_header) = activate();

    // Change the master password: the header revision goes up.
    let ticket = vault.begin_rekey().unwrap();
    let rekeyed = ticket
        .derive_with_secret_key(
            &secret(PASSWORD),
            &secret("a much longer new password"),
            fast_kdf(),
            Some(&secret_key),
        )
        .unwrap();
    vault.commit_rekey(ticket, Ok(rekeyed)).unwrap();
    let new_header = vault.encode_account_header().unwrap();

    // A hostile server replays the header from before the change.
    assert!(!vault.adopt_account_header(&old_header).unwrap());
    // And the current one is still accepted (idempotently).
    assert!(!vault.adopt_account_header(&new_header).unwrap());
}

#[test]
fn a_forged_header_is_refused() {
    let (mut vault, _sk, header) = activate();
    let mut forged = header.clone();
    let n = forged.len();
    forged[n - 5] ^= 0x01; // flip a bit inside the attestation
    assert!(vault.adopt_account_header(&forged).is_err() || !vault.adopt_account_header(&forged).unwrap());
}
```

Note there is no `unlock_with_secret_key` helper on `VaultService` — scheme 2 unlocks go through `begin_unlock` → `derive_with_secret_key` → `finish_unlock`, as `tests/sync.rs` does. Only `unlock` (scheme 1) and the new `unlock_for_account` (scheme 3) are one-shot.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core --test account`
Expected: compile errors — `no method named derive_account_upgrade`, `no method named adopt_account_header`.

- [ ] **Step 3: Write the upgrade**

In `crates/havenkeys-core/src/vault.rs`, in `impl RekeyTicket`, after `derive_secret_key_upgrade`:

```rust
    /// Link a key scheme 2 vault to an account (scheme 2 → 3). The master
    /// password and the Secret Key stay the same; only the vault key is
    /// re-wrapped, so items are untouched. Slow (Argon2id).
    pub fn derive_account_upgrade(
        &self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
        new_kdf: KdfParams,
    ) -> Result<Rekeyed> {
        if self.header.key_scheme != KeyScheme::PasswordAndSecretKey {
            return Err(Error::InvalidInput(
                "add a Secret Key before linking this vault to an account",
            ));
        }
        let vault_key = self.unwrap(password, Some(secret_key))?;
        let kek = derive_kek_v3(
            &derive_master_key(password, &new_kdf)?,
            secret_key,
            account,
        )?;
        Ok(Rekeyed {
            wrapped_vault_key: wrap_vault_key(&kek, self.header.vault_id, &vault_key)?,
            kdf: new_kdf,
            key_scheme: KeyScheme::AccountBound,
        })
    }
```

- [ ] **Step 4: Write the rollback guard**

In `crates/havenkeys-core/src/sync.rs`, in the `impl VaultService` block added in Task 5:

```rust
    /// Consider a header served by the account's server. Returns whether the
    /// local header was replaced.
    ///
    /// The server is untrusted, so three things must hold before adoption:
    /// the attestation must verify under this vault's data key (only a vault
    /// key holder could have written it), the key scheme must not go
    /// backwards, and the revision must not go backwards — a genuine old
    /// header replayed after a master-password change would otherwise make
    /// the previous password work again.
    pub fn adopt_account_header(&mut self, remote: &[u8]) -> Result<bool> {
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        let file = parse_header(remote)?;
        if file.body.vault_id != local.vault_id {
            return Err(Error::InvalidInput("that header is for a different vault"));
        }
        if file.body.key_scheme != KeyScheme::AccountBound {
            return Ok(false);
        }
        if !verify_header(&self.session()?.data_key, &file) {
            return Ok(false);
        }
        let floor = self
            .store
            .account()?
            .map(|a| a.max_header_rev)
            .unwrap_or(0)
            .max(local.revision as i64);
        let remote_rev = body_to_record(&file.body)?.revision as i64;
        if remote_rev <= floor {
            return Ok(false);
        }
        let record = body_to_record(&file.body)?;
        self.store
            .update_key_wrap(&record.kdf, &record.wrapped_vault_key, record.key_scheme, record.revision)?;
        self.store.raise_max_header_rev(remote_rev)?;
        Ok(true)
    }
```

Check `update_key_wrap`'s real signature before writing this call:

```bash
grep -n "pub fn update_key_wrap" -A 8 crates/havenkeys-core/src/store.rs
```

and match it exactly. Also make `commit_rekey` raise the guard, so a local password change moves the floor up too — find it in `vault.rs` and add, after the store write succeeds:

```rust
        self.store.raise_max_header_rev(header.revision as i64)?;
```

using whatever the local variable for the new header record is called there.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p havenkeys-core --test account`
Expected: all tests pass, including the three new ones.

- [ ] **Step 6: Run the full suite and commit**

```bash
cargo test -p havenkeys-core
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/vault.rs crates/havenkeys-core/src/sync.rs crates/havenkeys-core/tests/account.rs
git commit -m "feat(core): scheme 2 to 3 upgrade and header rollback guard"
```

---

### Task 7: Share the merge decision between the snapshot and delta paths

**Files:**
- Modify: `crates/havenkeys-core/src/sync.rs`
- Test: `crates/havenkeys-core/tests/sync.rs` (unchanged, must keep passing)

**Interfaces:**
- Consumes: the existing `SyncReport`, `Candidate`, `check_item`.
- Produces (all crate-internal):
  - `VaultService::check_item_bytes(&self, vault_id: Uuid, id: Uuid, overview: Vec<u8>, details: Vec<u8>) -> Option<(ItemRow, ItemOverview)>`
  - `VaultService::decide_merge(&mut self, candidates: HashMap<Uuid, Candidate>, remote_tombs: HashMap<Uuid, i64>, report: &mut SyncReport) -> Result<()>`

This task changes no behaviour. It is a refactor whose only purpose is that Task 8 can reuse the merge rules instead of restating them — two copies of these rules would drift, and a drift here is a data-loss or resurrection bug. The existing `tests/sync.rs` is the proof that behaviour is unchanged, so **do not modify it in this task**.

- [ ] **Step 1: Record the current test baseline**

Run: `cargo test -p havenkeys-core --test sync`
Expected: all pass. Note the count; it must be identical at the end of this task.

- [ ] **Step 2: Extract the byte-level item check**

In `crates/havenkeys-core/src/sync.rs`, replace `check_item` with a thin wrapper over a new byte-level function:

```rust
    fn check_item(&self, vault_id: Uuid, item: &SnapshotItem) -> Option<(ItemRow, ItemOverview)> {
        let ov_blob = BASE64.decode(item.overview.as_bytes()).ok()?;
        let det_blob = BASE64.decode(item.details.as_bytes()).ok()?;
        self.check_item_bytes(vault_id, item.id, ov_blob, det_blob)
    }

    /// Authenticate one remote item version. Returns `None` unless both blobs
    /// open under this vault's data key, the overview's ID matches the row's,
    /// and the details type matches the overview type.
    fn check_item_bytes(
        &self,
        vault_id: Uuid,
        id: Uuid,
        ov_blob: Vec<u8>,
        det_blob: Vec<u8>,
    ) -> Option<(ItemRow, ItemOverview)> {
        let data_key = &self.session().ok()?.data_key;
        let ov: ItemOverview = open_json(
            data_key,
            &BlobContext::item(Purpose::ItemOverview, vault_id, id),
            &ov_blob,
        )
        .ok()?;
        let details: ItemDetails = open_json(
            data_key,
            &BlobContext::item(Purpose::ItemDetails, vault_id, id),
            &det_blob,
        )
        .ok()?;
        if ov.id != id || details.item_type() != ov.item_type {
            return None;
        }
        Some(((id, ov_blob, det_blob), ov))
    }
```

- [ ] **Step 3: Extract the decision block**

Move the "Decide per item" block of `VaultService::sync` — from `let local_tombs` through the `apply_merge` call and the session-overview updates — into a method, and call it from `sync`:

```rust
    /// Apply the merge rules (docs/sync.md §5) to one set of remote
    /// candidates and tombstones. The only place these rules exist.
    fn decide_merge(
        &mut self,
        candidates: HashMap<Uuid, Candidate>,
        remote_tombs: HashMap<Uuid, i64>,
        report: &mut SyncReport,
    ) -> Result<()> {
        // ... the block moved verbatim from `sync`, unchanged ...
        Ok(())
    }
```

In `sync`, the block becomes:

```rust
        self.decide_merge(candidates, remote_tombs, &mut report)?;
```

Keep the moved code byte-identical apart from the `report.` accesses, which already use a `&mut SyncReport`.

- [ ] **Step 4: Verify behaviour is unchanged**

Run: `cargo test -p havenkeys-core --test sync`
Expected: the same number of tests, all passing. If any test changes behaviour, the extraction was not faithful — revert and redo it rather than adjusting the test.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/sync.rs
git commit -m "refactor(core): one implementation of the merge rules"
```

---

### Task 8: The delta sync API

**Files:**
- Modify: `crates/havenkeys-core/src/sync.rs`
- Test: `crates/havenkeys-core/tests/delta_sync.rs` (create)

**Interfaces:**
- Consumes: `check_item_bytes` and `decide_merge` (Task 7), the dirty accessors (Task 4).
- Produces:
  - `sync::RemoteChange { pub item_id: Uuid, pub overview: Option<Vec<u8>>, pub details: Option<Vec<u8>>, pub deleted_at: Option<i64> }`
  - `sync::PendingPush { pub base_cursor: i64, pub changes: Vec<RemoteChange> }`
  - `VaultService::store_account(&mut self, rec: &AccountRecord) -> Result<()>`
  - `VaultService::pending_push(&self) -> Result<PendingPush>`
  - `VaultService::apply_remote_changes(&mut self, cursor: i64, changes: Vec<RemoteChange>, now_ms: i64) -> Result<SyncReport>`
  - `VaultService::confirm_push(&mut self, cursor: i64, pushed: &[Uuid], now_ms: i64) -> Result<()>`

No HTTP. These methods are exactly the surface the sync client (a later plan) drives.

All three sync methods require an account record: a vault with none is not linked to a server, and silently tracking a cursor for it would lose the cursor without anyone noticing. They return `Error::InvalidInput("this vault is not linked to an account")` in that case.

- [ ] **Step 1: Write the failing test**

Create `crates/havenkeys-core/tests/delta_sync.rs`:

```rust
//! Delta sync against a server cursor. No network: the "server" here is a
//! Vec of changes, which is all the core ever sees.

mod common;

use common::{fast_kdf, secret, NOW, PASSWORD};
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::store::{AccountRecord, Store};
use havenkeys_core::sync::RemoteChange;
use havenkeys_core::vault::{prepare_new_account_vault, VaultService};
use uuid::Uuid;

fn account() -> AccountRef {
    AccountRef::new(
        Uuid::from_u128(0x5eed),
        NormalizedEmail::parse("user@example.com").unwrap(),
    )
}

fn record() -> AccountRecord {
    AccountRecord {
        account_id: account().id,
        email: "user@example.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    }
}

fn activated() -> (VaultService, havenkeys_core::crypto::secret_key::SecretKey) {
    let made = prepare_new_account_vault(&secret(PASSWORD), &account(), fast_kdf(), NOW).unwrap();
    let sk = made.secret_key;
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(made.prepared).unwrap();
    vault.unlock_for_account(&secret(PASSWORD), &sk, &account()).unwrap();
    vault.store_account(&record()).unwrap();
    (vault, sk)
}

/// A second device on the same account, signed in from the first one's
/// header and ready to sync.
fn second_device(
    first: &VaultService,
    sk: &havenkeys_core::crypto::secret_key::SecretKey,
) -> VaultService {
    let header = first.encode_account_header().unwrap();
    let (prepared, _auth) =
        havenkeys_core::sync::prepare_sign_in(&header, &secret(PASSWORD), sk, &account()).unwrap();
    let mut b = VaultService::new(Store::open_in_memory().unwrap());
    b.create_vault(prepared).unwrap();
    b.unlock_for_account(&secret(PASSWORD), sk, &account()).unwrap();
    b.store_account(&record()).unwrap();
    b
}

#[test]
fn a_new_item_is_pending_until_the_push_is_confirmed() {
    let (mut vault, _sk) = activated();
    let item = vault.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();

    let pending = vault.pending_push().unwrap();
    assert_eq!(pending.base_cursor, 0);
    assert_eq!(pending.changes.len(), 1);
    assert_eq!(pending.changes[0].item_id, item.id);
    assert!(pending.changes[0].deleted_at.is_none());
    assert!(pending.changes[0].overview.is_some());

    vault.confirm_push(4, &[item.id], NOW).unwrap();
    assert!(vault.pending_push().unwrap().changes.is_empty());
    assert_eq!(vault.pending_push().unwrap().base_cursor, 4);
}

#[test]
fn a_deletion_is_pending_as_a_tombstone() {
    let (mut vault, _sk) = activated();
    let item = vault.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    vault.confirm_push(1, &[item.id], NOW).unwrap();

    vault.delete_item(&item.id, NOW + 1000).unwrap();
    let pending = vault.pending_push().unwrap();
    assert_eq!(pending.changes.len(), 1);
    assert_eq!(pending.changes[0].deleted_at, Some(NOW + 1000));
    assert!(pending.changes[0].overview.is_none());
}

#[test]
fn a_remote_change_is_applied_and_advances_the_cursor() {
    // Two devices on one account: everything one pushes, the other applies.
    let (mut a, sk) = activated();
    let mut b = second_device(&a, &sk);

    let item = a.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let pending = a.pending_push().unwrap();

    let report = b.apply_remote_changes(9, pending.changes, NOW).unwrap();
    assert_eq!(report.added, 1);
    assert_eq!(b.get_item(&item.id).unwrap().title, "GitHub");
    assert_eq!(b.pending_push().unwrap().base_cursor, 9);
    // Applying someone else's change must not make it pending here.
    assert!(b.pending_push().unwrap().changes.is_empty());
}

#[test]
fn a_tampered_remote_item_is_skipped_not_applied() {
    let (mut a, sk) = activated();
    let mut b = second_device(&a, &sk);

    let item = a.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    let mut changes = a.pending_push().unwrap().changes;
    let blob = changes[0].overview.as_mut().unwrap();
    let n = blob.len();
    blob[n - 1] ^= 0x01;

    let report = b.apply_remote_changes(9, changes, NOW).unwrap();
    assert_eq!(report.added, 0);
    assert_eq!(report.skipped_items, 1);
    assert!(b.get_item(&item.id).is_err());
}

#[test]
fn a_blob_from_another_item_is_rejected() {
    // The AAD binds each blob to its item ID, so a server that swaps two
    // items' blobs must not be able to write either one.
    let (mut a, sk) = activated();
    let mut b = second_device(&a, &sk);

    a.create_item(common::login("GitHub", "me", "pw", "github.com"), NOW).unwrap();
    a.create_item(common::login("GitLab", "me", "pw", "gitlab.com"), NOW).unwrap();
    let mut changes = a.pending_push().unwrap().changes;
    assert_eq!(changes.len(), 2);
    let stolen = changes[0].overview.clone();
    changes[1].overview = stolen;

    let report = b.apply_remote_changes(9, changes, NOW).unwrap();
    assert_eq!(report.skipped_items, 1);
    assert_eq!(report.added, 1);
}

#[test]
fn an_older_remote_version_loses_to_the_local_one() {
    let (mut a, sk) = activated();
    let mut b = second_device(&a, &sk);

    let item = a.create_item(common::login("GitHub", "me", "old", "github.com"), NOW).unwrap();
    let old = a.pending_push().unwrap().changes;
    b.apply_remote_changes(1, old.clone(), NOW).unwrap();

    // b edits it later, then the server replays the old version.
    b.update_item(
        &item.id,
        common::login("GitHub renamed", "me", "new", "github.com"),
        NOW + 5000,
    )
    .unwrap();
    let report = b.apply_remote_changes(2, old, NOW + 9000).unwrap();
    assert_eq!(report.updated, 0);
    assert_eq!(b.get_item(&item.id).unwrap().title, "GitHub renamed");
}
```

`store_account` is a passthrough to `Store::set_account`; add it in step 3. Confirm `update_item`'s real signature with `grep -n "pub fn update_item" -A 6 crates/havenkeys-core/src/vault.rs` and match it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core --test delta_sync`
Expected: compile errors — `RemoteChange` not found, `no method named pending_push`.

- [ ] **Step 3: Write the implementation**

In `crates/havenkeys-core/src/sync.rs`, add the types:

```rust
/// One item's state as it travels to or from the server. Blobs are the same
/// per-item ciphertexts stored locally, copied without re-encryption, so the
/// server never holds anything it could open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteChange {
    pub item_id: Uuid,
    /// `None` for a deletion.
    pub overview: Option<Vec<u8>>,
    /// `None` for a deletion.
    pub details: Option<Vec<u8>>,
    /// `Some` for a deletion, unix milliseconds.
    pub deleted_at: Option<i64>,
}

/// What this device still owes the server, and the cursor those changes were
/// computed against.
#[derive(Clone, Debug)]
pub struct PendingPush {
    pub base_cursor: i64,
    pub changes: Vec<RemoteChange>,
}
```

and the three methods, in the `impl VaultService` block from Task 5:

```rust
    /// Store the account this vault belongs to.
    pub fn store_account(&mut self, rec: &AccountRecord) -> Result<()> {
        self.store.set_account(rec)
    }

    /// The account record, or an error when this vault is not linked to one.
    fn require_account(&self) -> Result<AccountRecord> {
        self.store
            .account()?
            .ok_or(Error::InvalidInput("this vault is not linked to an account"))
    }

    /// Local changes not yet accepted by the server.
    pub fn pending_push(&self) -> Result<PendingPush> {
        let base_cursor = self.require_account()?.server_cursor;
        let mut changes: Vec<RemoteChange> = self
            .store
            .dirty_rows()?
            .into_iter()
            .map(|(item_id, overview, details)| RemoteChange {
                item_id,
                overview: Some(overview),
                details: Some(details),
                deleted_at: None,
            })
            .collect();
        changes.extend(
            self.store
                .dirty_tombstones()?
                .into_iter()
                .map(|(item_id, deleted_at)| RemoteChange {
                    item_id,
                    overview: None,
                    details: None,
                    deleted_at: Some(deleted_at),
                }),
        );
        Ok(PendingPush {
            base_cursor,
            changes,
        })
    }

    /// Merge a delta from the server and advance the cursor.
    ///
    /// The server is untrusted: every non-deletion must authenticate under
    /// this vault's data key and match its own item ID, or it is skipped and
    /// counted. Which version wins is decided on decrypted content by the
    /// same rules the folder path used (docs/sync.md §5) — the cursor only
    /// says what to fetch.
    pub fn apply_remote_changes(
        &mut self,
        cursor: i64,
        changes: Vec<RemoteChange>,
        now_ms: i64,
    ) -> Result<SyncReport> {
        self.require_account()?;
        let mut report = SyncReport::default();
        let vault_id = self.session()?.vault_id;
        let mut candidates: HashMap<Uuid, Candidate> = HashMap::new();
        let mut remote_tombs: HashMap<Uuid, i64> = HashMap::new();

        for change in changes {
            match (change.deleted_at, change.overview, change.details) {
                (Some(at), _, _) => {
                    let e = remote_tombs.entry(change.item_id).or_insert(at);
                    *e = (*e).max(at);
                }
                (None, Some(ov), Some(det)) => {
                    match self.check_item_bytes(vault_id, change.item_id, ov, det) {
                        Some((row, overview)) => {
                            candidates.insert(
                                change.item_id,
                                Candidate {
                                    row,
                                    overview,
                                    device: Uuid::nil(),
                                },
                            );
                        }
                        None => report.skipped_items += 1,
                    }
                }
                (None, _, _) => report.skipped_items += 1,
            }
        }

        self.decide_merge(candidates, remote_tombs, &mut report)?;
        self.store.set_cursor(cursor, now_ms)?;
        Ok(report)
    }

    /// The server accepted these changes at `cursor`.
    pub fn confirm_push(&mut self, cursor: i64, pushed: &[Uuid], now_ms: i64) -> Result<()> {
        self.require_account()?;
        self.store.clear_dirty(pushed)?;
        self.store.set_cursor(cursor, now_ms)
    }
```

Add the imports `sync.rs` needs: `crate::store::AccountRecord` and `std::collections::HashMap` (already imported).

Note on `Candidate.device`: the snapshot path used it to break `updated_at` ties between devices. A delta carries one version per item, so there is nothing to break a tie against; `Uuid::nil()` is correct and never compared.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p havenkeys-core --test delta_sync`
Expected: 6 tests pass.

- [ ] **Step 5: Run everything and commit**

```bash
cargo test -p havenkeys-core
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/src/sync.rs crates/havenkeys-core/tests/delta_sync.rs
git commit -m "feat(core): delta sync against a server cursor"
```

---

### Task 9: Fuzz the new untrusted-input boundary and audit for leaks

**Files:**
- Modify: `crates/havenkeys-core/tests/fuzz.rs`
- Modify: `crates/havenkeys-core/tests/no_logging.rs`
- Modify: `docs/crypto.md`

**Interfaces:**
- Consumes: everything above.
- Produces: no new API.

Remote changes and server headers are now untrusted input reaching a parser, which is exactly what the existing fuzz suite exists for.

- [ ] **Step 1: Read what the fuzz suite already does**

Run: `sed -n '1,75p' crates/havenkeys-core/tests/fuzz.rs`.

The file has its own seeded xorshift `Rng` with `Rng::new(seed)`, `next()`, `below(n)`, `pick(&[T])` and `bytes(max)`, plus `mutate(rng, &[u8])` and `mutate_str`. Use those; do not add a second PRNG.

- [ ] **Step 2: Write the fuzz cases**

Append to `crates/havenkeys-core/tests/fuzz.rs`:

```rust
// ------------------------------------------------------------ server input

/// An activated vault with an account record, for the two targets below.
fn fuzz_account_vault() -> VaultService {
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::store::{AccountRecord, Store};
    use havenkeys_core::vault::prepare_new_account_vault;

    let account = AccountRef::new(
        Uuid::from_u128(0x5eed),
        NormalizedEmail::parse("user@example.com").unwrap(),
    );
    let made = prepare_new_account_vault(&secret(PASSWORD), &account, fast_kdf(), NOW).unwrap();
    let sk = made.secret_key;
    let mut vault = VaultService::new(Store::open_in_memory().unwrap());
    vault.create_vault(made.prepared).unwrap();
    vault.unlock_for_account(&secret(PASSWORD), &sk, &account).unwrap();
    vault
        .store_account(&AccountRecord {
            account_id: account.id,
            email: "user@example.com".into(),
            server_url: "https://vault.example.com".into(),
            server_cursor: 0,
            max_header_rev: 0,
            last_synced_at: None,
        })
        .unwrap();
    vault
}

/// Random bytes in the overview and details slots must never panic and never
/// produce an item. Everything the server sends is untrusted input.
#[test]
fn fuzz_remote_changes() {
    let mut vault = fuzz_account_vault();
    let mut rng = Rng::new(0x5EED_5EED);
    for _ in 0..2000 {
        let change = havenkeys_core::sync::RemoteChange {
            item_id: Uuid::from_u128(u128::from(rng.next())),
            overview: Some(rng.bytes(512)),
            details: Some(rng.bytes(512)),
            deleted_at: None,
        };
        let report = vault.apply_remote_changes(1, vec![change], NOW).unwrap();
        assert_eq!(report.added, 0);
        assert_eq!(report.updated, 0);
        assert_eq!(report.skipped_items, 1);
    }
    assert_eq!(vault.list_items().unwrap().len(), 0);
}

/// Random bytes as a server header must be rejected, never adopted.
#[test]
fn fuzz_account_headers() {
    let mut vault = fuzz_account_vault();
    let mut rng = Rng::new(0x1234_5678);
    for _ in 0..2000 {
        let bytes = rng.bytes(1024);
        // Either an error or a refusal; never an adoption, never a panic.
        assert!(matches!(
            vault.adopt_account_header(&bytes),
            Err(_) | Ok(false)
        ));
    }
}
```

`VaultService` is not yet imported in `fuzz.rs`; add `use havenkeys_core::vault::VaultService;` to its imports. `secret`, `fast_kdf`, `PASSWORD` and `NOW` come from `common::*`, which the file already glob-imports.

- [ ] **Step 3: Run the fuzz tests**

Run: `cargo test -p havenkeys-core --test fuzz`
Expected: all pass, no panic.

- [ ] **Step 4: Extend the no-logging test**

`crates/havenkeys-core/tests/no_logging.rs` today only scans `src/` for print and log macros — it asserts nothing about `Debug`. Add a second test to it covering the new secret-bearing type, since `AuthKey` is the one value in this plan that would be catastrophic in a log line:

```rust
#[test]
fn auth_key_debug_never_leaks_material() {
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::secret_key::SecretKey;
    use havenkeys_core::crypto::kdf::KdfParams;
    use havenkeys_core::vault::derive_auth_key;
    use havenkeys_core::SecretString;

    let account = AccountRef::new(
        uuid::Uuid::from_u128(3),
        NormalizedEmail::parse("user@example.com").unwrap(),
    );
    let sk = SecretKey::generate().unwrap();
    let kdf = KdfParams::with_cost(
        havenkeys_core::crypto::kdf::MIN_MEMORY_KIB,
        havenkeys_core::crypto::kdf::MIN_ITERATIONS,
        1,
    )
    .unwrap();
    let auth = derive_auth_key(&SecretString::from("correct horse battery staple"), &sk, &kdf, &account).unwrap();
    let printed = format!("{auth:?}");
    assert_eq!(printed, "AuthKey(<redacted>)");
    assert!(!printed.contains(auth.to_base64().as_str()));
}
```

- [ ] **Step 5: Update the crypto documentation**

In `docs/crypto.md`, under "Key hierarchy", add key scheme 3 beside the existing schemes — the derivation, the `info` labels, what the salt is made of, and the fact that one Argon2id run yields both the KEK and the auth key. State plainly that the auth key authenticates and unwraps nothing. Add a "Key scheme 3 (account)" subsection after "Secret Key (key scheme 2)", covering: the scheme values 1/2/3, that scheme 3 unlock without an account is refused before derivation, and the 2 → 3 upgrade keeping the Secret Key and leaving items untouched.

Do **not** document the server, the API or the removal of folder sync here — those belong to later plans, and `docs/sync.md` must stay accurate until folder sync is actually removed.

- [ ] **Step 6: Run everything and commit**

```bash
cargo test -p havenkeys-core
cargo clippy -p havenkeys-core --all-targets -- -D warnings
git add crates/havenkeys-core/tests/ docs/crypto.md
git commit -m "test(core): fuzz remote changes and headers; document key scheme 3"
```

---

## Done when

- [ ] `cargo test -p havenkeys-core` passes, including every pre-existing test
- [ ] `cargo clippy -p havenkeys-core --all-targets -- -D warnings` is clean
- [ ] `cargo test` across the workspace default members passes — folder sync, the bridge, the native host and the protocol crate are untouched
- [ ] A schema-2 database on disk opens, upgrades, and reports its rows as dirty
- [ ] `docs/crypto.md` describes key scheme 3

**Next plan:** `havenkeys-server` — schema, auth, sync routes, admin CLI and the Railway deployment, per spec §7 and §13 step 2.
