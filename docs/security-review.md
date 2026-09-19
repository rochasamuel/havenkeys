# Security Review: Desktop Phase

**Date:** 2026-09-19
**Scope:** `crates/havenkeys-core`, `apps/desktop/src-tauri`, `apps/desktop/src`
**Method:** self-review plus an independent review pass by a second reviewer
who was given the code, not the author's conclusions. Findings were verified
against the code; fixes are covered by tests where the core is involved.

> This is an internal review, not an independent security audit.

## Summary

No critical or high-severity issues were found. One medium issue (auto-lock
could be defeated by TOTP polling) and five low issues were fixed. The rest
are accepted limitations, documented below and in `threat-model.md`.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| 1 | Medium | Tauri / UI | TOTP refresh reset the idle timer, so auto-lock never fired while a TOTP item was open | **Fixed** |
| 2 | Low | Tauri | Race: auto-lock tick between unlock and timer re-arm could lock immediately | **Fixed** |
| 3 | Low | Tauri / core | Master-password change held the vault mutex through two Argon2id runs, delaying lock requests | **Fixed** |
| 4 | Low | Core | Failed SQLite delete still removed the item from the in-memory list | **Fixed** |
| 5 | Low | Tauri | Poisoned vault mutex silently disabled auto-lock | **Fixed** |
| 6 | Info | Core | Header could demand 4 GiB / t=64 Argon2id (local DoS) | **Fixed** (ceiling 1 GiB, t=16) |
| 7 | Low | UI | Pointer movement over an unfocused window counted as activity | **Fixed** |
| 8 | Low | Core | Master-password change does not rotate the vault key | Accepted, documented |
| 9 | Info | Core | Rollback/replay of whole file or individual blobs is not detected | Accepted, documented |
| 10 | Low | Tauri | Lock on OS screen-lock not implemented; suspend detection likely ineffective on Windows | Open |
| 11 | Low | All | Secrets can't be reliably wiped from WebView/IPC/serde buffers; no `mlock` | Accepted, documented |
| 12 | Low | Tauri | Unlock attempts are not rate-limited beyond Argon2id cost | Accepted |
| 13 | Info | Deps | 7 unmaintained/unsound advisories through Tauri's GTK stack | Accepted, tracked in `deny.toml` |
| 14 | Info | Tauri | CSP, capabilities and tray behaviour need a manual runtime check on the real app | Open |
| 15 | Low | Import | The plaintext `.1pux` export stays on disk unless the user deletes it; deletion is not a secure wipe | Accepted, warned in UI, documented |
| 16 | Info | Import | Imported plaintext passes through `serde_json::Value`; wiped after conversion, but intermediate copies may remain | Accepted, documented |

## Details

### 1. TOTP polling kept the vault unlocked (Medium, fixed)
**Attack scenario:** the user opens a login with TOTP and walks away. The UI
fetches a new code every 30 s, and `get_totp_code` reset the idle timer, so a
5-minute auto-lock never fired.
**Mitigation:** `get_totp_code` no longer records activity. Activity comes only
from `record_activity` (real input while the window has focus) and from
user-initiated commands.

### 2. Stale auto-lock tick after unlock (Low, fixed)
The timer was re-armed after the vault mutex was released. A tick in that gap
compared against the previous session's timestamps. The timer is now armed
while the vault lock is held, which matches the documented lock order.

### 3. Lock delayed by master-password change (Low, fixed)
The core now splits the flow: `begin_rekey` (under the lock),
`RekeyTicket::derive` (Argon2id, outside the lock) and `commit_rekey` (under
the lock). The commit is refused if the vault was locked in the meantime (epoch
check) or the header changed. Tests: `rekey_is_discarded_if_vault_locked_meanwhile`,
`rekey_refused_if_header_changed_meanwhile`.

### 4. Delete ordering (Low, fixed)
The row is now deleted from disk before it is removed from the in-memory cache.

### 5. Poisoned mutex (Low, fixed)
`auto_lock_tick` now recovers the guard from a poisoned mutex, as `lock()`
already did. Release builds use `panic = "abort"`, so poisoning only matters in
development builds.

### 6. KDF ceiling (Info, fixed)
Someone who can write to the vault file could make every unlock attempt
allocate 4 GiB. The header ceiling is now 1 GiB of memory and 16 iterations.

