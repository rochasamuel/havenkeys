# Login custom fields — Design

Status: draft, 2026-09-30.

> This software has not undergone an independent security audit.

## 1. Goal

1Password lets a login carry any number of extra fields, grouped in named
sections, added from an "add another field" menu: Text, URL, Email,
Address, Date, One-Time Password, Password, Phone. HavenKeys gets the same
on logins, so a login can hold security answers, PINs, recovery codes,
account numbers and a second authenticator, and so 1Password imports keep
their structure instead of being flattened into notes.

Custom fields are stored, shown and copied in the desktop app only. The
browser extension never sees them.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| What custom fields do | Stored, viewed, revealed and copied in the desktop app | Filled by the extension; used by autofill as a second OTP |
| Item types | Logins only | Secure notes too; replacing the Identity's own custom fields |
| Field types | Text, URL, Email, Address, Date, One-Time Password, Password, Phone | Text + Password only; no OTP type |
| Attach a File | Out of scope (encrypted file storage and sync is its own subsystem) | — |
| "Sign in with" | Stays the one login-level setting (spec 2026-09-28) | A repeatable field type |
| Grouping | Named sections, as in 1Password | One flat list |
| Order | User-defined; drag and keyboard reorder, across sections | Fixed insertion order |
| Where they live | Inside the encrypted `ItemDetails::Login`; nothing in the overview | Layout and labels in the overview (more metadata in memory for the whole session) |
| Concealed values in the editor | Keep/Set/Clear by field id, like the login password; never loaded to open the editor | The whole field list round-tripped through the UI |
| Plain values (Text, URL, Email, Phone, Date, Address) | Returned when the login is opened, like notes | Concealed until revealed |
| Search | Not searched (title, username and websites only) | Labels or values searchable |
| Password field history | None | History like the login password |
| Address | Structured, same parts as the Identity address, so a later "Copy street / city / …" menu needs no format change | One multi-line text |
| 1Password import | Section fields become custom fields with their type and label | Kept in notes as today |

## 3. What does not change

* The encrypted blob format, AAD binding and key hierarchy (`docs/crypto.md`).
* The server and sync: they carry item blobs without looking inside.
* The extension, the native-messaging protocol and every page-bound call
  (`find_matches`, fill, TOTP, save login, passkeys, sign-in-with). No
  request can name a custom field.
* The overview and search.
* Identity and Card items, including the Identity's own `CustomField`.
* Other importers (CSV, Bitwarden, …).

The vault format needs no migration: `sections` defaults to empty, and the
owner's vault is resettable anyway.

## 4. Data model (`havenkeys-core`)

### 4.1 Types (new module `custom_field.rs`)

```rust
pub struct FieldSection {
    pub id: Uuid,
    pub title: Option<SecretString>,   // None: untitled section
    pub fields: Vec<CustomField>,
}

pub struct CustomField {
    pub id: Uuid,
    pub label: SecretString,
    pub value: FieldValue,
}

#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FieldValue {
    Text(SecretString),               // single or multi-line
    Url(SecretString),                // http(s), parsed with `url`
    Email(SecretString),
    Phone(SecretString),
    Date(String),                     // "YYYY-MM-DD", a real calendar date
    Address(Box<AddressValue>),
    Password(SecretString),           // concealed
    Otp(TotpConfig),                  // secret never leaves the core
}

pub struct AddressValue {            // every part Option<SecretString>
    pub street, number, complement, neighborhood,
    city, state, postal_code, country,
}

pub enum AddressPart { Street, Number, Complement, Neighborhood,
                       City, State, PostalCode, Country }
```

* The address parts and their order are the Identity's (`identity.rs`).
  `AddressPart` exists now so that copy can take an optional part (§5.1);
  the desktop only sends `None` in this spec.
* `Debug` on every type prints ids and the field kind only, never labels
  or values. Strings are `SecretString` (wiped on drop), as the rest of
  `ItemDetails`.

### 4.2 Place in the login

```rust
ItemDetails::Login {
    …,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sections: Vec<FieldSection>,
}
```

Order is the order of the vectors. Nothing is added to `ItemOverview`: the
list and search do not need to know a login has custom fields.

### 4.3 Limits

| | Limit |
|---|---|
| Sections per login | 20 |
| Fields per login (all sections) | 100 |
| Section title, field label | 256 characters (`MAX_TITLE_CHARS`) |
| Text value | 64 KiB (`MAX_NOTES_BYTES`) |
| Password value | `MAX_PASSWORD_CHARS` via `check_password` |
| URL value | `MAX_URL_LEN` |
| Email, Phone, each address part | 512 characters |

Checked before anything is encrypted; a breach is
`Error::InvalidInput` with a generic message naming the limit, never the
value.

### 4.4 Input

