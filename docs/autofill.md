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
| `check_login(url, top_url, username, password, current?)` | `Add`, `Update(id)` or `Unchanged`. Never a password | Only logins matching the page are compared |
| `save_login(url, top_url, username, password, target)` | The item ID | `target` is a new login (with an optional title) or an update. An update must target a login matching the page and keeps its title; a new login is saved for the page's origin |

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
interacts with them**: a trusted click, Tab focus, ArrowDown or typing in an
input, or a click on the field's HavenKeys icon. Focus alone (including a
field the page autofocused) classifies only the focused field, to decide
whether it gets the icon; it never opens a menu.
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

### Field icon

A focused login, new-password or one-time-code field shows a small HavenKeys
icon at its right edge (`content/icon.ts`). Clicking it toggles the menu; on
a site with nothing saved it opens a "no logins for this site" menu instead
of doing nothing (`cs_open_menu` with `explicit: true`). The icon moves left
past the page's own button in the field (show password, clear), and is left
out when the field is too small or no spot is free. It lives in a closed
shadow root with `!important` inline styles, holds no vault data, and
ignores untrusted clicks. It follows its field every 500 ms and on scroll
and resize, and goes away when the field loses focus.

Typing in a login field opens the menu too, once per field: not after the
user closed the menu there (Escape, the icon, or a pick), and not again when
the desktop had nothing to offer.

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
| `autocomplete=current-password` | disqualifies | |
| `autocomplete=new-password` | ignored: sites set it on every field to switch off browser autofill (gov.br's CPF box) | |
| `type=email` | +80 | |
| Strong keyword in `name`/`id` (username, login, email, account, usuario…) or a document-number word (CPF, CNPJ, RG, documento, matrícula, NIF, NIE, DNI, CIF, RUT, CUIT/CUIL, CURP, RFC, cédula, passport, national id, codice fiscale) | +50 | |
| Strong keyword in placeholder, aria-label, title, `<label>`, `aria-labelledby` | +40 | |
| Weak keyword (user, mail, phone…) | +25 / +20 | |
| OTP keyword (otp, 2fa, verification code, token…) / weak (code, pin) | | +70 / +30 |
| `maxlength` 4–8, numeric input mode | | +15, +10 |
| 4–8 adjacent single-character boxes (split code) | | +65 |
| Last text field before the first password field | +40 | |
| Username-only step on a form whose intent is login, for a field with any username or document word | +30 | |
| Negative words (search, coupon, newsletter, address, name…) | −80 | 0 for postal/zip/promo/CVV… |
| Card or address `autocomplete` tokens | disqualifies | disqualifies |

Keywords are compared as whole words after normalization (lowercase, accents
removed, camelCase and punctuation split): `loginEmail` → `login email`.
English, Portuguese and Spanish are covered.

Thresholds: a username needs 50 when the group has a password field. Without
one (a username-only step) it needs 70, so a stray text box does not qualify.
A box labelled only "CPF" with a generic `name` reaches 70 on a sign-in form
(heading, submit button, action or path says sign in / entrar / login) but
not on a checkout form. Short ambiguous tokens (`id`, `ci`, `cc`, `run`) are
deliberately not document words.
An OTP field needs 60. Each group has at most one username: the best-scoring
text field before the first password field.

**Password roles** are resolved for the group as a whole, in this order:

1. `autocomplete`: `current-password`; `new-password` (a second one, or one
   with confirm wording, is the confirmation; one worded current, such as
   gov.br's "Digite sua senha atual", is the current password, because sites
   use `new-password` to switch off browser autofill).
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

With automatic sign-in on, the content script then presses the form's
button (see [Automatic sign-in](#automatic-sign-in)).

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

Both frames start at an estimated size (the menu from its row count, the
save prompt fixed) and then report their real content height once rendered
— `menu_resize` / `save_resize` to the background, which relays it to the
content script as `bg_resize_menu` / `bg_resize_save` — so a wrapped row or a
long (for example, Portuguese) error message is never clipped. The reported
height is validated on both hops: an integer within a fixed range (90–420px
for the menu, 100–320px for the save prompt — enough for the header, one row
and several wrapped rows, or a one-line question and a longer wrapped error)
and bound to the live session's token; anything else is dropped and the
frame keeps its estimated size.

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

Save and update prompts do not depend on the in-page suggestions
preference: they work whenever the extension has access to the site.

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
* **Change-password forms** rarely have a username field. With a new
  password, the form's current password goes along too, if the user typed it
  or we filled it from the vault (never one page script planted). It is only
  compared in the core, to find the login that changed, and is not kept.
* **Multi-step logins:** a username-only step is remembered in memory for 5
  minutes, per tab and origin, and joined with the password step.
* **Generated passwords:** if a generated password was filled but no submit
  was seen, it is offered when the page unloads, so it is not lost.

The background asks the desktop `check_login`. `Unchanged` (this username
already has this password for this site) shows nothing. `Update` when a
login for the site has this username with another password or, with no
username, when exactly one login for the site (with this username, if there
is one) has the form's current password; two logins sharing it are too
ambiguous, and the answer is `Add`. `Add` and `Update` show a prompt in the
tab's top frame. The prompt reappears on the next page
if the form navigated. The prompt shows the site and username (for an update from a form with no
username, the saved login's). The password
stays in background memory only, and is dropped on confirm, on "Not now",
after 3 minutes, and when the vault locks. Nothing is saved without the
click.

Saving an **update** changes only that login's password. The old one moves
to the login's password history (5 entries, shown in the desktop app), and
the bridge allows one browser-initiated change per item every 10 minutes. A
new login is saved with the page's origin as a whole-site rule.

**A new login's name** is editable in the prompt before saving. It starts as
the site's name when the page's host is in the bundled Passkeys Directory
(`github.com` → "GitHub"; the host or a subdomain of a listed domain), and
as the host (without `www.`) otherwise. The page's own `<title>` is never
used: it is page-controlled, so any site could name itself "GitHub" in the
vault. The prompt is an extension frame the page cannot read or type into,
so the name is the user's or ours. A cleared field falls back to the
suggestion. The field takes no focus when the prompt opens, and Enter saves
(trusted key presses only, past the click guard). The core checks the name
like any title (256 characters, no control characters). An update keeps the
login's title, and the protocol refuses a title with `itemId`.

## Automatic sign-in

After the user picks a login (the in-page menu, or **Fill** in the popup)
and Rust says the pick may auto-submit, HavenKeys keeps going: it presses
the site's sign-in button, and if the flow continues to a password step or a
one-time-code step, it fills and presses those too.

```text
pick → fill → press
         → (page navigates or re-renders) fill password → press
         → (OTP step) fill code → press
```

### Switches

* Vault setting `auto_sign_in` (Settings → *Browser extension* → "Sign in
  automatically after filling"), **default on**.
* Per-login switch `auto_sign_in` (login editor → "Sign in automatically on
  this site"), **default on**; disabled with "Turned off in Settings" when
  the global setting is off.
* `autoSubmit = settings.auto_sign_in && item.auto_sign_in`, computed only
  in Rust (`VaultService::auto_sign_in_for`, called after the origin check)
  and returned alongside every `fill_item` / `get_totp` result
  (`native-messaging.md`). The extension never decides this itself.

### The run

A pick whose fill comes back `autoSubmit: true` starts a **sign-in run** in
the background worker's memory (`background/signin-run.ts`): the tab, the
frame and the exact origin (scheme + host + port) of the page the user
picked in, the item ID, whether it has TOTP, and an expiry 2 minutes out
(`RUN_TTL_MS`). One run per tab; a new pick replaces it. Steps move forward
only, `username → password → otp`, each at most once, never retried.
Continuing a step still re-requests the value from Rust for the frame's
current URL, so the origin check runs again on every step. Nothing about a
run is persisted, and it holds no secrets — only the item ID, origin, frame,
step and timestamps.

### Detecting the next step

Two ways a next step can appear:

* **Navigation.** Every frame's content script sends `cs_ready` on load; the
  background replies with `watch: "password" | "otp"` only when the tab has
  a live run, the frame is the run's frame, and its origin is `run.origin`.
* **Re-render (SPAs).** After pressing, the content script starts watching
  at once for the step named in the `bg_fill` that carried `submit: true`.

Watching (`autofill/watch.ts`) checks once immediately with the same bounded
page lookup the popup fill uses (`findLoginGroup` / `findOtpGroup`, at most
200 candidates), then observes `document` with one `MutationObserver`
(`childList` + `subtree`, plus `class`/`style`/`hidden`/`disabled`/`type`
attributes), debounced to at most one check per 150 ms. A field counts only
once the same connected, visible, enabled element is found on two
consecutive checks, one debounce (about 150 ms) apart, so a field that
flickers in and out is not reported. The watch gives up after 30 s
(`WATCH_TIMEOUT_MS`) and ends the run.

### Pressing the button (`autofill/submit.ts`)

Candidates are `button`, `input[type=submit]`, `input[type=image]` and
`[role=button]`, at most 30, visible. Disabled buttons (the `disabled`
attribute, inside a `<fieldset disabled>`, or `aria-disabled="true"`) are
still candidates and can still be chosen — many sites enable
their submit button only once the input validates — but a disabled one is
never pressed until it enables (see below). Candidates are drawn first from
the filled group's own root (its `<form>`, or the container `groupRoot`
picked). For a form-less group, when no candidate in a scope reaches score
60 (or the scope has no visible candidate), the search continues to the next
ancestor, up to 3 of them, looking for a button beside the fields' own
container, which is common in SPAs. It stops and refuses (no press) on a
tie: a scope whose best candidate reaches 60 but beats the runner-up by less
than 20 ends the search there, rather than looking further up. A group
inside a `<form>` is searched only within that form.

| Signal | Score |
|---|---|
| The submit button of the filled field's own form (`type=submit`, or `button` with no type, inside `field.form`) | +60 |
| Label fits the step. `username`: continue, next, proximo, continuar, avancar, seguinte. `password`: sign in, log in, login, signin, entrar, acessar, iniciar sesion. `otp`: verify, confirm, submit, verificar, confirmar, enviar | +50 |
| Any existing submit word | +20 |
| After the last filled field in document order | +10 |
| Negative words: forgot, reset, create account, sign up, register, cadastrar, cancel, cancelar, back, voltar, resend, reenviar, show, mostrar, another, outra, passkey, "with google/apple/facebook/microsoft/github", "com google/apple/…" | disqualifies |

**Ambiguity rule:** press only if the best candidate scores at least 60 and
beats the runner-up by at least 20 in that same scope. Otherwise the fields
stay filled and the user presses, exactly like today's behaviour, and the
run ends.

Once a button is chosen: wait for it to become enabled (re-checked every
100 ms, up to 1 s — many sites enable a submit button only once the input
validates; still disabled after 1 s → no press, run ends); re-check the stop
conditions below; then `button.click()`, so the site's own click handlers,
validation and `submit` handlers run as they would for a real click. (Not
`form.requestSubmit(button)`: it skips the button's click handlers, where
sites like gov.br start their submit.) Before pressing an OTP
step, wait 500 ms first — if the field is gone or hidden by then, the site
already submitted on its own and nothing is pressed. Never a synthetic key
event, never Enter.

A press in flight is cancellable: it is tied to the run that started it, and
a user taking over, a `bg_run_end` (the vault locking, or a new pick), or
the frame's `pagehide` cancels it before it presses. The background also
re-checks the run's identity after each request to the desktop during a
continuation, so a lock, a stop, or a new pick that lands while that request
is in flight keeps the stale response from being filled.

Ending a run after its last step (the OTP step, or the password step of a
login without TOTP) does not cancel that step's press. When the content
script reports it is pressing that last step, the background drops its run
without sending `bg_run_end`, because the press may still be pending in the
frame (the 500 ms OTP wait, a button that enables late); the content script
ends its own run once the press completes. If the press itself throws, the
content script ends the run and tells the background.

### Stopping

Checked while watching, and again right before a press. Any of these ends
the run; nothing more is filled or pressed.

| Condition | Detected by | Effect |
|---|---|---|
| The user takes over | A trusted `keydown`, `input` or `pointerdown` on the page during the run | End (so Esc ends it) |
| A step comes back | The field of an already-submitted step is found again (password after its submit: wrong password; OTP after its submit: wrong code) | End, no fill |
| Challenge on the page | A visible iframe whose parsed URL host is `www.google.com`/`www.recaptcha.net` with a `/recaptcha/` path, `*.hcaptcha.com`, or `challenges.cloudflare.com`; or a visible `.g-recaptcha`, `.h-captcha`, `.cf-turnstile` element, anywhere in the document | Fill the current step, do not press, end |
| OTP without TOTP | `watch: "otp"` is never requested when `hasTotp` is false; an OTP field appearing then simply ends the run at the password step | End |
| Nothing appears | 30 s per step, 2 min overall | End |
| Context gone | Origin change, lock, tab closed, new pick | End |

Invisible reCAPTCHA badges do not count as a challenge, so they never
silently disable auto sign-in on the very common pages that carry one: a
challenge iframe with `size=invisible` is excluded, and so is a
`.g-recaptcha` / `.h-captcha` / `.cf-turnstile` element that is itself the
page's sign-in button (Google's documented button-bound invisible/v3
pattern, `<button class="g-recaptcha" data-callback=...>`) or that carries
`data-size="invisible"`.

### Popup-only mode

Without the host permission (withdrawn by the user), the popup injects the
content script through `activeTab`, which the browser revokes once the tab
navigates. A run started that way covers only the current page: single-page
forms and SPA steps, not a step on a page the site navigates to.

### Limitations

* **Host-hopping flows stop at the hop.** A run is bound to one exact
  origin; a step that lands on a different origin (a separate login
  subdomain, an SSO redirect) ends the run there, and the menu still works
  manually on the new page.
* **Sites that check `isTrusted` reject the scripted click** and simply do
  not react; the run then times out, or the user presses by hand.
* **CAPTCHAs stop the press.** The current step's fields stay filled; the
  CAPTCHA itself is never solved for the user.
* **Heuristics can pick no button.** When no candidate clearly wins, the
  fields are filled and the user presses, exactly like today's behaviour.

## Sign in with Google, Microsoft, GitHub, Apple

Some logins are never used with a password: the site is always reached
through "Sign in with Google" (or Microsoft, GitHub, Apple). HavenKeys can
remember which provider and account a login uses (`sign_in_with` on the
item), offer it on the site's own sign-in page, and — after one click —
press the site's button and pick the saved account on the provider's own
chooser. It never enters a password or grants permissions on this path.
Providers, their exact origins, and the wire messages are in
`native-messaging.md` §"Sign in with"; the security properties are in
`security-model.md` and `threat-model.md`.

Code: `apps/extension/src/autofill/sso.ts` (pure field detection, shared by
the balloon scan, the press and the chooser), `content/sso.ts` (per-page
state), `background/sso-state.ts` and `background/sso-handler.ts`
(cross-tab/cross-frame state and the wire calls).

### Recognizing a provider button

Candidates are the same broad set as other clickable elements (`button`,
`a[href]`, `[role=button]`, `[role=link]`, `input[type=submit|button]`), at
most `MAX_SSO_CANDIDATES` (400) of them, visible and not disabled. Each is
scored per provider from its text, `aria-label`, `title` and an inner
`img[alt]` (English and pt-BR: "Sign in / Log in / Continue / Sign up with
`<Provider>`", "Entrar / Continuar / Acessar com `<Provider>`"), with a bonus
when the `href` itself points at that provider's origin. A bare "`<Provider>`"
label counts only with context — another provider's button, or a login field,
on the page — so a plain "Google" is not enough on its own. Misleading text
("Google Drive", "GitHub repository", download-store wording, and so on) is
excluded. A label needs a score of `SSO_MIN_SCORE` (60) to qualify.

### The balloon

While in-page suggestions are on, the top frame watches for provider buttons
for `SCAN_WINDOW_MS` (60 s) after load and after every URL change (route
changes reopen the window), rescanning at most every `SCAN_DEBOUNCE_MS`
(750 ms) through the same debounced `MutationObserver` other autofill uses —
never the whole document on every mutation. When the set of recognized
providers changes, the content script tells the background
(`cs_sso_buttons`, no page data beyond the provider list); the background
asks `find_matches` for the page and keeps only matches whose `provider` is
one of them. If any remain, a small balloon opens in the top-right corner:
one row per login, `[icon] Sign in with `<Provider>` · `<account>``. It
follows the existing in-page suggestions preference — off, no balloon — and
is offered only in the top frame; an iframe never gets one. Closing it hides
it for that page (origin + pathname; a query or hash change alone does not
bring it back) until the next navigation. SSO logins also appear, with the
same icon, in the top-frame field menu and the toolbar popup (**Sign in**
instead of **Fill**); all three entry points start the same run.

### The run

1. The user picks an SSO item. The background asks the desktop `start_sso`,
   which re-checks the item against the page (the same origin check as
   `fill_item`) and returns the provider, the saved account, that provider's
   **exact origins** (a fixed table in Rust) and whether the run may
   auto-choose (`settings.auto_sign_in && item.auto_sign_in`). No secret
   comes back. The background keeps this, with no secrets, as an `SsoRun`:
   tab, frame, the site's origin, provider, account, the provider origins,
   `autoChoose`, a phase (`press` then `choose`), and an expiry
   `SSO_RUN_TTL_MS` (2 minutes) out. One run per tab; a new pick replaces it.
2. **Press.** The background asks the top frame to press that provider's
   button (`bg_sso_press`, carrying the origin the desktop matched); the
   content script refuses unless it is still the top frame and still on that
   exact origin, then looks for a **clear winner** among that provider's own
   candidates — the best score beats the runner-up by at least 20 points, as
   `submit.ts` requires for automatic sign-in — and clicks it. No winner, or
   a challenge (reCAPTCHA/hCaptcha/Turnstile) on the page: nothing is
   pressed, and the balloon shows "Couldn't find the Sign in with `<Provider>`
   button". Only the top frame ever presses; an SSO row inside an iframe
   login widget is reachable only through the balloon or the popup, not a
   subframe field menu.
3. **Choose.** Once pressed, the run only continues if `autoChoose` is on and
   the item has a saved account. A frame in the run's tab, or in a **popup
   that tab opened** (`openerTabId`, supplied by the browser, not the page),
   is told to choose only once its own origin is one of the run's provider
   origins. It retries on a debounced `MutationObserver` for up to
   `CHOOSE_WAIT_MS` (10 s), clicking the saved account only when
   `chooserRow` finds **exactly one** clickable, visible element whose text
   contains that account (case-insensitive) and whose own label is not
   itself consent wording — two matching rows, or none, and nothing is
   clicked. The run ends after one click, successful or not.

### Stop conditions

Checked before every press and before every choose attempt:

| Condition | Effect |
|---|---|
| Consent or permissions wording ("Continue as …", "Allow…", "Grant access/permission(s)…", "Authorize…", recognized at any length as a prefix, plus a few short bare words such as "Accept"/"Continuar" under 30 characters) | Never pressed; the run ends |
| Ambiguity: no clear-winner button, or more than one (or zero) chooser rows match the account | Nothing is clicked; the run ends |
| A password field / no matching chooser row (the provider asks for a password instead of showing a chooser) | `chooserRow` simply finds nothing to click; the ordinary field menu still offers the vault's login for the provider's own origin, and the user picks it manually |
| Any trusted user input (`keydown`, `input`, `pointerdown`) in the run's tab, or in its opener popup | Ends the run (`cs_sso_stop`) |
| The frame's origin is not the one the desktop matched (press), or not one of the run's provider origins (choose) | Refused |
| Vault lock, 2 minutes elapsed, or the tab navigates away | Ends the run |

### Save detection

A trusted (`isTrusted`) click on a recognized provider button starts a
`PendingSso` in background memory: the tab, the page URL, the provider, and
an expiry `PENDING_TTL_MS` (5 minutes) out. One per tab; a new click replaces
it; scripted clicks are ignored. The account is learned **only on that
provider's own origins**, in the tab or in a popup it opened, never from the
site's own page: a trusted click on an account-chooser row (the email-shaped
text inside it), or a username-only step submitted there. Reaching a
provider origin also marks the pending capture as "saw the provider" — this
is what "reached the provider" means; an embedded Google Identity Services
iframe that never navigates the top frame does **not** count, so a page
cannot arm a save prompt just by drawing a GSI button nobody clicked.

Once the provider has been reached, either of these asks the desktop
`check_sso` while the pending capture is still valid:

* the first later top-frame load in the tab on an origin that is **not** one
  of the provider's origins (back on the site, or somewhere else);
* the provider popup closing (many sites run OAuth in a popup and never
  reload the tab).

`unchanged` (this provider and account, or this provider with no account, is
already saved) shows nothing. `update` (exactly one login has this provider
and no account, typically imported from 1Password) offers to add the account
to it. `add` shows the ordinary save balloon, naming the site **where the
user clicked**, not the page they returned to, with the account and a title
editable before **Save**. A stale pending capture simply expires without
ever asking, so it can never block a later, unrelated offer. Everything —
pending captures, runs and open balloons — is dropped on lock.

### Known limitation

**Google Identity Services (GIS) iframe buttons and One Tap are not
recognized or pressed.** They are iframes from `accounts.google.com/gsi`
embedded directly in the site, not a same-origin button HavenKeys can find
and click; sites with their own "Sign in with Google" button work normally.
Corporate SSO (Okta, SAML, an organization's Azure AD tenant beyond the fixed
Microsoft origins) is out of scope entirely.

## Passkeys

HavenKeys answers a site's WebAuthn calls on the hosts the extension has
access to (every http/https host by default), and on no others. The in-page
suggestions preference does not affect it. Keys, formats and the Rust checks are
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

### Automatic upgrade

Right after HavenKeys fills a password on a site and the user signs in, some
sites call `create()` with `mediation: "conditional"` to offer a passkey for
that same sign-in (the *automatic passkey upgrade*). `page.ts` forwards this
like any other `create()` (`CreateOptions.conditional`); it is the desktop,
not the extension, that decides what happens next:

* The desktop remembers each password fill — which login, which site
  (registrable domain), when — for 5 minutes, in the unlocked session's
  memory only (`threat-model.md` T8, "Fill memory"). A conditional
  `create()` on that same site, within that window, naming that login's
  account name (folded) or none, and while the login can still take another
  passkey, is that fill's automatic upgrade.
* **`auto`** (the vault setting `auto_passkey_upgrade`, on by default): the
  passkey is created without a card. The credential goes back to the site at
  once, and the bridge shows a small notice frame for 4 seconds — "Passkey
  saved to HavenKeys · `<site>`", "Manage it in the HavenKeys app" — that the
  page cannot keep up (touching it removes it).
* **`ask`** (the setting off): the ordinary save card opens, titled "Add a
  passkey?" instead of "Save a passkey to HavenKeys?", with the login that
  was just filled preselected among the "Add to …" choices.
* Anything else — no recent fill, a different account name, the matching
  login already has 8 passkeys, the site's `excludeCredentials` already
  names a passkey HavenKeys holds, a passkey for that account already in the
  vault, a fill already spent on a silent save, the desktop locked or
  unreachable, or any error — falls back to the browser, exactly like a conditional `create()`
  from a site HavenKeys does not recognize (normally a no-op).

Rust makes this decision twice: once to answer `check_passkey_create`'s
`upgrade` field (`none` / `ask{itemId}` / `auto{itemId}`), and again inside
`passkey_create` itself, which refuses (`Denied`) a conditional request
unless it still comes out `Auto` for exactly the item named. The extension's
claim that a request is the automatic upgrade, or that it is for a
particular item, is never trusted on its own. A silent save only adds a
passkey: if any login already holds one for the same account (rpId and user
handle), the conditional request is refused and replacing it needs the card.
A successful silent save spends the fill, so one password fill grants at
most one silent passkey; a second conditional `create()` falls back until
the user fills again. This keeps the silent path narrow against hostile
pages and extension bugs; it is not a boundary against a compromised
extension, which could already use the ordinary clicked save
(`threat-model.md` T8).

Setting: Settings → *Browser extension* → "Add passkeys automatically after
I sign in" (off: "HavenKeys asks first").

If the page aborts, navigates away, or otherwise cancels its request while
an `auto` save is already in flight, the cancellation reaches the extension
too late to stop it: the passkey the desktop already created and sent to the
server **stays in the vault**, visible in that login's Passkeys list in the
desktop app, even though the site never receives it and no "saved" notice
appears. See Limitations.

### Passkey hints in the field menu

Opening a login field's menu on a site that has at least one saved login:

1. If the site's own conditional `get()` is already waiting (passkey
   autofill), HavenKeys' matching passkeys lead the menu, unchanged (see
   [Sign in](#sign-in)).
2. Otherwise, if HavenKeys already holds a passkey the page may use
   (`passkey_status`, answered by `has_passkey_for_page`), the first row is
   a hint with no click action: "You have a passkey for `<site>`" / "Use the
   site’s “Sign in with a passkey” option", ahead of the saved logins.
3. Otherwise, if the page matches an entry in the **Passkeys Directory**
   that has a help link, the last row is "`<name>` supports passkeys" /
   "How to add one". A click, through the menu's usual trusted-click guard,
   opens the entry's help URL with `chrome.tabs.create` and closes the menu.
   With no help link, or no match, the row is not shown.

`passkey_status` is a Lookup-class request (like `find_matches`), asked once
each time the menu opens. Its answer never reaches the page — only whether
our own menu shows a hint does, and a page can already see that the menu
frame appeared and roughly how tall it is (see Limitations, and the
`passkey_status` oracle entry in `threat-model.md` T8).

**Passkeys Directory.** The names, domains and help links behind row 3 are a
snapshot of the [Passkeys Directory by
2factorauth](https://github.com/2factorauth/passkeys), licensed
CC-BY-4.0 ("Passkeys Directory by 2factorauth"; see
`THIRD-PARTY-NOTICES.md`), committed at
`apps/extension/src/data/passkey-sites.json` and never fetched at runtime.
The Directory's public API (`passkeys-api.2fa.directory`) is keyed by domain
and carries no site names, so `scripts/update-passkey-directory.mjs` — run
by hand, not at build time — instead clones
`github.com/2factorauth/passkeys` and reads its source `entries/*/*.json`
files directly, keeping only entries with a valid hostname and passwordless
or MFA support, and only `https:` documentation links. The generated file is
reviewed like any other change before it is committed. Matching a page to an
entry (`findPasskeySite`) is a plain host-suffix comparison — the page's
host equals a domain or ends with `.` + a domain, longest domain wins — with
no Public Suffix List: this is only a UI hint, never an authorization
decision, so the worst a wrong match can do is show or hide a help link.
`github.com.evil.com` still never matches `github.com`. The site name is
rendered with `textContent` only, as untrusted third-party text.

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
| Vault locks while the card is open | The card closes; `NotAllowedError`. An automatic upgrade's "Add a passkey?" card instead falls back to the browser, as a locked conditional create does |
| Conditional create, no recent password fill / a different account name / the matching login already at 8 passkeys | Fallback, silently — as if HavenKeys were not installed |
| Conditional create, `auto_passkey_upgrade` off, a recent matching fill | "Add a passkey?" card, that login preselected |
| Automatic upgrade fails (offline, an internal error) | Fallback, no notice, nothing stored |
| Vault locks within the fill's 5-minute window | The fill memory is gone; a later conditional `create()` on that site falls back |
| `passkey_status` fails (locked, desktop not running, rate limited) | Treated as false: no "you have a passkey" hint, no Passkeys Directory row |

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

* Requested at install: `nativeMessaging`, `activeTab`, `scripting`,
  `storage`, and the host permissions `https://*/*` and `http://*/*`
  (changed 2026-09-28; before, the host permissions were optional and off).
  With the host grant, the background registers the content script for the
  granted patterns in all frames, so save prompts, passkeys and in-page
  suggestions work without setup. If the user withdraws the grant, it
  unregisters it, and the options page and popup offer **Allow**.
* **In-page suggestions** — the field icon and the menu under login fields,
  including the generated-password menu on sign-up fields — are a separate
  preference on the options page, on by default, stored as one boolean in
  `chrome.storage.local` (`shared/prefs.ts`). The background refuses to open
  a menu while it is off, and the content script hides the icon, following
  changes live. One exception keeps passkeys working: when a site's passkey
  autofill (a conditional `get()`) is waiting in the frame, clicking a login
  field still opens a menu with **only** those passkeys — no saved
  passwords, no hints, never from the icon. Turning it off does not affect
  save and update prompts, passkeys, popup fills or automatic sign-in
  started from the popup. The two passkey
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
* **Signals visible to the page.** A page can see the field icon's host
  element when a login field is focused (so, that HavenKeys is installed and
  active here), and that an iframe appeared after a click, and its height. That tells it the user has 1–5 logins for
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
* **Tabs opened before the extension was installed or site access allowed**
  need a reload.
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
* **`PublicKeyCredential.getClientCapabilities()` is unchanged.** A site
  that checks its `conditionalCreate` capability before offering the
  automatic upgrade sees the browser's own answer; HavenKeys does not
  intercept or change that call.
* **An abort during a silent save can leave an orphaned passkey.** If a page
  aborts, navigates away, or otherwise cancels its `create()` while the
  automatic upgrade's save is already in flight, the save is not rolled
  back: the passkey stays in the vault (visible in that login's Passkeys
  list in the desktop app) even though the site never receives it, and the
  "saved" notice does not appear either.