### 8. Password change does not rotate the vault key (Low, accepted)
**Attack scenario:** an attacker copied `vault.sqlite3` in the past and later
learns the old master password. They unwrap the vault key from the old copy,
and that key still decrypts current copies of the vault.
**Why accepted:** rotating the key requires re-encrypting every item. That is
feasible, but it's a separate feature. **Remaining limitation:** if you
suspect both your vault file *and* your old master password were exposed,
changing the password is not enough. Create a new vault and move your items
into it (a "rotate vault key" action is planned).

### 9. Rollback and replay (Info, accepted)
Each blob is bound to its vault, item and role, but not to a revision. An
attacker with write access can restore an older version of the whole file, or
an older blob for the same item (for example an old password). Detecting this
would need a monotonic counter stored outside the file (an OS keychain, for
instance). Out of scope for the MVP; listed in `threat-model.md` §4.

### 10. OS lock events (Low, open)
The vault locks on idle timeout, on exit, and on suspend. Suspend is detected
by comparing the wall clock with the monotonic clock. On Windows, `Instant`
keeps counting during sleep, so this heuristic likely never fires there. The
idle timeout still runs, unless it is set to "Never". Screen-lock events
(logind `Lock`, `WTS_SESSION_LOCK`, macOS `com.apple.screenIsLocked`) are not
handled on any platform. **Planned:** per-platform session-lock listeners in the
Tauri shell.

### 11. Memory (Low, accepted)
Keys and decrypted buffers in Rust are zeroized on drop. Copies remain that we
don't control: JSON IPC buffers, JavaScript strings in the WebView, serde
intermediates, stack copies of fixed arrays, and pages swapped to disk. The UI
limits how long secrets stay in React state. Revealed passwords hide again
after 30 s, and all state is discarded on lock.

### 12. Unlock rate limiting (Low, accepted)
Local online guessing through the UI costs one Argon2id run (~0.3–1 s) per
attempt. An attacker with that much access can copy the file and guess
offline instead, so a UI rate limit adds little.

### 13. Dependency advisories (Info, accepted)
`cargo audit` reports **no vulnerabilities**. There are 7 warnings:
`proc-macro-error` and 5 `unic-*` crates are unmaintained, and `glib`
0.18 has an unsoundness in `VariantStrIter`, an API HavenKeys does not use. All
come through Tauri's Linux stack; none are in `havenkeys-core`. `pnpm audit`
reports no known vulnerabilities. `cargo deny check` passes.

### 14. Runtime verification pending (Info, open)
The desktop binary now builds and links against WebKitGTK/AppIndicator, and the
app has been launched under WSLg. Automated UI checks run in Chromium against
Tauri's IPC mock, so these still need a manual check on the real app:
* the production CSP loads the app without violations,
* a command not in the capability is rejected,
* navigating to an external URL is blocked,
* the clipboard clears after the timeout,
* closing the window hides to the tray and the idle timer still locks the vault,
* *Quit* in the tray menu locks and exits.

### 15–16. 1Password import (added after the initial review)
The importer treats the export as hostile input. Size caps are applied to the
actual decompressed bytes, not the declared ones. The UI never supplies a file
path, and every item goes through normal validation. Tests cover garbage and
malformed archives, odd JSON shapes, and error messages that don't echo input.
The main residual risk is outside the app: the user's plaintext export file.
The UI warns about it and offers deletion; `security-model.md` §10 explains why
that is not a secure wipe.

## Verified properties

* Tampering with any byte of a blob, or swapping blobs between items, roles
  or vaults, fails authentication (`blob.rs`, `tests/security.rs`).
* A locked vault refuses every item, secret, settings and rekey operation.
* No plaintext title, username, URL, password, TOTP secret or note appears in
  the database file.
* Error messages never echo input; `Debug` output of secret-bearing types is
  redacted.
* The generator is unbiased (rand 0.10 `Uniform::sample`, Lemire's method
  with rejection, verified in source), and TOTP matches the RFC 6238 vectors.
* Lock ordering (vault → lock manager; clipboard independent) has no
  reverse acquisition.
* The capability grants exactly the 18 app commands plus event listen and
  unlisten. The production CSP has no `unsafe-*` sources.

---

# Security Review: Native Messaging Phase (Phase 4)

