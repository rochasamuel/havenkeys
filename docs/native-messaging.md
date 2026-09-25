# Native Messaging

How the browser extension talks to the desktop app, and where each check
happens.

> This software has not undergone an independent security audit.

## 1. Overview

```text
┌──────────── Browser ────────────┐     ┌──── Native host ────┐     ┌────────── Desktop app ──────────┐
│ popup, menu/save frames,         │stdio│ havenkeys-native-   │local│ havenkeys-bridge ──► havenkeys- │
│ content scripts ──► background   ├────►│ host                ├────►│ (authorize, rate    │   core      │
│                                  │◄────┤ validate + relay    │◄────┤  limit, dispatch)   (origin     │
└──────────────────────────────────┘     └─────────────────────┘sock │                      binding)   │
                                                                     └──────────────────────────────────┘
```

| Component | Code | Trusted for |
|---|---|---|
| Extension background worker | `apps/extension/src/background/` | Nothing on the desktop side. It picks page URLs from the browser's sender and tab data, never from page content |
| Native host | `crates/havenkeys-native-host` | Nothing. It has no keys and cannot open the vault. It validates and relays messages |
| Bridge | `crates/havenkeys-bridge` | Enforcing the rules in §5 |
| Core | `crates/havenkeys-core` | Origin binding (`find_matches`, `fill_for_page`, `totp_for_page`; for passkeys `authorize_rp` and the stored rpId) |

The browser starts the native host as a child process for each
`connectNative` port. The host does not open the vault itself. It connects
to the running desktop app over a local socket. If the app is not running,
every request gets `desktop_unavailable`, and the host tries to connect
again on the next request.

The wire types live in `crates/havenkeys-protocol` (Rust, the authority)
and `packages/protocol` (TypeScript mirror). The host links only the protocol
crate, not the core.

## 2. Setup

### Installed app

Install the desktop app, open it once, and install the extension from the
Chrome Web Store or addons.mozilla.org. Then unlock HavenKeys and turn on
Settings → *Browser extension* (step 5 below).

The installers ship `havenkeys-native-host` next to the app binary (a Tauri
`externalBin`, declared only in `apps/desktop/src-tauri/tauri.bundle.conf.json`
so that tests and `tauri dev` do not need it). At every start the app
registers it with the user's browsers (`apps/desktop/src-tauri/src/native_host.rs`,
`crates/havenkeys-native-host/src/register.rs`):

| OS | What the app writes |
|---|---|
| Linux | `<profile>/NativeMessagingHosts/com.havenkeys.bridge.json` for each Chromium browser whose profile directory exists under `$XDG_CONFIG_HOME` (Chrome, Chrome Beta, Chromium, Brave, Edge, Vivaldi), and `~/.mozilla/native-messaging-hosts/com.havenkeys.bridge.json` if `~/.mozilla` exists |
| macOS | The same files under `~/Library/Application Support/<browser>/` (Firefox: `Mozilla/NativeMessagingHosts/`) |
| Windows | `%LOCALAPPDATA%\HavenKeys\com.havenkeys.bridge.{chrome,firefox}.json`, and `HKCU\Software\<browser>\NativeMessagingHosts\com.havenkeys.bridge` pointing to them for Chrome, Edge, Brave, Chromium, Vivaldi and Firefox |

Only our own manifest files and keys are written, per user, with no
administrator rights, and a file is rewritten only when its content changed.
The manifests admit only the published extension IDs (§8). An AppImage, or
a macOS app Gatekeeper runs from a translocated path, runs from a directory
that disappears on exit, so there the host is first copied to the app's data
directory (`native-host/`) and that copy is registered. A build without the
sidecar (`tauri dev`, `cargo run`) registers nothing and leaves a hand-made
registration alone.

