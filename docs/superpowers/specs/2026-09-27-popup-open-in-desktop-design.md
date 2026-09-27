# Edit a login in the desktop app from the extension popup — Design

Status: proposed, 2026-09-27.

> This software has not undergone an independent security audit.

## 1. Goal

Each login row in the extension's **toolbar popup** gets one more icon
button, last in the row, after **Fill** and **Code**: **Edit in HavenKeys**.
Clicking it brings the HavenKeys desktop window to the front (restoring it
from the tray if hidden) with that login open in the editor.

The in-page autofill menu does not get this button.

Success: on github.com, the user opens the popup, clicks the edit icon on
"GitHub · alice", and the desktop app appears with GitHub's editor open.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Transport | New native-messaging request `open_item`, validated in Rust like `fill_item` | `havenkeys://` deep link (any site or program could fire it; no origin binding); "open the app" hint without navigation |
| Which items | Only a login the current tab's URL matches (same rule as Fill) | Any item ID the extension names |
| Unsaved edit of another item | Desktop asks "Discard your changes to ‹title›?" — Discard / Keep editing | Silently switch; keep the current edit and ignore |
| Popup after success | Closes (focus has moved to the desktop app) | Stays open |
| Rate limit class | `Secret` (the stricter bucket): the request has a visible effect | `Lookup` |

## 3. Flow

```text
Popup row  [Fill] [Code] [✎]
             │ click ✎
             ▼
popup_open_item { itemId }                      (popup → background)
             │ background accepts it only from the popup page,
             │ adds the active tab's url / top_url itself
             ▼
open_item { item_id, url, top_url }             (native messaging)
             ▼
Bridge (Rust)
  1. rate limit (Secret class)
  2. vault unlocked, bridging enabled
  3. item is a login saved for url (and top_url) — same matching as fill_item
     otherwise → denied / locked / not_found, hook NOT called
  4. on_open_item(item_id)
  ← ok {}   (no secret, no item data)
             ▼
Desktop (Rust hook)
  show_main_window; emit "vault://open-item" { id }
             ▼
VaultScreen (React)
  decideOpen(pane, dirty, id):
    editing id already        → nothing (window is already up)
    editing another, dirty    → confirm "Discard your changes to ‹title›?"
    otherwise                 → section = logins, clear search, pane = edit(id)
```

## 4. Components

### 4.1 Protocol (`crates/havenkeys-protocol`)

`Request::OpenItem { item_id: Uuid, url: String, top_url: Option<String> }`
with the same URL length/shape validation as `FillItem`. Response: an empty
`ResultBody::OpenItem {}` (wire `{"type":"open_item"}`, every result names
its request). The TypeScript protocol package (`packages/protocol`) mirrors
it.

### 4.2 Bridge (`crates/havenkeys-bridge`)

* `server.rs`: `OpenItem` → `RequestClass::Secret`.
* New hook `on_open_item: Box<dyn Fn(Uuid) + Send + Sync>` beside
  `on_lock` / `on_items_changed`, taken by the `Bridge` constructors.
* `dispatch.rs`: `require_enabled`, unlocked, then the item must be one of
  `find_matches(url, top_url)`'s results (the same origin-bound check
  `fill_item` relies on). Only then call the hook and return `OpenItem {}`.

### 4.3 Desktop (`apps/desktop`)

* Rust: the hook passed to the bridge runs on the bridge's thread; it
  calls `tray::show_main_window(&app)` and
  `app.emit("vault://open-item", OpenItemPayload { id })`.
* `VaultScreen.tsx`: listens for `vault://open-item`; asks a pure,
  tested `decideOpen(...)` what to do; shows the existing confirm dialog
  style for the dirty case.
* `ItemEditor.tsx`: reports whether it has unsaved changes
  (`onDirtyChange(boolean)`), compared against the values it opened with.
* Strings in `en` and `pt-BR`.

### 4.4 Extension (`apps/extension`)

* Popup `matchRow`: icon button (pencil) appended last to `.actions`,
  `title`/`aria-label` "Edit in HavenKeys"; on success `window.close()`;
  on error the row's status line shows the message, as Fill does.
* `popup-handler.ts`: `popup_open_item { itemId }` parsed strictly
  (UUID), accepted only from the popup page (the existing `isPopup`
  check), URL taken from the active tab, never from the message.
* Strings in `en` and `pt-BR`.

## 5. Errors

| Case | Popup shows |
|---|---|
| Desktop app not running | existing `desktop_unavailable` ("The HavenKeys app is not running.") |
| Vault locked | existing `locked` message |
| Login not saved for this page, or deleted | existing `denied` / `not_found` messages |
| Rate-limited | existing rate-limit message |
| Desktop offline | Editor opens read-only with the usual offline banner (not an error) |

## 6. Security

* **No secrets cross.** The request carries an item ID and the page URL;
  the reply is an empty OK.
* **Origin binding in Rust.** The bridge refuses any item the current URL
  does not match (CLAUDE.md §31–32), so the extension cannot open, or
  probe, arbitrary items.
* **Bounded side effect.** The only effect is showing the window and
  opening the editor; nothing is saved without the user. Rate limiting
  (Secret class) stops a compromised extension from repeatedly pulling
  the window to the front.
* **Popup only.** Content scripts cannot send `popup_open_item`.
* Docs: `docs/native-messaging.md` (request), `docs/security-model.md`
  (bridge request list and rate classes), `docs/threat-model.md`
  (a compromised extension can raise the window on a matching login).

## 7. Testing

**Protocol:** `open_item` round-trip; unknown fields, oversized URL,
malformed UUID rejected; TS mirror type-checks.

**Bridge:** refused while locked; refused when the item does not match the
URL (github.com item requested from evil.com); refused for an unknown ID;
refused when bridging is disabled; a matching item calls the hook exactly
once; a refused request never calls it; `OpenItem` is in the Secret class.

**Extension:** the popup's DOM has no test harness, so the button's
placement (last in the row, after Fill and Code) is checked manually;
`popup_open_item` parsing and handling are unit-tested — rejected from a
content-script sender, the URL taken from the tab and not the message, and
a malformed `itemId` rejected.

**Desktop:** `decideOpen` unit tests (same item, other item dirty, other
item clean, nothing open); `ItemEditor` dirty detection helper tested as a
pure function.

**Manual:** github.com open → popup → edit icon brings up the desktop
editor; again with the window hidden in the tray; again while editing
another login with unsaved changes.
