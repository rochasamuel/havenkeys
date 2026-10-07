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
4. No telemetry, analytics or crash reporting, and no third-party network
   calls beyond one bounded exception: checking for and downloading signed
   app updates from GitHub Releases (§18). The only other outbound
   connections the desktop app makes are to the account server you
   configure: on unlock, every 60 seconds while unlocked, and on every write
   (§13). Reads work offline from the local encrypted replica. The Android
   app adds one more, for sites already in the vault: their
   `assetlinks.json` (§22.7), behind a setting that is on by default.

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
A device's name is the computer's hostname or the phone's name with its
platform ("DESKTOP-SAMS (Windows)", "Sam's Pixel (Android)"), so the device
list tells the user's devices apart; it is sent again at every sign-in.

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
  installed (clipboard is handled in Rust). The `tauri-plugin-opener` crate is
  a dependency only for its free `open_url` function, called from
  `open_website`; the plugin is never registered, so the renderer has no
  opener commands.
* **Screen capture (TOTP QR scan).** `scan_totp_qr` reads the clipboard
  image and, only if that holds no TOTP code, captures every monitor
  (`xcap`). It runs only on the user's click in the unlocked app; pixels
  stay in memory for that one call and are never written, logged or sent.
  Only text that parses as `otpauth://totp` survives decoding. macOS asks
  for Screen Recording once; Wayland's portal asks every time.
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
* The renderer's only use of web storage is the UI language picked in
  Settings (`localStorage["hk-locale"]`: `en` or `pt-BR`, absent for
  Automatic). It is read before any vault is open and is not sensitive.

## 7. Renderer ↔ core interface

