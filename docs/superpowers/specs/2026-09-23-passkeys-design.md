# Passkeys — Design

Status: proposed, 2026-09-23.
Amends `CLAUDE.md`, which lists passkeys as out of scope for the MVP (see §11).

> This software has not undergone an independent security audit.

## 1. Goal

HavenKeys becomes a passkey (WebAuthn) provider for websites. A user can:

1. **Create** a passkey when a site offers one ("Add a passkey").
2. **Save** it in the vault, attached to a login, synced like any other item.
3. **Sign in** with it later, by explicit choice in the HavenKeys UI.
4. **Decline** at any point and hand the request to the browser's own
   WebAuthn UI (security key, phone, platform authenticator).

Unlocking HavenKeys itself with a passkey or platform authenticator is a
different feature and is not part of this design.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Role | Passkey provider for websites | Passkey as vault-unlock method (separate feature) |
| Where passkeys live | A field on `Login` items, like TOTP | A separate item type (splits one account into two entries); OS credential-provider APIs (platform-specific, impossible on Linux; possible later) |
| Where the key lives and signs | Rust core only (`passkey.rs`) | Extension/JS signing |
| User verification | Unlocked vault + explicit click ⇒ UV=1 | Desktop confirmation per sign-in; master-password re-prompt |
| Which sites | Only hosts the user granted for inline suggestions | Requesting new/broader host permissions |
| Signature counter | Always 0 | Incrementing (each sign-in would be a server write, and would fail offline) |
| Attestation | Always `none` | Self/packed attestation |
| Algorithms | ES256 (COSE -7) only | EdDSA, RS256 |

Out of scope (YAGNI): passkey import/export (FIDO CXF), attestation other than
`none`, all WebAuthn extensions (PRF, largeBlob, credProps, …;
`getClientExtensionResults()` returns `{}`),
hybrid/caBLE, creating or editing passkeys from the desktop app.

## 3. Data model

`ItemDetails::Login` gains:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
passkeys: Vec<Passkey>,   // at most MAX_PASSKEYS_PER_LOGIN = 8
```

```text
Passkey (inside the encrypted ItemDetails blob)
  credential_id   16 bytes from the CSPRNG
  rp_id           normalized (lowercase, punycode) relying-party ID
  user_handle     opaque bytes from the site, 1..=64
  user_name       site-supplied account name, ≤ MAX_USERNAME_CHARS
  display_name    optional, ≤ MAX_USERNAME_CHARS
  private_key     P-256 scalar, SecretBytes (zeroized on drop, redacted Debug)
  created_at      Unix ms
