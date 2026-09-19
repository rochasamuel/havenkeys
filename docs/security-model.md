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
4. No network. The desktop app makes no outbound connections, has no
   telemetry, analytics, crash reporting, or update checks.

## 2. What is protected

| Property | Mechanism |
|---|---|
| Confidentiality at rest | AES-256-GCM under a random vault key, wrapped by a KEK derived from the master password (Argon2id) and the Secret Key (HKDF) |
| Copies away from your devices | The sync folder and backups need the master password **and** the 128-bit Secret Key |
| Integrity at rest (per blob) | GCM tag + AAD binding to vault ID / item ID / role |
| No plaintext secrets in SQLite | Items table holds only `id` + two encrypted blobs |
| Master password never persisted | Held in `Zeroizing<String>` only for the duration of `unlock`/`create` |
| Locked vault refuses secret access | Every secret-returning core function requires an active `Session`; locking drops it |
| Minimal renderer exposure | Command allowlist; secrets only via `reveal_secret` / `reveal_previous_password` / `copy_secret` / `get_totp_code` |
| Recoverable password changes | A replaced password is kept, encrypted, in the item's password history (5 entries) |

## 3. What is NOT protected

* A malicious process running as your user while the vault is unlocked.
* Rollback of the vault file to an older version, or deletion of rows.
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
* Tombstones of deleted items: their random IDs and deletion times, so
  deletions can reach other devices.

Outside the vault database, `device.json` (same folder, mode 0600) holds the
device ID, the sync folder path and the **Secret Key in plain text**
(`sync.md` §6 explains why). The sync folder's plaintext is listed in
`sync.md` §6.

Everything else — item type, title, username, URLs, timestamps, passwords, TOTP
configuration, notes, and settings — is encrypted.

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
count as activity. Timer-driven calls such as TOTP refresh do not, and neither
do browser extension requests.

## 6. Desktop (Tauri) hardening

* **Command allowlist.** `build.rs` declares every app command; the only
  capability file grants exactly those plus `core:event` listen/unlisten.
  No `fs`, `shell`, `http`, `dialog`, `opener`, or `clipboard` plugins are
  installed (clipboard is handled in Rust).
* **CSP** (production):
  `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; frame-ancestors 'none'`.
  The dev CSP additionally allows inline styles for Vite HMR only.
* `withGlobalTauri: false`, `freezePrototype: true`.
* Navigation to any non-app URL is blocked; new windows are denied.
* No `eval`, `new Function`, `dangerouslySetInnerHTML`, or `innerHTML` in the UI.
* Devtools are disabled in release builds (Tauri default).

## 7. Renderer ↔ core interface

| Command | Requires unlocked | Returns secrets |
|---|---|---|
| `vault_status` | no | no |
| `create_vault`, `unlock_vault` | no | no |
| `lock_vault` | no | no |
| `change_master_password` | yes | no |
| `record_activity` | no | no |
| `list_items` (optional search query) | yes | no (title, username, URLs, flags) |
| `get_item` | yes | no (secret fields reported only as *present/absent*) |
| `reveal_secret` | yes | one field: password, login notes, or note body. Never the TOTP secret |
| `get_totp_code` | yes | current code only |
| `copy_secret` | yes | no, the value is copied to the clipboard inside Rust |
| `create_item`, `update_item` | yes | no. Edits send `keep`/`set`/`clear` per secret, so editing never requires reading the password or TOTP secret |
| `delete_item` | yes | no |
| `generate_password` | no | a fresh password (not stored) |
| `copy_generated_password` | no | no |
| `get_settings`, `update_settings` | yes | no |
| `import_1pux` | yes | no. Rust opens the native file picker; the renderer never supplies a path |
| `delete_import_file` | yes | no. Deletes only the file picked in the last import |

All inputs are length-limited and validated in Rust; the UI's validation is
convenience only.

The browser extension does not use these commands. It reaches the core
through the native-messaging bridge, which has its own much narrower request
set (`status`, `lock`, `find_matches`, `fill_item`, `get_totp`), all
origin-bound and rate-limited. See `native-messaging.md`.

## 8. Memory handling

* Keys (`master key`, `KEK`, `vault key`, `data key`) are
  `Zeroizing<[u8; 32]>` and are wiped on drop.
* Decrypted plaintext buffers are `Zeroizing<Vec<u8>>`; secret fields in item
  structs are a `SecretString` type that zeroizes on drop and redacts `Debug`.
* **Limitations:** `serde_json` and the Tauri IPC layer allocate intermediate
  copies we cannot wipe; the WebView keeps JavaScript strings until garbage
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

**web_accessible_resources:** `menu.html`, `save.html`, their scripts and
styles, the theme and one icon, for `https://*/*` and `http://*/*`. These are
the pages shown inside web pages. Nothing else can be loaded or framed by a
website.

**CSP (extension pages):**
`default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'`.
It has no `unsafe-eval` and no `unsafe-inline`. `frame-ancestors 'none'` was
removed in Phase 5, because the menu and save pages must be framed by web
pages. Framing by websites is limited to those pages by
`web_accessible_resources`.

**Content script:** it runs in the isolated world, keeps no state beyond the
page, reads the DOM as untrusted input, and acts only on trusted user
events. It writes secrets only into the `value` of visible, enabled fields
of the group the user chose, and never into attributes. Details and
limitations are in `autofill.md`.

## 13. Secret Key and sync

See `sync.md` and `crypto.md` for the full design. In summary:

* New vaults are protected by the master password **and** a 128-bit Secret
  Key, mixed into the KEK with HKDF. Copies of the vault that are not on one
  of your devices (the sync folder, backups) cannot be opened with the
  password alone.
* A new device joins with the master password and the Secret Key from the
  Emergency Kit. No server is involved; knowing both is the proof.
* The sync folder is untrusted storage. Every file is authenticated with
  the vault's data key: snapshots are bound to the vault and the writing
  device, items to their IDs, and the header by an attestation. Unauthentic
  files are ignored, older versions lose merges, and a password-only header
  is never adopted.
* The sync worker never holds the vault lock while it reads or writes the
  folder, so locking is never delayed.
* The Emergency Kit (Secret Key and QR code) is shown only while unlocked,
  on request. Right after a vault is created, continuing requires confirming
  it was saved.

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

## 15. Known limitations

See `threat-model.md` §4 and `security-review.md`.
