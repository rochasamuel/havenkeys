# Autofill

How HavenKeys decides which login belongs to a page, finds login fields, and
fills them.

* Domain matching and origin binding: Rust core
  (`crates/havenkeys-core/src/origin.rs`, `VaultService::{find_matches,
  fill_for_page, totp_for_page, check_login, save_login}`).
* Field detection and filling: the extension (`apps/extension/src/autofill/`,
  `content/`, `background/inline-handler.ts`).

> This software has not undergone an independent security audit.

## Domain matching rules

Every login has a list of website rules. Each rule is a normalized `http(s)`
URL plus a match type. Matching a page against a rule works like this:

1. **The page must be an `http` or `https` URL with a host.** `about:blank`,
   `data:`, `file:`, `javascript:`, `blob:`, and extension pages never match.
2. **No scheme downgrade.** An `https` rule never matches an `http` page. An
   `http` rule matches both `http` and `https` pages.
3. **Same port** (after applying scheme defaults).
4. **Then, depending on the match type:**

| Match type | UI label | Matches when | Strength |
|---|---|---|---|
| `exact` | This exact page | Same host and same path; query and fragment ignored | `exact_url` |
| `origin` | This exact site | Same host | `same_host` |
| `domain` (default) | Whole site, any subdomain | Same host, **or** same registrable domain (eTLD+1 from the Public Suffix List) | `same_host` / `same_site` |

A `domain` rule falls back to same-host matching when either side has no
registrable domain under a known public suffix. That covers IP addresses,
`localhost`, unknown TLDs such as `.internal`, and hosts that are themselves
public suffixes, such as `github.io`.

Suggestions are ranked `exact_url` → `same_host` → `same_site`, then by title.

### Examples (all are unit tests)

Rule `https://github.com`, match type `domain`:

| Page | Result |
|---|---|
| `https://github.com/login` | ✅ same host |
| `https://gist.github.com/` | ✅ same site |
| `http://github.com/` | ❌ scheme downgrade |
| `https://github.com.evil.com/` | ❌ different registrable domain |
| `https://github-login.example.com/` | ❌ |
| `https://evilgithub.com/` | ❌ |
| `https://github.com@evil.com/` | ❌ host is `evil.com` |
| `https://evil.com/?next=https://github.com` | ❌ |
| `https://gіthub.com/` (Cyrillic і) | ❌ different punycode host |
| `https://github.com:8443/` | ❌ different port |

Public-suffix boundaries:

| Rule | Page | Result |
|---|---|---|
| `https://alice.github.io` | `https://bob.github.io/` | ❌ each `github.io` site is separate |
| `https://bank.co.uk` | `https://evil.co.uk/` | ❌ |
| `https://bank.co.uk` | `https://login.bank.co.uk/` | ✅ same site |
| `http://192.168.1.1` | `http://192.168.1.2/` | ❌ IPs are host-only |

Hosts are compared after WHATWG URL parsing, so case, IDN punycode,
percent-encoding and a single trailing dot are all normalized first. No
matching is ever done with string prefixes, suffixes or regular expressions on
raw URLs.

## Origin binding (enforced in Rust)

The extension is never trusted to decide which item belongs to a page. For
every request the core re-derives the answer from the item's own rules:

| Core function | Returns | Check |
|---|---|---|
| `find_matches(url, top_url)` | ID, title, username, has-TOTP flag, strength. **No secrets.** | Only logins whose rules match the page |
| `fill_for_page(id, url, top_url)` | Username and password only | `Denied` unless the item is a login whose rules match the page |
| `totp_for_page(id, url, top_url, now)` | Current code only; the secret never leaves | Same as above |
| `check_login(url, top_url, username, password)` | `Add`, `Update(id)` or `Unchanged`. Never a password | Only logins matching the page are compared |
| `save_login(url, top_url, username, password, update?)` | The item ID | An update must target a login matching the page; a new login is saved for the page's origin |

All of them return `Locked` when the vault is locked. Secure notes are never
served to pages.

### Frames

