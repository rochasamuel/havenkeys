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
| Account email, device names (the computer's hostname or the phone's name), item count, change times | Low, but visible to the server | Plaintext in the server's Postgres. |
| Android unlock bundle (vault key + auth key, enrolment time, boot count) | Critical | Only when biometric unlock is on. Sealed by a Keystore AES-256-GCM key that every use must unlock with a strong biometric; `noBackupFilesDir/unlock-bundle.bin`. Refused after 14 days or a reboot (`security-model.md` §22.2). |
| Android Secret Key file | High (half of what opens a copy of the vault) | `filesDir/secret-key-<account>.bin`, sealed by a Keystore key that needs no user authentication. Never written in plaintext. |

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
  APIs). On the hosts the extension has access to (every http/https page by
  default), a content script reads the page's DOM as untrusted input. It acts only on trusted user events, and fills
  only what the background sends after the user picked an item in an
  extension-origin frame (`autofill.md`).
* **The passkey page script** (`webauthn/page.ts`) runs in the page's own
  JavaScript world, on granted hosts only, so it is exactly as trusted as
  the page: not at all. It holds no secrets and decides nothing; see T8.
* **The Android app** (`apps/android`, spec
  `2026-10-01-android-app-design.md`) is a second shell over the same core,
  through `havenkeys-mobile`. Its Kotlin code is partially trusted, like the
  desktop renderer: it collects facts from Android and talks to the Keystore
  and BiometricPrompt, but Rust decides every match, target and fill. The
  app being filled, everything in its `AssistStructure` and any WebView it
  hosts are untrusted. Android itself (the autofill framework, the package
  manager's signing certificates, the Keystore) is trusted. See T12.

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
  * A **splice** of halves from different versions (an old overview with newer
    details, or the reverse) is refused: the details blob's AAD binds the
    SHA-256 of its overview blob, so the pair does not open and is skipped.
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
* A page cannot start or extend an automatic sign-in run: it starts only
  from a trusted pick, and continuation never crosses origins (T9).

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
* **QR scan as a screen reader.** A compromised renderer can call
  `scan_totp_qr` while the vault is unlocked, which captures every monitor
  and every open window, including windows hidden behind others (not
  minimized ones, not HavenKeys' own). It can then save into an item a TOTP
  QR code shown in any of them but not yet
  in the vault (while online, since saving writes to the server), and read
  live codes for it through `get_totp_code` — as it already can for every
  item it can see. It never receives pixels, other decoded text, or the
  TOTP seed itself (`reveal_secret` refuses TOTP). On X11, Windows and
  macOS (after the one-time permission) the capture is silent; Wayland's
  portal prompts every time.

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

### T7b — Export and backup
* **Unattended unlocked desktop.** Every export asks for the master password
  again, checked in Rust, so walking up to an unlocked screen does not
  produce a file of every password. That is what the re-check is for: a
  person at the keyboard.
* **Stolen backup.** Sealed with Argon2id (128 MiB, 4 iterations at the
  default) and AES-256-GCM under the backup password; no Secret Key is mixed
  in, so the backup password's strength is all that protects it. The KDF
  parameters are authenticated, so they cannot be rewritten to make
  guessing cheaper.
* **Crafted backup.** Size, version, KDF bounds, closed structs and an item
  cap are checked before anything is restored; every item is then parsed and
  validated on its own (an item that fails counts as failed, the rest
  restores), passkeys are re-checked, and nothing existing is overwritten.
  Restored URL rules and timestamps are trusted as much as an import the
  user chose. An item deleted since the backup comes back by reviving the
  server's tombstone; an item another device wrote meanwhile is skipped.
* **Plaintext export left on disk.** Explicit confirmation, a warning, mode
  0600, no passkeys, and an encrypted backup as the default. We do not delete
  the file; keeping it safe is the user's responsibility.
* **Compromised renderer.** It cannot choose the path and cannot export
  without the master password. This is not a defence against it, though:
  while the vault is unlocked, a compromised renderer can already read items
  one by one through the existing reveal commands (T6), so the re-check
  only stops it getting everything in one file.

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
* **Presence probing.** A page can list credential IDs in
  `excludeCredentials` or `allowCredentials` and watch what happens, to learn
  whether HavenKeys holds a passkey for one of its accounts. *Mitigation:* an
  excluded credential is reported (`InvalidStateError`) only after the user
  clicks **Close** on the "already saved" card, never at once. *Residual:*
  a card appearing, or the field menu answering a conditional request, shows
  that HavenKeys has something for the site; with `allowCredentials`, a
  chooser versus an immediate fallback tells whether a listed credential is
  held. The page learns this only about its own rpId (`security-review.md`
  PK19).
* **Lost sessions.** A Manifest V3 worker can be suspended, losing every
  passkey session, and Firefox unloads the isolated bridge (not the page
  script) when the extension is disabled or updated. *Mitigation:* the
  bridge pings its session every 20 s and falls back (modal) or asks again
  (conditional) when it is gone; the page script falls back to the browser
  when no bridge acknowledges within 1 s and has its own ceiling; a card
  whose session is gone can still be closed. *Residual:* none known beyond
  the delay (`security-review.md` PK17, PK20).
* **The automatic upgrade's relaxed click rule.** A site's automatic passkey
  upgrade — `create()` with `mediation: "conditional"`, right after HavenKeys
  fills a password on that site — can save a passkey with no click in
  HavenKeys UI. The consent is that password fill itself: Rust remembers, in
  the unlocked session only, which login it filled on which site
  (`VaultService::fill_for_page` → `Session::recent_fills`), and
  `upgrade_for` allows the silent save only within the next 5 minutes
  (`UPGRADE_WINDOW_MS`), for that same login, on that same site, and only
  when the account name the site sent matches (folded) or the login has
  none. `stage_passkey_create` re-derives this decision itself and refuses
  (`Denied`) unless it comes out `Auto` for exactly the requested item — the
  extension's claim that a request is the automatic upgrade is not trusted.
  *Mitigation:* the vault setting `auto_passkey_upgrade` (on by default)
  turns this off; when off, the save card asks first, as for any other
  `create()`. A silent save only ever *adds* a passkey: a conditional create
  for an account (rpId and user handle) that any login already holds is
  refused, so replacing a passkey always needs the card. Each fill grants at
  most one silent passkey: a successful conditional create spends the fill
  for that login on that site, so a script on the site cannot repeat it with
  fresh user handles. *Residual:* the Rust check keeps the *silent* path
  narrow against extension bugs and hostile pages — a page cannot trigger a
  silent save without a recent HavenKeys fill of that login on that site. It
  is not a boundary against a compromised extension, and adds no capability
  to one: such an extension can already create passkeys for sites it names
  through the clicked path (`passkey_create` with `conditional: false`),
  which the core cannot tell from a real click ("Compromised extension"
  above) (`crates/havenkeys-core/tests/passkeys.rs`
  `conditional_create_needs_auto_for_exactly_that_login`,
  `upgrade_is_per_site_and_never_for_look_alikes`,
  `conditional_create_never_replaces_the_filled_logins_own_passkey`,
  `one_fill_grants_one_silent_passkey`;
  `crates/havenkeys-bridge/tests/bridge.rs` `passkey_upgrade_through_the_bridge`).
* **Fill memory.** The recent-fill list that grants that consent holds only
  an item ID and a site (the page's registrable domain, or its host with
  none), never a username, password or full URL. It exists only in the
  unlocked session's memory, is capped at 16 entries, is never written to
  disk, and is dropped on lock along with the rest of the session
  (`fill_memory_is_dropped_on_lock`).
* **`passkey_status` oracle.** The field menu's "you have a passkey" /
  "this site supports passkeys" hint asks the desktop whether it holds any
  passkey the page's rpId may use (`has_passkey_for_page`, wire request
  `passkey_status`). It is Lookup-class, like `find_matches`, and is asked
  only when the user opens a field menu on a page that already lists a saved
  login. The answer itself never reaches the page — it only changes which
  row our menu shows — but a page can see the menu frame's size change,
  which is the same residual signal `autofill.md` already documents for
  menu presence generally.
* **Directory data.** The "supports passkeys" hint's site names and help
  links (`apps/extension/src/data/passkey-sites.json`) are third-party text
  from the 2factorauth Passkeys Directory, not verified by HavenKeys or by
  the site itself. It is a snapshot committed to the repository and reviewed
  like code (`scripts/update-passkey-directory.mjs`), never fetched at
  runtime; the extension renders the name with `textContent` only, keeps
  only `https:` help links, and opens one only on a trusted click.

### T9 — Automatic sign-in
After the user picks a login, HavenKeys may finish that one sign-in — press
the button, fill a following password step and the TOTP code — without
further clicks
(`docs/superpowers/specs/2026-09-26-auto-sign-in-design.md`). This amends
rule #6 for one bounded case.

* **Only a trusted pick starts a run.** The pick is the existing guarded
  click in the extension's menu frame or the popup. A page cannot start,
  extend or re-target a run: `cs_run_step` is accepted only from the run's
  own tab, frame and origin, only for the step that comes next, once.
* **`autoSubmit` is decided in Rust.** `VaultService::auto_sign_in_for`
  computes `settings.auto_sign_in && item.auto_sign_in` after the existing
  origin check, so the extension cannot turn it on for a login the user
  switched off, and a page cannot turn it on at all.
* **Origin binding.** Continuation never crosses origins, even under a
  whole-site rule: a redirect to another subdomain gets no no-click fill,
  though the menu still works there manually. Every step's value is
  re-requested from Rust for the frame's current URL, so the origin check
  runs again each time, not just once at the pick.
* **No secrets in run state.** The run (`signin-run.ts`) holds only an item
  ID, the origin, the tab and frame ID, the current step and an expiry, in
  the background worker's memory only. It is never persisted and is dropped
  on lock, on the tab closing, on a new pick, on an origin change, and after
  2 minutes.
* **Accepted risk.** For up to 2 minutes after a pick, a page on that same
  origin receives the password and the current TOTP code without further
  clicks, the same class of relaxation as the automatic passkey upgrade
  (T8, "The automatic upgrade's relaxed click rule"). Script on that origin
  could already obtain the password after the single manual pick; the new
  part is that the OTP no longer needs its own click. Bounded by the origin
  binding, the forward-only single-use steps, the 2-minute window, and the
  global and per-login off switches. *Residual:* not a boundary against a
  compromised extension, which can call `fill_item` / `get_totp` directly
  regardless (`security-review.md` AS1).
* **Raising the desktop window.** A compromised extension can send
  `open_item` for a login saved for the page it reports, which brings the
  desktop window forward on that login's editor. It receives nothing, the
  editor saves nothing without the user, an unsaved edit elsewhere is
  protected by a discard prompt, and the request shares the `secret` rate
  limit.
* **A late step confirmation from a replaced run.** `cs_run_step` carries no
  run identity of its own — only the sender's tab, frame and origin, which
  the background compares against the live run. In a narrow race, a leftover
  step confirmation from a run the user has already replaced with a new pick
  in the same tab, frame and origin could advance the new run one step
  early. *Mitigation:* every value the step would fill is still re-requested
  from Rust for the frame's current URL, and the login was just picked by
  the user on that same origin (`security-review.md` AS2).
* **Wrong button.** The ambiguity rule and negative-word list make a
  mis-press unlikely; a mis-press can only act on the matched origin, so it
  cannot leak credentials elsewhere.

### T10 — The desktop updater
From 0.9.0 the desktop app checks GitHub Releases for newer signed builds and
can install one on the user's click
(`docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md`;
`security-model.md` §18). This is a new, bounded exception to "minimal
network exposure": the app can now reach `github.com` and its download
redirect host on its own.

* **A malicious update, from a compromised GitHub account, repository, CDN or
  network path.** Whoever controls those can serve any bytes they like as the
  next "release", but `tauri-plugin-updater`'s `download` verifies the
  minisign signature against the public key committed in `tauri.conf.json`
  before it returns any bytes. *Mitigation:* the signature check, which needs
  the private key to pass. *Residual:* none beyond the key itself — see next.
* **Theft of the updater's private key and its passphrase.** Whoever holds
  both can sign a release that every installation will accept and run,
  including the Rust core that holds the vault key — this is the actual trust
  root behind every future update. *Mitigations:* the private key and
  passphrase exist only in GitHub repository secrets (`TAURI_SIGNING_PRIVATE_KEY`,
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and an offline backup, never in the
  repository or a commit; releases stay drafts until the owner reviews and
  publishes them, so a forged draft is never auto-published; `development.md`
  documents a rotation procedure (ship one release signed with the old key
  whose config trusts the new public key, then sign every later release with
  the new one). *Residual:* accepted as a trust assumption — see
  `security-review.md` for its severity.
* **Replay of an old signed release** to push a device backwards to a version
  with a known flaw, including a crafted `latest.json` (which is not signed)
  that pairs a high version number with an old release's URL and signature.
  *Mitigation:* `requireSignedVersion: true` — each signature's trusted
  comment binds the version it was signed for, and an artifact whose signed
  version differs from the announced one (or that has none) is rejected;
  the plugin's comparator then installs only a version strictly greater than
  the one running. *Residual:* depends on every release being built with
  `@tauri-apps/cli` 2.12.0 or later (checked before publishing, see
  `development.md`).
* **Loss of the private key** (not an attack — a lost secret, a departed
  operator with no backup). *Consequence:* no further release can be signed,
  so no installation will ever be offered another automatic update again;
  every user has to install the next version by hand once new key material
  exists (a fresh key, and every future release re-signed with it, accepted
  the same way the very first updater-enabled release was).
* **Privacy.** GitHub sees the IP address, the time, and the fact that a
  HavenKeys installation is checking, on every automatic and manual check.
  *Mitigation:* Settings → Updates can turn the automatic check off; nothing
  is sent unless the user checks or updates by hand.

### T11 — Sign in with Google, Microsoft, GitHub, Apple
After the user picks a "Sign in with" login, HavenKeys may press that site's
provider button, click the saved account (or "Use another account") in the
provider's own chooser, and, if the account is not already signed in there,
sign it in with the vault's own login for that provider and account —
username, password and TOTP if it has one
(`docs/superpowers/specs/2026-09-28-sign-in-with-design.md`,
`docs/superpowers/specs/2026-09-29-sign-in-with-provider-login-design.md`).
This amends rule #6 for one more bounded case, alongside T9 and the automatic
passkey upgrade (T8). Details in `autofill.md` §"Sign in with", the wire
messages in `native-messaging.md` §"Sign in with", the mechanism in
`security-model.md` §17.

* **A fake provider button.** A hostile or careless page can put a
  "Sign in with Google" label on any element. *Mitigation:* pressing it does
  nothing HavenKeys did not already permit the page to do — it only
  navigates that same page, exactly as the page's own JavaScript could have.
  The run never treats the press itself as success; it continues only when a
  **later** frame lands on one of the provider's real origins, checked
  against the frame's own origin in the background, never against anything
  the page claims. *Residual:* none beyond a wasted click if the label was a
  lie.
* **A hostile page triggering the balloon.** Any page can have provider
  buttons, so any page can make the balloon appear. *Mitigation:* the
  balloon only ever lists **that page's own** matching logins — the same
  `find_matches` origin binding as the field menu — and shows no secret.
  Nothing is pressed or clicked until the user picks a row inside the
  balloon, which is an extension-origin frame the page cannot read or script
  and which requires the same trusted-click, 400 ms-armed guard as the field
  menu (`autofill.md`, "Suggestion UI"). *Residual:* a page learns that the
  user has a saved "Sign in with" login for it, the same class of signal
  `autofill.md` already documents for the ordinary menu and save prompts.
* **A provider look-alike origin.** A page cannot get the run to click an
  account, or complete a login, on `accounts-google.com` or
  `login.microsoftonline.com.evil.com`. *Mitigation:* the run's provider
  origins are an **exact-origin list** returned by Rust (`sso.rs`), never a
  suffix, prefix, or pattern match; a frame whose origin is not literally in
  that list is refused, for the press-confirmation check, the choose step
  and the login hand-off alike. *Residual:* none — this is the same
  exact-origin discipline `autofill.md` §Domain matching applies everywhere
  else, applied to a fixed, closed table instead of a per-item rule.
* **Site X triggering a login on the provider's real page.** Site X's own
  page cannot ask for a provider login directly; it can only get the user to
  pick its own "Sign in with" row, which starts the run, and the login
  hand-off fires only if the *provider itself* later shows a login form on
  one of the Rust-listed origins. *Mitigation:* the hand-off fills exactly
  the **single** provider login whose saved username equals the run's
  account — found by Rust's own `find_matches` for that provider page,
  independent of anything site X sent — after the same origin check
  `fill_item` always runs. **Two (or zero) matching provider logins and
  nothing is filled**; the run simply ends. *Residual:* none beyond the
  "fake provider button" and "look-alike origin" bullets above — site X
  never learns whether a hand-off happened, or with which login.
* **A chooser or login-form spoof inside the provider's own page.** A page
  that has compromised the *provider's own origin* (or a same-origin script
  it can inject there) could draw a fake row and get HavenKeys to click it,
  draw a fake login form and get HavenKeys to fill the provider login's
  email and password into it, or grab the account text HavenKeys reads for a
  save prompt. **Out of scope:** a compromised provider origin is outside
  this threat model — see §2, "Web pages are hostile by default," which does
  not extend to trusting an attacker who already controls
  `accounts.google.com` itself. What remains bounded even then: the click or
  fill only lands on an element already on that page, the fill is always the
  **one** provider login Rust matched to that exact origin and account — not
  a password for any other site or account — and a save prompt from a forged
  account name is still shown to the user before anything is written
  (`autofill.md`, "Save detection").
* **Consent screens.** A keyword match
  (`isConsentScreen`/`isConsentLabel`) refuses to press or click a row on a
  page that looks like a permissions or consent screen. *Mitigation:*
  documented as a heuristic, not a guarantee, in `security-model.md` §17.
  *Residual:* an unrecognized consent screen that also shows a row whose
  email matches the saved account (`security-review.md`, "Sign in with").
* **The provider login's secrets travel only to the provider's own,
  Rust-checked origin.** The login hand-off can now fill, and with all three
  switches on submit, the provider login's password and TOTP code — but only
  on the exact origin `fill_item`/`get_totp` returned them for, only for the
  single matching provider login, and never to site X: site X cannot see the
  provider's page or learn whether the hand-off ran. Page-sourced account
  text is still validated in Rust (254 characters, no control characters)
  and is only ever a suggestion the user can edit before saving.

### T12 — The Android app
The Android app (`docs/android.md`; `security-model.md` §22) holds the same
vault as the desktop and fills logins in other apps and in browsers through
Android Autofill. Nothing in this section has been run on a phone or an
emulator yet; the manual checklists in `security-review.md` ("Android M1",
"Android M2" and "Android M3") cover each item.

* **A malicious app with a borrowed package name.** Anyone can install an
  app called `com.github.android`. *Mitigation:* Rust identifies an app by
  its package name **and** the SHA-256 of its signing certificates, which
  Android's package manager reports (`CallerIdentity.kt`); an app binding or
  a Digital Asset Links statement matches only that pair. A package Android
  will not describe gives no certificates, and Rust refuses a target without
  one. *Residual:* none known for a sideloaded copy; an app whose real
  signing key was stolen is that app, as far as Android and HavenKeys can
  tell.
* **A WebView reporting another site's domain.** An app controls its own
  WebView and the `webDomain` it reports. *Mitigation:* only a browser on
  the fixed privileged list (`crates/havenkeys-core/data/android-browsers.json`,
  package and release certificate) is trusted to report a domain; every
  other caller is an app target, whose `webDomain` is ignored
  (`app_target.rs::classify`). *Residual:* none for matching; an app can
  still show a convincing login page and ask the user to type a password.
* **A non-privileged "browser".** A browser that is not on the list, or a
  listed package signed by someone else, is an app target: it gets logins
  only through a binding the user made or a site's Digital Asset Links file.
  *Residual:* such browsers fill nothing by domain; the list is refreshed
  by hand (`scripts/update-android-browsers.sh`).
* **A forged assist structure.** The authentication rows use mutable
  PendingIntents, so the app being filled starts HavenKeys' fill activity
  and could replace the structure Android attaches. *Mitigation:* the
  activities use the structure only when its package equals
  `getCallingPackage()` (`structureNamesCaller`), and Rust then binds the
  fill to that package's certificates. *Residual:* an app can only ask for
  its own logins.
* **Gated rows fired without a tap.** With "Confirm before filling" on, the
  app being filled holds each row's IntentSender and can fire it itself,
  and, because the Intent is mutable and `Intent.fillIn` merges extras, swap
  the item ID and mode it carries. While the vault is unlocked, the activity
  answers without showing anything. *Mitigation:* Rust returns only items
  matched to that same caller, exactly as for direct fill, whatever item the
  Intent names. *Residual:* "Confirm before filling" keeps the values out of
  the autofill framework until a row is used; it does not restore per-fill
  authorization: the matched app can obtain its own matched logins while
  the vault is unlocked (`security-review.md` AN3).
* **Third-party autofill reading the master password.** The device's
  autofill service (Google Autofill, another password manager) is a
  different app. *Mitigation:* every HavenKeys window is excluded from
  autofill and no input carries a hint, so the master password and the
  Secret Key are neither offered to it nor saved by it (AN19).
* **Phishing through an app's label.** An app chooses its own name. The
  binding prompt ("Use GitHub in com.github.android?") names the package;
  the label is shown below it, marked as the app's own claim.
* **A stolen phone.** Locked, it holds only ciphertext, the Keystore-sealed
  Secret Key file and, if enabled, the Keystore-sealed unlock bundle; the
  Keystore keys require an unlocked device. Unlocked and with HavenKeys
  unlocked, the thief has what the user has. *Stated cost of biometric
  unlock:* someone who passes this phone's strong biometric check gets what
  the master password gives — the vault and a server session — on this phone
  only, for up to 14 days and until the next reboot. Adding a fingerprint or
  face invalidates the key. The master password is never stored.
* **A stale or tampered bundle.** Rust refuses a bundle older than 14 days,
  dated in the future, made in another boot, made or used with an unknown
  boot count, or not exactly the version-1 layout; the Keystore's GCM tag
  refuses modified ciphertext; a server that refuses the bundle's auth key
  (password changed, device revoked) locks the vault with
  `bundle_refused`. Each refusal deletes the bundle and asks for the
  password (`security-model.md` §22.2). *Residual:* offline, a bundle whose
  password was changed elsewhere still opens the local replica until it
  expires, as the old password does on the desktop (`security-review.md` #8).
* **A hostile `assetlinks.json`.** A site in the vault can name any app.
  *Mitigation:* fetched over HTTPS only, from plain DNS names only, no
  redirects, at most 128 KiB, at most 256 statements, parsed statement by
  statement in Rust. A site vouching for an app gets that app only the logins
  saved for that site — which the site's owner could phish anyway.
  *Residual:* a network attacker with a CA the phone trusts (user-installed
  CAs are trusted on purpose, §22.9) can forge the file; combined with a
  malicious app whose package name contains the site's name, that app is
  offered the site's logins (`security-review.md` AN5).
* **Clipboard readers.** A copied secret is marked sensitive and cleared
  after the vault's delay and on lock. Android 10+ keeps background apps
  from reading the clipboard, but the foreground app, the keyboard and a
  clipboard-reading accessibility service can. See `security-model.md` §22.10.
* **A malicious app or page submitting a fake form to plant or overwrite a
  login (M2).** An app or a web page can show a login form and submit it so
  that Android offers to save. *Mitigation:* nothing is saved unless the user
  confirms Android's save sheet and the vault is unlocked; a browser save can
  only add a login whose whole-site rule is the page's own host (reported by
  the privileged browser, not claimed by the page) and can only update a
  login that already matches that page; an app save can only add a login
  bound to that app's package and certificates, with no website, and can only
  update a login matched to that app. It cannot change another site's or
  app's login (`security-model.md` §22.17). *Residual:* an app can choose its
  label, so the new login's title can imitate another service (AN32).
* **Another autofill service reading the editor.** The edit window is
  excluded from autofill services and is `FLAG_SECURE`, so a second
  autofill service gets no structure from it and no screenshot is possible.
  *Residual:* a keyboard app sees what is typed and may ignore the
  no-learning flag (AN35).
* **A stale edit overwriting a newer one.** Phone writes go through the
  server's revision check; a concurrent change becomes "This item changed on
  another device." and nothing is overwritten (`security-model.md` §22.16).
  Editing is online only.
* **Overlays.** HavenKeys sets `FLAG_SECURE` (no screenshots, blank recents).
  On Android 9–11 an app allowed to draw over others could disguise the
  "Use" button of the binding prompt; the autofill activities and the
  binding prompt drop touches that pass through another app's window
  (`filterTouchesWhenObscured`, `security-review.md` AN6). The main activity
  does not filter them.

**Passkeys and Credential Manager (Android M3; `security-model.md` §22.18).**

* **A malicious app relays a site's challenge** to get a signature for the
  site. *Mitigation:* for an app Rust ignores the supplied `clientDataHash`
  and builds `clientDataJSON` with the origin
  `android:apk-key-hash:<its certificate>`; a relying party that checks the
  origin rejects it, and the app's hash is never signed.
* **An impostor app with the real package name.** *Mitigation:* the site's
  `assetlinks.json` must list the package and the certificate Android
  reports for the caller; another certificate gets no passkey.
* **A site that grants only `handle_all_urls`.** No passkey: only
  `common.get_login_creds` authorizes one.
* **A stale or failed asset-links lookup.** No passkey: the answer must be
  within the 7-day cache. Offline, an app's passkey stops working after 7
  days until the phone is online again (AN38).
* **A malicious browser not on the privileged list that reports an origin.**
  Its origin is dropped: an origin counts only from a caller whose package
  and certificate are on Rust's list, and Kotlin passes Android's `getOrigin`
  only that list. The caller then reaches Rust as an app and is denied unless
  the site grants that app `get_login_creds`; an origin that still reaches
  Rust from an unlisted caller is refused there.
* **A passkey request while the vault is locked.** Only "Unlock HavenKeys" is
  offered; no passkey or login names.
* **A conditional (automatic) create.** Refused; nothing is saved without a
  tap in HavenKeys.
* **Residual:** user verification is enforced by Kotlin only (AN37); the
  placeholder `clientDataJSON` for browsers (AN41); a compromised phone or
  accessibility service can click through the prompt.

### T12 addendum — cards and identity (Android M4)

* **An app showing a fake checkout.** It gets only the card the user taps,
  the same as a site in the extension; cards are not bound to a site or an
  app, so any app or https page can ask. *Residual:* the framework holds up
  to 5 cards' values for the fill session unless "Confirm before filling"
  is on (AN45).
* **An app firing gated rows itself.** It can fire the IntentSender and
  rewrite the item ID or mode. *Mitigation:* HavenKeys asks "Fill ... in
  <site or package>?" before Rust returns anything, and accepts a card ID
  only when Rust offers that card to this form (AN46).
* **A hostile iframe in a checkout.** Left out by Rust's frame rule (the
  tab's site or a fixed list of processor card frames). *Residual:* it
  depends on the browser reporting frames truthfully (AN47).
* **Documents on http.** Refused in Rust; a document number also needs the
  separate confirmation.
* **A login fallback swallowing a card field.** The login classifier
  refuses card fields everywhere, so a username guess or a split code box
  never takes one.

### T13 — Phone-approved sign-in

Spec: `docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md`.
A new desktop shows a QR code; the user's unlocked phone scans it, shows the
new device's name and where the server saw it, and, after Allow behind the
phone's biometrics or device credential, seals the vault key and Secret Key to
the desktop's one-time key. The master password is not typed for that first
sign-in.

**The trust decision.** Everywhere else the master password is the last
barrier: the server and the Secret Key together cannot open a vault. Here an
unlocked phone plus the user's Allow is enough to bring a new device into the
vault. That is accepted, as in 1Password's and Bitwarden's device approval,
because:

1. The phone must be unlocked and online, and Allow asks for the phone's
   biometrics or device credential again. A phone with no screen lock shows
   "biometric unavailable" and approves nothing.
2. The key material is sealed (HPKE) to a public key that travels only from
   the desktop's screen to the phone's camera; the server is never sent it.
   HPKE base mode does not authenticate the sender, so this is what the
   desktop relies on: only a device that saw the screen can seal to that
   key. The server relays only ciphertext; it can neither swap the
   recipient nor seal an envelope of its own to the desktop.
3. The confirmation names the device and where the server saw it; the code
   expires in 2 minutes and works once.

| Threat | Defence |
|---|---|
| Hostile or compromised server | Is never sent the desktop's public key, so it can neither open the envelope nor seal one of its own (its own account, or a vault key it chose with a header attested by it) to the desktop; such an envelope does not open, and one whose account or vault ids differ from the claim's is refused, so no vault is created. It can lie about the device name and location. |
| Another account on the same server sees the code (screen share) | It could approve first, with its own vault. **Residual, documented below.** |
| A code from an attacker ("scan this...") | Name, IP and location shown; Allow needs biometrics; 2-minute, single-use code. **Residual, documented below.** |
| Guessed or leaked `pairing_id` | Details and approve need a session on the account; claim needs `claim_secret`, which is not in the code. |
| Replay | The envelope is cleared on claim, and the session is issued only by that claim; `info` binds the envelope to the server and the pairing. |
| Abuse of the open endpoints | Per-IP limits, pending cap, size limits, generic errors. |
| Compromised desktop renderer | Keys stay in Rust; React gets the QR grid and states. |
| Accidental disclosure | Envelope, Secret Key, vault key and token zeroized after use, never logged or put in errors; `Debug` impls are redacted. |
| Locked or offline phone | Rust refuses before calling the server. |

* **Residual: social engineering.** A person who is talked into scanning an
  attacker's code and tapping Allow, past the name, IP and location check
  and a biometric prompt, gives the attacker's device the vault key, the
  Secret Key and a session. That device can read and change the vault
  without the master password, and keeps the keys (the master password is
  needed only for later unlocks of the local copy) until the user revokes
  the device in Devices; revoking stops its server access but cannot take
  back what it already read. Nothing in the protocol can tell a deceived
  user from a willing one. The confirmation shows text the unauthenticated
  requester chose (the device name) and a location label from the server;
  see `security-review.md` PA4.
* **Residual: the same exposure class as the phone.** Someone holding the
  user's unlocked phone and able to pass the biometric gate (or a phone
  without one) can sign in a device of their own. That is already the
  power they have over the phone's vault.