`ItemInput` gains:

```rust
/// A login's custom fields. `None` keeps them as they are (every save path
/// that is not the desktop editor); `Some` replaces the whole layout, in
/// order.
#[serde(default)]
pub sections: Option<Vec<SectionInput>>,
```

```rust
pub struct SectionInput {
    pub id: Option<Uuid>,             // None: new section
    pub title: Option<String>,
    pub fields: Vec<FieldInput>,
}

pub struct FieldInput {
    pub id: Option<Uuid>,             // None: new field
    pub label: String,
    pub value: FieldValueInput,
}

#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FieldValueInput {
    Text(SecretString), Url(SecretString), Email(SecretString),
    Phone(SecretString), Date(String), Address(Box<AddressValue>),
    Password(SecretUpdate),
    Otp(SecretUpdate),                // Set: otpauth:// URI or Base32
}
```

All with `deny_unknown_fields` and redacted `Debug`, like `ItemInput`.
`ItemInput::blank` sets `sections: None`, so `login_input_from`,
`new_login_for_frame`, `new_login_for_passkey` and the importers' builders
keep a login's fields untouched without each naming the new key.
`sections` on a non-login item is refused, like `identity` on a login.

### 4.5 Rules when a layout is applied

In the `ItemType::Login` branch of the details build, after the password,
notes and TOTP (§4.6), one function `apply_sections(input, current)`:

1. Section ids and field ids given by the client must each exist **in this
   login's current sections**; an unknown id is `InvalidInput`. A new
   section or field has no id; the core gives it `Uuid::new_v4()`. A field
   may move to another section: ids are looked up across the whole login.
2. An id may appear once in the layout.
3. `Password(Keep)` and `Otp(Keep)` are accepted only when the field with
   that id currently holds the **same type**. So a save can neither copy a
   secret from another login (ids are never looked up outside this login)
   nor turn a hidden OTP secret into a revealable field.
4. `Keep` on a field that has no value (new field) is `InvalidInput`.
5. An empty `Set` is `Clear`, as `SecretUpdate::apply` already does. A
   cleared Password or OTP field stays, empty, with its label; a field is
   removed only by leaving it out of the layout.
6. Values are validated per type (§4.3, URL parse, calendar date,
   `TotpConfig::validate`).
7. Removed fields and sections are simply absent from the new details;
   their `SecretString`s are dropped and wiped.

### 4.6 Shared helpers (the pattern of the core-security refactor)

The same shape as the `refactor/core-security` branch: one helper per
check, used everywhere, with a test that pins today's behaviour and passes
before the change.

* **`apply_totp_update(update, current) -> Result<Option<TotpConfig>>`**:
  the `Keep / Clear / empty Set / Set → parse_totp_input` match now inline
  for the login's main TOTP moves into one function used by the main TOTP
  and by `Otp` fields. Behaviour of the main TOTP is unchanged (pinned by a
  test first).
* Value checks reuse `check_password`, `check_notes`, `clean_title` and the
  `url` parse already used for website rules; no second copy of any.
* Reading a login's details for field operations goes through one
  `login_field(&self, id, field_id) -> Result<CustomField>`: `session()` is
  its first step (a locked vault answers `Locked` before anything else),
  then `load_details`, then the field lookup. `reveal_login_field`,
  `login_field_totp` and `copy_login_field` all call it and only match on
  the result.
* Mapping a 1PUX field to a custom field is one function (§6) used for both
  section fields and extra `loginFields`.

## 5. Desktop

### 5.1 Rust commands (`apps/desktop/src-tauri`)

| Command | Returns | Notes |
|---|---|---|
| `login_fields(id)` | `Vec<SectionView>` | Text, URL, Email, Phone, Date, Address values included. Password → `{ hasValue }`. OTP → `{ hasOtp }`. |
| `reveal_login_field(id, fieldId)` | `SecretString` | Password fields only; any other type is `InvalidInput`. |
| `login_field_totp(id, fieldId)` | `TotpCode` | OTP fields only. |
| `copy_login_field(id, fieldId, part: Option<AddressPart>)` | `CopyResult` | Through `copy_from_vault`: read and copied in Rust, cleared after the vault's delay. OTP copies the current code. Address with `None` copies the formatted address; `part` only on Address. |

Every command calls `state.touch()` first, as the card commands do. The
four commands are added to the command allowlist and capability file; no
other permission changes. Saving goes through the existing create/update
item commands with `ItemInput.sections`.

### 5.2 Detail (`ItemDetail.tsx`)

Under the built-in fields, each section with its title (untitled: no
heading), each field with its label above the value:

* **Password**: dots, eye to reveal, Copy.
* **OTP**: live code with countdown ring, Copy.
* **URL**: link opened in the system browser the way the website field is.
* **Address**: formatted on several lines. **Date**: localized.
* Every field: **Copy**.

