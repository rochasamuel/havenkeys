# Threat Model

HavenKeys is a personal password manager whose vault lives on a server the
user runs, encrypted with keys that server never sees. This document lists the
assets it protects, the adversaries it considers, and — equally important — the
adversaries it does **not** defend against.

> This software has not undergone an independent security audit.

## 1. Assets

| Asset | Sensitivity | Where it lives |
|---|---|---|
| Master password | Critical | Typed by the user; exists transiently in the desktop UI process and Rust core. Never persisted. |
| Master key / KEK | Critical | Rust core memory during unlock only. Never persisted. |
| Vault key | Critical | Persisted **only** wrapped (encrypted) by the KEK. Plaintext only in Rust core memory while unlocked. |
| Passwords, TOTP secrets, notes | Critical | Encrypted at rest. Decrypted on demand in Rust core. |
| Passkey private keys (P-256) | Critical | Inside a login's encrypted details, like its password. Decrypted and used only in the Rust core (`passkey/`); never in any protocol message, command result, log or `Debug` output. |
| Usernames, titles, URLs | High (reveals which services a user has) | Encrypted at rest. Decrypted into memory on unlock (for list/search). |
| Item count, vault creation time, KDF parameters | Low | Plaintext in SQLite (needed to unlock). |
| Auth key | Critical if reused elsewhere, but it unwraps nothing | Derived at unlock from the master password and the Secret Key; sent to the server over TLS, stored there only as an Argon2id hash. |
| Session token | High while valid (24 h) | Server memory and the desktop's memory only. Never on disk; dropped when the vault locks. |
| Account email, device names, item count, change times | Low, but visible to the server | Plaintext in the server's Postgres. |

## 2. Trust boundaries

```text
 ┌──────────── trusted ─────────────┐   ┌────── partially trusted ──────┐   ┌──── untrusted ────┐
 │ Rust core (crypto, vault, lock,  │◄──┤ Desktop renderer (React UI)   │   │ Web pages         │
 │ authorization, origin matching)  │   │ Browser extension, native host│◄──┤ Page JavaScript   │
 └───────────────┬──────────────────┘   └───────────────────────────────┘   │ Page DOM, iframes │
                 │                                                           └───────────────────┘
          encrypted SQLite replica (untrusted storage: may be copied/modified)
                 │
                 │ HTTPS, session token, ciphertext only
                 ▼
 ┌──────────────────────────────────────────────────────────────────────────┐
 │ havenkeys-server + Postgres — untrusted for confidentiality and integrity │
 │ of item content, TRUSTED for availability and for existence: it decides   │
 │ what the vault contains, and a deletion it serves is applied.             │
 └──────────────────────────────────────────────────────────────────────────┘
```

* **Rust core** is the only component that holds keys and enforces policy.
* **The renderer** can ask for secrets only through an explicit allowlist of
  Tauri commands. It never holds the vault key and never performs cryptography.
  We still assume a renderer bug (e.g. XSS through a vault field) is possible,
  so commands are narrow and secrets are returned only on explicit actions.
* **The vault file** is treated as attacker-controlled input: it is parsed
  defensively and every blob is authenticated.
* **The browser extension and the native host** can only reach the core
  through the bridge (`native-messaging.md`). Every request is re-validated
  and origin-bound in Rust, and the master password and keys never cross it.
* **The server** holds ciphertext and metadata. It is untrusted for reading
  (it has no key) and for integrity of item content (every blob and the
  header authenticate under keys it never sees), but it *is* the authority on
  which items exist: see T1c. It identifies every request from its session
  token, never from anything the client's body claims.
* **The sync client** (`havenkeys-sync-client`) treats every answer as
  hostile: responses are bounded while being read, KDF parameters below the
  core's floor are refused before any derivation, and redirects are not
  followed, so a bearer token cannot be sent to a host the user did not
  choose.
* **Web pages** are hostile by default. They cannot message the extension
  (`externally_connectable` is empty, and page script has no extension
  APIs). When in-page suggestions are on, a content script reads the page's
  DOM as untrusted input. It acts only on trusted user events, and fills
  only what the background sends after the user picked an item in an
  extension-origin frame (`autofill.md`).
