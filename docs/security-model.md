# Security Model

This document describes *how* HavenKeys enforces the properties listed in
`threat-model.md`. Cryptographic details are in `crypto.md`.

> This software has not undergone an independent security audit.

## 1. Principles

1. The Rust core is the only trusted component. It owns keys, lock state,
   authorization and origin matching.
2. Everything else — the React renderer, the browser extension, web pages, and
   the vault file on disk — is treated as input to be validated.
3. Secrets are returned only for the operation that needs them, and only after
   explicit user action.
4. No telemetry, analytics, crash reporting or update checks, and no
   third-party network calls of any kind. The only outbound connections the
   desktop app makes are to the account server you configure: on unlock, every
   60 seconds while unlocked, and on every write (§13). Reads work offline
   from the local encrypted replica.

## 2. What is protected

| Property | Mechanism |
|---|---|
| Confidentiality at rest | AES-256-GCM under a random vault key, wrapped by a KEK derived from the master password (Argon2id) and the Secret Key (HKDF) |
| Copies away from your devices | The server's database and any backup need the master password **and** the 128-bit Secret Key |
| The server cannot read the vault | It stores item blobs and the header as opaque bytes; no key it holds opens any of them |
| The server cannot forge item content | Every blob authenticates under the vault's data key; one that does not open is skipped and counted, never applied. It can still **replay** a blob it was given earlier — see §3 |
| The server cannot replay an old header | The attestation must verify, and the revision may not fall below the highest the device has seen (`max_header_rev`) |
| Requests are bound to a session | The server derives the account and vault from the bearer token, never from the request body |
| Integrity at rest (per blob) | GCM tag + AAD binding to vault ID / item ID / role |
| No plaintext secrets in SQLite | Items table holds only `id`, the server revision, and two encrypted blobs (§4) |
| Master password never persisted | Held in `Zeroizing<String>` only for the duration of `unlock`/`create` |
| Locked vault refuses secret access | Every secret-returning core function requires an active `Session`; locking drops it |
| Minimal renderer exposure | Command allowlist; secrets only via `reveal_secret` / `reveal_previous_password` / `copy_secret` / `get_totp_code` |
| Recoverable password changes | A replaced password is kept, encrypted, in the item's password history (5 entries) |
| Passkey private keys never leave the core | Generated from the OS CSPRNG, sealed inside the login's encrypted details, and used to sign only in `havenkeys-core` (`passkey/`). No protocol message, Tauri command result, log or `Debug` output carries one (§15) |
| Master-password change is authenticated, not just session-authorized | `POST /v1/account/credentials` requires the *current* auth key, checked against the server's stored verifier under the same rate limiting as login; a stolen session token alone cannot change the password |
| Master-password change revokes other sessions | On success the server deletes every other session on the account; other devices must unlock again (locally with the old password, or online with the new one) before they can sync |

## 3. What is NOT protected

* A malicious process running as your user while the vault is unlocked.
* Rollback of the vault file to an older version, or deletion of rows.
* **Per-item rollback by the server.** A hostile or rolled-back server can
  re-serve an item blob it was given earlier at a higher revision. The blob is
  genuine, so it authenticates and is applied, silently returning a login to a
  previous password. Items have no revision floor; only the vault header does
  (`max_header_rev`). Password history (5 entries) is what limits the damage.
* Offline guessing of a weak master password.
* Anything already copied to the clipboard before it is cleared.
* Secrets in WebView memory after they have been displayed (JS strings cannot
  be wiped).

## 4. Plaintext metadata (intentional)

Stored unencrypted in SQLite:

* `format_version`, `vault_id` (random UUID), `created_at` of the vault.
* Argon2id parameters and salt (required to unlock), the key scheme
  (password only, or password + Secret Key) and the header revision.
* Item IDs (random UUIDv4) and the **number** of items, and the approximate
  size of each encrypted blob.
* Each item's server revision, which is a number, not a time.

There are no tombstones on a device: a deletion the server serves removes the
row, and the server keeps the tombstone (`server-sync.md` §3).

