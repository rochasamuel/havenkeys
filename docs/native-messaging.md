# Native Messaging

How the browser extension talks to the desktop app, and where each check
happens.

> This software has not undergone an independent security audit.

## 1. Overview

```text
┌──────────── Browser ────────────┐     ┌──── Native host ────┐     ┌────────── Desktop app ──────────┐
│ popup ──runtime msg──► background│stdio│ havenkeys-native-   │local│ havenkeys-bridge ──► havenkeys- │
│ (page content never reaches it)  ├────►│ host                ├────►│ (authorize, rate    │   core      │
│                                  │◄────┤ validate + relay    │◄────┤  limit, dispatch)   (origin     │
└──────────────────────────────────┘     └─────────────────────┘sock │                      binding)   │
                                                                     └──────────────────────────────────┘
```

| Component | Code | Trusted for |
|---|---|---|
| Extension background worker | `apps/extension/src/background/` | Nothing on the desktop side. It picks the page URL from the browser's tab data, never from page content |
| Native host | `crates/havenkeys-native-host` | Nothing. It has no keys and cannot open the vault. It validates and relays messages |
| Bridge | `crates/havenkeys-bridge` | Enforcing the rules in §5 |
| Core | `crates/havenkeys-core` | Origin binding (`find_matches`, `fill_for_page`, `totp_for_page`) |

The browser starts the native host as a child process for each
`connectNative` port. The host does not open the vault itself. It connects
to the running desktop app over a local socket. If the app is not running,
every request gets `desktop_unavailable`, and the host tries to connect
again on the next request.

The wire types live in `crates/havenkeys-protocol` (Rust, the authority)
and `packages/protocol` (TypeScript mirror). The host links only the protocol
crate, not the core.

## 2. Setup

1. Build the host: `pnpm build:host` (release build at
   `target/release/havenkeys-native-host`).
2. Register it with your browsers:
   * Linux/macOS: `scripts/install-native-host.sh` (or pass the binary path).
   * Windows: `powershell -ExecutionPolicy Bypass -File scripts\install-native-host.ps1`.
   * Remove with `--uninstall` / `-Uninstall`.
3. Build the extension: `pnpm build:extension`, which writes
   `apps/extension/dist/chrome` and `dist/firefox`.
4. Load it:
   * **Chrome/Edge/Brave:** `chrome://extensions` → Developer mode → *Load
     unpacked* → `apps/extension/dist/chrome`. The `key` in the manifest pins
     the ID to `olbclkanfbmilnmfhoojgcnpgdmilfmf`.
   * **Firefox:** `about:debugging` → This Firefox → *Load Temporary Add-on* →
     `apps/extension/dist/firefox/manifest.json` (ID `havenkeys@havenkeys.app`).
5. Start HavenKeys, unlock it, and turn on Settings → *Browser extension*.
   Browser integration is **off by default**, for new vaults and for vaults
   created before this setting existed, because while it is on, other
   programs running as you can use it too (§8).

The browser, the host and the desktop app must all run on the same OS. A
Windows browser cannot reach a desktop app running inside WSL.

## 3. Wire format

**Framing (both hops):** a 4-byte unsigned length in native byte order
(little-endian on every supported platform), followed by that many bytes of
UTF-8 JSON. The length is checked before anything is allocated.

**Request:**

```json
{"v":1,"id":7,"request":{"type":"find_matches","url":"https://github.com/login"}}
```

| `type` | Fields | Needs unlocked + integration on | Rate class |
|---|---|---|---|
| `status` | none | no | none |
| `lock` | none | no | none |
| `find_matches` | `url` | yes | lookup |
| `fill_item` | `itemId` (UUID), `url` | yes | secret |
| `get_totp` | `itemId` (UUID), `url` | yes | secret |

There is deliberately no request to unlock the vault, list items, search,
reveal notes, or read a TOTP secret.

**Responses** answer one request ID and carry exactly one of `result` or
`error`:

```json
{"v":1,"id":7,"result":{"type":"find_matches","matches":[{"id":"…","title":"GitHub","username":"octo","hasTotp":true,"strength":"same_host"}]}}
{"v":1,"id":8,"result":{"type":"fill_item","username":"octo","password":"…"}}
{"v":1,"id":9,"result":{"type":"get_totp","code":"123456","period":30,"secondsRemaining":12}}
{"v":1,"id":10,"error":{"code":"denied","message":"This item is not saved for this website."}}
```

`id` is `null` only when a request was so broken that its ID could not be
read.

**Events** have no ID: `{"v":1,"event":{"type":"locked"}}`. The types are
`locked`, `unlocked`, and `disconnected`. The host sends `disconnected` when
the desktop app goes away.