Every command the renderer can call — all 67 of them, which is the whole
surface. `build.rs` declares this list, `lib.rs` registers it, the capability
file grants exactly it, and `src/lib/commands.test.ts` fails if they (or the
commands `api.ts` calls) ever disagree. "Online"
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
| `sso_accounts` | yes | no (id, title, username of the provider's logins, at most 50) |
| `provider_login` | yes | no (one id, or none/several) |
| `password_history` | yes | no, timestamps only |
| `reveal_previous_password` | yes | one superseded password, on an explicit click |
| `list_passkeys` | yes | no: relying-party ID, account name, credential ID and creation time. Never the private key |
| `delete_passkey` | yes (online) | no. Removes one passkey from a login; passkeys cannot be created or edited from the desktop |
| `get_totp_code` | yes | current code only, never the seed |
| `copy_secret` | yes | no, the value is copied to the clipboard inside Rust |
| `open_website` | yes | no. Opens a website in the default browser only if it is one of that item's saved URLs and passes the http(s) check again; the renderer cannot open an arbitrary URL or scheme |
| `scan_totp_qr` | yes | no. Returns a token and issuer/account per code found; the `otpauth://` URI stays in Rust for 5 minutes, one scan at a time, cleared on lock. `create_item`/`update_item` accept `totp: { op: "scanned", value: token }` |
| `create_item`, `update_item` | yes (online) | no. Edits send `keep`/`set`/`clear` per secret, so editing never requires reading the password or TOTP secret |
| `delete_item` | yes (online) | no |
| `generate_password` | no | a fresh password (not stored) |
| `copy_generated_password` | no | no |
| `get_settings`, `update_settings` | yes | no. `update_settings` keeps the stored generator policy |
| `set_generator_options` | yes | no. Saves the generator tab's policy (encrypted settings); the extension's "Generate strong password" uses it |
| `import_file` | yes (online) | no. Takes only the source (a closed enum: 1Password, Bitwarden JSON/CSV, Chrome, Firefox, KeePassXC, LastPass); Rust opens the native file picker and the renderer never supplies a path |
| `restore_backup` | yes (online) | no. Takes only the backup password; Rust opens the native file picker and the renderer never supplies a path. Returns counts only |
| `export_summary` | yes | no. Counts only (logins, notes, cards, identity, passkeys) |
| `export_file` | yes | no. Takes the format (closed enum), the master password to re-check and, for a backup, the backup password; Rust opens the native save dialog and writes the file. Returns counts only |
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
| `set_ui_language` | no | no. Accepts only `en` or `pt-BR` and relabels the tray menu from a fixed table |
| `get_emergency_kit` | yes | **the Secret Key**, plus a QR encoding it. The only command that returns long-term key material, on explicit request, with its own unlocked check because the Secret Key lives outside the vault (`server-sync.md` §7) |
| `reveal_account_secret_key` | yes | **the Secret Key**, on an explicit reveal in the HavenKeys Account item. Same checks as `get_emergency_kit` |
| `identity_item_id` | yes | no. The Identity's derived item ID |
| `reveal_identity` | yes | **the identity's values** (and the formatted address), when its detail or editor opens. `Denied` for any item that is not an identity |
| `copy_identity_field` | yes | no. Copies one identity value, the address block or a custom field (`custom:<n>`, n < 50) from Rust, cleared after the clipboard delay; anything else fails to parse |
| `reveal_card` | yes | **a card's holder name, expiry, notes and chosen brand**, when its detail or editor opens; never the number or verification number (only whether they exist). `Denied` for any item that is not a card |
| `reveal_card_field` | yes | **the card number or verification number**, one at a time, on an explicit eye click; the field is a closed enum |
| `copy_card_field` | yes | no. Copies the holder name, number, verification number or expiry from Rust, cleared after the clipboard delay |
| `login_fields` | yes | **the plain values of a login's custom fields** (text, URL, email, phone, date, address parts) and flags: a Password field returns only `hasValue`, an OTP field only `hasOtp`. `NotFound` for an item that is not a login |
| `reveal_login_field` | yes | **one Password custom field's value**, on an explicit eye click, in the detail view and in the editor for a kept value. By field id; any other field type is refused |
| `login_field_totp` | yes | **the current code** of one OTP custom field, never its secret |
| `copy_login_field` | yes | no. Copies one field's value from Rust, read from the vault by field id, cleared after the clipboard delay |
| `open_login_field_url` | yes | no. Rust reads the URL field's saved value by field id, re-normalises it and opens it only if it is http(s); the renderer supplies no URL |
| `check_card_number` | no | no. Returns the brand the number's prefix names and whether its check digit passes; keeps nothing |
| `copy_account_field` | yes | no. Copies the account's email, server, account ID or Secret Key from Rust, cleared after the clipboard delay; the field is a closed enum |

All inputs are length-limited and validated in Rust; the UI's validation is
convenience only.

Custom fields (a login's titled sections of typed fields) are desktop-only and
live in the login's encrypted details blob, not in its overview. The renderer
receives plain values and flags through `login_fields`; a Password field's
value comes only through `reveal_login_field`, and an OTP field's secret never
leaves Rust (only codes, via `login_field_totp`). Copies and URL opens read the
value from the vault by field id, so the renderer never supplies the text or
URL. A save's field ids are checked against the same login: an id may appear
once and only if it belongs to that login, and keeping a value requires the
same type. Labels, titles and values are never put in error text or logs.

The browser extension does not use these commands. It reaches the core
through the native-messaging bridge, which has its own much narrower request
set (`status`, `lock`, `find_matches`, `fill_item`, `get_totp`, `open_item`,
`generate_password`, `check_login`, `save_login`, the four passkey
requests and the three identity requests, `find_identity`, `fill_identity`
and `open_identity`, and `show_unlock`), all rate-limited and all origin-bound except
`show_unlock`, which names no site or item and only raises the desktop
window. No request can name
a custom field, so none can read, fill or open one. See `native-messaging.md`.

`open_item` returns nothing: when the item is a login saved for the page,
the desktop shows its window with that login's editor open. It is in the
`secret` rate class because it has a visible effect.

`show_unlock` (the popup's "Unlock" button, and the Unlock icon in a locked
field menu) also returns nothing: the
desktop brings its window forward, on the unlock screen while locked. It is
answered in any lock state because it carries no secret either way, and it
is in the `secret` rate class for the same reason as `open_item`. The
master password is still typed only in the desktop app.

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
* **QR scan buffers.** The screen and clipboard pixel buffers decoded by
  `scan_totp_qr` are freed right after decoding but not zeroized, and
  further copies of them live in `xcap`, `arboard` and `rxing` internals and
  the OS beyond our control. The screen capture covers every monitor and
  every open window (hidden ones too, minimized and HavenKeys' own
  excepted), within one pixel budget. The clipboard image is fully decoded by
  `arboard` before the pixel budget applies to it. A crafted QR code that
  triggered a panic in the `rxing` decoder would end the app (release builds
  abort on panic) — an availability issue only, no secret would be exposed.
  Unit tests feed it noise and partial codes; no fuzz target exists yet for
  the QR decoder.

## 9. Clipboard

* Copy is always an explicit user action and happens in Rust (`arboard`); the
  secret does not transit the renderer.
* After the configured timeout (default 30 s, range 10–300 s) the clipboard is
  cleared **only if** it still contains the copied value.
* Clipboard is cleared on lock under the same condition.
* Other applications (clipboard managers, malware) may read the clipboard
  during this window. On Linux/X11 any client can read the selection.

## 10. Importing from other password managers

Sources: 1Password `.1pux`, Bitwarden unencrypted `.json` and `.csv`,
Chromium browsers' and Firefox's password CSVs, KeePassXC CSV and LastPass
CSV (spec 2026-10-05-import-formats).

* **No path from the renderer.** `import_file` opens the native file picker from
  Rust (`tauri-plugin-dialog`, used only on the Rust side; the capability
  grants the UI no dialog permission). A compromised renderer cannot make the
  core read an arbitrary file.
* **Hostile-input limits.** Archives are capped at 256 MiB and `export.data`
  at 64 MiB of actually decompressed bytes, which defeats zip bombs. Imports
  are capped at 50 000 items. Only `export.data` is read; attachments are not
  extracted. JSON and CSV exports are capped at 64 MiB.
* **The file must match the chosen source.** A CSV whose header lacks the
  source's required columns, or a JSON without Bitwarden's `items`, is refused
  as a whole; a single unreadable row (wrong number of fields, not UTF-8)
  counts as failed and the rest go on. Encrypted Bitwarden exports are
  refused: opening them would mean reimplementing Bitwarden's key scheme.
* **Never looser matching.** Bitwarden's per-URI match settings map to the
  same or a stricter HavenKeys rule (host → origin, exact → exact, all others
  → domain); every other source's websites match by domain, as typed ones do.
* **Passkeys are not imported.** Bitwarden's JSON carries passkey private
  keys; they are counted and dropped, never written into notes.
* **Same validation as the UI.** Every imported item goes through the normal
  item validation. Items that fail are counted, not stored half-valid.
* **In memory only.** The file is read into a zeroize-on-drop buffer, and the
  parsed JSON tree's strings are wiped after conversion (best effort; see §8).
  The CSV reader's internal row buffer is not wiped (the `csv` crate offers
  no hook); it is freed when the import ends.
  No temporary files are written.
* **Atomic.** All items are written in one SQLite transaction.
* **Counts only.** The import report contains counts, never item content.
* **Plaintext export on disk.** The export file stays where the user saved it.
  The UI warns about this and offers to delete it. That is a normal file
  deletion, **not a secure wipe**: on SSDs and journaling or copy-on-write
  filesystems the data may remain recoverable, and copies in backups or cloud
  sync folders are not affected.
* **Sent through the server like any other write.** Imported items are sealed
  locally and pushed in batches of at most 500, each atomic on the server.
  What the vault ends up with is what the server accepted, and that is the
  number the report shows.

## 10a. Export, backup and restore

Spec: `docs/superpowers/specs/2026-10-05-export-design.md`. Code:
`crates/havenkeys-core/src/export/` and `apps/desktop/src-tauri/src/export.rs`.

* **Formats.** An encrypted HavenKeys backup (`.hkbackup`, `crypto.md`),
  Bitwarden JSON and CSV. Plaintext exports **never contain passkey private
  keys**; only the encrypted backup carries passkeys.
* **The master password is re-checked on every export.** `export_file`
  verifies it in Rust (`PasswordCheck`, an Argon2id run outside the vault
  lock), so a stranger at an unlocked desktop gets no file. That walk-up
  case is what the re-check protects against. It is not a defence against a
  compromised renderer: while the vault is unlocked, such a renderer can
  already read items one by one through the existing reveal commands; the
  re-check only denies it the whole vault as one file. A backup also needs a backup password
  (the master password's length bounds, 10 to the maximum, and it must differ
  from the master password the caller typed). These rules are checked in Rust
  before the master password is verified; the comparison involves only two
  values the caller supplied, so it reveals nothing about the real master
  password.
* **The renderer never supplies a path.** Rust opens the native save and open
  dialogs; the capability grants the UI no dialog or filesystem permission.
* **Files are private and written atomically.** Written with mode 0600 to a
  temporary file `.<name>.<pid>.part` in the chosen folder, then renamed
  over the destination.
* **Restore** needs an unlocked, online vault. A backup is hostile input
  (`crypto.md` read order). Every item is rebuilt through the same
  validation as one typed by the user, and passkeys, password history and
  app bindings are re-checked (passkey count, credential ID length, user
  handle, normalised relying-party ID, P-256 key). Items are parsed one by
  one after the file decrypts, so an item this version cannot read counts as
  failed and the rest restores; a payload whose envelope cannot be read says
  so ("can't be read by this version") rather than "wrong password". Restore
  never overwrites an existing item; the identity is written under this
  vault's identity ID, and no other item may take that ID. Items are sealed
  and pushed like any other write, after a pull. The server keeps a
  tombstone for each deleted item, so an item deleted since the backup is
  refused as "new"; the client (`havenkeys-client/src/restore.rs`) then asks
  the server for that row and, only if it is still a tombstone, resends the
  item with the tombstone's revision as its base, which revives it. A
  conflict with a live item (written by another device after the pull) is
  counted as already present and left alone, and the rest of the batch is
  resent.
* **Export works offline** (it reads the local replica); restore needs the
  server because changes are written there.
* **Nothing is logged and the extension cannot export.** Errors are fixed
  strings; the summary and the restore report hold counts only; the browser
  bridge has no export command.
* **Memory.** Plaintext is rendered into a `Zeroizing<Vec<u8>>`, which wipes
  only the final buffer. Reallocations while serialising the JSON or CSV, and
  the `csv` writer's internal buffer, may leave copies in freed memory.
  Zeroisation of export plaintext is best effort (see §8).

Known limitations of export:

* **The plaintext file is the user's responsibility once written.** We warn
  and require an explicit "I understand"; we do not delete it, and we cannot
  stop other apps, cloud sync or backups from reading or copying it.
* **A crash between creating the temporary file and the rename** leaves a
  hidden 0600 `.part` file with plaintext in the chosen folder.
* **Password guessing through `export_file`.** A caller with the renderer
  can test master-password guesses at one Argon2id run per guess while the
  vault is unlocked. That is the cost of an unlock, and it is not otherwise
  rate-limited.
* **Locking while a backup is sealed.** The backup is rendered before the
  Argon2id step, so if the vault locks during it the file is still written.
* **A backup is only as strong as its backup password**, which is not
  combined with the Secret Key.

## 10b. Deleting the account

Settings → "Delete account and all data" (desktop and Android) needs the
vault unlocked and online, the account's email typed, and the master
password (Android also asks for biometrics or the screen lock). Rust checks
the password locally, then proves the auth key to the server, which erases
the account in one transaction (`docs/server-sync.md` §7a). Only after the
server's `204` does the device erase its vault file, the Secret Key and its
device id; the account's other devices do the same when they next sync or
sign in within 30 days and get `410` (by session token or by device id).

Not erased: database backups and server logs until they expire (≤ 30 days
promised), anonymous session-token hashes for 30 days, IP-keyed rate-limit
rows, vault files set aside by an earlier "Remove this device", backups the
user exported, and the copy on a device that stays offline for more than
30 days (it shows as signed out).

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
| `scripting` | yes | Inject the content script into that tab for a popup fill, and register the content script and passkey scripts for the granted hosts |
| `https://*/*`, `http://*/*` | yes (`host_permissions`) | Save/update prompts, passkeys and in-page suggestions need scripts in the pages the user visits. Granted at install. The user can withdraw it in the browser's site-access controls; the scripts are then unregistered, and the options page and popup offer **Allow**. Narrowing it to chosen sites currently turns the scripts off everywhere (fails closed; `autofill.md` §Permissions) |
| `storage` | yes | One boolean in `chrome.storage.local`: whether in-page suggestions (the field icon and menu) are shown. Nothing else is stored |

Why site access is asked for at install (changed 2026-09-28, spec
`2026-09-28-extension-defaults-design.md`): offering to save a new or
changed password and answering passkey requests both need a script in the
page, and asking users to find an options page first meant they silently
did not work. The cost is a larger default surface: the isolated content
script and the two passkey scripts now run in every http(s) page for every
user, not only after an opt-in, so a bug in them or a compromised extension
build reaches every page. They were already written to run everywhere once
granted and treat every page as hostile (`threat-model.md` §2, T8); nothing
in them changed. Nothing reaches the vault until the user turns on browser
integration in the desktop app, which stays opt-in (§14 and
`security-review.md` P6). The menu under login fields is a separate
preference (options page, on by default); turning it off does not stop save
prompts or passkeys.

Not requested: `<all_urls>` as a required permission, `tabs`,
`clipboardWrite`, `cookies`, `webRequest`, `webNavigation`, `notifications`.
`externally_connectable` is empty, so web pages and other extensions cannot
message the extension.

**Native host registration (desktop side):** the installers ship
`havenkeys-native-host` next to the app, and the app registers it with the
user's browsers at every start (`native-messaging.md` §2): per-user manifest
files, and on Windows per-user `HKCU` keys. This adds no new caller: the
manifests admit only our two extension IDs, the host checks the caller
again, and it remains a relay that cannot open the vault. It also adds no
new attacker: any program running as the user could already write the same
files and keys (`threat-model.md` §4). Browser integration still starts off,
and the bridge refuses page requests until the user turns it on.

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

**Passkey scripts:** for the granted hosts (every http/https host unless the
user narrowed site access), the background also registers two scripts with `chrome.scripting.registerContentScripts`, for
exactly the same granted patterns, in all frames, at `document_start`:
`webauthn-page.js` in the page's own world (`world: "MAIN"`), which wraps
`navigator.credentials`, and `webauthn-bridge.js` in the isolated world,
which relays it. They are registered and removed with the grant, as a group
separate from the inline content script, so a browser that rejects
`world: "MAIN"` loses passkeys but keeps in-page suggestions. No new
permission is needed: `scripting` and the host permissions already
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
* The **HavenKeys Account** item pinned first in the vault list is virtual:
  the desktop builds it from the account record and this device's Secret Key
  when shown, and it is never stored as a vault item. So the Secret Key never
  enters vault ciphertext or reaches the server, and the item is not in the
  item store the native-messaging bridge searches, so the extension cannot
  see it. It has no master-password field. The key is revealed only on a
  click and hidden again after 30 s, on lock, or on leaving the item
  (`specs/2026-09-29-account-item-design.md`).
* The **Identity** (name, documents, contact, address, custom fields, notes)
  is a stored, synced item like a login. Only the display name and the email
  go into its overview (decrypted at unlock, for the list and search);
  everything else is in its details blob and decrypted when the user opens
  it. There is exactly one per account: its item ID is derived from the vault
  key (`crypto.md`), each device creates it at connect if missing, the core
  refuses to create another or to delete it, and a create race ends in a
  409 and a pull. The extension cannot read it: `find_matches`,
  `fill_for_page` and `get_totp` refuse any item that is not a login
  (`tests/identity.rs`). Filling forms from it is a later, separate design.
* A **Card** (holder name, brand, number, verification number, expiry,
  notes) is a stored, synced item like a login; a vault holds any number.
  Its overview (decrypted at unlock, for the list and search) keeps only the
  brand, the **last four digits** (none when the number is under 12 digits)
  and the expiry; search matches those four digits, never the number. The
  number and verification number reach the UI only on an explicit reveal,
  one value per call, hidden again after 30 s and on lock; copies go from
  Rust to the clipboard. The brand is detected from the issuer prefix in
  Rust (`card.rs`); the Luhn check is shown as a warning, never enforced. The
  extension cannot read a card: `find_matches`, `fill_for_page` and
  `get_totp` refuse any item that is not a login (`tests/card.rs`). Checkouts
  are filled only through the card requests of §21
  (`specs/2026-09-29-card-autofill-design.md`).
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
  stored in the encrypted settings blob and enforced in Rust. The automatic
  passkey upgrade has its own switch in the same blob, `auto_passkey_upgrade`
  (on by default; §15).
* Cards add three requests, `find_cards`, `fill_card` and `save_card`
  (§21). A card leaves Rust only in answer to `fill_card`, only for the roles
  asked, and only to https frames that are same-site with the top page or on
  the processor list; `save_card` is the only write, and a card typed on a
  page reaches Rust only after the user's click on the save prompt.
* Passkeys add one more write, `passkey_create`, and four lookups or
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
* **Automatic upgrade.** A conditional `create()` (`mediation:
  "conditional"`) right after HavenKeys fills a password can save a passkey
  with no click at all, but the consent still lives entirely in Rust, in the
  unlocked session: `fill_for_page` records which login it filled, on which
  site, and when, in memory only (never persisted, dropped on lock); a
  conditional `check_passkey_create`/`passkey_create` for that same login and
  site within the next 5 minutes is `auto` when the vault setting
  `auto_passkey_upgrade` is on (`ask` when it is off), and `passkey_create`
  itself refuses (`denied`) unless it still comes out `auto` for exactly the
  item named. A silent save only adds a passkey (never replaces one) and
  spends the fill, so one fill grants at most one. This keeps the silent
  path narrow against extension bugs and hostile pages: a page cannot
  trigger a silent save without a recent HavenKeys fill of that login on
  that site. It is not a boundary against a compromised extension, which
  could already create passkeys for sites it names through the ordinary
  clicked `passkey_create` (`threat-model.md` T8, "Compromised extension",
  "Relaxed click rule" and "Fill memory"; `native-messaging.md` §7).
* **`passkey_status`.** A Lookup-class request answering only whether the
  vault holds any passkey `authorize_rp` allows for the page — nothing else
  about it. The field menu asks it once when it opens, to decide between a
  "you have a passkey" hint and a Passkeys Directory help link; the answer
  shapes only that menu and never reaches the page (`autofill.md`, "Passkey
  hints in the field menu").
* **What the extension sees:** titles, account names, credential IDs, and
  the public outputs of WebAuthn. Never a private key.

## 16. Automatic sign-in

See `autofill.md`, Automatic sign-in, for the full flow. In summary:

* **`autoSubmit` is decided in Rust, never by the extension.** It is
  `settings.auto_sign_in && item.auto_sign_in`, computed by
  `VaultService::auto_sign_in_for` and returned only after the existing
  origin check on `fill_item` / `get_totp`. Both switches default to on and
  read as on when absent from an older settings or item blob.
* **The run lives only in the background worker's memory**
  (`apps/extension/src/background/signin-run.ts`), never persisted, and
  holds no secrets: an item ID, an exact origin, a tab and frame ID, the
  current step and an expiry. It is bound to the exact origin (scheme, host,
  port) and frame of the pick that started it, moves forward only
  (`username → password → otp`), accepts each step once, and expires after
  2 minutes. A page cannot start or extend it — a `cs_run_step` message is
  accepted only from that same tab, frame and origin, only for the step that
  comes next — and every step's value is still fetched from Rust for the
  frame's current URL, so the origin check runs again each time.
* **Accepted risk.** For up to 2 minutes after a pick, a page on that same
  origin receives the password and the current TOTP code without further
  clicks — the same class of relaxation to rule #6 ("never autofill without
  explicit user interaction") as the automatic passkey upgrade. See
  `security-review.md` AS1 and AS2, and `threat-model.md` T9.

## 17. Sign in with a provider (Google, Microsoft, GitHub, Apple, Facebook, Discord, X, LinkedIn, GitLab)

See `autofill.md`, "Sign in with a provider (Google, Microsoft, GitHub, Apple, Facebook, Discord, X, LinkedIn, GitLab)", and
`native-messaging.md` §"Sign in with" for the full flow and wire messages.
In summary:

* **The cross-origin step is bounded to a fixed Rust list.** `start_sso`
  returns the provider's **exact origins** from a closed table in
  `havenkeys-core` (`sso.rs`) — never anything the page or the extension
  supplies. A run may act on those origins only, in the top frame of the
  run's tab or a popup it opened, within its `SSO_RUN_TTL_MS` (2 minutes).
  On the chooser: exactly one clickable element whose text holds the saved
  account, matched case-insensitively, or "Use another account" once it has
  been seen without that row on two consecutive attempts. Two matching
  account rows, or none, and nothing is clicked.
* **The login hand-off fills, and may press, the provider's own page —
  once, under several independent bounds.** After the user's pick, if the
  saved account is not already signed in at the provider, HavenKeys may
  fill, and with all three switches on press through, the provider's own
  login form there (`autofill.md` §"Sign in with", Login). This is bounded
  by: the run's Rust-listed origins; the top frame of the run's tab or its
  opener popup; the run's 2-minute expiry; **exactly one** provider login
  whose saved username equals the run's account (zero or more than one and
  nothing is filled); Rust's own origin check on `fill_item`, identical to
  every other fill; and the three switches — the site login's own
  `autoChoose`, the vault-wide `auto_sign_in` setting, and the provider
  login's own `auto_sign_in` switch. It is a one-time hand-off:
  `cs_sso_login` is honoured once per run, and the fill still runs through
  the same automatic-sign-in stop conditions (a visible CAPTCHA, the user's
  own input, an unrecognized step) as an ordinary same-site pick.
* **Which login is a provider's is decided in Rust.** `sso_accounts` (editor
  picker) and `provider_login` (detail link) use the same rule as a run and as
  `check_sso`: logins matched by `find_matches` to the provider's fixed
  origins, whose username equals the saved account. Both return ids, titles
  and usernames only.
