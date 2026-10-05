# Sign in with Google / Microsoft / GitHub / Apple — Design

Status: proposed, 2026-09-28.
Extends the automatic sign-in amendment (2026-09-26) to one more bounded
case: after the user picks a "Sign in with …" login, HavenKeys presses the
site's provider button and, on the provider's own page, picks the saved
account. It never enters a password or grants permissions on this path.

> This software has not undergone an independent security audit.

## 1. Goal

Many sites are used only through "Sign in with Google" (or Microsoft,
GitHub, Apple). Today HavenKeys cannot record that: the 1Password importer
turns `ssoLogin` into a line of notes, and the extension ignores provider
buttons. Like 1Password, HavenKeys should:

1. **Remember** how the user signs in to a site and with which account.
2. **Offer** it: on the site's sign-in page, a balloon in the top-right
   corner shows "Sign in with Google · user@gmail.com".
3. **Do it** after one click: press the site's provider button, then, on
   the provider's account chooser, click the saved account.

When unsure, it stops and leaves the rest to the user.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Model | A `sign_in_with` field on the existing login item | New `SsoLogin` item type (churn with no security gain); notes/URL convention (not validatable) |
| Providers | Closed list in Rust: Google, Microsoft, GitHub, Apple | Arbitrary/corporate SSO (Okta, SAML) |
| Provider page | Only click the saved account in the chooser; stop on password, consent or ambiguity | Fill the provider's password/TOTP from the vault (a pick on site X would authorize a password fill on another origin); press the site button only |
| When the balloon appears | A matching SSO item exists **and** that provider's button is visible in the top frame | Any login page; never (menu/popup only) |
| Saving | Detect a trusted click on a provider button, learn the account on the provider's chooser, ask on return | Manual only; detect without account |
| Account link to the provider's own login | Derived at display time (same provider origin + same username), never stored | Stored item-id reference (breaks when deleted) |
| Re-import from 1Password | Fills `sign_in_with` on an existing duplicate that lacks it | Reset the vault and import again |

## 3. Model (Rust, `havenkeys-core`)

### 3.1 Providers (`sso.rs`, new)

```rust
pub enum SsoProvider { Google, Microsoft, Github, Apple } // serde: lowercase
```

Each provider has a display name and the **exact origins** a run may
continue into:

| Provider | Origins |
|---|---|
| Google | `https://accounts.google.com` |
| Microsoft | `https://login.microsoftonline.com`, `https://login.live.com` |
| GitHub | `https://github.com` |
| Apple | `https://appleid.apple.com` |

The table lives only in Rust. `packages/protocol` holds a mirror for the
extension's save detection (§5.3), and a parity test checks it against the
Rust list.

### 3.2 Item fields

* `ItemOverview.sign_in_with: Option<SignInWith>`, with
  `SignInWith { provider: SsoProvider, account: Option<String> }`,
  `#[serde(default, skip_serializing_if = "Option::is_none")]`. Old vaults
  parse unchanged; no migration. The overview is encrypted at rest, like
  `username`.
* `ItemInput.sign_in_with: Option<SignInWith>`, sent in full like
  `username` and `urls` (`None` clears it). Mirrored in
  `apps/desktop/src-tauri/src/item_input.rs` and
  `apps/desktop/src/lib/types.ts`.
* Validation: only on `Login` (`check_shape` rejects it on a secure note).
  `account` is trimmed; empty becomes `None`; at most 254 characters; no
  control characters.
* A login may have `sign_in_with` and a password at the same time. A
  login with only `sign_in_with` has no password, which is already allowed.
* Search in memory includes `account`.

## 4. Protocol

Rust `crates/havenkeys-protocol/src/message.rs` and TS
`packages/protocol/src/index.ts`. Both sides stay strict
(`deny_unknown_fields` / `hasExactKeys`), and the fuzz tests cover the new
shapes.

* `Match` gains `provider: SsoProvider | null`. For an SSO item,
  `username` carries `sign_in_with.account` when the item has no username
  of its own.
