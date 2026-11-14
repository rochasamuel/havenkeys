# Passkey parity: credProps, transports, Ed25519 and PRF — Design

Status: implemented (2026-11-14). Builds on the passkeys design
(`2026-09-23-passkeys-design.md`) and the passkey upgrade
(`2026-09-24-passkey-upgrade-design.md`). No CLAUDE.md amendment: the
private key and the new PRF secret are generated, stored and used only in
the Rust core, as before.

> This software has not undergone an independent security audit.

## 1. Goal

Some sites do not treat HavenKeys as a passkey manager. Oracle's account
pages were the report. A look at what 1Password 8.12.44 answers showed four
gaps:

| | 1Password 8.12.44 | HavenKeys before |
|---|---|---|
| `credProps` | `{ rk: true }` when asked | `getClientExtensionResults()` always `{}` |
| `transports` | synced-passkey set | `["internal"]` |
| Algorithms | -7 ES256, -9 ESP256, -8 EdDSA, -19 Ed25519 (ML-DSA behind a flag) | ES256 only; a site listing ES256 after others still worked, one without it fell back to the browser |
| `prf` | supported (per-credential `credWithUV` / `credWithoutUV` secrets) | not supported |

This spec closes all four for the browser extension and, except PRF, for
Android.

## 2. Non-goals

* **ML-DSA** (-48/-49/-50). Not standardised for WebAuthn yet, and no audited
  Rust implementation we would take on.
* **RSA** (-257). Platform passkeys don't use it.
* **`largeBlob`, `credBlob`, `minPinLength`, `hmac-secret` in
  authenticator data.** HavenKeys is a platform-style provider, not a CTAP
  authenticator; PRF is answered at the WebAuthn client level (§5.5).
* **PRF on Android.** Follow-up (§8).

## 3. Transports

`getTransports()` and `toJSON().response.transports` report
`["hybrid", "internal"]`, the same as Android's `registration_json`.

That is what synced platform passkeys report (Google Password Manager and
iCloud Keychain both answer `hybrid` + `internal`). It is true of a HavenKeys
passkey: it is usable on this device, and it syncs to the user's phone,
which can sign for another device. `internal` alone describes a device-bound
key; some sites file such keys as "this device only" or treat them as
security keys. Transports are only a hint the site stores for its later
`allowCredentials`; HavenKeys ignores transports in `allowCredentials`
either way.

## 4. Algorithms and how keys are stored

### 4.1 Choice

`passkey::choose_algorithm(params)` (Rust, the decision) and
`chooseAlgorithm` (TypeScript mirror, for early fallback only) pick the
**first** entry of the site's `pubKeyCredParams` that HavenKeys supports:

| COSE | Name | Key | Signature |
|---|---|---|---|
| -7 | ES256 | P-256 | ASN.1 DER ECDSA over SHA-256, RFC 6979 |
| -9 | ESP256 (RFC 9864, fully specified) | P-256 | same as ES256 |
| -8 | EdDSA | Ed25519 | raw 64-byte PureEdDSA (RFC 8032) |
| -19 | Ed25519 (RFC 9864, fully specified) | Ed25519 | same as EdDSA |

An empty list means WebAuthn's default, ES256 (WebAuthn L3 §5.4). A list
with no `public-key` entry, or with nothing supported in its first 16
entries, falls back to the browser. The COSE key and
`getPublicKeyAlgorithm()` carry the identifier the site asked for: a site
that listed only -9 gets `alg: -9`, not -7.

The COSE_Key for Ed25519 is an OKP key in CTAP2 canonical order:
`{1: 1 (kty OKP), 3: alg (-8 or -19), -1: 6 (crv Ed25519), -2: x (32 bytes)}`
(RFC 9053 §7.2). `getPublicKey()` returns the SubjectPublicKeyInfo (OID
1.3.101.112). Ed25519 comes from `ed25519-dalek` 3 (dalek-cryptography,
BSD-3-Clause, on `curve25519-dalek` 5, already in the tree through HPKE).

### 4.2 The compatibility problem

A passkey is stored inside its login's encrypted details and syncs to every
device, some of which may run an older HavenKeys (0.35 and earlier). In
those versions:

* `Passkey` has no `deny_unknown_fields` and no catch-all for unknown fields;
  `ItemDetails::Login` has none either (only `ItemOverview` keeps unknown
  fields, since `2026-10-07-item-tags` §3.4).
* `private_key` is required, and `assert` reads it with
  `p256::SecretKey::from_slice`, which accepts 32 bytes, or 24–31 bytes
  left-padded with zeros.
* Every edit of the login (`build_item` through `stage_update`, a new
  passkey, dropping a replaced one) deserializes and re-serializes the
  `passkeys` list.

So a new field on `Passkey` (say `alg` or `prfSecret`) would load, and then
**be dropped** by the first edit on an older device. For `alg`, the
Ed25519 seed left in `private_key` would then be read as a P-256 scalar (a
random 32-byte value is a valid one): the passkey would sign with the wrong
key. For a PRF secret, the loss is permanent and would lock the user out of
whatever the site encrypted with PRF outputs. Making `private_key`
optional or moving keys elsewhere would make older versions fail to load the
whole login, or drop the passkey on edit.