Revealed values are dropped when the item changes, on unmount and on lock,
as today.

### 5.3 Editor (`ItemEditor.tsx`)

* After notes: **Add another field** → menu with the eight types; adds a
  row to the last section (creating an untitled one if there is none).
  **Add section** adds a titled section.
* Row: drag handle, label (defaults to the type's name), type-appropriate
  input, remove. Date: native date input. Address: small grid of parts.
  Password: masked with generate and reveal. OTP: `otpauth://` or Base32,
  QR scan as the main TOTP.
* Existing Password and OTP fields start as `Keep` with no value loaded;
  reveal in the editor calls `reveal_login_field`.
* Section title editable; removing a section with fields asks first.
* Reorder by native HTML5 drag and drop (no new dependency) and from the
  keyboard: focused handle + Alt+↑ / Alt+↓, crossing section boundaries.
  Sections reorder the same way.
* The editor snapshot and "unsaved changes" check include sections.
* The payload is built by one function `sectionsInput(state)` so it can be
  unit-tested.

### 5.4 Strings

Every new string in `en` and `pt-BR`.

## 6. 1Password import (`import/onepux.rs`)

For logins and passwords (`001`, `005`), each 1Password section becomes a
`FieldSection` (title kept) and each field a `CustomField` with its label,
by one function `custom_field_from(label, value, report) -> Option<FieldValue>`:

| 1PUX kind | Field |
|---|---|
| `string`, `menu`, `gender`, unknown text kinds | Text |
| `monthYear` | Text `MM/YYYY` |
| `concealed` | Password |
| `email`, `phone` | Email, Phone |
| `url` | URL if valid http(s), else Text |
| `date` | Date (existing `format_date`) |
| `address` | Address (`street`, `city`, `state`, `zip` → `postal_code`, `country`) |
| `totp` | first valid one stays the login's TOTP (as today); later valid ones → OTP; invalid → Text |
| `ssoLogin` | first recognized one stays `sign_in_with` (as today); others → Text |
| `sshKey` | Password, with the text `render_value` builds today |
| `file` | skipped and counted, as today |

* Extra `loginFields` (not the designated username/password) go to an
  untitled first section: `P` → Password, others → Text.
* Tags and URLs that are not websites still go to notes.
* Past the limits (§4.3) the rest go to notes as today and the report
  counts them (`fields_to_notes`), so nothing is dropped silently.
* Other categories keep today's behaviour.
* Temporary strings are zeroized as the importer does now.

## 7. Error handling

| Case | Error |
|---|---|
| Vault locked | `Locked` (before any other check) |
| Item not a login / field id not in this login | `NotFound` |
| Reveal on a non-Password field, TOTP on a non-OTP field, `part` on a non-Address | `InvalidInput` |
| Bad layout (unknown or duplicate id, `Keep` type mismatch, limits, bad URL/date/OTP) | `InvalidInput` with a generic message |
| `sections` on a non-login | `InvalidInput` |

No message contains a label or a value.

## 8. Testing

Core (`crates/havenkeys-core/tests/custom_fields.rs`, plus `security.rs`):

* Round trip: every type, several sections, order preserved.
* `sections: None` keeps the fields; every existing save path (browser save
  login, update, sign-in-with save, passkey create, import) leaves a
  login's fields untouched.
* `Keep`/`Set`/`Clear` for Password and OTP; empty `Set` is `Clear`.
* **Security:** `Keep` with a field id from another login → rejected;
  unknown id → rejected; duplicate id → rejected; `Keep` on an OTP id sent
  as `Password` (and the reverse) → rejected; locked vault → `Locked`
  before origin or id checks; reveal of Text/OTP → rejected; OTP secret is
  never in any command output; the page-bound fill paths return no custom
  field.
* Moving a field across sections keeps its value (`Keep`).
* OTP field codes match the RFC 6238 vectors.
* Limits, calendar dates (`2026-02-30` refused), URL parse.
* A login blob without `sections` loads.
* `apply_totp_update`: a table pinning today's main-TOTP behaviour, passing
  before the refactor.
* `Debug` of every new type contains no label or value.

Import: a 1PUX fixture with every kind, a second TOTP, `loginFields`
extras, a file, and a login over the limits.

Desktop: `sectionsInput` payloads (new, kept, set, cleared, reordered,
moved); the dirty check sees section edits; `pnpm typecheck`.

## 9. Documentation

* `docs/security-model.md`: custom fields are desktop-only; which values
  reach the webview on open and which only on reveal; OTP secrets never.
* `docs/crypto.md`: `sections` inside the login details.
* `docs/architecture.md`: the four new commands.
* `CLAUDE.md` is not amended: this stays inside the MVP's login items.
