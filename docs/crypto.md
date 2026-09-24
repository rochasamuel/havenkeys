# Cryptography

HavenKeys does not implement cryptographic primitives. All primitives come from
the RustCrypto project and the OS random number generator:

| Purpose | Primitive | Crate |
|---|---|---|
| Password-based key derivation | Argon2id (v0x13) | `argon2` |
| Key separation | HKDF-SHA-256 | `hkdf`, `sha2` |
| Authenticated encryption | AES-256-GCM, 96-bit nonce, 128-bit tag | `aes-gcm` (an earlier release was audited by NCC Group in 2020; the current 0.11 line has not been) |
| Randomness | OS CSPRNG (`getrandom`) | `rand` (`SysRng`) |
| TOTP | HMAC-SHA-1/256/512 (RFC 4226 / 6238) | `hmac`, `sha1`, `sha2` |
| Passkeys (WebAuthn ES256) | ECDSA P-256 with SHA-256, RFC 6979 deterministic nonces; SPKI encoding | `p256` 0.14 (`ecdsa`, `pkcs8` features), `sha2` |
| Memory hygiene | Zeroize-on-drop | `zeroize` |

The only code we wrote around these is *composition*: which key encrypts what,
with which associated data, and how bytes are laid out on disk. That code lives
in `crates/havenkeys-core/src/crypto/` and is intentionally small.

## Key hierarchy

```text
 master password (UTF-8, never stored)     Secret Key (128 random bits; on each
        │                                  device and on the Emergency Kit only)
        │  Argon2id(salt = 16 random bytes, m, t, p)         │
        ▼          params + salt stored in header            │
 master key (32 bytes, memory only)                          │
        │                                                    │
        │  HKDF-SHA-256(ikm  = master key ‖ Secret Key,  ◄────┤
        │               salt = account ID ‖ normalized email, │
        │               info = "havenkeys/v3/kek")            │  same ikm, same salt,
        ▼                                                      │  info = "havenkeys/v3/auth"
 KEK — key encryption key (32 bytes, memory only)               ▼
        │                                                 auth key (sent to the server
        │  AES-256-GCM unwrap of header.wrapped_vault_key  at login; unwraps nothing)
        ▼
 vault key (32 random bytes from the OS CSPRNG; stored only wrapped)
        │
        │  HKDF-SHA-256(ikm = vault key, info = "havenkeys/v1/data")
        ▼
 data key (32 bytes, memory only while unlocked)
        │
        ├── item overview blobs   (title, username, URLs, flags, timestamps)
        ├── item details blobs    (password, TOTP config, notes, note content, password history)
        └── settings blob         (auto-lock, clipboard timeout; device-local, unsynced)
```

Why this shape:

* **The master password is never an encryption key.** It only feeds Argon2id.
* **The vault key is random**, so vault strength does not depend on how the
  password is encoded, and changing the master password only rewraps 32 bytes
  (items are not re-encrypted).
  *Limitation:* because the vault key does not change, someone holding an old
  copy of the vault file **and** the old password (and Secret Key) can still
  read newer copies. See `security-review.md` #8.
* **Secret Key storage** is outside this hierarchy's scope but worth stating
  here: it is kept in the OS keychain (`apps/desktop/src-tauri/src/secret_store.rs`,
  the `keyring-core` crate; service `app.havenkeys`, user = account ID), with
  `device.json` (0600) as the fallback when no keychain is available, a call
  errors, or it does not answer within 5 seconds. Only the `H1-…` text is
  ever stored; errors from the store never carry the value. A keychain that
  errors or does not answer is never taken as "no key" (unlock asks to retry
  rather than for the Emergency Kit). See `server-sync.md` §7 for the
  trade-off this fallback keeps.
* **HKDF domain separation** (`info` strings) ensures the KEK, the auth key
  and the data key can never collide with each other or with future keys.