```

`ItemOverview` gains `has_passkey: bool` only. `rp_id`, user name and
credential ID stay inside the encrypted details; no new plaintext metadata.

`ItemInput` gains a `passkeys` operation limited to **delete by credential
ID**. Passkeys are never created or edited through `ItemInput`; creation only
happens through `passkey_create` (§6).

### 3.1 Format version

An older client that edits a login would deserialize `ItemDetails` without
the unknown `passkeys` field, re-encrypt it, and silently destroy the passkey.
To prevent that, `vault::FORMAT_VERSION` goes from 1 to 2. `sync.rs` already
refuses a header whose `vault_format` differs from its own, so an old client
stops syncing instead of overwriting. v2 reads v1 items unchanged; no SQLite
schema change is needed because the change is inside the encrypted blob.

## 4. Cryptography (`crates/havenkeys-core/src/passkey.rs`)

The only module that touches passkey private keys.

* **Library:** RustCrypto `p256` with the `ecdsa` feature (key generation from
  `OsRng`, ES256 signatures, DER-encoded). CBOR via `ciborium`. Both are
  vetted (maintenance, license, `cargo deny`, `cargo audit`) before being
  added. No hand-written cryptography.
* **`rp_id_allowed(rp_id, page, top)`** — the WebAuthn rule:
  * the page is `https`, or `http://localhost`;
  * `rp_id` equals the page host, or is a registrable parent of it under the
    Public Suffix List (reusing `origin.rs`'s `PageUrl` and `psl`);
  * `rp_id` is never itself a public suffix, and is an IP address only when it
    equals the page host;
  * in an iframe, the top-level page must be same-site with the frame, or the
    request is refused.
* **`client_data_json(type, challenge, origin, cross_origin)`** — built in
  Rust from the page URL supplied by the background worker, so the extension
  does not choose the signed origin.
* **`authenticator_data(rp_id, flags, attested?)`** —
  `SHA-256(rp_id) || flags || counter(0)`, flags UP|UV|BE|BS, plus attested
  credential data (zero AAGUID, credential ID, COSE EC2 public key) on create.
* **`register`** returns the `none`-format attestation object, credential ID,
  COSE public key and `clientDataJSON`.
* **`assert`** returns `authenticatorData`, the DER signature over
  `authenticatorData || SHA-256(clientDataJSON)`, and `userHandle`.

**Origin binding** (same principle as `fill_item`, CLAUDE.md §32): for every
`passkey_get`/`passkey_create`, Rust independently checks the vault is
unlocked, `rp_id_allowed(rp_id, url, top_url)`, and — for `get` — that the
item exists, is a login, holds that credential ID, and the stored `rp_id`
equals the requested one. Only then does it sign.

## 5. Extension

### 5.1 Page-context script

`apps/extension/src/webauthn/page.ts` → `webauthn.js`, registered with
`chrome.scripting.registerContentScripts` using `world: "MAIN"`,
`runAt: "document_start"`, `allFrames: true`, for **exactly the hosts granted
in `registration.ts`** and removed with them. No new permission. Chrome ≥ 120
and Firefox ≥ 128 (already the manifest minimums) support MAIN-world
registration.

It wraps `navigator.credentials.create`, `navigator.credentials.get` and
`PublicKeyCredential.isConditionalMediationAvailable`, keeping references to
the originals. It holds no secrets and makes no decisions: it copies the
request into plain data, and rebuilds a `PublicKeyCredential`-shaped object
from the reply. **On any error, refusal, "use another device", or unsupported
request it calls the original browser function.**

The rebuilt object must pass common site checks: `instanceof
PublicKeyCredential`, `rawId`, `type`, `authenticatorAttachment`,
`toJSON()`, `getClientExtensionResults()`, and on the attestation response
`getPublicKey()`, `getPublicKeyAlgorithm()`, `getAuthenticatorData()`,
`getTransports()`.

### 5.2 Message path

```text
page script (MAIN world, hostile)
   │ CustomEvent carrying a request ID, only plain data
   ▼
content script (ISOLATED)  validates shape, types and sizes
   │
   ▼
background                 url/topUrl from sender.url / sender.tab.url,
   │                       never from the message
   ▼
native host → Rust core    rp_id_allowed, stored rp_id, sign
```

Limits enforced in the content script and again in `havenkeys-protocol`:
challenge ≤ 1 KiB, each credential ID ≤ 1 KiB, ≤ 64 entries in
`allowCredentials`/`excludeCredentials`, user handle ≤ 64 bytes, names within
the existing username limit.

The page can forge every field except its origin, and the origin alone decides
which passkeys are reachable.

### 5.3 Explicit user action

No signature or creation happens without a click inside the extension's own
UI: the inline menu (`menu.html`) for sign-in and the save card (`save.html`)
for creation, both extension-origin iframes the page cannot read or script.
The existing overlay/clickjacking mitigations apply unchanged. A `get()` from
page JavaScript without that click never yields a signature.

## 6. Native messaging protocol

New `Request` variants in `havenkeys-protocol`, `deny_unknown_fields`,
binary fields as base64url strings with length checks:

| Request | Result | Notes |
|---|---|---|
| `find_passkeys {url, topUrl?, rpId, allowCredentials[]}` | `[{itemId, credentialId, title, userName}]` | public data only; filtered by `allowCredentials` when non-empty |
| `passkey_get {itemId, credentialId, url, topUrl?, rpId, challenge, crossOrigin}` | `{credentialId, authenticatorData, clientDataJSON, signature, userHandle}` | |
| `check_passkey_create {url, topUrl?, rpId, userHandle, userName, excludeCredentials[]}` | `{excluded, candidates: [{itemId, title, userName}]}` | candidates: logins matching the page, same username first |
| `passkey_create {url, topUrl?, rpId, userHandle, userName, displayName?, challenge, itemId?}` | `{credentialId, attestationObject, clientDataJSON, publicKey, publicKeyAlgorithm, authenticatorData}` | server write; `itemId` must match the page; `Offline` if the server is unreachable |

No response ever contains a private key. `ErrorCode` gains nothing new; the
existing `Denied`, `Locked`, `Offline`, `InvalidInput`, `TooLarge` cover the
cases.

## 7. UX flows

### 7.1 Create

1. Site calls `create()`. The page script forwards it unless
   `pubKeyCredParams` lacks ES256 (then: fallback).
2. `check_passkey_create`. If `excluded`, reply `InvalidStateError`.
3. The save card opens:

   ```text
   ┌──────────────────────────────────┐
   │ 🔑 Save passkey for github.com?  │
   │  Account: user@example.com       │
   │  ○ Add to "GitHub" (existing)    │
   │  ○ New login                     │
   │  [Save to HavenKeys] [Use another device]
   └──────────────────────────────────┘
   ```

4. **Save** → `passkey_create` → credential returned to the site. Dismiss →
   `NotAllowedError`. **Use another device** → original `create()`.

### 7.2 Sign in

* **Modal `get()`**: the chooser opens anchored at the top of the viewport and
  lists matching passkeys. One match is still one click; never automatic.
* **Conditional mediation** (`mediation: "conditional"`): matching passkeys
  appear in the existing field menu on username focus, above password logins,
  with a passkey badge. `isConditionalMediationAvailable()` returns true only
  where the script runs (granted hosts).
* No match: stay silent, hand the request to the browser.
* After a passkey sign-in nothing else is filled; a later OTP prompt uses the
  existing TOTP flow.

### 7.3 Desktop

* Login detail gains a **Passkeys** section: site (`rpId`), account name,
  created date, **Delete** with confirmation (a server write).
* List rows show a key badge from `has_passkey`.
* Deleting a login that holds passkeys warns that N passkeys go with it and
  site access may be lost.
* No creating or editing passkeys in the desktop app.

### 7.4 Errors and states

| Situation | Behavior |
|---|---|
| Vault locked / desktop not running / integration disabled | Fallback, silently |
| Locked while the desktop is running | Chooser offers "Unlock HavenKeys" and "Use another device" |
| Offline during create | Card: "Can't save while offline", plus fallback |
| No ES256 in `pubKeyCredParams` | Fallback |
| `rpId` not allowed for the origin | `SecurityError`; nothing sent to the desktop |
| Site aborts (`AbortSignal`) or timeout | Close UI, reply `AbortError` |
| Unexpected internal error | Fallback; fixed `ErrorCode` messages, no data |

### 7.5 Logging

Challenges, credential IDs, user handles, rpId/username pairs, signatures and
keys are never logged. `hygiene.test.ts` and the Rust `Debug` redaction tests
are extended to cover the new types.

## 8. Security analysis

New or changed threats, to be added to `threat-model.md`:

* **MAIN-world script.** Runs alongside hostile page JavaScript, which can
  replace or call the wrapper. That grants the page nothing it lacked: it can
  already call WebAuthn itself, and every decision is made in Rust against the
  browser-supplied origin. The wrapper holds no secrets.
* **UV from an unlocked vault.** Anyone at an unlocked, unattended computer can
  sign in with a passkey. Mitigated by auto-lock and OS-lock handling; stated
  as a limitation. UV=1 does not mean biometrics.
* **Synced private keys.** Passkey private keys live in the vault and reach the
  server as ciphertext under the vault key, like passwords. BE/BS flags tell
  relying parties so.
* **Counter 0.** Relying parties cannot detect cloned credentials by counter.
* **Compromised extension.** Can request signatures only for origins the
  browser reports and only for passkeys bound to that rpId — the same bound as
  password fill today.
* **Old clients.** Protected from destroying passkeys by `FORMAT_VERSION` 2
  (§3.1).

## 9. Testing

**Rust**

* `rp_id_allowed`: allowed — `github.com` on `github.com` and
  `accounts.github.com`; denied — on `github.com.evil.com`, `evilgithub.com`,
  `com`, `github.io` (public suffix), IP on a different host, `http:` other
  than localhost, cross-site iframe.
* Round trip: `register` → verify the self-reported public key → `assert` →
  verify the signature over `authData || SHA-256(clientDataJSON)`.
* W3C WebAuthn test vectors; `webauthn-rs` (dev-dependency only) as an
  independent relying-party verifier.
* Flags, counter 0, zero AAGUID, `none` attestation CBOR, exact
  `clientDataJSON` bytes.
* Security regressions: github.com passkey requested from evil.com → Denied;
  `itemId`/`credentialId` not bound to `rpId` → Denied; locked → Locked;
  tampered ciphertext → authentication failure; oversized/malformed/unknown
  fields → rejected without panic.
* Format: v1 vault opens under v2; v1 client refuses a v2 sync header; a
  password edit preserves passkeys; `MAX_PASSKEYS_PER_LOGIN` enforced.
* Fuzz the new protocol messages and `rp_id` parsing with the existing
  harness.

**Extension**

* Wrapper: falls back on every error path; rebuilt objects pass the checks in
  §5.1; abort handling.
* Content-script validation of sizes and types.
* Background never takes the origin from the message.
* No signature without a click in the extension UI.

**Manual** (added to the `security-review.md` checklist): webauthn.io,
github.com, google.com in Chrome and Firefox; conditional mediation; offline
create fallback; locked-vault behavior.

## 10. Delivery order

Each step lands with its tests and a green `cargo test`, `cargo clippy`,
`pnpm test`, `pnpm typecheck`.

1. Core: model, `passkey.rs`, `FORMAT_VERSION` 2.
2. Protocol and native host.
3. MAIN-world script, content-script bridge, background handlers.
4. Save card, chooser, conditional mediation.
5. Desktop: passkey section, badge, delete warning.
6. Docs and hardening: dependency audit, fuzzing, secret-logging audit,
   manual browser checks, `security-review.md` entries.

## 11. Documentation changes

* `CLAUDE.md`: remove passkeys from the out-of-scope list, with an amendment
  note pointing here (as §1 was amended).
* `threat-model.md`, `security-model.md`, `crypto.md` (key format, flags,
  counter), `native-messaging.md`, `autofill.md`, `roadmap.md`,
  `security-review.md`.
* Known limitations stated plainly: UV is not biometric; counter 0; no
  attestation; passkeys only on granted sites; encrypted passkeys are stored
  on the user's server.