### 4.3 Decision: everything in `private_key`

`private_key` is the one field every version keeps byte for byte. It now
holds either:

* **exactly 32 bytes**: a P-256 scalar with no PRF secret. Every passkey
  made before this spec, and every ES256/ESP256 passkey made without `prf`,
  keeps this form, so older devices use them exactly as before; or
* **a versioned envelope**, `0x01 || kind || flags || key(32) [|| prf(32)]`:
  `kind` 1 = P-256, 2 = Ed25519; `flags` bit 0 = a PRF secret follows; 35 or
  67 bytes. Unknown versions, kinds or flag bits, any other length, and a
  P-256 key outside the curve order are `Corrupted`.

What an older version does with an envelope:

* it **loads** the login (the JSON shape of `Passkey` is unchanged);
* it **lists** the passkey, and signing with it fails with `Corrupted`:
  `from_slice` refuses anything longer than 32 bytes. It never signs with an
  Ed25519 seed as a P-256 key;
* it **keeps** the bytes on any edit, so the key and the PRF secret are
  intact when a current version reads the login again;
* restoring an encrypted backup that holds one fails closed: its
  `check_passkeys` refuses a key that is not a P-256 scalar.

The cost: an Ed25519 passkey, or a passkey made with `prf`, does not work
on a device that has not been updated. Ordinary ES256 passkeys are not
affected. `KeyMaterial` (`passkey/key.rs`) is the only code that reads or
writes the format.

Tests: `passkey::tests::older_versions_load_new_passkeys_and_cannot_misuse_them`
deserializes new passkeys with the 0.35 struct shape, checks that its
signing path refuses them, re-serializes them through that shape and checks
key and PRF secret survive; `plain_es256_passkeys_keep_the_old_format`;
`key::tests::older_versions_refuse_every_envelope`.

## 5. PRF

### 5.1 When a passkey gets a PRF secret

Only when the site sends `extensions.prf` to `create()` (with or without
`eval`). Then the core draws 32 bytes from the OS CSPRNG
(`crypto::fill_random`) and stores them in the envelope next to the key
(§4.3), sealed with the login's details exactly like the private key.

Not always, because a PRF secret moves the passkey into the envelope form,
which older devices cannot sign with (§4.3). Giving every new ES256 passkey
one would break ordinary sign-in on every device not yet updated. A site
that wants PRF asks at creation, which is what the WebAuthn PRF guidance
and current sites do. The cost: a site that asks for PRF only at sign-in
gets no outputs from a passkey made without it (§10).

The create result says `prf: { enabled: true }`, plus `results` when `eval`
was given (we can evaluate at creation, as CTAP 2.2 `hmac-secret-mc` does).
`evalByCredential` at creation is the browser's `NotSupportedError`; the
page script hands such a request to the browser.

Passkeys made before this spec, or without `prf`, have no secret. A
sign-in asking for PRF with one gets a signature and no outputs; the
extension answers `prf: {}`.

### 5.2 Computation

Exactly WebAuthn L3 §10.1.4 on top of CTAP 2.2 `hmac-secret`, with the one
secret HavenKeys keeps (it always asserts UV, so it is the `credWithUV`
secret; there is no `credWithoutUV`):

```text
salt'  = SHA-256("WebAuthn PRF" || 0x00 || salt)
output = HMAC-SHA-256(prf_secret, salt')
```

for `first` and, when given, `second`. `sha2` and `hmac` (RustCrypto),
both already used by the core. The salts are hashed in Rust; the extension
sends the site's raw values. The known-answer test computes the expected
value with `ring` and checks two fixed values computed with Python's
`hashlib`/`hmac`.

### 5.3 Inputs, bounds and which input applies

* Salts: 0–1024 bytes each (WebAuthn sets no bound and allows empty; sites
  send 32). Larger: the page script hands the request to the browser.
* `evalByCredential`: at most 64 entries. Keys that are not base64url, a
  non-empty map with no `allowCredentials`, or one of our 16-byte IDs the
  site did not allow: the browser's error, so the request goes to the
  browser. Keys of other lengths cannot be ours and are dropped.
* The background picks the input for the credential the user chose: its
  `evalByCredential` entry, else `eval`. Only that one pair goes to the
  desktop.
* A page request larger than the bridge accepts (16 KiB, e.g. many 1 KiB
  salts) goes to the browser.

### 5.4 Secrets

PRF outputs are secrets: sites derive encryption keys from them. In Rust
they are `Zeroizing<[u8; 32]>` (`PrfOutput`, redacted `Debug`), base64url on
the wire inside `WireSecret` (zeroized on drop, redacted `Debug`), never
logged. In the extension they are JavaScript strings and `ArrayBuffer`s,
which cannot be wiped; they live only for the message that carries them to
the page, are never stored and never logged. The page receives them because
it asked; that is the feature.

### 5.5 Not in authenticator data

The credential's authenticator data carries no `hmac-secret` extension
output and the ED flag stays clear: the outputs are WebAuthn client
extension results, as with other platform providers, and the
authenticator-data layout (and its tests) stay as they were.