`top_url` is the tab's top-level page when the fields are in an iframe. A
login is then served only if its rules match **both** the frame and the top
page. A `github.com` login frame embedded in `evil.com` gets nothing, while a
`login.example.com` frame inside `www.example.com` works for a whole-site
rule. An unparseable `top_url` denies everything. Regression tests:
`a1_frame_on_foreign_top_page_is_denied` (core) and
`a1_frame_needs_matching_top_page` (bridge).

The background worker takes both URLs from the browser's sender data: the
frame's URL, and for iframes the tab's URL. It never uses anything the page
or the content script says. If the top URL is not readable, the frame is
ignored. Credentials, query and fragment are removed before URLs leave the
browser.

## Field detection

Code: `apps/extension/src/autofill/`. `classify.ts` is pure scoring over
features; `group.ts` extracts features from the DOM.

### When it runs

Nothing scans the page on load. Fields are classified **when the user
interacts with them**: a trusted click, Tab focus, or ArrowDown in an input.
At that point the content script builds the field's **login group**:

1. The field's `<form>`, unless that form has more than 40 inputs (a page
   wrapper, as in ASP.NET).
2. Otherwise the nearest ancestor, up to 8 levels, that holds another usable
   input.

At most 60 usable inputs of the group are examined. The cost of a click is
therefore independent of page size. Attack 7 (a page with 5,000 inputs) is a
test: `autofill.test.ts` checks the classification stays bounded and fast.

Dynamically rendered forms (React, Vue, Angular, SPAs, multi-step flows)
need no special handling. The DOM is read at the moment of interaction, so
whatever the framework has rendered is what gets classified. A
`MutationObserver` is used only where something must be watched between
interactions: it watches the suggestion frame, and closes the menu if the
page removes or restyles it, and scroll, resize and SPA route changes
(`popstate`, `hashchange`) reposition or close the menu.

### Usable inputs

An input is considered only if it is not disabled, not read-only, not of a
non-text type (hidden, checkbox, date, and so on), and **visible**: rendered
with a size of at least 4×4 px, and not hidden by `display`, `visibility`,
`opacity` or `content-visibility` (`checkVisibility`). Hidden "honeypot"
fields that harvest filled credentials are never classified or filled.

### Signals

Each field gets a score per role. No single heuristic decides.

| Signal | Username | OTP |
|---|---|---|
| `autocomplete=username` / `email` / `tel` | +100 / +90 / +30 | |
| `autocomplete=one-time-code` | disqualifies | +120 |
| `type=email` | +80 | |
| Strong keyword in `name`/`id` (username, login, email, account, usuario, cpf…) | +50 | |
| Strong keyword in placeholder, aria-label, title, `<label>`, `aria-labelledby` | +40 | |
| Weak keyword (user, mail, phone…) | +25 / +20 | |
| OTP keyword (otp, 2fa, verification code, token…) / weak (code, pin) | | +70 / +30 |
| `maxlength` 4–8, numeric input mode | | +15, +10 |
| 4–8 adjacent single-character boxes (split code) | | +65 |
| Last text field before the first password field | +40 | |
| Negative words (search, coupon, newsletter, address, name…) | −80 | 0 for postal/zip/promo/CVV… |
| Card or address `autocomplete` tokens | disqualifies | disqualifies |

Keywords are compared as whole words after normalization (lowercase, accents
removed, camelCase and punctuation split): `loginEmail` → `login email`.
English, Portuguese and Spanish are covered.

Thresholds: a username needs 50 when the group has a password field. Without
one (a username-only step) it needs 70, so a stray text box does not qualify.
An OTP field needs 60. Each group has at most one username: the best-scoring
text field before the first password field.

**Password roles** are resolved for the group as a whole, in this order:

1. `autocomplete`: `current-password`; `new-password` (a second one, or one
   with confirm wording, is the confirmation).
2. Wording: confirm / repeat / again → confirmation; current / old →
   current; new / create / choose → new.