Outside the vault database, `device.json` (same folder, mode 0600) holds the
device ID. The Secret Key itself is kept in the OS keychain (Windows
Credential Manager, macOS Keychain, the Secret Service on Linux); it falls
back to **plain text in `device.json`** only when no keychain is available or
a call to it fails or times out, and Settings → Account shows this
(`secretKeyStorage: "file"`). `server-sync.md` §7 explains both paths and
their limits.

The server's own database holds, in plaintext: the account's email, its KDF
parameters and salt, an Argon2id hash of the auth key, device names and
timestamps, and per item its random UUID, its revision, the size of each
encrypted blob and when it last changed. That metadata is the price of this
design; it is listed in `server-sync.md` §6 and in `threat-model.md` T1b.

Everything else — item type, title, username, URLs, timestamps, passwords, TOTP
configuration, notes, passkeys, and settings — is encrypted. Passkeys add no
plaintext: the relying-party ID, account name, credential ID, user handle
and private key are inside the item's encrypted details, and the only new
overview field, `has_passkey`, is inside the encrypted overview.

## 5. Lock states

```text
            create/unlock              success
 LOCKED ───────────────────► UNLOCKING ─────────► UNLOCKED
   ▲                             │ failure            │
   │◄────────────────────────────┘                    │ lock / auto-lock /
   │                                                  │ suspend / exit
   └──────────────── LOCKING ◄────────────────────────┘
```

On lock the core:

* drops the `Session`, zeroizing the data key and the decrypted overview
  cache (titles, usernames, URLs),
* increments the **session epoch**, which invalidates in-flight unlock and
  re-key operations started against the previous session,
* pushes a `locked` event to every connected browser extension (every later
  extension request is refused with `locked` regardless),
* clears the clipboard if it still contains a value we placed there,
* emits `vault://locked` so the UI discards all item state.

Auto-lock triggers (see `crates/havenkeys-core/src/lock.rs`):

* inactivity timeout: Never / 5 / 15 / 30 / 60 minutes (default 15),
* system suspend/resume (wall clock advanced much further than the monotonic
  clock between two ticks),
