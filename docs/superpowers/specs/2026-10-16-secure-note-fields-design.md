# Secure note fields — Design

Status: approved, 2026-10-16; implemented on feat/note-fields.

> This software has not undergone an independent security audit.

## 1. Goal

A secure note can carry the same typed custom fields as a login (spec
2026-09-30-login-custom-fields): named sections of Text, URL, Email, Phone,
Date, Address, Password and One-Time Password fields, below the note's text.
A note can then hold a server's address and root password, a Wi-Fi network
and its key, or an API key and its endpoint, with hidden values masked and
copyable instead of sitting in the note body as plain text.

Imports use the same fields. Bitwarden and 1Password notes, and the item
types HavenKeys turns into notes (1Password Server, Database, API
Credential, Wi-Fi, SSH Key, …; Bitwarden Identity and SSH Key), keep their
fields typed instead of flattened into the body. Today a 1Password Server's
password is imported as visible text in the note.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Where the fields live | `sections` inside `ItemDetails::SecureNote`, the same `FieldSection` type as the login | One `sections` list on every item type (moves login data, gives cards and identities fields they don't need, and decides the item-categories draft early); fields written as formatted text in the body |
| Field types, limits, edit rules | Exactly the login's (`custom_field.rs`) | A separate, smaller set for notes |
| Note text | Stays the main content, shown first; may be empty when the note has fields | Fields first; text as one more field |
| Desktop | Full view and edit, same components as logins | — |
| Android | Read-only; every custom field (notes and logins) shows its real label | Desktop only, keeping the generic "Field" label; full Android field editing (no field editor exists on Android yet) |
| Extension, autofill | Never see note fields; notes still never match a website | Fill from note fields |
| Search | Not searched (title and tags, as today for notes) | Labels or values searchable |
| Vault health | Does not check field passwords (as for logins) | Weak/reused checks on note fields |
| Imports | Bitwarden and 1Password notes and converted items keep typed fields | Only real notes; no import change |

## 3. What does not change

* The encrypted blob format, AAD binding and key hierarchy (`docs/crypto.md`).
* The overview: nothing about a note's fields is in it, so the server and
  the in-memory search index see no section titles, labels or values.
* The Keep/Set/Clear rule for Password and OTP values: concealed values are
  never sent to the renderer to open the editor; OTP secrets never leave
  Rust, only codes do.
* Login custom fields: same storage, same behavior. Only the names of the
  functions and commands that reach them change (§4.3, §5.1).
* Cards and identities: no `sections`. The Identity keeps its own custom
  fields.

## 4. Data model (`havenkeys-core`)

### 4.1 Place in the note

```rust
ItemDetails::SecureNote {
    content: SecretString,
    /// Custom fields, in sections (spec 2026-10-16-secure-note-fields).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sections: Vec<FieldSection>,
}
```

A note saved before this change has no `sections` key and loads with an
empty list; a note without fields serializes exactly as before. No
migration.

### 4.2 Input and validation

* `ItemInput.sections` (`model.rs:305`) is accepted for `Login` and
  `SecureNote`; the check at `model.rs:567` rejects it for `Card` and
  `Identity` only.
* The note's save path (`vault.rs`, around `:2646-2723`) reads the current
  note's sections and runs the same `custom_field::apply_sections(sections,
  cur_sections)` as the login. `sections: None` keeps the current fields.
* Limits are the login's (`MAX_SECTIONS`, `MAX_FIELDS`,
  `MAX_SHORT_VALUE_CHARS`, `MAX_TOTAL_BYTES`), counted per item.
  `MAX_NOTE_CONTENT_BYTES` still applies to `content` alone.
* A note needs a title, as today. Its content may be empty.

### 4.3 Item-level accessors

`Vault::login_sections` and `Vault::login_field` (`vault.rs:2268, 2278`)
become `item_sections` and `item_field`. They return a login's or a note's
sections and `NotFound` for any other item type. Every caller is renamed;
there is no alias.

### 4.4 Exports and backup

* **Bitwarden JSON** (`export/bitwarden.rs`): Bitwarden items of any type
  carry `fields`, so a note's fields use the conversion logins already use
  (`:198-212`): flattened across sections, Address as `formatted()`,
  Password as a hidden field, OTP as a hidden field holding the
  `otpauth://` URI. Section titles are lost, as for logins.
* **CSV** (`export/mod.rs:128-139`): a note's fields are counted in
  `custom_fields_left_out`, so the existing warning names them.
* **Encrypted backup**: carries them already (`ItemDetails` is stored
  whole). Restore (`export/restore.rs` `sections_input`) builds
  `ItemInput.sections` for notes as well as logins.

### 4.5 Imports

The section builder logins use (`import/common.rs` `LoginSections`) is
renamed `ItemSections` and used for notes too.

* **Bitwarden** (`import/bitwarden.rs`)
  * Type 2 (secure note): custom fields go into one untitled section, as for
    logins. Type 1 (hidden) becomes Password; anything else becomes Text.
    The body is the item's notes only.
  * Types turned into notes (4 identity, 5 SSH key, others): the type's own
    values go into a section titled with the type name ("Identity",
    "SSH key"). The SSH `privateKey`, and the identity's `ssn`,
    `passportNumber` and `licenseNumber`, become Password fields; the rest
    become Text. Custom fields follow in an untitled
    section. The body is the "Imported from Bitwarden (…)" line, then the
    notes.
