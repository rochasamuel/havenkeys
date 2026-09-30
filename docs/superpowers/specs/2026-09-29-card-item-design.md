# Card item — Design

Status: draft, 2026-09-29.

> This software has not undergone an independent security audit.

## 1. Goal

1Password keeps a **Credit Card** item: cardholder name, type, number,
verification number and expiry date, shown with the card network's logo.
HavenKeys gets the same item, for as many cards as the user has.

This is sub-project 1 of 2:

1. **This spec:** the Card item type in the vault (core, storage, sync,
   import) and in the desktop app (list, detail, editor, copy), with the
   card networks' logos.
2. **Next spec:** `2026-09-29-card-autofill-design.md`, filling checkout
   forms from a card and saving a card typed into a checkout.

Nothing in this spec lets the extension read a card.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| How many cards | Any number; **New → Card** | One per account (as the Identity) |
| Fields | Cardholder name, type, number, verification number, expiry, notes (the fields in 1Password's card view) | Plus valid-from, PIN, bank and custom fields; 1Password's full set |
| Type (brand) | Detected in Rust from the number; the user can override it | Picked by hand only |
| Brands with a logo | Visa, Mastercard, American Express, Elo, Hipercard, Diners Club, Discover, JCB, UnionPay, Maestro; others show a generic card | Only the international networks |
| Invalid check digit | A warning in the editor; saved anyway | Rejected (some real cards fail it) |
| Verification number | Stored, encrypted, masked like a password | Never stored |
| Number and CVV in the editor | Keep/replace, like a login password; never sent to open the editor | Revealed into the editor |
| 1Password import | Credit cards become Cards; fields without a place go to notes | Kept as secure notes |

## 3. What does not change

* The encrypted blob format, AAD binding and key hierarchy (`docs/crypto.md`).
* The server: it stores item blobs without knowing their type.
* Sync, the write path and conflicts.
* Native messaging and the extension. `find_matches`, `fill_for_page`,
  `authorize_for_page`, `get_totp` and the identity requests keep denying
  or skipping anything that is not their type. The card requests come in
  the next spec.

## 4. Data model (`havenkeys-core`)

### 4.1 Type

`ItemType` gains `Card` (`"card"` on the wire). `ItemDetails` gains:

```rust
Card(Box<CardFields>)
```

The fields, their validation and brand detection live in a new
`src/card.rs`, as the identity's live in `src/identity.rs`.

### 4.2 Fields

Wire names are camelCase; `deny_unknown_fields`.

| Field | Type | Rule |
|---|---|---|
| `cardholderName` | `Option<SecretString>` | ≤ 256 chars, no control characters |
| `brand` | `Option<CardBrand>` | `visa`, `mastercard`, `amex`, `elo`, `hipercard`, `diners`, `discover`, `jcb`, `unionpay`, `maestro`, `other` |
| `number` | `Option<SecretString>` | Spaces and `-` removed on save; then 8–19 ASCII digits |
| `verificationNumber` | `Option<SecretString>` | 3–8 ASCII digits |
| `expiry` | `Option<CardExpiry>` | month 1–12 and a 4-digit year 1970–2099, stored as `"YYYY-MM"` |
| `notes` | `Option<SecretString>` | 64 KiB, as login notes; newlines and tabs allowed |

Whitespace-only values are stored as absent. A card needs at least a
number or a title; an empty card is rejected.

`Debug` of `CardFields` prints no values. `SecretString` wipes on drop.

### 4.3 Brand

`card::detect_brand(digits: &str) -> Option<CardBrand>` uses issuer
prefix (IIN) ranges. Elo and Hipercard are checked first, because some of
their ranges sit inside the ranges that Visa, Mastercard and Discover
cover. The table of ranges is data in `card.rs`, with its source cited in a
comment, and each range has a test.

`brand` stores only the user's choice. When it is absent, the brand shown
and filled (`CardFields::effective_brand()`) is `detect_brand(number)`, else
`other`; it is computed when the summary is built, so "Detect from number"
keeps working after later edits.

`card::check_digit_ok(digits) -> bool` is the Luhn check. It is only
reported, never enforced.

### 4.4 Overview and search

`ItemOverview` gains:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub card: Option<CardSummary>,   // { brand, last4, expiry }
```

* `title`: the user's title, or the brand's display name ("Mastercard")
  when empty, or "Card" when there is no brand.
* `card`: brand, the number's last four digits (absent when the number is
  shorter than 12 digits), and the expiry. Nothing else of the number.
* `username`: none. `has_notes` as usual; the other `has_*` flags false.

Search (`vault.rs::search`) also matches `card.last4` when the query is
digits. The full number, the CVV and the cardholder name are only in the
details blob, decrypted when the item is opened.

### 4.5 Input

`ItemInput` gains `card: Option<CardInput>`:

```rust
struct CardInput {
    cardholder_name: Option<SecretString>,
    brand: Option<CardBrand>,          // None = detect
    number: SecretUpdate,              // Keep | Set | Clear
    verification_number: SecretUpdate,
    expiry: Option<CardExpiry>,
    notes: Option<SecretString>,
}
```

`check_shape` requires `card` for `Card` and rejects it for the other
types, and rejects login, note and identity fields on a card. `Keep` on a
create is `Clear`.

### 4.6 Rules

* `stage_create`, `stage_update`, `stage_delete` work on cards as on
  logins. A type change into or out of `Card` is refused, as today.
* Import dedupe treats cards as their own kind; two cards are the same
  when their number and expiry are equal.

## 5. Desktop

### 5.1 Rust commands

* `reveal_card(id)` returns a card's cardholder name, brand, expiry,
  notes, and `hasNumber` / `hasVerificationNumber` flags; never the number
  or CVV (as `reveal_identity`; `get_item` only returns the overview).
* `reveal_card_field(id, field)`: `field` is `number` or
  `verificationNumber`. Only while unlocked, only for a Card (`Denied`
  otherwise).
* `copy_card_field(id, field)`: `cardholderName`, `number`,
  `verificationNumber` or `expiry` (copied as `MM/YYYY`). Rust reads the
  value and copies it with the vault's clipboard-clear delay, as
  `copy_secret` does.
* `check_card_number(number) -> { brand, checkDigitOk }`: for the editor's
  live logo and warning. Returns nothing else; the number is not kept.

All added to `build.rs`, `lib.rs`, `capabilities/main.json` and `api.ts`
(the four places `commands.test.ts` checks).

### 5.2 Sidebar and list

* A **Cards** entry with a count under Vault, after Secure notes.
* **New → Card**.
* Rows: the brand logo, the title and `•••• 7609 · 11/2033`.

### 5.3 Detail

Header: the brand logo on a light tile (as in 1Password), the title, and
"Card" as the kind. Then, in this order, the fields that have a value:

* **cardholder name**;
* **type**: the logo and the brand's name;
* **number**: `•••• •••• •••• 7609` (the overview keeps only the last 4); the eye reveals it grouped for its
  brand (Amex and Diners 14-digit 4-6-5 / 4-6-4, the rest in fours);
* **verification number**: masked; the eye reveals it;
* **expiry date**: `11/2033`, with an **Expired** badge when the month
  has passed;
* **notes**.

Every field has a copy button (through `copy_card_field`). A reveal hides
again after 30 s and on lock, as passwords do. Footer: created/updated
dates and Delete.

### 5.4 Editor

* Title (placeholder: the detected brand's name).
* Cardholder name.
* Number: grouped as the user types, the detected logo at its end, and
  "This number fails the card check digit. Check it for typos." under it
  when `checkDigitOk` is false. `check_card_number` is called 300 ms after
  the last keystroke. Editing an existing card shows the field empty with
  "Leave empty to keep the saved number"; typing sends `Set`.
* Type: a select, "Detect from number" by default, then the ten brands and
  Other.
* Verification number: masked, `inputmode="numeric"`, same keep/replace.
* Expiry: one `MM/YYYY` field; `MM/YY` is accepted and expanded to 20YY.
* Notes.

Rust's validation errors are shown as elsewhere. Offline, Save is disabled
like the other editors.

### 5.5 Logos (`packages/ui/src/card-brand-icons.ts`)

Full-colour path data in the same shape as `provider-icons.ts`
(`{ viewBox, shapes: [{ d | rect, fill }] }`), rendered with JSX on the
desktop and `createElementNS` in the extension; never parsed from strings.
One entry per brand in §4.2 except `other`, which uses a generic card glyph
(also added to `components/Icon.tsx` for the sidebar).

The path data is taken from a source whose licence allows redistribution.
The plan checks the licence of each logo before importing it and records
source and licence in `THIRD-PARTY-NOTICES.md`. A brand with no such source
gets the generic glyph and its name, and the plan says which. The logos
only identify the card's network, next to its name.

### 5.6 Strings

A new `card` block in `i18n/en.ts` and `i18n/pt-BR.ts`: field labels
(pt-BR: Nome do titular, Bandeira, Número, Código de segurança, Validade),
brand names, the check-digit warning, the expired badge, copy toasts.

## 6. 1Password import (`import/onepux.rs`)

Category `002` becomes a Card:

| 1Password field | Card field |
|---|---|
| `cardholder` | `cardholderName` |
| `type` (`creditCardType`) | `brand` (`visa`, `mc`/`master`, `amex`, `elo`, `hipercard`, `diners`, `discover`, `jcb`, `unionpay`, `maestro`; else detected) |
| `ccnum` (`creditCardNumber`) | `number` |
| `cvv` | `verificationNumber` |
| `expiry` (`monthYear`, e.g. `202612`) | `expiry` |
| notes | `notes` |

Every other field (valid from, bank, PIN, phones, limits, custom sections)
is appended to the notes as `label: value` lines, so nothing is lost. A
value that fails validation (a 20-digit number, a 9-digit CVV) moves to the
notes the same way, and the card is still imported. `import/mod.rs`'s
comment and the import summary count cards as their own kind.

## 7. Error handling

| Case | Behaviour |
|---|---|
| Invalid field on save | Rust's `InvalidInput` message, no values in it |
| Check digit fails | Warning in the editor; saved |
| Locked during reveal/copy | `Locked`, as the other commands |
| `reveal_card_field` on a non-card | `Denied` |
| Import value out of range | Moved to notes; the card is imported |

## 8. Testing

Core (`crates/havenkeys-core/tests/card.rs`):

* a card round-trips through seal/unseal; the overview holds only title,
  brand, last 4 and expiry;
* validation: each limit, number cleanup, CVV length, expiry range and
  format, empty card rejected, unknown fields rejected;
* `detect_brand`: at least one number per range per brand, the Elo and
  Hipercard overlaps, unknown prefixes → `None`;
* `check_digit_ok` on valid and one-digit-off numbers;
* brand on save: override kept, detection when absent, `other` fallback;
* `SecretUpdate` keep/set/clear for number and CVV;
* search by last 4, never by the full number;
* security regressions: `find_matches`, `fill_for_page`, `get_totp` and
  the identity requests never return a card on any origin;
* `Debug` of `CardFields` and `CardInput` shows no values.

Import: a 1Password export with a card (all fields, extra sections, an
out-of-range value) becomes one Card with the extras in its notes.

Desktop Rust: `copy_card_field` / `reveal_card_field` field parsing and
type checks; number grouping per brand.

Frontend (Vitest): the list subtitle, the expired badge, `MM/YY` expansion,
editor keep/replace.

Logos: every brand has an entry; the test used for `provider-icons.ts`
(path data only, valid colours).

`ui:check` in both languages and themes: the card list, a card detail with
the number revealed, the editor with the check-digit warning, an expired
card.

Then `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, `tsc`,
`vitest`, and `cargo audit`. No new crate is expected.

## 9. Documentation

* `docs/security-model.md`: the card item, what is in the overview (last 4
  and expiry), that the number and CVV reach the UI only on reveal.
* `README.md`: one line in the desktop feature list.
* `THIRD-PARTY-NOTICES.md`: the logos' sources and licences.
