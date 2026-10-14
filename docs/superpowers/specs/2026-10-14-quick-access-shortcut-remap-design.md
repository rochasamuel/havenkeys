# Quick Access shortcut remap — design

Date: 2026-10-14. Status: approved in conversation.

Extends `2026-10-13-quick-access-design.md` (A3), which fixed the shortcut
to Cmd/Ctrl+Shift+Space and listed a custom shortcut recorder as out of
scope. When another app owns that chord, Quick Access is unusable today;
this lets the user pick another one.

## 1. Goal

From Settings → Quick Access the user records a new global shortcut. It
replaces the default only if the operating system accepts it; otherwise
nothing changes and the user is told why.

## 2. Decisions taken

| Question | Decision |
|---|---|
| How to choose | Record any combination (press the keys), not a preset list |
| When available | Always (Change… and Reset), and offered from the "in use" warning |
| Who registers | Rust only, as today; the renderer proposes a chord, Rust validates it |
| What a shortcut does | Only toggles the Quick Access window (unchanged) |
| Stored | `quick-access.json`, optional `shortcut_keys`, per device, plaintext |
| Old shortcut while recording | Stays active until a new one is accepted |

## 3. Behaviour

* **Settings row:** under the Quick Access switch, "Shortcut" shows the
  current chord with platform symbols (⌘⇧Space / Ctrl+Shift+Space), and two
  buttons: **Change…** and **Reset** (Reset only when not the default). The
  row is shown while the switch is on.
* **In use:** the existing warning ("The shortcut is in use by another
  app") gains a **Choose another shortcut** button that starts recording.
* **Recording:** the chord field reads "Press the new shortcut (Esc to
  cancel)". Keys are captured by physical key (`KeyboardEvent.code`), so
  the keyboard layout does not matter. Modifier-only presses keep waiting.
  Esc cancels. The first complete chord is sent to Rust.
* **Results:**
  * Accepted → the row shows the new chord; the old one is released.
  * Refused by the rules (§4) → a short reason ("Add Cmd, Ctrl or Alt", "This
    key can't be used", "Reserved by the system"), recording stays open.
  * Taken by another app → "That shortcut is in use by another app",
    recording stays open; the previous shortcut keeps working.
* **Reset:** back to Cmd/Ctrl+Shift+Space through the same command (with
  the same "taken" handling).
* Copy in `en` and `pt-BR`; impeccable review of the row and the recording
  state.

## 4. Rules (Rust)

A chord is accepted only if:

* It has at least one of Cmd (Super), Ctrl or Alt. Shift may be added;
  Shift alone is not enough.
* The key is a letter A–Z, digit 0–9, Space, F1–F12, or one of
  `` ` - = [ ] \ ; ' , . / ``.
* It is not on a fixed denylist of system and editing chords: Cmd/Ctrl +
  C, V, X, A, Z, Y, Q, W, Tab; Cmd+Space; Cmd+Shift+3/4/5; Alt+Tab; Alt+F4;
  Ctrl+Alt+Delete; Cmd+Ctrl+Q.
* The wire string is at most 64 bytes and parses as the plugin's
  `Shortcut` form (e.g. `"Alt+Shift+Space"`, `"Super+Shift+K"`).

`CmdOrCtrl` is not used: the renderer sends the concrete modifiers it saw
on this platform.

## 5. Rust changes

* `QuickAccessSetting` gains `#[serde(default)] shortcut_keys:
  Option<String>`. Load: missing, unparsable or rule-breaking → default
  chord. Unknown fields ignored (as today).
* `QuickAccess` keeps the current `Shortcut` in its registration state; the
  plugin handler compares the pressed shortcut with it (not with a fixed
  function).
* `ShortcutHost::register/unregister` take the `Shortcut`.
* New command `set_quick_access_shortcut(keys: Option<String>) ->
  QuickAccessStatus` (`None` = default). Main capability only
  (`capabilities/main.json`); needs the vault unlocked, like
  `set_quick_access`. Async (the plugin call waits on the main thread).
  Under the existing `applying` lock:
  1. Parse and check the rules; refused → error code
     `quick_access_shortcut_invalid` with a reason (`modifier`, `key`,
     `reserved`).
  2. Same as the current chord → return the status unchanged.
  3. If the switch is on: register the new chord while the old one is still
     registered. Taken → return the status with `candidateInUse: true` and
     change nothing.
  4. Unregister the old chord (if registered), save the file, swap the
     state. If saving fails, unregister the new chord, re-register the old
     one, and return the `quick_access_settings` error.
  5. If the switch is off: only validate and save.
* `QuickAccessStatus` gains `keys: String` (current chord, plugin form) and
  `isDefault: bool`, and `candidateInUse: bool` (false except in step 3).

## 6. Security

* The renderer still cannot register, unregister or query arbitrary global
  shortcuts: no `global-shortcut:` permission is granted; the only path is
  the validated command above, and the only action bound is "toggle Quick
  Access".
* The command is in `main.json` only; `quick.json` is unchanged.
* `quick-access.json` stays non-secret plaintext metadata (security-model
  §4). A tampered value is checked by the same rules on load.
* Security review: update QA1/QA4; new QA5 (Info): the main renderer can
  choose which allowed chord opens Quick Access; worst case it picks a
  chord that shadows a user's app shortcut while unlocked, which the user
  sees in Settings and can reset.

## 7. Tests

Rust: rules table (accepted and refused, each reason); load falls back to
the default for missing/garbage/denylisted values; switching registers new
before releasing old; taken → nothing changes; save failure rolls back;
switch off → only saves; handler matches the current chord; capability
test still pins `quick.json` and checks the new command is in `main.json`
only.

vitest: recording starts and cancels (Esc), modifier-only waits, a chord is
sent once, refusal reasons and "taken" keep recording open, accepted chord
shows with platform symbols, Reset shown only when not default.

## 8. Docs

`docs/security-model.md` (§4 file contents, §6/§7 command list),
`docs/security-review.md` (QA1, QA4, QA5), `docs/architecture.md` if the
Quick Access section names the fixed chord. Roadmap note on A3
(uncommitted). No `CLAUDE.md` amendment.