* **1Password** (`import/onepux.rs`)
  * `003` (secure note): its sections become typed fields with the mapping
    logins use (`:382-457`). The body is the note's `notesPlain`.
  * Other categories turned into notes: same mapping for their sections;
    websites become URL fields in an untitled first section. The body is the
    "Imported from 1Password (…)" line, then the notes.
* A value that does not fit a field (over a limit, or more fields than
  `MAX_FIELDS`) falls back to today's behavior: it is appended to the body
  as text. A failed field never drops a value.
* The import report's counts (`secure_notes`, `converted_to_notes`) do not
  change.
* "Already in the vault" (`vault.rs` `dedupe_key_parts`) for a note hashes
  its fields too (section titles, labels, types and values), so two notes
  with the same title and text but different fields are not merged. A note
  without fields hashes exactly as before.

### 4.6 Android bridge (`havenkeys-mobile`)

* `items.rs:247-255`: each custom field's `ViewField` gets its real label,
  prefixed by its section's title when there is one ("Server · Password").
  This applies to logins and notes. The Kotlin "Field" fallback
  (`R.string.field_custom`) is used only for an empty label.
* `items.rs:263`: a note shows `content` first, then its fields, with the
  same `custom.<uuid>` keys, kinds (Secret when concealed, which includes
  OTP fields, whose reveal answers the current code; Text otherwise) and
  reveal path (`items.rs:317-325`).
* `edit.rs:509`: `has_custom_fields` is true for a note with fields, so the
  existing "custom fields kept" message shows. A phone edit sends
  `sections: None`, which keeps them.

## 5. Desktop

### 5.1 Rust commands (`apps/desktop/src-tauri`)

The five commands in `src/custom_field.rs` are renamed and accept a login
or a note:

| Before | After |
|---|---|
| `login_fields` | `item_fields` |
| `reveal_login_field` | `reveal_item_field` |
| `login_field_totp` | `item_field_totp` |
| `copy_login_field` | `copy_item_field` |
| `open_login_field_url` | `open_item_field_url` |

`lib.rs` registration, the capability/permission files and the `api.ts`
wrappers (`itemFields`, `revealItemField`, …) change to match. No other
command is added. Any other item type gets `NotFound`, as before.

### 5.2 Detail (`ItemDetail.tsx`)

* The note text stays first, in its sheet.
* `<CustomSections itemId updatedAt>` renders below it for a note, with the
  login's rows and actions (reveal, copy, code, open URL).
* A note with fields and empty text hides the sheet rather than showing an
  empty box.
* Plain values come with `item_fields` when the note opens, like the
  note's text; Password values only on reveal; OTP only as codes.

### 5.3 Editor (`ItemEditor.tsx`)

* The note textarea stays first; `<CustomFieldsEditor>` follows it, loaded
  through `api.itemFields` exactly as for logins (`:112`).
* The note's `ItemInput` includes `sections` (from `sectionsInput`).
* The textarea is 14 rows for a note without fields and 6 rows once it has
  one, growing with its content, so the fields are in view.
* A new note starts with no fields: the text area and the "Add field"
  button.

### 5.4 Strings

No new strings expected; the login editor's field and section strings are
reused. Any that say "login" are reworded for both.

### 5.5 Design review

The note detail and editor get the impeccable check before the work is
called done.

## 6. Error handling

Unchanged from the login spec §7: limit and validation errors are the
existing `InvalidInput` messages, which never contain field labels or
values. A field id that is not on the item, or an item that is neither a
login nor a note, is `NotFound`.

## 7. Compatibility

A device on a release from before this change ignores a note's fields and
drops them if it rewrites the note (any save of that note). This is the
same limit login custom fields had. The security-model limitation is
updated to name notes: update every device before adding fields to notes.

## 8. Testing

* **Core**
  * A note with fields round-trips through save, load and sync.
  * A note blob without `sections` loads with none; a note without fields
    serializes without the key.
  * Keep/Set/Clear of a Password field and Keep/Clear of an OTP field on a
    note; `sections: None` keeps them.
  * `sections` on a card or identity is rejected.
  * A note with fields and empty content saves; a note without a title is
    still rejected.
  * Limits apply to note fields.
* **Exports**: Bitwarden JSON note with fields (hidden Password, OTP as URI,
  Address formatted); CSV counts note fields as left out; backup restore
  keeps note sections whole, including OTP setups.
* **Imports**: fixtures for a Bitwarden note with a hidden and a text field;
  Bitwarden SSH key (private key as Password); a 1Password secure note with
  sections; a 1Password Server (URL, username, password typed); an
  oversized value falling back to the body.
* **Android bridge**: the view of a login and of a note shows real labels
  with section prefixes; revealing a note's Password field and code of its
  OTP field; `has_custom_fields` for a note; a phone edit of a note keeps
  its fields.
* **Desktop**: Tauri command tests for revealing and copying a note's field
  and refusing a card's; Vitest for the editor building a note's
  `sections`, the empty-text sheet being hidden, and the renamed API
  wrappers.

## 9. Documentation

* `docs/crypto.md` §(item contents): a note's `sections` live in the same
  `ItemDetails::SecureNote` blob.
* `docs/security-model.md`: the command table rows (renamed, notes
  included), the custom fields paragraph ("a login's or a note's"), and the
  older-releases limitation.
* `docs/architecture.md`: `custom_field.rs` and the command names.
* `docs/android.md` and `README.md`: notes show their fields on Android,
  with real labels; editing them is desktop-only.
* `docs/security-review.md`: a manual check that a phone edit of a note
  keeps its fields.