* **The passkey page script** (`webauthn/page.ts`) runs in the page's own
  JavaScript world, on granted hosts only, so it is exactly as trusted as
  the page: not at all. It holds no secrets and decides nothing; see T8.

## 3. Adversaries we defend against

### T1 — Offline attacker with a copy of the vault file
Stolen laptop backup, synced folder leak, malware exfiltrating files.

* All item content (titles, usernames, URLs, passwords, TOTP secrets, notes,
  timestamps, item type) is encrypted with AES-256-GCM.
* The vault key is random (256-bit, OS CSPRNG) and wrapped with a KEK derived
  from the master password with Argon2id (memory-hard, salted) and, for
  vaults with a Secret Key, the 128-bit Secret Key (HKDF).
* **Copies without the Secret Key** (the server's database, a backup, a
  copied `vault.sqlite3`): guessing the master password is not enough. The attacker
  would also have to guess 128 random bits.
* **A full copy of a device** (which includes `device.json` and so the Secret
  Key), or a password-only vault: the attacker's best strategy is offline
  guessing of the master password, at a cost set by the Argon2id parameters.
  **A weak master password remains guessable**; we enforce a minimum length
  but cannot guarantee strength.

### T1b — The server operator, or someone who has taken the server
They can read, change, delete, reorder and replay everything the server
stores, and they see every request.

* They **cannot read** item content. Overviews and details are AES-256-GCM
  blobs sealed under keys derived from the master password and the Secret
  Key, neither of which reaches the server. The vault header is stored as
  bytes and never parsed there.
* They **cannot forge** item content or a header: both authenticate. A blob
  that does not open under the vault's data key is skipped and counted
  (`SyncReport::skipped_items`), leaving the previous row alone. A header must
  carry an attestation only a vault-key holder could write.
* They **can replay** an item blob they were given earlier. Forging is
  impossible, but an old blob is genuine, so it authenticates and is applied:
  the server can silently return a login to a previous password by re-serving
  its old blob at a higher revision. Items have no revision floor — only the
  header does (`max_header_rev`), which is what stops an old *header* being
  replayed to make a retired master password work again on a device that has
  already seen a newer one. Two consequences worth stating plainly:
  * A device signing in **for the first time** has no floor to compare
    against, so a captured pre-rotation header plus the retired master
    password and the Secret Key will open the vault on a fresh device.
  * Password history (5 entries) is what limits an item rollback; a patient
    server can cycle through it.
* They **cannot impersonate a device**: the auth key is stored only as an
  Argon2id hash, and `auth/params` answers identically for addresses that have
  no account, so the server is not an account-enumeration oracle either.
* They **cannot change the account's password with a stolen session token
  alone**: `POST /v1/account/credentials` also requires the *current* auth
  key, checked against the stored verifier under the same rate limiting as
  login. A successful change deletes every other session on the account.
* They **can delete or withhold** data — see T1c — and they see metadata: the
  account's email, how many items exist, how large each is, when each changed,
  how many devices there are and what they are called, and each device's IP
  and rough activity. That metadata is the price of this design and is not
  encrypted.

### T1c — A server that destroys data (inherent, not a flaw)
The server is the single writer. A deletion it serves is indistinguishable
from one the user made, because there is nothing else for a device to compare
it against: the local SQLite is a replica that follows the server, not an
independent copy.

* A compromised or failing server can therefore **empty a vault**, and every
  device will apply it.
* The mitigations are operational, not cryptographic: **tested backups**
  (`docs/deployment.md` §5 — a restore drill is a prerequisite of storing a
  real vault) and the user noticing. `SyncReport` carries the deletion count
  so the UI can say how much disappeared.
* Availability is a correctness concern here, not a convenience: a server
  that is down means no login can be saved, no password rotated, no item
  deleted. Reads keep working offline.

### T2 — Attacker who can modify the vault file
* Every blob is authenticated (AEAD). Associated data binds each ciphertext to
  the vault ID, the item ID, and the blob role (overview/details/settings), so
  blobs cannot be swapped between items or roles without detection.