* **The KEK derivation takes the account ID and the normalized email as HKDF
  salt** — not the vault ID — so it is bound to the identity that owns the
  vault, not to any particular copy of it, and the same password used for two
  different accounts yields unrelated KEKs even in the (astronomically
  unlikely) event of an Argon2 salt collision.
* **The KEK binds to the account.** The Argon2id master key and the Secret
  Key are concatenated as HKDF input keying material (`ikm = master key ‖
  Secret Key`), with that account-derived salt and info label
  `"havenkeys/v3/kek"`. A second HKDF expansion over that *same* input keying
  material and that *same* salt, with info label `"havenkeys/v3/auth"`,
  yields an auth key sent to the server at login. One Argon2id run — the
  expensive step — therefore produces both the KEK and the auth key; the auth
  key authenticates the device to the server and **unwraps nothing**. HKDF's
  info-string separation means it cannot be walked back to the KEK, the
  master key or the Secret Key. See "Key scheme 3 (account)" below.

### Key scheme 3 (account)

Links a vault to a `havenkeys-server` account. There is one key scheme: a
vault always has an account, a Secret Key and a master password, all three
feeding the KEK derivation.

* **`key_scheme` values.** The header field is stored as the integer `3`.
  `KeyScheme` is kept as an enum with one variant (`AccountBound`) rather
  than a unit struct so the header's serde form stays forward-compatible if
  a new scheme is ever added; today `3` is the only value `Store` accepts —
  anything else is `Error::UnsupportedVersion`.
* **A new account vault** (`prepare_new_account_vault`) generates a fresh
  Secret Key and derives the KEK and the auth key from one Argon2id run,
  during activation or second-device sign-in. There is no other way to
  create a vault.
* **Impossible to forget, not merely refused.**
  `UnlockTicket::derive_session_for_account` takes `&SecretKey` and
  `&AccountRef` by reference, not as options, so a caller cannot reach a
  derivation without both. The earlier `derive_kek_for`, which accepted
  options and returned `Error::SecretKeyRequired` before `derive_master_key`
  ran, is gone: the guarantee now lives in the type signature rather than in
  a runtime check and its tests.
* **Email normalization.** The salt above depends on the account's email, so
  every client must reduce it to the same bytes. `NormalizedEmail::parse`
  applies exactly three steps, in this order: trim leading/trailing
  whitespace, Unicode-normalize to NFC, then lowercase. What the derivation
  depends on is that every client applies these same steps in the same
  order and gets the same bytes back — not that the result is itself
  strictly NFC-normalized. It usually is, but lowercasing after NFC can
  still introduce a decomposition for a handful of codepoints (`U+0130`
  LATIN CAPITAL LETTER I WITH DOT ABOVE is one); a future non-Rust client
  must reproduce trim → NFC → lowercase, in that order and nothing more, to
  derive the same key.
* **Auth key.** Sent to the server at login as proof of possession of the
  master password and the Secret Key (`derive_auth_key` /
  `derive_auth_key_from_master`). Its `Debug` implementation prints
  `AuthKey(<redacted>)`; see "Key hierarchy" above for what it does and does
  not do.
* **Changing the master password.** `VaultService::begin_rekey` reads the
  account record from the local store into the `RekeyTicket`, and
  `RekeyTicket::derive_for_account` re-derives both the current and the new
  KEK from it. The account therefore comes from the store, never from the
  caller: the renderer cannot ask for a rewrap under an identity of its
  choosing. Only the header changes — the vault key is unwrapped and
  re-wrapped, items are untouched, and `header_revision` advances (which
  also raises the rollback floor, `max_header_rev`).

  For an account-bound vault this derivation also produces the *old* and
  *new* login (auth) keys from the same two Argon2id runs, because the
  server's verifier has to change with the KEK: `derive_for_account` yields
  both, the core encodes what the header would be at `local + 1` without
  writing it, and `change_master_password` sends all of it to
  `POST /v1/account/credentials` in one request. Only after that request
  returns a 2xx does `commit_rekey` persist the rewrap locally, at the
  revision the server assigned — never before. This ordering (server first,
  local commit only on success) replaced an earlier local-first one that
  could leave the server's verifier and KDF stale after a change, locking
  every device out (`security-review.md` S14). See `server-sync.md` §6 for
  what the other devices then do.