* **Compatibility.** An app older than this change cannot decode a login whose
  provider is Facebook, Discord, X, LinkedIn or GitLab and reports it as
  unreadable until updated. The extension ships through the browser stores
  separately from the desktop app: an older extension rejects a
  `find_matches` response that contains a new provider, so its menu shows
  nothing on that site until the extension is updated.
* **The extension gains no secret it could not already request.**
  `fill_item` for a login saved for the provider's own origin, asked from
  that origin, was always allowed; what changes is that no click on the
  provider's page needs to precede it. Nothing is sent to the site that
  started the run: it can at most cause a run that ends on the real
  provider page, filling the user's own provider login, after the user
  picked its "Sign in with" row there — it cannot see the provider page.
* **Consent is never granted.** A keyword-based check
  (`isConsentScreen`/`isConsentLabel`) recognizes permission and consent
  wording ("Continue as …", "Allow…", "Grant access/permission(s)…",
  "Authorize…") and refuses to press or click anything on such a screen. This
  is a heuristic over untrusted page text, not a cryptographic guarantee: see
  `security-review.md` for its residual risk.
* **Page-sourced account text is untrusted.** The text the extension reads
  from a provider's chooser or a username step is only ever a suggestion. It
  reaches Rust as a plain field on `check_sso`/`save_sso` and is validated
  there like any other input: trimmed, at most 254 characters, no control
  characters. It never decides which origins a run may act on. The same
  holds for a `login_hint` read from a URL the clicked tab or its popup
  loads: the site chose it.
* **`check_sso` returns account names, never secrets.** When a save prompt
  will be shown (`add`/`update`), Rust adds the usernames of the vault's
  logins for that provider's sign-in page, at most 10, so the prompt can
  offer them as choices. This happens only after a trusted click on a
  provider button, while the vault is unlocked and browser integration is
  on. The extension already gets the same names from `find_matches` on the
  provider's page. They are held in background memory for the prompt's
  life (5 minutes at most) and shown only in the extension-origin balloon,
  which the page cannot read.
* **`start_sso` is origin-bound exactly like `fill_item`.** The item must be
  a login with `sign_in_with` whose own website rules match the page; an
  unrelated item, a wrong origin, or a locked vault all fail the same way as
  every other secret-returning request.
* **No new browser permission.** Recognizing provider buttons and reading a
  chooser's account text both happen inside the same content script that
  already runs on granted pages; nothing new is requested (`autofill.md`
  §Permissions, §12 above).

## 18. In-app updates

See `docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md` for the
full design; `threat-model.md` lists the threats, `development.md` the
release and key-rotation steps.

* **The only new destinations** are `github.com` and the host GitHub's
  download redirect points at, both over HTTPS. Nothing else the app talks to
  changes. A request reveals the caller's IP address, the time, and the fact
  that a HavenKeys installation is checking for or downloading an update — no
  account, device ID, email or vault data. The automatic check can be turned
  off in Settings → Updates; a manual check and download then happen only on
  a click.
* **Integrity comes from the minisign signature, not from TLS.** Every
  release asset is signed with a key generated once by the project owner; the
  public half (key ID `1D3AD839EC826779`) is committed in `tauri.conf.json`.
  `tauri-plugin-updater`'s `download` verifies the signature before it
  returns any bytes — a compromised GitHub account, repository, CDN or
  network path can serve whatever it likes, but cannot make an install accept
  it without the private key.
* **No downgrade.** `latest.json` is fetched over TLS but is not itself
  signed; only the artifacts are. On its own, the comparator (install only a
  version greater than the one running) would trust the manifest's `version`
  field, so a crafted manifest could pair "99.0.0" with an older release's
  URL and still-valid signature. The updater config therefore sets
  `requireSignedVersion: true`: the signature's trusted comment (covered by
  the signature) records the app version it was signed for, and `download`
  rejects an artifact whose signed version differs from the announced one or
  that carries no version at all. With both checks, a captured old release
  cannot be replayed to push a device backwards. Releases must be built with
  `@tauri-apps/cli` 2.12.0 or later, which writes `version:` into every
  `.sig`; `development.md` has the pre-publish check.
* **Order:** download → verify the signature (inside `download`, before any
  bytes are returned) → lock the vault (`state.lock(app, "update")`, the same
  path as quitting) → install → restart. A bad or missing signature aborts
  before the vault is touched, so a failed or rejected update never leaves
  keys in a half-exited process.
* **The renderer stays unprivileged.** No `tauri-plugin-updater` permission is
  granted to the webview; the capability file grants only the four named
  commands (`update_status`, `check_for_update`, `install_update`,
  `set_update_auto_check`). The webview never calls the plugin directly.
* **Release notes are plain text**, truncated to 4,000 characters, and shown
  to the user as a text node — never HTML, never interpolated into markup.
* **The "Download" URL is a constant** in Rust
  (`https://github.com/rochasamuel/havenkeys/releases/latest`), used only when
  the running install cannot update itself in place (`.deb`/`.rpm`). It is
  never taken from the downloaded `latest.json`, so a compromised manifest
  cannot redirect that click.
* **The setting** (`autoCheck`, default on) lives in `updates.json` next to
  `device.json`, outside the encrypted vault. It holds nothing secret, and it
  has to be readable while the vault is locked so the app can decide whether
  to check for updates before anyone unlocks anything.
* **Debug builds never check automatically** (`cfg!(debug_assertions)`
  short-circuits the scheduler), so a development run never reaches GitHub on
  its own.

## 19. Known limitations

See `threat-model.md` §4 and `security-review.md`.

- **Custom fields and older releases.** A device running a HavenKeys release
  from before login custom fields ignores a login's custom fields and drops
  them if it rewrites that login (any save, including a browser password
  update). Update every device before using custom fields.

## 20. Filling forms from the Identity

Spec: `docs/superpowers/specs/2026-09-29-identity-autofill-design.md`.

Every other fill is **bound to a site**: Rust releases a login only to a page
its saved websites match. The Identity has no website. Any page, a phishing
page included, can show a form that asks for a name and an address, and
HavenKeys will offer to fill it there. That is accepted, because an identity
is for filling forms on sites the user has not seen before. The protection
comes from these rules instead:

1. Nothing is filled without the user's click in the extension's own menu (an
   iframe the page cannot read or click) or popup.
2. Only visible, editable fields of the form the user clicked in are filled,
   re-checked at the moment of writing; hidden fields never are. "Visible"
   is stricter than for logins (`autofill/visibility.ts`): besides being
   rendered with a size, the field must lie inside the page's scrollable
   area (not `left: -9999px`), its combined opacity with its ancestors must
   be at least 0.1, no `clip` or `clip-path` on it or an ancestor may
   collapse it, and no `overflow: hidden`/`clip` ancestor may be collapsed
   or cut it off. At most 16 elements are examined per field.
