# Filling checkouts from a Card — Design

Status: draft, 2026-09-29.
Builds on `2026-09-29-card-item-design.md` (sub-project 1, the Card item).
Amends CLAUDE.md §25 (a note, like the earlier ones) and the extension's
trust model in `docs/security-model.md`, which today says card fields are
refused.

> This software has not undergone an independent security audit.

## 1. Goal

Checkout forms ask for the same card every time. The browser extension
fills them from a saved Card after one click in the HavenKeys menu or popup,
across the field layouts sites use (one expiry field or two, split number
inputs, selects) and across the payment processors' iframes. When the user
types a new card into a checkout, HavenKeys offers to save it.

## 2. The trust decision

A card, like the Identity, is **not bound to a site**: it is offered on any
https checkout, because a card is for paying sites the user has not saved.
The protection comes from these rules instead:

1. Nothing is filled without the user's click in the extension's own menu
   (an iframe the page cannot read or click) or popup.
2. Only visible, editable card fields of the checkout the user clicked in
   are filled, re-checked at the moment of writing; hidden fields never
   are.
3. The top page is **https**. Each frame filled is https **and** is
   same-site as the top page or has an origin on a fixed list of payment
   processors in Rust.
4. Only the values the form's fields ask for leave Rust.
5. Existing values are never overwritten, unless HavenKeys wrote them.
6. A card fill never submits a form.
7. Nothing is saved without the user's click on the save prompt.

What this does not protect against: a user who picks a card on a phishing
checkout gives that page the card, CVV included. The menu row names the
page's site ("Fill on *shop.com*") to make that pick deliberate.