* Tampered blobs produce a generic authentication error; no plaintext is
  returned.
* **Not defended:** rollback (replacing the whole file, or a row, with an older
  valid version) and deletion of items. See Known limitations.

### T3 — Malicious web pages
Page scripts may fabricate forms, hide inputs, use iframes, spoof URLs, or try
to message the extension. Mitigations (detailed in `security-model.md` and
`autofill.md`):

* Origin binding is enforced in Rust, with conservative PSL-based domain
  matching.
* Frames are served only if the item also matches the top page, so a
  `github.com` login frame in `evil.com` gets nothing.
* Page URLs come from the browser, never from the page.
* Fills happen only after a trusted user click in an extension-origin menu
  the page cannot read or script. Clicks are ignored until the menu has been
  visible for 400 ms and, on Chromium, only while it is unobscured.
* Fills go only to visible, enabled fields of the group the user clicked.
  Hidden fields are never filled.
* Fills go to one frame, only if that frame is still on the matched origin.
* Synthetic events are ignored.
* A save is offered only for passwords the user typed, so a page cannot use
  save prompts as a password oracle.
* No secret data goes into the DOM beyond the filled input values.

### T4 — Compromised or buggy extension or native host
The Rust side re-validates every request: it must be a known request type,
under the size and rate limits, with the vault unlocked, integration switched
on, and the item's own rules matching the page. The extension never receives
the vault key or the master password, `find_matches` returns no secrets, and
an unknown item ID looks the same as "not saved for this site". A compromised
extension can still obtain the password of any login **for a site whose URL
it claims**, within the rate limit. The browser-reported tab URL is only as
trustworthy as the extension that reports it.

### T5 — Compromised renderer / UI process
* No cryptography in JavaScript; no keys in JavaScript.
* Strict Tauri command allowlist; no filesystem, shell or HTTP plugins. The
  dialog, autostart and single-instance plugins are used from Rust only; the
  renderer is granted none of their commands.
* Strict CSP, no remote content, navigation outside the app is blocked.
* A fully compromised renderer **can** request any secret while the vault is
  unlocked (it is the UI, after all). We limit the blast radius (it cannot read
  the vault file, derive keys, or keep access after lock) but do not claim to
  prevent this.

### T6 — Accidental disclosure
Secrets must not end up in logs, error messages, panic messages, `Debug`
output, URLs, window titles, notifications or browser storage.

* Secret-bearing Rust types implement a redacting `Debug`.
* Errors are fixed strings without payloads.
* The core never logs item content; the logging policy is enforced by review
  and a test that scans the crate for print macros.
* Clipboard: copying is explicit, done in Rust, and cleared after a
  configurable timeout if the clipboard still holds our value.

### T7 — Shoulder surfing / unattended unlocked machine
Passwords are masked by default; auto-lock timeout (keeps running while the
window is hidden in the tray); lock on quit; lock when the OS session locks
(Windows, Linux with logind); lock on system suspend (detected by
wall-clock vs monotonic clock divergence).

### T8 — Passkeys
HavenKeys acts as a WebAuthn authenticator for websites
(`docs/superpowers/specs/2026-09-23-passkeys-design.md`). The private key is
generated, stored and used only in the Rust core; the extension relays
public data and the user's click. New or changed threats:

* **The MAIN-world script.** To answer `navigator.credentials.create/get`,
  a script (`webauthn/page.ts`) runs in the page's own world, alongside
  hostile page JavaScript, which can call, replace or observe the wrapper,
  and forge every field of a request. *Mitigation:* the wrapper holds no
  secrets and makes no decisions. A request crosses to the isolated-world
  bridge as a JSON string, is parsed there with exact shapes and limits, and
  the background takes the frame URL (and the top URL for iframes) from the
  browser's sender data, never from the message. Rust checks the relying-party
  ID against that URL (`authorize_rp`), and a passkey is used only if its
  stored rpId equals the checked one. *Residual:* the page gains nothing it
  lacked — it could already call WebAuthn itself — but it can see that
  HavenKeys answered, and it can hand every request to the browser instead.
