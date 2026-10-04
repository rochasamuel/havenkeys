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
| 10 | Low | Tauri | Lock on OS screen-lock not implemented; suspend detection likely ineffective on Windows | **Fixed** in Phase 6 (Windows, Linux/logind); macOS open |
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

### 10. OS lock events (Low, fixed in Phase 6 on Windows and Linux)
The vault locks on idle timeout, on exit, and on suspend. Suspend is detected
by comparing the wall clock with the monotonic clock. On Windows, `Instant`
keeps counting during sleep, so this heuristic likely never fires there. The
idle timeout still runs, unless it is set to "Never". Screen-lock events
(logind `Lock`, `WTS_SESSION_LOCK`, macOS `com.apple.screenIsLocked`) are not
handled on any platform.

**Phase 6:** the vault now locks when the session locks, polled every 5 s
through `crates/havenkeys-oslock`. On Windows it reads the WTS session lock
flag, and on Linux logind's `LockedHint`. macOS remains open.

### 11. Memory (Low, accepted)
Keys and decrypted buffers in Rust are zeroized on drop. Copies remain that we
don't control: JSON IPC buffers, JavaScript strings in the WebView, serde
intermediates, stack copies of fixed arrays, and pages swapped to disk. The UI
limits how long secrets stay in React state: a revealed password hides again
after 30 s and revealed login notes after 120 s. A secure note's body is the
exception — it is decrypted when the note is opened and stays in renderer
state until you navigate away, because the note *is* the view. On lock, the
whole vault tree is unmounted (`key={session}`), so every revealed secret in
component state goes with it.

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
* The capability grants exactly the 31 app commands plus event listen and
  unlisten, and `src/lib/commands.test.ts` fails if `build.rs`, the
  `generate_handler!` list and the capability file ever disagree. The
  production CSP has no `unsafe-*` sources.

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
| P10 | Low | Protocol (Windows) | Another user can squat the pipe name before HavenKeys starts | Fixed in code (see BR-1), not yet verified on Windows |
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

---

# Security Review: Hardening (Phase 6, in progress)