## 3. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| What one click fills | The whole card, CVV included | CVV on a second click; a confirmation in the desktop app |
| http pages | Never; the menu says why | Allowed |
| Cross-site iframes | Same-site, or a fixed list of payment-processor origins | Any frame; same-site only (Stripe checkouts would not fill) |
| Frames per click | All the tab's eligible card frames, in one request | One request per frame (more secret requests, more rate-limit use) |
| Field layouts | Rust returns whole values; the extension shapes them to each field | Rust composes every format (would need the field's shape in the request) |
| Existing values | Never overwritten, unless HavenKeys filled them | Always overwritten |
| Submit | Never | Under the automatic sign-in rules |
| Save from checkout | Asked on submit, in the top frame or same-site frames | No save prompt; also from processor frames |

## 4. Roles

| Role | Value from Rust |
|---|---|
| `cardholderName` | the cardholder name |
| `cardholderGivenName` | its first word |
| `cardholderFamilyName` | the rest, after the first word |
| `number` | the digits |
| `verificationNumber` | the digits |
| `expiryMonth` | `"1"`–`"12"`, unpadded |
| `expiryYear` | four digits |
| `brand` | the brand id (`visa`, `mastercard`, …) |

A role with no value is omitted. The extension builds every field format
(§5.4) from these values; it adds no data.

## 5. Extension

### 5.1 Classification (`autofill/card.ts`, new)

`cardRole(field, env) → { role, shape, confidence } | null`, scored like the
login classifier. `shape` is the field's layout: `whole`, `expiry`
(month and year in one field), `slice(n, of)` (a split number) or `select`.

* **autocomplete** (strongest, after stripping `section-*`, `billing`,
  `shipping`): `cc-name`, `cc-given-name`, `cc-family-name`, `cc-number`,
  `cc-csc`, `cc-exp` → expiry, `cc-exp-month`, `cc-exp-year`, `cc-type` →
  brand.
* **words** in name, id, label, placeholder and aria-label, English and
  Portuguese, accent-insensitive: card number/número do cartão/numero,
  cvv/cvc/csc/security code/código de segurança, expiry/expiration/
  validade/vencimento, month/mês, year/ano, name on card/nome impresso/
  titular/cardholder, card type/bandeira.
* **shape**: `inputmode=numeric` or `type=tel` with `maxlength` 16–23 and a
  number word → number; `maxlength` 3–4 near a number or expiry field with a
  code word → CVV; a select with 12 month options → month; a select with ≥
  10 consecutive year options → year.
* **Split number**: 3–5 adjacent text inputs with `maxlength` 4–6 (sum
  13–19) under one number label or `cc-number` on the first → one number,
  `slice(i, n)` each. An Amex 4-6-5 split is recognised by its lengths.

Never a card field: password, one-time-code and search fields; hidden,
disabled or read-only fields (the identity's stricter visibility applies).

A field needs confidence ≥ 0.5. A **card group** is the fields of one form
(the existing grouping); it **qualifies** when it has a number field, or a
CVV and an expiry field. Across frames, the **tab's card fields** are the
qualifying group in the clicked frame plus the card fields of every other
eligible frame (§6.2 step 3), so Stripe's three one-field iframes count
together.

The login and identity classifiers keep refusing card fields
(`isCardField`), so a card field only ever gets the card menu.

### 5.2 Menu

* New `MenuKind` `"card"` on a card field, on the same trusted events as
  today and respecting the in-page menu preference.
* One row per card from `find_cards`: the brand logo, the title,
  `•••• 7609 · 11/33`, and "Fill on *shop.com*" (the top page's site).
  Expired cards last, dimmed, still selectable.
* http top page: "HavenKeys fills cards only on secure (https) pages", no
  rows.
* Locked: the usual unlock row. No cards: "No cards saved — add one in
  HavenKeys", which opens the desktop app.
* In a cross-site frame not on the processor list: no menu.

### 5.3 Messages

On a pick the background:

1. asks every frame of the tab for its card fields' roles
   (`bg_card_roles`, as `bg_identity_roles`), with a 300 ms timeout per
   frame;
2. keeps the clicked frame and the frames that report card fields, up to 8,
   clicked frame first;
3. sends one `fill_card` with each frame's URL and roles;
4. delivers each frame's values to that frame by `documentId`, as login
   fills do. The content script re-classifies its fields and writes.

A frame whose document changed since step 1 drops its values.

### 5.4 Writing (`autofill/card-fill.ts`, new)

* Only **empty** fields or fields HavenKeys wrote (the provenance map).
* Each field re-checked with the identity's fillable check just before
  writing.
* `<input>`: the native value setter, then `input` and `change` events
  (the React value tracker is handled as in `identity-fill.ts`).
* **Number**: digits only; with spaces when the placeholder shows grouped
  digits (`0000 0000 …`); a `slice` gets its part.
* **Expiry in one field**: the format from the placeholder or `pattern`
  (`MM/YY`, `MM/YYYY`, `MM / YY`, `MM-YY`, `MMYY`, and `AA` for the year in
  Portuguese placeholders); else by `maxlength` (4 → `MMYY`, 5 → `MM/YY`,
  7 → `MM/YYYY`); else `MM/YY`. The month is always padded here.
* **Month**: text gets `11`, or padded (`03`) when `maxlength` is 2 or the
  placeholder shows `MM`. A select matches option value or text: `3`,
  `03`, the English and Portuguese month name or its three-letter form.
* **Year**: 2 or 4 digits by `maxlength`/placeholder; a select matches
  either.
* **Brand select**: the option matching the brand id, its display name or a
  common code (`VI`, `MC`, `AMEX`, `AX`, `ELO`, `HC`, `DC`, `DI`, `JCB`,
  `CUP`), ignoring case and accents.
* **CVV**: only when its length fits `maxlength`; otherwise left empty.
* **Names**: whole, or given and family name.
* A select with no matching option is skipped.
* Values are never written to attributes and never logged; the content
  script drops the message after writing.

### 5.5 Popup

A **Fill card** button under the logins when the top page is https and the
tab has card fields. It lists the cards (logo, title, last 4, expiry) and,
on a pick, runs §5.3 with the top frame as the clicked frame.

### 5.6 Save prompt

On a form submit (the existing submit detection), in the top frame or a
same-site frame:

* the group has a number that passes the check digit, and an expiry;
* the background compares its last 4 and expiry with `find_cards`; a match
  shows nothing (the number is not sent to Rust before the user confirms);
* the site is not excluded from save prompts.

The prompt reuses the save-login frame: "Save card to HavenKeys?", the
brand logo, `•••• 7609`, the expiry, and a title field (default: the brand
name). The logo is display only: the background picks it from the
number's prefix with a copy of Rust's IIN table in `packages/protocol`,
kept equal by a parity test (as the role lists are). What is saved uses
Rust's detection. **Save** sends `save_card`;
**Not now** drops it. The pending card is held in the background's memory
only, like a pending login, and dropped on dismiss, navigation, lock or
2 minutes.

Cards typed into processor iframes are not offered for saving: the submit
happens in another frame. This is a known limitation.

## 6. Protocol and Rust

### 6.1 Requests

```text
find_cards { url, topUrl? }
  → find_cards { insecure: bool, cards: [{ id, title, brand, last4, expiry }] }

fill_card { itemId, topUrl, frames: [{ url, roles: [Role, …] }] }
  → fill_card { frames: [{ values: [{ role, value }] }] }

save_card { url, topUrl?, title?, cardholderName?, number,
            verificationNumber?, expiry? }
  → save_card { itemId }
```

* `find_cards`: at most 50 cards, overview data only. `insecure` is true
  and `cards` empty when the top page is not https.
* `fill_card`: 1–8 frames; the answer has one entry per frame, same order.
  Each frame: 1–8 roles, no duplicates, each known. `value` ≤ 512 bytes.
* `save_card`: fields validated as the Card item's (§4.2 of the item spec);
  the number must pass the check digit; `title` ≤ the protocol's title
  limit.
* Rate-limit class: `find_cards` is a *lookup*; `fill_card` and
  `save_card` are *secret* requests.
* `Debug` of every card request and result prints no values.

Added to `crates/havenkeys-protocol` (`Request`, `ResultBody`, `kind()`,
`urls()` — every frame URL — size checks), `packages/protocol` (types,
parser, a role-parity test), the bridge dispatch, and
`docs/native-messaging.md`.

### 6.2 What Rust checks (`VaultService::card_for_page`, new `card_page.rs`)

1. Unlocked; browser integration on (`require_enabled`).
2. `topUrl` parses (`PageUrl::parse`) and is **https**; each frame URL is
   https.
3. Each frame is `site_of(frame) == site_of(top)`, or its exact origin is
   in `PAYMENT_FRAME_ORIGINS`. Any other frame → `Denied` for the whole
   request.
4. `itemId` names a Card; otherwise `NotFound` (the same answer for a
   missing item and another type).
5. Only the requested roles, per §4.

`find_cards` runs steps 1–3 on the one URL it has (with `topUrl` when in a
frame). `save_card` runs 1–3 with the frame required to be same-site (not a
processor), then stages a create through the normal write path; offline it
fails with the existing "needs the server" error and the prompt says so.

### 6.3 `PAYMENT_FRAME_ORIGINS`

A fixed list in Rust next to the sign-in provider list: exact `https`
origins of the iframes that payment processors serve card fields from.
Candidates: Stripe (`https://js.stripe.com`), Adyen's live checkout-shopper
origins per region, Braintree hosted fields, Mercado Pago secure fields,
Pagar.me, PagSeguro, Cielo. The plan confirms each origin from the
processor's own documentation and leaves out any it cannot confirm. No test
or sandbox origins. Adding an origin is a code change with a test.

## 7. Error handling

| Case | Behaviour |
|---|---|
| Vault locked | Menu shows the usual unlock row |
| Integration off | No card menu (as logins) |
| http top page | Menu explains; no rows; `fill_card` denied |
| Frame not same-site and not a processor | No menu there; `Denied` if requested |
| A frame times out collecting roles | Left out of the fill |
| Card deleted since the menu opened | `NotFound`; "This card is no longer in HavenKeys" |
| Rate limited | The usual rate-limit message |
| CVV longer than the field allows | CVV left empty |
| Select with no matching option | Skipped |
| Everything already filled | "Nothing to fill", no request |
| Save offline | Prompt shows the needs-the-server message |

## 8. Testing

Rust:

* protocol: the three requests parse; 0 or 9 frames, 0 or 9 roles,
  duplicates, unknown roles, oversize values and an invalid number in
  `save_card` are rejected; `Debug` redacts;
* core `card_for_page`: locked, http top, http frame, same-site frame,
  processor frame, other cross-site frame (whole request denied), non-card
  ID and missing ID give the same `NotFound`, only requested roles, name
  split, unpadded month;
* bridge: integration off, rate-limit class of each request;
* security regressions: `evil.com` framing `shop.com` gets nothing;
  `http://shop.com` gets nothing; a frame claiming `js.stripe.com.evil.com`
  is denied; `find_matches` still never returns cards.

Extension (vitest, jsdom, inline fixtures):

* classification: a Brazilian checkout (número do cartão, nome impresso,
  validade `MM/AA`, CVV), a US checkout with `autocomplete`, a Stripe-like
  set of three single-field frames, a 4-input split number, an Amex
  4-6-5 split, month and year selects; login and identity classifiers
  never claim these fields;
* writing: every expiry format, padded and unpadded months, month names in
  both languages, 2- and 4-digit years, brand select codes, CVV
  `maxlength`, empty-only and provenance overwrite, hidden-since-open
  skipped, the React value tracker;
* multi-frame pick: frames collected, a timed-out frame left out, values
  routed by `documentId`;
* save prompt: shown for a new card, not for a known last 4 + expiry, not
  for a failing check digit, not on excluded sites, dropped after 2
  minutes;
* protocol TS parsing mirrors the Rust cases.

`ui:check`: the card menu, the http notice, the popup picker and the save
prompt, in both languages and themes.

## 9. Documentation

* `docs/autofill.md`: card signals, qualifying groups, frames, writing
  rules.
* `docs/native-messaging.md`: the three requests.
* `docs/security-model.md`: §2 of this spec; replaces "card fields are
  refused"; the processor list and why.
* CLAUDE.md §25: an amendment note pointing here.
* `README.md`: one line in the extension's feature list.

## 10. Revisions from planning

Found while writing `docs/superpowers/plans/2026-09-29-card-autofill.md`;
they take precedence over the sections above.

1. `fill_card` values are ≤ 1024 bytes (a 256-character accented
   cardholder name exceeds 512).
2. Stripe also serves card frames from subdomains of `js.stripe.com`; the
   list has a label-safe "subdomains of" entry for it.
3. A menu opened inside a processor iframe is drawn by the top frame over
   that iframe (`bg_host_menu`, located with `runtime.getFrameId`), falling
   back to the frame itself; a 40-px iframe would clip it.
4. Before `fill_card`, the background runs a `find_cards` lookup for each
   extra frame and drops the ones Rust would deny, so an unrelated frame
   with card-like inputs does not deny the whole fill.
5. "No cards saved" is informational: no request opens the desktop app
   without an item.
6. There is no save-prompt exclusion list; that condition is dropped.
7. A pending card survives navigation for its 2 minutes (checkouts navigate
   on submit), as pending logins do.
8. The save prompt is offered for cards typed in the top frame only (the
   background cannot tell same-site frames apart without the public-suffix
   list).
9. "Fill on *site*" is shown once, in the menu header.
10. The popup lists the cards, each with its own Fill button.
11. The typed-number check compares digits only, so masked/reformatting
    number fields still get the save prompt.

Confirmed processor origins: Stripe (`https://js.stripe.com` and
subdomains), Adyen live (`checkoutshopper-live`, `-live-us`, `-live-au`,
`-live-apse`, `-live-in`, `-live-nea` `.adyen.com`), Braintree
(`https://assets.braintreegateway.com`), Mercado Pago
(`https://secure-fields.mercadopago.com`,
`https://api-static.mercadopago.com`). Pagar.me, PagSeguro and Cielo are
left out: their documented integrations keep card inputs on the merchant's
page, which the same-site rule covers.