3. Only the values those fields ask for leave Rust (`roles`).
4. **Documents** (CPF, RG, passport, driver's license) need a second,
   explicit click that names them, and only on https pages. Rust enforces
   both: it leaves documents out unless `documents` is true and the page is
   https.
5. A cross-site iframe gets nothing (`denied`, checked in Rust).
6. Existing values are never overwritten, except values HavenKeys itself
   wrote.
7. Identity fills never submit a form.

Requests: `find_identity` (lookup class; title, email and role names, never a
value), `fill_identity` and `open_identity` (secret class). See
`native-messaging.md`. The extension classifies fields and picks roles, but
that is convenience, not the boundary: Rust still answers only known roles,
only for an http(s) page, and documents only as above. Card fields are
refused by the identity classifier; cards are filled only through the card
flow below (§21), and no card data exists in the identity.

**What this does not protect against.** A user who clicks "Fill" on a
phishing page gives that page the non-document values the form shows fields
for, and the documents too if they confirm. The confirmation names the
documents and the page's site to make that choice deliberate, but it cannot
tell a real shop from a copy. A page can read what was written into its own
fields, as with any form. On an http page a network attacker can read or
alter the page, including injecting script that reads the fields and sends
them the moment they are filled, so values filled there should be treated
as exposed; documents are never filled there.

**Known limitations of the visibility check.** A field **covered by another
element** (a page drawing an opaque box over it) still counts as visible:
hit testing (`elementFromPoint`) would refuse legitimate forms whose
floating labels sit on top of their inputs, so it is not used. Colour tricks
(text and background the same colour) and fields scrolled far into a long
page are not detected either. The field must still belong to the form the
user clicked in, and nothing is filled without that click.

**Role probing by a compromised content script.** `cs_open_menu` carries the
roles the content script says the form has, and whether a menu opens
(`ok`) tells whether the identity has a value for at least one of them. A
compromised content script (which needs an extension compromise, not just a
hostile page) could send one role at a time and learn which roles the
identity has a value for, never the values themselves. That is low value, and such a script could already fill
and read the form after a click.

**Firefox data-collection declaration.** `manifest/firefox.json` declares
`authenticationInfo` (logins, passkeys, one-time codes),
`personallyIdentifyingInfo` (the identity's name, contact, address and
documents) and `financialAndPaymentInfo` (saved cards filled into
checkouts, and a typed card offered for saving) as required data collection
permissions. Nothing is collected by a third party: the declaration states
that the extension handles this data locally, to fill forms on the user's
request.

## 21. Filling checkouts from a Card

Spec: `docs/superpowers/specs/2026-09-29-card-autofill-design.md`.

Like the Identity, a Card is **not bound to a site**: it is offered on any
https checkout, because a card is for paying sites the user has not saved.
A phishing checkout can show a card form and HavenKeys will offer to fill
it. The protection comes from these rules instead:

1. Nothing is filled without the user's click in the extension's own menu (an
   iframe the page cannot read or click) or popup. The popup's Fill card is
   bound to the origin of the list the user saw: if the tab moved to another
   origin, Rust is never asked.
2. Only visible, editable card fields of the checkout the user clicked in
   are filled, re-checked at the moment of writing, with the same strict
   visibility test as the identity; hidden fields never are.
3. The top page is **https**. Each frame filled is https **and** same-site
   as the top page or has an origin on the processor list below. Rust
   checks this for every frame of a `fill_card`, and one bad frame denies
   the whole request.
4. Only the values the form's fields ask for leave Rust (`roles`, at most 8
   per frame, 8 frames).
5. Existing values are never overwritten, unless HavenKeys wrote them.
6. A card fill never submits a form.
7. Nothing is saved without the user's click on the save prompt.

Requests: `find_cards` (lookup class; title, brand, last four digits and
expiry, never the number or CVV), `fill_card` and `save_card` (secret class).
See `native-messaging.md`. The extension classifies fields and picks roles,
but that is convenience, not the boundary: Rust answers only known roles for
https frames as above, and `find_matches`, `fill_item` and `get_totp` still
refuse a Card.

**Payment processors.** A card frame served from another site (Stripe
Elements, Adyen, Braintree, Mercado Pago) is the normal way checkouts are
built, and a cross-site frame otherwise gets nothing. `PAYMENT_FRAME_ORIGINS`
in Rust lists the exact https origins each processor documents for its card
fields: Stripe `js.stripe.com` (and subdomains, matched on a whole label),
the Adyen live checkout-shopper regions, Braintree's
`assets.braintreegateway.com`, and Mercado Pago's `secure-fields` and
`api-static` origins. No test or sandbox origins; a new entry needs a code
change and a test. Pagar.me, PagSeguro and Cielo are left out because their
documented integrations keep the card inputs on the merchant's own page,
which the same-site rule already covers.

**Which frames get a card.** Rust compares each frame only with the top
page, so the extension also refuses nesting. It has no `webNavigation`
permission (minimal permissions), so eligibility comes from a report each
candidate frame's content script sends (`content/ancestry.ts`), taken by the
background as data to check, never as authority:

* Chromium: `location.ancestorOrigins`. Every ancestor must be the top
  page's origin or accepted by Rust as a processor or same-site frame.
* Firefox (no `ancestorOrigins`): a processor frame only if it is a direct
  child of the top page.
* A missing or malformed report means a subframe gets nothing.

So a card frame nested under a cross-site frame that is not a processor
(an ad iframe embedding Stripe) is refused. Before `fill_card`, the
background vets each extra frame with a `find_cards` lookup (at most 12 per
pick) and drops the ones Rust would deny, so an unrelated frame with
card-like inputs does not deny the whole fill. A frame whose document
changed since the roles were collected drops its values (Chromium; see the
limitation below).

**Menu in a small iframe.** A card menu opened inside a processor's 40-px
iframe would be clipped, so the top frame draws it over that iframe. The
iframe is found with `runtime.getFrameId` where the browser has it, else by
its `src` (the frame's full URL from the browser's sender data, credentials
removed, then without query or fragment) and, among several, by the size the
frame reports; if that does not single one out, the menu is drawn inside the
frame. The full URL goes only to the tab's top frame, never to the desktop;
a wrong match only misplaces the menu. The
choice still comes from a trusted click in the menu's own frame.

**Save prompt.** After a form submit in the **top frame**, a card the user
typed (the digits typed equal the field's digits; a number HavenKeys filled
or a page script wrote never counts) with a valid check digit and an expiry,
which no saved card matches by last four digits and expiry, opens "Save card
to HavenKeys?". The number is not sent to Rust before **Save**. Until then it
is held in the background's memory only, for 2 minutes, and dropped on
dismiss, lock, tab close and expiry (it survives navigation, because
checkouts navigate on submit). Cards typed into processor iframes are not
offered for saving.

**What this does not protect against.** A user who picks a card on a phishing
checkout gives that page the card, CVV included; the menu header names the
page's site to make the pick deliberate, but it cannot tell a real shop from
a copy. A page can read what was written into its own fields. On an http page
nothing is filled (Rust denies it and the menu says why).

**Known limitations.**

* Firefox frames have no `documentId`, so a fill there is pinned to the
  frame only by the origin check: a frame that navigates to another origin
  between the role scan and the write is stopped by the origin check, not by
  a document identity.
* The visibility check has the limits described in §20 (a covered field
  still counts as visible).
* Firefox data-collection declaration: see §20 (`financialAndPaymentInfo`).

## 22. Android app

Spec: `docs/superpowers/specs/2026-10-01-android-app-design.md`. Building
and running: `docs/android.md`. Threats: `threat-model.md` T12.

**Nothing in `apps/android` has run on a phone or an emulator yet.** What
this section says about Android's behaviour (the Keystore, BiometricPrompt,
the autofill framework, package visibility, `FLAG_SECURE`, the clipboard)
describes the code and Android's documentation, not a test on a device. The
manual checklists are in `security-review.md` ("Android M1" and, for editing
and saving, "Android M2" and, for passkeys, "Android M3": not yet run).

### 22.1 Boundaries

* `crates/havenkeys-mobile` is the only Rust surface Kotlin can call
  (UniFFI). It never returns keys, blobs, whole vault objects or a TOTP
  secret, and every call that returns a secret checks the lock state, the
  item and the fill target in Rust.
* Kotlin collects facts from Android — the screen's structure, the caller's
  package name and signing certificates, the boot count — and talks to the
  Keystore and BiometricPrompt. It never decides whether an item may be
  filled.
* There is one `MobileVault` per process. The app's screens and the
  `AutofillService` share it, so one unlock opens both and one lock closes
  both. The vault exists only in memory: the process dying locks it.

### 22.2 Unlock and the biometric unlock bundle

The master password unlocks as on the desktop (key scheme 3, `crypto.md`).
It is needed after sign-in, after a reboot, every 14 days, and whenever the
unlock bundle is refused.

Biometric unlock is opt-in (Settings), with the master password entered once.
Rust derives the keys from it, checks it by unwrapping the vault key, and
returns the **unlock bundle**, 81 bytes
(`crates/havenkeys-core/src/unlock_bundle.rs`):

```text
version (1) ‖ vault key (32) ‖ auth key (32) ‖ enrolled_at_ms (i64 BE) ‖ boot_count (i64 BE)
```

Kotlin seals it with a fresh Keystore key and zeroes its copy
(`security/BiometricKeys.kt`). The file is `noBackupFilesDir/unlock-bundle.bin`:
the GCM IV (12 bytes) followed by the ciphertext and tag. The key:

* AES-256-GCM, alias `havenkeys.unlock`, StrongBox when the phone has it,
  the TEE otherwise;
* `setUserAuthenticationRequired(true)`, `BIOMETRIC_STRONG` only, with no
  validity window: every use needs a BiometricPrompt bound to that cipher
  (`CryptoObject`). On Android 9–10, `setUserAuthenticationValidityDurationSeconds(-1)`
  gives the same per-use rule. The prompt does not offer the device PIN;
  its other button is "Use master password";
* `setInvalidatedByBiometricEnrollment(true)`: adding a fingerprint or face
  destroys it;
* `setUnlockedDeviceRequired(true)`: unusable while the phone is locked.

**Unlocking:** BiometricPrompt → the Keystore decrypts → the bytes go
straight to `unlock_with_bundle` → Kotlin zeroes its array, however the
call ends. Rust then refuses the bundle, and the unlock screen deletes it and
asks for the password, when:

* it is older than 14 days, or dated in the future (a clock set back must
  not stretch the window);
* the boot count differs (`Settings.Global.BOOT_COUNT`): a reboot ends it;
* the boot count is unknown (negative), at enrolment or at use. A phone that
  does not report `BOOT_COUNT` cannot turn biometric unlock on at all;
* it is not exactly the version-1 layout, or its vault key does not open the
  vault;
* the server refuses its auth key (the master password changed on another
  device, or this device was revoked). That arrives after the vault opened,
  as a lock with reason `bundle_refused`; the app deletes the bundle and its
  key on that event.

A Keystore key invalidated by a new enrolment, or a file that does not open,
is handled the same way: biometric unlock ends and the password is asked for.
The bundle is deleted when it is refused, not proactively at day 14.

**Why the auth key is in the bundle:** a biometric unlock must be able to
sign in to the server and write. This is the one place the auth key is
persisted, Keystore-sealed (`crypto.md`, "Unlock bundle").

**Stated cost:** someone who passes this phone's strong biometric check gets
what the master password gives — the vault and a server session — on this
phone only. The master password is never stored.

### 22.3 The Secret Key file