* explicit lock (button, Ctrl+L, tray menu, the extension's *Lock* button)
  and quitting from the tray.

**Tray behaviour.** Closing the window hides HavenKeys to the system tray; it
does **not** lock the vault. The vault then locks under the rules above, most
importantly the idle timeout, which keeps running while the window is hidden.
This matches mainstream password managers and is needed for the browser
extension later. With auto-lock set to "Never", a hidden window keeps the
vault unlocked until you lock or quit. The tray shows only static text,
never item data.

**Open at login.** Off by default; Settings → Startup turns it on for this
computer. It is an OS entry (a login item on macOS, `HKCU\…\Run` on
Windows, `~/.config/autostart` on Linux) written through
`tauri-plugin-autostart`, and the OS is its only record: it is not stored in
the vault or synced. A login launch passes `--autostart` and starts **locked,
in the tray, with no window**; it never unlocks anything. Changing the
setting requires an unlocked vault. The plugin's own JS commands are not
granted to the renderer; only `launch_at_login` and `set_launch_at_login`
are. `tauri-plugin-single-instance` makes a second launch show the running
instance's window and exit, so two processes never open the same vault
file.

**Screen lock.** The vault also locks when the operating-system session
locks (reason `screen_lock`). The auto-lock thread polls every 5 seconds
through `crates/havenkeys-oslock`:

* **Windows:** the session lock flag from `WTSQuerySessionInformationW`.
  This is the only `unsafe` code in HavenKeys: one FFI call, in its own
  crate.
* **Linux:** logind's `LockedHint`, read with `loginctl`. GNOME, KDE and
  other logind-aware lockers set it.
* **macOS:** not implemented.

It fires once per lock, so a stuck lock flag cannot keep relocking the vault.
Where the state is unknown (no logind, as in WSL), nothing happens and the
probe switches itself off. Windows usually locks the session on sleep, which
also covers the suspend heuristic's blind spot there: `Instant` keeps
counting during sleep on Windows. See `security-review.md` #10.

Only real input while the window has focus, and commands the user triggers,
count as activity. Machine-driven calls do not: TOTP refresh, the periodic
pull, browser-extension requests, and the item-list refreshes that follow a
sync or an extension save (`list_items` and `get_item` deliberately do not
touch the timer, so a server that changes one item a minute cannot hold the
vault open). Typing in the search box still counts — it reaches the timer as
a keystroke through `record_activity`, not through the search command.

## 6. Desktop (Tauri) hardening

* **Command allowlist.** `build.rs` declares every app command; the only
  capability file grants exactly those plus `core:event` listen/unlisten and
  `core:window:allow-start-dragging`, which lets the window be dragged by
  its own title-bar areas now that macOS uses an overlay title bar. It moves
  the window and nothing else.
  No `fs`, `shell`, `http`, `dialog`, `opener`, or `clipboard` plugins are
  installed (clipboard is handled in Rust).
* **CSP** (production):
  `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; frame-ancestors 'none'`.
  The dev CSP additionally allows inline styles for Vite HMR only.
* `withGlobalTauri: false`, `freezePrototype: true`.
* Navigation to any non-app URL is blocked (`on_navigation` + `is_app_url`);
  the app never opens a second window, and the CSP's `frame-src 'none'`
  stops embedded content. There is no explicit `window.open` handler: the
  renderer has no reason to call it, but nothing in this repository denies
  it beyond the navigation guard and the CSP.
* No `eval`, `new Function`, `dangerouslySetInnerHTML`, or `innerHTML` in the UI.
* Devtools are disabled in release builds (Tauri default).

## 7. Renderer ↔ core interface

Every command the renderer can call — all 36 of them, which is the whole
surface. `build.rs` declares this list, the capability file grants exactly it,
and `src/lib/commands.test.ts` fails if the three ever disagree. "Online"
means a live server session, which a locked vault does not have.

| Command | Requires unlocked | Returns secrets |
|---|---|---|
| `vault_status` | no | no |
| `unlock_vault` | no | no |
| `lock_vault` | no | no |
| `change_master_password` | yes (online) | no |
| `record_activity` | no | no. The only intended way the idle timer is reset |
| `list_items` (optional search query) | yes | no (title, username, URLs, flags) |
| `get_item` | yes | no (secret fields reported only as *present/absent*) |
| `reveal_secret` | yes | one field: password, login notes, or note body. Never the TOTP secret |
| `password_history` | yes | no, timestamps only |
| `reveal_previous_password` | yes | one superseded password, on an explicit click |
| `list_passkeys` | yes | no: relying-party ID, account name, credential ID and creation time. Never the private key |
| `delete_passkey` | yes (online) | no. Removes one passkey from a login; passkeys cannot be created or edited from the desktop |
| `get_totp_code` | yes | current code only, never the seed |
| `copy_secret` | yes | no, the value is copied to the clipboard inside Rust |
| `create_item`, `update_item` | yes (online) | no. Edits send `keep`/`set`/`clear` per secret, so editing never requires reading the password or TOTP secret |
| `delete_item` | yes (online) | no |
| `generate_password` | no | a fresh password (not stored) |
| `copy_generated_password` | no | no |
| `get_settings`, `update_settings` | yes | no |
| `import_1pux` | yes (online) | no. Rust opens the native file picker; the renderer never supplies a path |
| `delete_import_file` | yes | no. Deletes only the file picked in the last import |
| `device_status` | no | no (key scheme, whether a Secret Key is needed, online) |
| `account_status` | no | no (email, server URL, account ID, last sync) |
| `activate_account` | no | no. Creates the vault from an invite |
| `sign_in` | no | no. Joins an existing account on a new computer |
| `sign_out` | no | no. Ends the server session and locks |
| `list_devices`, `revoke_device` | yes (online) | no |
| `remove_device` | yes | no. See §13 |
| `launch_at_login`, `set_launch_at_login` | no | no |
| `sync_now`, `resync_vault` | yes (online) | no |
| `get_emergency_kit` | yes | **the Secret Key**, plus a QR encoding it. The only command that returns long-term key material, on explicit request, with its own unlocked check because the Secret Key lives outside the vault (`server-sync.md` §7) |

All inputs are length-limited and validated in Rust; the UI's validation is
convenience only.

The browser extension does not use these commands. It reaches the core
through the native-messaging bridge, which has its own much narrower request
set (`status`, `lock`, `find_matches`, `fill_item`, `get_totp`,
`generate_password`, `check_login`, `save_login`, and the four passkey
requests), all origin-bound and rate-limited. See `native-messaging.md`.

## 8. Memory handling

* Keys (`master key`, `KEK`, `vault key`, `data key`) are
  `Zeroizing<[u8; 32]>` and are wiped on drop.
* Decrypted plaintext buffers are `Zeroizing<Vec<u8>>`; secret fields in item
  structs are a `SecretString` type that zeroizes on drop and redacts `Debug`.
* Passkey private keys are `SecretBytes` (`secret.rs`): a zeroize-on-drop
  byte buffer with a redacting `Debug` and no `Display`. A key is generated
  into a `Zeroizing` array, and during signing it exists as a `p256`
  `SigningKey`, which zeroizes its scalar on drop.
* **Limitations:** `serde_json` and the Tauri IPC layer allocate intermediate
  copies we cannot wipe (for passkeys, the base64url text of the key while
  the login's details are sealed or opened); the WebView keeps JavaScript strings until garbage
  collection; the OS may swap pages to disk (we do not `mlock`). Zeroization
  reduces, but does not eliminate, the window in which secrets are in memory.

## 9. Clipboard

* Copy is always an explicit user action and happens in Rust (`arboard`); the
  secret does not transit the renderer.
* After the configured timeout (default 30 s, range 10–300 s) the clipboard is
  cleared **only if** it still contains the copied value.
* Clipboard is cleared on lock under the same condition.
* Other applications (clipboard managers, malware) may read the clipboard
  during this window. On Linux/X11 any client can read the selection.

## 10. Importing from 1Password (`.1pux`)

* **No path from the renderer.** `import_1pux` opens the native file picker from
  Rust (`tauri-plugin-dialog`, used only on the Rust side; the capability
  grants the UI no dialog permission). A compromised renderer cannot make the
  core read an arbitrary file.
* **Hostile-input limits.** Archives are capped at 256 MiB and `export.data`
  at 64 MiB of actually decompressed bytes, which defeats zip bombs. Imports
  are capped at 50 000 items. Only `export.data` is read; attachments are not
  extracted.
* **Same validation as the UI.** Every imported item goes through the normal
  item validation. Items that fail are counted, not stored half-valid.
* **In memory only.** The file is read into a zeroize-on-drop buffer, and the
  parsed JSON tree's strings are wiped after conversion (best effort; see §8).
  No temporary files are written.
* **Atomic.** All items are written in one SQLite transaction.
* **Counts only.** The import report contains counts, never item content.
* **Plaintext export on disk.** The `.1pux` file stays where the user saved it.
  The UI warns about this and offers to delete it. That is a normal file
  deletion, **not a secure wipe**: on SSDs and journaling or copy-on-write
  filesystems the data may remain recoverable, and copies in backups or cloud
  sync folders are not affected.
* **Sent through the server like any other write.** Imported items are sealed
  locally and pushed in batches of at most 500, each atomic on the server.
  What the vault ends up with is what the server accepted, and that is the
  number the report shows.

## 11. Logging

Neither the core crate nor the Tauri shell logs anything. The lock reason
(`idle`, `suspend`, `user`, `exit`) is sent to the UI as an event. Errors are enum
variants rendered as fixed strings. A test fails the build if print/log
macros appear in the core crate.

## 12. Browser extension permissions

| Permission | Required? | Why |
|---|---|---|
| `nativeMessaging` | yes | Talk to the native messaging host, the extension's only route to the vault |
| `activeTab` | yes | When the user clicks the toolbar button: read that tab's URL to look up its logins, and allow filling it. Only that tab, until it navigates |
| `scripting` | yes | Inject the content script into that tab for a popup fill, and register the content script when in-page suggestions are on |
| `https://*/*`, `http://*/*` | **optional**, off by default | In-page suggestions and save prompts need a content script in the pages the user visits. Requested only from the options page, when the user turns suggestions on. The browser can narrow the grant to chosen sites, and the registered content script follows the grant |

Why not ask for everything at install: most of the value, filling with origin
binding, works with `activeTab`. Broad host access is what a malicious page
or a compromised extension build would most want, so the user decides.
Without it, the extension never runs in pages the user did not click it on.

Not requested: `<all_urls>` as a required permission, `tabs`, `storage`,
`clipboardWrite`, `cookies`, `webRequest`, `webNavigation`, `notifications`.
`externally_connectable` is empty, so web pages and other extensions cannot
message the extension.

**web_accessible_resources:** `menu.html`, `save.html`, `passkey.html`,
their scripts and styles, the theme, and the bundled fonts (`fonts.css` and three
Latin-subset `.woff2` files), for `https://*/*` and `http://*/*`. These are
the pages shown inside web pages and what they load. Nothing else can be
loaded or framed by a website. The fonts add no new signal: a site could
already tell the extension is installed by probing `menu.html`.

**CSP (extension pages):**
`default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'`.
`font-src 'self'` lets extension pages use the fonts bundled in the package;
no font is ever fetched from the network.
It has no `unsafe-eval` and no `unsafe-inline`. `frame-ancestors 'none'` was
removed in Phase 5, because the menu and save pages must be framed by web
pages. Framing by websites is limited to those pages by
`web_accessible_resources`.

**Passkey scripts:** when the user grants host access, the background also
registers two scripts with `chrome.scripting.registerContentScripts`, for
exactly the same granted patterns, in all frames, at `document_start`:
`webauthn-page.js` in the page's own world (`world: "MAIN"`), which wraps
`navigator.credentials`, and `webauthn-bridge.js` in the isolated world,
which relays it. They are registered and removed with the grant, as a group
separate from the inline content script, so a browser that rejects
`world: "MAIN"` loses passkeys but keeps in-page suggestions. No new
permission is needed: `scripting` and the optional host permissions already
cover registering scripts in granted pages, and neither script is a
web-accessible resource. See §15.

**Content script:** it runs in the isolated world, keeps no state beyond the
page, reads the DOM as untrusted input, and acts only on trusted user
events. It writes secrets only into the `value` of visible, enabled fields
of the group the user chose, and never into attributes. Details and
limitations are in `autofill.md`.

## 13. Secret Key, the account, and the server

See `server-sync.md` and `crypto.md` for the full design. In summary:

* Every vault is protected by the master password **and** a 128-bit Secret
  Key, mixed into the KEK with HKDF and bound to the account. Copies of the
  vault that are not on one of your devices — the server's database, its
  backups — cannot be opened with the password alone.
* **Activation** is the only way a vault is created: an invite from the
  server's operator, then a master password. Keys are derived and the header
  built in memory, the server is asked to accept them, and only then is
  anything written to disk, so a refused activation leaves no vault bound to
  an account no server knows.
* **A second device** needs the server address, the email, the master
  password and the Secret Key. It proves all four by opening the header the
  server serves; any one being wrong gives the same message.
* **The auth key** is a separate HKDF branch from the same Argon2id run as
  the KEK (`info = "havenkeys/v3/auth"`). It authenticates and unwraps
  nothing, which is what makes handing it to the server safe. The server
  stores only an Argon2id hash of it.
* **The session token** is 32 random bytes, valid 24 hours, held in memory
  only, and dropped when the vault locks — so a locked vault cannot reach the
  server at all. The server stores only its SHA-256.
* **The server is untrusted storage.** Item blobs and the header authenticate
  under keys it never sees; one that does not is skipped and counted, not
  applied. It is nevertheless the authority on which items exist, so it can
  destroy data (`threat-model.md` T1c) and tested backups are a prerequisite
  (`deployment.md` §5).
* **Writes need the server**: nothing is recorded locally until it has
  assigned a revision, so the replica is never ahead of the authority.
  Offline, every mutating command refuses with `Error::Offline` and the UI
  says so rather than letting a click fail.
* The sync client never holds the vault lock across a request, so locking is
  never delayed by a slow or hostile server.
* The Emergency Kit (Secret Key, account, email, server and a QR code) is
  shown only while unlocked, on request. Right after activation, continuing
  requires confirming it was saved.
* **The Secret Key lives in the OS keychain** (Windows Credential Manager,
  macOS Keychain, the Secret Service on Linux; service `app.havenkeys`, user
  = account ID), with `device.json` as the fallback when no keychain is
  available or a call to it fails or does not answer within 5 seconds.
  Moving it there narrows, but does not remove, the same trust boundary as
  the plaintext file it replaces: any process running as the user can
  usually read a Linux Secret Service or Windows Credential Manager entry
  too; macOS may prompt for access. It protects copies of the vault away
  from this device, not this device from something already running as you.
  A keychain that fails or does not answer is never read as "no key": unlock
  asks the user to approve the keychain's prompt and retry
  (`keychain_unavailable`) instead of asking for the Emergency Kit.
* **Changing the master password** (`change_master_password`) is one atomic
  server operation, not a local-first one: the device proves it knows the
  *current* auth key (rate-limited like login), and the server updates the
  KDF parameters, the login verifier and the vault header together, then
  deletes every other session on the account. The device making the change
  commits its local rewrap only after the server accepts it. Other devices
  are signed out and pick up the new password at their next unlock, through
  an online fallback that independently verifies the served header before
  adopting it (`server-sync.md` §6). A lost response is resolved by asking
  the server's current KDF parameters: the new ones mean the change was
  applied and it is committed locally; otherwise the user is told it did not
  happen, or that it could not be confirmed (`password_change_unknown`).
* **"Remove this device"** (Settings → Account, typed email required) locks
  the vault, renames the local vault file to
  `vault.sqlite3.removed-<timestamp>` rather than deleting it — it stays on
  disk as ciphertext, openable later only with the master password and the
  Secret Key from the Emergency Kit — and only then best-effort revokes the
  device on the server (revoking first would leave an intact vault on a
  device the server refuses if the rename failed). It forgets the Secret Key
  from both stores and issues a fresh device ID, returning the app to first
  run. A keychain delete that fails is retried once after a busy keychain
  frees up; if it still fails, the user is told to delete the
  `app.havenkeys` entry by hand. It requires an unlocked vault, so it is not reachable
  when the vault fails to open at startup.

## 14. Browser bridge

See `native-messaging.md` for the full protocol. In summary:

* The extension, the native host and the socket are all **untrusted input**.
  The bridge in the desktop process re-checks every request: protocol
  version and exact shape, size limits, rate limits, unlocked vault,
  integration switch, and core origin binding.
* The master password and the vault key never cross the bridge. There is no
  unlock request.
* Secrets cross it only in answer to `fill_item` (username and password of
  one item that matches the page), `get_totp` (the current code) and
  `generate_password` (a fresh random password). Toward the desktop, only in
  `check_login`/`save_login` (a password the user just submitted on the page).
* The only write is `save_login`: add a login for the page, or replace the
  password of a login matching the page. Replaced passwords go to the item's
  password history (5 entries), and at most one change per item every 10
  minutes is allowed from the browser.
* For iframes, items must match both the frame and the top-level page.
* The socket lives in a `0700` per-user directory that both sides verify.
  Peer UIDs are checked on Unix.
* Browser integration is opt-in (off by default) in Settings. The switch is
  stored in the encrypted settings blob and enforced in Rust.
* Passkeys add one more write, `passkey_create`, and three lookups or
  signatures (§15). Toward the browser they carry only public WebAuthn data
  (credential IDs, public keys, signatures, authenticator data).

## 15. Passkeys

HavenKeys is a WebAuthn authenticator for websites
(`docs/superpowers/specs/2026-09-23-passkeys-design.md`). Keys and formats
are in `crypto.md` §Passkeys, the messages in `native-messaging.md`, the UI
in `autofill.md` §Passkeys, and the threats in `threat-model.md` T8.

```text
 page JavaScript + webauthn-page.js     MAIN world: hostile. Wraps navigator.credentials,
        │                               holds nothing, decides nothing
        │  CustomEvent, JSON string only
        ▼
 webauthn-bridge.js                     ISOLATED world: parses exact shapes and limits
        │  runtime message (no URL in it)
        ▼
 background worker                      frame URL, top URL, origin, documentId from the
        │                               browser's sender data; one session per tab
        │  ◄── pick/save from passkey.html or the field menu (extension origin,
        │      click guard) — nothing is signed or created without it
        ▼
 native host ──► desktop bridge         strict protocol, sizes, rate limits,
        │                               unlocked + integration on
        ▼
 havenkeys-core (passkey/)              authorize_rp(rpId, url, topUrl); stored rpId must
                                        equal it; sign or create; key never leaves
```

* **The origin is the only thing the page cannot forge, and it alone decides
  which passkeys are reachable.** `authorize_rp` requires a secure context
  (https, or http on `localhost`), an rpId equal to the frame's host or a
  parent of it within the host's registrable domain (never a public suffix;
  an IP only when it is the host itself), and, for an iframe, a top page
  that is same-site with the frame and itself a secure context. A signature needs a passkey whose stored
  rpId equals the authorized one. The page's `rpId` is otherwise just input.
* **`clientDataJSON` is built in Rust** from the origin `authorize_rp`
  derived from the browser-reported URL, so the extension cannot choose the
  origin that is signed.
* **Explicit action.** A modal `get()` or a `create()` opens the passkey card
  (`passkey.html`, an extension-origin frame the page cannot read or script,
  with the same click guard as the menu). A conditional `get()` offers its
  passkeys in the field menu. Only a trusted, guarded click there makes the
  background ask the desktop to sign or create. The background accepts a
  pick only for a passkey (or a save target) the session offered, from a
  frame in the same tab, with the session's random token, one operation at a
  time.
* **Sessions** bind the token to the tab, frame, `documentId` (Chromium) and
  origin of the requesting frame. They end on a result, on cancel, on the
  site's `AbortSignal`, on the page's `pagehide`, after the site's timeout
  (clamped to 10 seconds – 5 minutes; 30 minutes for conditional requests),
  when the tab closes, and when the vault locks or the desktop goes away. A
  card opened while the vault was locked shows "locked"; each time it polls,
  the background looks the passkeys up again, so it updates on unlock even
  after the native port has closed. The bridge pings its session every 20 s,
  which keeps the Manifest V3 worker awake and detects a session lost to a
  worker restart: a conditional request is then asked for again, a modal
  one goes to the browser. The page script falls back to the browser when
  the bridge does not acknowledge a request within 1 s.
* **No presence answer without a click.** When the site's
  `excludeCredentials` names a passkey HavenKeys holds, the card says so and
  the site gets `InvalidStateError` only after the user clicks **Close**.
  Signals that remain are listed in `security-review.md` PK19.
* **Lookup budget.** One `get()`/`create()` lookup per tab at a time; another
  from the same tab meanwhile goes to the browser, so a page cannot drain the
  desktop's shared lookup rate limit.
* **Fallback.** Every failure the user did not choose — the desktop not
  running, locked for a conditional request, integration off, no matching
  passkey, an rpId the desktop does not allow for the page (the browser
  applies its own rule and raises `SecurityError` itself, or serves related
  origins and permitted cross-site frames), a request HavenKeys does not
  support, an internal error — hands
  the call to the browser's own `navigator.credentials`, so the site behaves
  as if HavenKeys were not installed. The user can also choose "Use another
  device".
* **Writes.** `passkey_create` is a server write like `save_login`: the
  credential goes back to the site only after the server accepted the
  sealed login. Offline, it is refused and nothing is stored. Registering
  the same account (rpId and user handle) again replaces its passkey
  wherever in the vault it is (WebAuthn does the same), ignoring the login
  the user picked.
* **What the extension sees:** titles, account names, credential IDs, and
  the public outputs of WebAuthn. Never a private key.

## 16. Known limitations

See `threat-model.md` §4 and `security-review.md`.