* **Residual: another account sees the code.** On a server with several
  accounts, someone signed in to another account who sees the desktop's QR
  code (a screen share, a shoulder) can scan it and approve first, sealing
  their own vault to it. The desktop then signs in to their account, not
  the user's; the user's vault is not exposed, but what the user saves there
  goes to the other account. Mitigation: after pairing, the desktop shows
  "Signed in as <email>" and how to remove the computer
  (`security-review.md` PA14).
* **A server operator** can create a pairing request of its own, but it
  is never sent the desktop's public key, so it cannot read the envelope
  the phone seals to that camera-delivered key or seal one of its own to
  the desktop, and cannot make a phone approve without the user's tap.

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
  it may be the only local copy if the account's server is gone and no
  backup was exported (Settings → Export) — so it remains on disk under the
  same T1 protection (master password and Secret Key) indefinitely, with no
  prompt to clean it up later.
* **A plaintext export file after it is written** (Bitwarden JSON, CSV):
  other applications, cloud sync folders and backups may read or copy it,
  and a crash while writing can leave a hidden `.part` file. See T7b.
* **Side channels** beyond what the underlying audited libraries protect
  against (RustCrypto `aes-gcm` uses constant-time implementations when AES-NI
  is available).
* **A weak master password.**
* **Clipboard managers / other applications reading the clipboard** during the
  clear window.