* **UV from an unlocked vault.** Assertions and new credentials carry
  UV=1 ("user verified") because the vault is unlocked and the user clicked
  in the extension's own UI. That is not biometrics and not a fresh
  verification: anyone at an unlocked, unattended computer can sign in with a
  passkey. *Mitigation:* auto-lock, lock on OS screen lock and on suspend
  (T7). *Residual:* accepted and stated (`security-review.md` PK1).
* **Synced private keys.** Passkey keys live in the vault and reach the
  server as ciphertext under the vault data key, like passwords (T1b applies
  unchanged). The BE and BS flags are set, which tells relying parties the
  credential is backup-eligible and backed up. *Residual:* a device with the
  vault unlocked holds every passkey; there is no device-bound passkey.
* **Signature counter 0.** The counter is always 0, so relying parties cannot
  use it to detect a cloned credential. A counter that incremented would make
  every sign-in a server write and would fail offline. *Residual:* accepted
  (`security-review.md` PK2).
* **Compromised extension.** It can request a signature only for a frame URL
  the browser reports, and only with a passkey whose stored rpId the core
  allows for that URL — the same bound as password fill (T4). It can create
  passkeys (a server write) for sites it names, within the rate limit, and a
  re-registration for an account that already has a passkey replaces the old
  one, with no history to recover it from (`security-review.md` PK5).
  *Residual:* as T4; the browser-reported URL is only as trustworthy as
  the extension that reports it.
* **Old app versions.** A HavenKeys build from before passkeys does not know
  the `passkeys` field, so editing a login there and saving it drops the
  login's passkeys. *Mitigation:* operational only — update every device
  before saving a passkey. *Residual:* accepted (`security-review.md` PK3).
* **Clickjacking the passkey card.** The chooser and save card are an
  extension-origin frame with the same click guard as the menu. On Firefox
  only the 400 ms delay applies, so a page could trick a click on its *own*
  passkey prompt. *Residual:* the result is a sign-in or a new passkey for
  that page's own rpId, nothing else (`security-review.md` PK7).

## 4. Out of scope (not defended)

* **Malware running as the same OS user while the vault is unlocked.** It can
  read process memory, inject into the UI, log keystrokes, or read the
  clipboard. No local password manager can fully defend against this. It can
  also connect to the bridge socket, or run the native host itself, and ask
  for logins as the extension does (rate-limited). This is easier than
  reading memory; see `native-messaging.md` §8.
* **Kernel/root compromise, hardware attacks, cold-boot / DMA attacks.**
* **Keyloggers capturing the master password.**
* **Memory forensics after lock.** We zeroize key material and decrypted
  buffers we control, but copies may remain in allocator free lists, the
  WebView's JS heap, IPC buffers, or swap. See `security-model.md` §Memory.
* **Rollback attacks** on the vault file.
* **Reading the OS keychain entry from another process running as the same
  user.** Any such process can usually read a Linux Secret Service or
  Windows Credential Manager entry; macOS may prompt for access. This is the
  same trust boundary as the plaintext `device.json` fallback it protects
  against instead — it protects copies of the vault that are not on one of
  your devices, not this device from something already running as you.
* **The vault file `vault.sqlite3.removed-<timestamp>` left by "Remove this
  device".** It is kept, deliberately, as ciphertext rather than deleted —
  it may be the only local copy if the account's server is gone and export
  does not exist yet — so it remains on disk under the same T1 protection
  (master password and Secret Key) indefinitely, with no prompt to clean it
  up later.
* **Side channels** beyond what the underlying audited libraries protect
  against (RustCrypto `aes-gcm` uses constant-time implementations when AES-NI
  is available).
* **A weak master password.**
* **Clipboard managers / other applications reading the clipboard** during the
  clear window.

## 5. Abuse cases tracked as regression tests