* `StartSso { itemId, url, topUrl? }` → `{ provider, account, providerOrigins, autoChoose }`.
  * Rust runs the same `authorize_for_page` as `fill_item`. Item not matching
    the page, missing or without `sign_in_with` → `Denied`. Vault locked →
    `Locked`.
  * `autoChoose = settings.auto_sign_in && item.auto_sign_in`.
  * No secret is returned.
* `CheckSso { url, topUrl?, provider, account? }` →
  `{ action: "add" | "update" | "unchanged", itemId }`, among the logins
  matching the page:
  * `unchanged`: one has this provider and this account (case-insensitive),
    or this provider and no account was learned.
  * `update`: none has this account, but exactly one has this provider and
    **no** account (typically imported from 1Password). The balloon offers
    to add the account to it.
  * `add`: otherwise.
* `SaveSso { url, topUrl?, provider, account?, itemId?, title? }` → `{ itemId }`.
  * Without `itemId`: same path as `stage_save_login`. One `Domain` rule for
    the frame's origin, title from the prompt or the host, no password
    required, `auto_sign_in` on.
  * With `itemId`: the login must match the page and already sign in with
    `provider`. Only its account changes, and `title` must be absent.

## 5. Detection and saving (extension)

### 5.1 Recognizing a provider button (`autofill/sso.ts`, new, pure)

Candidates: visible, enabled `button`, `a[href]`, `[role=button]`,
`input[type=submit|button]`. Signals, scored like `submit.ts`:

* Text, `aria-label`, `title` or inner `img[alt]` matching
  "Sign in / Log in / Continue / Sign up with <Provider>" and the pt-BR
  forms "Entrar / Continuar / Acessar com <Provider>": strong.
* A bare "<Provider>" label: counts only next to other provider buttons or
  a login form.
* `href` pointing at a provider origin (for example
  `accounts.google.com/o/oauth2/…`): adds points.
* Negatives: text that merely contains the name ("Google Drive",
  "GitHub repository", "Download on the App Store").

Scanning reuses the existing debounced MutationObserver roots. The whole
document is never rescanned on every mutation.

`sso.ts` returns, per provider, the best candidate and whether it is a clear
winner (threshold and margin as in `submit.ts`).

### 5.2 Recording the intent

A trusted (`isTrusted`) click on a recognized provider button sends
`cs_sso_click { provider }`. The background keeps, in memory only:

```text
PendingSso { tabId, url, topUrl, provider, account: null, expires: now + 5 min }
```

One per tab; a new click replaces it. Scripted clicks are ignored.

A site may embed the provider's own button frame (Google Identity Services
draws "Sign in with Google" in an `accounts.google.com/gsi/button` iframe,
personalized to "Continue as <name>" when signed in). A trusted click on
that frame's button counts as a click for that provider whatever its label,
and the background, checking the frame's URL from the browser, records the
embedding top page as `url`. That click already is the provider's page, so
it counts as reaching the provider: Chrome gives the popup Google then opens
no opener tab, so neither the popup nor its closing can be tied to the site.
The question is asked on the tab's next top-frame load (GSI posts the
credential to the site's `login_uri`). The email a personalized button
shows is kept as a suggestion, like a `login_hint`. Without FedCM (Firefox),
Google covers that iframe with a transparent overlay button in the site's
own page ("Sign in with Google. Opens in new tab"); a click on a provider
button whose wrapper holds the provider's button frame is sent with
`embedded: true` and counts the same way. A provider tab with no opener
whose URL is on the provider's origin and names the site in its `origin`
parameter (Google's OAuth popup: `…/o/oauth2/v2/auth?…&origin=<site>`) is
tied to that site's pending click, when exactly one matches, so the account
the user picks in it is suggested. Clicks on any other
page of a provider's own origins are ignored. (Offering and pressing such a button is out of reach:
it lives in a cross-origin frame.)

### 5.3 Learning the account (provider origins only)

While a tab (or a popup it opened, `openerTabId`) has a `PendingSso`, a
frame whose origin is in that provider's origins may report the account:

* A trusted click on an account-chooser row: the email-shaped text inside
  the clicked row becomes `account`.