* **A rooted or otherwise compromised Android phone, and apps granted
  Accessibility.** An accessibility service can read every screen and type
  into it, HavenKeys' included; root can read process memory. HavenKeys does
  not detect either.
* **The Android autofill framework and the keyboard.** Both are part of the
  operating system the user chose; with direct fill the framework holds the
  matched logins' values for one fill session (`security-model.md` §22.4).

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
| A18 | Page probes `excludeCredentials` to learn, without a click, whether HavenKeys holds one of its accounts' passkeys | No answer until the user clicks **Close** on the "already saved" card | `apps/extension/src/background/webauthn-handler.test.ts` ("reports an excluded credential only after the user closes the card") |
| A19 | Page fires WebAuthn requests in a loop to drain the desktop's lookup rate limit | One lookup per tab at a time; the rest go to the browser | `apps/extension/src/background/webauthn-handler.test.ts` ("allows one wa_get/wa_create lookup in flight per tab") |
| A1u | Automatic passkey upgrade (`conditional create`) requested for a login other than the one just filled, for another site, before any fill, or after the 5-minute window | DENIED; falls back to the browser, nothing created | `crates/havenkeys-core/tests/passkeys.rs` (`conditional_create_needs_auto_for_exactly_that_login`), `crates/havenkeys-bridge/tests/bridge.rs` (`passkey_upgrade_through_the_bridge`) |
| A2u | Automatic upgrade on the same registrable domain as the fill (a subdomain) versus a look-alike domain | Subdomain: allowed if the login is offered there; look-alike (`github.com.evil.com`): `upgrade: none`, `authorize_rp` itself refuses | `crates/havenkeys-core/tests/passkeys.rs` (`upgrade_is_per_site_and_never_for_look_alikes`) |
| A3u | Page repeats the automatic upgrade after one silent save (fresh user handles), or aims it at an account whose passkey the vault already holds | DENIED; falls back to the browser, the existing passkey is untouched | `crates/havenkeys-core/tests/passkeys.rs` (`one_fill_grants_one_silent_passkey`, `conditional_create_never_replaces_the_filled_logins_own_passkey`) |
| A1s | `start_sso` for an item that does not match the page, is not a login, or has no `sign_in_with`, or a locked vault | DENIED/`locked`; no secret, no provider origins for a wrong page | `crates/havenkeys-core/tests/security.rs` (`start_sso_is_origin_bound_and_returns_no_secret`), `crates/havenkeys-bridge/tests/bridge.rs` (`start_sso_attacks_are_denied`) |
| A2s | A run tries to click a chooser row on an origin not in the provider's exact list, or in a frame/tab that is neither the run's own nor its opener popup | Refused; nothing clicked | `apps/extension/src/background/sso-state.test.ts`, `content/sso.test.ts` |
| A3s | `save_sso` with `itemId` targets a login that does not match the page or does not already sign in with that provider | DENIED | `crates/havenkeys-core/tests/security.rs` (`save_sso_adds_without_a_password_and_updates_only_the_account`) |
| A1a | Android: a page on evil.com in a privileged browser asks for github.com's login | Nothing matched; fill DENIED | `crates/havenkeys-mobile/tests/regressions.rs` (`attack_1_a_page_on_evil_com_gets_nothing_for_github`) |
| A2a | Android: an app with GitHub's package name and another certificate, or a WebView claiming `github.com`, asks for an item by ID | DENIED | `regressions.rs` (`attack_2_an_arbitrary_item_id_needs_a_matching_target`), `crates/havenkeys-core/src/app_fill.rs` (`the_same_package_signed_by_someone_else_gets_nothing`), `app_target.rs` (`an_app_claiming_a_web_domain_is_still_an_app`, `chrome_signed_by_someone_else_is_an_app`) |
| A3a | Android: vault locked, fill, TOTP or reveal requested | `locked` | `regressions.rs` (`attack_3_a_locked_vault_fills_nothing`), `app_fill.rs` (`a_locked_vault_fills_no_app`) |
| A5a | Android: malformed item IDs, oversized package names, certificate lists or domains | Refused, no panic | `regressions.rs` (`attack_5_malformed_ids_and_targets_are_refused_without_a_panic`), `app_target.rs` (`bad_identities_are_refused`, `a_domain_that_would_change_the_url_is_refused`) |
| A20 | Stale, other-boot, unknown-boot, future-dated, tampered or truncated unlock bundle | `bundle_refused` / unlock fails | `crates/havenkeys-core/src/unlock_bundle.rs` tests, `regressions.rs` (`a_stale_or_tampered_bundle_is_refused`), `crates/havenkeys-mobile/src/unlock.rs` |
| A21 | Hostile `assetlinks.json`: redirect, oversized, malformed, other package or certificate, IP or `localhost` host | No vouching; one bad statement voids only itself | `crates/havenkeys-core/src/asset_links.rs` tests, `crates/havenkeys-mobile/src/asset_links_fetch.rs` tests, `crates/havenkeys-core/tests/fuzz.rs` (`fuzz_asset_links_parser`) |
| A22 | The app being filled replaces the assist structure to name another app | Structure ignored, nothing filled | `apps/android/app/src/test/.../autofill/StructureNamesCallerTest.kt` (unit test of the check only; the activity flow is unverified on a device) |
| A1c | Android passkeys: an impostor app (the right package, another certificate) asks for the site's passkey | Nothing offered or signed | `crates/havenkeys-mobile/tests/regressions.rs` (`attack_an_impostor_app_gets_no_passkey`), `crates/havenkeys-core/tests/passkeys_android.rs` (`attack_an_app_the_site_does_not_vouch_for_gets_nothing`) |
| A2c | Android passkeys: an app vouched only for filling (`handle_all_urls`), or with a stale or failed lookup, asks for a passkey | Nothing offered or signed | `regressions.rs` (`attack_an_app_vouched_only_for_filling_gets_no_passkey`), `passkeys_android.rs` (`a_stale_failed_or_fill_only_lookup_vouches_for_nothing`) |
| A3c | Android passkeys: a caller that is not a privileged browser reports a website's origin | `denied` | `regressions.rs` (`attack_an_app_reporting_a_websites_origin_gets_nothing`) |
| A4c | Android passkeys: an app supplies a `clientDataHash` to be signed | Ignored; Rust signs its own `clientDataJSON` with the `apk-key-hash` origin | `regressions.rs` (`attack_an_apps_client_data_hash_is_not_signed`) |
| A5c | Android passkeys: vault locked, offers or sign-in requested | `locked`; nothing offered or signed | `regressions.rs` (`attack_a_locked_vault_gives_no_passkey`), `passkeys_android.rs` (`a_locked_vault_is_refused_first`) |
| A6c | Android passkeys: an app picks a login not matched to it, or asks for another RP's site | Refused | `passkeys_android.rs` (`a_chosen_login_must_be_matched_to_the_app`, `a_site_vouches_only_for_its_own_rp_id`) |
