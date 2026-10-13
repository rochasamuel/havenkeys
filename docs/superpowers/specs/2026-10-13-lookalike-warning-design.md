# Look-alike site warning — design

Date: 2026-10-13. Status: approved in conversation.

Implements item **B4** of `docs/autofill-roadmap.md` (anti-phishing /
homograph detection).

## 1. Goal

When the page in front of the user imitates a site they have a login for
(`pаypal.com` with a Cyrillic "а", `g00gle.com`), HavenKeys says so instead
of staying silent. Today a look-alike simply has no matches, so the user
sees nothing unusual and may type the password by hand, or accept an offer
to save it for the fake site.

The check runs in the Rust core, offline, over the vault's own data while it
is unlocked. It only ever **adds a warning**: matching, filling and origin
binding are unchanged, and a look-alike still never matches or fills.

## 2. Decisions taken

| Question | Decision |
|---|---|
| What counts as a look-alike | Unicode confusables (UTS #39 skeleton) plus a small fixed digit map (`0→o`, `1→l`, `3→e`, `5→s`) |
| Not detected | Typos (edit distance), the same name on another suffix (`paypal.co`, `paypal.co.uk`), names that merely contain the brand (`paypal-login.com`) |
| Compared on | The whole registrable domain (eTLD+1), suffix included |
| Against | Registrable domains of every saved login's website rules |
| Library | `unicode-security` (unicode-rs), after `cargo deny` / `cargo audit` and a maintenance check (CLAUDE.md §48) |
| Transport | A new request `check_lookalike`; `find_matches` unchanged (older extensions reject unknown keys) |
| Surfaces | Extension: in-page menu, save prompt, popup. Android: autofill warning row for browser pages and the save sheet's description |
| Blocking | Never: "Save anyway" stays possible; nothing else is blocked |
| Desktop UI | None in this version |

## 3. Detection (`crates/havenkeys-core/src/lookalike.rs`)

### 3.1 Key

For a registrable domain `d` (ASCII, punycode as stored by `url`):

1. Decode to Unicode (`idna::domain_to_unicode`; already in the tree through
   `url`). A decoding error leaves `d` as is.
2. Replace each of `0 1 3 5` with `o l e s`.
3. Apply `unicode_security::skeleton` (UTS #39), which maps confusable
   characters to a prototype (Cyrillic `а` → `a`, `rn` → `m`, …).
4. Lowercase.

`key(d)` is the result. The digit map runs before the skeleton so that a
digit and the letter it imitates reach the same prototype.

### 3.2 Rule

`VaultService::lookalike_for_page(page_url, top_url) -> Result<Option<String>>`:

* `Locked` when the vault is locked.
* `None` when the page URL does not parse, or the page has no registrable
  domain under a known suffix (IP addresses, `localhost`, unknown suffixes).
* `None` when any login matches the page under the vault's current policy
  (direct or through an equivalent-domains group): the user is where their
  login says.
* Otherwise, for the frame page and then the top page (when given): if a
  saved site `S` (a registrable domain of any login's website rule, any
  match type) satisfies `S != page_site` and `key(S) == key(page_site)`,
  return `Some(S)`. With several, the first in sorted order (stable
  answers).
* Trashed items are not considered (they are not in the live overviews).

The returned string is the saved site's ASCII registrable domain. It is
shown to the user only by HavenKeys' own UI (extension frames, popup,
Android); the page never receives it.

### 3.3 What it never does

It never changes `find_matches`, fill/TOTP authorization, save checks or
passkeys. It sends nothing over the network. It caches nothing to disk.

## 4. Protocol

* Request: `{ "type": "check_lookalike", "url": string, "topUrl"?: string }`
  (same URL limits as `find_matches`).
* Response: `{ "type": "check_lookalike", "lookalikeOf": string | null }`;
  `lookalikeOf` is at most 253 characters, ASCII.
* The bridge rate-limits it in the same class as `find_matches`, refuses it
  while locked (`locked`) and when browser integration is off, like every
  page request. A frozen account still gets the answer (it is a warning,
  not a fill).
* Version skew: an older extension never sends it; a newer extension
  talking to an older desktop receives an error and treats it as "no
  warning".

## 5. Extension

The extension asks `check_lookalike` only when `find_matches` returned no
logins for the frame, so ordinary pages cost no extra request.

* **In-page menu (login fields):** on a look-alike page the menu opens on
  the user's focus/click as today, with one warning row and no fill rows:
  "Not paypal.com — this site only looks like it." The row fills nothing.
* **Save prompt:** on a look-alike page the card leads with a warning —
  "This isn't paypal.com. Don't save or enter your paypal.com password
  here." — and keeps "Save anyway" as a secondary action.
* **Popup:** the "No saved logins for this site" notice is replaced by the
  same warning.
* The saved site is rendered with `textContent` only.
* Copy in `en` and `pt-BR`; final wording from the impeccable review.

## 6. Android

Browser pages only. Apps are identified by package and signing certificate,
not a domain, so they cannot be look-alikes in this sense.

* **Mobile API:** `MobileVault::lookalike_for(target) -> Option<String>`
  (UniFFI), `None` for app targets; same rules as §3.2.
* **Autofill:** when a browser page's login form has no matches and the page
  is a look-alike, the fill response carries one warning row (menu and
  inline chip) — "Not paypal.com — this site only looks like it" — that
  fills nothing; tapping it opens HavenKeys on a short explanation screen
  (the existing authentication-activity pattern). The "Search HavenKeys" row
  stays available.
* **Save:** the response's `SaveInfo` gets a description with the same
  warning (`SaveInfo.Builder.setDescription`), shown in the system's save
  sheet. The system's Save button remains "save anyway".
* Copy in `values` and `values-pt-rBR`; impeccable review of the row and the
  screen.

## 7. Security review entry

New `security-review.md` section "Look-alike warning":

* **LA1 (Info):** the warning reveals to HavenKeys' own UI which saved site a
  look-alike imitates; the page cannot read extension frames, the popup or
  Android's autofill UI. Same exposure class as `find_matches` titles.
* **LA2 (Low, accepted):** coverage is partial by design — typos, other
  suffixes and brand-containing names are not detected; absence of a
  warning is not a guarantee. Documented in `autofill.md` and the threat
  model.
* **LA3 (Info):** a false alarm cannot block the user ("Save anyway", typing
  by hand); a look-alike never gains a fill.

## 8. Tests

Core:

* Warns: `pаypal.com` (Cyrillic а, as punycode — compute it with `url`, do not hand-write it) vs saved `paypal.com`;
  `g00gle.com` vs `google.com`; `rnicrosoft.com` vs `microsoft.com`;
  `paypa1.com` vs `paypal.com`.
* Does not warn: `paypal.co.uk` vs `paypal.com`; `paypal.com` itself;
  `icloud.com` with a saved `apple.com` login (equivalent group, setting
  on); an unrelated site; IP and `localhost` pages; a page that some login
  matches directly even if it also resembles another saved site.
* A frame on a normal site inside a look-alike top page warns.
* Locked → `Locked`. Trashed logins are ignored.
* Fuzz: random Unicode hosts never panic; the key is deterministic.

Bridge: request/response round trip, rate limit, locked, integration off.

Protocol (TS + Rust): parse/serialize, oversized `lookalikeOf` rejected,
unknown keys rejected.

Extension: menu shows the warning row with no fill rows; save card shows the
warning and "Save anyway" still saves; popup notice; nothing asked when
matches exist; an error from an older desktop shows nothing.

Android: Rust `lookalike_for` for browser and app targets; Kotlin unit tests
for the warning row and the `SaveInfo` description; detekt.

## 9. Docs

`docs/autofill.md` (what is and is not detected), `docs/threat-model.md`
(phishing section), `docs/native-messaging.md` (new request),
`docs/security-review.md` (§7). The roadmap marks B4 done (uncommitted).
No `CLAUDE.md` amendment: nothing is relaxed.