3. Position and **group intent**. Intent comes from headings, submit buttons,
   the form's name and action, and the page path, which weigh three times
   more than other links and buttons in the group. "Already have an account?
   Log in" on a signup page therefore does not flip the form to login.
   * One password: `new-password` if the intent is signup or change,
     otherwise `password` (login).
   * Two: new + confirmation, or current + new for a change form.
   * Three: current, new, confirmation.

A password-typed field that scores very high as an OTP ("enter the 6-digit
code") is treated as an OTP field.

Each classification carries a confidence (0–1).

### Supported shapes (all tested in `autofill.test.ts`)

Username + password, email + password, labels only, `aria-labelledby`,
password-only step, username-only step, form-less SPA markup, open shadow
roots, fields inserted after load, multiple forms on one page, signup with
confirmation, single-password signup, three-field change-password,
single-box and split-box OTP. It also covers the negative cases: hidden,
disabled and read-only fields, search boxes, newsletter boxes, and postal
codes.

## Filling

Values are written with the native `value` setter, then `input` and `change`
events are dispatched. React's value tracker sees the change, and so do Vue
and Angular (tested). Secrets go only into the `value` property of visible,
enabled fields **in the group the user interacted with**. They never go into
attributes, never into hidden fields, and never into other forms.

| Menu kind | Offered when the user clicks | Fills |
|---|---|---|
| Login | a username or (current) password field | username and password fields of that group |
| One-time code | an OTP field | the code, one digit per box for split fields |
| New password | a new-password or confirmation field | a password generated **by the desktop** (Rust CSPRNG, default policy: 24 chars, all classes) into the new + confirmation fields |

## User flow

```text
User clicks a login field (trusted event)
  → content script classifies the group, sends cs_open_menu(kind)
  → background: find_matches(frame URL, top URL) via the desktop
      no matches / app not running / integration off → nothing appears
      locked → small "HavenKeys is locked" menu
  → content script shows the menu frame under the field
  → user clicks a login in the menu (an extension page)
  → background: fill_item(id, frame URL, top URL); the desktop re-checks origin
  → background sends the values to that frame only (and, on Chromium, that document only)
  → content script checks it is still on the matched origin, fills the group
```

The TOTP flow is the same, with `get_totp`. Nothing is ever filled on page
load, on programmatic focus, or on events a page synthesizes: every trigger
checks `isTrusted`.

### Suggestion UI

The menu and the save prompt are extension pages (`menu.html`, `save.html`)
in iframes. Page script cannot read them (cross-origin), restyle their
content, or forge clicks inside them. They show titles, usernames and site
names only. Passwords and codes go from the background worker straight to
the content script and never pass through these pages.

The page does control the `<iframe>` element itself, so:

* all its styles are set inline with `!important`, which beats page
  stylesheets;
* a `MutationObserver` closes it if the page touches its attributes or
  removes it;
* clicks inside it are ignored until it has been visible for 400 ms, and, on
  Chromium, only while IntersectionObserver v2 reports it unobscured and
  opaque. This is a clickjacking guard (see Limitations for Firefox).

A menu session is a random 128-bit token bound to the tab, frame and URL the
user clicked in. Picks are accepted only from a menu frame in the same tab,
only for items the menu offered, once, and within 5 minutes. Locking the
vault drops every session.

The token is **not** a secret the page cannot see. It is passed in the menu
frame's URL fragment, and the frame is attached to the page's own
`documentElement`, so page script can read it off the iframe's `src` and can
instantiate `menu.html` itself (it is a web-accessible resource on every
http(s) origin, with no `use_dynamic_url`). What the token does is scope a
pick to one tab, one frame and one offer set; what stops it becoming a
credential leak is that the fill is addressed to the frame the user focused
and the core re-checks the item against that frame's origin in Rust. The
residual risk is a misdirected fill or save prompt on the page's *own*
origin, driven by a click the user meant for something else. See Limitations.

## Toolbar popup (no host permission)

Each login in the popup has **Fill**, and TOTP logins have **Code**; click
the code to fill it. With only `activeTab`, the popup injects the content
script into the tab's top frame and fills the first visible login (or OTP)
form there. The same origin check applies. Login forms inside iframes cannot
be filled this way.

## Saving logins

Detection is conservative:

* **Triggers:** a form `submit` event, a trusted Enter in a login field, or a
  trusted click on a submit button. Outside a `<form>`, a button counts only
  if its label says it submits ("Sign in", "Next", "Entrar", and so on), so
  "show password" toggles do not.
* **Only passwords the user produced.** The content script records where
  each field's value came from: typed by the user (trusted `input` events),
  generated by us, or filled from the vault. It also records the value. A
  submission is reported only if the password field still holds a value the
  user typed or we generated. A vault fill is not new. A value planted or
  swapped by page script is not the user's: otherwise a page could forge a
  submit and use the prompt as an oracle for "is this the saved password?".
* A new password beats the current one (signup, change password), and is
  dropped if its confirmation does not match.
* **Multi-step logins:** a username-only step is remembered in memory for 5
  minutes, per tab and origin, and joined with the password step.
* **Generated passwords:** if a generated password was filled but no submit
  was seen, it is offered when the page unloads, so it is not lost.

The background asks the desktop `check_login`. `Unchanged` (this username
already has this password for this site) shows nothing. `Add` and `Update`
show a prompt in the tab's top frame. The prompt reappears on the next page
if the form navigated. The prompt shows the site and username. The password
stays in background memory only, and is dropped on confirm, on "Not now",
after 3 minutes, and when the vault locks. Nothing is saved without the
click.

Saving an **update** changes only that login's password. The old one moves
to the login's password history (5 entries, shown in the desktop app), and
the bridge allows one browser-initiated change per item every 10 minutes. A
new login is saved with the page's origin as a whole-site rule and the host
(without `www.`) as its title.

## Passkeys

HavenKeys answers a site's WebAuthn calls on the hosts the user granted for
in-page suggestions, and on no others. Keys, formats and the Rust checks are
in `crypto.md` §Passkeys and `security-model.md` §15.

### How a request reaches HavenKeys

`webauthn-page.js` runs in the page's own world at `document_start`, before
the site's scripts, and wraps `navigator.credentials.create`,
`navigator.credentials.get` and `PublicKeyCredential.isConditionalMediationAvailable`.
It turns a request into plain data for the isolated-world bridge, and turns
the answer back into a `PublicKeyCredential`-shaped object (`rawId`, `type`,
`authenticatorAttachment: "platform"`, `toJSON()`,
`getClientExtensionResults()` returning `{}`, and on create
`getPublicKey()`, `getPublicKeyAlgorithm()`, `getAuthenticatorData()`,
`getTransports()`).

It hands the call straight to the browser's own implementation, unchanged,
when:

* there is no `publicKey` member, or `get()` asks for `mediation: "silent"`;
* `create()` asks for `mediation: "conditional"` (automatic passkey upgrade
  after a password sign-in is not offered);
* `create()` asks for `authenticatorAttachment: "cross-platform"`, or does not
  list ES256 (`-7`) in `pubKeyCredParams`;
* the challenge is empty or over 1024 bytes, the user handle is empty or over
  64 bytes, the `rpId` is empty or over 253 characters, or the options cannot
  be read;
* `get()` names `allowCredentials`, none of them 16 bytes long (so none can
  be ours).

A non-finite or negative `timeout` is sent as "none". Otherwise the request
goes through the bridge to the background, which asks the desktop.

### Sign in

* **Modal `get()`:** if the vault has passkeys for this page, the passkey
  card (`passkey.html`) opens at the top right of the viewport and lists them
  (login title and account name). The user clicks one to sign in — one match
  is still one click, never automatic — or clicks **Use another device**
  (the browser's own UI), **Cancel** or Escape (`NotAllowedError`). With no
  match, nothing appears and the browser's own UI runs.
* **Conditional `get()`** (`mediation: "conditional"`, passkey autofill):
  HavenKeys and the browser's own conditional request run side by side. When
  the user clicks a username or password field in the frame that made the
  request, the field menu lists that frame's matching passkeys first, marked
  "Passkey · account", above the saved logins. Picking one signs in and
  aborts the browser's request; if the user picks from the browser's UI
  instead, HavenKeys' request is cancelled. `isConditionalMediationAvailable()`
  returns true wherever the script runs.
* After a passkey sign-in nothing else is filled. A later OTP field uses the
  normal one-time-code menu.

### Create

1. The desktop is asked whether the site's `excludeCredentials` names a
   passkey already in the vault. If so, the card opens saying "A passkey for
   this account is already saved in HavenKeys", with **Close** and **Use
   another device**. Only a click on **Close** gives the site
   `InvalidStateError`; **Use another device** hands the request to the
   browser, and Escape or the timeout give `NotAllowedError`. The site is
   not told at once, so a page cannot learn without a click which of its
   accounts HavenKeys holds (`security-review.md` PK19).
2. Otherwise the save card opens: the site, the account name the site sent,
   and a choice of **Add to "…"** for each login saved for this page (at most
   8 passkeys per login; the one with the same username first) or **New
   login** (titled after the host, with a whole-site rule for the page's
   origin).
3. **Save to HavenKeys** creates the key in the desktop, sends the updated
   login to the server, and only then returns the credential to the site.
   **Use another device** hands the original request to the browser.
   **Cancel** or Escape gives `NotAllowedError`.

If the vault already holds a passkey for the same site and account (rpId and
user handle), the new one **replaces** it, in the login that holds it, even
if the user picked a different login or "New login". WebAuthn authenticators
do the same.

### Errors and states

| Situation | What the site sees |
|---|---|
| Desktop not running, integration off, internal error, nothing to offer | The browser's own WebAuthn, as if HavenKeys were not installed |
| Vault locked, modal request | A card saying HavenKeys is locked; it updates to the real chooser or save card once the vault is unlocked (the card asks every 1.5 s, and the background looks the passkeys up again each time). **Use another device** still works |
| Vault locked, conditional request | The browser's own conditional UI only; the page has to ask again after unlock |
| Offline during create | The card shows that HavenKeys is offline and stays open; nothing is stored. **Use another device** still works |
| `rpId` not allowed for the page (checked in Rust) | The browser's own WebAuthn, which applies the same rule and raises `SecurityError` itself, or serves the paths it allows and HavenKeys does not (related origins, permitted cross-site iframes) |
| Site aborts (`AbortSignal`) | The card closes; the site's own abort reason |
| Site's timeout (clamped to 10 seconds – 5 minutes) passes | The card closes; `NotAllowedError` |
| Page leaves (`pagehide`, including entering the back/forward cache), or removes, hides or moves the card's frame | Request cancelled (`AbortError` on `pagehide`, `NotAllowedError` for the frame). For a conditional request only HavenKeys' side ends; the browser's own conditional request still answers the site |
| Extension disabled or updated while the page stays open (the page script remains, the bridge is gone) | No acknowledgement within 1 s: the browser's own WebAuthn |
| Background worker restarted, losing the session | Modal: the browser's own WebAuthn (within 20 s, or at once when the user clicks Cancel or **Use another device** on the card). Conditional: HavenKeys asks again |
| Vault locks while the card is open | The card closes; `NotAllowedError` |

### Sessions

The background keeps one passkey session per tab, bound to the random token
in the card's URL fragment and to the tab, frame, `documentId` (Chromium)
and origin of the frame that asked. Picks are accepted only for passkeys or
logins the session offered, one at a time. A new request in the same tab
ends the previous one with `AbortError`. The token is visible to the page
(as for the menu); what it cannot do is change which origin the desktop
checks. While one `get()`/`create()` lookup is in flight for a tab, another
from the same tab falls back to the browser at once, so a page that fires
requests in a loop cannot use up the desktop's shared lookup budget.

Sessions live in the background worker's memory, and a Manifest V3 worker
can be suspended when idle. The bridge therefore pings its session every
20 s (`wa_ping`), which keeps the worker awake and tells the bridge when the
session is gone anyway. A lost conditional session is requested again with
the options the bridge kept; a lost modal one ends in the browser's own
WebAuthn and the card is removed. A card whose session is gone can still be
closed: Cancel, Escape, **Close** and **Use another device** are then sent
to every frame of the tab, and only the bridge holding that token acts.

The bridge acknowledges each request synchronously. The page script falls
back to the browser when no acknowledgement comes within 1 s (no bridge), and
ends an acknowledged modal request that is never answered with
`NotAllowedError` after the clamped timeout plus 15 s.

## Permissions and injection

* Default: `nativeMessaging`, `activeTab`, `scripting`. The content script
  runs only in a tab after the user fills from the popup there.
* In-page suggestions and save prompts are opt-in, from the options page.
  They request the optional host permissions `https://*/*` and `http://*/*`.
  When granted, the background registers the content script for the granted
  patterns in all frames. When revoked, it unregisters it. The two passkey
  scripts (§Passkeys) follow the same grant, registered as a separate group
  at `document_start`, so a browser that refuses `world: "MAIN"` keeps
  in-page suggestions.
* **Per-site grants are not followed yet.** Registration asks only whether the
  broad `https://*/*` / `http://*/*` patterns are held. If you narrow site
  access to chosen sites through the browser's own controls, that check
  returns false and in-page suggestions stop working everywhere rather than
  working on the sites you chose. It fails closed, but it is not the
  behaviour the browser UI implies; reading the real grant with
  `permissions.getAll()` is the fix.

See `security-model.md` §12.

## Limitations

* **Clickjacking on Firefox.** Firefox has no IntersectionObserver v2, so a
  page that covers the menu with a click-through decoy is guarded only by
  the 400 ms delay. The item filled still has to match the page's origin in
  Rust.
* **Page-controlled URL paths.** A page can change its own path with
  `history.pushState`. That only matters for "exact page" rules, and only
  within the same origin, which can already do anything to itself.
* **Signals visible to the page.** A page can see that an iframe appeared
  after a click, and its height. That tells it the user has 1–5 logins for
  this site, or that HavenKeys is locked. On Chromium the extension ID is
  fixed, so any page can detect that HavenKeys is installed by loading
  `menu.html`. Firefox uses a random per-install ID.
* **Menus inside iframes** are drawn inside that frame, so a small frame can
  clip them.
* **Closed shadow roots** are not reachable, so fields inside them are not
  classified.
* **Heuristics can be wrong.** A wrong guess can show a menu on the wrong
  field or miss a form. It cannot fill another site's login: the origin check
  is in Rust.
* **Save prompts** depend on seeing the submission. Script-driven logins that
  never fire a submit, click or Enter are missed, except for generated
  passwords (offered on unload).
* **Tabs opened before inline suggestions were turned on** need a reload.
  So do passkeys: the page script must run before the site's own scripts.
* **Passkeys only on granted sites.** Without the host grant, sites get the
  browser's own WebAuthn.
* **Passkey clickjacking on Firefox** has only the 400 ms delay, as for the
  menu. The worst case is a sign-in or a new passkey for the page's own
  rpId.
* **One passkey request per tab.** A granted iframe that calls WebAuthn ends
  the top page's pending request (and the reverse). Denial of service only.
* **Back/forward cache.** Leaving a page cancels HavenKeys' side of its
  passkey requests, so a page restored from the cache has no HavenKeys
  passkey autofill until it calls `get()` again. The browser's own
  conditional request is left running and can still answer the site.
* **Lock events need an open native port.** The extension closes its port to
  the host after 60 s without a request, and then hears no events. A card
  open when the vault locks stays up until it is used or expires; using it
  then gets "HavenKeys is locked" from the desktop. A locked card does
  update on unlock, because its own polling re-runs the lookup.
* **Signals visible to the page (passkeys).** A card appearing, or a
  conditional request answered by the field menu, shows the page that
  HavenKeys has something for the site. With `allowCredentials`, a chooser
  versus an immediate fallback to the browser tells the page whether one of
  the listed credentials is in HavenKeys (`security-review.md` PK19).
* **No WebAuthn extensions** (PRF, largeBlob, credProps, …), no attestation
  other than `none`, and no hybrid (phone) transport from HavenKeys itself;
  **Use another device** reaches the browser's own.
