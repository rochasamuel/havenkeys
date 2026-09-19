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
| Confidentiality at rest | AES-256-GCM under a random vault key, wrapped by an Argon2id-derived KEK |
| Integrity at rest (per blob) | GCM tag + AAD binding to vault ID / item ID / role |
| No plaintext secrets in SQLite | Items table holds only `id` + two encrypted blobs |
| Master password never persisted | Held in `Zeroizing<String>` only for the duration of `unlock`/`create` |
| Locked vault refuses secret access | Every secret-returning core function requires an active `Session`; locking drops it |
| Minimal renderer exposure | Command allowlist; secrets only via `reveal_secret` / `copy_secret` / `get_totp_code` |

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
* Argon2id parameters and salt (required to unlock).
* Item IDs (random UUIDv4) and the **number** of items, and the approximate
  size of each encrypted blob.

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
* increments the **session epoch**, which invalidates any authorization issued
  against the previous session (used by the extension channel),
* clears the clipboard if it still contains a value we placed there,
* emits `vault://locked` so the UI discards all item state.

Auto-lock triggers (see `crates/havenkeys-core/src/lock.rs`):

* inactivity timeout: Never / 5 / 15 / 30 / 60 minutes (default 15),
* system suspend/resume (wall clock advanced much further than the monotonic
  clock between two ticks),
* explicit lock (button, Ctrl+L, tray menu) and quitting from the tray.

**Tray behaviour.** Closing the window hides HavenKeys to the system tray; it
does **not** lock the vault. The vault then locks under the rules above, most
importantly the idle timeout, which keeps running while the window is hidden.
This matches mainstream password managers and is needed for the browser
extension later. With auto-lock set to "Never", a hidden window keeps the
vault unlocked until you lock or quit. The tray shows only static text,
never item data.

OS screen-lock signals (logind / Windows WTS / macOS distributed
notifications) are **not yet** wired up. On Windows the suspend heuristic is
likely ineffective, because `Instant` keeps counting during sleep. See
`security-review.md` #10.

Only real input while the window has focus, and commands the user triggers,
count as activity. Timer-driven calls such as TOTP refresh do not.

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

## 12. Browser extension permissions (planned)

Documented here once the extension ships (Phase 4–5). Target set:

| Permission | Why |
|---|---|
| `nativeMessaging` | talk to the desktop native host |
| `activeTab` / `scripting` | inject the autofill content script into the focused tab after a user gesture where possible |
| `storage` (session only) | non-secret UI preferences; never secrets |

`<all_urls>` host permissions will be avoided unless on-focus suggestions
prove impossible without them; any such decision will be justified here.

## 13. Known limitations

See `threat-model.md` §4 and `security-review.md`.