**Date:** 2026-09-19
**Scope:** `crates/havenkeys-protocol`, `crates/havenkeys-bridge`,
`crates/havenkeys-native-host`, the bridge wiring in `apps/desktop/src-tauri`,
`packages/protocol`, `apps/extension`, `scripts/install-native-host.*`
**Method:** self-review, then an independent review by a second reviewer who
was given the code but not the author's conclusions. Findings were checked
against the code. Fixes are covered by tests where the code runs on this
platform. Windows paths were checked by reading and cross-compiling only.

> This is an internal review, not an independent security audit.

## Summary

No critical or high-severity issues were found. No path was found that
returns a secret for an item on a page it is not saved for, while the vault
is locked, or while integration is off. One medium issue (Windows pipe DACL)
and three low issues were fixed.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| P1 | Medium | Bridge (Windows) | Default named-pipe DACL let other users open read handles, take all connection slots and watch lock/unlock events | **Fixed** (owner-only DACL); unverified on Windows |
| P2 | Low | Protocol (Windows) | Pipe name from a filtered `USERNAME` could be identical for different users (non-ASCII names) | **Fixed** (hash of profile path) |
| P3 | Low | Bridge | Idle or half-sent connections held slots forever (same-user DoS) | **Fixed** on Unix (10 min idle timeout); open on Windows |
| P4 | Low | Protocol | `Debug` of fill results printed usernames | **Fixed** |
| P5 | Info | Protocol / host | Secret copies from growing serialization buffers and from untagged deserialization | **Fixed** (pre-sized zeroizing buffer, keyed parsing); residual copies documented |
| P6 | Info | Core settings | Browser integration defaulted to on, including for upgraded vaults | **Changed** to opt-in |
| P7 | Info | Desktop | `unlocked` event could arrive after a concurrent `locked` | **Fixed** (sent under the vault lock) |
| P8 | Info | Scripts | Control characters in the host path produced an invalid manifest | **Fixed** |
| P9 | Medium | Architecture | Any same-user process can use the bridge like the extension | Accepted, documented (`native-messaging.md` §8) |
| P10 | Low | Protocol (Windows) | Another user can squat the pipe name before HavenKeys starts | Open, documented |
| P11 | Info | Extension | Unpacked-extension ID is pinned by a public key, which anyone can reuse | Accepted, documented |

## Details

### P1. Windows pipe readable by other users (Medium, fixed)
**Attack scenario:** another user on the same machine opens eight read-only
handles to the victim's pipe. The default named-pipe DACL grants Everyone
read access, and the server accepted any connection, because the Windows
peer check was a stub. Each connection held a slot forever, which disabled
the victim's browser integration, and each one received the victim's
lock/unlock events.
**Mitigation:** the listener is created with the protected DACL
`D:P(A;;GA;;;OW)` (owner only). **Remaining:** not tested on Windows. If
HavenKeys runs elevated, the owner is the Administrators group, and a
non-elevated host is refused. That fails closed.

### P3. Connection slots (Low, fixed on Unix)
Each connection now has a 10-minute receive timeout. That covers idle
connections and frames stalled mid-transfer. The extension closes its port
after 60 s idle, so real hosts never hit the timeout. Named pipes have no
receive timeout in `interprocess`, so on Windows a same-user process can
still hold every slot. That is denial of service by a process that is out of
scope anyway.

### P6. Opt-in integration (Info, changed)
While browser integration is on, any process running as the user can make
the extension's requests (P9). It is now off by default for new vaults and
for vaults created before the setting existed. The user turns it on in
Settings → Browser extension, which explains the trade-off.

### P9. Same-user callers (Medium, accepted)
A process running as the user can connect to the socket, or start the native
host itself with the right argument. It can then enumerate matches for URLs
it guesses, and fetch passwords for them at up to about 30 per minute after
a burst of 10. This is inside the "malware as the same user" class that
`threat-model.md` §4 excludes, but it is cheaper than reading process memory.
Candidate mitigations for later: verify the peer executable's code signature,
require a one-time pairing approval in the desktop UI, or confirm each fill
on the desktop.

## Verified properties (this phase)

* Every page request re-checks, in Rust: exact protocol shape, size limits,
  rate limits, unlocked vault, integration switch, and the item's own URL
  rules against the page (`crates/havenkeys-bridge/tests/bridge.rs`).
* Unknown item IDs, secure notes, other sites' logins and logins without
  TOTP all return the same `denied`.