> This software has not undergone an independent security audit.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| H1 | Low | Tauri | No lock on OS screen lock (#10) | **Fixed** on Windows (WTS) and Linux (logind `LockedHint`); macOS open |
| H2 | Info | Tests | Only the protocol parser was fuzzed | **Fixed**: blobs, TOTP, URLs and domain matching (including generated look-alike hosts), item input, `.1pux` import, and the TS validators and classifier are fuzzed in every test run (`development.md`, Fuzzing) |
| H3 | Info | Extension | Lock events reach the extension only while its native port is open (it closes after 60 s idle). A pending save prompt can outlive a lock until its 3-minute expiry. The desktop refuses the save while locked regardless | Accepted |
| H4 | Info | Audit | Error-message and logging audit: errors reaching UIs are fixed strings; no print macros outside the two developer `examples/`; the only non-test panic is the startup `expect`; no `console` in any UI | No action needed |
| H5 | Info | Tauri | CSP and capability audit: production CSP has no `unsafe-*`, `freezePrototype` is on, one capability with the command allowlist, and the dialog plugin is Rust-only (no renderer permissions) | No action needed; runtime check (#14) still pending |

Found while fuzzing: nothing in the product code. The look-alike generator
first flagged `evil.comlogin.microsoftonline.com` against a
`login.microsoftonline.com` whole-site rule. That is a real subdomain of
`microsoftonline.com`, so the match is correct under the documented rules,
and the fault was in the generator. It is a reminder that whole-site rules
(the default) cover every subdomain of the registrable domain, and that
"This exact site" exists for sites where that is too broad.

Windows pipe owner check (P10) is fixed in code by BR-1 (unverified on Windows). Still open for Phase 6: desktop confirmation
for browser-initiated password changes (F2 residual), macOS screen lock,
export, and the manual runtime checks (#14, Phase 5 browser checklist).

---

# Security Review: Secret Key and folder sync

> This software has not undergone an independent security audit.

Scope: `crates/havenkeys-core/src/{crypto/secret_key.rs, crypto/keys.rs,
vault.rs, store.rs, sync.rs}`, and `apps/desktop/src-tauri/src/{device.rs,
account.rs, sync.rs}` with its UI. Design: `server-sync.md` (then `sync.md`), `crypto.md`.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| K1 | Info | Design | The Secret Key is stored in plain text in `device.json` on each device | Superseded by the OS keychain (S17–S20 below); `device.json` remains the fallback when no keychain is available. Same model as 1Password either way: it protects copies away from your devices, not a device someone can already read |
| K2 | Info | Design | Losing every device that holds the Secret Key, and the Emergency Kit, loses the vault | Accepted; the kit is shown at creation, confirming it was saved is required, and it can be shown again while unlocked |
| K3 | Low | Sync | Conflicts are decided by device clocks (newest `updatedAt` wins) | Accepted, documented (superseded by `server-sync.md`; the server now orders writes) |
| K4 | Low | Sync | Anyone with access to the folder can delete files and stop updates | Accepted (denial of service only), documented |
| K5 | Info | Sync | The folder reveals the vault ID, KDF parameters, wrapped key, number of devices, snapshot sizes and sync times | Accepted, documented |
| K6 | Info | Sync | Any unlocked device can change any item or the master password for all devices | Inherent: every device holds the vault key. Same-user malware is out of scope |
| K7 | Low | Core | A header from the folder is plaintext and read before any key exists | Mitigated: a new device requires the attestation to verify under the key it derived; an unlocked device adopts only attested, newer, key-scheme-2 headers. Forged headers are replaced (tested) |
| K8 | Info | Core | Deleted items leave tombstones (random ID and time) in plaintext in the local database | Accepted, listed in `security-model.md` §4 |
| K9 | Info | Desktop | The sync worker reads and writes the folder while other code runs | By design: no lock is held during I/O, and the merge re-checks that the vault is still unlocked |

Tests: `crates/havenkeys-core/tests/sync.rs` (joining needs both secrets;
nothing in the folder is plaintext; edits, deletes and resurrection;
concurrent edits; replayed snapshots; tampered, foreign and garbage files;
forged header; password change spreading; key-scheme and lock checks),
`crypto/secret_key.rs` (format, typos, redaction), and `store.rs`
(schema 1 → 2 migration).

Verification pending: none of this has run in the desktop app on a real
cloud folder yet. Before relying on it, create a vault, save the kit, set a
OneDrive folder, and join from a second computer (`roadmap.md` §1).

---

# Security Review: Server, Sync Client and the Account Desktop

**Date:** 2026-09-20
**Scope:** `crates/havenkeys-server`, `crates/havenkeys-sync-client`, the
changes to `crates/havenkeys-core` (`stage_save_login`, `stage_import`,
`reset_sync_cursor`, `derive_session_for_account`), `crates/havenkeys-bridge`
(the writer hook), and `apps/desktop/src-tauri/src/{account,sync,import}.rs`
with its UI. Design: `docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`.
**Method:** self-review of the implementation against the design, plus the
test suites written alongside it (`scripts/test-server.sh` runs the server
and client suites against a real Postgres).

> This is an internal review, not an independent security audit. The server
> has not been deployed, and nothing here has been reviewed by anyone else.

## Summary

One medium issue was found and fixed (an unauthenticated route doing Argon2id
work before it checked anything), along with two low ones. The largest risks
in this change are not bugs but properties of the design: the server can
destroy data, and availability now decides whether the vault can be changed
at all.

**Update, 2026-09-24** (`docs/superpowers/specs/2026-09-23-vault-account-fixes-design.md`):
S14 and S15 are new findings from that design's implementation, S12 is
resolved by the same fix as S14, and S16–S25 record the Secret Key keychain
and "Remove this device" limitations it introduced; S26–S28 come from the
final whole-branch review. Numbering continues in
this table rather than starting a new one, since this work extends the same
server/desktop account surface reviewed below.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| S1 | Medium | Server | `POST /v1/accounts/activate` hashed the auth key with Argon2id before validating the invite, so a stranger could buy ~20 ms of CPU per request | **Fixed** (invite checked first; per-address rate limit) |
| S2 | Low | Desktop | Activation wrote the Secret Key *after* the server accepted it; a crash in between left an activated account whose only Secret Key was gone | **Fixed** (written first; sign-in can use the stored key) |
| S3 | Low | Sync client | The session carried a nil account id, inviting later code to ask the server which account it is signed in to | **Fixed** (the caller's id is carried) |
| S4 | Low | Server | A page could cut a revision in half, handing out a cursor past rows the client never received | **Fixed** before merge (caught by its own test) |
| S5 | High (design) | Server | A hostile or failing server can delete a vault, and every device applies it | Accepted, inherent; mitigated by backups (`deployment.md` §5) and `resync_vault` |
| S6 | Medium (design) | All | Availability is a correctness concern: a server that is down means nothing can be saved, rotated or deleted | Accepted, documented |
| S7 | Low | Server | `auth/params` tells a persistent prober *when* an address stops being unknown (decoy params become real ones at activation) | Accepted |
| S8 | Low | Server | Per-address rate limiting can be used to lock out a shared address; per-account limiting to lock out a known account | Accepted |
| S9 | Low | Client | Pulled items are bounded by size but not re-checked against the UI's field limits | Accepted, documented (`server-sync.md` §6) |
| S10 | Low | Server | Every authenticated request updates `devices.last_seen_at`, giving the server per-request activity | Accepted |
| S11 | Info | Server | TLS is the deployment's responsibility; the server does not terminate it or set HSTS | Accepted, documented (`deployment.md` §1) |
| S12 | Low | Desktop | A master-password change is local until the next successful sync publishes the header | **Resolved**: the header now changes server-first, in the same transaction as the verifier (S14) |
| S13 | Info | Bridge | A save from the extension blocks one bridge thread for the round trip (no vault lock held) | Accepted |
| S14 | High | Server / Desktop | A master-password change updated only the local header; the server's login verifier and KDF parameters stayed at their activation values, so every device — including the one that made the change — failed to log in afterwards. Two such changes made offline in sequence left the local header revision two ahead of the server's, and every later publish conflicted | **Fixed** (`POST /v1/account/credentials`: one atomic server transaction; the device commits locally only after a 2xx) |
| S15 | Low | Core / Desktop | A pulled item that failed to decrypt was skipped and the cursor still advanced past it unconditionally; it was never retried, and nothing beyond a per-run count said so | **Fixed** (`unreadable_items` table; retried every sync via `POST /v1/items/fetch`; shown in a persistent banner) |
| S16 | Low | Desktop | Unlocking with the new password while the local header is stale costs up to 3 s before "wrong password" is shown, because a local failure now falls back to an online check | Accepted, documented (`server-sync.md` §6) |
| S17 | Info | Desktop | A keychain call that does not answer within 5 s falls back to `device.json` for that save; the key is migrated back to the keychain at the next start | Accepted, documented (`server-sync.md` §7) |
| S18 | Info | Desktop | If installing or connecting to the platform keychain fails at startup, keys saved during that run go to `device.json`. It is no longer treated as "no keychain": a lookup is reported as *no answer*, never as *no key*, so unlock asks to retry (`keychain_unavailable`) instead of asking for the Emergency Kit, and an install that timed out becomes usable once it finishes | Accepted, documented (`server-sync.md` §7) |
| S19 | Low | Desktop | A keychain `set` that timed out and completes later could re-add an entry after "Remove this device" deleted it — a narrow race between an abandoned write and a delete | Accepted, documented (`server-sync.md` §7) |
| S20 | Info | Desktop | Any process running as the user can usually read this computer's keychain entry (Linux Secret Service, Windows Credential Manager); macOS may prompt. Same trust boundary as the `device.json` fallback it replaces (K1) | Accepted, documented (`server-sync.md` §7, `security-model.md` §13) |
| S21 | Info | Desktop | "Remove this device" renames the vault file (`vault.sqlite3.removed-<timestamp>`) instead of deleting it; it stays on disk as ciphertext, recoverable only with the master password and the Secret Key from the Emergency Kit | Accepted, documented (design §6.1) |
| S22 | Info | Desktop | "Remove this device" requires an unlocked vault, so it is not reachable when the vault fails to open at startup | Accepted, known gap |
| S23 | Low | Server | A device whose online unlock fallback logs in but then fails locally (for example a header that fails to verify) leaves a server session live until it expires (24 h) | Accepted |
| S24 | Info | Deployment | The desktop and server must be upgraded together: `PUT /v1/vault/header` is gone, so an old desktop cannot publish a header to a new server. The local store also moved from schema 4 to 5 with no migration, so an existing `vault.sqlite3` does not open in the new desktop; each desktop signs in again from its Emergency Kit. An account whose password was changed with the old build has a stale server verifier (S14) that no admin command can reset | Accepted, operational; upgrade steps below and in `deployment.md` §6.1 |
| S25 | Info | Verification | The Windows and macOS keychain backends (`windows_native_keyring_store`, `apple_native_keyring_store`) were not compiled or tested in this environment (Linux only); verify on Windows and macOS before relying on them | Open, not yet verified on Windows/macOS |
| S26 | Low | Desktop | A sync already in flight when "Remove this device" runs, followed at once by a sign-in to another account, could apply the old account's pulled data to the new vault | Accepted, not fixed (unlikely: needs a pull to straddle removal *and* a completed sign-in) |
| S27 | Low | Desktop | A master-password change whose response was lost used to report failure although the server may have applied it, and took the device offline so the next sync could not adopt the new header | **Fixed** (the device asks the server's current KDF parameters: new → committed as a success; old → reported as offline; no answer → `password_change_unknown`, "use the new one if the current stops working"; the session is kept) |
| S28 | Low | Desktop | "Remove this device" ignored a failed keychain delete, leaving the Secret Key behind (design §6.3), and revoked the device on the server before setting the file aside | **Fixed** (a busy or failing keychain is waited for and retried once; if the entry still cannot be deleted, removal completes and the user is told to delete `app.havenkeys` by hand; the revocation now follows the rename) |

## Details

### S1. Argon2id before authentication (Medium, fixed)
**Attack scenario:** anyone who can reach the server posts activation
requests with a made-up invite. Each one cost an Argon2id hash (19 MiB, t=2)
before the server looked at whether the invite was real, so a handful of
concurrent requests could keep the CPU busy and starve legitimate logins.

**Fix:** the invite is checked first — a SHA-256 and one indexed row, in
constant time — and only a plausible invite reaches the hash. The attempt is
also rate limited per address, sharing the `login_attempts` machinery, so a
run of bad invites is refused with 429. The authoritative check still happens
again inside the transaction under `FOR UPDATE`, so two racing activations
still resolve to one. Covered by
`tests/auth.rs::repeated_bad_invites_are_rate_limited`.

### S2. The Secret Key could be lost between server and disk (Low, fixed)
**Attack scenario:** not an attack — a crash, a full disk, or a kill at the
wrong moment. Activation created the account on the server, then wrote the
vault, then saved the Secret Key. A failure between the first and the last
left an account whose invite was spent, whose vault existed server-side, and
whose Secret Key had never been written anywhere. The vault could never be
opened again, by anyone, and the user had no way to know why.

**Fix:** the Secret Key is written to `device.json` before the server is
asked. A key stored for an activation that never completed is inert. Sign-in
now accepts an empty Secret Key field and uses the stored one, so the
interrupted setup is finished rather than abandoned.

### S3. A session that did not know its own account (Low, fixed)
`login` built a `Session` with `Uuid::nil()` as the account id, because the
login response does not name the account. Nothing read it yet, but the
obvious next step — filling it from the response — would let the server
decide which account a device believes it is signed in to. The id the caller
derived keys against is carried into the session instead, which is the only
value that means anything cryptographically.

### S4. Paging could skip a batch's tail (Low, fixed before merge)
The cursor is a revision, and every row a batch touched shares one. A page
that ended mid-revision returned a cursor of that revision, and the next pull
asked for `revision > cursor` — skipping every row of that batch that had not
fitted. Those items would never have been pulled again. The applier now only
ever sees whole revisions: the server reads a page plus one batch's worth and
cuts at the last complete revision, which always leaves progress because a
batch is capped at 500 changes. Caught by
`tests/sync.rs::a_page_never_cuts_a_batch_in_half`.

### S5. The server can destroy the vault (High, accepted, inherent)
The server is the single writer, so a deletion it serves is indistinguishable
from one the user made — there is nothing left to compare it against, because
each device's SQLite is a replica that follows the server rather than an
independent copy. A compromised server can therefore empty every device.

This cannot be fixed with cryptography inside this design: authenticating
deletions would only prove that *a key holder* asked, which a server that has
taken over a session can still arrange. It is mitigated operationally:

* tested backups, which `docs/deployment.md` §5 states are a prerequisite of
  storing a real vault, not a follow-up;
* the pull report carries deletion counts, so the UI can say how much went;
* `resync_vault` re-reads the whole vault when a replica is suspect.

It is stated in `threat-model.md` T1c and in the README, because a user
choosing this design should know they are trusting their server's backups
with the existence of their passwords, if not their contents.

### S6. Availability is now correctness (Medium, accepted)
A server that is down is not "sync is behind" — it is "no login can be saved,
no password rotated, no item deleted". Reads keep working from the replica,
including autofill, which is what makes this tolerable. The UI says
`Offline — the vault is read-only until it reconnects` rather than letting a
click fail, and the extension's save prompt is refused with `offline` so the
browser can say something accurate.

### S7. `auth/params` leaks the activation moment (Low, accepted)
For an address with no active account the endpoint answers with a decoy
account id and salt derived from `SERVER_SECRET`, stable across calls, so a
single probe cannot distinguish a real account from an invented one. A prober
who asks repeatedly *over time* sees the answer change when an account
activates. Fixing it would mean pre-computing decoys that later become real,
which trades a small leak for a large amount of state. Accepted: the
enumeration oracle is closed; the transition is visible.

### S8. Rate limiting as a denial of service (Low, accepted)
Counters are keyed per account and per address. Someone who knows an email
can make five bad attempts and lock that account out for a minute, escalating
to thirty; someone behind the same NAT as a victim can do the same by
address. The alternative — no limit — is worse, because the auth key is
verified with a deliberately modest Argon2id. Accepted, documented here.

### S9. Pulled items are size-bounded, not field-checked (Low, accepted)
The sync client refuses a page over 500 changes, a blob over 8 MiB (before
decoding, so an oversized one costs a comparison), and a response over 17 MiB
as it reads. It does not re-apply the per-field limits a locally created item
passes (`MAX_NOTE_CONTENT_BYTES`, `MAX_PASSWORD_CHARS`, …). An item within
the blob cap but larger than the UI would produce is stored as served. It
still has to authenticate under the vault's data key, so this is only
reachable by a device that legitimately wrote it.

### S10. Per-request activity metadata (Low, accepted)
The session lookup updates `devices.last_seen_at` on every authenticated
request, so the server sees each device's activity pattern, not just that it
exists. This is inherent to a server that authorizes requests; it is listed
with the rest of the metadata in `threat-model.md` T1b.

### S11. TLS is the deployment's job (Info, accepted)
The server speaks HTTP and expects a platform in front of it to terminate
TLS; it does not redirect, set HSTS, or refuse plain HTTP, because behind a
private network it should not. The client compensates where it can: a base
URL that is not HTTPS is refused unless it is localhost, and redirects are
never followed, so a bearer token cannot be handed to another host.
`docs/deployment.md` §1 states TLS as a requirement rather than an option.

### S12. A password change is local until it is published (Low, resolved)
Originally: `change_master_password` re-wrapped the vault key locally and
then published the header. If publishing failed, this device used the new
password while others still accepted the old one, until the next sync
noticed the local header revision was ahead and published it. This "publish
later" ordering is what produced S14's stale-server-verifier failure and its
worse case (two offline changes deadlocking every later publish). The 2026-09-23
design replaced it: the server change (`POST /v1/account/credentials`) is
made first, and the local rewrap is committed only after a 2xx, at the
revision the server returned. A local-ahead state can no longer arise from a
password change; `sync_now` treats it as an internal error if observed
anyway rather than trying to publish it. See S14.

### S14. Stale server verifier after a password change (High, fixed)
**Attack/failure scenario:** as S12 describes, the original
`change_master_password` rewrapped the vault key and published a new header,
but never touched the server's login verifier or KDF parameters — those
stayed at their activation values. The next login by *any* device, including
the one that had just changed the password, hashed the new password against
the old verifier and failed: the account was effectively locked out of the
server by its own password change. Because the local header revision had
advanced while the server's had not, two such changes made offline in
sequence left the local revision two ahead of the server's, and every later
publish attempt conflicted with no way to resolve on its own.

**Fix:** a single route, `POST /v1/account/credentials`, updates the KDF
parameters, the auth verifier and the vault header together in one server
transaction. The caller proves it knows the *current* auth key, checked
against the stored verifier and rate-limited exactly like a login (a stolen
session token alone cannot change the password); `baseHeaderRevision` must
match the server's current one or the request is refused with a conflict.
On success every other session on the account is deleted. The device making
the change commits its local rewrap only after a 2xx, at the revision the
server returned — never before. Every other device is now signed out and
picks up the new password at its next unlock, through an online fallback
that verifies the served header before adopting it (`server-sync.md` §6).
Tested in `crates/havenkeys-server/tests`, `havenkeys-sync-client`,
`havenkeys-core` (the rekey and online-unlock-verification tests), and the
real-server round trip added for this fix (`654b81f`, "a password change
reaches a second device and survives a lost response").

### S15. Unreadable pulled items were skipped permanently (Low, fixed)
**Failure scenario:** `apply_remote_changes` advanced the stored sync cursor
unconditionally, including past a change whose blob failed to authenticate
under the data key or was missing entirely. That item was then never pulled
again: the next sync starts at `since=cursor`, which is already past it.
Nothing signalled this beyond a per-run `SyncReport.skipped_items` count, and
the only way back was a full manual re-download.

**Fix:** such a change is now recorded in a local `unreadable_items` table
(vault schema 5) in the *same transaction* that upserts, deletes and
advances the cursor for the rest of that pull's changes, so the cursor and
the retry list can never disagree. At the end of every `sync_now`, if the
table is not empty, the app fetches those IDs (in chunks of up to 500) from
the new `POST /v1/items/fetch` and applies the result through the same apply
path, without moving the cursor; an item that now decrypts, or that the
server has since deleted, leaves the table. `VaultStatus.unreadableItems`
drives a persistent banner on the vault screen with a **Re-download** action
until the count reaches zero. `reset_sync_cursor` clears the table too, since
a full re-download re-evaluates every item anyway.

### S24. Upgrading to the account-fixes build (Info, operational)
The server route set changed (`PUT /v1/vault/header` is gone, replaced by
`POST /v1/account/credentials`), and the desktop's local store went from
schema 4 to schema 5 with no migration (the only vault is the owner's, and
it lives on the server). Upgrade in this order:

1. **Before upgrading anything**, check that no account on the server had
   its master password changed with the old build. Such an account has a
   stale verifier (S14): its devices all fail to sign in after the change,
   so it shows as permanently offline. There is **no admin command that
   resets a verifier** — it is derived from the password on the client and
   cannot be computed on the server. There is no in-place repair: changing
   the password again on the old build cannot fix it, because that build
   needs a server session to change a password and picks a fresh KDF salt
   each time. The account must be deleted and recreated
   (`admin delete-account`, `admin new-account`), which loses its items.
2. Upgrade the server.
3. Upgrade every desktop. Change no passwords in between.
4. On each desktop, before starting the new build, move `vault.sqlite3` and
   `device.json` out of the app's data folder to somewhere safe. Start the
   new build and choose **Sign in** with the Emergency Kit. Once the vault
   has synced, the moved files can be deleted.

### S13. A bridge thread waits for the network (Info, accepted)
Saving a login from the browser blocks the bridge thread that is handling
that request until the server answers or times out (30 s). The vault lock is
not held, so locking, auto-lock and every other request are unaffected, and
the bridge already serves each connection on its own thread.

## Verified properties (this phase)

Each of these has a test that fails if the property stops holding.

* An authenticated account reaches nothing belonging to another one, on every
  route that takes a session, including writing with another vault's item id
  (`tests/isolation.rs`).
* A wrong auth key, an unknown email and an account that was invited but
  never activated produce byte-identical answers (`tests/auth.rs`).
* `auth/params` answers the same shape for unknown addresses, stably across
  calls, with a different decoy per address.
* Five failed logins block the account; a success clears the counters.
* A revoked device loses its session on its next request and cannot sign in
  again (`tests/devices.rs`).
* A stale `baseRevision` refuses the whole batch and names the conflicting
  items; nothing from that batch is written (`tests/sync.rs`).
* A batch is applied under one revision, and the pull cursor never skips a
  batch's tail.
* Oversized bodies, oversized blobs, oversized headers, unknown fields,
  non-UUID ids and malformed JSON are all refused, and no error quotes the
  request back (`tests/limits.rs`).
* No token, auth key, invite, blob, header or email appears in any log line,
  while the account id still does (`tests/no_logging.rs`).
* A hostile server cannot make the client panic or allocate without bound:
  malformed, truncated, oversized and self-contradictory answers all produce
  a `SyncError` (`havenkeys-sync-client/tests/hostile.rs`).
* KDF parameters below the core's floor are refused before any derivation,
  and a key scheme below 3 in a served header is refused.
* An item written on one device opens on a second that signed in with only
  the master password and the Secret Key, against the real server and a real
  Postgres (`tests/round_trip.rs`).
* A staged write — from the UI, from an import, or from the browser
  extension — touches neither the store nor the session cache until the
  server has accepted it, and a lock in between discards it
  (`havenkeys-core/tests/writes.rs`, `tests/security.rs`).
* The extension still cannot write to a login that is not saved for the page
  it is on (`havenkeys-bridge/tests/bridge.rs`).

## Verification pending

* **The deploy itself.** Nothing here has run outside a test container: no
  TLS, no Railway health check, no real network. `docs/deployment.md` is the
  checklist.
* **The restore drill** (`deployment.md` §5). Until it has been run, the
  vault has no backup, and S5 has no mitigation.
* **Two devices in real use.** The round trip runs two `VaultService`
  instances in one process; nobody has yet unlocked the app on two computers
  and watched a change cross.
* **Argon2id parameters on target hardware** for the server's verifier
  (19 MiB, t=2, p=1) under concurrent load.

---

# Security Review: Passkeys

**Date:** 2026-09-24
**Scope:** `crates/havenkeys-core/src/passkey/` and how `vault.rs`,
`model.rs` and `secret.rs` use it; the passkey requests in
`crates/havenkeys-protocol` and `crates/havenkeys-bridge`;
`list_passkeys`/`delete_passkey` in `apps/desktop/src-tauri`; in the
extension, `src/webauthn/`, `background/webauthn-handler.ts`,
`background/registration.ts`, `menu/passkey.ts` and the passkey rows of the
field menu; the TypeScript protocol mirror. Branch base `cd81037`, reviewed
at `2e0eb73`. Design: `docs/superpowers/specs/2026-09-23-passkeys-design.md`.
**Method:** self-review of the implementation against the design, per-task
code reviews during development, the test suites written alongside it, and
the audits below. No browser was available, so nothing here has been run
against a real relying party (see the manual checklist at the end).

> This is an internal review, not an independent security audit.

**Update, 2026-09-24** (`docs/superpowers/plans/2026-09-24-passkey-upgrade.md`,
design `docs/superpowers/specs/2026-09-24-passkey-upgrade-design.md`): PK22–PK27
are new findings from the automatic passkey upgrade built on top of this
review — a conditional `create()` right after a HavenKeys password fill, plus
the field menu's "you have a passkey" / Passkeys Directory hint
(`passkey_status`). Branch base `07f2436`, this task's verification at
`e409c7b`. Numbering continues in this table rather than starting a new one,
since this work extends the same passkey surface reviewed above.

**Update, 2026-09-26** (`docs/superpowers/sdd/2026-09-26-auto-sign-in/`,
design `docs/superpowers/specs/2026-09-26-auto-sign-in-design.md`): AS1 and
AS2 are new findings from automatic sign-in — after a pick whose fill Rust
marks `autoSubmit`, HavenKeys presses the site's button and, unattended,
carries a multi-step or TOTP sign-in through to the end. It shares the same
background-worker and content-script surface as the passkey work above
(sessions, sender-derived origins, guarded picks), so its findings continue
the same table rather than starting a new one. Branch base `f1c1b69`, this
task's verification below.

## Summary

We found no critical or high-severity issues. Passkey private keys stay in
the Rust core. Every signature and creation is bound in Rust to a
relying-party ID that the page, at the URL the browser reports, is allowed
to use. Nothing is signed or created without a guarded click in extension
UI. What remains is design trade-offs (UV from an unlocked vault, counter 0,
synced keys) and edge cases around replacement, sessions and old app
versions. All of them are listed below.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| PK1 | Medium | Design | UV is asserted because the vault is unlocked and the user clicked. There is no fresh verification and no biometrics | Accepted, documented |
| PK2 | Low | Design | The signature counter is always 0, so relying parties cannot detect cloned credentials by counter | Accepted, documented |
| PK3 | Medium | Core / operations | An app version without passkey support drops a login's passkeys when it edits that login | Accepted; update every device first |
| PK4 | Low | Extension | "Use another device" calls the browser after an async hop, and may lose user activation on sites that require it | Accepted |
| PK5 | Medium | Core / bridge | Replace-before-confirm: re-registering an account replaces its working passkey before the site has accepted the new one | Accepted, documented |
| PK6 | Low | Extension | One passkey session per tab: a granted-host iframe can abort the top frame's request | Accepted (denial of service only) |
| PK7 | Low | Extension | Firefox has only the time-based click guard, so a page can clickjack its own `passkey.html#token` | Accepted (page's own rpId only) |
| PK8 | Low | Core | IP-address rpIds are accepted when they equal the page host, although browsers refuse WebAuthn there | Accepted |
| PK9 | Low | Core | `authorize_rp` does not check that the top-level page is a secure context | **Fixed** (final review) |
| PK10 | Info | Core | `clientDataJSON` strings are escaped with `serde_json`, not WebAuthn's `CCDToString` | Accepted; the bytes are identical for every value that can occur |
| PK11 | Low | Extension | `pagehide` cancels conditional requests, so a page restored from the back/forward cache loses passkey autofill until it asks again | **Fixed** (final review): the site's promise is left to the browser's own conditional request |
| PK12 | Low | Extension | Lock and unlock events arrive only while the native port is open, and it closes after 60 s idle. A locked card may never refresh, and a card may outlive a lock | **Fixed** (final review): the card's polling re-runs the lookup; a card that outlives a lock still fails closed |
| PK13 | Info | Extension / core | If the site aborts a `create()` after the server accepted it, the vault keeps a passkey the site never registered | Accepted; visible and deletable in the desktop |
| PK14 | Info | Extension | The save card shows the account name the site chose (up to 512 characters) | Accepted |
| PK15 | Info | Core | The redacting `Debug` of `Registration`, `Assertion`, `StagedPasskey` and `PasskeyMatch` was confirmed by reading the code; only `Passkey` has a test | Accepted |
| PK16 | Info | Deps | New dependencies: `p256` 0.14 and its RustCrypto tree, plus `ciborium` (dev only) | Audited below: no advisories, licenses allowed |
| PK17 | Medium | Extension | The MV3 worker can be suspended once the native port idles out, losing every passkey session: conditional passkeys vanish, and a modal card cannot be closed | **Fixed** (final review) |
| PK18 | Low | Extension | A desktop `denied` became `SecurityError`, breaking paths the browser allows (related origins, permitted cross-site iframes) | **Fixed** (final review): fallback |
| PK19 | Low | Extension | `excludeCredentials` answered `InvalidStateError` at once, so any page could probe which of its accounts HavenKeys holds without a click | **Fixed** (final review); remaining signals accepted |
| PK20 | Low | Extension | With the isolated bridge gone (Firefox unloads it on disable/update) the page's promise never settled | **Fixed** (final review) |
| PK21 | Low | Extension / bridge | A page could drain the desktop's shared lookup rate limit with WebAuthn calls, and a site timeout of 0 closed the card at once | **Fixed** (final review) |
| PK22 | Medium | Core / extension (automatic upgrade) | A conditional `create()` right after a HavenKeys password fill can save a passkey with no card and no click at all, relaxing rule #6 (explicit user action) for that one case | Accepted, documented: bounded by a 5-minute fill window, same login and site, matching (folded) account name, add-only (never replaces), one silent passkey per fill, and the `auto_passkey_upgrade` off switch. **Not a boundary against a compromised extension**: such an extension could already create a passkey for a site it names through the ordinary clicked `passkey_create` (`conditional: false`), which the core cannot tell from a real click; the automatic-upgrade check narrows what a *hostile page* can trigger unassisted, and adds no new capability to a compromised extension |
| PK23 | Low | Core (check/create timing gap) | `check_passkey_create` has no `userHandle` (only `passkey_create`'s wire shape carries one), so it can answer `upgrade: auto` for an account whose passkey the vault already holds under that handle; `stage_passkey_create` then denies via its `find_passkey_holder` check | Accepted: fails closed. The extension's `catch` around the `passkey_create` call (`webauthn-handler.ts` `beginUpgrade`) falls back to the browser with no notice and no card. Cost: that one request goes to the browser instead of HavenKeys, silently; nothing is created, replaced, or shown to the user |
| PK24 | Info | Core (fill consumption) | A recent-fill entry is deleted at `stage_passkey_create` time — before the server confirms the write — "spent even if the write later fails" (code comment, `passkey/vault.rs`) | Accepted, the conservative choice: a script cannot repeat a silent create with a fresh user handle while the first request is still in flight. Residual: if the server write then fails (offline, error), the site gets no passkey and the user must fill the password again to re-arm the window for a second silent attempt |
| PK25 | Info | Extension (silent-save abort) | If the page aborts, navigates away, or otherwise cancels its request while an `auto` (silent) save is already in flight, the passkey the desktop already created and committed **stays in the vault**, even though the site never receives it and no "saved" notice appears | Accepted, same class as PK13 (orphan passkey after a late abort), but here it can happen with no card ever shown. Visible and deletable in that login's Passkeys list in the desktop app (`autofill.md`, Limitations) |
| PK26 | Info | Extension (`passkey_status`) | The field menu's "you have a passkey for `<site>`" hint asks a Lookup-class `passkey_status` request once per menu open; the yes/no answer itself never reaches the page, but a page can already see the menu's iframe appear and estimate its height, which changes by one row depending on the answer | Accepted, the same residual signal `threat-model.md` and `autofill.md` already document for menu presence generally; `passkey_status` adds no item ID, credential ID, or account data to what a page can observe |
| PK27 | Info | Extension (Passkeys Directory data) | The "`<name>` supports passkeys" hint's site names and help links are third-party data from the 2factorauth Passkeys Directory (`apps/extension/src/data/passkey-sites.json`), matched to a page by plain host-suffix comparison with no Public Suffix List | Accepted: this is a UI hint only, never an authorization decision — a wrong match can only show or hide a static help link, and `github.com.evil.com` still never matches `github.com`. The data is a build-time snapshot committed to the repo and reviewed like code (`scripts/update-passkey-directory.mjs`, run by hand, never fetched at runtime); the site name is rendered with `textContent` only, and only `https:` help links are kept, opened only after a trusted click via `chrome.tabs.create` |
| AS1 | Medium | Extension / core (automatic sign-in) | For up to 2 minutes after a pick, a page on the same origin receives the password and the current TOTP code without further clicks, relaxing rule #6 for the rest of that one sign-in | Accepted, documented: starts only from a trusted pick; bound to tab, frame and exact origin; forward-only single-use steps; every value re-requested from Rust with the origin check; global and per-login off switches (`auto_sign_in`). Script on that origin could already obtain the password after the single manual pick; the new part is that the OTP no longer needs its own click. Not a boundary against a compromised extension, which can call fill_item / get_totp directly |
| AS2 | Low | Extension (automatic sign-in) | `cs_run_step` messages carry no run identity, so in a narrow race a leftover step confirmation from a replaced run in the same tab/frame/origin can advance the newly picked login's run one step early | Accepted: bounded because every value is re-requested from Rust for the frame's URL, and the login was just picked by the user on that origin |

## Details

### PK1. UV from an unlocked vault (Medium, accepted)
**Attack scenario:** someone sits at the user's unlocked, unattended
computer, opens a site that has a HavenKeys passkey, and signs in with one
click. The relying party sees UV=1 and may treat it as a second factor.
**Mitigation:** auto-lock, and lock on OS screen lock and on suspend
(`threat-model.md` T7). A signature needs a click in the extension's passkey
card or field menu. **Remaining:** UV=1 means "the vault was unlocked with
the master password". It does not mean biometrics or a fresh check. The
design rejected a desktop confirmation or a re-prompt for each sign-in.

### PK2. Counter 0 (Low, accepted)
Every assertion carries signCount 0 (`crypto.md` §Passkeys). A relying party
that uses the counter to spot a cloned authenticator gets no signal from it.
Synced passkeys from other providers do the same. The BE and BS flags tell
the relying party that the credential is synced.

### PK3. Old app versions drop passkeys (Medium, accepted)
**Attack scenario (accidental):** a device still runs a build from before
passkeys, and the user edits a login there. That build reads the details
without the `passkeys` field and seals them back without it. The server
accepts the write, and every device loses that login's passkeys. There is no
history to restore them from. **Mitigation:** operational only: update
HavenKeys on every device before saving a passkey. The design chose not to
bump the format, because the only vault belongs to the project owner and may
be reset. **Remaining:** the loss is silent.

### PK4. "Use another device" and user activation (Low, accepted)
The click happens in the extension's frame. The fallback then reaches the
page's original `navigator.credentials` through the background and the
bridge. A site or browser that requires transient user activation for
WebAuthn may refuse the call if that hop takes too long. The user can cancel
and use the site's own "sign in with a security key" path.

### PK5. Replace-before-confirm (Medium, accepted)
When the vault already holds a passkey for the same rpId and user handle, a
new `create()` replaces it. The replacement happens in the same server write
that stores the new passkey (`stage_passkey_create`), which is how WebAuthn
Level 3 §5.1.3 step 21.3 says an authenticator behaves.
**Failure and attack scenarios:**
* (a) The site rejects the new credential after we returned it. Causes
  include the site's own error, the user closing the tab, or the page
  aborting after the server accepted.
* (b) The server accepted the write but the local commit fails, for example
  because the vault locked in between. The site gets an error.

In both cases the old, working passkey is gone, and the site knows only the
old one. A compromised extension that has learned a user handle (assertions
return it) can do the same on purpose, within the secret rate limit. There
is no per-item cooldown for passkey writes. **Mitigation:** none in code.
The design keeps one passkey per account. **Remaining:** the user may lose
passkey access to that account and have to recover through the site. Later
work (`roadmap.md`) could add a confirmation step that keeps the old key
until the site has used the new one.

### PK6. One passkey session per tab (Low, accepted)
The background keeps one session per tab. A new request from any granted
frame in the tab ends the pending one with `AbortError`. So an iframe on a
granted host, such as an ad, can keep aborting the top page's passkey
request. It gains nothing else, because the iframe's own request is checked
against its own origin. This is a denial of service only.

### PK7. Firefox clickjacking of the passkey card (Low, accepted)
This is the same issue as F3 for the menu. Firefox has no
IntersectionObserver v2, so the card's only click guard is the 400 ms delay.
The token is in the frame's URL fragment, which the page can read, and
`passkey.html` is a web-accessible resource the page can frame itself. So a
page could trick the user into clicking its own passkey prompt. The result
is a sign-in or a new passkey for that page's own origin and rpId, which the
page could request anyway.

### PK8. IP-address rpIds (Low, accepted)
As the design specified, `authorize_rp` accepts an IP rpId when it equals
the page's host (rpId `127.0.0.1` on `https://127.0.0.1/`). Browsers refuse
WebAuthn on IP hosts, so HavenKeys is more permissive than the platform
here. No other site's passkey is reachable, because the stored rpId must
equal the host.

### PK9. Top page's scheme not checked (Low, fixed)
For an iframe, `authorize_rp` requires the frame to be a secure context and
same-site with the top page. It does not require the top page to be a secure
context, and `same_site` compares hosts, not schemes. So an
`https://login.example.com` frame inside `http://example.com` would get a
signature, with `crossOrigin: true` and an `http:` `topOrigin` in
`clientDataJSON`. Browsers treat such a frame as a non-secure context and
refuse WebAuthn. The impact stays within the same site, and a relying party
can reject the `topOrigin`. **Fixed** in the final review: `authorize_rp`
applies `secure_context` to the top URL too (test
`frames_must_be_same_site_with_the_top_page`).

### PK10. clientDataJSON escaping (Info, accepted)
Values are escaped with `serde_json`. WebAuthn's `CCDToString` escapes
differently only for control characters and some non-ASCII characters. None
of those can occur here: the type is fixed, the challenge is base64url, and
origins are ASCII serializations. The test `client_data_is_exact` pins the
bytes. See `crypto.md` §Passkeys.

### PK11. Back/forward cache (Low, fixed)
The bridge cancels every pending request on `pagehide`, and `pagehide` also
fires when a page enters the back/forward cache. So when a page is restored,
its conditional `get()` has already been rejected with `AbortError`, and
HavenKeys passkeys no longer appear in its field menu until the page calls
`get()` again. In that path the wrapper also did not abort the browser's own
conditional request, which kept running with nobody waiting for its result.
**Fixed** in the final review: an `AbortError` on HavenKeys' side that the
site did not cause (`pagehide`, or a newer request replacing ours) no longer
rejects the site's promise. It is left to the browser's own conditional
request, which is still running and can answer it. Only the site's own
abort rejects and stops both. **Remaining:** after a restore from the cache,
HavenKeys' passkeys are missing from the field menu until the page calls
`get()` again.

### PK12. Events need an open native port (Low, fixed)
The background learns of `locked` and `unlocked` only while its port to the
native host is open. It closes the port after 60 s without a request (H3 is
the same effect for save prompts). While a card is open, it polls the
background, not the desktop, so the port can close under it. Two things
follow:
* A card opened while the vault was locked does not refresh when the vault
  is unlocked. The user has to cancel and try again.
* A card that is open when the vault locks stays up. A pick or save from it
  is refused by the desktop with `locked`.

Both fail closed. **Fixed** in the final review: `pk_state` on a locked
session re-runs the lookup (once at a time), so the card's own 1.5 s polling
turns it into the chooser or save card after an unlock, whether or not the
`unlocked` event arrived. The second point remains and fails closed.

### PK13. Orphan passkey after a late abort (Info, accepted)
The page may abort after the user clicked Save and the server accepted the
write. Then the site never receives the credential, but the passkey stays in
the vault. It is listed on the login in the desktop app and can be deleted
there.

### PK14. Site-supplied account name on the save card (Info, accepted)
The card shows `user.name` as the site sent it. The page script cuts it to
512 characters, and the core refuses control characters. A site can put
misleading text there. It cannot change the site name the card shows, which
comes from the browser-reported URL.

### PK15. Debug redaction coverage (Info, accepted)
See the secret-logging review below. By inspection, the redacting `Debug`
impls exist and are correct. Only `Passkey`'s has a unit test.

### PK16. Dependencies (Info)
See Audits.

### PK17. Worker suspension lost passkey sessions (Medium, fixed)
Sessions live in the background worker's memory. The native port closes
after 60 s idle, and an MV3 worker is then suspended about 30 s later,
dropping every session. Conditional passkeys vanished from the field menu on
a login page left open for about 90 s. A modal card got "This prompt has
expired." on every button, could not be closed, and the page waited for the
bridge's timer (up to 5 min 10 s). **Fix:** the bridge pings its session
every 20 s (`wa_ping`, validated like `wa_cancel`, answered only for a live
session of the same frame). The ping keeps the worker awake. When the
session is gone, a conditional request is sent again with the options the
bridge kept (if that yields a fallback, the browser's own leg carries on),
and a modal request ends in a fallback and its card is removed. A card
whose session is gone can still be closed at once: Cancel, Escape, Close
and "Use another device" for an unknown token make the background send the
result to every frame of the card's tab; only the bridge that holds the
128-bit token (the card and the bridge know it) acts on it, and a finished
request is no longer pending there. Tests: `webauthn-handler.test.ts`
("worker suspension"), `bridge.test.ts` ("lost sessions"),
`menu/passkey.test.ts`.

### PK18. `denied` became `SecurityError` (Low, fixed)
The design (§7.4) answered an rpId the desktop does not allow with
`SecurityError`. `authorize_rp` is stricter than browsers in two places:
Related Origin Requests, and cross-site iframes the top page permitted with
`allow="publickey-credentials-get"`. Both are legitimate and the browser
would serve them. **Fix:** `denied` falls back to the browser everywhere in
the background handler (initial lookup and the lookup after an unlock). The
browser applies the same rpId rule and raises `SecurityError` itself where
it applies, so the site sees what it would see without HavenKeys.

### PK19. Presence probing through `excludeCredentials` (Low, fixed; residual accepted)
**Attack scenario:** a page calls `create()` with a guessed or known
credential ID in `excludeCredentials`. HavenKeys answered `InvalidStateError`
at once when the vault held it, and fell through otherwise, so the page
learned, without any click, whether the user's HavenKeys holds that
account's passkey. **Fix:** the create card opens in an "already saved"
state ("A passkey for this account is already saved in HavenKeys") with
**Close** and **Use another device**. Only a guarded click on Close
finishes with `InvalidStateError`; "Use another device" hands the request to
the browser; Escape, the timeout or the site's abort give the usual
`NotAllowedError`/`AbortError`. **Remaining (accepted):** a frame appearing
reveals that HavenKeys has something for the site (as for the menu); for a
`get()` with `allowCredentials`, a chooser versus an immediate fallback tells
the page whether one of the listed credentials is in HavenKeys. In every
case the page learns this only about its own rpId, which a platform
authenticator's UI reveals in similar ways.

### PK20. Page promise hung without the bridge (Low, fixed)
Firefox unloads an extension's isolated content scripts when it is disabled
or updated but leaves the MAIN-world wrapper in open pages. A WebAuthn call
there was sent to no one and never settled. **Fix:** the bridge acknowledges
each valid request synchronously (`{id, outcome: "ack"}`, parsed strictly).
Without an acknowledgement within 1 s, the wrapper hands a modal or create
request to the browser, and ends only its own leg of a conditional one. An
acknowledged modal request that is never answered ends with
`NotAllowedError` after the clamped timeout plus 15 s (no ceiling for
conditional requests). The wrapper uses `setTimeout`/`clearTimeout`
captured before page script runs. Tests: `page.test.ts` ("missing or silent
bridge").

### PK21. Lookup budget and tiny timeouts (Low, fixed)
The desktop's lookup rate limit is shared by every connection. A page could
fire `get()`/`create()` in a loop and use it up, starving the field menu.
**Fix:** at most one `wa_get`/`wa_create` lookup in flight per tab; another
meanwhile falls back at once without reaching the desktop. Separately, a
site `timeout` of 0 or 1000 ms closed the chooser before anyone could read
it. Site timeouts are now clamped to 10 s – 5 min wherever they drive our
UI or session timers (page ceiling, bridge timer, background session TTL).
Also in this wave: `list_passkeys` (desktop) no longer resets the auto-lock
timer, like `list_items`.

### PK22. Automatic upgrade: a relaxed click rule (Medium, accepted)
**What it does:** right after HavenKeys fills a password and the site calls
`create()` with `mediation: "conditional"` (its own "upgrade to a passkey"
offer), HavenKeys can save a passkey with no card and no click in HavenKeys
UI at all — the *consent* is that recent password fill itself. This is a
deliberate, narrow exception to Critical Engineering Rule #6 ("never
autofill — or, here, create a credential — without explicit user
interaction").

**Why it is bounded:** the decision is made in Rust, never trusted from the
extension. `VaultService::fill_for_page` records, in the unlocked session's
memory only, which login was filled on which site and when
(`Session::recent_fills`, capped at 16, dropped on lock). `upgrade_for`
allows a silent save only within `UPGRADE_WINDOW_MS` (5 minutes) of that
fill, for that same login, on that same site (registrable domain), and only
when the account name the site sent matches (folded) or the login has none.
`stage_passkey_create` re-derives this itself and refuses (`Denied`) unless
it still comes out `Auto` for exactly the item named — the extension's claim
that a request is the automatic upgrade is never taken on trust
(`conditional_create_needs_auto_for_exactly_that_login`). A silent save only
ever *adds* a passkey: if any login already holds one for the same account
(rpId and user handle), the request is denied
(`conditional_create_never_replaces_the_filled_logins_own_passkey`), so
replacing a passkey always needs the card. A successful silent save spends
the fill, so one password fill grants at most one silent passkey
(`one_fill_grants_one_silent_passkey`). The vault setting
`auto_passkey_upgrade` (on by default) turns the whole thing into the
ordinary "Add a passkey?" card instead.

**Residual (accepted):** this is not a boundary against a *compromised
extension*. Such an extension can already send `passkey_create` with
`conditional: false` and any `itemId` it names for a site it names — the
ordinary clicked path — and the core has no way to distinguish that from a
real user click, because the click happens in extension-controlled UI
(`threat-model.md`, "Compromised extension"). The automatic-upgrade check
adds no new capability to a compromised extension; it only keeps the
*silent* path narrow against hostile *pages* and extension *bugs*, which
cannot forge a recent HavenKeys fill of a specific login on a specific site.
This is stated in the shipped docs (`threat-model.md`, "The automatic
upgrade's relaxed click rule"; `security-model.md` §15, "Automatic upgrade";
`autofill.md`, "Automatic upgrade").

### PK23. `check_passkey_create` can say `auto` where `stage_passkey_create` denies (Low, accepted)
`check_passkey_create`'s wire shape (and `CreateQuery`) carries `userName`
but not `userHandle` — the site does not send a handle until the actual
`create()` call reaches `passkey_create`. So `upgrade_for`, called from
`check_passkey_create`, cannot check `find_passkey_holder` (which needs the
rpId *and* the user handle) and can answer `Upgrade::Auto` for an account
whose passkey the vault already holds under a handle it has not seen yet.
`stage_passkey_create` re-derives the same decision but *does* have the
handle by then, sees `holder.is_some()`, and denies the conditional request
(`req.conditional && holder.is_some()` → `Error::Denied`).

**Why this is safe:** the extension's `beginUpgrade` (`webauthn-handler.ts`)
sends the `passkey_create` request only after `check_passkey_create`
answered `auto`, and wraps that second call in a `catch` that returns
`FALLBACK` on any error, including `denied` — no card, no notice, silent
fallback to the browser, exactly as for "no recent fill" or any other
disqualifying condition. No passkey is created or replaced, and nothing is
shown to the user beyond what they would see without HavenKeys. **Cost if
this ever mattered:** that one request goes to the browser instead of
HavenKeys for that page, which is the same outcome as HavenKeys not being
installed.

### PK24. The fill is spent at stage time, not at server-confirmation time (Info, accepted)
`stage_passkey_create` deletes the matching `recent_fills` entry as soon as
it builds the write, before the caller sends it to the server and commits
it — the code comment states this explicitly: "Spent even if the write
later fails: the user can fill again." This was a deliberate final-review
choice (over spending the fill only after a confirmed commit) because the
alternative would let a script keep a conditional `create()` in flight and
repeat it with a fresh user handle before the first write resolves.
**Residual:** if the server write then fails (offline, a transient error),
the site receives no passkey and the fill is already gone; the user has to
fill the password again to re-arm the 5-minute window for a second silent
attempt. This costs the user one re-fill at worst, never a lost passkey or
an incorrect grant.

### PK25. Orphan passkey after an abort during a *silent* save (Info, accepted)
This is PK13 for the automatic-upgrade path specifically, and worth its own
entry because no card is ever shown here. If the page aborts, navigates
away, or otherwise cancels its `create()` request after HavenKeys has
already created the passkey and committed it to the server, the passkey
**stays in the vault** — the site never receives the credential, and the
"Passkey saved to HavenKeys" notice does not appear either, because the
cancellation reaches the extension too late to stop a save already in
flight. The orphaned passkey is visible in that login's Passkeys list in the
desktop app and can be deleted there, exactly as for PK13. Documented in
`autofill.md` ("Automatic upgrade" and "Limitations").

### PK26. `passkey_status`: a one-bit answer with the same residual signal as menu presence (Info, accepted)
The field menu asks `passkey_status` (core: `has_passkey_for_page`) once
each time it opens on a page that already lists a saved login, to choose
between a "you have a passkey for `<site>`" hint and a Passkeys Directory
help row. It is Lookup-class, like `find_matches`, and its only output is a
boolean; it names no item, credential ID, or account. The boolean itself
never reaches the page — only whether *our own* menu shows an extra hint row
does. A page can already observe that the menu's iframe appeared and
estimate its rough height (the residual `autofill.md` already documents for
menu presence in general); `passkey_status` changes that height by at most
one row and adds no other observable signal. Accepted as no worse than the
existing menu-presence residual.

### PK27. Passkeys Directory: third-party data used only as a UI hint (Info, accepted)
The "`<name>` supports passkeys" / "How to add one" row's site names and
help links come from a snapshot of the [2factorauth Passkeys
Directory](https://github.com/2factorauth/passkeys)
(`apps/extension/src/data/passkey-sites.json`, CC-BY-4.0, credited in
`THIRD-PARTY-NOTICES.md`). It is committed at build time by
`scripts/update-passkey-directory.mjs`, run by hand and reviewed like any
other change — never fetched at runtime, so a compromise of the upstream
repository cannot affect an installed HavenKeys without a maintainer
committing the resulting diff. `findPasskeySite` matches a page to an entry
by plain host-suffix comparison (page host equals a domain, or ends with
`.` + a domain, longest domain wins) with no Public Suffix List. **Why this
is acceptable despite the weaker matching:** this is a UI hint, never an
authorization decision — the worst a wrong match can do is show or hide a
static help link; it grants no credential, fills nothing, and
`github.com.evil.com` still never matches `github.com` under this scheme
(no fabricated domain in the dataset ends with a real one as a suffix). The
site name is rendered with `textContent` only (never `innerHTML`), only
`https:` documentation links are kept by the generator, and a link opens
only after the menu's usual trusted-click guard, via `chrome.tabs.create` in
a new tab, carrying no vault data.

### AS1. Automatic sign-in: a relaxed click rule (Medium, accepted)
**What it does:** after a pick whose fill Rust marks `autoSubmit`, HavenKeys
presses the site's sign-in button on its own, and, unattended, follows a
multi-step or TOTP sign-in to the end: fill password → press → (page
navigates or re-renders) fill the next step → press, up to and including the
one-time code. This relaxes Critical Engineering Rule #6 for the rest of
that one sign-in, for up to 2 minutes after the pick.

**Why it is bounded:** the decision is made in Rust, never trusted from the
extension — `VaultService::auto_sign_in_for` computes
`settings.auto_sign_in && item.auto_sign_in` after the same origin check
that already gates `fill_item`/`get_totp`. The run
(`background/signin-run.ts`) is bound to the exact tab, frame and origin of
the pick, moves forward only (`username → password → otp`), accepts each
step once, never retries, and expires 2 minutes after the pick
(`RUN_TTL_MS`). A page cannot start or extend a run: `cs_run_step` is
accepted only from that same tab, frame and origin, and only for the step
that comes next (`signin-run.ts` `accept`). Every step's value is still
fetched from Rust for the frame's *current* URL, so the origin check runs
again on each step, not just once at the pick. Global (`auto_sign_in`) and
per-login (`auto_sign_in`) switches, both on by default, let the user turn
this off entirely or per site.

**Residual (accepted):** a page on the picked login's own origin already
gains nothing new here that a manual pick did not already give it — the
password was already handed to that page's own script the moment the user
picked the login, with or without this feature. What automatic sign-in adds
is that a TOTP code, which previously needed its own click, can now reach
the page without one, for up to 2 minutes. This is the same class of
relaxation as the automatic passkey upgrade (PK22): **not a boundary against
a compromised extension**, which can already call `fill_item` and
`get_totp` directly for any item and origin it names, within the rate
limit — a run only changes what a hostile *page* can obtain unassisted, not
what a compromised extension can already do.

### AS2. A late step confirmation from a replaced run (Low, accepted)
**What it does:** `cs_run_step` (content script → background) carries only
`{ kind }`; the background matches it to the live run by the sender's tab,
frame and origin (from the browser's own sender data), not by a per-run
token. If the user picks a new login in the same tab while an older run's
watcher is still mid-flight — for instance, a stale `cs_run_step` message
already queued in the runtime message pipe when the new pick replaces the
run — the background could, in a narrow timing window, treat that leftover
confirmation as belonging to the new run and step it forward one stage
early.

**Why it is bounded:** the content script itself guards the common case —
each watch and pending press is tied to a local run sequence number
(`runSeq` in `content/index.ts`) that a new pick, `bg_run_end`, or the
user taking over increments, so a superseded watcher's own result is
suppressed before it is even sent. What remains is the residual background
race described above. Even there, nothing is filled or pressed on
mistaken trust alone: the background still calls `fill_item` or `get_totp`
for the frame's *current* URL and receives Rust's own origin-checked answer
before filling anything, and the new run was itself started by the user
picking that same login on that same origin moments earlier. The worst
outcome is a step of the *user's own, just-picked* login being pressed one
step ahead of schedule on the origin they just interacted with — not a
credential reaching an unrelated item or site.

**Fix considered and deferred:** binding `cs_run_step` to a per-run token
(as menu and save sessions already are) would close this race outright. It
was left as an accepted limitation for this MVP because the residual is
already narrow — same user, same login, same origin, no secret exposure
beyond what a normal continuation would produce moments later — and the
fix would add a token to thread through every watch and press call for a
race that has not been observed to occur in practice.

The per-task reviews also deferred some minor items that are not security
findings. They are tracked in the development ledger, not here. Examples:
`Host::parse` percent-decodes an rpId such as `github%2Ecom` into
`github.com` before the equality check, which cannot widen what a page
reaches; `assert` accepts a stored key shorter than 32 bytes; and
`find_passkeys` results are cut off at 50.

### AN30. The autofill service's scope has no exception handler (Low, fixed)
**Component:** `HavenAutofillService`. Parsing a structure Android hands
the service (fill, and now save) is done in a coroutine scope with no
`CoroutineExceptionHandler`. **Scenario:** a hostile app builds a view
structure that makes the parser throw something unexpected; the uncaught
exception ends the process. **Effect:** the in-memory vault goes with the
process, so it fails closed (the user unlocks again); it can be used to
annoy, not to read. The M1 fill path behaves the same. **Remaining:**
catching and answering "nothing" would be kinder; not done in M2.

**Fixed (2026-10-02):** fill and save work runs inside `guarded` (`ServiceGuard.kt`): anything the parser or planner throws ends that request with "nothing" (a save with "could not save"); the service scope also has a `CoroutineExceptionHandler` as a last net. Cancellation still propagates. Pinned by `ServiceGuardTest`.

### AN31. The editor's draft holds typed secrets (Low, mitigated)
**Component:** `ui/edit/`. The draft (a password, a TOTP key, a card number,
a note) lives in the edit screen's `remember`ed state for as long as the
screen is open, and in the JVM `String`s the text fields use. **Mitigation:**
no `rememberSaveable`, `SavedStateHandle`, Intent extra or ViewModel state
(`EditorState.toString()` prints nothing); `MainActivity` handles
`orientation|screenSize|screenLayout|smallestScreenSize|keyboardHidden|keyboard|density|uiMode|fontScale`
itself so the draft survives them without being written anywhere (a locale
change or process death discards it); the lock wipe and leaving the screen
drop it; `FLAG_SECURE`; the window is excluded from autofill services. A
value that exists loads asynchronously, its field is read-only until it
arrives, and Save waits for loads. **Remaining:** JVM strings cannot be
zeroed (same as the master password field), and the draft is readable by
malware as the same user while unlocked (threat model section 4).

### AN32. An app can make HavenKeys save a login named like another service (Low, mitigated)
**Component:** `autofill_save` (`save.rs`, `app_fill.rs`). **Scenario:** an
app labels itself "GitHub" and shows a login form; the user confirms
Android's save sheet; the new login is titled "GitHub". **Mitigation:** the
login is bound to that app's package and every signing certificate and has
no website, so it never fills GitHub's site or app, and it never updates a
login that is not already matched to that app. **Remaining:** the title can
mislead the user later; the binding is shown only on the desktop.

### AN33. A save is lost if the vault locks before submission (Info, accepted)
**Component:** `SaveRequestReader`, `onSaveRequest`. `SaveInfo` is attached
only while unlocked and the save is performed only while unlocked. If the
vault locks (auto-lock, screen off) before the user submits, nothing is
saved and the user sees "HavenKeys locked before saving." No safety impact;
the user types the login again. The answer to Android comes within 8
seconds; a later failure shows a Toast.

### AN34. Username-first sign-in on Android 9 (Info, accepted)
**Component:** `SaveForm`. `FLAG_DELAY_SAVE` (username screen, then password
screen, saved as one login) exists on Android 10+. On Android 9 the password
step saves a login without a username unless that screen has one. The user
can add it in the editor.

### AN35. Keyboards may ignore the no-learning flag (Info, accepted)
**Component:** `NoPersonalizedLearning.kt`. Every editor field adds
`EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING`, and single-line secrets use
the password keyboard with autocorrect off. A keyboard app is free to ignore
both; a malicious keyboard sees everything typed in any app. HavenKeys
cannot detect or prevent that.

### AN36. M2 has not run on a device (Info, open)
The editor, conflict handling, the lock wipe in the editor, the keyboard
flags and the Autofill save sheet are verified by JVM unit tests, the host
Rust tests and `tests/round_trip.rs` (two phones editing one vault through a
real server), not on Android. The Android M2 checklist below is open.

## Audits and full verification

Run on 2026-09-24 at `2e0eb73` (WSL2, Linux 6.6).

| Command | Result |
|---|---|
| `cargo test` | Everything passes except the tests that need Postgres. `cargo test` stops at the first failing binary, so it was re-run with `--no-fail-fast`: 329 passed, 60 failed. Every failure is in the 10 `havenkeys-server` integration-test binaries or in `havenkeys-sync-client/tests/round_trip.rs`, and all of them panic with `Postgres is not reachable — run scripts/test-server.sh: … ConnectionRefused`. **Environmental:** there is no Postgres here, and this branch does not touch those tests. The passkey suites pass: `passkeys.rs` (11), core unit tests (117), `fuzz.rs` (8), `security.rs` (24), bridge (26) and protocol `messages.rs` (17) |
| `cargo test -p havenkeys-desktop` (not a default member) | 34 passed |
| `cargo clippy -p havenkeys-core -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host -p havenkeys-oslock -p havenkeys-server -p havenkeys-sync-client --all-targets -- -D warnings` | Clean |
| `cargo clippy -p havenkeys-desktop --all-targets -- -D warnings` | Clean |
| `cargo fmt --all -- --check` | Failed on test code in `passkey/rp.rs` (from `8973fec`). Formatted in `2e0eb73` (a `style:` commit); now clean |
| `pnpm -r test` | All pass: extension 172, protocol 11, desktop 10, ui 6, web 14 |
| `pnpm -r typecheck` | Clean |
| `pnpm --filter @havenkeys/extension build` | Builds |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok`. Two warnings are about `deny.toml` itself. The `Unicode-DFS-2016` allowance matches no crate. The `RUSTSEC-2024-0429` (glib) ignore matches nothing in cargo-deny's view, although `cargo audit` still reports that advisory (below). Both are left as they are |
| `cargo audit` | Fetched the advisory database (1269 advisories) and scanned 676 crates: **no vulnerabilities**. There are 7 allowed warnings, the same as #13: `proc-macro-error` and five `unic-*` crates are unmaintained, and `glib`'s `VariantStrIter` is unsound. All come through Tauri's Linux GTK stack. None comes from the passkey dependencies |
| `pnpm audit` | No known vulnerabilities found |

**Licenses of the new crates** (all in the `deny.toml` allow list):
* Apache-2.0 OR MIT: `p256` 0.14.0, `ecdsa` 0.17.0, `elliptic-curve` 0.14.1,
  `primefield` 0.14.0, `primeorder` 0.14.0, `crypto-bigint` 0.7.5, `rfc6979`
  0.6.0, `sec1` 0.8.1, `pkcs8` 0.11.0, `spki` 0.8.0, `der` 0.8.2,
  `signature` 3.0.0, `hybrid-array` 0.4.15, `base16ct` 1.0.0.
* MIT/Apache-2.0: `ff` 0.14.0 and `group` 0.14.0.
* Apache-2.0, dev-dependency only: `ciborium` 0.2.2, with `ciborium-io` and
  `ciborium-ll`.

### Re-run for the automatic upgrade (Task 10, `e409c7b`)

Full verification was re-run after the automatic-upgrade feature and its
final fix wave (WSL2, Linux 6.6). No dependency was added on this branch:
`git diff 07f2436..HEAD -- Cargo.lock pnpm-lock.yaml` is empty.

| Command | Result |
|---|---|
| `cargo test --workspace --exclude havenkeys-server --exclude havenkeys-sync-client` | 348 passed, 0 failed (core 118 unit + 129 integration across `account`/`fuzz`/`no_logging`/`passkeys`(26)/`replica`/`security`/`vault`/`writes`, bridge 31, protocol 25, native-host 8, oslock 5, desktop-lib 34). `havenkeys-server` and `havenkeys-sync-client/tests/round_trip.rs` still need Postgres, not available here — unchanged, environmental |
| `pnpm -r test` | 265 passed, 0 failed: extension 223, protocol 12, desktop 10, ui 6, web 14 |
| `pnpm typecheck`, `pnpm build:extension` | Clean; extension bundle builds |
| `pnpm lint:rust` (the same 7-crate clippy set, `-D warnings`) | Clean |
| `cargo fmt --all -- --check` | Failed on two test files this branch touched (`havenkeys-bridge/tests/bridge.rs`, `havenkeys-core/tests/passkeys.rs`). Formatted in `e409c7b` (`style: rustfmt`, untouched files left alone); now clean |
| `cargo audit` | Same 7 allowed warnings as above (`proc-macro-error`, five `unic-*`, `glib` `VariantStrIter`), all through Tauri's Linux GTK stack; **no vulnerabilities** |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok`; the same two `deny.toml`-only warnings as above |
| `pnpm audit` | No known vulnerabilities found |

**Secret-logging audit of this diff** (`git diff 07f2436..HEAD`): no new
`console.*`, `eprintln!`, or error string was added in `apps/extension`,
`apps/desktop`, or the Rust crates; the only new `format!` uses are in test
fixtures. `RecentFill` (`crates/havenkeys-core/src/vault.rs`) derives no
`Debug` impl. The new `Upgrade` and `UpgradeHint` types derive `Debug` but
hold only an item ID (`Uuid`), the same non-secret convention as
`PasskeyMatch`/`PasskeyCandidate`/`StagedPasskey`; `CreateCheck`'s derived
`Debug` composes `Suggestion`'s pre-existing redacted `Debug` (title and
username hidden) with `Upgrade`. `scripts/update-passkey-directory.mjs`
(a hand-run dev tool, not part of the shipped extension or desktop) logs
only a public entry count and the upstream commit hash of the Passkeys
Directory repository — no vault data.

### Re-run for automatic sign-in (Task 11, `b30e7b9`)

Full verification was run for the automatic-sign-in feature (WSL2, Linux
6.6.87.2-microsoft-standard-WSL2). No dependency was added on this branch:
`git diff f1c1b69..HEAD -- Cargo.lock pnpm-lock.yaml` is empty.

| Command | Result |
|---|---|
| `pnpm -r typecheck` | Clean: protocol, ui, desktop, extension, web |
| `pnpm -r test` | 328 passed, 0 failed: extension 285, protocol 13, ui 6, desktop 10, web 14 |
| `cargo test --workspace` | Stops at the first failing binary: `havenkeys-server`'s `admin.rs` (4 tests), unrelated to this branch — every one panics with `Postgres is not reachable — run scripts/test-server.sh: … ConnectionRefused` |
| `cargo test --workspace --exclude havenkeys-server --exclude havenkeys-sync-client` | 359 passed, 0 failed, across every other workspace crate (core, bridge, protocol, native-host, oslock, desktop-lib) |
| `cargo test -p havenkeys-sync-client --test round_trip` | 6 failed, all `Postgres is not reachable — run scripts/test-server.sh: Error { kind: Connect, cause: Some(Os { code: 111, kind: ConnectionRefused, message: "Connection refused" }) }`. **Environmental, pre-existing:** no Postgres is running here, and this branch does not touch the sync client; the same failure mode as the `havenkeys-server` tests above |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `pnpm audit --prod` | No known vulnerabilities found |
| `cargo audit` | Same 7 allowed warnings as the previous re-run (`proc-macro-error`, five `unic-*`, `glib` `VariantStrIter`), all through Tauri's Linux GTK stack, none from this feature; **no vulnerabilities** |

This task changed only Markdown documentation and `CLAUDE.md`; no source file
in `apps/` or `crates/` was touched, so the secret-logging and trust-boundary
review above still applies unchanged.

## Secret-logging and trust-boundary review

* **Where private keys appear.** `grep -rn "private_key\|SecretBytes" crates
  apps/desktop/src-tauri/src` finds hits only in `passkey/` (`mod.rs`,
  `webauthn.rs`), `secret.rs`, their tests, and the `pub use` in `lib.rs`.
  There are none in the desktop shell, the bridge, the protocol or the host.
  `Passkey` is serialized only as part of `ItemDetails`, and `ItemDetails`
  only through `seal_json` (encryption) in `vault.rs`. No other
  `serde_json::to_*` call in the core touches it. `PasskeyInfo`, the only
  passkey type the desktop commands return, has no key field.
* **Extension output.** `grep -rn "console\.\|innerHTML" apps/extension/src`
  finds nothing outside test files: `hygiene.test.ts`, which enforces the
  rule, and DOM fixtures in `content.test.ts` and `autofill.test.ts`.
* **No page-supplied field is used as a URL.** This was confirmed by reading
  `webauthn-handler.ts`, `bridge.ts` and `index.ts` in full:
  * The frame URL, top URL, origin and document ID come from the browser's
    sender data (`contentFrame`), with credentials, query and fragment
    stripped. The one exception is `fullUrl` (credentials stripped, query
    and fragment kept), which is sent only to the tab's top frame to find a
    card iframe (`bg_host_menu`) and never to the desktop.
  * The page's `rpId` is only ever sent as `rpId`. When it is absent, the
    default is the host of the sender URL.
  * The card's site label is `displayHost` of the sender URL.
  * The only URL the bridge builds is `chrome.runtime.getURL("passkey.html")`
    plus the token the background returned, which is checked against the
    token pattern.
* **`Debug` output.** No names, credential IDs, user handles or keys appear
  in any `Debug` output:
  * `Passkey` prints `Passkey(<redacted>)` (tested), and `SecretBytes`
    prints `SecretBytes(<redacted>)`.
  * `B64Url` prints its length only.
  * `Registration` and `Assertion` print `Registration(..)` and
    `Assertion(..)`.
  * `StagedPasskey`, the core and protocol `PasskeyMatch`, and
    `PasskeyCandidate` print the item ID only.
  * `CreateCheck` prints `Suggestion`s, whose `Debug` shows the ID and
    strength only.
  * The protocol's `Request` and `ResultBody` print the request type only.
  * `RpContext` derives `Debug` and would print the rpId and origins. Those
    are browsing metadata, not secrets, and nothing prints them.
* **Trust boundaries** match `security-model.md` §15:
  * the page script is treated as hostile;
  * the bridge parses exact shapes;
  * the background never takes a URL from a message;
  * the core re-derives everything from the URL.

## Verified properties (this phase)

Each property has a test that fails if it stops holding.

* A passkey for github.com cannot be used from evil.com, a look-alike, a
  public suffix, plain http (other than localhost) or a cross-site frame
  (`rp.rs`, `tests/passkeys.rs`, `bridge.rs`, `fuzz_authorize_rp`).
* An item ID or credential ID that is not bound to the requested rpId gets
  `denied`, and at the bridge that looks the same as an unknown one (A2p).
* A locked vault signs nothing and creates nothing (A3p).
* Every registration gets a fresh private key and a random 16-byte
  credential ID. A damaged stored key gives `Corrupted`, never a signature.
* Authenticator data, flags (`0x1d`/`0x5d`), counter 0, the zero AAGUID,
  `none` attestation and `clientDataJSON` are byte-exact. The COSE and SPKI
  keys agree, and signatures verify.
* Logins saved before passkeys existed still open. Editing a login keeps its
  passkeys. The limit of 8 per login is enforced. Re-registering an account
  replaces its passkey across logins. An offline create stores nothing.
* Oversized, malformed and unknown-field passkey messages are rejected
  without a panic, in Rust and in TypeScript, and results are validated.
* The page script falls back to the browser on every path the user did not
  choose, keeps working when the page replaces globals, and honours
  `AbortSignal`.
* The background signs or creates only after a pick from the session's own
  frame, only for an offered passkey or login, and one at a time. Sessions
  are bound to the tab, frame, document and origin, and end when the vault
  locks.
* The passkey script group registers independently of the inline one.
* A conditional `create()` is only ever the automatic upgrade for the
  login and site just filled: a different login, a different site (even a
  look-alike or a subdomain outside the login's own rules), or no fill in
  the last 5 minutes all deny it and fall back (A1u, A2u;
  `upgrade_is_per_site_and_never_for_look_alikes`,
  `a_fill_on_one_site_is_no_upgrade_on_another_site_of_the_same_login`,
  `conditional_create_needs_auto_for_exactly_that_login`).
* A silent save never replaces an existing passkey, and one password fill
  grants at most one (A3u;
  `conditional_create_never_replaces_the_filled_logins_own_passkey`,
  `one_fill_grants_one_silent_passkey`). The fill memory is capped at 16
  entries and dropped on lock (`fill_memory_is_dropped_on_lock`).
* `has_passkey_for_page` runs the same `authorize_rp` check as every other
  passkey lookup and returns nothing but a boolean; it never names an item
  or a credential.
* The Passkeys Directory match is a plain host-suffix comparison that a
  look-alike domain (`github.com.evil.com`) never satisfies against
  `github.com`, and the help link opens only on a trusted click.

## Manual checklist (verification pending)

No browser is available in the environment this was built in, so none of
these checks has been run. Record the results here when they are.

Setup: build and load `apps/extension/dist/chrome` (and `dist/firefox`).
Run the desktop app with Settings → Browser extension on, and turn on
in-page suggestions (the host grant).

| # | Check | Chrome | Firefox |
|---|---|---|---|
| M1 | Firefox 128: `scripting.registerContentScripts` accepts `world: "MAIN"`. The page script runs at `document_start` **before the site's own scripts**, so a site that captures `navigator.credentials` early still gets the wrapper. The isolated bridge is listening before the first request | n/a | Not yet run |
| M2 | The same ordering on Chrome: the wrapper runs before site scripts, and the bridge listens before the first request | Not yet run | n/a |
| M3 | webauthn.io: register a passkey, get the save card, click **Save to HavenKeys**. The site accepts the attestation (`none`, ES256) | Not yet run | Not yet run |
| M4 | webauthn.io: modal sign-in, get the chooser, pick a passkey. The site verifies the assertion | Not yet run | Not yet run |
| M5 | webauthn.io: conditional sign-in (passkey autofill). The passkey appears first in the field menu, and picking it signs in. Picking from the browser's own UI instead also works | Not yet run | Not yet run |
| M6 | github.com: add a passkey, sign out, and sign in with it | Not yet run | Not yet run |
| M7 | google.com: add a passkey and sign in with it | Not yet run | Not yet run |
| M8 | "Use another device" reaches the browser's own UI, on create and on get. Note any site that refuses because user activation was lost (PK4) | Not yet run | Not yet run |
| M9 | Offline create: stop the server, then create. The card shows the offline message, nothing is stored, and "Use another device" works | Not yet run | Not yet run |
| M10 | Locked vault: a modal request shows the locked card, and unlocking within 60 s turns it into the chooser or save card. A conditional request shows only the browser's UI | Not yet run | Not yet run |
| M11 | Locking the vault while the card is open closes it with `NotAllowedError`. After the port idles out, a pick is refused as locked instead (PK12) | Not yet run | Not yet run |
| M16 | Leave a login page with a conditional request open for 3 minutes: HavenKeys passkeys are still in the field menu. Open a modal chooser, stop the worker from the browser's extension page: the card closes within 20 s (or at once on Cancel) and the browser's own UI takes over (PK17) | Not yet run | Not yet run |
| M17 | A site whose `excludeCredentials` names a HavenKeys passkey: the "already saved" card appears, and the site reports "already registered" only after **Close** (PK19) | Not yet run | Not yet run |
| M12 | A site with a strict CSP (for example github.com): the MAIN-world script runs and the passkey frame loads | Not yet run | Not yet run |
| M13 | Desktop: the login shows its passkey (site, account, created date) and the list shows the badge. Deleting the passkey asks for confirmation, and afterwards the site no longer accepts it | Not yet run | Not yet run |
| M14 | Re-registering the same account replaces the passkey (one entry in the desktop), and the site accepts the new one | Not yet run | Not yet run |
| M15 | On Chromium, a translucent overlay over the passkey card cannot get clicks through | Not yet run | n/a |
| M18 | google.com: sign in with a saved password, let Google's own "create a passkey" prompt fire as a conditional `create()`. With **Add passkeys automatically** on: no card, a "Passkey saved to HavenKeys" notice, and the login gains a passkey. With the setting off: the "Add a passkey?" card opens instead, that login preselected | Not yet run | Not yet run |
| M19 | A Passkeys Directory site with a saved password and no passkey (for example github.com): open the field menu and confirm the "`<name>` supports passkeys" / "How to add one" row appears and its link opens the site's own help page in a new tab. After saving a passkey for it, confirm the row is replaced by "You have a passkey for `<site>`" | Not yet run | Not yet run |

---

# Security Review: Desktop Auto-Update

**Date:** 2026-09-27
**Scope:** `apps/desktop/src-tauri/src/{updates.rs, updater.rs, tray.rs}`, the
banner and Settings → Updates UI (`apps/desktop/src`), the updater
configuration in `tauri.conf.json` / `tauri.bundle.conf.json`,
`apps/desktop/src-tauri/windows/hooks.nsh`, and
`.github/workflows/release.yml`. Design:
`docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md`.
**Method:** self-review of the implementation against the design, the test
suites written alongside it, and the audits below. No release has yet been
cut with this pipeline, so nothing here has run end to end against a real
GitHub Release (see the manual checklist at the end).

> This is an internal review, not an independent security audit.

## Summary

Component: **Desktop updater** (`tauri-plugin-updater` 2.12.0, driven only
from Rust; no plugin permission reaches the webview). No critical issues were
found. The design's central property — a bad or missing signature aborts
before anything is installed or the vault is touched — is enforced by the
plugin's own `download()` (confirmed in its source: it verifies the minisign
signature before returning any bytes) and by the straight-line order in
`updater.rs::install` (download → verify → lock → install). The residual risk
worth naming plainly is the one every code-signed auto-updater carries: whoever
holds the signing key can ship code to every installation.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| UP1 | Info | Updater | Malicious update from a compromised GitHub account, repository, CDN or network path | **Mitigated**: `Update::download` verifies the minisign signature against the public key in `tauri.conf.json` before returning bytes; a bad or missing signature aborts with nothing installed and the vault untouched |
| UP2 | High | Updater (key custody) | Theft of the updater's private key and its passphrase lets the holder ship code to every installation, including the Rust core that holds the vault key | Accepted, inherent to any signed-updater design; mitigated operationally (key + passphrase only in repository secrets and an offline backup, never committed; releases stay drafts until reviewed and published; rotation procedure documented) |
| UP3 | Info | Updater | Downgrade / replay of an old, still validly signed release, including a crafted (unsigned) `latest.json` pairing a high version with an old release's URL and signature | **Mitigated**: `requireSignedVersion: true` rejects any artifact whose signed version (in the signature's trusted comment) differs from the announced one or is missing; the default comparator then installs only a strictly greater version. Releases must be built with CLI ≥ 2.12.0 (UM6) |
| UP4 | Info | Updater / UI | Release notes from `latest.json` rendered where a script tag or event handler could execute if treated as markup | **Mitigated by design**: notes are truncated to 4,000 characters and rendered as a text node, never HTML; `updates.rs` has a unit test asserting an `<img onerror=...>` payload survives truncation as literal text |
| UP5 | Medium | Windows install / native host | Browsers keep `havenkeys-native-host.exe` running (it retries `connectNative` while the desktop app is gone); Windows cannot overwrite a running executable, so an in-place update could fail while a browser holds the file open | **Mitigated**: an NSIS `NSIS_HOOK_PREINSTALL` hook (`windows/hooks.nsh`) runs `taskkill /F /IM havenkeys-native-host.exe` before files are copied, so the installer never meets a locked file; this also fixes today's manual in-place installs. **Unverified**: whether the extension's next `connectNative` successfully restarts the host after the update (M20 below) |
| UP6 | Info | Renderer exposure | The renderer could be given more updater surface than it needs | **Mitigated by design**: no `tauri-plugin-updater` permission is granted; the capability file allows only the four named commands (`update_status`, `check_for_update`, `install_update`, `set_update_auto_check`); the webview never calls the plugin |
| UP7 | Info | Privacy | GitHub sees every automatic and manual update check (IP address, time, that HavenKeys is checking) | Accepted, documented; the automatic check can be turned off in Settings → Updates |

## Details

### UP1. Malicious update (Info, mitigated)
**Attack scenario:** an attacker who controls the GitHub account, the
repository, a CDN in front of it, or a network path in between serves
arbitrary bytes as "the next release" — anything from a modified installer to
a completely unrelated binary.
**Mitigation:** `tauri-plugin-updater`'s `Update::download` verifies the
minisign signature of the downloaded artifact against the public key embedded
in `tauri.conf.json` (key ID `1D3AD839EC826779`) and only returns bytes once
that check passes (confirmed by reading the plugin's source, not just its
docs). `updater.rs::install` treats any `Err` from `download` identically to a
network failure: `fail_install`, no lock, no install, no restart. TLS to
`github.com` and its download redirect host raises the bar for tampering
in-flight, but the signature — not TLS — is what actually gates installation.
**Residual:** none beyond the signing key itself (UP2).

### UP2. Theft of the signing key (High, accepted)
**Attack scenario:** whoever obtains both the private key file and its
passphrase can sign a release that every current and future installation will
verify successfully and run — including the Rust core that decrypts vault
items. This is not a bug to fix; it is the trust root the entire update
mechanism rests on, the same as for any code-signed auto-updater (1Password,
browsers, OS updaters included).
**Mitigations:**
* The key was generated once, by the project owner, on their own machine —
  never by an agent, never in CI.
* The private key and its passphrase exist only as GitHub repository secrets
  (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and in an
  offline backup; they are never committed, logged, or printed in a build.
* Releases are created as **drafts**; `/releases/latest` (what the updater
  polls) ignores drafts, so a forged or accidentally-triggered release build
  cannot reach any installation until a human reviews and publishes it.
* A documented rotation procedure (`development.md`, "Releases and updates")
  lets a compromised key be replaced: one release signed with the old key
  whose config already trusts the new public key, then every later release
  signed with the new key only.
**Severity:** High, because a successful theft has no cryptographic
containment — it is a full compromise of every installation that later
updates. **Residual:** accepted as a trust assumption inherent to the design;
see `threat-model.md` T10.

### UP3. Downgrade / replay (Info, mitigated)
**Attack scenario:** an attacker replays an old, genuinely signed release
(one with a known vulnerability) hoping to push an installation backwards.
`latest.json` is not signed — only the artifacts are — so the attacker can
serve a manifest that announces `99.0.0` but points at an old release's URL
and its still-valid signature.
**Mitigation:** the updater config sets `requireSignedVersion: true`. The
Tauri CLI (2.12.0 and later) writes the app version into each signature's
trusted comment, which the signature covers; `Update::download` rejects an
artifact whose signed version differs from the version the manifest announced,
or that carries no version at all (plugin source, `verify_signed_version`).
The default comparator then only reports an update when the announced (and
now proven) version is strictly greater than `app.package_info().version`.
`updater::tests::the_updater_refuses_downgrades_and_unsigned_versions` reads
the shipped `tauri.conf.json` through the plugin's own `Config` and asserts
the flag is on and downgrades are not allowed.
**Residual:** a release built with an older CLI would carry no version and be
refused by every install, not accepted; the pre-publish check (UM6,
`development.md`) catches that before publishing.

### UP4. Notes injection (Info, mitigated by design)
**Attack scenario:** a release's notes (`latest.json`'s `notes` field, taken
from the GitHub release body at build time) contain markup or a script
payload, hoping the desktop UI executes or renders it as HTML.
**Mitigation:** `updates.rs::truncate_notes` only truncates (at a character
boundary; `NOTES_LIMIT = 4000`) and does not alter content — its own test
(`notes_are_truncated_on_a_character_boundary`) feeds it
`<img src=x onerror=alert(1)>` and asserts the string survives unchanged. The
UI is responsible for rendering that string as a text node rather than HTML,
matching the project-wide rule against `innerHTML`/`dangerouslySetInnerHTML`.
**Residual:** none identified; enforced by the same UI-hygiene convention as
every other user-controlled string in the app.

### UP5. Windows: the native host holds the file the installer needs to write (Medium, mitigated; unverified)
**Attack/failure scenario (not an attacker, an ordinary case):** a browser
extension keeps `havenkeys-native-host.exe` running — it retries
`connectNative` while the desktop app is closed for the update — and Windows
refuses to overwrite a running executable. Without a fix, an in-place update
(and, separately, today's manual in-place reinstall) could fail partway
through, leaving a broken install.
**Mitigation:** `windows/hooks.nsh` registers `NSIS_HOOK_PREINSTALL`, which
runs `taskkill /F /IM havenkeys-native-host.exe` (reaching only this user's
own processes, matching a per-user install) before the installer copies any
file.
**Remaining limitation:** this has no dedicated automated test — it is NSIS
script, not Rust or TypeScript — and the design explicitly calls out that the
extension reconnecting afterward (a fresh `connectNative` spawning a new host
process) must be confirmed by hand. See the manual checklist, UM4.

### UP6. Renderer exposure (Info, mitigated by design)
The updater plugin is registered in `lib.rs` but its own permission is never
added to the capability file (confirmed: `grep -rn "updater" apps/desktop/src-tauri/capabilities/`
finds only the four `allow-<command>` entries for `update_status`,
`check_for_update`, `install_update` and `set_update_auto_check`, the same
allowlist convention every other command follows). The renderer cannot call
`check()`, `download()` or `install()` on the plugin directly, and has no way
to reach the plugin's own JS API.

### UP7. Privacy: GitHub sees checks (Info, accepted)
Every automatic (once at start, then every 24 h) and manual check is an HTTPS
request to GitHub, which learns the IP address, the time, and that a HavenKeys
installation exists and is checking. This is inherent to using GitHub Releases
as the distribution point. **Mitigation:** Settings → Updates can turn the
automatic check off entirely (`autoCheck: false`); a manual check or install
then only reaches GitHub on an explicit click.

## Audits

Run on 2026-09-27 (WSL2, Linux 6.6.87.2-microsoft-standard-WSL2), after Tasks
1–6 of this branch (`tauri-plugin-updater = "2"`, resolved to 2.12.0, is the
only new Cargo dependency; no new npm dependency was added).

| Command | Result |
|---|---|
| `cargo test -p havenkeys-desktop` | 78 passed, 0 failed, including every `updates::tests::*` state-machine and settings-file test (found update → available, install refused outside `Available`, concurrent checks collapse, background failures stay silent while manual ones report, progress reported in whole-percent/mebibyte steps, `updates.json` default/round-trip/corrupt-file/unknown-fields-ignored, file mode 0600 on Unix, notes truncated on a character boundary without splitting a multi-byte character, `can_install_in_place` per platform/environment) |
| `pnpm --filter @havenkeys/desktop test` | 8 files, 54 tests passed, 0 failed |
| `cargo clippy -p havenkeys-desktop --all-targets -- -D warnings` | Clean |
| `pnpm audit` | No known vulnerabilities found |
| `cargo audit` | Same 7 pre-existing allowed warnings as every earlier review in this file (`proc-macro-error`, five `unic-*` crates, `glib`'s `VariantStrIter`), all through Tauri's Linux GTK stack, none introduced by the updater; **no vulnerabilities** |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok`. The same two `deny.toml`-only warnings as every earlier review (`Unicode-DFS-2016` matches no crate; the `RUSTSEC-2024-0429` ignore is not currently matched by `cargo-deny`'s view although `cargo audit` still reports it). No new license or advisory needed adding to `deny.toml` |

**New dependency tree** (`cargo tree --manifest-path apps/desktop/src-tauri/Cargo.toml -e normal -p tauri-plugin-updater`):
`tauri-plugin-updater` 2.12.0 pulls in `reqwest` 0.13, `minisign-verify` 0.2.5
(the signature check itself), `flate2`, `infer`, `cfb`, `dirs`, `base64`,
`futures-util`, `http`, and their transitive dependencies — all already
covered by `cargo deny check licenses`' passing run above, so none needed
adding to the allow list.

**`THIRD-PARTY-NOTICES.md`:** unchanged. That file lists only the embedded
fonts and the Passkeys Directory data snapshot — assets bundled into the app
whose licences require an accompanying notice — not a general enumeration of
Rust or npm dependencies (those are covered by the blanket statement at its
top and enforced by `cargo deny check licenses`). `tauri-plugin-updater` and
its dependency tree add no embedded font, data file, or copyleft-beyond-MPL
license that would need a new entry.

## Verified properties

* A bad or missing signature never reaches `install()`: `install_update`
  aborts through `fail_install` on any `Err` from `download`, before the vault
  lock is touched (`updater.rs::install`).
* `install` is reachable only from `Phase::Available` (`Machine::begin_install`
  returns `NotAvailable` otherwise), so a second click, or a click with
  nothing on offer, cannot start a second download or install.
* The vault is locked (`state.lock(app, "update")`) only *after* a
  successfully verified download and only *before* the platform install step
  runs — never before verification, never after installation has already
  begun.
* Concurrent checks collapse into one (`begin_check` returns `false` while
  `Checking`, `Downloading` or `Installing`); a check cannot interrupt an
  install in progress.
* No updater plugin permission is granted to the renderer; only the four named
  commands are.
* Release notes cannot inject markup: truncation preserves content exactly,
  and the UI renders it as text.
* Debug builds never schedule an automatic check (`cfg!(debug_assertions)`
  short-circuits `schedule`).

## Remaining limitations

* **Installers are still not OS code-signed.** Windows SmartScreen and macOS
  Gatekeeper warn on the *first* manual install of an updater-enabled version
  (0.9.0 itself, and any user still on 0.8.0 or earlier); the OS has no way to
  vouch for that first binary. Once installed, every *automatic* update after
  that is verified by the minisign signature instead, but the initial trust
  decision is still the user's own.
* **`.deb`/`.rpm` installs are not updated in place.** Those users get a
  notice and a link to the release page and must download and install the new
  package themselves; only Windows, macOS and the AppImage self-update.
* **GitHub sees every check**, automatic or manual, as documented in UP7.
* **The daily timer is a monotonic sleep** (`tokio::time::sleep(CHECK_INTERVAL)`
  in `updater.rs::schedule`), not a wall-clock schedule. A machine that
  suspends often can go well over 24 hours between two automatic checks,
  because sleep time may not fully count toward the timer depending on the
  runtime and platform; a machine that is rarely suspended keeps to
  approximately 24 hours. This affects only how promptly an update is
  *offered*, never whether an installed one is verified.
* **The install order (verify → lock → install) is enforced by the
  straight-line structure of `updater.rs::install`, not by a dedicated
  regression test.** A future refactor of that function could reorder the
  steps without a test failing to catch it. Worth adding if that function is
  touched again.
* **Nothing in this section has run against a real, published GitHub
  Release yet.** See the manual checklist below.

## Manual checklist (verification pending)

No release has been cut with the signing pipeline in place, so none of these
checks has been run. Record the results here when they are (Task 8 of the
implementation plan).

| # | Check | Windows | macOS | Linux (AppImage) |
|---|---|---|---|---|
| UM1 | Publish a throwaway 0.9.0, then a signed 0.9.1: the running 0.9.0 offers 0.9.1 in the banner, the notes shown match the release, clicking Update downloads, verifies, locks the vault, installs, and restarts on 0.9.1 | Not yet run | Not yet run | Not yet run |
| UM2 | A release whose asset is tampered with (or re-signed with a different key) after publishing is refused: the banner shows the fixed failure message, nothing is installed, and the vault is not locked | Not yet run | Not yet run | Not yet run |
| UM3 | Settings → Updates: "Check for updates automatically" off stops the daily timer; "Check now" still works and reports "You're up to date" / offers the update / shows the error message correctly | Not yet run | Not yet run | Not yet run |
| UM4 | Windows only: with a browser open and `havenkeys-native-host.exe` running (extension installed and connected), install an update; the installer completes without a file-in-use error, and the extension's next request successfully starts a new host process (UP5) | Not yet run | n/a | n/a |
| UM5 | `.deb`/`.rpm` installs show the "Download" banner/button instead of "Update", and it opens the release page at the fixed `RELEASES_URL` rather than any URL taken from `latest.json` | n/a | n/a | Not yet run (`.deb`/`.rpm` path) |
| UM6 | Before publishing each release: every `.sig` asset's trusted comment (`base64 -d <file>.sig`, third line) contains `version:<the release version>` (UP3); an install refuses a manifest whose version does not match | Not yet run | Not yet run | Not yet run |

---

## Full-application security scan (2026-09-27)

**Date:** 2026-09-27
**Scope:** the whole application, reviewed by six parallel internal reviewers, one per trust boundary: crypto and vault core (`crates/havenkeys-core`), the desktop↔extension bridge and native messaging (`crates/havenkeys-bridge`, `havenkeys-protocol`, `havenkeys-native-host`), the browser extension (`apps/extension`), the Tauri desktop app (`apps/desktop/src-tauri`, `apps/desktop/src`), the server and sync client (`crates/havenkeys-server`, `crates/havenkeys-sync-client`), and dependencies/build/release supply chain. Full reports:
`.superpowers/security-scan/{cr-crypto-vault,br-bridge-native,ex-extension,dt-desktop,sv-server-sync,sc-supply-chain}.md`.
**Method:** each reviewer read their area's source in full, traced untrusted input (a malicious web page, a copied vault file, a compromised extension, a malicious or compromised server operator, a local same-user process, a crafted import file) to its sensitive sink, and confirmed candidate findings against the code, with throwaway scratch scripts or tests outside the repo where that was cheap. A finding already recorded elsewhere in this document as accepted or fixed is marked KNOWN rather than reported again, unless the existing entry proved incomplete.

> This is an internal review carried out by the project's own development process, not an independent third-party security audit (CLAUDE.md §50).

### Summary

No Critical or High findings. Five Medium findings, one of which was found independently from two different trust boundaries and describes the same underlying gap: CR1 and SV-1 both show that a hostile or compromised server can splice an old item overview (URL rules, `auto_sign_in`) with a newer details blob (the password), or the reverse, so the current secret reaches an origin the item no longer names. Fourteen Low and six Info findings round out the rest. Two findings were already fixed and one already mitigated by the time this scan ran: the Windows NSIS `taskkill` path-planting issue (DT5) is fixed, the release workflow now pins `--locked` on both build steps (SC2, fixed), and the release workflow now compiles without the signing secrets present (SC1, mitigated). The four Medium findings with a code fix on this branch are fixed: CR1/SV-1 (details blob bound to its overview), SV-2 (pull and fetch answers cut by bytes), EX-01 (opaque-origin documents) and BR-1 (Windows pipe squatting, unverified on Windows); SV-3's paging and memory part is fixed and its quota part is open. Everything else is open (planned) or accepted, as detailed below.

| ID | Title | Severity | Component | Status |
|---|---|---|---|---|
| CR1 / SV-1 | A hostile server can splice an old item overview (URL rules) with a newer details blob (password), or vice versa, so the current secret reaches an origin the item no longer names | Medium | Core sync / server | Fixed |
| CR2 | A corrupt or missing settings blob silently falls back to `auto_sign_in`/`auto_passkey_upgrade` = on | Low | Core vault | Open (planned) |
| CR3 | A crafted `.1pux` with millions of ZIP central-directory entries costs ~7× its size in memory before any size check runs | Low | Core import | Open (planned) |
| BR-1 | Windows pipe squatting: the recorded impact on P10 understates that typed and generated passwords reach the squatter | Medium | Protocol (Windows) | Fixed in code, not yet verified on Windows |
| BR-2 | The per-item password-change rate-limit budget is spent even when the write is denied or fails | Low | Bridge | Open (planned) |
| EX-01 | Popup Fill writes credentials into a CSP-sandboxed (opaque-origin) document on the login's own origin | Medium | Extension (popup) | **Fixed** |
| EX-02 | "Visible" field detection admits off-screen/clipped/covered/near-transparent inputs; fill and auto-submit ignore a cross-origin form `action` | Low | Extension (autofill) | Open (planned) |
| EX-03 | Menu/save tokens live in the iframe `src`, so a page can re-frame `menu.html#token`/`save.html#token` itself; the frames.ts tamper guards are moot and Firefox relies on the arming delay alone | Low | Extension (menu) | Open (planned) |
| EX-04 | Save-prompt oracle on a page-planted username, after the password is already known to the attacker | Info | Extension | Accepted (residual of F1) |
| DT1 | A server session can be installed after the vault locks; a locked vault then stays "online" and account commands still work | Low | Desktop (sync) | Partly fixed (lock re-checked under the vault guard before a session is installed; no epoch check; device list/revoke still skip `is_unlocked()`) |
| DT2 | Extension-driven `open_item` resets the auto-lock idle timer | Low | Desktop (bridge hook) | Open (planned) |
| DT3 | On Windows, the vault file and the keychain-stored Secret Key both roam with the user profile by default | Low | Desktop (storage) | Open (planned) |
| DT4 | Linux screen-lock detection disables itself for the rest of the run after three transient probe failures | Low | Desktop (oslock) | Open (planned) |
| DT5 | NSIS pre-install hook ran `taskkill` without a path (binary planting from the installer's folder) | Low | Desktop (Windows installer) | **Fixed** |
| DT6 | An unreadable `device.json` is silently replaced, discarding a file-stored Secret Key and the device ID | Info | Desktop (device store) | Open (planned) |
| SV-2 | Pull pages are limited by row count, not bytes; an ordinary vault can permanently exceed the client's 17 MiB cap and sync stops for good | Medium | Server / sync client | Fixed (server: `a_vault_of_big_items_pulls_in_pages_under_the_cap`, `a_fetch_of_big_items_names_what_did_not_fit`; client re-asks `unanswered`) |
| SV-3 | Any account holder can exhaust the server's memory and disk (no byte bound on pull/fetch, no per-account quota) | Low | Server | Partly fixed: byte-budgeted paging and per-request memory; per-account quota still open (planned) |
| SV-4 | Unauthenticated login is an Argon2id amplifier that can starve the DB connection pool (no wait timeout) | Low | Server (auth) | Open (planned) |
| SV-5 | Restoring a server backup leaves every replica silently out of step (cursor ahead, ghost items, permanent conflicts) | Low | Server / sync | Open (planned) |
| SV-6 | The sync client honours system/environment proxies, contrary to its manifest comment | Info | Sync client | Accepted (same-user precondition) |
| SV-7 | A malicious server can force maximum-cost Argon2id and an unbounded pull loop | Info | Server / sync client | Accepted (subsumed by S5/S6) |
| SV-8 | `admin new-account --server-url` accepts `http://localhost.evil.com` by prefix match | Info | Server (admin CLI) | Accepted (client fails closed regardless) |
| SC1 | Signing secrets were exposed to the entire `cargo build`/`tauri build` process, not just the sign step | Medium | CI (release workflow) | **Mitigated** |
| SC2 | The main release build did not pin `cargo build` to `--locked` | Low | CI (release workflow) | **Fixed** |
| SC3 | No automated secret scanning in CI | Info | CI | Open (planned) |

### Details

#### CR1 / SV-1. Overview/details splice across revisions (Medium, fixed)
**Status:** fixed. A details blob's AAD now includes the SHA-256 of the overview blob written with it (`BlobContext::item_details`), so a spliced pair fails to open and is skipped. Test: `sync::tests::a_spliced_item_is_refused`. Whole-item replay of an older version stays as recorded (#9, S5, T1b). Details written before the fix do not open; the vault is reset, no migration.
**Attack scenario:** a hostile or compromised `havenkeys-server` operator keeps every blob version it has ever received. It later serves an item made of an old overview (stale URL rules, or a stale `auto_sign_in` flag) paired with a newer details blob (the current password), or the reverse. Each blob authenticates on its own — the AEAD's AAD binds only purpose, vault ID and item ID, never a revision or the sibling blob — so `check_item_bytes` accepts the pair as an ordinary update. The item then matches an origin the user has since removed, and `fill_for_page`/`totp_for_page` hand that origin the *current* secret. The user still has to pick the suggestion; there is no silent fill.
**Evidence:** CR1 CONFIRMED via a throwaway probe crate linking the real `havenkeys-core` (`scratchpad/cr/probe`); SV-1 CONFIRMED independently via a separate throwaway crate (`scratchpad/sv/mix`) — both reproduce the same splice through the public core API and land the *new* password on the *removed* domain.
**Suggested fix:** bind the two blobs of one write together — a random per-write id, or `SHA-256` of one blob, carried in both plaintexts (or both AADs) and checked in `check_item_bytes` — plus the per-item monotonic revision floor already proposed for plain replay (`docs/server-sync.md` §7, `docs/threat-model.md` T1b).
**KNOWN?** Item replay in general is already documented and accepted (#9, S5, T1b); the cross-revision *splice* and its confidentiality effect (rather than just integrity/availability) were not recorded before this scan.

#### CR2. Settings fallback to "on" when the settings blob is missing or won't open (Low, open)
**Attack scenario:** someone who can write `vault.sqlite3` but has no keys (a tampered or restored backup, or another local account with file access) corrupts or deletes the `settings` row. `open_session` discards the failure and silently falls back to `Settings::default()`, which is `auto_sign_in = true` and `auto_passkey_upgrade = true` — switching a user who had turned both off back to automatic behavior, with nothing shown in `status()` or `damaged_items`.
**Evidence:** CONFIRMED — a probe corrupts the settings row, unlocks again, and observes `auto_sign_in=true auto_passkey_upgrade=true` with `damaged_items: 0`.
**Suggested fix:** on a settings blob that exists but fails to open or validate, fall back to the most restrictive values and surface a `damaged_settings` flag; keep `Settings::default()` only for the genuine "no row yet" case.

#### CR3. Unbounded ZIP central-directory parsing in `.1pux` import (Low, open)
**Attack scenario:** a crafted `.1pux` (an in-scope hostile input) is a ZIP64 archive with an empty export and a million empty `files/*` entries. `ZipArchive::new` builds per-entry metadata for the whole central directory before any HavenKeys size or count check runs, costing about 7× the archive's size in memory — enough to exhaust memory on a small machine well inside the existing 256 MiB archive cap.
**Evidence:** CONFIRMED — a 98 MB / 1,000,000-entry crafted `.1pux` parsed at 717 MB peak RSS in 933 ms; extrapolated to the 256 MiB cap, roughly 1.8 GB.
**Suggested fix:** read the end-of-central-directory record (or use the zip crate's own entry-count limit) before calling `ZipArchive::new`; refuse more than about 10,000 entries.

#### BR-1. Windows pipe squatting: understated impact on the existing P10 entry (Medium, fixed in code, not yet verified on Windows)
**Attack scenario:** another account on the same Windows machine pre-computes and creates the predictable named pipe (a hash of the profile path) before HavenKeys starts, or after it exits; the client side of `Endpoint::connect` never verifies who owns the server end of the pipe it opens. The existing P10 entry above (Native Messaging Phase) says the squatter "would receive your page URLs and could return fake answers. It could not read your vault." That is incomplete: every login form the victim submits sends `check_login {url, username, password}` with the **plaintext typed password**, automatically, before any save prompt, and a confirmed save sends `save_login` the same way. The squatter can also answer `generate_password` with a password of its own choosing, or answer `fill_item`/`autoSubmit: true` with attacker-controlled credentials to silently sign the victim into an attacker account. This is a cross-user credential compromise, in scope because P1 already treats other local users as attackers.
**Evidence:** LIKELY (Windows is not runnable in this environment); traced through `crates/havenkeys-protocol/src/endpoint.rs:103-147`, `crates/havenkeys-native-host/src/lib.rs:95-110`, and `apps/extension/src/background/inline-handler.ts:241-243`.
**Suggested fix:** on Windows, have `Endpoint::connect` check the pipe server process's token SID against the caller's own SID (`GetNamedPipeServerProcessId` + `GetSecurityInfo`) before trusting the connection; surface a failed `serve()` in Settings → Browser extension instead of swallowing it.
**Fix:** `Endpoint::connect` on Windows reads the pipe server's process ID (`peer_creds().pid()`) and refuses the pipe (`PermissionDenied`) unless that process's token user SID equals ours (`havenkeys_protocol::win_identity::process_is_current_user`); the desktop bridge checks each client the same way in `peer_is_same_user`. Every failure path (no PID, process not openable, token unreadable) answers "not the same user". P10 is fixed by the same change. Residual: a squatter can still deny service by holding the name. **Not yet verified on Windows:** with a second Windows account, create the pipe name first; the extension must report HavenKeys unreachable and send nothing.
**Note on P10 (this file, Native Messaging Phase, above):** P10's recorded impact — "could not read your vault" — remains true but is incomplete on its own: a squatter cannot read the vault, but it does receive plaintext passwords the user types or generates during the squatted session, and can plant fake fills. P10 is fixed in code by BR-1 (the client now checks the pipe server's user too), unverified on Windows.

#### BR-2. Per-item rate-limit budget spent before authorization or success (Low, open)
**Attack scenario:** `allow_item_update` runs before dispatch, so a denied request (wrong origin, unknown item ID) or a failed write (server offline) still spends the item's 10-minute budget. A legitimate retry after a transient server outage is then refused as `rate_limited`, or a compromised extension/same-user process (P9, accepted) can jam a known item's password-update budget indefinitely without ever touching it. Availability only; no secret or integrity impact.
**Evidence:** CONFIRMED by the existing test `password_updates_are_limited_per_item` (`crates/havenkeys-bridge/tests/bridge.rs:692`).
**Suggested fix:** check the per-item budget without recording it before dispatch; record it only after `(self.inner.save)(…)` returns `Ok`.

#### EX-01. Popup Fill into a CSP-sandboxed document (Medium, fixed)
**Attack scenario:** a site hosts user-uploaded HTML under `Content-Security-Policy: sandbox allow-scripts` on its own origin — a standard way to host untrusted content "safely" same-origin. The document gets an opaque origin, but `tab.url` still reports the real site, so the HavenKeys popup offers the real saved login. `fillTab` injects the content script via `chrome.scripting.executeScript`, which does run inside sandboxed documents, and `handleFill` compares against `location.origin` (the document's real origin, not `"null"`), so the fill proceeds. The attacker's script running in the sandboxed document then reads the filled username, password, and (with Code → fill) TOTP code. The in-page, non-popup fill path is already safe: it rejects `sender.origin === "null"`.
**Evidence:** CONFIRMED — browser behaviour reproduced with a Playwright/Chromium build serving a `sandbox allow-scripts allow-forms` test page (`scratchpad/ex/t2.mjs`); the HavenKeys code path (`background/index.ts:79-87`, `background/popup-handler.ts:149-170`, `content/index.ts:357`) confirmed by reading.
**Suggested fix:** in `handleFill` (and the passkey bridge, as defence in depth), refuse unless `self.origin === location.origin`; or probe with `executeScript({func: () => self.origin})` before injecting from the popup path. Correct `docs/native-messaging.md:343`'s unqualified "a sandboxed frame (origin null) is ignored."
**Update, 2026-10-02:** fixed. `handleFill` returns `{ filled: 0, pressing: null }` and the passkey bridge answers `fallback` (sending nothing to the background) when `self.origin !== location.origin`. Tests: `refuses_a_popup_fill_into_an_opaque_origin_document` (`content/content.test.ts`) and `falls back without asking the background in an opaque-origin document` (`webauthn/bridge.test.ts`). `docs/native-messaging.md` now states the rule. Verified in Chromium 145 (a content script sees `self.origin` `"null"` in a CSP-sandboxed document); Firefox not yet checked.

#### EX-02. Visible-field heuristic and missing form-action check (Low, open)
**Attack scenario:** with only HTML/CSS injection on the credential's origin (no script needed), an attacker places a genuine `<input type=password>` off-screen, clipped, covered, or shrunk to 4×4px at near-zero opacity, alongside a visible-looking form whose `action` points at an attacker-controlled origin. `isRendered`/`isFillable` treat all of these as fillable, so the password lands in a field the user never saw; if auto sign-in is on (the default), the extension also auto-presses the submit button, posting the password cross-origin.
**Evidence:** CONFIRMED — `scratchpad/ex/t3.mjs` bundles the real `group.ts`/`fill.ts`/`submit.ts` and shows off-screen, clipped and covered fields all get filled (only `opacity:0` and smaller are excluded).
**Suggested fix:** require viewport intersection and an `elementFromPoint` hit-test in `isRendered`; never fill a password field other than one that is itself hit-testable; do not auto-press (and consider not filling at all without a warning) when the form/button's `action`/`formaction` resolves to a different origin than the frame. Correct the Phase 5 "Verified properties" claim that hidden honeypot fields are never touched — that only holds for `opacity:0`/`display:none`/`visibility:hidden`/sub-4px fields.

#### EX-03. Menu/save tokens in the iframe `src` are re-framable (Low, open)
**Attack scenario:** on the credential's own origin (XSS), a page reads the one-time token out of the real menu iframe's `src`, then creates its own transparent `<iframe src="…/menu.html#token">` under a bait button in a way that avoids the attribute/removal MutationObserver. After the 400 ms arming delay, a click on the bait is accepted as a real pick, filling the page's own field where the script reads it. The same applies to `save.html`, and `cs_ready` hands a pending save token to whatever page loads next in the tab, from any origin. On Chromium, IntersectionObserver v2 still blocks this; on Firefox nothing does.
**Evidence:** CONFIRMED by code; the Chromium mitigation's continued effectiveness was verified directly (`scratchpad/ex/t.mjs`: a 60×30 or covered frame correctly reports `isVisible:false`).
**Suggested fix:** never put the token in the frame's URL — create the frame first, then post the token over `postMessage`/a `MessageChannel` after `load`; bind `cs_ready`'s pending save token to the submitting frame's origin.
**KNOWN?** Substantially, as F3 (mitigated, Firefox delay only) and PK7 (both above). New here: F3's `!important` styles and tamper observer add nothing against a hostile page — it is fully bypassable by re-framing — and the save token can leak to a cross-origin next page in the same tab.

#### EX-04. Save-prompt oracle on a planted username (Info, accepted)
After the F1 fix, a script that already knows the user's typed password can still rewrite the username field and force a submit, learning (from whether a save prompt appears) which saved username the vault holds that password under. **Why accepted:** a narrow residual of the already-accepted F1 tradeoff — the attacker must already have the password and learns only which of the site's own usernames it is saved under, at the existing rate limit.

#### DT1. Server session installed after lock; locked-but-online desktop (Low, partly fixed)
**Attack scenario:** `sync::connect`/`sign_in` spawn a server login in the background and install the resulting session with `set_online` without re-checking the vault's lock state or epoch. A malicious or slow server can simply hold the response until the vault locks (user lock, screen lock, extension-driven lock); the session then lands after the lock, leaving a *locked* vault reachable as "online": the bearer token stays resident, `sync_now` polls every 60 s and fails silently without going offline, and `list_devices`/`revoke_device` (which check only `state.session()`, not `is_unlocked()`) work from a locked renderer, contrary to the documented invariant that a locked vault has no session.
**Evidence:** LIKELY, traced through what is now `crates/havenkeys-client/src/sync.rs` (`connect`) and `crates/havenkeys-client/src/account.rs` (`sign_in`, `unlock_from_server`); the code then lived in `apps/desktop/src-tauri/src/sync.rs` and `account.rs`. Not exercised against a running app with a deliberately delayed server.
**Suggested fix:** capture the vault epoch when the login is spawned; install the session only under the vault guard and only if still unlocked at the same epoch; add an explicit `is_unlocked()` check to `list_devices`/`revoke_device`.
**Update, 2026-10-01** (`docs/superpowers/sdd/2026-10-01-havenkeys-client-extraction/`): partly fixed. `connect`, `sign_in` and `unlock_from_server` in `crates/havenkeys-client` now install the session only under the vault guard and only if the vault is still unlocked; a lock that landed during the login (or, in `sign_in`, while the keychain saved the Secret Key) drops the session unused and announces no `connectivity(true)`. A concurrent lock takes the vault guard first, so it can no longer be overtaken. Tests: `sync::tests::a_lock_during_login_drops_the_new_session` (a stub server holds the login while the vault locks) and `account::tests::sign_in::a_lock_while_saving_the_secret_key_leaves_the_device_offline`. Still open: there is no epoch check, so a lock followed by an unlock while a slow login is pending still installs that first login's session on the new unlock; and `list_devices`/`revoke_device` still check only for a session, not `is_unlocked()`.

#### DT2. `open_item` from the bridge resets the idle timer (Low, open)
**Attack scenario:** a compromised extension, or any same-user process (P9, accepted) that can name two logins matching a claimed page, alternates `open_item` between them every few minutes. Each triggers `ItemEditor` remounting, which calls `get_settings`/`reveal_secret` — both of which call `touch()` — so the idle timer never expires and the vault stays unlocked indefinitely while nobody is at the machine.
**Evidence:** LIKELY, by code trace; same class as the already-fixed #1 (TOTP polling kept the vault unlocked).
**Suggested fix:** stop `get_settings` and the editor's automatic note load from calling `touch()`; rely on `record_activity` for genuine interaction.

#### DT3. Vault and Secret Key roam with the Windows profile (Low, open)
**Attack scenario:** on a domain-joined machine with roaming profiles, `app_data_dir()` resolves to `%APPDATA%` (roaming), and the keychain store defaults to `CRED_PERSIST_ENTERPRISE`, so both the encrypted vault file and the Secret Key (in Credential Manager, or in `device.json` on the file-store fallback) are copied to the profile server at every logoff — undermining the Secret Key's stated purpose of protecting copies that leave the device.
**Evidence:** LIKELY, from the vendored `tauri` and `windows-native-keyring-store` sources plus `lib.rs:59-72`/`secret_store.rs:233-237`; not verified on a real roaming profile.
**Suggested fix:** use `%LOCALAPPDATA%` (`dirs::data_local_dir()`) for the vault and `device.json`; create the keychain entry with `persistence: Local`. At minimum, document the limitation next to S20.

#### DT4. Linux screen-lock detection disables itself permanently after transient failures (Low, open)
**Attack scenario:** three consecutive `loginctl` probe failures (a 2 s timeout under load, a D-Bus/logind restart, resume from suspend) permanently set the probe to "unknown" for the rest of the run, silently disabling screen-lock-triggered vault locking until restart, with nothing shown anywhere. A local attacker who can load the machine, or ordinary system hiccups, can trigger it.
**Evidence:** LIKELY, code read.
**Suggested fix:** back off and retry instead of disabling permanently; show a Settings indicator when screen-lock detection is unavailable.

#### DT5. NSIS pre-install `taskkill` without a path (Low, **fixed**)
The hook originally ran `nsExec::Exec 'taskkill /F /IM havenkeys-native-host.exe'`, an unqualified program name that `CreateProcess` resolves by searching the running executable's own directory first — typically `Downloads` for a manual install, where a drive-by-planted `taskkill.exe` could then run as the user. `apps/desktop/src-tauri/windows/hooks.nsh` now calls `"$SYSDIR\taskkill.exe"` explicitly, closing the planting path. Automatic updates were never affected: the updater downloads into a fresh random temp directory.

#### DT6. Unreadable `device.json` silently replaced (Info, open)
**Scenario:** `Device::load` treats both a read error and any parse failure (including an unknown field on a downgrade, since the type is `deny_unknown_fields`) as "absent," and immediately overwrites it with a new UUID and `file_key: None` — discarding the only copy of a file-stored Secret Key on machines where the keychain wasn't available, and creating a new device identity on the server. Availability only, not confidentiality.
**Suggested fix:** don't overwrite on a read error; move the file aside (rather than replace it) on a parse error.

#### SV-2. Pull pages bounded by row count, not bytes (Medium, fixed)
**Attack scenario (no attacker required):** the server pages pulls at up to 500 rows with no byte cap; the client refuses any response over 17 MiB. A vault whose first 500 rows exceed that — as few as 13 one-megabyte secure notes, or roughly 500 items averaging 35 KB — wedges every device that signs in from cursor 0 at the same point, forever; a `TooLarge` failure never takes the device offline, so it looks "online" while never catching up on later items or deletions.
**Evidence:** LIKELY, measured with the real core: a single 1 MiB ASCII note serializes to about 1.4 MB of base64, so about 12.7 fit per 17 MiB page; a note using `\u0001`-escaped bytes serializes about 6× larger, so about 2.1 fit.
**Suggested fix:** cut pages by a byte budget (for example, 12 MiB) as well as by row count, on both `/v1/sync` and `/v1/items/fetch`; surface `TooLarge` as a visible "sync stuck" state instead of a silent per-tick failure.
**Fix:** `/v1/sync` and `/v1/items/fetch` cut answers at 12 MiB (whole revisions, at least one), reading blob sizes first and blobs only for rows that go out; fetch names `unanswered` items. Tests: `a_vault_of_big_items_pulls_in_pages_under_the_cap`, `a_fetch_of_big_items_names_what_did_not_fit`, and the `page_len` unit tests.
**KNOWN?** No — S9 (above) covers per-field limits on pulled items, not page size; `transport.rs`'s comment calling 17 MiB "the largest answer the protocol can legitimately produce" is not accurate given the measurements above.

#### SV-3. No byte bound or per-account quota on pull/fetch (Low, partly fixed)
**Attack scenario:** any account holder writes about 100 items with ~6 MiB blobs each (each write within the 16 MiB body cap), then calls `/v1/sync?since=0`; the server materializes the whole page (up to 1001 rows) into memory before building JSON — several GB for one request — OOM-killing the container and every account hosted on it.
**Fixed:** paging and the server's memory use per request (see SV-2). **Still open:** the per-account storage cap enforced in the write transaction (planned).

#### SV-4. Unauthenticated login as an Argon2id amplifier starving the DB pool (Low, open)
**Attack scenario:** an outsider floods `/v1/auth/login` with random emails (each gets its own decoy key, so the per-account counter never fires); the per-address limiter is weaker than it looks — a routed IPv6 /64 gives a fresh budget per source address when the server isn't behind a proxy, and check-then-record is a race under a concurrent burst. Every login holds one of 10 pooled DB connections through a full Argon2id run, and `pool.get()` has no wait timeout, so authenticated `/v1/sync`/`/v1/items` requests queue until the client's own 30 s timeout gives up — devices drop to offline or read-only.
**Suggested fix:** give `pool.get()` a wait timeout; release the connection before verifying; bound concurrent Argon2id work with a semaphore that fails fast; key the per-address limiter on the /64 for IPv6; make the check-then-record update atomic.
**KNOWN?** Partly — `X-Forwarded-For` spoofing is already documented, and S8 (above) covers limiter lock-out; the IPv6 /64 rotation, the check-then-record race, and unbounded pool-wait starvation are new.

#### SV-5. Backup restore leaves replicas silently out of step (Low, open)
**Scenario (operational, not an attacker):** after the documented backup-restore mitigation for S5 (above), the server's revision counter can fall below what existing devices have already seen. Items created or edited after the snapshot then conflict forever, new writes can collide with revisions devices already consider pulled, and nothing ever removes the resulting ghost items. The restore drill in `docs/deployment.md` §5 only checks that a *fresh* sign-in sees the items, not what happens to devices that were already signed in.
**Suggested fix:** store a server-side "vault epoch" that changes on every restore (or detect a revision below the client's cursor), and have the client wipe and re-pull from zero when it changes; update the restore procedure to say every existing device needs a fresh pull, not just `reset_sync_cursor`.

#### SV-6. Sync client honours proxy environment variables (Info, accepted)
Contrary to the crate's own "no proxy auto-detection" comment, `reqwest`'s default system-proxy detection is active. This only matters for the `http://localhost`/`127.0.0.1` development URL — production traffic is `https`, so a proxy sees only an opaque CONNECT tunnel — where a `HTTP_PROXY`/`ALL_PROXY` set in the user's own environment would see the bearer token and auth key in cleartext. **Why accepted:** this needs control of the user's own environment, the same same-user precondition already accepted elsewhere in this document (P9). A one-line `.no_proxy()` fix, or correcting the comment, is cheap if this file is touched again.

#### SV-7. A malicious server can force maximum-cost KDF work and an unbounded pull loop (Info, accepted)
Server-supplied KDF parameters and `hasMore` paging are both accepted anywhere inside their validated ranges, so a hostile server can make every sign-in or unlock-fallback cost a 1 GiB/t=16 Argon2id run, or keep a sync loop running indefinitely. **Why accepted:** neither leaks anything — the derived key still can't open the header without the Secret Key — and a hostile server can already refuse service outright, which is already accepted as inherent (S5, S6, above).

#### SV-8. Admin CLI accepts `http://localhost.evil.com` by prefix match (Info, accepted)
`admin new-account --server-url` checks `starts_with("http://localhost")` rather than parsing the host, but the sync client's own `HttpTransport::new` compares the exact host and refuses the URL, so activation fails closed regardless. **Why accepted:** no practical impact given the client-side check; worth tightening if the admin CLI is touched again.

#### SC1. Signing secrets exposed to the whole build (Medium, **mitigated**)
The release workflow previously ran `tauri-action` (which compiles the entire `havenkeys-desktop` dependency graph via `tauri build`, then signs) with `TAURI_SIGNING_PRIVATE_KEY`/`_PASSWORD` set as step `env:` for that whole step — any `build.rs` from a compromised or typosquatted dependency would inherit those secrets and could exfiltrate them from a GitHub-hosted runner with outbound network access. `.github/workflows/release.yml` now splits the job: a "Compile the app (no signing secrets)" step runs `pnpm tauri build --no-bundle --config src-tauri/tauri.bundle.conf.json ... -- --locked`, which builds every dependency, proc macro and the frontend without the signing key present; the later `tauri-apps/tauri-action` step (the one that does hold the key) only packages and signs, with `tauri.package.conf.json` replacing the frontend build with a no-op and Cargo reusing the artifacts already built in the first step. **Residual:** the Tauri CLI itself, the bundler's own tooling, and anything that still ends up recompiling inside the signing step run with the key present. Verification that no dependency actually recompiles during the signing step, at a real release, is pending — see the checklist below.

#### SC2. Main release build not pinned to `--locked` (Low, **fixed**)
Both the compile step and the packaging/signing step now pass `-- --locked`, matching the discipline `scripts/build-native-host-sidecar.mjs` already had for the sidecar build, so a `Cargo.lock` desynced from `Cargo.toml` at tag time fails the build instead of silently re-resolving dependencies.

#### SC3. No automated secret scanning in CI (Info, open)
`gitleaks` has been run manually against the full repository history at least once (2026-09-23, two reviewed and confirmed-benign findings), but nothing runs it on push or PR, so a future accidental secret commit would only be caught if someone remembers to run it by hand.
**Suggested fix:** add a lightweight `gitleaks`/`trufflehog` step to a PR-triggered workflow with `permissions: contents: read` only.

### Checked and sound

**Crypto and vault core:**
* KDF parameters are floor/ceiling-checked on every derivation path, including a server- or file-supplied header; a weakened KDF still cannot unwrap the vault key without the Secret Key.
* AEAD is AES-256-GCM with a CSPRNG nonce on every seal and unambiguous AAD binding version, algorithm, purpose, vault and item; there is no downgrade path for unknown versions or algorithms.
* The key hierarchy (HKDF with distinct `info` labels, a 128-bit CSPRNG Secret Key, separated data/vault keys) and every secret-returning path (gated on an unlocked session) hold up as documented.
* Rekeying, header adoption and epoch checks all refuse a stale, mismatched, or wrong-vault state.
* The password generator uses rejection sampling and an unbiased shuffle over a CSPRNG; TOTP correctly bounds digits/period/secret length; passkey signing uses RFC 6979 with the RP ID checked against the browser-reported URL under PSL-aware registrable-domain rules.
* `Debug` output is redacted on every secret-bearing type; SQLite statements are all parameterized and the vault file is created 0600.

**Bridge and native messaging:**
* No path returns or changes anything while locked, with integration off, for a non-matching item, or for an unauthorized RP ID — verified across 43 origin-matching, 34 `authorize_rp`, and 13 protocol-parsing test cases.
* Fill, TOTP, save-login and passkey operations are all independently origin/RP-bound in Rust, never trusting the extension's own claims.
* Framing, malformed messages and oversized messages are all rejected safely on both hops; Unix socket and Windows pipe peer identity are both checked (Windows pipe squatting before the peer check runs is now checked on the client side too: BR-1, fixed in code, unverified on Windows).
* Rate limits, native-host manifest registration, and the AppImage/translocated-app path-hijack protection all hold as documented.

**Browser extension:**
* Senders are told apart only from browser-provided data (tab URL, `sender.origin`, `sender.url`), never from page-supplied message content, for authorization decisions.
* Menus and picks require a trusted user gesture, are single-use and tab-bound, and expire; auto sign-in runs are bound to tab/frame/origin, move forward only, and end on any trusted user input.
* Secrets never reach the menu/save/passkey UI beyond titles and usernames rendered with `textContent`; a full grep of `src` (excluding tests) finds no `innerHTML`, `eval`, browser storage API, or unexpected `postMessage`.
* Extension CSP has no `unsafe-*`; permissions are `nativeMessaging`/`activeTab`/`scripting` only, with host access optional and user-granted; the full suite (26 files, 338 tests) passes.

**Desktop (Tauri):**
* The Tauri command allowlist is exactly the 43 commands the build declares, with no `allow-emit` and no filesystem/shell/http/dialog plugin permission beyond what's needed; the production CSP has no `unsafe-*` and navigation to non-app URLs is refused.
* Master-password change, rekey, and header adoption all require the current password and epoch, and are only committed after the server accepts.
* TOTP seeds and passkey private keys never reach the renderer; the QR-scan token and the import file path are both time-boxed and cleared on lock.
* Locking drops the vault session, clears the clipboard, scan slot and import state, and remounts the whole vault UI tree, so no revealed secret survives in React state.
* The updater's install order (download → verify signature → lock → install) is enforced by the plugin and the function's straight-line structure, and no updater plugin permission reaches the renderer.

**Server and sync client:**
* The auth key is a separate HKDF branch requiring the 128-bit Secret Key; a server, or a server-chosen weak KDF, cannot turn it into a password guess, and a downgraded KDF cannot open the real header.
* Authorization on every route is derived from one session query keyed on account/device/vault; there is no IDOR, and item AEAD (AAD = purpose ‖ vault ‖ item) prevents cross-item or cross-vault substitution — same-item version splicing, formerly the one exception, is now refused by the overview binding (CR1/SV-1, fixed).
* Sessions are 256-bit CSPRNG tokens stored only as SHA-256 with 24 h expiry; invites are single-use, TTL-bound, and compared in constant time; SQL is fully parameterized; enumeration is defended with decoy KDF parameters and uniform error responses.
* Client TLS uses rustls with default verification; `http` is allowed only for exact localhost addresses; the container runs as non-root with no build-time secrets.

**Supply chain:**
* No committed secret anywhere in git history (targeted `-S` searches across all refs for private-key, AWS, Slack and GitHub token markers all return nothing); `.gitleaksignore`'s two entries are genuinely benign on inspection.
* `cargo audit`, `cargo deny check` and `pnpm audit` all pass, or show only the same pre-existing, already-triaged advisories as every earlier review in this document.
* All GitHub Actions in `release.yml` are pinned to full commit SHAs; workflow permissions are `contents: write` only; the trigger is a tag push only (no `pull_request`/`pull_request_target`), so forked-PR secret exposure is not reachable.
* pnpm's build-script gating is active and was observed live (an `esbuild` install script was ignored, not run); the Dockerfile excludes vault, import-export and `.env*` files from the build context via `.dockerignore` and runs the server as a non-root user.
* The extension and website load no remote scripts, fonts, or stylesheets.

### Pending manual checklist items

| # | Check |
|---|---|
| SC-M1 | At the first release cut with the split compile/sign workflow (SC1 above): confirm the "Package, sign and attach installers" step's logs show no dependency, proc-macro, or frontend recompilation (nothing for Cargo to build beyond packaging/signing), and that every `.sig` asset's trusted comment still carries `version:<the release version>` — this repeats UM6 above, now also confirming SC1's isolation held in practice. |

## Extension site access on by default (2026-09-28)

Spec: `docs/superpowers/specs/2026-09-28-extension-defaults-design.md`.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| E1 | Info | Extension | The content script and the two passkey scripts run on every http(s) page by default, instead of only after the user opted in on the options page | Accepted (spec §9) |
| E2 | Info | Extension | Narrowing site access to chosen sites in the browser turns the scripts off everywhere (registration checks only the broad patterns) | Open, fails closed (`autofill.md` §Permissions) |

### E1. Default-on site access (Info, accepted)
Save and update prompts and passkeys need a script in the page, and an
opt-in behind the options page meant they silently did not work. The host
patterns moved from `optional_host_permissions` to `host_permissions`.
**Cost:** a bug in the content or passkey scripts, or a compromised
extension build, now reaches every page for every user. The scripts were
already written to run on any granted page and treat it as hostile; none of
them changed. **What did not change:** every page request still goes
through the desktop, and browser integration there stays off by default
(P6), so nothing reaches the vault until the user turns it on. The new
`storage` permission holds one boolean (whether in-page suggestions are
shown); `hygiene.test.ts` allows storage in `shared/prefs.ts` only.
**Update impact:** Chrome disables an installed 0.9.x extension when it
updates to a version with new host permissions, until the user accepts
them.

## Sign in with Google, Microsoft, GitHub and Apple (2026-09-28)

Spec: `docs/superpowers/specs/2026-09-28-sign-in-with-design.md`. Scope: the
`sign_in_with` item field and `SsoProvider`/`sso.rs`
(`crates/havenkeys-core`), `start_sso`/`check_sso`/`save_sso`
(`crates/havenkeys-protocol`, `crates/havenkeys-bridge`), the extension's
button/chooser detection and run
(`apps/extension/src/autofill/sso.ts`, `content/sso.ts`,
`background/sso-state.ts`, `background/sso-handler.ts`), the desktop UI
(`ItemDetail.tsx`, `ItemEditor.tsx`), and the `.1pux` importer's `ssoLogin`
mapping and re-import upgrade.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| SSO1 | Low | Extension (save detection) | A trusted click on a page element that merely *says* "Sign in with Google" arms a save prompt | Accepted, bounded |
| SSO2 | Low | Extension / Core | The account text offered in the save and choose flows comes from the provider page | Accepted, mitigated |
| SSO3 | Medium | Extension | Consent detection is keyword-based | Accepted, documented |
| SSO4 | Low | Extension | `openerTabId` (which frame may act as "the run's popup") is trusted | Accepted, sound |
| SSO5 | Info | Extension | Google Identity Services (GIS) iframe buttons and One Tap are not recognized or pressed | Accepted, functional limitation |
| SSO6 | Medium | Extension (background/content) | A pick on site X can fill the provider's password without a second click there | Accepted, bounded |

### SSO1. A fake "Sign in with" label arms a save prompt (Low, accepted)
**Attack/failure scenario:** any page can put "Sign in with Google" text on
any clickable element and get a user to click it. The click starts a
`PendingSso` in background memory and, if the flow later reaches a real
Google origin and returns, offers to save that account against the page's
own site.
**Why low:** the click only *arms* a possible save; nothing is written
without a second explicit action. The resulting balloon always names the
site being saved for and requires **Save**, and it can only ever offer to
save a login *for the page the user clicked on* — it cannot target another
site, and it carries no password. A misleading label at worst produces an
unwanted save offer the user can dismiss or ignore (`autofill.md`, "Save
detection").

### SSO2. Account text comes from the provider page (Low, accepted/mitigated)
**Attack scenario:** a compromised or malicious provider-origin page (or one
whose chooser the extension misreads) could feed an arbitrary string as the
"account" shown in the save prompt or clicked in a later run.
**Mitigation:** the text is validated on arrival in Rust — trimmed, at most
254 characters, no control characters (`crates/havenkeys-core/src/sso.rs`)
— and is always shown to the user, editable, before a save is confirmed. On
the *choose* side, a bad value can only cause `chooserRow` to fail to find a
unique matching row (nothing is clicked) or, if it happens to collide with a
real row's visible email, click that row — the same click a user reaching
that chooser manually could make. No secret is read or written based on this
text; it is a display value and a lookup key into the vault's own logins,
never a credential.

### SSO3. Consent detection is keyword-based (Medium, accepted, documented)
**Failure mode:** `isConsentScreen`/`isConsentLabel` recognize a fixed list
of English and Portuguese consent/permission phrases ("Continue as …",
"Allow…", "Grant access/permission(s)…", "Authorize…", plus a few short bare
words). A provider redesign that phrases its consent screen outside this
list, while also showing a row whose visible email equals the saved account,
could be clicked as if it were an ordinary chooser row.
**Mitigation (why not worse):** a click still requires `chooserRow` to find
**exactly one** clickable, visible element whose text contains the saved
account — an unrecognized consent screen still needs that same coincidence
to be reachable at all, and pressing it only grants whatever that specific
screen asks for, on the provider's own real origin, for an account the user
already saved as belonging to their own login. The keyword list itself also
actively stops the run wherever it does match, which is the common case for
the four supported providers' actual consent screens as of this writing.
**Limitation, documented:** provider page redesigns can change this
behaviour; the word list in `apps/extension/src/autofill/sso.ts` is not verified against live provider pages
on an ongoing basis (`autofill.md`, "Known limitation"; `security-model.md`
§17).

### SSO4. `openerTabId` trust (Low, accepted, sound)
**Question:** the run accepts a choose step from the top frame of the run's
own tab, or of a tab whose `openerTabId` is the run's tab (iframes never
choose) — could a hostile page
forge this to reach another tab's run?
**Why sound:** `openerTabId` is supplied by the browser itself: the
background reads it from `sender.tab.openerTabId` on the content script's
message (`background/index.ts`), never from anything a page's message
content claims. A page opening a
popup to a provider origin, with that popup's `openerTabId` pointing back at
it, is exactly the legitimate OAuth-popup flow the design accounts for
(spec §6.2, "a popup whose `openerTabId` is the run's tab"). The run still
independently requires that popup's own frame origin to be one of the
provider's exact origins before it will act — the opener relationship alone
grants nothing.

### SSO5. GIS iframe / One Tap not supported (Info, functional limitation)
Google Identity Services renders its button and One Tap prompt inside a
cross-origin iframe from `accounts.google.com/gsi` that HavenKeys' content
script cannot reach or classify as a same-page button. Sites using GIS this
way get no balloon-driven press; sites with their own button (the common
"Continue with Google" pattern most sites use) work normally. Not a security
finding — documented in `autofill.md` as a known limitation and in the
design's out-of-scope list (§13).

### SSO6. A pick on site X can fill the provider's password without a second click (Medium, accepted)
**Scenario:** the user picks a "Sign in with `<Provider>`" login on site X.
If the saved account is not already signed in at the provider (no chooser
row, or the provider asks for a password), HavenKeys' login hand-off
(`docs/superpowers/specs/2026-09-29-sign-in-with-provider-login-design.md`)
fills — and, with the flow's three switches on, submits — the vault's
provider login on the provider's own page, without the user clicking
anything there. This is new: before this change, HavenKeys only ever clicked
an already-signed-in chooser row on the provider's page.
**Mitigation (spec §5):** the hand-off is bounded by all of the following
at once — the run's Rust-listed provider origins (`sso.rs`), the top frame
of the run's tab or its opener popup, the run's 2-minute expiry, **exactly
one** provider login whose saved username equals the run's account (zero or
more than one and nothing is filled), Rust's own origin check on
`fill_item`/`get_totp` (identical to every other fill), and the three
switches (the site login's `autoChoose`, the vault-wide `auto_sign_in`
setting, and the provider login's own `auto_sign_in` switch). The extension
gains no secret it could not already request — `fill_item` for a login saved
for the provider's own origin, asked from that origin, was always allowed;
what changes is that no click on that page needs to precede it. Nothing
reaches site X: it can at most cause a run that ends on the real provider
page, with the user's own provider login, after the user's own pick.
**Limitation, documented:** the hand-off relies on the same page-structure
heuristics (`findLoginGroup`, `chooserRow`, `anotherAccountButton`,
`isConsentScreen`) as the rest of autofill, so it can land on a recovery or
sign-up step reached after a chooser-row click, filling the same vault email
there too (`autofill.md`, "Known limitation"). A visible CAPTCHA stops the
run before anything is pressed. Phone prompts, security keys, passkeys and
"stay signed in?" screens are not recognized as a login form at all, so the
run simply stops and the user finishes by hand.

---

# Security Review: Android M1

**Date:** 2026-10-01
**Scope:** branch `android-m1`: `crates/havenkeys-mobile`, the Android parts
of `crates/havenkeys-core` (`unlock_bundle.rs`, `app_target.rs`,
`app_fill.rs`, `asset_links.rs`, `local.rs`, `data/android-browsers.json`),
`crates/havenkeys-client/src/bundle.rs`, the `platform-verifier` feature of
`crates/havenkeys-sync-client`, and `apps/android` (manifest, network and
backup configuration, `security/`, `autofill/`, `clipboard/`, `data/`, the
UI's secret handling, the Gradle build and CI). Design:
`docs/superpowers/specs/2026-10-01-android-app-design.md`.
**Method:** self-review of the code against the spec and the global
constraints of the implementation plan, the per-task reviews made while it
was built, and the commands below. **Nothing in `apps/android` has run on a
phone or an emulator**: every Android behaviour below is read from the code
and Android's documentation, and the manual checklist at the end is still
open.

> This is an internal review, not an independent security audit.

## Summary

No critical or high-severity issue was found in the code. The central
properties hold in the host tests: Rust decides every target, match and fill;
an app is identified by package **and** signing certificate; a WebView's
domain is never trusted; a locked vault returns nothing; a stale, other-boot
or tampered unlock bundle is refused. The largest open item is not a flaw
but a gap: none of it has been exercised on Android (AN2).

The final whole-branch review found two Medium issues, both fixed: reads
reset the idle timer, so background refreshes kept the vault unlocked
(AN7), and the master password fields were offered to third-party autofill
services (AN19). AN1, AN6, AN9 and AN20–AN24 were fixed in the same wave.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| AN1 | Low | App (`AppContainer`, `HavenApp.appScope`) | App-wide event collectors have no exception handler: a Keystore `ProviderException` while deleting a key, or a `ClipboardManager` failure while clearing, ends the process | **Fixed** (`4beb735`) |
| AN2 | Info | Whole app | Nothing has run on a device or emulator: instrumented `KeystoreTest`, the QR scanner, BiometricPrompt, the navigation lock wipe, `FLAG_SECURE`, the clipboard flags, the `AutofillService` end to end, `getCallingPackage()` in the fill activities, package visibility | Open: manual checklist |
| AN3 | Low | Autofill (gated rows) | With "Confirm before filling" on and the vault unlocked, the app being filled can fire its rows' IntentSenders without a tap, and swap the item ID and mode they carry, and HavenKeys answers without showing anything | Accepted, documented (Rust returns only that caller's matched logins) |
| AN4 | Info | CI | `.github/workflows/android.yml` has never run on GitHub | Open |
| AN5 | Low | Network / Digital Asset Links | User-installed CAs are trusted; whoever holds one can intercept the server connection and forge `assetlinks.json`, so a malicious app named after a site can be offered that site's logins | Accepted, documented |
| AN6 | Low | UI (binding prompt, unlock) | No protection against touches through another app's overlay (`filterTouchesWhenObscured` / `setHideOverlayWindows`); matters mainly on Android 9–11 | **Fixed** (`d11030a`) for the autofill activities and the binding prompt |
| AN7 | Medium | Mobile API / auto-lock | Every read (`list_items`, `search`, `item_view`, `reveal`, `totp`) reset the idle timer, and the app calls them on its own (a sync's `items_changed` every 30 s while in the foreground, the TOTP countdown every second), so auto-lock never fired with the screen on | **Fixed** (`eee642f`) |
| AN8 | Low | Settings → Devices | Revoking this phone from its own Devices list leaves its biometric bundle in place (Sign out deletes it) | Fixed (revoking this phone emits `signed_out`) |
| AN9 | Low | Mobile API / auto-lock | Secret-returning calls check the lock state, not the auto-lock clock; after the process thaws, a fill can be answered before the overdue 5-second tick locks | **Fixed** (`eee642f`) |
| AN10 | Medium | Autofill (package visibility) | Confirmed on a Galaxy S24+ (Android 16): Android hid Chrome and every app from HavenKeys, so no certificates were read and nothing was ever filled | **Fixed** for the `github` flavor (`QUERY_ALL_PACKAGES`); open for `play` |
| AN11 | Info | Auto-lock | Rust's monotonic clock excludes deep sleep on Android, so the core's suspend detection probably locks the vault after any sleep longer than 30 s, even with auto-lock off | Open: to observe on a device (fails closed) |
| AN12 | Info | App (`VaultEventsHub`) | Events go through a `SharedFlow` with `DROP_OLDEST`; a slow collector could miss `SignedOut`/`Removed` and keep Keystore keys | Accepted |
| AN13 | Info | App memory | The master password, typed Secret Key, revealed and fill values are JVM `String`s; the Kit's QR camera frames are not zeroed | Accepted, documented |
| AN14 | Info | Manifest | Libraries add components to the merged manifest (`ProfileInstallReceiver`, exported and `DUMP`-guarded, in every build; `PreviewActivity` and `ComponentActivity` in debug builds); `ManifestTest` checks the source manifest only | Accepted, documented |
| AN15 | Info | Digital Asset Links (privacy) | A fill request in an app makes HavenKeys contact up to 8 vault sites named like the app, telling them (and the network) that the phone holds a login there | Accepted, setting to turn off |
| AN16 | Info | "Search HavenKeys…" offline | Offline, a confirmed "Use <login> in <package>?" fills any login once without storing a binding | Accepted (the confirmation is the authorization) |
| AN17 | Info | Onboarding / unlock | The master password's minimum length (10) is checked by the UI only, on desktop and Android | Accepted |
| AN18 | Info | Dependencies | `yoke-derive` 0.8.3 in `Cargo.lock` has been yanked (already so on `main`) | Fixed (0.8.4) |
| AN19 | Medium | Unlock, onboarding (master password, Secret Key) | The master password fields carried `ContentType.Password`/`NewPassword`, so a third-party autofill service (Google Autofill, another password manager) was offered the master password and the Secret Key and could offer to save them | **Fixed** (`d11030a`) |
| AN20 | Low | Network (`havenkeys-sync-client`) | A release build of the phone app accepted `http://localhost` as a server address | **Fixed** (`a129cff`) |
| AN21 | Low | Settings (`settings.rs`) | A device-settings blob that exists but does not open turned "Confirm before filling" off (the default) | **Fixed** (`a129cff`) |
| AN22 | Low | Vault list (`items.rs`) | The list's website came from string splitting and kept a saved URL's userinfo (`https://user:pw@host/` showed `user:pw@host`) | **Fixed** (`a129cff`) |
| AN23 | Info | Secret Key file (`SecretKeyCipher`) | Two first uses at once could each generate the Keystore key, leaving a seal under a replaced key | **Fixed** (`4beb735`) |
| AN24 | Info | Item screen | Revealed values were remembered by position, so after a reload added or removed a field a shown value could appear in another row | **Fixed** (`4beb735`) |
| AN25 | High | TLS on Android (`network_security_config.xml`) | Found on the first real phone: sign-in always said "offline". Android's certificate check downloads the certificate's revocation list over plain HTTP (Let's Encrypt has no OCSP, only CRLs at `http://*.c.lencr.org`); the app's no-cleartext rule blocked it, and the verifier reports any revocation failure as "revoked" | **Fixed**: plain HTTP allowed to `lencr.org` only, for Android's stack (Rust still refuses plain HTTP to the server) |
| AN26 | Medium | Autofill (`LoginFormFinder`) | Found on the same phone: a login whose only password says `autocomplete="new-password"` (dashboard.render.com) was taken for a sign-up form, so nothing was offered | **Fixed**: that one field, beside a username with no confirmation, is the login's password |
| AN27 | Medium | Autofill (`LoginFormFinder`, `FieldClassifier`) | Found in the Riot app: a code split one box per digit (`maxlength=1`, and a stray `new-password` hint) was not a code field, and the app shows Android only the tapped box | **Fixed**: the tapped one-character box (HTML `maxlength` or a native view's `maxTextLength`) on a screen with no login field is a code field; in a row of such boxes the first one |
| AN28 | Info | Autofill (`DatasetFactory`, `AutofillAuthActivity`) | Android fills only the tapped box, which keeps one digit, so a split code cannot be filled whole | **Mitigated**: a "Copy one-time code" row beside each offered code copies it (explicit tap, Rust asked for that target, sensitive clip, usual clearing) for pasting |
| AN30 | Low | Autofill (`HavenAutofillService` scope) | The service's coroutine scope has no exception handler: an unexpected throw while parsing a hostile structure (fill or save) ends the process | Fixed (`guarded`, `ServiceGuard.kt`) |
| AN31 | Low | Editor (`ui/edit/`) | The draft holds typed secrets in Compose state, and typed values pass through JVM `String`s, for as long as the screen is open | Mitigated (no saved state, rotation handled in place, lock wipe, `FLAG_SECURE`, autofill exclusion); strings are not zeroed |
| AN32 | Low | Autofill save (`autofill_save`) | An app can make HavenKeys save a login named like another service | Mitigated: bound to that app only; title can mislead |
| AN33 | Info | Autofill save | A save request is lost if the vault locks before the user submits | Accepted (user sees "HavenKeys locked before saving.") |
| AN34 | Info | Autofill save (`SaveForm`) | On Android 9 a username-first sign-in has no `FLAG_DELAY_SAVE`: the password step saves without the username unless that screen has one | Accepted |
| AN35 | Info | Editor keyboards | `IME_FLAG_NO_PERSONALIZED_LEARNING` is a request: a keyboard may ignore it and learn or log what is typed | Accepted (Android limitation) |
| AN36 | Info | Editor, Autofill save | Nothing in M2 has run on a device or emulator; the Android M2 checklist is open | Open |

## Details

### AN1. App-wide collectors can end the process (Low, fixed)
**Component:** `AppContainer.kt` (`wipeKeysOnExit`, `clearClipboardOnLock`),
launched in `HavenApp.appScope` (`SupervisorJob`, no
`CoroutineExceptionHandler`). **Attack scenario:** not an attack; a failure.
`keystoreDelete` catches `GeneralSecurityException` and `IOException`, but a
Keystore `ProviderException` (a `RuntimeException`) or a `ClipboardManager`
exception escapes the collector and, with no handler, crashes the app.
**Effect:** fail-closed for the vault (it lives only in memory), but a crash
during sign-out or removal can leave the biometric key, the bundle or the
Secret Key's Keystore key behind until the next sign-out. **Mitigation
(fixed in `4beb735`):** Keystore calls go through `keystoreOr`, which
answers a fallback on `GeneralSecurityException`, `IOException` or any
`RuntimeException` (the biometric check answers false, a delete does
nothing), and `SensitiveClipboard.clearIfOurs` catches `RuntimeException`
around the clipboard; nothing is logged. Covered by `KeystoreWipeTest`
(`aKeystoreThatThrowsAnswersTheFallback`,
`aFailedDeleteDoesNotStopTheCollector`) and `ClipboardLockTest`
(`aClipboardThatThrowsDoesNotEndTheCaller`). **Remaining limitation:** a
failed delete leaves the key until the next sign-out or removal, as before,
but no longer ends the process.

### AN2. Nothing has run on Android (Info, open)
**Component:** all of `apps/android`. No emulator system image was installed
on the build machine and no phone was attached. The JVM unit tests (157)
cover parsing, classification, planning, the structure-caller check, the
clipboard decision, ViewModels and the manifest; the Rust tests cover every
security decision. Not exercised: the Keystore (`KeystoreTest` is compiled,
never run), BiometricPrompt and its `CryptoObject`, the camera QR scanner,
the navigation lock wipe, `FLAG_SECURE`, the clipboard's sensitive flag and
clearing, the `AutofillService` with real apps and browsers, whether
`getCallingPackage()` is non-null when Android starts the fill activities,
and package visibility (AN10). **Mitigation:** the manual checklist below
and one run of `connectedGithubDebugAndroidTest`. **Remaining limitation:**
any of these may fail on a real phone; most fail closed (no fill, no
biometric unlock), but that is a claim to check, not a result.

### AN3. Gated rows can be fired by the app being filled (Low, accepted)
**Component:** `DatasetFactory.sender`, `AutofillAuthActivity`. **Attack
scenario:** a malicious app that HavenKeys matched (a binding the user made,
or a site that vouches for it) holds the IntentSender of each gated row and
fires it without the user tapping; with the vault unlocked, the activity
calls `autofill_fill` and returns the login. The IntentSender's Intent is
mutable, and `Intent.fillIn` merges the sender's extras, so the app can also
replace `EXTRA_ITEM_ID` and `EXTRA_MODE`: it can ask for any of its matched
logins, or a code instead of a password. **Mitigation:** the activity
accepts the structure only when it names the calling package, and Rust
returns only logins matched to that package and certificate — the same set
direct fill would have put in the framework, whatever item ID or mode the
Intent names. **Remaining limitation:** "Confirm before filling" keeps
values out of the autofill framework until a row is used; it does not
restore per-fill authorization and is not a per-fill prompt while unlocked:
the matched app can obtain its own matched logins without the user. The spec's
description ("tapping opens a minimal HavenKeys activity") matches the code;
the checklist's "each fill asks" should be read that way
(`docs/android.md`).

### AN4. CI has not run (Info, open)
The workflow is written and its actions are pinned by SHA, but no push has
triggered it on GitHub. The commands it runs were run locally (below).

### AN5. User-installed CAs and Digital Asset Links (Low, accepted)
**Component:** `rustls-platform-verifier` (Android's CA store, user CAs
included), `res/xml/network_security_config.xml` (the same for Android's own
stack), `asset_links_fetch.rs`. **Attack scenario:** a CA the user
installed (or a device policy pushed) is held by an attacker on the network
path. They can read the server connection's metadata, the auth key at
sign-in and the session token (not the vault: everything is ciphertext), and
answer `https://github.com/.well-known/assetlinks.json` with a file that
vouches for their own app; if that app's package name contains `github`
and the user focuses a login field in it, it is offered GitHub's logins
(direct fill after one tap). **Mitigation:** none beyond Android's warnings
when a user CA is installed; the trade-off is deliberate, so a self-hosted
server with its own CA works as it does in the phone's browser. **Remaining
limitation:** as described. A narrower option, not taken: trust only system
CAs for the Digital Asset Links fetch.

### AN6. Overlays (Low, fixed)
**Component:** `AutofillSearchActivity` ("Use <login> in <package>?"),
`AutofillAuthActivity`, `MainActivity`. **Attack scenario:** an app with
"display over other apps" draws a window over the binding prompt and gets
the user to tap "Use" while believing they tapped something else, binding a
login to that app. Android 12+ blocks touches through most untrusted
overlays; Android 9–11 do not. **Mitigation (fixed in `d11030a`):**
`AutofillAuthActivity` and `AutofillSearchActivity` set
`filterTouchesWhenObscured` on their window, and the binding dialog, which
has a window of its own, sets it through
`SecureDialogWindow(ignoreObscuredTouches = true)`; a tap that passed
through another app's window is dropped. `ManifestTest` checks both.
**Remaining limitation:** `MainActivity` (unlock, the vault) does not filter
obscured touches, so legitimate overlays (screen filters, accessibility)
keep working there; nothing in it binds or fills for another app. Not yet
observed on a device.

### AN7. Reads held the idle timer (Medium, fixed)
**Component:** `MobileVault::{list_items, search, item_view, reveal, totp}`,
each of which called `touch()`. **Scenario:** the app calls these on its
own, not only when the user acts: `HavenApp` runs `sync_if_due` every 30
seconds while in the foreground, every sync emits `items_changed`, and the
vault list and item screen reload on it; the item screen also asks for the
live TOTP code once a second. With the screen on (charging dock, "stay
awake"), and with any screen open, the idle timeout was never reached; a
server that changed one item now and then would do the same. This is
desktop finding #1 again, on Android, and wider. **Mitigation (fixed in
`eee642f`):** no read touches the timer (the rationale is commented in
`items.rs`, as on the desktop); `MainActivity.onUserInteraction` remains the
signal for taps, and the vault search box reports typing, which a soft
keyboard does not route through `onUserInteraction`. Covered by
`items::tests::reads_do_not_postpone_the_auto_lock` and
`VaultViewModelTest.typingInTheSearchCountsAsActivityButLoadingDoesNot`.
**Remaining limitation:** typing in a dialog's text field (enrolling
biometrics, removing the vault) is not reported as activity.

### AN8. Revoking this phone keeps its bundle (Low, fixed)
**Component:** `DevicesViewModel.revoke`, `wipeKeysOnExit`. Sign out emits
`signed_out` and deletes the biometric key and bundle; revoking the current
device from the list does not. **Mitigation:** the server refuses the
bundle's auth key at the next online bundle unlock, which locks with
`bundle_refused` and deletes it. **Remaining limitation:** offline, the
bundle still opens the local replica until it expires (14 days or a reboot).

**Fixed (2026-10-02):** `revoke_device` emits `signed_out` when the revoked device is this phone, so the app deletes the biometric key and bundle as on sign-out (`account.rs`, `revoking_this_phone_signs_it_out_and_another_does_not`).

### AN9. A fill right after the process thaws (Low, fixed)
**Component:** `MobileVault` (`require_unlocked`), `LockClock`. Auto-lock is
a Rust thread ticking every 5 seconds, plus a check when the app returns to
the foreground. When Android freezes the cached process, the thread does not
run. **Scenario:** the auto-lock deadline passes while the process is
frozen; the user focuses a login field in another app; Android binds the
`AutofillService`, and `autofill_matches`/`autofill_fill` (which check only
that the vault is unlocked) may answer before the overdue tick locks.
**Mitigation (fixed in `eee642f`):** every vault read starts with
`MobileVault::unlocked`, which applies an overdue auto-lock and then
requires the vault to be unlocked: `autofill_matches`, `autofill_fill`,
`autofill_totp`, `autofill_search`, `autofill_bind_and_fill`, `reveal`,
`totp`, `item_view`, `list_items`, `search` and `create_unlock_bundle`.
`LockClock::touch` checks whether the lock is due first, so a late touch
locks an overdue vault instead of rescuing it. The autofill planner shows
"Unlock HavenKeys" when Rust locked on the way (`locked`). Covered by
`an_overdue_vault_locks_before_any_fill`,
`an_overdue_vault_locks_before_any_read`, `an_overdue_vault_enrolls_nothing`,
`a_late_touch_locks_an_overdue_vault_instead_of_rescuing_it` and
`FillPlannerTest.aVaultRustLocksOnTheWayAsksToUnlockFirst`. **Remaining
limitation:** none known for these calls; not observed on a device.

### AN10. Package visibility (Medium, fixed for `github`)
**Component:** `CallerIdentity.kt`, `src/github/AndroidManifest.xml`. On a
Galaxy S24+ (Android 16) the caller was not visible to the autofill
service: `certDigests` returned nothing for Chrome, Rust refused the
target, and no app or site was ever offered a login. The plan's answer
(Task 23 Step 4) is applied: the `github` flavor requests
`QUERY_ALL_PACKAGES`, checked by `ManifestTest`. HavenKeys reads only the
signing certificates of the package being filled. **Remaining
limitation:** the `play` flavor has no such permission (Google Play
restricts it), so autofill there would fail the same way until another
route (for example `<queries>` for the privileged browsers, or Play's
autofill-service exception) is chosen.

### AN11. Sleep and the suspend detector (Info, open)
**Component:** `LockClock` (`Instant`), core `LockManager::tick`. The
`LockManager` locks when wall time advances more than 30 seconds beyond
monotonic time. On Android, Rust's `Instant` uses `CLOCK_MONOTONIC`, which
stops in deep sleep, so a phone that sleeps probably locks HavenKeys at the
next tick even with "Lock when the screen turns off" off and auto-lock set
to never. Safe, possibly surprising; to be confirmed on a device.

### AN12. Lossy event flow (Info, accepted)
**Component:** `VaultEventsHub` (`replay = 1`, `extraBufferCapacity = 16`,
`DROP_OLDEST`). Rust's events are posted without suspending; a collector more
than 17 events behind loses the oldest. The Keystore and clipboard
collectors do almost no work and events are rare, so this is not expected in
practice.

### AN13. JVM memory (Info, accepted)
See `security-model.md` §22.12: strings cannot be wiped; the bundle's
`ByteArray` is zeroed but copies may exist; the Kit's camera frames are not
zeroed.

### AN14. Library components in the merged manifest (Info, accepted)
`androidx.profileinstaller.ProfileInstallReceiver` is exported in every
build but requires `android.permission.DUMP` (signature|privileged), so only
the system and `adb shell` can reach it. `PreviewActivity` (Compose tooling)
and `androidx.activity.ComponentActivity` (Compose test manifest) are in
debug builds only. `ManifestTest` reads the source manifest; a check of the
merged release manifest would catch a future library adding more.

### AN15. Digital Asset Links requests (Info, accepted)
See `security-model.md` §22.7. The setting turns them off.

### AN16. Offline "Search HavenKeys…" (Info, accepted)
Online, confirming stores a binding and the fill goes through the normal app
match. Offline nothing can be stored, so `autofill_bind_and_fill` reads the
chosen login directly. The prompt names the package, and only HavenKeys'
own activity can confirm it; overlays are AN6.

### AN17. Password length (Info, accepted)
The 10-character minimum when activating is a UI check
(`OnboardingScreen.kt`, and the desktop's own); a client calling Rust
directly could set a shorter password. Argon2id cost and the Secret Key
still apply.

### AN18. Yanked `yoke-derive` (Info, fixed)
`cargo deny check` and `cargo audit` warn that `yoke-derive` 0.8.3 is
yanked. It is the same on `main`, so not introduced here; a
`cargo update -p yoke-derive` is the likely fix.

**Fixed (2026-10-02):** `cargo update -p yoke-derive` (0.8.3 → 0.8.4); `cargo deny check` passes and `cargo audit` no longer reports it.

### AN19. The master password reached third-party autofill (Medium, fixed)
**Component:** `UnlockScreen.kt`, `OnboardingScreen.kt` (master password,
new password, Secret Key), `SettingsDialog.kt` (the password asked to turn
on biometric unlock). **Attack scenario:** the unlock and onboarding fields
were marked `ContentType.Password`/`NewPassword`, so the device's autofill
service — Google Autofill, or another password manager the user picked for
other apps — was given the structure of the screen where the master password
and the Secret Key are typed and could offer to save them to its own cloud.
That breaks CLAUDE.md §9 ("never expose it… never persist it"). The plan
asked for these hints (Task 18); this ruling overrides it. **Mitigation
(fixed in `d11030a`):** no secret input carries an autofill hint; every
activity sets `importantForAutofill = IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS`
on its window, and every dialog with a text field does the same on its own
window (`SecureDialogWindow`). `ManifestTest` checks every activity, every
dialog with a text field, and that no source names
`ContentType.Password`/`NewPassword`. **Remaining limitation:** read from
Android's documentation, not observed on a device (manual checklist).

### AN20. `http://localhost` in a release build (Low, fixed)
**Component:** `havenkeys-sync-client` `HttpTransport::new`. Android's
release network config forbids cleartext, but Rust's sockets do not go
through it, and the transport accepted `http://localhost`,
`http://127.0.0.1` and `http://[::1]` in every build. On a phone, anything
listening on a loopback port (another app) could then receive the session
token if the user typed such an address. **Mitigation (fixed in
`a129cff`):** on Android and iOS, plain HTTP to loopback is accepted only
when the native library is a debug build (`debug_assertions`);
`scripts/build-android.sh --release` refuses it. The desktop keeps it in
every build, as before. Covered by
`transport::tests::plain_http_to_localhost_follows_the_build` and
`mobile_release_builds_refuse_localhost_http`.

### AN21. Unreadable device settings turned "Confirm before filling" off (Low, fixed)
**Component:** `settings.rs`. A device-settings blob that exists but does
not open (damaged, replaced) fell back to the defaults, where "Confirm
before filling" is off. **Mitigation (fixed in `a129cff`):** a blob that
does not open, or a vault that cannot be read, answers the defaults with
"Confirm before filling" on; only a phone that never wrote its settings gets
the plain defaults. Covered by
`an_unreadable_settings_blob_keeps_confirm_before_filling_on`.

### AN22. Userinfo in the vault list (Low, fixed)
**Component:** `items.rs` `url_host`. A saved URL such as
`https://user:pw@host/` showed `user:pw@host` as the item's website in the
list. **Mitigation (fixed in `a129cff`):** the website is parsed with `url`
and shown as `host_str()` (plus a non-default port). Covered by
`the_website_shown_is_the_host_never_the_userinfo`.

### AN23. Concurrent Keystore key creation (Info, fixed)
`SecretKeyCipher.key()` now runs under the same lock as `delete()`, so two
first uses cannot each generate a key (`4beb735`). Keystore code; not
JVM-testable, covered by reading.

### AN24. Revealed values and row positions (Info, fixed)
The item screen's rows are keyed by field key, so a revealed value stays
with its own field when a reload adds or removes one (`4beb735`). Compose
behaviour; covered by reading.

### AN25. Revocation lists blocked by the cleartext rule (High, fixed)
First run on a Galaxy S24+ (Android 16): every sign-in to a server with a
Let's Encrypt certificate failed with `invalid peer certificate: Revoked`,
shown as "offline". The certificate was not revoked (not in the CA's
current CRL). rustls-platform-verifier's Android half validates with
`PKIXRevocationChecker` (`SOFT_FAIL`, `ONLY_END_ENTITY`); with no OCSP URL
in the certificate it fetches the CRL over plain HTTP, which the app's
`network_security_config.xml` forbade, and the library maps any failure in
that step to "revoked". The fix allows cleartext to `lencr.org` (and its
subdomains) in both build types. CRLs are signed by the CA, so plain HTTP
cannot forge one; it reveals to the network which CRL shard was fetched.
The file governs Android's HTTP stack only: Rust's transport still refuses
plain HTTP to the vault server. **Remaining limitation:** a self-hosted
server whose certificate comes from another CA that publishes only HTTP
CRLs will fail the same way until its host is added.

### AN26. `new-password` on a login's password (Medium, fixed)
dashboard.render.com's login marks its only password field
`autocomplete="new-password"`; the form finder read that as a sign-up and
offered nothing, not even on the email field. Now a single new-password
field beside a username, with no confirmation and no other password, is the
login's password. Two new-password fields (sign-up), a confirmation, a lone
new-password without a username (a reset page) and change-password forms
keep their old handling. Covered by `LoginFormFinderTest`.

### AN27. Split code boxes (Medium, fixed)
The Riot app's code step is six one-character inputs, and Android's
structure holds only the focused one, with an autofill hint of
`new-password`. Nothing classified it, so no code was offered. Now a
focused, visible, enabled text box limited to one character — by HTML
`maxlength` or, for native views, `ViewNode.maxTextLength` (now in
`FieldFacts`) — on a screen with no username or password is a code field;
when the row of boxes is visible, the first box. Covered by
`LoginFormFinderTest` (the Riot structure, a web row of six, native boxes,
and a one-character box beside a login that stays a login).

### AN28. Copying a code for split boxes (Info, mitigated)
Android lets the service fill only the box that was tapped, and the box
keeps one character, so filling the code there sets only its first digit.
Every code the planner offers now has a "Copy one-time code" row with a copy
icon. It fills nothing; tapping it opens `AutofillAuthActivity`, which asks
Rust for that login's code for the same target (refused otherwise) and
copies it through `SensitiveClipboard`: marked sensitive, cleared after the
vault's delay and on lock. **Remaining limitation:** while on the clipboard,
the foreground app and the keyboard can read the code (§22.10); it is a
30-second code, and copying needs the user's tap.

### AN29. Release APK built after a debug native build (Low, mitigated)
`scripts/build-android.sh` without `--release` leaves a debug library
(which accepts plain HTTP to loopback, AN20) in `jniLibs`. The release
workflow always runs `scripts/build-android.sh --release` on a clean runner
before `assembleGithubRelease`. A local release build still needs
`--release` first.

## Audits and full verification

Run on 2026-10-01 (WSL2, Linux 6.6.87.2-microsoft-standard-WSL2), at the
tip of `android-m1` plus this documentation commit. Postgres for the server
suites came from `scripts/test-server.sh`'s container
(`HAVENKEYS_TEST_DATABASE_URL` set).

| Command | Result |
|---|---|
| `cargo test --workspace` (with `HAVENKEYS_TEST_DATABASE_URL`) | 805 passed, 0 failed, across 53 test targets, server and sync-client suites against Postgres included |
| `cargo test -p havenkeys-mobile --features testing` | 42 passed (37 unit, 5 in `tests/regressions.rs`), 0 failed; re-run after the fix below |
| `cargo clippy --workspace --all-targets -- -D warnings` | First run **failed**: an unused `#[must_use]` `Forgotten` in a test in `crates/havenkeys-mobile/src/unlock.rs`. Fixed in a separate commit (the test now asserts the forget was saved); then clean |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok`. Warnings: `yoke-derive` 0.8.3 yanked (AN18), one licence allowance and six advisory ignores that match nothing |
| `cargo audit` | No vulnerabilities; 3 allowed warnings: `proc-macro-error` (unmaintained) and `glib` `VariantStrIter` (unsound), both through the desktop's GTK stack as in every earlier review, and the yanked `yoke-derive` |
| `pnpm test` | **1 failure** of 872 tests: the extension's timing check "attack 7: thousands of inputs cost a bounded amount of work" took 509–548 ms against a 500 ms budget, in three runs here. This branch does not touch the extension; the same file passes (42/42) when run alone. Desktop 97, protocol 27, UI 8, web 14 passed |
| `scripts/build-android.sh --release` (`ANDROID_NDK_HOME=…/ndk/30.0.16248370`) | Built `arm64-v8a`, `armeabi-v7a`, `x86_64`; bindings regenerated with no change |
| `./gradlew detekt testGithubDebugUnitTest lintGithubDebug assembleGithubRelease` | BUILD SUCCESSFUL. `forbidLogging` and detekt passed; 157 JVM unit tests, 0 failed (re-run with `--rerun-tasks`); `lintGithubDebug` passed; `app-github-release-unsigned.apk` built (R8, about 29 MB). The Kotlin compiler warns of two unused expressions in `AppContainer.kt` (`Unit` in `keystoreDelete`'s catch blocks) |
| `scripts/build-android.sh` (debug, afterwards) | Built; committed bindings unchanged |
| Instrumented tests (`connectedGithubDebugAndroidTest`) | **Not run**: no emulator or device |

## Secret-logging and trust-boundary review

* No Kotlin logging: `forbidLogging` passed (no `android.util.Log`,
  `println`, `print`, `printStackTrace`, `System.out/err` in any source),
  detekt's `ForbiddenImport`/`ForbiddenMethodCall` are on, and the Rust crate
  denies print macros. `jni`'s `LogErrorAndDefault` in `android_tls.rs` goes
  through the `log` crate, which has no logger installed in the app.
* `MobileError` carries a code and a fixed sentence; `CipherError` displays
  "keystore failure"; `UnlockBundle` and `AppIdentity` redact `Debug`;
  `DatasetPlan` and `Outcome.Ok` keep values out of `toString()`.
* Intents carry only a mode and an item ID. Fill values go into `Dataset`s
  and the `EXTRA_AUTHENTICATION_RESULT` the autofill API requires, nowhere
  else.
* Typed secrets are `remember`ed, never `rememberSaveable`d, and no
  ViewModel `UiState` holds a password, Secret Key, revealed value or code.
* Every `havenkeys-mobile` call that reads the vault starts with
  `MobileVault::unlocked` (an overdue auto-lock applies first, then the
  lock state is checked) and, for autofill, re-derives the target from the
  facts on every call. `autofill_search` returns no secret.

## Verified properties (host tests)

* A privileged browser's page on `evil.com` gets nothing for `github.com`;
  an http page stays http (`regressions.rs` attack 1, `app_target.rs`).
* An app with GitHub's package name and another certificate, and a WebView
  claiming `github.com`, get nothing; Chrome's package signed by someone else
  is an app (`regressions.rs` attack 2, `app_fill.rs`, `app_target.rs`).
* A locked vault fills, reveals and answers TOTP for nothing (attack 3).
* Malformed IDs and oversized targets are refused without a panic (attack 5).
* A bundle older than 14 days, from the future, from another boot, with an
  unknown boot count, truncated or tampered is refused; a wrong master
  password enrols nothing (`unlock_bundle.rs`, `regressions.rs`).
* `assetlinks.json`: redirects are not followed, oversized answers and
  non-DNS hosts are refused, one bad statement voids only itself, other
  relations, packages and certificates do not vouch, the cache keeps files a
  week and failures an hour and is bounded (`asset_links.rs`,
  `asset_links_fetch.rs`, `fuzz_asset_links_parser`).
* A Keystore cipher that fails never leaves the Secret Key in plaintext on
  disk (`key_file.rs`).
* A structure that does not name the calling package is ignored
  (`StructureNamesCallerTest`).

## Manual checklist (verification pending)

None of these has been run.

- [ ] Real phone, Android 14+: sign in by scanning the kit; unlock with password; enroll fingerprint; unlock with fingerprint; reboot → password required; add a fingerprint → bundle refused, password required.
- [ ] Chrome (Autofill using another service) and Firefox: login on github.com fills after a tap; github.com.evil.com (hosts file or a test domain) offers nothing; an http page does not get an https login.
- [ ] GitHub app: offered through Digital Asset Links or after "Search HavenKeys…" binding; a sideloaded app with the same package name and another key gets nothing.
- [ ] Google app (WebView sign-in): its WebView's domain is not trusted.
- [ ] Locked vault: "Unlock HavenKeys" appears; nothing fills without unlocking.
- [ ] Confirm before filling on: each fill asks; off: one tap fills.
- [ ] TOTP: OTP field after a login offers the code.
- [ ] Recents thumbnail is blank; screenshots are blocked.
- [ ] Airplane mode: unlock, reveal, TOTP and autofill work; sync shows offline.
- [ ] With Google Autofill (or another password manager) as the device's autofill service for other apps: HavenKeys' unlock, onboarding and biometric-enroll fields get no suggestion, and Google Autofill does not offer to save the master password or the Secret Key (AN19).
- [ ] An app with "display over other apps" covering the binding prompt or a gated row's activity: the tap through it is ignored (AN6).
- [ ] With the screen kept on and a TOTP login open, the vault locks at the auto-lock time (AN7); after the app sat frozen past the deadline, the next fill shows "Unlock HavenKeys" (AN9).
- [ ] Before the first release: build a release APK with R8 and smoke-test it on a phone (unlock, sync, reveal, autofill), so R8 has not stripped anything JNI or JNA reaches by reflection.

"Each fill asks" means each row opens HavenKeys (AN3). Also owed: one run of
`connectedGithubDebugAndroidTest` on an emulator or phone. Package
visibility was checked on a phone and fixed for the `github` flavor (AN10).

## Android M2 (editing and saving from Autofill)

**Scope:** branch `android-m2`: `crates/havenkeys-mobile` (`edit.rs`,
`save.rs`), `app_fill.rs` in the core, and `apps/android` (`ui/edit/`,
`autofill/SaveForm.kt`, `SaveCollector.kt`, `SaveRequestReader.kt`,
`onSaveRequest`). **Nothing here has run on a phone or an emulator.** New
findings are AN30–AN36 above.

Properties checked by host tests and `tests/round_trip.rs`: a write offline
is refused before staging; a stale revision becomes `item_changed_elsewhere`
and nothing is overwritten; a phone edit keeps custom fields, passkeys, app
bindings, "Sign in with" and a card's brand; a browser save adds a
whole-site rule for the page's own host and updates only a login matching
that page; an app save adds a login bound to the app, with no website, and
updates only a login matched to that app; a locked vault saves nothing.
JVM tests check that `SubmittedForm`, `SubmittedLogin` and `EditorState`
do not print values and that the ViewModel's state does not hold a draft.

Secret-leak grep over the diff
(`git diff main -- apps/android crates/havenkeys-mobile | grep -nE 'Log\.|println|toString\(\)|\$\{?(draft|login|value|password)'`):
every hit is a redacting `toString()` override, a test asserting that a
value is absent from `toString()`, or `toString()` on a non-secret (an app
label, a node's text value read only for a confirmed save and truncated).

### Manual checklist (Android M2, verification pending)

None of these has been run.

- [ ] Editor: create a login, a secure note and a card; edit each; delete one. Each shows in the list and on the desktop.
- [ ] Edit a login that has a custom field, a passkey and an app binding on the desktop: after a phone edit all three are still there.
- [ ] Change a password: the field loads the current value, stays read-only until loaded, and Save waits; leaving a field untouched keeps the stored value.
- [ ] Rotate the phone, change the theme and the font size with an open draft: the draft stays. Change the locale (or kill the process): the draft is gone.
- [ ] Lock (auto-lock or the lock button) with a draft open: the editor closes and the draft is gone; recents show a blank thumbnail.
- [ ] With another autofill service active: the editor's fields get no suggestion and no offer to save.
- [ ] Airplane mode: the editor shows the offline banner and refuses to save; reading still works.
- [ ] Edit the same item on the desktop and the phone, save on the phone second: "This item changed on another device." and the item reloads.
- [ ] A keyboard other than the default: confirm the editor's fields are not offered personalized suggestions (AN35: a keyboard may ignore the flag).
- [ ] Autofill save, Chrome and Firefox: sign in to a new site, confirm Android's save sheet; the login appears with that site only. Sign in again with a changed password: updated; with the same one: unchanged. github.com.evil.com does not update github.com's login.
- [ ] Autofill save, an app: sign in to a sideloaded test app, confirm; the login appears bound to that app, with no website. A second app with another certificate and the same label does not update it.
- [ ] A username-first sign-in on Android 10+: one login with both values. On Android 9: password step only (AN34).
- [ ] Lock before submitting: "HavenKeys locked before saving." (AN33); offline: "HavenKeys is offline. The login was not saved."
- [ ] HavenKeys' own screens never offer to save.
- [ ] Also owed from M1: `connectedGithubDebugAndroidTest` on an emulator or phone.

### Verification (2026-10-02, tip of `android-m2` plus this documentation)

| Command | Result |
|---|---|
| `cargo test -p havenkeys-core -p havenkeys-client -p havenkeys-mobile --features havenkeys-mobile/testing` | 568 passed, 0 failed |
| (not run: `cargo test --workspace`; the three Android-facing crates above were tested, not the server and desktop suites) | |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo test -p havenkeys-mobile --features server-tests --test round_trip` (Postgres container) | 1 passed |
| `cargo audit` | No vulnerabilities; the same 3 allowed warnings as Android M1 |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| `scripts/build-android.sh`, then `git status --porcelain` on `kotlin/uniffi` | Built; no change (bindings current) |
| `./gradlew detekt testGithubDebugUnitTest lintGithubDebug assembleGithubDebug` | BUILD SUCCESSFUL; 212 JVM unit tests, 0 failed |

## Android M3 (passkeys and Credential Manager)

**Scope:** branch `android-m3`: the core's `passkey/` (`rp.rs`, `app.rs`,
`vault.rs`, `webauthn.rs`), `asset_links.rs` and `app_target.rs`;
`crates/havenkeys-mobile` (`credentials.rs`, `passkey_json.rs`); and
`apps/android` (`credentials/`, `security/WindowHardening.kt`,
`BiometricGate`, the Autofill setup section, the manifest and
`res/xml/credential_provider.xml`). Design:
`docs/superpowers/specs/2026-10-01-android-app-design.md` §8. **Nothing here
has run on a phone or an emulator.** This is an internal review, not an
independent audit.

Properties checked by host tests (`regressions.rs`,
`passkeys_android.rs`, `tests/round_trip.rs`): an impostor app (right package,
another certificate) gets no passkey; an app vouched only for filling
(`handle_all_urls`) gets none; a stale or failed lookup vouches for nothing;
an origin that reaches Rust from a caller that is not a privileged browser
is refused (in the real flow Kotlin drops such an origin, so the caller is an
app and is denied unless the site vouches for it);
an app's `clientDataHash` is not signed; a locked vault offers and signs
nothing; a chosen login must match the app; a passkey made for an app works
on the website and back.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| AN37 | Medium | `CredentialGetActivity`, `PasskeyCreateActivity` | User verification is enforced in Kotlin only | Accepted |
| AN38 | Low | `asset_links.rs`, `credentials.rs` | An app's passkey stops working after 7 days offline | Accepted |
| AN39 | Low | `CallerFacts`, `credentials.rs` | Origin trust relies on Android's `getOrigin` plus Rust's list | Mitigated |
| AN40 | Low | `CredentialResults`, service | Mutable PendingIntents; request codes restart per process | Mitigated |
| AN41 | Info | `passkey_json.rs` | `clientDataJSON` in a browser response is a placeholder | Accepted |
| AN42 | Low | credential activities, service | An unexpected exception ends the request as a generic error | Accepted |
| AN43 | Low | `credential_password` | The password path asks no user verification when unlocked | Accepted |
| AN44 | Info | all of M3 | Not run on a device | Open |
| AN45 | Low | `WalletPlanner`, `WalletDatasets` | Card values in the autofill framework | Accepted |
| AN46 | Info | `AutofillAuthActivity`, `WalletConfirmation` | Gated card and identity rows confirm in HavenKeys | Mitigated |
| AN47 | Low | `StructureParser`, `CardFormFinder` | Frames depend on what the browser reports | Accepted |
| AN48 | Info | `StructureParser.isEmpty` | Emptiness is read from the structure | Accepted |
| AN49 | Info | `CardSaveReader`, `autofill_save_card` | Saving cards | Accepted |
| AN50 | Info | all of M4 | Not run on a device | Open |

### AN37. User verification is enforced in Kotlin only (Medium, accepted)
**Component:** `CredentialGetActivity`, `PasskeyCreateActivity`,
`BiometricGate.verifyUser`.
**Scenario:** WebAuthn's "user verified" flag is set by Rust, but the prompt
that earns it is Kotlin's. A modified app, or code in the process, could
call Rust's passkey operations without showing `BiometricPrompt`. Rust
cannot tell.
**Mitigation:** every sign-in and create shows `BiometricPrompt`
(`BIOMETRIC_STRONG` or the device screen lock), skipped only right after an
unlock in the same activity; a phone with no screen lock is refused. The
activities are not exported, the vault is unlocked only in this process, and
the signature needs the unlocked vault.
**Remaining:** the app process is trusted for user verification, as the
extension trusts the desktop's click. Code running in the process, or a
rooted phone, can sign without a prompt.

### AN38. Offline app passkeys after the 7-day cache (Low, accepted)
**Component:** `asset_links.rs`, `credentials.rs`.
**Scenario:** an app may use a passkey only if the site's `assetlinks.json`
vouched for it within 7 days. A phone offline longer cannot sign in to an
app with a passkey; an attacker who blocks the lookup can do the same.
**Mitigation:** the check fails closed (no passkey, nothing signed); the
answer refreshes as soon as the phone is online; browsers are unaffected
(their origin does not depend on the file).
**Remaining:** an availability cost, accepted over trusting an old answer.

### AN39. Reliance on Android's `getOrigin` plus Rust's list (Low, mitigated)
**Component:** `CallerFacts.kt`, `credentials.rs`, `app_target.rs`.
**Scenario:** an origin is valid only from a privileged browser. Android
decides, from the list Kotlin passes, whether the caller may report one; a
bug there, or a list entry for a browser that mishandles origins, would let
a page's origin be trusted.
**Mitigation:** Rust checks the caller's package and certificate against its
own list again and refuses an origin from any other caller (Kotlin already
drops an origin Android will not vouch for, so such a caller is treated as an
app and is denied unless the site grants it `get_login_creds`); the list
is the one autofill uses, changed only by hand
(`scripts/update-android-browsers.sh`), and the origin then goes through
`authorize_rp` like the extension's.
**Remaining:** a privileged browser is trusted to report the right origin.

### AN40. Mutable PendingIntents (Low, mitigated)
**Component:** `CredentialResults.kt`, `HavenCredentialService`.
**Scenario:** Credential Manager must add the request to the entries'
PendingIntents, so they cannot be immutable. A holder could try to alter or
redirect them; the per-process request-code counter restarts when the process
does, and `FLAG_UPDATE_CURRENT` then lets a new intent replace an old one
with the same code.
**Mitigation:** the intents are explicit and target only HavenKeys'
non-exported activities; the extras carry ids and the item title
(vault metadata, not a secret) and non-secret request data; Rust re-checks the caller, origin, RP ID and item on every
request, so a replaced intent can only ask Rust a question it answers again.
**Remaining:** a stale entry may open the wrong one of HavenKeys' own
screens; nothing is signed that Rust's checks would not allow.

### AN41. Placeholder `clientDataJSON` (Info, accepted)
**Component:** `passkey_json.rs`.
**Scenario:** for a browser, Rust signs the supplied `clientDataHash`, so the
`clientDataJSON` in the response is a placeholder that the browser replaces
with its own. A browser that did not replace it would send a value that does
not match the signature, and the site would refuse the sign-in.
**Mitigation:** none needed for security; the signature covers the browser's
own bytes, and Rust never signs a hash for an app (AN39's list decides who is
a browser).
**Remaining:** relies on browsers following Credential Manager's contract.

### AN42. An unexpected exception ends the request as a generic error (Low, accepted)
**Component:** `CredentialGetActivity`, `PasskeyCreateActivity`,
`CredentialUnlockActivity`, `HavenCredentialService`, `CallerFacts.kt`.
**Scenario:** an exception that no code maps to an error (a failed
read of the caller's signing information, an unexpected Android or UniFFI
failure) would crash the process, losing the unlocked vault, and leave
Credential Manager unanswered.
**Mitigation:** each entry point catches `Exception` (never
`CancellationException`) and answers exactly once: the get and create
activities and the service answer with `GetCredentialUnknownException` or
`CreateCredentialUnknownException`, and the unlock activity answers with no
entries. Unreadable signing information gives a caller with no certificates,
which Rust refuses. The exception message is never kept, logged or shown
(CLAUDE.md §39). Nothing is signed on this path.
**Remaining:** the user and the site see a generic failure without an
explanation.

### AN43. The password path asks no user verification when unlocked (Low, accepted)
**Component:** `credential_password`, `CredentialGetActivity`.
**Scenario:** with the vault unlocked, a password chosen through Credential
Manager is returned without `BiometricPrompt`, so someone holding an unlocked
phone can use it.
**Mitigation:** the same as direct fill (§22.4): the target rules of §22.5,
only logins with a username, auto-lock, and the lock on screen-off; the
value goes from Rust straight into the result Intent. Passkeys do ask.
**Remaining:** accepted for parity with direct fill.

### AN44. M3 has not run on a device (Info, open)
No part of Credential Manager integration (the service binding, the browser
`getOrigin` flow, `BiometricPrompt` from these activities, the UI) has run on
a phone or an emulator; the list below is what remains.

### AN45. Card values in the autofill framework (Low, accepted)
**Component:** `WalletPlanner`, `WalletDatasets`. While unlocked and
"Confirm before filling" is off, a card form's response carries the values
of up to 5 cards (number, code, expiry, name) for that form's allowed
frames, and the identity's non-document values. **Scenario:** any app or
https page that shows a card form gets rows for the user's cards.
**Mitigation:** the values go to Android's autofill framework (part of the
OS, already trusted with every keystroke), which hands the app only the
row the user taps. This is more exposure than the extension's pick, not
the same: the extension asks for a card's values only after the user
picks it, while here up to 5 cards' values sit in the framework before
any tap. Rust refuses http pages and cross-site frames. **Remaining:** the
framework holds up to 5 cards' values for the life of the fill session;
turn on "Confirm before filling" to keep them out of it. The user decided
this at planning (CLAUDE.md amendment of 2026-10-02). In Kotlin, the
UniFFI-generated data classes `CardValue`, `IdentityValue` and `SaveCard`
(like M1's `FillValues`) print their values in the generated `toString()`:
they must never be logged or interpolated into a string, and pass from the
repository straight into datasets.

### AN46. Gated card and identity rows confirm in HavenKeys (Info, mitigated)
**Component:** `AutofillAuthActivity`, `WalletConfirmation`,
`WalletConfirmDialog`. As in AN3, the app being filled can fire a gated row
itself, and rewrite its item ID or mode. Cards and the identity are not
bound to the app, so unlike logins Rust's answer would not limit what it
gets. **Mitigation:** a gated card or identity row (documents, "Confirm
before filling", every row after an unlock) shows a question naming the
card and the site or package in a `FLAG_SECURE` window that ignores
obscured touches; Rust is asked only after the user's tap, and the answer is
one dataset. The response to an unlock row carries no values. The card ID is
accepted only when Rust offers that card to this form.

### AN47. Frames depend on what the browser reports (Low, accepted)
**Component:** `StructureParser`, `CardFormFinder`. Rust leaves out frames
that are neither the tab's site nor a payment processor's card frame, but
it can only judge the frames the browser's structure names. A browser that
reports a cross-site iframe's fields under the page's own domain makes them
look like the page's. **Remaining:** the browser's own cross-frame autofill
policy then applies; HavenKeys cannot see more than the structure says.

### AN48. Emptiness is read from the structure (Info, accepted)
**Component:** `StructureParser.isEmpty`. To never overwrite, each field's
current value is read to decide whether it is empty; only the boolean is
kept. The values are already in the structure Android hands every autofill
service.

### AN49. Saving cards (Info, accepted)
**Component:** `CardSaveReader`, `autofill_save_card`. A card is saved only
after the user confirmed Android's save sheet; the number reaches Rust then.
A number already saved is `Unchanged`; a new card needs the server. A card
typed into a payment processor's iframe is not saved (the extension's
limitation too). We expect Android to show its sheet only when a value
changed from what was filled; this is to be verified on a device (AN50).
**Remaining:** Android keeps the `SaveInfo` and client state of the
session's last fill response only, so moving from the card form to an
address field after typing a card (a new response, for the identity) can
lose the card's save sheet. A save sheet attached to a payment processor's
frame ends in "HavenKeys could not save this card.", because Rust refuses
saves from processor frames.

### AN50. M4 has not run on a device (Info, open)
Card and identity classification, shaping, planning and saving are
verified by JVM unit tests, the host Rust tests and `tests/round_trip.rs`,
not on Android. The Android M4 checklist in `docs/android.md` is open.

### Manual checklist (Android M3, verification pending)

None of these has been run.

- [ ] A real Android 14+ phone; HavenKeys enabled in Passwords & passkeys (the Autofill setup button opens it).
- [ ] `webauthn.io` in Chrome: create a passkey, then sign in.
- [ ] `webauthn.io` in Firefox: create a passkey, then sign in.
- [ ] `github.com` in Chrome: create a passkey, then sign in.
- [ ] GitHub app: passkey sign-in works through Digital Asset Links; a sideloaded app with the same package name and another key gets nothing.
- [ ] The same passkey then signs in from the desktop extension (and one made on the desktop signs in on the phone).
- [ ] Locked phone vault: "Unlock HavenKeys" appears; after unlocking, the entries appear.
- [ ] Airplane mode: sign in works in a browser; create says HavenKeys is offline and saves nothing.
- [ ] A re-registration with `excludeCredentials` ends with "This account already has a passkey in HavenKeys."
- [ ] No screen lock on the phone: passkeys are refused; with one, a sign-in asks for the fingerprint, face or screen lock.
- [ ] A conditional create (Chrome offering to upgrade a password) is refused and nothing is saved.
- [ ] Credential Manager's password list offers only logins with a username, and none for a different site.
- [ ] The three credential screens block screenshots and show a blank recents thumbnail; a tap through another app's overlay is ignored.
- [ ] Also owed from M1: `connectedGithubDebugAndroidTest` on an emulator or phone.

### Manual checklist (Android M4, verification pending)

None of these has been run; the same list is in `docs/android.md`.

- [ ] Chrome, https checkout with number, expiry (one field) and CVV: rows show `•••• 1111 · 04/33`; tapping fills all three; a field already typed in stays.
- [ ] Chrome, checkout with month and year lists: both chosen.
- [ ] Chrome, Stripe Elements checkout: the card fills inside Stripe's frames.
- [ ] Chrome, http checkout: no card rows.
- [ ] An app's card form: rows; "Confirm before filling" on: HavenKeys asks "Fill ... in <package>?" first.
- [ ] Locked: "Unlock HavenKeys", then the rows, then HavenKeys asks before filling.
- [ ] An address form: the identity fills names, address and phone; a CPF field: "Fill CPF too" asks first; on http no document row.
- [ ] Type a new card and submit: Android's save sheet; saved; type it again: nothing new.
- [ ] Offline: saving a new card says the card was not saved.
- [ ] Cards fill in both Chrome and Firefox (each browser's first field may be its own address bar; the page's site must still be read from the first field inside the page).
- [ ] TalkBack reads the card rows; dark theme.

### Verification (2026-10-02, tip of `android-m3` plus this documentation)

| Command | Result |
|---|---|
| `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` | All test results ok, 0 failed; clippy clean |
| `cargo test -p havenkeys-mobile --features testing --test regressions` | 10 passed |
| `scripts/test-server.sh` and `cargo test -p havenkeys-mobile --features server-tests --test round_trip` (Postgres on port 5433) | Server suite passed (62, 15 and 1 tests in its binaries); round trip 1 passed |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` |
| `cargo audit` | No vulnerabilities; the same 3 allowed warnings |
| `scripts/build-android.sh`, then `git diff --exit-code apps/android/app/src/main/kotlin/uniffi` | Built; no diff (bindings current) |
| `./gradlew :app:testGithubDebugUnitTest :app:testPlayDebugUnitTest :app:detekt :app:lintGithubDebug :app:assembleGithubRelease` | BUILD SUCCESSFUL; 230 JVM unit tests per flavor, 0 failed |

### Verification (2026-10-02, Android M4)

Nothing below ran on a phone or an emulator (AN50).

| Command | Result |
|---|---|
| Branch review at `cf7eda9`: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings` | 922 passed, 0 failed; clippy clean |
| Branch review at `cf7eda9`: `cargo test -p havenkeys-mobile --features testing --test regressions`; `--features server-tests,testing --test round_trip` (Postgres on port 5433) | 15 passed; round trip 1 passed |
| Branch review at `cf7eda9`: `scripts/build-android.sh`, then `git diff --exit-code apps/android/app/src/main/kotlin/uniffi` | No diff (bindings current) |
| Branch review at `cf7eda9`: `./gradlew :app:testGithubDebugUnitTest :app:detekt :app:forbidLogging :app:lintGithubDebug :app:assembleGithubRelease` | BUILD SUCCESSFUL; 311 JVM unit tests, 0 failed |
| Final fixes: `cargo fmt --all -- --check` | Clean |
| Final fixes: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings` | 922 passed, 0 failed; clippy clean |
| Final fixes: `regressions`; `round_trip` (Postgres on port 5433) | 15 passed; 1 passed |
| Final fixes: `scripts/build-android.sh`, then `git diff --exit-code apps/android/app/src/main/kotlin/uniffi` | No diff (bindings current) |
| Final fixes: `./gradlew :app:testGithubDebugUnitTest`, three runs | 315 JVM unit tests, 0 failed, each run |
| Final fixes: `./gradlew :app:detekt :app:forbidLogging :app:lintGithubDebug :app:assembleGithubDebug` | BUILD SUCCESSFUL |
| Final fixes: `LoginFormFinderTest.thousandsOfInputsStayFast` with all 16 cores busy, 8 runs each (best of 5, ms) | Before: 47 22 45 53 43 46 31 41; after normalizing each field once: 20 31 33 35 27 29 31 30 |

## Phone-approved sign-in (2026-10-04)

**Scope:** branch `phone-approved-sign-in`: `crates/havenkeys-core/src/pairing.rs`
and the vault service's sealing of the envelope; `crates/havenkeys-server`
(`routes/pairings.rs`, `locate.rs`, `migrations/0002_pairings.sql`, limits);
`havenkeys-sync-client` and `havenkeys-client` (pairing calls);
`havenkeys-mobile` (`scan_pairing`, approve, deny); the desktop's
`PhoneSignInPanel` and Tauri commands; the Android Settings entry, scanner
and confirmation sheet. Design:
`docs/superpowers/specs/2026-10-03-phone-approved-sign-in-design.md`;
documentation: `threat-model.md` T13, `security-model.md` §23, `crypto.md`.
**The Android part has not run on a phone or an emulator.** This is an
internal review, not an independent audit.

| # | Severity | Component | Finding | Status |
|---|---|---|---|---|
| PA1 | Medium | whole feature | A deceived user who scans an attacker's code and taps Allow gives the attacker's device the vault | Accepted, documented |
| PA2 | Low | server, `pairing.rs` | A malicious server cannot substitute the key; it can lie about name and location | Mitigated |
| PA3 | Low | `pairings` table | `token` is plaintext until claimed, and removal is lazy | Accepted |
| PA4 | Low | `locate.rs`, `create`, confirmation sheet | Requester-chosen name and GeoIP label can carry bidi/zero-width characters; country code unchecked | Open |
| PA5 | Low | `create` | Rate-limit count and insert are not atomic | Open |
| PA6 | Low | `/v1/pairings*` | No per-route body limit on unauthenticated routes | Open |
| PA7 | Low | `approve`, `claim` | An unused session from a failed finish, or an approved-but-never-claimed device, stays until expiry or revoke | Accepted |
| PA8 | Info | Android deny | A failed deny call is ignored | Accepted |
| PA9 | Low | `VaultService` session | The core session now holds the vault key while unlocked | Accepted |
| PA10 | Info | `pairing.rs` | Envelope and payload parsers | Mitigated |
| PA11 | Info | logging | Secret logging | Mitigated |
| PA12 | Info | `claim` | Atomic single-use claim and 10-minute cut-off | Mitigated |
| PA13 | Info | Android | Not run on a device | Open |

### PA1. An attacker's code approved by a deceived user (Medium, accepted)
**Component:** the feature as a whole; the Android confirmation sheet.
**Scenario:** an attacker shows a QR code ("scan to verify your account")
and the user taps Allow on the phone, past the name, IP and location line
and the biometric prompt.
**Mitigation:** the confirmation names the device and shows the IP and
location the server saw; Allow needs biometrics or the device credential;
the code lives 2 minutes and works once; the approved device appears in
Devices as "Approved by <phone>" and can be revoked.
**Remaining:** nothing in the protocol distinguishes a deceived user from
a willing one. The device receives the vault key and Secret Key, so it can
read and change the vault without the master password until revoked, and
revoking cannot take back what it read. Documented in `threat-model.md` T13.

### PA2. A malicious server (Low, mitigated)
**Component:** server routes; `PairingLink`, `seal`.
**Scenario:** the server swaps the desktop's public key, or lies about the
requester.
**Mitigation:** the public key travels from the desktop's screen to the
phone's camera, not through the server, so it cannot be swapped; the
envelope's `info` binds server and pairing id. The server can invent the
name or location text shown on the phone (see PA4); that is harmless
without the private key, but it can make a request look more or less
trustworthy. A server can also create requests of its own and answer a
claim with a token of its choice; the desktop then holds a session it
cannot use to read anything the server does not already hold.
**Remaining:** the confirmation's text is only as honest as the server.

### PA3. `pairings.token` in plaintext until claimed (Low, accepted)
**Component:** `pairings` table, `approve`, `claim`, `create`.
**Scenario:** someone who can read the database while a pairing is approved
and unclaimed reads the new device's session token.
**Mitigation:** the claim clears token and envelope; the claim refuses rows
older than 10 minutes; the token's hash is what `sessions` stores; the
envelope is ciphertext the server cannot open. The window is intended to be
10 minutes, but removal happens only in the next `POST /v1/pairings` (any
caller), so on a server that sees no new pairings the row, with its token,
can stay until the next create; the token itself stops working after 24
hours, but the plaintext string stays in the row.
**Remaining:** a database reader in that window gains a session for that
device, but no key material. A server that only ever sees approved,
unclaimed pairings is rare; a periodic task would close it.

### PA4. Bidi and zero-width characters in what the phone shows (Low, open)
**Component:** `clean_device_name`, `locate.rs::label`, the Android sheet.
**Scenario:** the unauthenticated requester chooses `deviceName`; it may
contain Unicode bidirectional or zero-width characters that reorder or
hide text on the phone's security confirmation ("Desktop – Linux" can be
made to read as something else). The GeoIP label from the database drops
control characters but not format characters, and the country code is not
checked to be two ASCII letters.
**Mitigation:** length (64 characters) and control characters are limited;
the IP is the server's own, not the requester's; Allow still needs
biometrics.
**Remaining:** a spoofing aid for PA1. Fix by rejecting Unicode format
(`Cf`) and bidi characters in the name and label and requiring two ASCII
letters for the country code.

### PA5. Create rate limit is not atomic (Low, open)
**Component:** `routes/pairings.rs::create`.
**Scenario:** the per-IP count and the insert are separate statements, so
parallel creates from one IP can pass the check together and exceed the 10
and 3 caps.
**Mitigation:** the overshoot is bounded by the number of concurrent
requests and the global limits; each row is small and removed after 10
minutes.
**Remaining:** a small, bounded flood of rows from one source. A
transaction with an advisory lock per IP would close it.

### PA6. No per-route body limit on unauthenticated routes (Low, open)
**Component:** `POST /v1/pairings`, `POST /v1/pairings/{id}/claim`.
**Scenario:** an anonymous caller sends bodies up to the server's global
limit.
**Mitigation:** the global body limit applies; JSON bodies are parsed with
unknown fields refused and every field length-checked; the limits on
fields are enforced after parsing.
**Remaining:** a tighter limit (about 1 KiB) on these two routes would cut
the work an anonymous caller can cause.

### PA7. Unused sessions and unclaimed devices (Low, accepted)
**Component:** `approve`, `claim`, Devices.
**Scenario:** if the desktop fails after the claim (header check fails, it
crashes), the session token it received stays valid until it expires (24
hours) or the device is revoked. A pairing that is approved but never
claimed leaves a registered device in Devices.
**Mitigation:** the token is in a 24-hour session only; the user can revoke
the device in Devices ("Approved by <phone>"); a second approval for the
same device replaces its earlier sessions.
**Remaining:** a stray device or session until expiry or revoke. It cannot
open the vault: only the envelope's recipient has the keys.

### PA8. A failed deny is ignored (Info, accepted)
**Component:** Android confirmation sheet.
**Scenario:** the deny call fails (offline); the app does not retry or report it.
**Mitigation:** the request expires in 120 seconds and nothing was sealed;
the desktop shows an expired code.
**Remaining:** the request stays approvable for up to two minutes by
someone else who has a session on the account and the link.

### PA9. The vault key in the unlocked session (Low, accepted)
**Component:** `VaultService`, `crypto::keys::Key256`.
**Scenario:** a memory read of the unlocked core process now finds the
vault key as well as the data key.
**Mitigation:** the same class of exposure as the data key, which already
opens every item; the vault key is zeroized on lock, never returned by an
API, never crosses Tauri, uniffi or Kotlin, and the envelope is sealed
inside the core.
**Remaining:** memory zeroization is best effort (`security-model.md` §8).

### PA10. Envelope and payload parsers (Info, mitigated)
**Component:** `PairingKeys::open`, `PairingPayload::decode`,
`PairingLink::parse`.
**Mitigation:** every length is checked before use; versions and suites are
exact; trailing bytes are refused; errors do not say which check failed;
`garbage_never_panics` (deterministic garbage over the link, envelope and
payload parsers) and tampering tests pass.
**Remaining:** no `cargo-fuzz` target exists for these three parsers (the
spec asked for one); the deterministic tests are the substitute.

### PA11. Secret logging (Info, mitigated)
**Component:** server routes, core types, desktop and Android.
**Mitigation:** the server logs only the outcome and ids; `PairingKeys`,
`ClaimSecret` and `PairingPayload` print as `<redacted>` (test
`nothing_secret_is_printed`); Android's `forbidLogging` check still runs
with `detekt`. There is no dedicated `no_logging` test for the server's
pairing routes.
**Remaining:** none known.

### PA12. Claim is atomic and single-use (Info, mitigated)
**Component:** `claim`.
**Mitigation:** one transaction under `FOR UPDATE`; the right secret gets
the token once; rows older than 10 minutes are refused even if not yet
deleted; wrong secrets count against the IP.
**Remaining:** none known.

### PA13. Not run on a device (Info, open)
The Android scanner, the confirmation sheet, the biometric gate and the
"no screen lock" path were checked by JVM unit tests only.

### Tests relied on

`cargo test -p havenkeys-core` (the `pairing` tests); `scripts/test-server.sh`
(`crates/havenkeys-server/tests/pairings.rs`); `havenkeys-client`'s
`tests/pairing.rs` and `havenkeys-mobile`'s `tests/round_trip.rs`; the
desktop's vitest suite; Android's `:app:testGithubDebugUnitTest`.

### Audits (2026-10-04, branch tip)

| Command | Result |
|---|---|
| `cargo audit` | No vulnerabilities; 2 allowed warnings (`proc-macro-error` RUSTSEC-2024-0370 unmaintained, `glib` RUSTSEC-2024-0429 unsound), both through Tauri's Linux stack and unchanged |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok` (one unused ignore, RUSTSEC-2025-0100, is reported as matching no crate) |
| `npm audit --omit=dev` in `apps/desktop` | Not possible: the workspace uses pnpm and has no `package-lock.json` (ENOLOCK) |
| `pnpm audit --prod` in `apps/desktop` | No known vulnerabilities |

New dependencies for this feature: `hpke` 0.14.1 (RustCrypto family, the
envelope) and `maxminddb` 0.32.0 (the optional location database); both
pass `cargo deny`.