On Android the Secret Key lives in `filesDir/secret-key-<account>.bin`,
written by Rust (`crates/havenkeys-mobile/src/key_file.rs`) and sealed by a
second Keystore key (`security/SecretKeyCipher.kt`: AES-256-GCM, alias
`havenkeys.secret_key`, `setUnlockedDeviceRequired(true)`, no user
authentication — the Secret Key alone opens nothing). Kotlin only encrypts
and decrypts bytes. There is **no plaintext fallback**: the desktop's
`device.json` fallback is turned off (`without_file_fallback`), and a cipher
that fails writes nothing (tested). Sign out keeps the file, as the desktop
keeps its keychain entry, so the next unlock needs no Emergency Kit; Remove
device forgets it and deletes its Keystore key.

### 22.4 Direct fill and "Confirm before filling"

This is one of the two documented relaxations of CLAUDE.md §33: this one
for logins, the other for cards and the identity (§22.4a, CLAUDE.md
amendment of 2026-10-02).

* **Direct fill** (default; "Confirm before filling" off) while unlocked:
  the fill response carries one dataset per login Rust matched to the
  requesting app or page — at most 5 — each with its username and password
  (or, for a code-only step, its current TOTP code). The Android autofill
  framework, part of the operating system, holds those values for that fill
  session; the requesting app receives only the dataset the user taps.
* **"Confirm before filling" on**, or the vault locked: each row is
  authentication-gated and carries no value. Tapping it starts HavenKeys'
  `AutofillAuthActivity`, which unlocks first if needed (password or
  biometrics) and then calls `autofill_fill(id, target)` or
  `autofill_totp(id, target)`; Rust re-checks the target and returns only
  that login's username and password, or its code. While the vault is
  unlocked the activity shows nothing: "confirm" means HavenKeys checks this
  fill, not that the user is asked again.
* The app being filled holds each gated row's IntentSender and can fire it
  without a tap; the Intent is mutable and `Intent.fillIn` merges extras, so
  it can also swap the item ID and mode the row carries. Rust still returns
  only items matched to that same caller (§22.5), whatever the Intent names,
  so the most it gets is what direct fill would have offered it. "Confirm
  before filling" therefore keeps values out of the autofill framework until
  a row is used; it does not restore per-fill authorization against the
  matched app itself (`security-review.md` AN3).
* An unreadable device-settings blob (damaged, replaced) is read as
  "Confirm before filling" on: a bad blob never turns it off.
* Locked, the response is one "Unlock HavenKeys" row that names no login.
* Nothing is filled without a tap on a row. Saving logins from Autofill is
  §22.17.
* An OTP-only form (no username or password field) is offered the codes of
  the matched logins that have TOTP, never their passwords.

### 22.4a Cards and identity (Android M4)

Android Autofill also fills cards and the identity (`WalletPlanner`,
`WalletDatasets`, `FormRouter`). Nothing here has run on a phone yet
(`security-review.md` AN50).

