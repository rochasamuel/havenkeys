# Filling forms from the Identity — Design

Status: proposed, 2026-09-29.
Builds on `2026-09-29-identity-item-design.md` (sub-project 1, the Identity
item). Amends CLAUDE.md §25 (a note, like the earlier ones) and the
extension's trust model in `docs/security-model.md`.

> This software has not undergone an independent security audit.

## 1. Goal

Sign-up and checkout forms ask for the same things every time: name, email,
phone, address, CEP, CPF, birth date. The browser extension fills them from
the account's Identity after one click in the HavenKeys menu, like
1Password's identity fill.

## 2. The trust decision

Every fill HavenKeys does today is **bound to a site**: Rust releases a login
only to a page its saved websites match. An identity has no website. Any
page, a phishing page included, can show a form that asks for it, and
HavenKeys will offer to fill it there.

That is accepted, because an identity is for filling forms on sites the user
has not seen before. The protection comes from these rules instead (§5, §6):

1. Nothing is filled without the user's click in the extension's own menu
   (an iframe the page cannot read or click) or popup.
2. Only visible, editable fields of the form the user clicked in are filled,
   re-checked at the moment of writing; hidden fields never are.
3. Only the values those fields ask for leave Rust.
4. **Documents** (CPF, RG, passport, CNH) need a second, explicit click
   that names them, and only on https pages.
5. A cross-site iframe gets nothing.
6. Existing values are never overwritten.
7. Identity fills never submit a form.

What this does not protect against: a user who clicks "Fill" on a phishing
page gives that page the non-document values the form shows fields for, and
the documents too if they confirm. The confirmation step names the documents
and the page's site to make that choice deliberate.

## 3. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Which values | All; documents after a second click naming them | Everything in one click; documents never filled |
| Where offered | The in-page menu on identity fields; the sign-up email/username menu below logins; a popup button | Identity fields only; popup only |
| Scope of one fill | The form (group) holding the clicked field | The whole page; the one field |
| Existing values | Never overwritten, unless HavenKeys filled them | Always overwritten (1Password) |
| Documents on http | Never | Allowed after confirmation |
| Iframes | Same-site as the top page only | Any frame; top frame only |
| Submit | Never | Under the automatic sign-in rules |
| Derived values (full name, address line 1, date parts) | Composed in Rust | Composed in the extension (would need more values sent) |

## 4. Roles

A field classified as identity data gets one **role**:

| Group | Roles |
|---|---|
| Name | `fullName`, `firstName`, `middleName`, `lastName` |
| Contact | `email`, `phone` |
| Personal | `birthDate`, `birthDay`, `birthMonth`, `birthYear`, `company` |
| Address | `street`, `number`, `complement`, `addressLine1`, `addressLine2`, `neighborhood`, `city`, `state`, `postalCode`, `country` |
| Internet | `username` |
| Documents | `cpf`, `rg`, `passport`, `driversLicense` |

Values, all computed in Rust from the Identity (`IdentityFields`):

* `fullName`: first, middle and last joined (`display_name`).
* `phone`: mobile, else home, else work.
* `birthDate`: `YYYY-MM-DD`; `birthDay`/`birthMonth`/`birthYear`: its parts,
  unpadded day and month (`"20"`, `"4"`, `"2000"`).
* `addressLine1`: `street, number` when both, else whichever exists.
* `addressLine2`: `complement`.
* Every other role: the field of the same name.

A role with no value is omitted from the answer.

## 5. Extension

### 5.1 Classification (`autofill/identity.ts`, new)

`identityRole(field, env) → { role, confidence } | null`, scored like the
login classifier:

* **autocomplete** (strongest, after stripping `section-*`, `shipping`,
  `billing`, `home`, `work`, `mobile` prefixes): `name` → fullName,
  `given-name`, `additional-name`, `family-name`, `email`, `tel`/`tel-national`
  → phone, `bday`, `bday-day`, `bday-month`, `bday-year`, `organization` →
  company, `street-address` → addressLine1 (a textarea: street, number,
  complement joined by line breaks in the fill, §5.4), `address-line1`,
  `address-line2`, `address-level2` → city, `address-level1` → state,
  `address-level3` → neighborhood, `postal-code`, `country`, `country-name`,
  `username`;
* **input type**: `email` → email, `tel` → phone, `date` with a birth word →
  birthDate;
* **words** in name, id, label, placeholder and aria-label, English and
  Portuguese, accent-insensitive: nome/first name, sobrenome/last name, nome
  completo/full name, e-mail, celular/telefone/phone, nascimento/birth,
  empresa/company, logradouro/rua/endereço/street/address, número/number,
  complemento/apt/suite, bairro/neighborhood, cidade/city, estado/UF/state,
  CEP/zip/postal, país/country, CPF, RG/identidade, passaporte/passport,
  CNH/driver.

Never an identity field: password, one-time-code and card fields
(`isCardField`), search boxes (`type=search`, role=search), hidden, disabled
or read-only fields, and fields inside a login group whose intent is `login`.

A field needs confidence ≥ 0.5 to count. A **form qualifies** for the
identity menu when it has at least two identity fields, or one when the
field's autocomplete names the role (so a lone `autocomplete=postal-code`
still works).

The login classifier is unchanged: the words it treats as "not a username"
stay negatives there.

### 5.2 Menu

* New `MenuKind` `"identity"`. `menuKindFor` returns it for a field with an
  identity role in a qualifying form, when the field is not a login field.
  Menus open on the same trusted events as today and respect the in-page
  menu preference.
* On a **sign-up** group (`groupIntent` = `signup`), the email/username
  field's `login` menu also shows the identity row, below any logins.
* The row: the ID-card icon, the identity's name (or "Identity"), and
  "Fills N fields" (N = identity fields in the group with a value to fill).