* A username-only step submitted there (the existing `readSubmission`)
  also sets `account`.

This text comes from a web page. It is only a suggestion shown in the
balloon, the user can edit it, and Rust validates it on save.

### 5.4 Asking on return

Once the flow has reached one of the provider's origins (in the tab, or in
a popup it opened), either of these triggers `CheckSso` while `PendingSso`
is valid:

* the first later top-frame load in the tab on an origin that is **not**
  one of the provider's origins;
* the provider popup closing (many sites run OAuth in a popup and never
  reload the tab).

A load before the provider was reached (a site's own `/auth/google`
interstitial) does not count.

* `unchanged` → nothing, pending dropped.
* `update` → the same balloon asking "Add this account to your saved
  login?", with the account editable and no title field.
* `add` → the save balloon (the existing top-right surface, `saveBox`):

```text
Save login for typeform.com?
[G] Sign in with Google · [user@gmail.com]   (editable)
Title: [Typeform]                              (editable)
[Save]  [Not now]
```

The item is saved for the site **where the user clicked**, not the page
they returned to. The balloon names that site. **Save** → `SaveSso`.
**Not now**, closing it or expiry → pending dropped.

### 5.5 Unchanged behaviour

Provider buttons stay in `NEGATIVE_WORDS` for password auto sign-in.
Password save detection is unchanged.

## 6. Offering and running

### 6.1 The balloon

* In the top frame, when `sso.ts` finds visible provider buttons, the
  content script sends `cs_sso_buttons { providers }`. No page data.
* The background calls `find_matches` for the top frame URL and keeps
  matches whose `provider` is in `providers`. If none are left, nothing is
  shown.
* The balloon (top right) shows one row per item:
  `[icon] Sign in with Google · user@gmail.com`, plus a close button.
  Closing hides it for that tab until the next navigation.
* It follows the existing in-page suggestions preference: off → no balloon.
* SSO items also appear in the field menu and the popup with the provider
  icon. The popup's **Fill** reads **Sign in**. All three start the same
  run.

### 6.2 The run (`background/sso-run.ts`, new, pure state)

1. The user picks an SSO item. The background sends `StartSso`.
2. It creates, in memory, with no secrets:

   ```text
   SsoRun { tabId, siteOrigin, provider, account, providerOrigins,
            autoChoose, phase: "press" | "choose", expires: now + 2 min }
   ```

   One per tab; a new pick replaces it.
3. **Press.** The top frame presses that provider's button if `sso.ts`
   finds a clear winner (`element.click()`). Otherwise the run ends and the
   balloon shows "Couldn't find the Sign in with Google button".
4. **Choose.** A frame in the run's tab, or in a popup whose `openerTabId`
   is the run's tab, on an origin in `providerOrigins`, is told to choose
   the account if `autoChoose` is on and `account` is set. It clicks the
   chooser row only when **exactly one** row's email equals `account`
   (case-insensitive). The run then ends.

### 6.3 The run stops without acting when

* `account` is empty, missing from the chooser, or matches more than one
  row.
* The provider asks for a password. The normal field menu still offers the
  vault's login for the provider's origin, and the user picks it.
* A consent/permissions screen appears ("Allow", "Continue as …").
  HavenKeys never grants permissions on the user's behalf.
* Any trusted user input in the tab, a vault lock, 2 minutes pass, or the
  tab goes to an origin that is neither `siteOrigin` nor in
  `providerOrigins`.

## 7. Security

* Rust decides whether an item may be used on a page (`authorize_for_page`)
  and which origins a run may continue into. The extension cannot widen
  either.
* No password, TOTP secret or code is returned on this path.
* A malicious page can make the balloon appear only with items that match
  **its own** origin, the same exposure as the field menu today. It learns
  nothing unless the user picks.
* A malicious page can fake a provider button. Pressing it only navigates
  that page. The run continues only on the provider's real origins, checked
  against the frame's origin in the background, not the page's claims.
* The provider page is untrusted input: account text is validated,
  and a chooser row is clicked only on an exact, unique email match.
