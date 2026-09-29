# Identity item — Design

Status: proposed, 2026-09-29.

> This software has not undergone an independent security audit.

## 1. Goal

1Password keeps an **Identity** item: name, birth date, documents, address,
phones, email and the like, used to fill sign-up and checkout forms. HavenKeys
gets the same: every account has exactly one Identity, created automatically,
that holds everything about the person and anything else they want to
remember.

This is sub-project 1 of 2:

1. **This spec:** the Identity item type in the vault (core, storage, sync)
   and in the desktop app (view, editor, copy), created automatically once
   per account.
2. **Next spec:** filling registration and checkout forms from the Identity
   in the browser extension (field detection for address/phone/name/document
   fields, a new native-messaging request, and an authorization model for a
   fill that is not bound to a site's URL rules).

The data model here is shaped for sub-project 2 (typed, split address fields)
but nothing in this spec lets the extension read an identity.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| How many identities | Exactly one per account; no "New identity" | One by default plus more (autofill would have to ask which) |
| Fields | Fixed typed fields plus user-defined custom fields | Fixed only; fully free-form (autofill would have to guess) |
| Created by | Each device at unlock/reconnect, if missing | Only at activation (misses existing vaults); the user by hand |
| Item ID | Derived from the vault key (HKDF), same on every device | Random (duplicates across devices); a hash of the account ID (the server could tell which blob is the identity) |
| Delete | Refused by the core; no Delete in the UI. Fields can be cleared | Deletable (would be recreated at the next unlock anyway) |
| Where the fields live | All in the details blob; name and email also in the overview | Everything in the overview (decrypted at every unlock, kept in memory) |
| Document numbers | Masked in the UI with a reveal, like passwords | Shown in the clear |
| Format checks (CPF, CEP, phone) | Length and character set only | Checksums and national formats (rejects foreign data; later if wanted) |
| 1Password import | Unchanged: imported identities stay secure notes | Merging into the one identity |

## 3. What does not change

* The encrypted blob format, AAD binding and key hierarchy (`docs/crypto.md`).
* The server. It stores item blobs without knowing their type
  (`migrations/0001_init.sql`), so the identity is two more opaque blobs.
* Sync, the write path and conflicts (`2026-09-20-server-authoritative-vault-design.md`).
* Native messaging and the extension. `find_matches` keeps only logins,
  `fill_for_page` / `authorize_for_page` deny anything that is not a login,
  and no new request is added here.
* The importer (`import/onepux.rs`), which keeps converting non-login,
  non-note categories to secure notes.

## 4. Data model (`havenkeys-core`, `model.rs`)

### 4.1 Type

`ItemType` gains `Identity` (`"identity"` on the wire). `ItemDetails` gains:

```rust
Identity(Box<IdentityFields>)
```

### 4.2 Fields

Every field is `Option<SecretString>` (wiped on drop, redacted in `Debug`),
except `custom`. Wire names are camelCase; `deny_unknown_fields`.

| Section | Fields | Limit (chars) |
|---|---|---|
| Identification | `firstName`, `middleName`, `lastName`, `gender`, `occupation`, `company`, `jobTitle` | 256 each |
| | `birthDate`: `YYYY-MM-DD`, a real calendar date, not after today | 10 |
| Documents | `cpf`, `rg`, `passport`, `driversLicense` | 64 each |
| Contact | `email` | 512 |
| | `mobilePhone`, `homePhone`, `workPhone`: digits, spaces and `+ ( ) - .` | 32 each |
| Address | `street`, `complement`, `neighborhood`, `city`, `state`, `country` | 256 each |
| | `number`, `postalCode` | 32 each |
| Internet | `username` | 512 |
| | `website`: an http(s) URL, parsed with `url` like login websites | 2048 |
| Custom | `custom: Vec<CustomField { label, value, hidden: bool }>`, at most 50 | label 128, value 4096 |
| Notes | `notes` | 64 KiB, as login notes |

Whitespace-only values are stored as absent. Control characters other than
newline (in `notes`, `street` and custom values) are rejected. A custom field
with an empty label is rejected; one with an empty value is dropped.

### 4.3 Overview and search

The overview carries only what the list and search need:

* `title`: the full name (`firstName middleName lastName`, trimmed and
  joined with single spaces) or the empty string when there is none. The UI
  shows "Identity" for an empty title.
* `username`: the identity's email, so the list shows it as the subtitle and
  search matches it (the existing `vault.rs` search covers `title` and
  `username`).
* `has_notes`; the other `has_*` flags false.

Everything else is only in the details blob, decrypted when the item is
opened (as a secure note's body is).

### 4.4 Input

`ItemInput` gains `identity: Option<IdentityFields>`. `check_shape` requires
it for `Identity` and rejects it for the other types, and rejects login and
note fields on an identity. An update replaces the whole identity: the
editor always has the decrypted fields, so there is no per-field
`SecretUpdate`.

## 5. The one identity

### 5.1 Its ID

```text
identity_item_id = UUID(v8) from the first 16 bytes of
    HKDF-SHA256(ikm = vault key, salt = none, info = "havenkeys/v3/identity-item-id")
```

built with `uuid::Uuid::new_v8`, which sets the version and variant bits.
The ID is recomputed on each unlock; it depends only on the vault key, so a
master-password change (which rewraps, not replaces, the vault key) keeps it. Every device that
holds the vault key computes the same ID; the server, which holds only
ciphertext and plaintext item IDs, cannot tell which item is the identity. It
is computed on unlock and kept in the session; `VaultService::identity_item_id()`
returns it while unlocked (`Locked` otherwise).

### 5.2 Creating it

A new core method:

```rust
fn stage_identity_if_missing(&self, email: &str, now_ms: i64) -> Result<Option<StagedWrite>>
```

returns `None` when the replica already has an item with that ID, otherwise
stages an Identity with `email` prefilled and `baseRevision = null`.

The desktop calls it at the end of `sync::connect` (after the first pull, so
the replica is current), which runs at every unlock with a session and every
reconnect:

* staged → `push`; on success the item is in the replica;
* the server answers **409** (another device created it between the pull and
  the push) → ignore and `sync_now`, which brings the other device's copy;
* any other failure → ignore; the next connect tries again. A failure here
  never fails the unlock and is not shown as an error.

Offline, nothing is created; the next connect does it.

### 5.3 Rules the core enforces

* `stage_delete` on the identity ID → `Denied`.
* `stage_create` with `item_type: Identity` → `Denied`: the only way an
  identity comes to exist is §5.2, with the derived ID.
* `stage_update` of the identity is allowed; of any other item into an
  identity (type change) is already refused.
* An identity received from the server under any other ID is kept and shown
  (it can only come from a modified client), but is not "the identity": the
  sidebar entry opens the one with the derived ID.
* The import dedupe and counts treat identities as their own kind and never
  merge into them.

## 6. Desktop

### 6.1 Rust commands

* `get_item`, `update_item` accept identities (no new commands).
* `reveal_identity(id) -> IdentityFields`: the decrypted fields, only while
  unlocked, only for an Identity item (`Denied` for others). Called when the
  detail or the editor opens, dropped with the component (on lock too).
* `copy_identity_field(id, field) -> CopyResult`: `field` is one of the
  typed field names above, `address` (the formatted block, §6.3) or
  `custom:<index>`. Rust reads the value and copies it with the vault's
  clipboard-clear delay, as `copy_secret` does.
* `identity_item_id() -> Uuid`: for the sidebar entry.

All added to `build.rs`, `lib.rs`, `capabilities/main.json` and `api.ts`
(the four places `commands.test.ts` checks).

### 6.2 Sidebar and list

* An **Identity** entry (ID-card icon) under Vault, after Secure notes. It
  selects the identity in All items and opens its detail. No count.
* In All items the identity is an ordinary row: an ID-card avatar, the name
  (or "Identity") and the email. Search matches name and email.
* The New menu gains nothing.

### 6.3 Detail

Header: ID-card avatar, full name (or "Identity"), and "Identity" as the
kind. Then, in this order, one group per section with a section title
(IDENTIFICATION, DOCUMENTS, CONTACT, ADDRESS, INTERNET, OTHER, NOTES):

* only fields with a value are shown; a section with none is hidden;
* each field has a copy button;
* documents and `hidden` custom fields are masked with an eye to reveal
  (auto-hide after 30 s, as passwords);
* the birth date is shown in the UI locale's date format;
* the address group shows the parts as fields and, above them, the combined
  block with **Copy address**. The block's layout is Brazilian
  (`street, number – complement`, `neighborhood`, `city – state`,
  `CEP postalCode`, `country`) when `country` is Brazil/Brasil/BR, and
  otherwise `street number, complement`, `city, state postalCode`, `country`;
* the website opens through the existing `open_website` checks;
* when every field is empty: "Your identity is empty" and **Fill in your
  identity**, which opens the editor;
* footer: created/updated dates; no Delete.

### 6.4 Editor

The same sections as inputs. Birth date is a date input; phones use
`type="tel"`; the rest are text. Custom fields: **Add field**, each row a
label, a value, a *Hidden* switch and a remove button. Save sends the whole
identity through `update_item`; Rust's validation errors are shown as
elsewhere. Offline, Save is disabled like the other editors.

### 6.5 Strings

A new `identity` block in `i18n/en.ts` and `i18n/pt-BR.ts`: section titles,
field labels, the empty state, copy toasts. pt-BR uses Brazilian terms (Nome,
Sobrenome, Data de nascimento, CPF, RG, CNH, CEP, Bairro, Complemento).

## 7. Error handling

| Case | Behaviour |
|---|---|
| Identity creation fails at connect | Silent; retried at the next connect |
| Two devices create it at once | The second gets 409, then pulls the first's |
| Identity missing (offline since activation) | Sidebar entry disabled, with "Connect to create your identity" |
| Invalid field on save | Rust's `InvalidInput` message, no values in it |
| Locked during reveal/copy | `Locked`, as the other commands |
| Delete attempted by a modified renderer | `Denied` |

## 8. Testing

Core (`crates/havenkeys-core/tests`):

* identity round-trips through seal/unseal; overview holds only name, email
  and flags;
* validation: each limit, birth date (format, impossible dates, future),
  phones, website, custom field count/label, unknown fields rejected;
* the derived ID is equal on two devices of one account and differs between
  two accounts; `identity_item_id` is `Locked` when locked;
* `stage_identity_if_missing` stages once, then returns `None` once the item
  is in the replica;
* `stage_delete` of the identity and `stage_create` of an identity are
  `Denied`;
* security regressions: `find_matches`, `fill_for_page` and `get_totp` never
  return the identity, on any origin;
* `Debug` of `IdentityFields` shows no values.

Desktop Rust: `copy_identity_field` field parsing (known names, `address`,
`custom:<n>` in and out of range, anything else rejected); the address block
formatter (Brazilian and default layouts).

Frontend (Vitest): empty-section hiding, the display title fallback.

`ui:check` scenarios in both languages and themes: identity empty, identity
full, a document revealed, the editor with custom fields.

Then `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, `tsc`,
`vitest`, and `cargo audit`. No new crate is expected: `hkdf`, `sha2` and `uuid` are
already in the core; `uuid` gains its `v8` feature (no new code from outside
the crate).

## 9. Documentation

* `docs/security-model.md`: the identity item, what is in the overview, the
  derived ID and why, that the extension cannot read it (until spec 2).
* `docs/crypto.md`: the new HKDF info string.
* `README.md`: one line in the desktop feature list.
