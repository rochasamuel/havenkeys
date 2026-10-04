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
| Linux | `<profile>/NativeMessagingHosts/com.havenkeys.bridge.json` for each Chromium browser whose profile directory exists under `$XDG_CONFIG_HOME` (Chrome, Chrome Beta, Chromium, Brave, Edge, Vivaldi), and `~/.mozilla/native-messaging-hosts/com.havenkeys.bridge.json` if Firefox has a profile in `~/.mozilla`, `$XDG_CONFIG_HOME/mozilla` (Firefox 147+), `~/snap/firefox` or `~/.var/app/org.mozilla.firefox` (the snap and Flatpak builds reach the host through the WebExtensions portal, which reads the same file) |
| macOS | The same files under `~/Library/Application Support/<browser>/` (Firefox: `Mozilla/NativeMessagingHosts/`, written when `Mozilla/` or `Firefox/` exists) |
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
6. Save prompts, passkeys and suggestions when clicking login fields are on
   from install. Reload tabs that were already open. To hide the menu under
   login fields, turn off *Suggestions in login fields* in the extension's
   options (the *Settings* button in the popup); save prompts and passkeys
   keep working.
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
| `generator_options` | none (returns the policy saved in the desktop's generator tab) | yes | lookup |
| `generate_password` | `options`? (`length` 8–128, `uppercase`, `lowercase`, `digits`, `symbols`, `avoidAmbiguous`; at least one class on; else the desktop generator tab's saved policy) | yes | lookup |
| `check_login` | `url`, `topUrl`?, `username` (string or null), `password`, `currentPassword`? (a change-password form's current password) | yes | secret |
| `save_login` | `url`, `topUrl`?, `username`, `password`, `itemId` (UUID or null), `title`? (a new login's name; refused with `itemId`) | yes | secret, plus one update per item per 10 min |
| `find_passkeys` | `url`, `topUrl`?, `rpId`, `allowCredentials` (list) | yes | lookup |
| `passkey_get` | `itemId` (UUID), `credentialId`, `url`, `topUrl`?, `rpId`, `challenge` | yes | secret |
| `check_passkey_create` | `url`, `topUrl`?, `rpId`, `userName`, `excludeCredentials` (list), `conditional` (bool) | yes | lookup |
| `passkey_create` | `url`, `topUrl`?, `rpId`, `challenge`, `userHandle`, `userName`, `displayName` (string or null), `itemId` (UUID or null), `conditional` (bool) | yes | secret; a server write |
| `passkey_status` | `url`, `topUrl`? | yes | lookup |
| `open_item` | `itemId` (UUID), `url`, `topUrl`? | yes | secret |
| `find_identity` | `url`, `topUrl`? | yes | lookup |
| `fill_identity` | `url`, `topUrl`?, `roles` (1-40 known roles, no duplicates), `documents` (bool) | yes | secret |
| `open_identity` | `url`, `topUrl`? | yes | secret |
| `find_cards` | `url`, `topUrl`? | yes | lookup |
| `fill_card` | `itemId` (UUID), `topUrl`, `frames` (1-8 of `{ url, roles }`; each frame 1-8 known roles, no duplicates) | yes | secret |
| `save_card` | `url`, `topUrl`?, `number`, `verificationNumber`?, `expiry`? (`MM/YY`), `cardholderName`?, `title`? | yes | secret; a server write |
| `start_sso` | `itemId` (UUID), `url`, `topUrl`? | yes | secret |
| `check_sso` | `url`, `topUrl`?, `provider` (`"google"` \| `"microsoft"` \| `"github"` \| `"apple"`), `account` (string or null) | yes | secret |
| `save_sso` | `url`, `topUrl`?, `provider`, `account` (string or null), `itemId` (UUID or null), `title`? (a new login's name; refused with `itemId`) | yes | secret, plus one update per item per 10 min (shares `save_login`'s per-item limiter) |
| `show_unlock` | none | no | secret |

`topUrl` is present only when `url` is an iframe. It is the tab's top-level
page, and items must match both (`autofill.md`, Frames). Optional fields may
be omitted, which means null.

`open_item` shows the desktop window with that login open in the editor; the
item must be saved for `url`, like `fill_item`. Nothing is returned.

`show_unlock` brings the desktop window forward; while the vault is locked
that window is the unlock screen, with the password field focused. It is
the popup's "Unlock" button. It carries no secret and returns none
(`{"type":"show_unlock"}`), so it is answered in any lock state and never
touches the vault; the browser-integration switch is sealed in the vault
and cannot be read while locked. It is in the `secret` rate class because
it raises a window, so a script cannot keep pulling the desktop to the
front. The master password is never sent over this protocol: there is
still no request that unlocks the vault. Only the toolbar popup sends it;
the background refuses `popup_*` messages from any other sender.

**Identity requests.** The Identity has no saved website, so these three are
not bound to a site the way `fill_item` is (`security-model.md` §20 explains
why and what protects it). Results are named after their requests:

* `find_identity` returns `{ title, email, roles }`: the identity's name, its
  email (both at most 1024 and 2048 bytes) and the roles it has a value for,
  as names only (documents included, so the menu can say "also asks for
  CPF"). It never carries a value. An empty `roles` list is an empty identity.
* `fill_identity` returns `{ values: [ { role, value } ] }`, in the order
  asked, for the roles that have a value. Each value is at most 4096 bytes.
* `open_identity` returns `{}` and shows the identity in the desktop window,
  as `open_item` does for a login. It works when the identity is empty.

Roles: `fullName`, `firstName`, `middleName`, `lastName`, `email`, `phone`,
`birthDate`, `birthDay`, `birthMonth`, `birthYear`, `company`, `street`,
`number`, `complement`, `addressLine1`, `addressLine2`, `neighborhood`,
`city`, `state`, `postalCode`, `country`, `username`, `cpf`, `rg`,
`passport`, `driversLicense`. Composed values (full name, address line 1,
birth-date parts, phone) are built in Rust. An unknown role, a duplicate or
0 or more than 40 roles is a malformed message.

What Rust checks (`VaultService::identity_*_for_page`): the vault is unlocked
and browser integration is on; the page URL is http(s); in a frame, `topUrl`
is http(s) too and both are the same site, else `denied`; the identity
exists (`not_found` otherwise). Document roles (`cpf`, `rg`, `passport`,
`driversLicense`) are answered only when `documents` is true **and** the
page is https, and are otherwise silently left out. Only the requested roles
are returned. `find_identity` stops after the existence check and returns the
title, email and role names; `open_identity` stops after the URL check.

**Card requests.** Like the Identity, a Card has no saved website
(`security-model.md` §21 explains why and what protects it), so these three
are not bound to a site the way `fill_item` is. `topUrl` on `find_cards` is
present only when `url` is an iframe; on `fill_card` it is required.

* `find_cards` returns `{ insecure, cards }`: at most 50 cards, each
  `{ id, title, brand, last4, expiry }` (overview data only; `last4` and
  `expiry` may be null). When the top page is not https, `insecure` is true
  and `cards` is empty. The extension does at most 12 of these lookups per
  pick, to vet frames before a fill.
* `fill_card` returns `{ frames: [ { values: [ { role, value } ] } ] }`, one
  entry per requested frame, in the same order, holding values only for the
  roles that frame asked for and the card has. Each value is at most 1024
  bytes. Roles: `cardholderName`, `cardholderGivenName`,
  `cardholderFamilyName`, `number`, `verificationNumber`, `expiryMonth`
  (`"1"` to `"12"`, unpadded), `expiryYear` (four digits), `brand` (the brand
  id). An unknown role, a duplicate, 0 or more than 8 roles per frame, or 0
  or more than 8 frames is a malformed message. The frame URLs must fit the
  request size limit like any other field.
* `save_card` stages a create through the normal write path and returns the
  new item ID. The number must pass the check digit and be at most 64 bytes;
  the verification number at most 16; `title` follows the `save_login`
  limit. Offline it fails with `offline`.

What Rust checks (`VaultService::card_for_page`, `card_page.rs`): the vault
is unlocked and browser integration is on (the bridge's `require_enabled`);
`topUrl` and every frame URL are https, else `denied`; each frame is
same-site with the top page or has an origin on the processor list (§5),
else the **whole** `fill_card` is `denied` (one bad frame denies the
request); `itemId` names a Card, else `not_found` (the same answer for a
missing item and another item type). Only the requested roles are returned.
`find_cards` stops after the URL checks. `save_card` requires its frame to be
same-site with the top page (a processor frame is not enough).

`conditional` on `check_passkey_create` and `passkey_create` marks the
site's automatic passkey upgrade (`create()` with `mediation:
"conditional"`, right after a HavenKeys password fill). It does not change
either request's shape, only what the core checks: `check_passkey_create`'s
result then carries a real `upgrade` decision instead of always `none`, and
`passkey_create` refuses (`denied`) unless that decision is still `auto` for
exactly the item named (`autofill.md`, Automatic upgrade).

There is deliberately no request to unlock the vault, list items, search,
reveal notes, read a TOTP secret, read a passkey's private key, delete items
or passkeys, or change anything except one login's password, passkeys or
"Sign in with" account. There are three writes. `save_login` can add a login
for the page it names, or replace the password of a login that matches that
page; the old password is kept in the item's history. `passkey_create` adds a
passkey (see [Passkeys](#passkeys) below). `save_sso` can add a login that
signs in with a provider, or set the account on a login that already signs in
with that provider (see [Sign in with](#sign-in-with) below); it never touches
a password.

`start_sso` asks whether `itemId` may start a "Sign in with" run on `url` —
the same origin check as `fill_item`, but no secret comes back. `check_sso`
asks whether saving a provider and account for `url` would add a login,
update one, or do nothing, among the logins already saved for that page.
`Match.provider` (`find_matches`'s result) is `null` for an ordinary login and
the provider name for a "Sign in with" login; for such an item, `username`
carries its account when the item has no username of its own.

**Responses** answer one request ID and carry exactly one of `result` or
`error`:

```json
{"v":1,"id":7,"result":{"type":"find_matches","matches":[{"id":"…","title":"GitHub","username":"octo","hasTotp":true,"strength":"same_host","provider":null}]}}
{"v":1,"id":8,"result":{"type":"fill_item","username":"octo","password":"…","autoSubmit":true}}
{"v":1,"id":9,"result":{"type":"get_totp","code":"123456","period":30,"secondsRemaining":12,"autoSubmit":true}}
{"v":1,"id":11,"result":{"type":"generate_password","password":"…"}}
{"v":1,"id":12,"result":{"type":"check_login","action":"update","itemId":"…"}}
{"v":1,"id":13,"result":{"type":"save_login","itemId":"…"}}
{"v":1,"id":14,"result":{"type":"find_passkeys","passkeys":[{"itemId":"…","credentialId":"…","title":"GitHub","userName":"octo"}]}}
{"v":1,"id":15,"result":{"type":"passkey_get","credentialId":"…","authenticatorData":"…","clientDataJson":"…","signature":"…","userHandle":"…"}}
{"v":1,"id":16,"result":{"type":"check_passkey_create","excluded":false,"candidates":[{"itemId":"…","title":"GitHub","username":"octo"}],"upgrade":{"kind":"auto","itemId":"…"}}}
{"v":1,"id":17,"result":{"type":"passkey_create","credentialId":"…","attestationObject":"…","clientDataJson":"…","authenticatorData":"…","publicKey":"…","publicKeyAlgorithm":-7}}
{"v":1,"id":18,"result":{"type":"passkey_status","hasPasskey":true}}
{"v":1,"id":10,"result":{"type":"open_item"}}
{"v":1,"id":25,"result":{"type":"show_unlock"}}
{"v":1,"id":22,"result":{"type":"find_cards","insecure":false,"cards":[{"id":"…","title":"Visa","brand":"visa","last4":"4242","expiry":"11/33"}]}}
{"v":1,"id":23,"result":{"type":"fill_card","frames":[{"values":[{"role":"number","value":"…"},{"role":"expiryMonth","value":"11"}]}]}}
{"v":1,"id":24,"result":{"type":"save_card","itemId":"…"}}
{"v":1,"id":19,"result":{"type":"start_sso","provider":"google","account":"user@gmail.com","providerOrigins":["https://accounts.google.com"],"autoChoose":true}}
{"v":1,"id":20,"result":{"type":"check_sso","action":"add","itemId":null,"accounts":["user@gmail.com"]}}
{"v":1,"id":21,"result":{"type":"save_sso","itemId":"…"}}
{"v":1,"id":10,"error":{"code":"denied","message":"This item is not saved for this website."}}
```

`upgrade.kind` is `"none"`, `"ask"` or `"auto"`. `itemId` is present only
when `kind` is `"ask"` or `"auto"`; `kind` is `"none"` whenever `conditional`
was false (and whenever `excluded` is true).

`fill_item` and `get_totp` results also carry `autoSubmit`: a boolean,
`settings.auto_sign_in && item.auto_sign_in`, computed by
`VaultService::auto_sign_in_for` after the origin check that already gates
the rest of the result. It tells the extension whether it may press the
site's sign-in button after filling this value (`autofill.md`, Automatic
sign-in). The parser on both ends requires it to be present and boolean;
anything else is rejected like any other malformed result.

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
| Password (and `currentPassword`) in `check_login`/`save_login` | 16 KiB (the core allows 4096 characters) | host, bridge, core |
| Username | 2 KiB (the core allows 512 characters) | host, bridge, core |
| `title` in `save_login` | 1 KiB, not empty (the core allows 256 characters, no control characters) | extension, host, bridge, core |
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
   Card requests are the exception to origin binding: a Card is offered on
   any https page, and the check is instead https, then same-site with the
   top page or a processor frame from the fixed list in
   `PAYMENT_FRAME_ORIGINS` (`crates/havenkeys-core/src/card_page.rs`). The
   list, each entry confirmed from the processor's own documentation or SDK
   source: Stripe (`https://js.stripe.com` and its subdomains, matched on a
   whole label, so `js.stripe.com.evil.com` and `evil-js.stripe.com` are
   denied); Adyen live checkout-shopper (`checkoutshopper-live`, `-live-us`,
   `-live-au`, `-live-apse`, `-live-in`, `-live-nea` on `adyen.com`);
   Braintree Hosted Fields (`https://assets.braintreegateway.com`); Mercado
   Pago Secure Fields (`https://secure-fields.mercadopago.com`,
   `https://api-static.mercadopago.com`). A processor origin must have the
   default port. No test or sandbox origins. Adding an origin is a code
   change with a test.
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
  events. See §8 for the squatting caveat. Both ends also check the peer's
  Windows user (token user SID); see the pipe-squatting bullet.
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

**Permissions:** `nativeMessaging`, `activeTab`, `scripting`, `storage` (one
boolean: whether in-page suggestions are shown) and the host permissions
`https://*/*` and `http://*/*`, all requested at install. See
`security-model.md` §12 and `autofill.md`.

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
    the browser. On Chromium, the background refuses in-page requests from a
    sandboxed frame (the browser reports sender origin `null`). Firefox
    reports no `sender.origin`, so there the content script's own
    `self.origin` check is what applies. A popup fill and the passkey bridge
    also refuse a document whose `self.origin` is opaque, since `tab.url`
    still names the real site.
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

### Sign in with

The three requests come from the background's SSO handler
(`background/sso-handler.ts`), driven by a trusted pick in the balloon, field
menu or popup (`start_sso`) and by the content script's own save detection
(`check_sso`, `save_sso`). `url`/`topUrl` are the browser's sender data, as
for logins; `provider` and `account` come from the page (a provider button's
identity, and text read off the provider's own chooser or a username step)
and are treated as untrusted input.

| Request | Core function | Returns | Checks |
|---|---|---|---|
| `start_sso` | `start_sso_for_page` | the item's provider and saved account, the provider's exact origins (from Rust's fixed table), and whether the run may auto-choose. **No secret** | Same origin check as `fill_item`; the item must be a login with `sign_in_with`. `autoChoose = settings.auto_sign_in && item.auto_sign_in` |
| `check_sso` | `check_sso`, `provider_accounts` | `add` (no login has this provider+account or this provider with no account), `update` (exactly one login has this provider and no account) or `unchanged`, the `itemId` an `update` would change, and, unless `unchanged`, `accounts`: the usernames (trimmed, lowercased, deduplicated, sorted, at most 10) of logins whose rules match the provider's own sign-in origins, offered as choices in the save prompt. Omitted when empty; an older desktop never sends it | Among logins whose own website rules match the page |
| `save_sso` | `stage_save_sso` | the item ID | Without `itemId`: the same path as `save_login`, one whole-site rule for the frame's origin, `auto_sign_in` on, no password required. With `itemId`: the login must match the page and already sign in with `provider`; only its `account` changes, and `title` must be absent |

`account` is validated the same way on every one of these requests: trimmed,
empty becomes `null`, at most 254 characters, no control characters
(`crates/havenkeys-core/src/sso.rs`). All three requests draw from the secret
bucket; `save_sso` with an `itemId` shares `save_login`'s per-item cooldown
(one update per item per 10 minutes). None of `start_sso`, `check_sso` or
`save_sso` itself carries a secret — the chooser click they drive only clicks
an account row it finds by exact, case-insensitive email match, on an origin
from Rust's list, and never presses a consent or permissions screen.

After that pick, if the saved account is not already signed in at the
provider, the background may go on to request `fill_item` (and, for a login
with TOTP, `get_totp`) for the **single** vault login Rust's `find_matches`
matches to the provider page whose username equals the run's account —
exactly the same requests, and the same origin check, as any other fill, now
made against the provider's own origin instead of the site the user picked
on. Two matching logins, or none, and neither is requested. Whatever comes
back is filled, and pressed on, only under the automatic sign-in rules and
switches (`autofill.md` §"Sign in with", "Login";
`docs/superpowers/specs/2026-09-29-sign-in-with-provider-login-design.md`).

The extension side — how a provider button is recognized, how the balloon and
the run work, and the save-detection flow — is in `autofill.md` §"Sign in
with Google, Microsoft, GitHub, Apple".

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
  derived from your profile path and is predictable. The DACL protects only
  a pipe HavenKeys created, so both ends check the other: the native host
  reads the pipe server's process ID and refuses the pipe unless that process
  runs as the same Windows user (token user SID); the desktop app checks each
  client the same way. Any failure to find or open the process counts as
  "another user". The comparison uses token user SIDs, not owners, so
  elevation alone does not change the answer. An elevated desktop app still
  owns its pipe as Administrators, though, and the owner-only DACL refuses an
  unelevated host, which fails closed. None of this is verified on Windows
  yet. A residual race: a process ID read from the pipe could in theory be
  reused before the check runs; this is hard to exploit.
  What remains: if another user squats the name first, HavenKeys cannot
  serve until that pipe goes away. That is an availability problem, not a
  confidentiality one; the extension reports HavenKeys as unreachable and
  sends nothing.
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
| `crates/havenkeys-core/tests/security.rs` | `start_sso`/`check_sso`/`stage_save_sso` origin binding and locked/denied cases, account validation, provider-origin table |
| `crates/havenkeys-protocol/tests/messages.rs`, `crates/havenkeys-bridge/tests/bridge.rs` | `start_sso`/`check_sso`/`save_sso` parse with exact shapes and bounds; `Match.provider`; attacks over the bridge; the per-item update limiter shared with `save_login` |
| `apps/extension/src/autofill/sso.test.ts` | Provider-button and chooser-row recognition (English and pt-BR, bare labels, OAuth links, hidden/disabled buttons, misleading text, thousands of buttons), consent-phrase detection |
| `apps/extension/src/background/sso-state.test.ts`, `sso-handler.test.ts`, `content/sso.test.ts` | `PendingSso`/`SsoRun` TTLs, ending a run on user input or an off-list origin, popup acceptance only via `openerTabId`, chooser matching (0/1/2 rows), `autoChoose` off, save detection (untrusted clicks ignored, account captured only on provider origins, balloon on return, `unchanged` → no balloon) |