### From source

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
     unpacked* → `apps/extension/dist/chrome`. Chrome Web Store rejects a
     manifest `key` field on upload, so the manifest no longer pins an ID:
     unpacked loads get an ID derived from the folder's path (stable on one
     machine, different on another), and once published the Chrome Web Store
     assigns the extension's permanent ID at first upload. That ID is
     `fmmfkakdkkcfpdnfmbngnlelbfaogafo`, and it is what
     `CHROME_EXTENSION_ORIGIN` in `crates/havenkeys-native-host/src/lib.rs`
     and `CHROME_ORIGIN` in `scripts/install-native-host.sh` and
     `install-native-host.ps1` allow. Local testing of an unpacked load needs
     those edited to the ID shown in `chrome://extensions`.
   * **Firefox:** `about:debugging` → This Firefox → *Load Temporary Add-on* →
     `apps/extension/dist/firefox/manifest.json` (ID `havenkeys@havenkeys.app`).
5. Start HavenKeys, unlock it, and turn on Settings → *Browser extension*.
6. Optional: to get suggestions when clicking login fields and offers to save
   logins, open the extension's options (the *Settings* button in the popup)
   and turn on *Suggestions in login fields*. Reload tabs that were already
   open.
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
| `find_matches` | `url`, `topUrl`? | yes | lookup |
| `fill_item` | `itemId` (UUID), `url`, `topUrl`? | yes | secret |
| `get_totp` | `itemId` (UUID), `url`, `topUrl`? | yes | secret |
| `generate_password` | none | yes | lookup |
| `check_login` | `url`, `topUrl`?, `username` (string or null), `password` | yes | secret |
| `save_login` | `url`, `topUrl`?, `username`, `password`, `itemId` (UUID or null) | yes | secret, plus one update per item per 10 min |
| `find_passkeys` | `url`, `topUrl`?, `rpId`, `allowCredentials` (list) | yes | lookup |
| `passkey_get` | `itemId` (UUID), `credentialId`, `url`, `topUrl`?, `rpId`, `challenge` | yes | secret |
| `check_passkey_create` | `url`, `topUrl`?, `rpId`, `userName`, `excludeCredentials` (list), `conditional` (bool) | yes | lookup |
| `passkey_create` | `url`, `topUrl`?, `rpId`, `challenge`, `userHandle`, `userName`, `displayName` (string or null), `itemId` (UUID or null), `conditional` (bool) | yes | secret; a server write |
| `passkey_status` | `url`, `topUrl`? | yes | lookup |

`topUrl` is present only when `url` is an iframe. It is the tab's top-level
page, and items must match both (`autofill.md`, Frames). Optional fields may
be omitted, which means null.

