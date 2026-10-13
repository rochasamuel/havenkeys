# Quick Access window — design

Date: 2026-10-13. Status: approved in conversation.

Implements item **A3** of `docs/autofill-roadmap.md` (global shortcut +
Quick Access). It is useful on its own (search and copy from any app) and
is the window that A1 (fill into desktop apps) will later extend.

## 1. Goal

From any application, a global shortcut opens a small HavenKeys window:
type a few letters, press Enter, and the login's password is on the
clipboard (cleared after the vault's clipboard delay); the window gets out
of the way and the previous app is in front again.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Actions | Copy password (Enter), username, TOTP code; open the login's website |
| Items | Logins only in this version |
| Locked | The window asks for the master password itself, then searches |
| Shortcut | `Cmd+Shift+Space` (macOS), `Ctrl+Shift+Space` (Windows, Linux X11); fixed in this version |
| Setting | Settings → General: "Quick Access shortcut" switch, on by default, per device |
| Registered by | Rust only (`tauri-plugin-global-shortcut`); no renderer permission |
| Fill into apps / browser | Out of scope (A1; extension already fills browsers) |
| Custom shortcut recorder | Out of scope |

## 3. Behaviour

* **Open:** the shortcut shows the `quick` window centred on the active
  display, always on top, without decorations (about 640×420), search field
  focused. Pressing the shortcut while it is open hides it.
* **Locked:** the window shows a master-password field and an Unlock button.
  Unlocking uses the existing `unlock_vault` command; the main window
  follows the shared lock state as it does today. A wrong password shows the
  existing generic error.
* **Search:** the existing `list_items` command with a query (in-memory
  search, no index, §37), filtered to logins. Each row shows title and
  username; secrets are never in the list or in React state. Arrow keys move
  the selection.
* **Keys:**
  * Enter — copy the password (`copy_secret`, field `password`), then hide.
  * Cmd/Ctrl+C — copy the username, then hide.
  * Cmd/Ctrl+T — copy the TOTP code (only when the login has TOTP), then
    hide.
  * Cmd/Ctrl+O — open the login's first website (`open_website`), then
    hide.
  * Esc — hide.
* No "Copied" toast: hiding the window is the confirmation, and the
  clipboard delay is shown in Settings.
* **Hide:** on Esc, after an action, and when the window loses focus. The
  operating system returns focus to the previous application.
* **Lock:** when the vault locks (manual, auto-lock, OS session lock) the
  window hides and its view resets (query cleared).
* **Shortcut taken:** if registration fails (another app owns the
  shortcut), the switch in Settings shows "The shortcut is in use by another
  app" and stays on; nothing else breaks.

## 4. Security boundary

* **Separate window, same bundle:** the `quick` window loads the app's own
  local bundle (a separate HTML entry, e.g. `quick.html`), under the same
  CSP. It never loads remote content.
* **Separate capability:** `capabilities/quick.json` grants only:
  `vault-status`, `unlock-vault`, `list-items`, `copy-secret`,
  `open-website`, and the window commands it needs (hide itself, listen to
  the lock event). It does not get edit, reveal, export, settings,
  account, pairing or passkey commands. `main.json` is unchanged.
* **Secrets stay in Rust:** copying already happens in Rust
  (`copy_secret` reads the value and writes the clipboard; the renderer
  gets only the clear delay). Quick Access never calls `reveal_secret`.
* **Shortcut in Rust:** registration and unregistration happen in Rust from
  the setting; the renderer cannot add or change global shortcuts.
* **Activity:** typing and copying count as activity for auto-lock exactly
  as in the main window (`copy_secret` already touches).
* **Limits (documented):** global shortcuts do not work on Linux Wayland
  sessions; the window must take keyboard focus (a non-activating panel
  comes with A1); clipboard limits of §34 apply unchanged.

## 5. Setting

The shortcut must work while the vault is locked (to unlock from Quick
Access), and the vault `Settings` and local slots are sealed under the vault
key. So the preference lives in a small plaintext file in the app's config
directory, `quick-access.json`, holding only `{ "shortcut": true }`. It is
not sensitive and is listed as intentionally plaintext metadata in
`docs/security-model.md` (§36). Missing or unreadable file → default on.
Turning the switch off unregisters the shortcut immediately and writes the
file; on registers it.

## 6. Tests

Rust (`apps/desktop/src-tauri`):

* The setting registers/unregisters the shortcut (through a small trait so
  the test does not need a real OS hook).
* `capabilities/quick.json` grants exactly the allowed permission list (a
  test parses the JSON and compares to a constant).
* Lock hides the quick window (event emitted; handler covered by a unit
  test where practical).

React (vitest):

* Locked → password field; unlock → search.
* Typing filters; arrows move; Enter calls `copy_secret` with `password`
  and then hide; Cmd/Ctrl+C, +T (only with TOTP), +O call the right command;
  Esc hides.
* No component state ever holds a secret (the component only receives
  `CopyResult`).

Manual (in `docs/security-review.md` checklist): macOS and Windows — the
shortcut opens the window over another app, Enter copies, focus returns.

## 7. Docs and review

`docs/architecture.md` (second window), `docs/security-model.md` (Tauri
capabilities: the quick window's list), `docs/development.md` if the build
needs a second entry, `docs/security-review.md` entry (QA1: new window and
capability, Info; QA2: global shortcut can be triggered by any local
process that can send key events — same trust level as the user's session,
Info). Impeccable review of the window. Roadmap marks A3 done
(uncommitted). No `CLAUDE.md` amendment.
