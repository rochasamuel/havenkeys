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

Still open for Phase 6: Windows pipe owner check (P10), desktop confirmation
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
    stripped.
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