* **An account-bound vault never exists without its account record.** The
  record holds the durable `max_header_rev` rollback floor, so a vault
  without one would silently fall back to the in-memory `revision` alone.
  `create_account_vault` (activation and second-device sign-in) writes the
  account row before the header, so an interruption can leave a record
  without the matching wrap — harmless, and overwritten by the retry — but
  never an account-bound vault without its record. `commit_rekey` refuses to
  persist a rewrap if the account row is missing.
* **The account record is not re-pointable.** `Store::set_account` refuses a
  write whose `account_id` differs from the one already stored. Its
  `ON CONFLICT` clause deliberately preserves `server_cursor` and
  `max_header_rev`, which is right for signing in again to the same account
  and ruinous for a different one: the stale cursor would make the first
  pull skip everything before it, and the stale floor would fail every
  header the new account serves. Re-pointing a vault at another account or
  server needs its own path that resets both, and does not exist yet.

## Argon2id parameters

Defaults are above RFC 9106 §4's "second recommended option" (64 MiB, t=3),
chosen from measurements rather than assumed:

| Parameter | Default | Accepted range when reading a header |
|---|---|---|
| memory (`m_cost`) | 131 072 KiB (128 MiB) | 19 456 KiB – 1 GiB |
| iterations (`t_cost`) | 4 | 2 – 16 |
| parallelism (`p_cost`) | 4 | 1 – 16 |
| salt | 16 random bytes | exactly 16 |
| output | 32 bytes | 32 |

The floors reject a tampered header that tries to make unlocking cheap for
future guesses; the ceilings stop a malicious header from exhausting memory.

**Benchmark on your hardware** before changing defaults:

```sh
cargo run --release -p havenkeys-core --example kdf_bench
```

The `argon2` crate is built without its `parallel` feature, so lanes are
computed sequentially.

Measured on the development machine (x86-64, WSL2, release build, 2026-09-19):

| m | t | p | time per derivation |
|---|---|---|---|
| 19 MiB | 2 | 1 | 24 ms (test-only floor) |
| 64 MiB | 3 | 4 | 129 ms (RFC 9106 option 2) |
| 128 MiB | 3 | 4 | 265 ms |
| **128 MiB** | **4** | **4** | **325 ms (default)** |
| 256 MiB | 3 | 4 | 578 ms |

This is a fast desktop CPU; expect 2–4× longer on older laptops, which keeps
unlock around 1 s. The parameters are stored per vault, so they can be raised
later (a master-password change re-derives with the current defaults).

Master passwords are Unicode-normalized (NFC) before derivation so that the
same visible password typed through different input methods unlocks the vault.

## Encrypted blob format (v1)

All ciphertexts on disk use one binary layout:

```text
offset  size  field
0       1     blob_version   = 0x01
1       1     algorithm      = 0x01 (AES-256-GCM)
2       12    nonce          (random, OS CSPRNG)
14      n     ciphertext     (n = plaintext length)
14+n    16    GCM tag
```

The parser rejects: unknown version, unknown algorithm, input shorter than
30 bytes, and input larger than the per-blob limit (8 MiB).

### Associated data

Every encryption authenticates, in addition to the plaintext:

```text
AAD = "havenkeys" || 0x00 || blob_version || algorithm || purpose || 0x00 || context...
```

| Purpose | Context bound into AAD |
|---|---|
| `vault-key` | vault ID |
| `item-overview` | vault ID, item ID |
| `item-details` | vault ID, item ID |
| `settings` | vault ID |
| `sync-header` | vault ID (the account header a device publishes and reads, `sync.rs`) |

Consequences: a blob copied into another item row, another role, or another
vault fails authentication. Header bytes (version, algorithm) are authenticated
too, so they cannot be altered to trigger a different code path.