`conditional` on `check_passkey_create` and `passkey_create` marks the
site's automatic passkey upgrade (`create()` with `mediation:
"conditional"`, right after a HavenKeys password fill). It does not change
either request's shape, only what the core checks: `check_passkey_create`'s
result then carries a real `upgrade` decision instead of always `none`, and
`passkey_create` refuses (`denied`) unless that decision is still `auto` for
exactly the item named (`autofill.md`, Automatic upgrade).

There is deliberately no request to unlock the vault, list items, search,
reveal notes, read a TOTP secret, read a passkey's private key, delete items
or passkeys, or change anything except one login's password or passkeys.
There are two writes. `save_login` can add a login for the page it names, or
replace the password of a login that matches that page; the old password is
kept in the item's history. `passkey_create` adds a passkey (see
[Passkeys](#passkeys) below).

**Responses** answer one request ID and carry exactly one of `result` or
`error`:

```json
{"v":1,"id":7,"result":{"type":"find_matches","matches":[{"id":"…","title":"GitHub","username":"octo","hasTotp":true,"strength":"same_host"}]}}
{"v":1,"id":8,"result":{"type":"fill_item","username":"octo","password":"…"}}
{"v":1,"id":9,"result":{"type":"get_totp","code":"123456","period":30,"secondsRemaining":12}}
{"v":1,"id":11,"result":{"type":"generate_password","password":"…"}}
{"v":1,"id":12,"result":{"type":"check_login","action":"update","itemId":"…"}}
{"v":1,"id":13,"result":{"type":"save_login","itemId":"…"}}
{"v":1,"id":14,"result":{"type":"find_passkeys","passkeys":[{"itemId":"…","credentialId":"…","title":"GitHub","userName":"octo"}]}}
{"v":1,"id":15,"result":{"type":"passkey_get","credentialId":"…","authenticatorData":"…","clientDataJson":"…","signature":"…","userHandle":"…"}}
{"v":1,"id":16,"result":{"type":"check_passkey_create","excluded":false,"candidates":[{"itemId":"…","title":"GitHub","username":"octo"}],"upgrade":{"kind":"auto","itemId":"…"}}}
{"v":1,"id":17,"result":{"type":"passkey_create","credentialId":"…","attestationObject":"…","clientDataJson":"…","authenticatorData":"…","publicKey":"…","publicKeyAlgorithm":-7}}
{"v":1,"id":18,"result":{"type":"passkey_status","hasPasskey":true}}
{"v":1,"id":10,"error":{"code":"denied","message":"This item is not saved for this website."}}
```

`upgrade.kind` is `"none"`, `"ask"` or `"auto"`. `itemId` is present only
when `kind` is `"ask"` or `"auto"`; `kind` is `"none"` whenever `conditional`
was false (and whenever `excluded` is true).

`id` is `null` only when a request was so broken that its ID could not be
read.

**Events** have no ID: `{"v":1,"event":{"type":"locked"}}`. The types are
`locked`, `unlocked`, and `disconnected`. The host sends `disconnected` when
the desktop app goes away.

**Error codes:** `locked`, `busy`, `no_vault`, `not_found`, `denied`,
`invalid_input`, `decryption`, `corrupted`, `malformed`, `too_large`,
`unsupported_version`, `rate_limited`, `integration_disabled`,
`desktop_unavailable`, `offline`, `internal`. Messages are fixed strings chosen by the
Rust side. The host replaces any message text coming from the socket with
its own fixed text, so no input or secret can be echoed back through an
error.

### Limits

| Limit | Value | Where |
|---|---|---|
| Request frame | 16 KiB | host (stdin) and bridge (socket) |
| Response frame | 256 KiB (Chrome's own cap is 1 MiB) | bridge, host |
| URL, top URL | 4096 bytes each | extension, host, bridge |
| Password in `check_login`/`save_login` | 16 KiB (the core allows 4096 characters) | host, bridge, core |
| Username | 2 KiB (the core allows 512 characters) | host, bridge, core |
| Suggestions per `find_matches` | 50 | bridge, host, extension |
| Passkeys per `find_passkeys`, candidates per `check_passkey_create` | 50 | bridge, host, extension |
| Binary fields (`credentialId`, `challenge`, `userHandle`, list entries, and every binary result) | unpadded base64url | extension, host, bridge |
| `credentialId`, and each `allowCredentials`/`excludeCredentials` entry | exactly 16 bytes | page script (drops others), extension, host, bridge |
| `allowCredentials`, `excludeCredentials` | at most 64 entries | extension, host, bridge |
| `challenge` | 1–1024 bytes | page script (hands others to the browser), extension, host, bridge, core |
| `userHandle` | 1–64 bytes | page script, extension, host, bridge, core |
| `rpId` | 1–253 bytes | page script, extension, host, bridge; the core also requires a valid domain or IP |
| `userName`, `displayName` | 2 KiB on the wire; 512 characters, no control characters, in the core (the page script cuts them to 512) | page script, host, bridge, core |
| Passkeys per login | 8 | core |
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
5. **Origin binding (core):** `fill_item`, `get_totp` and `save_login` with
   an `itemId` succeed only if the item is a login whose own website rules
   match `url`, and `topUrl` too when given, using the rules in
   `autofill.md`. The extension's opinion about which item belongs to a page
   is never used. An unknown item ID, a secure note, a login for another
   site, and a login with no TOTP all return the same `denied`, so IDs cannot
   be probed. Passkey requests are bound by the relying-party ID instead
   ([Passkeys](#passkeys)).
6. **Secret minimization:** `find_matches` returns ID, title, username, a
   has-TOTP flag and the match strength. `fill_item` returns the username and
   password only. `get_totp` returns the current code only; the secret never
   leaves the core. `check_login` compares passwords inside the core and
   returns only add / update / unchanged.
7. **Writes:** `save_login` updates change only the password, and the
   replaced one goes to the item's password history (5 entries). The bridge
   allows one such change per item every 10 minutes, so a flood cannot push
   the real password out of the history quickly. After a successful save, the
   desktop UI is told to refresh its list (`vault://items-changed`, no data).
   `passkey_create` is the same kind of write: sealed, sent to the server,
   and answered only after the server accepted it.

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