**Error codes:** `locked`, `busy`, `no_vault`, `not_found`, `denied`,
`invalid_input`, `decryption`, `corrupted`, `malformed`, `too_large`,
`unsupported_version`, `rate_limited`, `integration_disabled`,
`desktop_unavailable`, `internal`. Messages are fixed strings chosen by the
Rust side. The host replaces any message text coming from the socket with
its own fixed text, so no input or secret can be echoed back through an
error.

### Limits

| Limit | Value | Where |
|---|---|---|
| Request frame | 16 KiB | host (stdin) and bridge (socket) |
| Response frame | 256 KiB (Chrome's own cap is 1 MiB) | bridge, host |
| URL | 4096 bytes | extension, host, bridge |
| Suggestions per `find_matches` | 50 | bridge, host, extension |
| Concurrent host connections | 8 | bridge |
| Lookup rate | burst 60, then 5/s | bridge (global) |
| Secret rate | burst 10, then 1 per 2 s | bridge (global) |
| Request timeout | 10 s | extension |
| Idle port | closed after 60 s | extension |

## 4. Validation at each hop

Every hop parses into strict types. Unknown request types, unknown fields at
any level, wrong field types, a missing or different `v`, over-long URLs and
invalid UUIDs are all rejected.

| Hop | On a malformed message | On an oversized frame |
|---|---|---|
| Host ← extension | Answers `malformed`, never forwards it, keeps running | Skips the payload with a fixed buffer, answers `too_large`, keeps running |
| Bridge ← host | Answers `malformed`, connection stays open | Answers `too_large` from the length header alone, then closes the connection |
| Host ← bridge | Drops the message | Treats it as a disconnect |
| Extension ← host | Ignores the message; the request times out | n/a (browser-enforced) |

The host re-encodes each validated request from its typed form, so the
desktop only ever sees canonical JSON.

## 5. Authorization (desktop side)

For every request the bridge and core check, in this order:

1. **Parses** as a known request of protocol version 1 (§4).
2. **Rate limit:** lookups and secret requests draw from separate global
   token buckets. Denied requests also consume tokens, so probing is limited
   too.
3. **Vault unlocked:** otherwise `locked`.
4. **Integration enabled** in the (encrypted) settings: otherwise
   `integration_disabled`.
5. **Origin binding (core):** `fill_item` and `get_totp` succeed only if the
   item is a login whose own website rules match `url`, using the rules in
   `autofill.md`. The extension's opinion about which item belongs to a page
   is never used. An unknown item ID, a secure note, a login for another
   site, and a login with no TOTP all return the same `denied`, so IDs cannot
   be probed.
6. **Secret minimization:** `find_matches` returns ID, title, username, a
   has-TOTP flag and the match strength. `fill_item` returns the username and
   password only. `get_totp` returns the current code only; the secret never
   leaves the core.

`lock` locks the vault exactly like the lock button: the session is dropped,
the clipboard is cleared if it still holds our value, the UI is notified, and
every connected host gets a `locked` event.

Extension requests do **not** reset the auto-lock idle timer. Using the
browser does not keep the vault open.

## 6. Transport security

### Local socket

* **Linux/macOS:** a Unix domain socket at
  `$XDG_RUNTIME_DIR/havenkeys/bridge.sock`. On macOS the fallback is
  `$TMPDIR/havenkeys/`, then `~/.cache/havenkeys/`.
  * The desktop creates the `havenkeys` directory with mode `0700`, and
    refuses to serve if it is a symlink, owned by another user, or has any
    group/other permission bits.
  * The host makes the same check before connecting, so it will not talk to
    a socket in a directory another user prepared.
  * The desktop checks each connection's peer effective UID
    (`SO_PEERCRED`/`getpeereid`) and drops other users.
  * A leftover socket file is removed at startup only if nothing answers on
    it. If another HavenKeys instance answers, the second instance runs with
    browser integration off.
  * Linux abstract-namespace sockets are not used, because they have no file
    permissions.
* **Windows:** a named pipe `\\.\pipe\havenkeys-bridge-<16 hex chars>`,
  named after a hash of the user's profile path so that two users never
  share a name. The desktop creates it with an explicit protected DACL,
  `D:P(A;;GA;;;OW)`, which grants access to the pipe's owner only. The default
  DACL would give every user read access. That is not enough to send
  requests, but it is enough to take connection slots and watch lock/unlock
  events. See §8 for the squatting caveat.
* Connections that send nothing for 10 minutes are closed on Linux/macOS, so
  idle or half-sent connections cannot hold slots. Named pipes have no
  receive timeout.

### Who may launch the host

* The browser enforces the host manifest's `allowed_origins` (Chromium) or
  `allowed_extensions` (Firefox). Each lists only the HavenKeys extension.
* The host also checks the caller argument the browser passes, and exits with
  status 2 for anything else. This is a second check for honest callers, not
  a security boundary: any local process can start the host with the right
  argument (§8).
* In release builds the host ignores `HAVENKEYS_BRIDGE_ENDPOINT`, a debug-only
  override that the tests use.

### Hygiene

The host installs a silent panic hook, writes only protocol frames to stdout,
and logs nothing. Buffers that can hold secrets (frames, responses) are
zeroized on drop.

## 7. Extension side

**Permissions:** `nativeMessaging` and `activeTab`. There are no host
permissions and no content scripts yet (Phase 5). `activeTab` lets the
background worker read the URL of the tab where the user clicked the toolbar
button, and only that tab.

* The background worker is the only code that talks to the native host.
* It accepts runtime messages only from its own popup page
  (`sender.id === runtime.id`, no `sender.tab`, `sender.url` is `popup.html`).
* `externally_connectable` is empty (Chromium), so web pages and other
  extensions cannot message it.
* The popup never supplies a URL. The worker reads the active tab's URL
  itself for each request and removes credentials, query and fragment before
  sending it. Non-http(s) pages are never sent.
* Every message from the host goes through `parseIncoming` (exact shapes
  only), and each result must match the type of the request it answers.
* Nothing is stored: no `chrome.storage`, no caches.
* The popup builds its DOM with `createElement`/`textContent` only. It never
  shows a password. A TOTP code is shown on click and removed when it
  expires.
* CSP for extension pages is
  `default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`.

## 8. Known limitations

* **Same-user processes can use the bridge.** Any program running as you
  can connect to the socket, or start the native host itself, and make the
  same requests as the extension while the vault is unlocked. Such a program
  could enumerate matches for sites it guesses, and fetch passwords for them
  within the rate limit. Malware running as your user is out of scope
  (`threat-model.md` §4), but this path is easier than reading process
  memory. Possible future mitigations include verifying the peer's code
  signature (as 1Password does), a per-browser pairing approval in the
  desktop UI, or requiring desktop confirmation for each fill.
* **The extension ID is not a secret.** The public key in the Chromium
  manifest pins the ID of unpacked installs. Anyone can load a different
  unpacked extension with the same key (Developer mode is required). A
  store-published build would have its ID enforced by the store.
* **Windows pipe squatting.** Another user logged into the same machine
  could create your pipe name before HavenKeys starts, because the name is
  derived from your profile path and is predictable. Your native host would
  then talk to their pipe, which would receive your page URLs and could
  return fake answers. It could not read your vault. The server-process check
  (`GetNamedPipeServerProcessId`, then owner SID) is not implemented yet.
* **Windows is unverified.** The Windows code paths compile but have not been
  run. That includes the owner-only DACL: if HavenKeys runs elevated, the
  pipe's owner is the Administrators group, and a non-elevated host will be
  refused. That failure is closed, not open.
* **Same-user denial of service.** On Windows, a process running as you can
  hold all 8 connection slots indefinitely.
* **Rate limits are global,** so a noisy caller can exhaust them for the
  real extension (denial of service only).
* **Snap/Flatpak browsers** run the host in a sandbox with a different
  runtime directory, so it will not find the socket. Use a non-sandboxed
  browser build.
* **Memory:** responses are serialized into pre-sized, zeroize-on-drop
  buffers, and the host parses desktop messages without serde's untagged
  buffering. Some copies are still out of our control: serde's buffering of
  internally tagged results when a string needs unescaping, the standard
  library's stdout buffer in the host, and the browser's own copies.
* **Fill confirmation happens in the browser.** The explicit user action
  (clicking a suggestion) happens in extension UI. The desktop does not
  prompt for each fill.

## 9. Tests

| Test | Covers |
|---|---|
| `crates/havenkeys-protocol/tests/messages.rs` | A5 parsing cases, version handling, error text never echoes input, deterministic fuzz (50 000 cases) |
| `crates/havenkeys-protocol/src/frame.rs` | A6 framing: rejected before allocation, resync after discard, truncated streams |
| `crates/havenkeys-bridge/tests/bridge.rs` | A1 wrong origin, A2 arbitrary item ID, A3 locked, integration switch, rate limit, secret minimization, and over a real socket: A5, A6, lock events, connection limit, second-instance and stale-socket handling, non-private directory refusal |
| `crates/havenkeys-native-host/tests/host.rs` | Runs the real host binary: relaying, Firefox/Chrome caller check, desktop not running, A5/A6 at the host, truncated input, and a hostile fake desktop (invalid messages dropped, spoofed error text replaced, `disconnected` event) |
| `packages/protocol/src/index.test.ts` | TS validator accepts exact shapes only |
| `apps/extension/src/**/*.test.ts` | Native client (ID correlation, timeouts, host loss, idle close, type mismatch), popup request validation, URL stripping, the popup never supplying URLs |