### Nonces

Nonces are 96 random bits from the OS CSPRNG, generated per encryption. Every
save of an item re-encrypts with a fresh nonce. NIST SP 800-38D bounds random
96-bit nonces to 2³² encryptions per key; a personal vault performs many
orders of magnitude fewer, and the data key changes whenever a new vault is
created. A test asserts no nonce repeats across a large sample.

## Password generator

Characters are drawn from the selected classes using `rand`'s `Uniform`
distribution over `SysRng` (the OS CSPRNG), which uses rejection sampling (no modulo bias). One
character from each selected class is guaranteed, then the whole password is
shuffled with a Fisher–Yates shuffle using the same unbiased sampler.

Entropy estimate reported to the UI: `length × log2(alphabet size)` (a slight
over-estimate because of the per-class guarantee).

## TOTP

RFC 6238 over HMAC-SHA-1 / SHA-256 / SHA-512, 6 or 8 digits, period 1–300 s.
Secrets are Base32 (RFC 4648, padding optional, case-insensitive, spaces
ignored). `otpauth://totp/...` URIs are parsed with the `url` crate. HOTP
(`otpauth://hotp`) is rejected. Tested against the RFC 6238 Appendix B vectors.

## Passkeys

HavenKeys acts as a WebAuthn Level 3 authenticator for ES256 (COSE algorithm
`-7`) only. The code is `crates/havenkeys-core/src/passkey/`; `webauthn.rs` is
the only place a passkey private key is turned into a signing key. As
everywhere else, the cryptography comes from RustCrypto; what we wrote is the
byte layout WebAuthn prescribes around it.

### Keys and credential IDs

* **Private key:** a P-256 scalar, 32 bytes. It is generated by filling 32
  bytes from the OS CSPRNG (`crypto::fill_random`) into a zeroizing buffer
  and handing them to `SigningKey::from_slice`, which rejects zero and
  values not below the curve order. A rejected draw is thrown away and
  redrawn (at most 8 times, then `Error::Rng`), never reduced, so there is no
  bias.
* **Where it lives:** in `Passkey.private_key` (`SecretBytes`: zeroized on
  drop, redacted `Debug`, no `Display`), inside the `passkeys` list of
  `ItemDetails::Login`. That list is sealed with the rest of the login's
  details as an `item-details` blob, under the vault data key, with the vault
  ID and item ID in the associated data (§Associated data). In the JSON
  plaintext the key is unpadded base64url. It is never in an overview, a
  protocol message or a command result, and it syncs to the server only as
  part of that ciphertext.
* **Credential ID:** 16 bytes from the OS CSPRNG. HavenKeys only ever creates
  16-byte IDs, so the protocol accepts exactly that length, and the page
  script drops any listed ID of another length (it cannot be ours).
* **Format version:** none added. `passkeys` is a new `#[serde(default)]`
  field that is omitted when empty, so a login saved before passkeys opens
  unchanged and an empty list writes the same bytes as before. The cost is
  that an older app drops the field when it re-saves a login
  (`security-review.md` PK3).

### Authenticator data

```text
offset  size  field
0       32    rpIdHash = SHA-256(rpId)           (rpId normalized: lowercase, punycode)
32      1     flags    = 0x1d on get, 0x5d on create
33      4     signCount = 0, big-endian
--- create only (attested credential data) ---
37      16    AAGUID = 16 zero bytes
53      2     credentialIdLength = 16, big-endian
55      16    credentialId
71      77    credentialPublicKey (COSE_Key, below)
```

Flags: `0x01` UP (user present), `0x04` UV (user verified), `0x08` BE (backup
eligible), `0x10` BS (backed up), and on create `0x40` AT (attested credential
data included).

* **UP and UV** are set because a signature or creation happens only after a
  click in the extension's own UI while the vault is unlocked. UV here means
  "the vault was unlocked with the master password", **not** biometrics or a
  fresh verification (`threat-model.md` T8).