* `find_matches` carries no password, TOTP secret or notes. `fill_item`
  carries only the username and password. `get_totp` carries only the code.
* Frames are rejected by their length header before any allocation, at both
  hops. The host stays in sync after skipping an oversized frame.
* 50 000 fuzzed and mutated inputs never panic the request parser.
* The native host drops malformed desktop messages and replaces error text
  with fixed strings (tested against a hostile fake desktop).
* A stalled peer cannot block locking: events go out through `try_send` to
  bounded per-connection queues, and `on_lock` runs with no bridge lock held.
* The extension accepts messages only from its own popup page, has an empty
  `externally_connectable`, stores nothing, builds its DOM without
  `innerHTML`, and runs under a CSP with no `unsafe-*` sources. Permissions
  are `nativeMessaging` and `activeTab` only. (Phase 5 extends both; see
  below.)
* Tested end to end with the real desktop binary and the real native host on
  Linux. Not yet tested inside a real browser (none is installed in this
  environment).

---

# Security Review: Autofill Phase (Phase 5)

> This software has not undergone an independent security audit. This is a
> self-review of the Phase 5 changes: field detection, in-page suggestions,
> save-login, password generation from the extension, TOTP fill, and the new
> bridge requests (`generate_password`, `check_login`, `save_login`, `topUrl`).

Scope: `apps/extension/src/{autofill,content,menu,options,background}`,
`crates/havenkeys-core` (`check_login`, `save_login`, frame binding, password
history), `crates/havenkeys-protocol`, `crates/havenkeys-bridge`, and the
desktop's history commands.

## Summary

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| F1 | Medium | Extension (save-login) | A page could plant a password, forge a `submit`, and watch for a save prompt to learn whether it guessed the saved password (same-origin oracle, e.g. after XSS) | **Fixed**: only passwords the user typed (trusted `input`) or we generated, still unchanged at submit, are reported |
| F2 | Medium | Bridge / core | A compromised extension could replace a login's password repeatedly and push the real one out of history, or overwrite without any record | **Fixed**: replaced passwords go to an encrypted history (5 entries); one browser-initiated change per item per 10 min. Residual: a patient attacker can still cycle history in about an hour |
| F3 | Medium | Extension (menu) | Clickjacking: the page controls the menu's `<iframe>` element | **Mitigated**: inline `!important` styles, tamper observer, 400 ms arming delay, IntersectionObserver v2 visibility on Chromium. Weaker on Firefox (delay only). Accepted, documented |
| F4 | Low | Extension (fill) | A frame could navigate between the user's pick and the fill, and receive another origin's credentials | **Fixed by design**: fills are addressed to the frame (and, on Chromium, its `documentId`), and the content script refuses unless its origin equals the matched one |
| F5 | Low | Core / extension | A login iframe of site A embedded in site B was matched on the frame URL alone | **Fixed**: `topUrl` is sent for iframes, and the core requires the item to match both |
| F6 | Low | Architecture | The extension can now write to the vault (`save_login`) | Accepted: writes are origin-bound, rate-limited, and never destroy a password (history) |
| F7 | Low | Extension | A generated password could be lost if the submit was not detected | **Mitigated**: offered on page unload if still in the field. Residual for flows that neither submit nor unload |
| F8 | Info | Extension | Pages can see that a menu frame appeared and its height (1–5 logins, or locked). On Chromium, the fixed extension ID lets any page detect the installation through `web_accessible_resources` | Accepted, documented |
| F9 | Info | Extension (CSP) | `frame-ancestors 'none'` removed so web pages can frame the menu and save pages | Accepted: `web_accessible_resources` limits framing to those two pages |
| F10 | Info | Extension | A pending save holds the submitted password in worker memory (JS strings cannot be wiped) | Accepted: at most 3 min, dropped on confirm, dismiss and lock |
| F11 | Info | Core | "Exact page" rules compare paths, which a page can change within its own origin (`pushState`) | Accepted: same-origin only |
| F12 | Info | Extension | Heuristics can misclassify fields (wrong menu, missed form) | Accepted: cannot cross origins; the security checks do not depend on classification |
| F13 | Info | All | Not yet exercised in a real browser | Open: see "Verification pending" |

## Details

### F1. Save prompt as a password oracle (Medium, fixed)