**Permissions:** `nativeMessaging`, `activeTab` and `scripting`, plus the
optional host permissions `https://*/*` and `http://*/*`. The optional ones
are requested only when the user turns on in-page suggestions on the options
page. See `security-model.md` §12 and `autofill.md`.

* The background worker is the only code that talks to the native host.
* It accepts runtime messages from three kinds of sender, told apart by the
  browser's sender data. Each has its own strict parser:
  * the toolbar popup (`popup.html`, no tab);
  * the in-page menu, save and passkey frames (`menu.html`, `save.html`,
    `passkey.html`, in a tab). These carry only a 128-bit session token,
    which is valid only for that tab;
  * content scripts, in http(s) frames of a tab, including the passkey
    bridge (`webauthn-bridge.js`). The frame URL, the top URL
    for iframes, and on Chromium the frame's origin and document ID come from
    the browser. A sandboxed frame (origin `null`) is ignored.
* `externally_connectable` is empty (Chromium), so web pages and other
  extensions cannot message it.
* No sender ever supplies a URL. The worker takes URLs from the browser and
  removes credentials, query and fragment before sending them. Non-http(s)
  pages are never sent.
* Fills are sent to one frame (`frameId`, plus `documentId` on Chromium).
  The content script also refuses unless its own origin still equals the one
  the desktop matched.
* Every message from the host goes through `parseIncoming` (exact shapes
  only), and each result must match the type of the request it answers.
* Nothing is stored: no `chrome.storage`, no caches. Menu sessions,
  pending save prompts (which hold the submitted password), and remembered
  usernames from a first login step live in worker memory with short
  lifetimes. They are dropped on `locked` and `disconnected` events.
* Passkey sessions also live in worker memory. The bridge pings its session
  (`wa_ping`, answered `{ok: true}` only for a live session of the same
  frame) every 20 s, which keeps the worker awake and detects a session lost
  to a worker restart. A card whose session is gone can still be closed: the
  worker then sends the result to every frame of the card's tab, and only
  the bridge holding that token acts on it. One `wa_get`/`wa_create` lookup
  runs per tab at a time; another meanwhile gets a fallback without reaching
  the host, so a page cannot drain the shared lookup rate limit.
* All extension UI builds its DOM with `createElement`/`textContent`. A test
  (`src/hygiene.test.ts`) fails the build on `innerHTML`, `console`, `eval`,
  browser storage, or `value`/`data-` attributes.
* CSP for extension pages is
  `default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'`.
  `frame-ancestors` was dropped in Phase 5 because the menu and save pages
  must be framed by web pages. Which extension pages a web page may frame is
  controlled by `web_accessible_resources`, which lists only those pages
  (`menu.html`, `save.html`, `passkey.html`) and their assets.

### Passkeys