* **BE and BS** are set because a HavenKeys passkey is a synced, multi-device
  credential: its key is in the vault, which is on the user's server and on
  every device signed in to it. Relying parties use these flags to tell a
  synced passkey from a device-bound one, and WebAuthn fixes BE for the
  credential's lifetime.
* **The counter is always 0.** An incrementing counter would make every
  sign-in a server write, which would fail offline and race between devices.
  WebAuthn allows 0 ("the authenticator does not implement a counter").
  Consequence: a relying party cannot use the counter to detect a cloned
  credential (`security-review.md` PK2).

### Public key and attestation

The public key is a COSE_Key map in CTAP2 canonical key order:

```text
{ 1: 2 (kty EC2), 3: -7 (alg ES256), -1: 1 (crv P-256), -2: x (32 bytes), -3: y (32 bytes) }
```

`create()` also returns it as SubjectPublicKeyInfo DER (`getPublicKey()`),
from `p256`'s `pkcs8` encoder.

Attestation is always `none`, with a zero AAGUID:

```text
{ "fmt": "none", "attStmt": {}, "authData": <authenticator data> }
```

A relying party therefore cannot tell which authenticator made the passkey,
and a site that insists on attestation will refuse it.

### clientDataJSON

Built in Rust (`client_data_json`), from the origin that `authorize_rp`
derived from the frame URL the browser reported, so neither the page nor the
extension chooses the origin that gets signed:

```text
{"type":"webauthn.get"|"webauthn.create","challenge":<base64url(challenge), unpadded>,
 "origin":<frame origin>,"crossOrigin":<bool>[,"topOrigin":<top-level origin>]}
```

Keys are in the order of WebAuthn §5.8.1.1 (limited verification algorithm).
`crossOrigin` is true, and `topOrigin` present, only for an iframe whose
origin differs from the top page's (same-site frames in a secure top page
only; a cross-site frame, or one in an `http:` top page, is refused before
this point). Challenges are 1–1024 bytes.

String values are escaped with `serde_json`, not with the spec's
`CCDToString`. The two differ only for characters that never appear here:
the type is fixed, the challenge is base64url, and origins are ASCII
serializations (IDNs in punycode). The bytes are identical in practice; the
test `client_data_is_exact` pins them.

### Signatures

`get()` returns `authenticatorData`, and an ASN.1 DER ECDSA signature made
with `p256`'s `SigningKey` over `authenticatorData || SHA-256(clientDataJSON)`
(ES256: the message is hashed with SHA-256; nonces are deterministic, RFC
6979). It also returns the stored user handle. The private key is loaded into
a `SigningKey` (which zeroizes its scalar on drop) only for that call. A
stored key that is not a valid scalar gives `Error::Corrupted`, never a
signature.

### CBOR

The three CBOR shapes above (COSE key, attestation object, and their
integer/byte/text/map contents) are written by a small encoder in
`passkey/cbor.rs`, following RFC 8949 §4.2.1 deterministic encoding
(shortest-form heads; map entries in the order the caller gives, which is
CTAP2 canonical). It only encodes; HavenKeys never parses CBOR.

Tests:

* The encoder against RFC 8949 Appendix A vectors: integers 0, 1, 23, 24,
  100, 1000, 1 000 000, 10¹², −1, −10, −100, −1000 and `i64::MIN`; empty and
  4-byte byte strings; `""`, `"a"`, `"IETF"`; the empty map and `{1: 2, 3: 4}`.
* `ciborium` (a dev-dependency only) independently decodes the attestation
  object and the COSE key a real registration produces, and the test checks
  the COSE key names the same point as the SPKI key.
* The authenticator data layout, flags and zero counter byte by byte, and a
  signature from `assert` verified with `p256`'s verifier over
  `authenticatorData || SHA-256(clientDataJSON)`.

These tests use our own encoder and RustCrypto's verifier. The independent
check that real relying parties accept the result is the manual checklist in
`security-review.md` (webauthn.io, github.com, google.com), which has not
been run yet.
