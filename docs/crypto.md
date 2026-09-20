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
* **Refused before derivation.** `derive_kek_for` requires both a Secret Key
  and an `AccountRef` and returns `Error::SecretKeyRequired` or
  `Error::InvalidInput` immediately, before `derive_master_key` runs. A
  caller that forgets to pass either is refused without paying for an
  Argon2id derivation.
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