The five passkey requests come from the background worker's WebAuthn
handler (`background/webauthn-handler.ts`), which the passkey bridge in the
page feeds, plus `passkey_status` from the inline field-menu handler
(`background/inline-handler.ts`). `url` and `topUrl` come from the browser's
sender data, exactly as for logins; `rpId` comes from the site (or defaults
to the frame's host) and is untrusted.

| Request | Core function | Returns | Checks |
|---|---|---|---|
| `find_passkeys` | `find_passkeys` | per passkey: item ID, credential ID, login title, account name. **No secrets** | `authorize_rp(rpId, url, topUrl)`; stored rpId equals it; filtered by `allowCredentials` when non-empty |
| `passkey_get` | `passkey_assert` | credential ID, authenticator data, `clientDataJSON`, signature, user handle | challenge 1–1024 bytes; `authorize_rp`; the item is a login holding that credential ID with that stored rpId. Anything else is `denied` (an unknown item's `not_found` is mapped to `denied` too) |
| `check_passkey_create` | `check_passkey_create` | `excluded`, logins that could hold the new passkey (ID, title, username; no candidates when `excluded`), and `upgrade` | `authorize_rp`; candidates are logins whose own website rules match the page and that hold fewer than 8 passkeys, same username first. `upgrade` is computed only when `conditional` is true and nothing is excluded: `auto`/`ask` needs a password fill of that same login on that same site within the last 5 minutes (`vault.rs::RecentFill`, `UPGRADE_WINDOW_MS`), a matching (folded) account name or none, and room for another passkey; `auto` when the vault setting `auto_passkey_upgrade` is on, `ask` when it is off; otherwise `none` |
| `passkey_create` | `stage_passkey_create` | credential ID, attestation object, `clientDataJSON`, authenticator data, SPKI public key, algorithm `-7` | `authorize_rp`; bounds; `itemId`, if given, must be a login offered for the page. When `conditional` is true, the same upgrade decision is recomputed and the request is `denied` unless it is still `auto` for exactly `itemId` — the caller's claim that this is the automatic upgrade is never taken on trust. A conditional request is also `denied` when any login already holds a passkey for the same rpId and user handle (an upgrade only adds), and a successful one spends the fill it relied on (one fill, one silent passkey). If a login anywhere in the vault already holds a passkey for the same rpId and user handle, the new one replaces it **in that login**, whatever `itemId` says. Sent to the server; `offline` if it cannot be, and nothing is stored |
| `passkey_status` | `has_passkey_for_page` | `hasPasskey`: whether any passkey in the vault has a stored rpId `authorize_rp` allows for this page. **No other data** | `authorize_rp` for every stored passkey it checks; no item is named or returned |

`find_passkeys`, `check_passkey_create` and `passkey_status` draw from the
lookup bucket; `passkey_get` and `passkey_create` from the secret bucket.
There is no per-item cooldown for passkey writes. No response ever contains
a private key, and `Debug` of the requests and results shows only their
type.

The extension side of the passkey flow — which page events become which
request, and what the user clicks — is in `autofill.md` §Passkeys.

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
* **The extension ID is not a secret.** The Chromium
  manifest no longer carries a `key` field — the Chrome Web Store rejects
  one on upload (§2) — so an unpacked install gets an ID derived from its
  folder path, and the published ID is the one the store assigned,
  `fmmfkakdkkcfpdnfmbngnlelbfaogafo`. `CHROME_EXTENSION_ORIGIN` in
  `havenkeys-native-host` and `CHROME_ORIGIN` in the install scripts allow
  only that ID, so an unpacked load won't connect unless you edit them. Note
  also that an ID derived from a public key pins nothing on its own: the key
  that produced the old ID is in this repository's history, and anyone can
  put it in their own manifest to reproduce that ID. Only a store-published
  build has its ID enforced by the store. What actually keeps other local
  callers out is the same-user socket check in the bullet above — and, as
  that bullet says, it does not keep out a process running as you.
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
* **Registration is rewritten at every start, and outlives the app.** An
  installed app points the manifests at its own bundled host each time it
  starts, replacing one made by the install scripts. Uninstalling does not
  remove the manifests or registry keys; they then name a missing file, and
  the extension shows "Not connected". `scripts/install-native-host.sh
  --uninstall` (or `.ps1 -Uninstall`) removes them.
* **Sandboxed browsers are not registered.** Snap and Flatpak browsers on
  Linux read manifests inside their sandbox and may not be allowed to start
  a host outside it; the app writes only the standard locations.
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
  prompt for each fill or save.
* **A compromised extension can write.** Within the rate limits it can add
  logins for sites it names, and replace the password of a login for a site
  it names. Old passwords stay in the item's history, and there is at most
  one change per item every 10 minutes, but a patient attacker can still
  cycle a password out of the 5-entry history over about an hour.

## 9. Tests

| Test | Covers |
|---|---|
| `crates/havenkeys-protocol/tests/messages.rs` | A5 parsing cases, version handling, error text never echoes input, deterministic fuzz (50 000 cases) |
| `crates/havenkeys-protocol/src/frame.rs` | A6 framing: rejected before allocation, resync after discard, truncated streams |
| `crates/havenkeys-bridge/tests/bridge.rs` | A1 wrong origin, A2 arbitrary item ID, A3 locked, integration switch, rate limit, secret minimization, and over a real socket: A5, A6, lock events, connection limit, second-instance and stale-socket handling, non-private directory refusal |
| `crates/havenkeys-native-host/tests/host.rs` | Runs the real host binary: relaying, Firefox/Chrome caller check, desktop not running, A5/A6 at the host, truncated input, and a hostile fake desktop (invalid messages dropped, spoofed error text replaced, `disconnected` event) |
| `packages/protocol/src/index.test.ts` | TS validator accepts exact shapes only |
| `apps/extension/src/**/*.test.ts` | Native client (ID correlation, timeouts, host loss, idle close, type mismatch), popup request validation, URL stripping, the popup never supplying URLs, content/menu/save message validation, menu sessions (single use, same tab only, offered items only, expiry, lock), save prompts (unchanged logins, multi-step, expiry, other tabs), content-script origin check and sender check, untrusted events, source hygiene |
| `crates/havenkeys-core/tests/security.rs` (Phase 5) | Frames on foreign top pages (A1), `check_login` classification, `save_login` origin binding (A2 for writes), password history bound |
| `crates/havenkeys-core/tests/passkeys.rs`, `src/passkey/*.rs` | Passkey create → sign in, A1p/A2p/A3p, attaching to a login and keeping passkeys through edits, re-registration replacing across logins, the per-login limit, removal, input bounds; `authorize_rp` rules; byte layouts; the automatic upgrade (A1u/A2u): `auto`/`ask` after a recent fill, the 5-minute window and clock-skew handling, folded account-name matching, per-site scoping (`upgrade_is_per_site_and_never_for_look_alikes`, `a_fill_on_one_site_is_no_upgrade_on_another_site_of_the_same_login`), `conditional_create_needs_auto_for_exactly_that_login`, add-only and one silent passkey per fill (`conditional_create_never_replaces_the_filled_logins_own_passkey`, `one_fill_grants_one_silent_passkey`), the 16-entry fill memory cap, fill memory dropped on lock |
| `crates/havenkeys-protocol/tests/messages.rs`, `crates/havenkeys-bridge/tests/bridge.rs` | Passkey requests parse with exact shapes and bounds, results validate; create → get over the bridge, attacks, offline create stores nothing; `passkey_upgrade_through_the_bridge` (A1u end to end), `passkey_status_through_the_bridge` |
| `apps/extension/src/webauthn/*.test.ts`, `background/webauthn-handler.test.ts`, `background/registration.test.ts` | Page script fallback paths and rebuilt credentials, bridge parsing, abort and timeout handling, sessions (offered passkeys only, same tab and frame, one operation at a time, lock and unlock), script registration groups, the automatic upgrade's auto/ask/fallback routing (including a lock during the "Add a passkey?" card falling back) and the "saved" notice |
| `apps/extension/src/background/passkey-sites.test.ts`, `menu/passkey.test.ts`, `menu/menu.test.ts` | Passkeys Directory file shape and host matching (including evil-suffix cases), the field-menu hint rows and their order, the help link opening only on a trusted click |