* **Documents**: when the group has document fields, clicking the row does
  not fill. The menu shows "*shop.com* also asks for: CPF, RG" with **Fill
  CPF and RG too** and **Fill without documents**. On an http page the
  question is not asked; documents are left empty and the menu says so.
* No identity, or an empty one: the row says "Your identity is empty — fill
  it in HavenKeys"; clicking it sends `open_identity`, which opens the
  identity in the desktop app. (`open_item` cannot be reused: it only opens
  a login saved for the page.)

### 5.3 Messages

The content script sends the roles of the group's fields with the open
request; the background keeps them in the menu session (as it keeps offered
items) and sends `fill_identity` with exactly those roles when the user
picks. The fill result goes back to that frame by `documentId`, as login
fills do, carrying `{field index → value}` for the session's fields.

### 5.4 Writing

* Only the group of the clicked field; for the popup, the first group in
  the top frame with ≥ 2 identity fields.
* Only **empty** fields, or fields whose current value HavenKeys wrote (the
  existing provenance map). A value the user typed or the site filled is
  kept.
* Each field is re-checked with `isFillable` just before writing; a field
  hidden since the menu opened is skipped.
* `<input>`: the native value setter, then `input` and `change` events, as
  `setValue` does. `type=date` accepts `birthDate` only. A `maxlength`
  shorter than the phone value retries without a leading `+55 ` / `+55`.
* `<select>` (new): the option whose value or text equals the value,
  ignoring case, accents and surrounding space; for `state` also the
  Brazilian UF ↔ state-name table (27 entries) both ways; for `country` also
  `BR` ↔ Brasil/Brazil. No match: skipped. Selection by setting
  `selectedIndex`, then `input`/`change`.
* `<textarea>` (new, `street-address` only): street, number and complement
  on separate lines.
* Values are never written to attributes and never logged; the content
  script drops the message after writing.

### 5.5 Popup

A **Fill identity** button under the logins, shown when the vault is
unlocked and the identity has a value. It injects into the top frame like
the popup's login fill, then follows §5.2's document step in the popup.

## 6. Protocol and Rust

### 6.1 Requests

```text
find_identity { url, topUrl? }
  → identity { available: bool, title: string, email: string | null }

fill_identity { url, topUrl?, roles: [Role, …], documents: bool }
  → identity_values { values: [ { role, value } ] }

open_identity { url, topUrl? }
  → opened {}          (the desktop shows the identity, as open_item does)
```

* `roles`: 1–40 entries, no duplicates, each a known role; anything else is
  a malformed message.
* `title` ≤ 1024 bytes, `email` ≤ 2048 bytes, each value ≤ 4096 bytes; the
  response stays under the 256 KiB limit.
* Rate limit class: `find_identity` is a *lookup*; `fill_identity` and
  `open_identity` are *secret* requests, like `open_item` (server.rs
  mapping).

All three added to `crates/havenkeys-protocol` (`Request`, `ResultBody`,
`kind()`, `urls()`, size checks), `packages/protocol` (types and parser),
the bridge dispatch, and `docs/native-messaging.md`.

### 6.2 What Rust checks (`VaultService::identity_for_page`, new)

1. Unlocked; browser integration on (`require_enabled`).
2. The page URL is http(s) (`PageUrl::parse`); in a frame, `topUrl` is too
   and `site_of(page) == site_of(top)`, else `Denied`.
3. The identity (derived ID) exists, else `NotFound`.
4. Document roles are answered only when `documents` is true **and** the
   page is https; otherwise they are silently left out (the extension shows
   them as not filled).
5. Only the requested roles, per §4.

`find_identity` runs steps 1–3 and returns the title and email only;
`open_identity` runs steps 1–2 and opens the identity if it exists.

## 7. Error handling

| Case | Behaviour |
|---|---|
| Vault locked | Menu shows the usual "unlock HavenKeys" row |
| Integration off | No identity row (as logins) |
| Identity missing or empty | Row opens it in the desktop app |
| Cross-site frame | `Denied`; no identity row |
| Rate limited | The usual rate-limit message |
| A select with no matching option | That field skipped |
| Everything already filled | "Nothing to fill" in the menu, no request |

## 8. Testing

Rust:

* protocol: the three requests parse; unknown roles, duplicates, 0 or 41 roles
  and oversize values are rejected; responses validated;
* core `identity_for_page`: locked, missing, cross-site frame, same-site
  frame, http without documents, https with and without `documents`, only
  requested roles, derived values (full name, address line 1, date parts,
  phone fallback);
* bridge: integration off, rate-limit class of each request;
* security regressions: `evil.com` embedding a `shop.com` frame gets nothing;
  1000 roles rejected; documents never on http.

Extension (vitest, jsdom, inline fixtures):

* classification: a Brazilian checkout (nome, CPF, celular, CEP, logradouro,
  número, complemento, bairro, cidade, UF select), a US shipping form with
  `autocomplete`, a sign-up form (identity row under the email menu), a
  login form (no identity menu), a search box and a newsletter-only email
  (no identity menu);
* writing: empty-only, provenance overwrite, hidden-since-open skipped,
  select matching (UF code and name, accents), date input, textarea,
  phone `maxlength` retry, React value tracker;
* menu: the document step, http notice, empty identity row;
* protocol TS parsing mirrors the Rust cases.

`ui:check`: the identity menu, the document step, the popup button, in both
languages and themes.

## 9. Documentation

* `docs/autofill.md`: identity signals, qualifying forms, writing rules.
* `docs/native-messaging.md`: the three requests.
* `docs/security-model.md`: §2 of this spec, the not-site-bound decision and
  its mitigations.
* CLAUDE.md §25: an amendment note pointing here.
* `README.md`: one line in the extension's feature list.
