# Threat Model

HavenKeys is a local-first personal password manager. This document lists the
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
| Usernames, titles, URLs | High (reveals which services a user has) | Encrypted at rest. Decrypted into memory on unlock (for list/search). |
| Item count, vault creation time, KDF parameters | Low | Plaintext in SQLite (needed to unlock). |

## 2. Trust boundaries

```text
 ┌──────────── trusted ─────────────┐   ┌────── partially trusted ──────┐   ┌──── untrusted ────┐
 │ Rust core (crypto, vault, lock,  │◄──┤ Desktop renderer (React UI)   │   │ Web pages         │
 │ authorization, origin matching)  │   │ Browser extension, native host│◄──┤ Page JavaScript   │
 └───────────────┬──────────────────┘   └───────────────────────────────┘   │ Page DOM, iframes │
                 │                                                           └───────────────────┘
          encrypted SQLite file (untrusted storage: may be copied/modified)
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
* **Web pages** are hostile by default. They cannot message the extension
  (`externally_connectable` is empty, and page script has no extension
  APIs). When in-page suggestions are on, a content script reads the page's
  DOM as untrusted input. It acts only on trusted user events, and fills
  only what the background sends after the user picked an item in an
  extension-origin frame (`autofill.md`).

## 3. Adversaries we defend against

### T1 — Offline attacker with a copy of the vault file
Stolen laptop backup, synced folder leak, malware exfiltrating files.

* All item content (titles, usernames, URLs, passwords, TOTP secrets, notes,
  timestamps, item type) is encrypted with AES-256-GCM.
* The vault key is random (256-bit, OS CSPRNG) and wrapped with a KEK derived
  from the master password with Argon2id (memory-hard, salted).
* The attacker's best strategy is offline guessing of the master password, at a
  cost set by the Argon2id parameters. **A weak master password remains
  guessable**; we enforce a minimum length but cannot guarantee strength.

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
* Strict Tauri command allowlist; no filesystem, shell, HTTP or dialog plugins.
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
window is hidden in the tray); lock on quit; lock on
system suspend (detected by wall-clock vs monotonic clock divergence).

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
