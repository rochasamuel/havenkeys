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
* **Key scheme 3 binds the KEK to an account.** The same Argon2id master key
  and the Secret Key are concatenated exactly as in scheme 2 (`ikm = master
  key ‖ Secret Key`), but the HKDF salt becomes the account ID's 16 raw bytes
  followed by the normalized email's UTF-8 bytes, and the info label is
  `"havenkeys/v3/kek"`. A second HKDF expansion over that *same* input keying
  material and that *same* salt, with info label `"havenkeys/v3/auth"`,
  yields an auth key sent to the sync server at login. One Argon2id run —
  the expensive step — therefore produces both the KEK and the auth key; the
  auth key authenticates the device to the server and **unwraps nothing**.
  HKDF's info-string separation means it cannot be walked back to the KEK,
  the master key or the Secret Key. See "Key scheme 3 (account)" below.

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

### Key scheme 3 (account)

Links a vault to a `havenkeys-server` account, so that device can pull and
push a delta sync feed against a server cursor. The account is not the
Secret Key: linking does not remove or replace it, it adds a third input to
the KEK derivation.

* **`key_scheme` values.** The header field now takes `1` (password only),
  `2` (password + Secret Key) or `3` (password + Secret Key + account).
  There is no scheme that combines an account with `1`: scheme 3 always
  requires the Secret Key, because it is derived exactly like scheme 2 with
  the account mixed in (see "Key hierarchy" above).
* **A new account vault** (`prepare_new_account_vault`) generates a fresh
  Secret Key and derives straight into scheme 3, so first-time account
  creation never passes through scheme 2.
* **Refused before derivation.** `derive_kek_for` matches on
  `(KeyScheme::AccountBound, Some(secret_key), None)` — a scheme 3 header
  with a Secret Key but no `AccountRef` — and returns
  `Error::InvalidInput` immediately, before `derive_master_key` runs. A
  caller that forgets to pass the account (or a vault opened outside its
  account context) is refused without paying for an Argon2id derivation.
* **The 2 → 3 upgrade** (`RekeyTicket::derive_account_upgrade`) requires the
  vault to currently be at scheme 2, unwraps the vault key with the current
  password and Secret Key, and re-wraps that *same* vault key under a new
  scheme 3 KEK with a fresh Argon2id salt. The Secret Key does not change,
  and items are never touched or re-encrypted — only the header's key wrap,
  KDF parameters and `key_scheme` change (and `header_revision` advances, as
  for any rewrap).
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

**Deletions in the delta sync feed are not authenticated.** `RemoteChange`,
the unit the feed moves in, carries a `deleted_at` field that is plaintext
the server supplies, sealed by nothing — unlike item content, which is
AEAD-bound to the vault ID, item ID and role and so cannot be forged or
moved between items. A hostile or compromised server can therefore delete
any item on every device that pulls the change. Three specifics, all
verified against `decide_merge` in `src/sync.rs`:

* a forged tombstone for an item ID that never existed on the device is
  still *persisted* as a local row (`apply_merge` inserts it unconditionally),
  so a server can mint unbounded tombstone rows for IDs it invents;
* a `deleted_at` far in the future poisons that item ID permanently, since
  `decide_merge`'s resurrection arm (`(None, Some(&lt)) if ru > lt`) can then
  never be satisfied again for it — `apply_remote_changes` mitigates only
  this worst case (a `deleted_at` more than 24 hours past `now_ms` is
  rejected and counted in `skipped_items`); it does **not** close the hole,
  since a server sending `deleted_at = now_ms` still deletes the item;
* it destroys locally-dirty work that was never pushed to the server:
  `decide_merge` does not consult the `dirty` flag before deciding a
  deletion wins.

Authenticating deletions — a sealed tombstone format, a store migration and
the matching server schema — is required before any server ships; see the
design spec §9 and `docs/roadmap.md`.

**Contract for a future `havenkeys-sync-client`:** `apply_remote_changes`
advances the stored cursor unconditionally, even when some changes in the
batch were skipped (a tampered blob, a blob for the wrong item, or the
future-dated deletion above). A corrupted or tampered item therefore never
reaches the device again unless the server later holds a *new* version of
it — the cursor has already moved past it. This differs from the folder
model, which re-read whole device snapshots each sync and so self-healed on
the next run. `SyncReport.skipped_items` is what a client must watch: a
nonzero count is the signal to re-pull from cursor 0 (or otherwise recover),
because the delta stream will not resurface the skipped item on its own.

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
