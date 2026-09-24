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
and "Remove this device" limitations it introduced. Numbering continues in
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
| S18 | Info | Desktop | If installing or connecting to the platform keychain fails at startup, the device is treated as having no keychain for the rest of that run (the Secret Key may be asked for) | Accepted, documented (`server-sync.md` §7) |
| S19 | Low | Desktop | A keychain `set` that timed out and completes later could re-add an entry after "Remove this device" deleted it — a narrow race between an abandoned write and a delete | Accepted, documented (`server-sync.md` §7) |
| S20 | Info | Desktop | Any process running as the user can usually read this computer's keychain entry (Linux Secret Service, Windows Credential Manager); macOS may prompt. Same trust boundary as the `device.json` fallback it replaces (K1) | Accepted, documented (`server-sync.md` §7, `security-model.md` §13) |
| S21 | Info | Desktop | "Remove this device" renames the vault file (`vault.sqlite3.removed-<timestamp>`) instead of deleting it; it stays on disk as ciphertext, recoverable only with the master password and the Secret Key from the Emergency Kit | Accepted, documented (design §6.1) |
| S22 | Info | Desktop | "Remove this device" requires an unlocked vault, so it is not reachable when the vault fails to open at startup | Accepted, known gap |
| S23 | Low | Server | A device whose online unlock fallback logs in but then fails locally (for example a header that fails to verify) leaves a server session live until it expires (24 h) | Accepted |
| S24 | Info | Deployment | The desktop and server must be upgraded together: `PUT /v1/vault/header` is gone, so an old desktop cannot publish a header to a new server | Accepted, operational |
| S25 | Info | Verification | The Windows and macOS keychain backends (`windows_native_keyring_store`, `apple_native_keyring_store`) were compiled but not exercised in this environment | Open, not yet verified on Windows/macOS |

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