The first version captured any submission whose password field was not
filled from the vault. A script on the page, such as an XSS on the real
site, could set the field to a guess and dispatch `submit`. The desktop's
`check_login` answered `unchanged` for a correct guess, and no prompt
appeared. The prompt's presence is visible to the page, so it leaked one bit
per guess at up to 30 guesses a minute.

Now the content script records the source of each field's value together
with the value itself: `user` on trusted `input` events, `generated` or
`vault` on our fills. `readSubmission` reports a password only if its source
is `user` or `generated` **and** the field still holds exactly that value. A
value planted, or swapped after the user typed, has no matching record.
Tests: `autofill.test.ts` ("ignores passwords a page script planted or
swapped"), `content.test.ts` ("no save-prompt oracle").

### F2. Destructive writes (Medium, fixed; residual accepted)

`save_login` with an item ID replaces a password. Everything that can use
the bridge could do this: a compromised extension, or same-user malware (P9).
Now:

* `build_item` moves every replaced or cleared password into the login's
  `password_history`. That covers edits from the desktop too. The history
  lives inside the encrypted details blob, is capped at 5 entries, and is
  shown in the desktop app with a per-entry reveal. Old vaults read as empty
  history (`serde(default)`).
* The bridge allows one browser-initiated password change per item every 10
  minutes (`RateLimiter::allow_item_update`), on top of the secret-request
  bucket.
* Only the password changes. The title, username, URLs, TOTP and notes are
  kept.

Residual: 5 changes spaced 10 minutes apart still flush the history. A
desktop-side confirmation for browser-initiated updates would close this. It
is deferred, and listed in `native-messaging.md` §8.

### F3. Clickjacking (Medium, mitigated)

The menu and save prompt are extension-origin iframes. The page cannot read
them or forge clicks inside them, but it controls the `<iframe>` element.
Mitigations are in `content/frames.ts` and `menu/common.ts`. On Chromium,
IntersectionObserver v2 (`trackVisibility`) makes the menu refuse clicks
while any part of it is covered, transformed or translucent. Firefox lacks
v2, so only the 400 ms delay applies. In every case the item filled must
match the page in Rust. A successful clickjack can only put this site's own
login into this site's form.

## Verified properties (this phase)

* Page URLs sent to the desktop come from the browser's sender and tab data.
  For iframes, the top page must be readable or the frame is ignored, and
  the core requires a match on both (`a1_frame_*` tests).
* Menus open only on trusted user events. Synthetic pointer, key and focus
  events do nothing (`content.test.ts`).
* Menu picks are single-use, same-tab, limited to items the menu offered,
  and expire. Locking drops every session and pending save
  (`inline-handler.test.ts`).
* Fills reach one frame and only on the matched origin. Messages to the
  content script are accepted only from the background worker, and with
  exact shapes.
* Only visible, enabled, non-read-only fields in the chosen group are
  filled. Hidden honeypot fields are never touched. Secrets never appear in
  attributes or markup (`autofill.test.ts` checks the serialized DOM).
* Classification of a field on a page with 5,000 inputs examines at most 60,
  and the page is never scanned up front (attack 7).
* `check_login` returns no passwords. `find_matches` still carries no
  secrets. The menu and save pages never receive a password or code.
* Generated passwords come from the Rust generator (OS CSPRNG, rejection
  sampling), never from JavaScript.
* Extension sources contain no `innerHTML`, `console`, `eval`, browser
  storage, or value/data attributes (`hygiene.test.ts`).
* `pnpm audit` reports no known vulnerabilities after adding `jsdom` (test
  only). `cargo deny` passes, and `cargo audit` shows only the previously
  accepted GTK-stack warnings.

## Verification pending

No browser is available in the environment this was built in. Before relying
on Phase 5, check by hand in Chrome and Firefox:

1. The popup's **Fill** and **Code → fill** work on a real login page.
2. The options page toggle grants and removes host access, and new tabs get
   suggestions. On Firefox, check that `optional_host_permissions` and
   `scripting.registerContentScripts` behave as on Chrome.
3. The menu frame loads inside pages with strict CSPs (for example
   github.com). Browsers exempt extension frames from page CSP, but confirm.
4. Save and update prompts appear after a real login and a password change,
   and the desktop list refreshes.
5. On Chromium, a page that covers the menu (for example a translucent
   overlay with `pointer-events: none`) cannot get clicks through.