| # | Attack | Expected | Test location |
|---|---|---|---|
| A1 | Credential request for github.com while on evil.com | DENIED | `crates/havenkeys-core/tests/security.rs`, `crates/havenkeys-bridge/tests/bridge.rs` |
| A2 | Extension requests arbitrary item ID | Only if item matches origin; unknown IDs indistinguishable | `crates/havenkeys-core/tests/security.rs`, `crates/havenkeys-bridge/tests/bridge.rs` |
| A3 | Vault locked, secret requested | DENIED | `crates/havenkeys-core/tests/security.rs`, `crates/havenkeys-bridge/tests/bridge.rs` |
| A4 | Ciphertext modified | Auth failure, no plaintext | `crates/havenkeys-core/tests/security.rs`, `blob.rs` |
| A5 | Malformed native message | Rejected, no crash | `crates/havenkeys-protocol/tests/messages.rs` (incl. fuzz), `crates/havenkeys-bridge/tests/bridge.rs`, `crates/havenkeys-native-host/tests/host.rs` |
| A6 | Oversized native message | Rejected by size limit | `crates/havenkeys-protocol/src/frame.rs`, `crates/havenkeys-bridge/tests/bridge.rs`, `crates/havenkeys-native-host/tests/host.rs` |
| A7 | Page creates thousands of inputs | No significant slowdown | `apps/extension/src/autofill/autofill.test.ts` (5,000 inputs, bounded group) |
| A8 | Blob swapped between items / roles | Auth failure | `crates/havenkeys-core/tests/security.rs` |
| A9 | Unknown vault format version | Refused safely | `crates/havenkeys-core/tests/security.rs` |
| A10 | Login frame of github.com embedded in evil.com | DENIED (item must match top page too) | `crates/havenkeys-core/tests/security.rs`, `crates/havenkeys-bridge/tests/bridge.rs`, `inline-handler.test.ts` |
| A11 | Page synthesizes clicks/keys/focus to open menus or trigger fills | Ignored (`isTrusted`) | `apps/extension/src/content/content.test.ts` |
| A12 | Fill arrives after the frame navigated to another origin | Refused by the content script (origin check; Chromium also pins the document) | `content.test.ts` |
| A13 | Menu frame or other tab tries to pick an item the menu did not offer | Refused | `inline-handler.test.ts` |
| A14 | Page plants a password and forges a submit to probe the vault | No report, no prompt | `autofill.test.ts`, `content.test.ts` |
| A15 | Extension overwrites another site's login, or floods password changes | DENIED / one change per item per 10 min; old passwords kept in history | `crates/havenkeys-core/tests/security.rs`, `crates/havenkeys-bridge/tests/bridge.rs` |
| A1p | Passkey assertion for github.com requested from evil.com (or from a look-alike, a public suffix, plain http, a cross-site frame) | DENIED; nothing signed | `crates/havenkeys-core/src/passkey/rp.rs`, `crates/havenkeys-core/tests/passkeys.rs` (`attack_wrong_origin_is_denied`), `crates/havenkeys-bridge/tests/bridge.rs` (`passkey_attacks_through_the_bridge`), `tests/fuzz.rs` (`fuzz_authorize_rp`) |
| A2p | Extension asks to sign with an arbitrary item ID / credential ID, or one bound to another rpId | DENIED, indistinguishable from "no such passkey" at the bridge | `crates/havenkeys-core/tests/passkeys.rs` (`attack_arbitrary_ids_are_denied`), `crates/havenkeys-bridge/tests/bridge.rs` |
| A3p | Vault locked, passkey sign-in or creation requested | `locked`; nothing signed or created | `crates/havenkeys-core/tests/passkeys.rs` (`attack_locked_vault_is_refused`), `crates/havenkeys-bridge/tests/bridge.rs` |
| A16 | Page forges or oversizes a WebAuthn request (challenge, user handle, rpId, credential lists, unknown fields) | Handed to the browser by the page script, or rejected by the bridge parser, the native host and the Rust protocol | `apps/extension/src/webauthn/page.test.ts`, `messages.test.ts`, `crates/havenkeys-protocol/tests/messages.rs` (`passkey_requests_are_bounded`) |
| A17 | Page tries to get a passkey signature without a click in the extension's frame | No signature: only a pick from the passkey frame or the field menu signs | `apps/extension/src/background/webauthn-handler.test.ts` |