## 6. credProps and capabilities

* `create()` with `extensions.credProps: true` returns
  `{ credProps: { rk: true } }` from `getClientExtensionResults()` and in
  `toJSON().clientExtensionResults`. Every HavenKeys passkey is
  discoverable. Not asked, not returned.
* Only outputs HavenKeys produced are returned: `credProps` when asked,
  `prf` when asked. Nothing else.
* `PublicKeyCredential.getClientCapabilities()` adds
  `extension:credProps: true` and `extension:prf: true` to the two
  platform-authenticator keys already there.

## 7. Protocol and version skew

Rust (`havenkeys-protocol`) and TypeScript (`@havenkeys/protocol`) change
in step:

| Message | New field | Sent when |
|---|---|---|
| `passkey_create` request | `algorithms` (≤ 16 integers) | ES256 is not the site's first supported algorithm |
| `passkey_create` request | `prf` (`{}` or `{ eval: { first, second? } }`) | the site asked for `prf` |
| `passkey_get` request | `prf` (`{ first, second? }`) | the site sent PRF input for the chosen credential |
| `passkey_create` result | `prf` (`{ enabled, results? }`) | the request had `prf` |
| `passkey_get` result | `prf` (`{ first, second? }`, 32 bytes each) | the request had `prf` and the passkey has a secret |
| `passkey_create` result | `publicKeyAlgorithm` -7, -9, -8 or -19 | always (was -7 only) |

Both sides reject unknown fields, so:

* **New extension, older desktop.** The fields are omitted when not needed,
  so ordinary ES256 creation and sign-in work as before. When they are
  needed, the older desktop answers `malformed`; the extension retries once
  without `prf` (the site then sees `prf: { enabled: false }`, or no outputs
  at sign-in), and without `algorithms` only if the site also listed ES256.
  A site that accepts only Ed25519 gets an error card, not a key type it
  did not ask for.
* **Older extension, new desktop.** It never sends the fields, so the
  desktop never sends results with them and makes ES256 keys. Results keep
  their old shape.

## 8. Android

`passkey_json.rs` now passes the site's algorithm list to the core, so
Android creates Ed25519/ESP256 passkeys like the extension, reports
`publicKeyAlgorithm` from the registration, and reports `["hybrid",
"internal"]`. `credProps: { rk: true }` was already returned.

PRF on Android is a follow-up: Credential Manager providers receive the
request JSON from the calling browser or app, and whether `prf.eval` arrives
as the site's salt or already hashed by the browser has to be confirmed on
devices before outputs are computed there. A wrong guess would give the same
credential different outputs on Android and desktop. Until then Android
returns no `prf` output; passkeys with a PRF secret still sign in there.

## 9. Security

* **What a hostile page can learn:** nothing new about the vault. PRF
  outputs come only from a credential the user picked in HavenKeys UI for
  the page's own rpId (the same `authorize_rp` and stored-rpId checks as a
  signature), for salts the page chose. They are a keyed function of the
  page's input: they reveal nothing about the PRF secret, the private key or
  another site's outputs (different secret per credential). Whether a
  passkey has a PRF secret is visible to the site that created it, as in
  WebAuthn.
* **What a compromised extension can do:** ask for PRF outputs of any
  passkey it may already ask a signature for, which it could do with a
  click it fakes today. It cannot read the PRF secret.
* **Older clients:** §4.3. No data loss, no misuse; new-format passkeys fail
  on them until updated.
* Findings PK28–PK30 in `docs/security-review.md`.

## 10. Testing

* Core: `passkey/key.rs` (algorithm order, envelope round trip and refusal,
  older-version refusal, RFC 8032 test 1, signatures verified with `ring`,
  PRF known answers from `ring` and Python, per-credential secrets),
  `passkey/webauthn.rs` (Ed25519 registration and assertion verified with
  `ring`, COSE OKP layout, ESP256, PRF at create and get agree, no secret no
  outputs, oversized salts), `passkey/mod.rs` (the 0.35 struct shape),
  `export/restore.rs` (backups carry envelopes).
* Protocol: Rust `tests/messages.rs` (parse, bounds, round trip without the
  new fields, results, redacted `Debug`, fuzz seeds); TypeScript
  `index.test.ts` and `fuzz.test.ts`.
* Bridge: `ed25519_and_prf_through_the_bridge`.
* Extension: `page.test.ts` (credProps, transports, algorithms, PRF in and
  out, what goes to the browser), `bridge.test.ts`,
  `webauthn-handler.test.ts` (which fields are sent, older-desktop retry,
  `evalByCredential` choice).
* Android: `passkey_json.rs` (algorithm list, transports).

Open questions:

1. Oracle itself has not been retested against a build with these changes.
2. `prf: {}` for a sign-in whose passkey has no secret: browsers differ in
   what they return there; sites read it as "no results".
3. A site that asks for PRF only at sign-in (never at creation) gets no
   outputs from HavenKeys passkeys. If that turns out to be common, giving
   new ES256 passkeys a secret by default can follow once most devices run
   this version.
4. Android PRF (§8).
