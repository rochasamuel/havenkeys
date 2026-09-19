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
 master password (UTF-8, never stored)        Secret Key (128 random bits; on each
        │                                      device and on the Emergency Kit only)
        │  Argon2id(salt = 16 random bytes, m, t, p)          │
        ▼          params + salt stored in header             │
 master key (32 bytes, memory only)                           │
        │                                                     │
        │  HKDF-SHA-256(ikm  = master key ‖ Secret Key, ◄──────┘
        │               salt = vault_id,
        │               info = "havenkeys/v2/kek")         key scheme 2
        │  (key scheme 1, older vaults: ikm = master key,
        │   info = "havenkeys/v1/kek")
        ▼
 KEK — key encryption key (32 bytes, memory only)
        │
        │  AES-256-GCM unwrap of header.wrapped_vault_key
        ▼
 vault key (32 random bytes from the OS CSPRNG; stored only wrapped)
        │
        │  HKDF-SHA-256(ikm = vault key, info = "havenkeys/v1/data")
        ▼
 data key (32 bytes, memory only while unlocked)
        │
        ├── item overview blobs   (title, username, URLs, flags, timestamps)
        ├── item details blobs    (password, TOTP config, notes, note content, password history)
        ├── settings blob         (auto-lock, clipboard timeout, …)
        ├── sync snapshots        (one per device, in the sync folder)
        └── sync header attestation
```

Why this shape:

* **The master password is never an encryption key.** It only feeds Argon2id.
* **The vault key is random**, so vault strength does not depend on how the
  password is encoded, and changing the master password only rewraps 32 bytes
  (items are not re-encrypted).
  *Limitation:* because the vault key does not change, someone holding an old
  copy of the vault file **and** the old password (and Secret Key) can still
  read newer copies. See `security-review.md` #8.
* **HKDF domain separation** (`info` strings) ensures the KEKs of both
  schemes and the data key can never collide with each other or with future
  keys.
* The KEK derivation takes the vault ID as HKDF salt, so the same password on
  two vaults yields unrelated KEKs even in the (astronomically unlikely) event
  of an Argon2 salt collision.

### Secret Key (key scheme 2)

Implemented as planned in earlier versions of this document, and in the
style of 1Password's two-secret key derivation:

* **Generation:** 16 bytes from the OS CSPRNG when a vault is created (or
  when an older vault is upgraded).
* **Mixing:** the Argon2id master key and the Secret Key are concatenated as
  HKDF input keying material, with the vault ID as salt and
  `"havenkeys/v2/kek"` as info. HKDF is used as specified, as an extractor
  and expander over the combined input. No new construction is involved.
* **What it adds:** a copy of the vault that is not on one of your devices
  (the sync folder, a backup of `vault.sqlite3`) cannot be attacked by
  guessing the master password alone. The attacker would also need to guess
  128 random bits. A weak master password is still weak on a device that
  holds the Secret Key.
* **Text form:** `H1-` then 26 Base32 (RFC 4648) characters of key and 2
  check characters, grouped by 4:
  `H1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`. The check characters are 10 bits of
  `SHA-256("havenkeys/secret-key/check" ‖ key)`. They catch typos before the
  slow Argon2id step and are not a security feature. Parsing ignores case,
  spaces and dashes, and maps 0→O, 1→I, 8→B.
* **Storage:**
  * on each device: `device.json`, outside the vault database (see
    `sync.md` §6 for why it is plain text there);
  * on paper or PDF: the Emergency Kit.
  * It never goes into the vault database, the sync folder, logs, or the
    browser extension.
* **The header records the scheme** (`key_scheme` 1 or 2). Scheme 1 vaults
  keep working, and *Add a Secret Key* rewraps their vault key under a scheme
  2 KEK, with the same master password and a fresh Argon2id salt. Items are
  not touched. A scheme 2 unlock without a Secret Key is refused before any
  key derivation.
* **Header revision:** every rewrap (password change, Secret Key upgrade)
  increments `header_revision`, so devices sharing a sync folder can tell
  which header is newest (`sync.md` §4).

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
| `sync-snapshot` | vault ID, device ID (in the item-ID slot) |
| `sync-header` | vault ID |

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
