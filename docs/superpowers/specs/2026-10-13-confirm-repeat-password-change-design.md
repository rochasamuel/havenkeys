# Desktop confirmation for repeat browser password changes — design

Date: 2026-10-13. Status: approved in conversation.

Implements item **E2** of `docs/autofill-roadmap.md` and closes the residual
of finding **F2** in `docs/security-review.md` ("a patient attacker can still
cycle history in about an hour").

## 1. Goal

Anything that can use the browser bridge — a compromised extension, or
malware running as the user — can call `save_login` with an item ID and
replace that login's password. Replaced passwords go to a 5-entry encrypted
history, and the bridge allows one browser-initiated change per login every
10 minutes, so five spaced changes flush the real password out of history.

After this change, a **repeat** change needs an explicit Allow in the
desktop app, so cycling the history needs five clicks by the user.

## 2. Decisions taken

| Question | Decision |
|---|---|
| When to ask | A browser `save_login` that replaces an existing login's password, when that login's password already changed in the last 24 h |
| First change in 24 h | Goes through as today (the extension's save card is the consent) |
| Where "last change" comes from | The login's `password_history` (`replaced_at`): persistent, synced, survives restarts |
| Desktop edits | Count (a desktop change followed by a browser update within 24 h asks once) |
| Where the user answers | A dialog in the desktop main window, brought to the front |
| Timeout | 60 s, then deny |
| Pending prompts | At most one; a second confirmation request gets `busy` |
| 10-minute limit | Kept; an update the user allowed on the desktop is not refused by it |
| Out of scope | New logins, cards, "Sign in with" saves, Android saves (not via the bridge) |

## 3. Core

`VaultService::update_needs_confirmation(id: &Uuid, now_ms: i64) -> Result<bool>`:
true when the login's newest `password_history` entry has
`now_ms - replaced_at < 24 h` (a clock that went backwards counts as recent).
`NotFound` / `Locked` as usual. No secret leaves; only the timestamps are
read.

A confirmation summary for the dialog, without secrets:
`ConfirmUpdate { item_id, title, username: Option<String>, site: String }`
where `site` is the page's registrable domain (or host).

## 4. Bridge

In the `SaveLogin` arm, when `item_id` is present (an update):

1. Authorize exactly as today (`stage_save_login`'s checks, frozen gate,
   rate limits).
2. Ask the core `update_needs_confirmation`. If false: continue as today.
3. If true: build `ConfirmUpdate`, **release the vault guard**, and call the
   confirmation hook installed by the desktop
   (`Bridge::set_confirm_update_hook(Fn(ConfirmUpdate) -> bool)`), with a
   60 s deadline. Only one confirmation may be pending; while one is,
   another request needing confirmation returns `busy`.
4. Deny (`denied`) on: the user's Deny, the deadline, the vault locking
   meanwhile, no hook installed, or the hook failing.
5. On Allow: take the vault guard again and stage the save from scratch
   (re-authorize: the item, origin and lock state may have changed). The
   10-minute item limit does not refuse an allowed update.
6. Logging: none of title, username or site is logged.

## 5. Desktop

* The hook shows the main window (from the tray if needed), focuses it, and
  opens a modal dialog:
  * Title: "Change this password?"
  * Body: "The browser wants to change the password of "GitHub" (octo) on
    github.com. It was already changed in the last 24 hours."
  * Buttons: "Allow" and "Deny" (Deny is the default/escape action).
* The dialog never shows a password. It closes itself on lock, on timeout
  and when the window closes.
* Implementation: the hook sends a Tauri event with the summary and waits on
  a one-shot channel; a new command `answer_update_confirmation(id, allow)`
  answers it. The command accepts only the ID of the pending confirmation.
* Copy in `en` and `pt-BR`; impeccable review of the dialog.

## 6. Extension

* `save_login` requests use a 75 s timeout (others keep 10 s).
* While a save is in flight for more than 2 s, the save card shows "Confirm
  in HavenKeys…". `denied` or a timeout shows "Not saved"; `busy` shows "Not
  saved — HavenKeys is waiting for another confirmation".
* Version skew: an older extension keeps its 10 s timeout, so an update that
  needs confirmation fails unless answered quickly; nothing else changes. An
  older desktop never asks.

## 7. Tests

Core: needs confirmation only with a history entry younger than 24 h; first
change passes; a desktop edit counts; an entry synced from another device
counts; clock skew counts as recent; locked → `Locked`.

Bridge (with a fake hook): allow saves; deny, timeout, lock-while-waiting and
no-hook all deny; the vault guard is not held while the hook runs (a lock
request completes during the wait); re-authorization after Allow (item
deleted meanwhile → `not_found`); second pending → `busy`; an allowed update
is not refused by the 10-minute limit; new-login saves never ask.

Desktop: the command answers only the pending ID; the dialog renders no
secret; Allow/Deny wiring (vitest).

Extension: timeout value for `save_login`; waiting state after 2 s; `denied`,
`busy` and timeout states.

## 8. Docs

`docs/security-review.md` (F2 residual → closed, with this spec),
`docs/native-messaging.md` §8, `docs/threat-model.md` (compromised extension
section), `docs/autofill.md` (save flow). Roadmap marks E2 done
(uncommitted). No `CLAUDE.md` amendment: this only adds a confirmation.