* **What Rust decides** (`autofill_cards`, `autofill_card_values`,
  `autofill_identity`, `autofill_identity_values`, `autofill_save_card`;
  the same rules as the extension's card and identity fills). Cards are
  offered only to https browser pages and to apps; never to an http page in
  a browser (an app, its WebView included, gets them whatever it shows). A frame is allowed only if it is
  the tab's own site or a card frame of a payment processor on Rust's fixed
  list; any other frame is left out, and a refused focused frame gets
  nothing. Rust is told which roles the form asks for and returns only
  those values. Document numbers are returned only with the confirmation
  flag, and only on https pages or in apps. Cards and the identity are not
  bound to an app or a site, so no page rule applies to an app's WebView.
* **What Kotlin does.** It classifies fields (`CardFieldClassifier`,
  `IdentityFieldClassifier`), picks the form (`FormRouter`), shapes values
  to each field (`ValueShaper`: slices, expiry formats, lists, dates; a value
  that does not fit `maxlength` is left out, never cut) and fills only empty
  fields. It decides nothing about whether a value may be returned.
* **Direct fill.** While unlocked and "Confirm before filling" is off, the
  response carries the values of up to 5 cards (number, code, expiry, name)
  and the identity's non-document values for the allowed frames of that form
  to the operating system's autofill framework, which hands the app only
  the row the user taps (CLAUDE.md amendment of 2026-10-02; AN45).
* **Gated rows.** Document numbers, "Confirm before filling" and every row
  offered after "Unlock HavenKeys" carry no value. They open
  `AutofillAuthActivity`, which asks "Fill ... in <site or package>?" in a
  `FLAG_SECURE` window that ignores obscured touches. Rust is asked only
  after the tap, and the answer is one dataset. The app being filled cannot
  stand in for that tap (AN46).
* **Saving a card.** Only after the user confirms Android's save sheet; the
  number reaches Rust then. It needs the server (offline says the card was
  not saved), and a number already saved answers `Unchanged`, so nothing is
  duplicated. A card is saved only from the tab's own site, never from a
  processor frame (AN49).
* **Limits.** Frames depend on what the browser reports (AN47); emptiness is
  read from the structure and only a boolean is kept (AN48).
* **Generated records print their values.** The UniFFI-generated Kotlin
  data classes `CardValue`, `IdentityValue` and `SaveCard` (like M1's
  `FillValues`) carry the number, code, document numbers and other values,
  and their generated `toString()` prints them. They must never be logged,
  interpolated into a string, or passed to anything that may print them;
  they go from the repository straight into datasets (AN45).

### 22.5 Fill targets: browsers and apps

Rust decides the target on every call (`crates/havenkeys-core/src/app_target.rs`)
from the caller's package name, its signing certificates' SHA-256, and the
structure's `webDomain` and scheme:

* **Browser target:** the package and one of its certificates are on the
  privileged list, `crates/havenkeys-core/data/android-browsers.json` — the
  list Google's Credential Manager uses, release signatures only, refreshed
  by hand with `scripts/update-android-browsers.sh` and reviewed before
  committing. The reported domain and scheme become a page URL, matched by
  `origin.rs` exactly as the extension's page URL is. No scheme, no domain,
  or a domain containing `/ \ @ ? #` or a space gets nothing; an http page
  stays http, so it does not get an https login.
* **App target:** every other caller, WebViews inside apps included — the
  app controls its WebView, so its `webDomain` is ignored. An app matches a
  login only through an app binding (§22.6) or a Digital Asset Links
  statement (§22.7).
* HavenKeys never fills itself.
* The certificates come from `PackageManager` (`signingInfo.apkContentsSigners`).
  A package Android will not describe gives none, and Rust refuses a target
  without one. Android 11+ hides other packages from HavenKeys, Chrome
  included (seen on a Galaxy S24+, Android 16: no certificates, every target
  refused), so the `github` flavor requests `QUERY_ALL_PACKAGES`
  (`src/github/AndroidManifest.xml`). It lets HavenKeys list every installed
  app; HavenKeys reads only the signing certificates of the app it is
  filling. The `play` flavor does not request it yet: Google Play restricts
  that permission, and the `play` build's alternative is still open
  (`security-review.md` AN10).

### 22.6 App bindings and "Search HavenKeys…"

For an app target with a login field, the dropdown always ends with "Search
HavenKeys…". It opens `AutofillSearchActivity` (unlocking first if needed),
which searches logins by title, username and website (no secrets). Picking
one asks "Use <login> in <package>?". The prompt names the app by its
**package name**; the app's label is shown below it, marked as the app's own
claim, because an app can call itself anything.

Confirming calls `autofill_bind_and_fill`. Online, Rust stores an app
binding — package name plus certificate SHA-256, one per signing
certificate, at most 32 per login — inside the login's encrypted
details (an item write), then fills through the normal app match. Offline,
nothing is stored: the user's confirmation is that one fill's authorization,
and the toast says "Filled once". A browser target is never bound.

Like gated rows, the search row's IntentSender is held by the app being
filled. Both activities accept the structure Android attaches only when its
package equals `getCallingPackage()` (`structureNamesCaller`), because the
app that starts them could replace it; Rust then binds the fill to that
package's certificates. Whether `getCallingPackage()` is non-null in this
flow on real Android versions is unverified.

### 22.7 Digital Asset Links

When an app target asks for a fill and the setting "Check website–app links"
is on (default), Rust (`crates/havenkeys-mobile/src/asset_links_fetch.rs`)
fetches `https://<host>/.well-known/assetlinks.json` for the vault's login
hosts whose registrable domain's first label appears in the package name
(`com.github.android` → `github.com`), at most 8 hosts per request:

* HTTPS only; plain DNS names only (no IPs, no `localhost`, no single-label
  names); redirects are not followed; 3-second timeout; at most 128 KiB and
  256 statements; parsed statement by statement in Rust, so one bad statement
  voids only itself.
* A statement counts when its relation is
  `delegate_permission/common.handle_all_urls` or `common.get_login_creds`,
  and it names the package and one of its certificates. The site then
  vouches for the app, which is offered the logins saved for that site.
* The cache is sealed with the vault's data key in a device-local slot
  (`local_blob`, schema 6; never synced): a file is kept 7 days, a failure 1
  hour, 256 hosts at most. Nothing is fetched while locked.
* The requests go out with the user agent `havenkeys`. They tell each site
  (and anyone watching the network) that this phone holds a login for it and
  that an app with a matching name asked for a fill, at most once a week per
  site. Turning the setting off stops every fetch; bindings still work.

### 22.8 Locking

* The core `LockManager` and its auto-lock choices (never, 5, 15, 30, 60
  minutes), ticked every 5 seconds by a Rust thread and checked again when
  the app returns to the foreground. Only the user resets the idle timer:
  taps in the main activity (`onUserInteraction`) and typing in the search
  screen. No read does (list, search, open, reveal, TOTP), because the app
  also makes them on its own — a sync's `items_changed` reloads every 30
  seconds in the foreground, the item screen's TOTP code every second — and
  counting those would keep the vault unlocked forever (`security-review.md`
  AN7, desktop #1). Autofill does not reset it either.
* Every vault read, autofill call and biometric enrolment first applies an
  overdue auto-lock (`MobileVault::unlocked`), so a process that sat frozen
  past the deadline answers nothing before locking; a late touch locks an
  overdue vault instead of rescuing it (AN9).
* Lock when the screen turns off: a setting, on by default.
* The `LockManager` also locks when the wall clock jumps more than 30 seconds
  past the monotonic clock. Rust's monotonic clock on Android does not count
  deep sleep, so a phone that slept for more than 30 seconds probably locks
  on the next tick even with auto-lock set to never. Not yet observed on a
  device.
* Locking zeroes the Rust session and drops the server token. Every screen
  observes the lock: navigation returns to Unlock and revealed values go
  with their composables.
* Release builds use `panic = "abort"`: a Rust panic ends the process, and
  the in-memory vault with it (fail-closed).

### 22.9 Network

* The only outbound connections in M1: the user's server, and the Digital
  Asset Links fetches (§22.7). No analytics, no crash reporting, no updater
  (the spec's GitHub update check is not part of M1).
* All of HavenKeys' connections are made by Rust (reqwest, rustls), not by
  Android's network stack. TLS certificates are checked by
  `rustls-platform-verifier`, which asks Android's own trust manager over
  JNI, with Android's CA store (system and user-installed CAs). If its JNI
  initialisation fails, the bundled roots are used: every certificate is
  still verified, only user-installed CAs are missed.
* Plain HTTP: a release build of the native library refuses it everywhere,
  `localhost` included; a debug build (`scripts/build-android.sh` without
  `--release`) accepts it only to `localhost`, `127.0.0.1` and `[::1]`, for
  `adb reverse` (`security-review.md` AN20). The Digital Asset Links fetch
  is HTTPS only. `network_security_config.xml` says the same for Android's
  own stack (release: cleartext forbidden; debug: `localhost` and
  `127.0.0.1` allowed); it does not govern Rust's sockets, so Rust enforces
  its own rule. The desktop keeps `localhost` HTTP in every build.
* One cleartext exception for Android's stack, in both build types:
  `lencr.org` and its subdomains. Android's certificate check downloads the
  server certificate's revocation list itself, and Let's Encrypt publishes
  CRLs only over plain HTTP; blocked, every sign-in failed as "revoked"
  (`security-review.md` AN25). CRLs are signed by the CA, so this cannot be
  used to forge a verdict, and Rust's sockets are not affected.
* **User-installed CAs are trusted, on purpose:** a self-hosted server signed
  by the user's own CA must work, as it does in the phone's browser. The
  cost: anyone whose CA the user installed (or a device policy installed) can
  intercept the server connection — it sees ciphertext, the auth key at
  sign-in and the session token, not the vault — and forge
  `assetlinks.json` answers (`security-review.md` AN5). How the verifier
  treats user CAs has not been checked on a device.

### 22.10 Clipboard

* Copy is an explicit tap. The clip is marked sensitive
  (`android.content.extra.IS_SENSITIVE`), which hides it from the clipboard
  preview and keyboard suggestions on Android 13+.
* It is cleared after the vault's delay, and on lock, sign-out and removal,
  if it is still HavenKeys' clip.
* Android 10+ hides the clipboard from apps in the background, so HavenKeys
  usually cannot tell whether the user copied something else since. It
  clears anyway — the safe side for a secret, which may remove a newer copy
  made in another app.
* The timer lives in the process. If the process dies before it fires and no
  lock event ran, the clip stays.
* The foreground app, the keyboard and accessibility services can read the
  clipboard while it holds the value.
* Autofill offers "Copy one-time code" beside each code it offers. Some apps
  split a code across one box per digit and show Android only the tapped
  box, so the code cannot be filled into all of them; the user pastes it
  instead. The row fills nothing: tapping it opens HavenKeys, which asks
  Rust for the code for that exact app or site and copies it under the
  rules above.

### 22.11 Permissions, exported components and hardening

| Permission | Why |
|---|---|
| `INTERNET` | The user's server and `assetlinks.json` |
| `USE_BIOMETRIC` | Biometric unlock (BiometricPrompt) |
| `CAMERA` | Scanning the Emergency Kit's QR code; requested when scanning. `android.hardware.camera` is not required |
| `QUERY_ALL_PACKAGES` (`github` flavor only) | Reading the signing certificates of the app or browser being filled (§22.5) |

No other permission is declared (`ManifestTest` checks both sets). In
particular there is no `REQUEST_INSTALL_PACKAGES` and no accessibility
service.

**Exported components** in the source manifest: the launcher activity
(`MainActivity`), `HavenAutofillService`, guarded by
`android.permission.BIND_AUTOFILL_SERVICE`, and `HavenCredentialService`,
guarded by `android.permission.BIND_CREDENTIAL_PROVIDER_SERVICE` (§22.18), so
only the system can bind them.
`AutofillAuthActivity`, `AutofillSearchActivity` and the three credential
activities are not exported; only
HavenKeys' own PendingIntents reach them. There is no provider and no
manifest receiver (the screen-off receiver is registered at run time).
Libraries add components to the **merged** manifest: androidx
`ProfileInstallReceiver` (exported, guarded by `android.permission.DUMP`,
which only the system and the shell hold) in every build, and
`PreviewActivity` and `androidx.activity.ComponentActivity` in debug builds
only. `ManifestTest` checks the source manifest only.

**Hardening:**

* `FLAG_SECURE` on every activity: no screenshots or screen recording, and a
  blank recents thumbnail. Debug-only library activities do not set it.
* No autofill service sees HavenKeys' own fields: every activity sets
  `importantForAutofill = IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS` on
  its window, every dialog with a text field does the same on its own
  window (`SecureDialogWindow`), and no input carries an autofill hint. The
  master password and the Secret Key are never offered to, or saved by,
  Google Autofill or another password manager (CLAUDE.md §9;
  `security-review.md` AN19).
* The autofill activities and the app-binding prompt drop taps that pass
  through another app's window (`filterTouchesWhenObscured`, AN6).
  `MainActivity` does not, so screen filters and accessibility overlays keep
  working there.
* `allowBackup="false"`, and backup and data-extraction rules that exclude
  every domain, for cloud backup and device-to-device transfer.
* Release builds are minified and shrunk with R8.
* Chrome is listed as a compatibility package in `autofill_service.xml` for
  versions before Chrome's own autofill-service support; no other browser is.

### 22.12 Secrets in Kotlin, and memory

* Repositories and ViewModel state hold overviews only (titles, usernames,
  websites). A revealed value lives in the `remember`ed state of the
  composable showing it and is cleared after 30 seconds, when it leaves the
  screen, when the app goes to the background, and on lock. Never in a
  ViewModel, navigation argument, `SavedStateHandle`, Intent extra or
  `rememberSaveable`. Intents carry only a mode and an item ID; fill values
  go only into datasets.
* Typed secrets (master password, Secret Key, invite) go from the text field
  straight to Rust. Password fields use the password keyboard with
  autocorrect off.
* `MobileException` carries a stable code and a fixed sentence, never a
  secret. Types that hold a value override `toString()`.
* **Limitations.** The JVM cannot wipe a `String`: the master password, a
  typed Secret Key, revealed values and fill values stay in the Java heap
  until garbage collection. The decrypted bundle passes briefly through a
  `ByteArray` that is zeroed at once, but the JVM, the Keystore provider and
  UniFFI's buffers may hold copies we cannot reach. Camera frames of the
  Emergency Kit's QR code (which contains the Secret Key) are not zeroed.
  These join the limits in §8. The editor and Autofill save add to this:
  every typed value (a password, a TOTP key, a card number, a note) and every
  value read from a save request passes through `String`s that stay in the
  Java heap until garbage collection (§22.16, §22.17).

### 22.13 Logging and build supply chain

* Nothing logs. detekt's `ForbiddenImport`/`ForbiddenMethodCall` forbid
  `android.util.Log`, `println`, `print` and `printStackTrace`. Because
  detekt 1.23's type resolution cannot read Kotlin 2.4 metadata, a Gradle
  task, `forbidLogging`, also scans every Kotlin source as text and fails the
  build on any logging call; `detekt` depends on it. The Rust crate denies
  `print!`/`eprint!`/`dbg!`.
* Gradle dependency verification (`apps/android/gradle/verification-metadata.xml`,
  SHA-256) pins every dependency, including the `rustls-platform-verifier`
  AAR, which comes from a mutable GitHub Maven branch; every dependency bump
  regenerates it. CI validates the Gradle wrapper's checksum. The generated
  UniFFI bindings are committed, and CI fails if a rebuild changes them.

### 22.14 Known limitations

* Everything in this section is unverified on a device (above).
* The master password's minimum length (10) is checked by the UI, on the
  desktop and on Android; Rust does not enforce it.
* Revoking this phone from its own Devices list leaves its biometric bundle
  in place (Sign out deletes it). The next online bundle unlock is refused
  by the server and deletes it then.
* The Keystore cleanup on sign-out and removal runs from an app-wide event
  collector; a Keystore or clipboard failure there is caught silently, so a
  failed delete leaves its key until the next sign-out or removal
  (`security-review.md` AN1).
* "Confirm before filling" does not stop the matched app from firing its
  own gated rows while the vault is unlocked (§22.4, AN3).
* Writes need the server; nothing is edited or saved offline (§22.16).
* A save from Autofill is lost if the vault locks before the user submits
  the form, and the title of an app-saved login comes from the app's own
  label (§22.17, AN32, AN33).
* The Autofill service's coroutine scope has no exception handler: an
  unexpected throw while parsing a hostile structure ends the process, which
  locks the vault (fails closed, as in the fill path; AN30).
* Editing and saving have not run on a phone or an emulator (AN36).

### 22.15 Distribution

* The APK is published on GitHub Releases from `android-v*` tags by
  `.github/workflows/android-release.yml`, never through a store. It is a
  release build: the workflow builds the release-mode Rust library
  (`scripts/build-android.sh --release`) and refuses an APK that is unsigned
  or debuggable.
* It is signed with the HavenKeys release key (custody:
  `docs/android.md`). Android installs an update only when it is signed with
  the same key, so a tampered or re-signed APK cannot replace an installed
  HavenKeys.
* Each release's notes list the signing certificate's SHA-256, which the
  download page also shows once the fingerprint is set in
  `ANDROID_CERT_SHA256` (`apps/web/src/lib/releases.ts`), and the release ships
  `HavenKeys-<version>.apk.sha256`. A first install can be checked with
  `apksigner verify --print-certs`. The checksum and the certificate come
  from the same GitHub release as the APK, so they catch a damaged download,
  not a compromised release: only a fingerprint known from another source
  does that.
* The release is created as a draft and never marked "latest", so the
  desktop download and auto-updater are unaffected; publishing it is a
  manual step (`docs/website.md`).
* The download page fetches the release list from GitHub's API and accepts
  the APK only from this repository's release-download URL.

### 22.16 Editing

Android M2. The editor (`ui/edit/`) creates, edits and deletes logins,
secure notes and cards, and edits the identity (the core refuses to create or
delete the identity from a phone).

* **The editor sends changes, not items.** Each field is `Keep`, `Replace`
  or `Remove`; Rust merges them into the stored item
  (`crates/havenkeys-mobile/src/edit.rs`). A hidden value (password, TOTP
  key, card number, security code, a note's text) is read into the editor
  only when the user taps Change, through `reveal`; a blank replacement of
  a TOTP key keeps it (only Remove clears it).
  A card whose title is its brand's name opens with a blank title, so a new
  number of another brand renames it. Everything the editor
  does not show is kept by every phone edit: custom fields, passkeys, app
  bindings, "Sign in with" and a card's brand. The editing API returns field
  names and presence, never a value, so `havenkeys-mobile` still returns no
  whole item.
* **The draft lives in the edit screen's composition only.** It is
  `remember`ed, never `rememberSaveable`d, so it is not written to saved
  instance state. `MainActivity` handles
  `orientation|screenSize|screenLayout|smallestScreenSize|keyboardHidden|keyboard|density|uiMode|fontScale`
  changes itself, so rotating, folding, a theme change or a font-size change
  does not recreate the activity and an open draft survives; a locale change
  or the process dying still discards it. The lock wipe and leaving the
  screen drop the draft. The window is `FLAG_SECURE` and excluded from
  autofill services, so neither a screenshot nor another autofill service
  sees the fields.
* **Keyboards.** Every editor field adds
  `EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING` (`NoPersonalizedLearning.kt`),
  and single-line secrets use the password keyboard with autocorrect off.
  Keyboards may ignore the flag (an Android limitation), so a third-party
  keyboard can still see and retain what is typed.
* **A value being loaded cannot be edited.** A present plain-text value
  (for example a card's holder or expiry, an identity's name) loads when the
  editor opens, and its field stays disabled until it arrives. A present
  hidden value loads when the user taps Change, and its field opens only
  after the load finishes. Save waits for every load, so a save cannot
  replace a value the user has not seen with an empty one.
* **Writes are online only.** `create_item`, `update_item` and `delete_item`
  refuse with `offline` before staging anything; the editor shows a banner.
  Nothing is recorded locally until the server has accepted the write
  (`stage → push → commit`). The server's revision check turns a concurrent
  edit into "This item changed on another device." and the app reloads;
  nothing is overwritten silently. An edit made from an older revision than
  the phone now holds (a pull brought another device's change while the
  editor was open) is refused on the phone with the same "This item changed
  on another device." and reloaded, before anything is sent.
* **Typed values pass through JVM `String`s** that cannot be zeroed, the
  same limitation as the master password field (§22.12).

### 22.17 Saving from Autofill

Android M2. When the user submits a login form in an app or a browser,
Android offers its own save sheet; HavenKeys saves only if the user confirms
it, which is the confirmation CLAUDE.md §27 asks for.

* **`SaveInfo` is attached only to a response built while unlocked, for a
  target Rust classified** (a browser page or an app). A locked vault gets
  no `SaveInfo`, so a login typed while locked is not saved. HavenKeys' own
  screens are never saved.
* **Values are read from the save request only for the fields being saved**
  (`StructureParser.textOf`); `FieldFacts` keeps no typed text. Rust decides
  whether the save adds a login, updates one, or is unchanged
  (`crates/havenkeys-mobile/src/save.rs`, `app_fill.rs`).
* **A browser save** adds a login with a whole-site rule for the page's own
  host (from the privileged browser's reported page, §22.5), and updates only
  a login that already matches that page. It never touches a login that does
  not match.
* **An app save** adds a login bound to that app's package and every signing
  certificate, with no website, and updates only a login matched to that app
  (bound to it, or vouched for by Digital Asset Links, §22.7). It never
  touches any other login. The new login's title comes from the app's
  label, which the app chooses: see `security-review.md` AN32.
* **A username-first sign-in** (username on one screen, password on the
  next) uses `FLAG_DELAY_SAVE` on Android 10+ so the two screens save as one
  login. On Android 9 there is no such flag: the password step saves
  without the username unless that screen has one.
* **Timing.** The answer to Android comes within 8 seconds; a later failure
  (for example the server refusing) shows a Toast. If the vault locks before
  the user submits, nothing is saved and the user sees "HavenKeys locked
  before saving."
* Offline, a save answers "HavenKeys is offline. The login was not saved."
  and stages nothing.
* Autofill is not app use: neither the fill nor the save path resets the
  idle timer.

### 22.18 Passkeys and Credential Manager

Android M3 (Android 14+). `HavenCredentialService` makes HavenKeys a
credential provider for Credential Manager, so websites in browsers and apps
can sign in with, and create, passkeys that live in the vault (the same
objects the desktop and the extension use, §15), and Credential Manager can
offer saved passwords. Rust decides everything that matters; Kotlin reports
Android's facts and draws UI. Nothing here has run on a phone yet.

* **Components.** `HavenCredentialService` is the only newly exported
  component. It is guarded by `android.permission.BIND_CREDENTIAL_PROVIDER_SERVICE`,
  so only the system can bind it. `CredentialUnlockActivity`,
  `CredentialGetActivity` and `PasskeyCreateActivity` are not exported; only
  HavenKeys' own PendingIntents reach them. Each sets `FLAG_SECURE`,
  `filterTouchesWhenObscured`, excludes itself from autofill and from
  recents. No new permission is declared. Credential Manager hands the
  service the caller's signing information, so passkeys need no package
  visibility: the `play` flavor, which has no `QUERY_ALL_PACKAGES`, can use
  them (§22.5).
* **Caller rules** (`credentials.rs`, `passkey/rp.rs`). A request carries an
  origin only from a browser whose package and a signing certificate are on
  Rust's privileged list, the same list autofill uses (§22.5). Kotlin asks
  Android's `getOrigin` with that list, and Rust checks the caller against
  the list again. Kotlin drops an origin Android will not vouch for, so a
  caller that is not a privileged browser reaches Rust as an app and is
  denied unless the site grants that app `get_login_creds`; if an origin
  nevertheless reaches Rust from a caller not on the list, Rust refuses it.
  Every caller without an origin is an app. Only a browser's origin is
  treated as a page URL, under the extension's rule (`authorize_rp`).
* **App passkeys.** An app may use an RP ID only if the RP ID is a
  domain with a registrable domain of its own (subdomains allowed; never a
  public suffix, IP address or `localhost`) and
  `https://<rp_id>/.well-known/assetlinks.json`, fresh in the 7-day cache
  (§22.7), lists the app's package and the signing certificate Android
  reports with `delegate_permission/common.get_login_creds`.
  `handle_all_urls` alone does not authorize passkeys. The origin the site
  sees is `android:apk-key-hash:<unpadded base64url SHA-256 of that
  certificate>`, so it names the certificate the site vouched for. A passkey
  created for an app gets a login whose website is the RP's site.
* **`clientDataHash`.** Rust signs a hash supplied by the caller only for a
  browser. An app's hash is ignored and Rust builds `clientDataJSON` itself;
  signing an app's hash would let the app choose the origin the site sees.
* **User verification.** Every passkey use (sign-in or create) shows
  `BiometricPrompt` with `BIOMETRIC_STRONG` or the device screen lock. The
  prompt is skipped only right after the user unlocked HavenKeys in that same
  activity. A phone with no screen lock is refused. **Rust cannot check that
  Kotlin prompted**: the app process is trusted to enforce user verification,
  as the extension trusts the desktop's click (security-review AN37).
* **Passwords.** For a password request Rust applies the target rules of
  §22.5 (a browser's page, or an app's bindings and vouched sites). Only
  logins with a username are offered. The password goes from Rust straight
  into the result Intent; no extra, state or log holds it. When the vault is
  already unlocked this path asks no user verification, like direct fill
  (§22.4). Saving a password through Credential Manager is not supported;
  saving stays with Autofill (§22.17).
* **Creating.** A passkey is created only online and after the user taps
  "Save" in HavenKeys; nothing is recorded locally until the server
  accepted it. A conditional create (Chrome's automatic upgrade) is refused
  on Android, so nothing is saved without a tap in HavenKeys. A request whose
  `excludeCredentials` names a passkey the vault holds for that RP ends with
  "This account already has a passkey in HavenKeys."
* **Responses.** `transports` is `["internal"]` and `credProps.rk` is `true`.
  Offers list the RP ID, account name and credential ID, never a key.
  `clientDataJSON` for a browser is a placeholder that the browser replaces
  (`crypto.md`, Passkeys).
* **PendingIntents.** The entries the service returns carry PendingIntents
  that must be mutable (Credential Manager adds the request to them). They
  are explicit, point only at HavenKeys' non-exported activities, and use a
  per-process request-code counter. Rust re-checks the caller, the origin and
  the item on every request, so a replayed or redirected Intent can at most
  repeat a decision Rust makes again.
* **Locking.** A locked vault offers one entry, "Unlock HavenKeys", and no
  passkey or login names. Credential Manager and Autofill are not app use:
  neither touches the idle timer.
* **Known limitations:** `security-review.md` AN37-AN44.

### 22.19 Device-local sealed slots

The following data is sealed with the vault's data key in device-local slots
(`local_blob`), never synced to the server, and readable only while unlocked:

* **Activity**: Item UUIDs with decayed use scores (most recent fills and copies
  are weighted higher) and last-use times (Unix ms), plus up to 10 most recent
  search queries. Used by the Home screen to list frequently used items and
  recently created items, and by search to show recent queries. An unreadable
  or newer-version document reads as empty.
* **Asset Links cache**: Web hosts and their associated app packages/certificates
  (schema 6), for matching Autofill requests to logins. A file is kept 7 days,
  a fetch failure 1 hour, 256 hosts at most. See §22.7.
* **Device settings**: "Lock when the screen turns off", "Confirm before
  filling" and "Check website–app links" for this phone
  (`crates/havenkeys-mobile/src/settings.rs`). Nothing written yet reads as
  the defaults; a blob that does not open reads as the defaults with
  "Confirm before filling" on, so a damaged blob never turns it off.

Each slot is sealed under its own blob purpose (`device-settings`,
`asset-links`, `activity`), so one slot's blob does not open as another's,
and another vault's key opens none of them. The activity record reveals,
to someone who can unlock the vault, which items are used most and the
recent searches: no more than the vault itself; to anyone else it is
ciphertext.

On Android the Home and search screens hold this data only in their
ViewModels' memory and drop it on lock, sign-out and removal. The query
being typed lives in the search screen's ViewModel and its field's
composition state; it is never put into a navigation route, saved instance
state, `SavedStateHandle` or a log, and it becomes a recent search only
when a result is opened.

## 23. Signing in a new desktop from the phone

Spec: `docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md`.
The trust decision and its residual risk are in `threat-model.md` T13; the
envelope is specified in `crypto.md`; the review is in `security-review.md`
("Phone-approved sign-in"). This is an internal review, not an independent
audit, and nothing on the Android side has run on a phone.

### 23.1 What moves where

The new desktop generates an HPKE key pair and a 32-byte `claim_secret` in
Rust, in memory only. Its QR code carries
`havenkeys://pair/v1?server=<url>&id=<pairing_id>&pk=<public key>` (at most
512 bytes; exactly those three parameters, once each). The phone's Rust
core accepts only that link, refuses another server's, and refuses when
locked or offline. After Allow (behind `BiometricGate`) the core seals the
vault key, the Secret Key, the account and vault ids and the email to `pk`;
the server stores that ciphertext and the desktop claims it. The public key
is never sent to the server, only shown in the QR code: HPKE base mode does
not authenticate the sender, so this is what keeps the server from sealing
an envelope of its own to the desktop. The desktop checks the vault
header's attestation with the vault key it received and that its account
and vault ids equal the envelope's, builds the local vault, goes online,
and shows "Signed in as <email>" (PA14). The next unlock is the ordinary one (master password and
the stored Secret Key).

### 23.2 Endpoints

| Route | Auth | Limits | Returns |
|---|---|---|---|
| `POST /v1/pairings` | none | At most 10 pairings per IP in 10 minutes and 3 pending per IP; over that, `429`. Body at most 1 KiB. Unknown fields refused; device name cleaned to 64 characters, without bidi or zero-width characters; `claimHash` exactly 32 bytes base64url; no public key (a `publicKey` field is refused as unknown) | `{pairingId, expiresAt}` (expiry 120 s) |
| `GET /v1/pairings/{id}` | session | Id must be 22 base64url characters (else 404) | `{deviceName, ip, location, createdAt, expiresAt}`; binds the pairing to the caller's account |
| `POST /v1/pairings/{id}/approve` | session | Envelope at most 4096 bytes (`MAX_PAIRING_ENVELOPE_BYTES`), standard base64 in JSON | `204`. In one transaction: registers the device (`register_device`, so the 64-device cap and revoked-device rules apply; a revoked or other-account device id, or the approver's own, is `400`, never `401`, so the phone is not signed out), sets `devices.approved_by`, stores the envelope. No session yet |
| `POST /v1/pairings/{id}/deny` | session | as above | `204` |
| `POST /v1/pairings/{id}/claim` | `claim_secret` (not a session) | A wrong secret counts against the IP like a failed login (`rate_limit`); a blocked IP is refused first | `{state: "waiting"}`, `{state: "denied"}`, or `{state: "approved", token, expiresAt, accountId, vaultId, envelope}`; the approved answer replaces any sessions of that device and issues its 24-hour token in the claim's transaction |

Unknown, expired, used, denied, other-account and malformed-id pairings all
answer 404 (`not_found`), so the answers do not say which. A device cannot
approve its own pairing. The claim runs under a row lock in one transaction,
so two claims with the right secret cannot both get a session; it answers
once (the row becomes `claimed`, its envelope cleared) and refuses
any row older than 10 minutes. A pairing is bound to the first account whose
session reads or approves it. All bodies are under the global request-size
limit; there is no per-route limit on the unauthenticated routes (PA6).

Where the code differs from the spec: a denied pairing is marked `claimed`
by the claim that reports the denial, not deleted, so it keeps counting
against its IP's limit; an approved row can still be claimed after the
2-minute `expires_at` and up to 10 minutes after creation.

### 23.3 What the `pairings` table holds, and for how long

Migration `0002_pairings.sql`: `id`, `state` (`pending`, `approved`,
`denied`, `claimed`), the desktop's `device_id` and `device_name`,
`claim_hash` (SHA-256 of the claim secret), the requester's `ip` and
`location` label, timestamps, `account_id` (once bound), and, after
approval, the `envelope`. It holds no public key and no session token: the
key travels only in the QR code, and the session is issued by the claim
(its hash goes in `sessions` as for any login), so an approval never
claimed leaves no session. The envelope is cleared when the claim succeeds.
The server has no periodic task: every `POST /v1/pairings` first deletes all
pairings older than 10 minutes, whatever their state, and the claim refuses
older rows. So an approved pairing nobody claims keeps its envelope until
the next create (by anyone) deletes it, which is not bounded in time on a
server that sees no new pairings (PA3). The envelope is ciphertext the
server cannot open. `claim_hash` is a hash of a 256-bit random secret.

`devices.approved_by` holds the approving device's id, and the Devices lists
(desktop and phone) show "Approved by <name>". The new device is an ordinary
device in every other way and can be revoked there.

### 23.4 Locating the requester

Optional `HAVENKEYS_GEOIP_DATABASE=/path/to/db.mmdb` (a MaxMind-format
database such as DB-IP Lite, which the operator downloads). It is read at
start-up with `maxminddb` and looked up locally; no third party is called.
Without it, `location` is null and the phone shows the IP only. The IP is
the server's own (`client_ip`, which honours `X-Forwarded-For` only when
`trust_forwarded_for` is set). City and country text from the database is
dropped if it has control characters or exceeds 64 characters; format
(bidi/zero-width) characters are not filtered, and the country code is not
checked to be two ASCII letters (PA4).

### 23.5 The core session now holds the vault key

An unlocked vault used to keep only the data key. Sealing the envelope needs
the vault key (so the new device can build its local vault exactly as a
sign-in does and verify the header itself), so the core session keeps the
vault key too while unlocked: a `Key256`, zeroized on lock like the data key
and never returned by any API. Its exposure class is the data key's: both
open the vault, and both live only in the unlocked core process. The
envelope is sealed inside `VaultService`; the vault key never reaches the
Tauri, uniffi or Kotlin layers.

### 23.6 Client-side rules

* Rust on the desktop keeps the key pair and claim secret in `HavenClient`;
  React gets the QR module grid and states (waiting, expired, denied,
  approved), never a key. Polls are serialised, and the panel never drops an
  approved poll answer, even if the window re-renders or the code expires
  during the poll. The code's 120 s are counted on the desktop's own clock,
  with one last poll at 0; a failed or gone pairing stops the polling and
  keeps its message, while being offline does not.
* The three Tauri commands (`pairing_start`, `pairing_poll`,
  `pairing_cancel`) are allowlisted; the server address is typed, the
  account comes from the phone.
* On Android, Allow needs `BiometricGate`; on a phone with no screen lock it
  shows "biometric unavailable" and approves nothing. A code that already
  failed is not requested again on every camera frame. The link and the
  pairing's details are kept only while the confirmation sheet is open.
* Errors say "The sign-in could not be completed. Ask for a new code." and
  similar; none contains a key, token or envelope.
* Logs: the server logs only `outcome` and ids at `info`; envelopes, tokens,
  claim secrets and keys are never logged, and `PairingKeys`, `ClaimSecret`
  and `PairingPayload` print as `<redacted>`.

## 24. Vault health

Spec: `docs/superpowers/specs/2026-10-07-vault-health-design.md`. No rule of
`CLAUDE.md` is relaxed.

### 24.1 What it does

A screen (desktop and Android) lists the logins that need attention: weak,
reused and old passwords, a site that accepts passkeys or one-time codes
that the login does not use, an `http://` website, and duplicate logins.
Everything is computed in `havenkeys-core` (`health/`) on the unlocked
vault. No network access and no third party: leaked-password lookups (Have
I Been Pwned) are deliberately not implemented.

### 24.2 What the report contains

`HealthReport` holds item IDs, check kinds, group indexes and counts,
and per issue a `help` flag (whether the site's directory entry has an
https setup guide; derived from public directory data, not from the
password). It holds **no** password, strength score, length, hash, title,
username or URL; the apps already hold the item overviews and join on the
ID. The `reused_group` and `duplicate_group` numbers are indexes assigned
when the report is built, ordered by the smallest item ID in each group,
so the number says nothing about the password. `Debug` is redacted on
every report type that carries item IDs (`HealthCounts` and `HealthCheck`
derive it and hold no secret). A test serializes a report and asserts that no password or
substring of one (4+ characters) appears in it.

What the report does reveal is *which* logins are weak or reused. Anyone
who can read the unlocked UI's memory can already read the overviews and
reveal passwords, so this adds no new reader.

### 24.3 Where it lives and when it is dropped

* The report is cached in the unlocked vault state and dropped with it on
  lock. It is recomputed after any item change (a local write, or a sync
  pull that changed at least one item; an empty pull does not clear it),
  after unlock, and when it is older than 10 minutes (`HEALTH_CACHE_MS`),
  so "old" does not go stale in a long session.
* The passwords are read under the vault lock into a short-lived snapshot
  (`health_snapshot`). The lock is released before the slow part (zxcvbn)
  runs, so fills from the extension or Android Autofill never wait on it.
  The snapshot is released when the computation ends. The snapshot carries
  a generation and the session epoch; `store_health` caches the result only
  if neither moved. A report computed from a stale snapshot (an item
  changed meanwhile) is returned to its caller but never cached; one
  computed across a lock (with or without a later unlock) is neither cached
  nor returned: `HavenClient::health_report` answers `Locked`.
* Grouping of reused passwords uses a `HashMap` keyed by a borrowed `&str`,
  dropped before the report is returned. Nothing derived from a password is
  kept.
* The desktop `health_report` command deliberately does **not** count as
  user activity for auto-lock (`CLAUDE.md` §16), like `list_items`: the view reloads in
  the background, and a machine-driven call must not hold the vault open.
  Reloads happen on item changes, not on search.

### 24.4 zxcvbn and memory

`zxcvbn` (MIT) copies up to 100 characters of each password
into its own allocations and drops its match list without zeroizing it. This
is the same limitation as every other heap `String` in the process (§8,
"Memory handling"): the core cannot control a third-party crate's
buffers. The passwords it sees are the ones already in the vault's decrypted
details; the exposure window is the compute (milliseconds per password), and
the process is the same one that holds the vault key. Accepted and
documented.

### 24.5 Site directories and help links

* `crates/havenkeys-core/data/passkey-sites.json` (moved from the
  extension; the extension imports the same file) and
  `crates/havenkeys-core/data/twofactor-sites.json` (sites listing `totp` in
  `2factorauth/twofactorauth`) are embedded with `include_str!`, parsed once,
  and never fetched at runtime. They are refreshed by
  `scripts/update-passkey-directory.mjs` and
  `scripts/update-twofactor-directory.mjs` and reviewed like code. An entry
  that fails validation is dropped, never a panic.
* A login matches a directory site only through `origin.rs`: the rule's host
  equals the site's domain or is a subdomain of it, decided on the registrable
  domain from the Public Suffix List. `evilgithub.com`, `github.com.evil.com`
  and `github-login.example.com` never match `github.com`.
* The data is taken unedited from upstream, and neither list has a top-level
  `google.com` entry (only product subdomains such as `mail.google.com`), so
  a login saved as `google.com` gets no passkey or 2FA suggestion.
* "How to enable" links come from the directory entry the core looked up
  from the login's own URL rules; the UI never passes a URL. Only `https`
  links are returned; no link is `NotFound`. The report's `help` flag comes
  from the same lookup (`Directory::help_for`), so the desktop offers "How
  to enable" only where a link exists. The desktop's Rust opens it
  (`open_health_help`); on Android Kotlin hands the string to an `Intent`
  unchanged. A link is opened only on an explicit click, in the system
  browser; the desktop never loads a remote page (§41 of the project rules).

### 24.6 Dismissals

A dismissed check is the login's `health_ignored` list, ordinary encrypted
item data that syncs (§4: the server sees one more ciphertext update, as for
any edit). It needs the server like every change. It does not change
`updated_at`, so dismissing does not reorder "recently edited". Only login
items accept it; any other ID is rejected. Every item editor keeps the field
unchanged, as it does for `app_bindings`. It is read leniently: a check kind
this build does not know (added by a newer app) is dropped rather than
failing the whole login.

### 24.7 Commands

Three allowlisted desktop commands (`health_report`, `set_health_ignored`,
`open_health_help`) and three `havenkeys-mobile` calls (`healthReport`,
`setHealthIgnored`, `healthHelpUrl`). A locked vault returns `Locked`; errors
carry no item content. Native messaging and the extension are not touched.

### 24.8 Known limitations

* Group indexes are assigned before dismissals, so after some members of a
  reused or duplicate group are dismissed a chip can show fewer members than
  the group has (the UIs never show fewer than 2 / 1).
* An imported login's `created_at` is the import time unless the format
  carried one, so "old" under-reports for imports. Likewise a login created
  without a password that gets one later is measured from `created_at` (no
  history entry records the first password), so it can show "old" early.
* Both UIs build a login's new dismissed list from the report's
  `dismissed`, which only lists checks that currently apply. A dismissal
  whose check no longer applies (dormant, e.g. "weak" after the password
  was changed) is dropped when the user dismisses or restores another check
  on that login, and the check shows again if it later applies.
* "Weak" is zxcvbn's estimate, not a guarantee; a passphrase it scores 3 can
  still be guessable by someone who knows the user.