* Consent screens are never pressed.
* The run holds no secrets, lives in background memory, and ends on lock.
* The cross-origin step is bounded: provider origins only, one action,
  2 minutes, and only after an explicit pick. This is recorded as an
  amendment note on CLAUDE.md §25.

## 8. Desktop

* **Detail (`ItemDetail.tsx`):** a "Sign in with" block with the provider
  icon, name and account. If a vault login matches the provider's origin
  with the same username, clicking the block opens it. This is computed at
  display time.
* **Editor (`ItemEditor.tsx`):** a "Sign in with" section with a picker
  (None / Google / Microsoft / GitHub / Apple) and an account field. With a
  provider chosen, username and password collapse under "Also has a
  password".
* **List:** subtitle "Google · user@gmail.com".
* Provider icons are bundled local SVGs. Nothing is loaded from the
  network.

## 9. 1Password import (`import/onepux.rs`)

* `ssoLogin` with a known provider → `sign_in_with { provider, account }`.
  `account` is filled only if the export carries one. An unknown provider
  stays a notes line, as today.
* **Re-import upgrade.** When an imported item with `sign_in_with` hits the
  duplicate key (`dedupe_key_parts`: title + username + URLs) of an
  existing login **without** `sign_in_with`, the importer sets only
  `sign_in_with` on the existing item. If that item's notes are exactly the
  old `Sign in with <Provider>` line, the notes are cleared. Anything else
  stays untouched.
* The import report gains `sso_upgraded`, shown as "N logins updated with
  Sign in with".

## 10. Translations

All new strings in `en` and `pt-BR`, in the extension and the desktop, and
covered by the existing parity tests.

## 11. Testing

**Rust**
* An old vault without `sign_in_with` opens; the new field round-trips.
* `check_shape` rejects `sign_in_with` on a secure note; invalid account is
  rejected.
* `StartSso` attacks: page on `evil.com` or `typeform.com.evil.com` →
  `Denied`; locked → `Locked`; unknown item ID → `Denied`; item without
  `sign_in_with` → `Denied`. The result never contains a password.
* `SaveSso` works without a password; `CheckSso` → `unchanged` for the same
  provider and account.
* Importer: `ssoLogin` mapping, unknown provider to notes, re-import
  upgrade, notes cleared only when exactly the old line.
* Fuzz the new messages.

**Protocol (TS)**
* Strict parsers with `provider`; fuzz; provider-origin parity with Rust.

**Extension**
* `sso.ts` fixtures:
  * real buttons in English and pt-BR;
  * a bare "Google" label, alone and next to other providers;
  * OAuth links;
  * hidden and disabled buttons;
  * misleading text ("Google Drive");
  * thousands of buttons (performance).
* `SsoRun`:
  * TTL;
  * ends on user input;
  * an origin outside the list ends it;
  * a popup is accepted only with the run tab's `openerTabId`;
  * a chooser with 0, 1 or 2 matching rows;
  * a consent screen is never pressed;
  * `autoChoose` off → press only.
* Save:
  * an untrusted click is ignored;
  * the account is captured only on provider origins;
  * the balloon appears on return;
  * `unchanged` → no balloon.

## 12. Docs

* `docs/autofill.md`: detection, balloon, run, the GIS limitation.
* `docs/native-messaging.md`: `StartSso`, `CheckSso`, `SaveSso`,
  `Match.provider`.
* `docs/security-model.md` and `docs/threat-model.md`: the cross-origin
  run limited to Rust's list, no automatic consent, page-sourced account
  text treated as untrusted.
* `docs/security-review.md`: new findings, if any.
* CLAUDE.md §25: amendment note pointing to this spec.

## 13. Out of scope (v1)

* Google Identity Services buttons and One Tap. They are iframes from
  `accounts.google.com/gsi` embedded in the site, and v1 does not recognize
  or press them. Sites with their own provider button work.
* Corporate SSO (Okta, SAML, Azure AD tenant pages beyond the listed
  origins) and providers other than the four.
* Filling the provider's password or TOTP automatically.
