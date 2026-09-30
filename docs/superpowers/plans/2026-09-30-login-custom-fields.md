# Login Custom Fields Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Logins get any number of labelled, typed custom fields (Text, URL, Email, Address, Date, One-Time Password, Password, Phone) grouped in named sections, stored inside the encrypted login details, shown, revealed, copied and reordered in the desktop app, and filled from 1Password imports.

**Architecture:** A new core module `custom_field.rs` owns the stored types, the editor's input types, the view the UI receives (concealed values replaced by flags) and one `apply_sections` that validates a submitted layout against the login's current fields (ids from this login only, same type for `Keep`). `ItemDetails::Login` gains `sections`, `ItemInput` gains `sections: Option<…>` (`None` keeps them, so every existing save path is untouched). The desktop gets five commands in `src-tauri/src/custom_field.rs`, a pure TS editor model in `src/lib/customFields.ts`, and two React components.

**Tech Stack:** Rust (havenkeys-core, Tauri 2 desktop crate), React 19 + TypeScript, vitest, Playwright (`pnpm ui:check`).

**Spec:** `docs/superpowers/specs/2026-09-30-login-custom-fields-design.md` (read it first). Style to follow: the `refactor/core-security` merge `2bea072` (`git log 2bea072^1..2bea072^2`): one helper per check, shared builders on `ItemInput::blank`, `session()` first, and a test that pins today's behaviour **before** a refactor, passing on the old code.

## Global Constraints

- `CLAUDE.md` rules apply. Never log, print, format into an error, or `Debug`-print a label, section title or value. `Debug` impls print ids and the field kind only.
- Commits: **no `Co-Authored-By` trailer** (user preference). Work on branch `login-custom-fields`; merge to `main` with `--no-ff` only if the user asks.
- Custom fields are desktop-only. No change to `havenkeys-protocol`, the native host, the bridge's messages or the extension.
- Wire names (exact). Field types: `text url email phone date address password otp`. Address parts (camelCase): `street number complement neighborhood city state postalCode country`. Stored and input values use `{"type": "<kind>", "value": …}`. Views use a flat `{"id", "label", "type", …}` with `value` for plain kinds, `parts` + `formatted` for address, `hasValue` for password and `hasOtp` for otp.
- Limits: 20 sections; 100 fields per login; label and section title ≤ 256 chars, no control characters; Text ≤ 64 KiB (`check_notes`); Password via `check_password`; URL via `normalize_url` (empty allowed); Email, Phone, Date and each address part ≤ 512 chars, one line; Date `YYYY-MM-DD`, a real calendar date (empty allowed); **all values, labels and titles of one login together ≤ 256 KiB** (clarifies spec §4.3 "the whole blob stays bounded").
- Dates are stored as `SecretString` rather than the spec's `String`, so they are wiped on drop like every other value.
- A field id or section id sent by the UI must exist in **this** login's current fields and appear once. `Keep` is allowed only on an existing field. An existing id must keep its type.
- The item-not-a-login and field-not-found errors are `NotFound`. A wrong operation on an existing field (reveal a Text field, a code from a Password field, `part` on a non-address) is `InvalidInput`. A locked vault is `Locked` before either.
- 1Password `loginFields` extras go into an untitled section placed **after** the 1Password sections. The spec says "first", but `loginFields` are read after `sections` and moving them would change today's order of the notes.
- Opening a URL field is a fifth command, `open_login_field_url`, built like `open_website`: the renderer names the field and Rust opens only that field's saved and re-normalised http(s) address.
- Every user-visible string goes in both `apps/desktop/src/i18n/en.ts` and `pt-BR.ts`; the `Messages` type enforces the shape.

## Review Focus

1. **Saving a login before its custom fields have loaded, or after loading them failed.** The fields must be kept, not wiped. The editor sends no `sections` key while they are unknown. Pinned in Task 6 (`sectionsInput(null)` is `undefined`) and Task 3 (`none_keeps_the_fields`).
2. **Changing a login's password from the browser's save prompt.** The login's custom fields must survive. Pinned in Task 3 (`a_browser_password_update_keeps_the_fields`).
3. **Dragging a Password or OTP field to another section and saving without revealing it.** The secret must be kept. Pinned in Task 2 (`a_field_moved_to_another_section_keeps_its_secret`) and Task 6 (`moveField` across sections keeps `KEEP`).
4. **A 1Password login with an odd value**: a TOTP that does not parse, a phone with letters, a date far outside 0001–9999, or more than 100 fields. The login must import; the value becomes Text, or goes to notes and is counted, and the item does not fail. Pinned in Task 4 (`odd_values_become_text_and_overflow_goes_to_notes`).
5. **A cleared Password or OTP field.** It keeps its label, reveal and copy answer `NotFound`, and the view shows `hasValue: false` with no eye or copy button. Pinned in Task 2 (`a_cleared_secret_field_keeps_its_label`) and Task 3 (`reveal_and_copy_of_an_empty_field_are_not_found`).

---

## File Structure

**Rust core (`crates/havenkeys-core`)**
- Create `src/custom_field.rs`: stored types (`FieldSection`, `CustomField`, `FieldValue`, `AddressValue`, `AddressPart`, `FieldKind`); input types (`SectionInput<O>`, `FieldInput<O>`, `FieldValueInput<O>`); views (`SectionView`, `FieldView`, `FieldValueView`, `views`); `clean_plain`, `apply_sections`; and per-field reads on `CustomField` (`concealed`, `totp_code`, `copy_value`, `url`). Unit tests are in the file.
- Modify `src/lib.rs`: `pub mod custom_field;`.
- Modify `src/model.rs`:
  - `ItemDetails::Login.sections` and `ItemInput.sections`;
  - `ItemInput::blank` sets `sections: None`;
  - `check_shape` refuses `sections` on a non-login;
  - `SecretUpdate::is_keep` becomes `pub(crate)`, and there is a new `SecretUpdate::apply_totp`.
- Modify `src/identity.rs`: `Allow` and `clean_value` become `pub(crate)`; a new `pub(crate) fn parse_ymd`; `check_birth_date` uses it.
- Modify `src/vault.rs`:
  - `build_item`'s login arm uses `apply_totp` and `apply_sections`;
  - new `login_sections` and `login_field`;
  - `sections: None` / `sections: Vec::new()` wherever the compiler asks.
- Modify `src/import/mod.rs`: `ImportReport.fields_to_notes`.
- Modify `src/import/onepux.rs`: the `LoginSections` collector, `custom_field_from`, and the changes to `login_fields` and `convert_item`.
- Tests:
  - create `tests/custom_fields.rs`;
  - modify `tests/writes.rs` (TOTP pin), `tests/identity.rs` (date pin) and `tests/fuzz.rs`;
  - add `sections: None` to every full `ItemInput { … }` literal in `tests/*.rs`, `crates/havenkeys-bridge/tests/bridge.rs` and `crates/havenkeys-sync-client/tests/round_trip.rs`.

**Desktop Rust (`apps/desktop/src-tauri`)**
- Create `src/custom_field.rs`: the commands `login_fields`, `reveal_login_field`, `login_field_totp`, `copy_login_field` and `open_login_field_url`.
- Modify `src/item_input.rs`: `sections`, plus a shared `resolve_totp`.
- Modify `src/lib.rs` (module and `generate_handler!`), `build.rs` (`COMMANDS`) and `capabilities/main.json`.

**Desktop UI (`apps/desktop/src`)**
- Modify `lib/types.ts`, `lib/api.ts`, `lib/hooks.ts` (`useTotp` takes a loader) and `lib/format.ts` (`formatDay`).
- Create `lib/customFields.ts` and `lib/customFields.test.ts`.
- Modify `lib/openItem.ts` (`EditorSnapshot.sections`) and `lib/openItem.test.ts`.
- Modify `i18n/en.ts` and `i18n/pt-BR.ts` (a new `fields` block and `import.fieldsToNotes`).
- Create `components/TotpEdit.tsx`: the one-time-code setup row, extracted from `ItemEditor.tsx` and used by the main TOTP and by OTP fields.
- Create `views/CustomSections.tsx` (detail) and `views/CustomFieldsEditor.tsx` (editor).
- Modify `views/ItemDetail.tsx`, `views/ItemEditor.tsx`, `views/ImportSection.tsx` and the stylesheet that holds `.edit-row` (find it with `grep -rln "\.edit-row" src`).

**Docs**: `docs/security-model.md`, `docs/crypto.md`, `docs/architecture.md`.

---

### Task 1: Shared helpers — `SecretUpdate::apply_totp` and `parse_ymd` (behaviour-preserving)

**Files:**
- Modify: `crates/havenkeys-core/src/model.rs` (`SecretUpdate` impl, ~line 190)
- Modify: `crates/havenkeys-core/src/vault.rs` (`build_item` login arm, ~line 1981)
- Modify: `crates/havenkeys-core/src/identity.rs` (`Allow`, `clean_value`, `check_birth_date`, ~lines 230–316)
- Test: `crates/havenkeys-core/tests/writes.rs` and `crates/havenkeys-core/tests/identity.rs`

**Interfaces:**
- Produces:
  - `impl SecretUpdate { pub(crate) fn is_keep(&self) -> bool; pub(crate) fn apply_totp(self, current: Option<TotpConfig>) -> Result<Option<TotpConfig>> }`
  - `pub(crate) enum Allow { Line, Text, Phone }`
  - `pub(crate) fn clean_value(value: Option<SecretString>, max_chars: usize, allow: Allow, what: &'static str) -> Result<Option<SecretString>>`
  - `pub(crate) fn parse_ymd(value: &str) -> Option<(i64, i64, i64)>`

- [ ] **Step 1: Create the branch**

```bash
git checkout -b login-custom-fields
```

- [ ] **Step 2: Write the pin test for the main TOTP (passes on today's code)**

Append to `crates/havenkeys-core/tests/writes.rs` (it already has `mod common; use common::*;`; add any missing `use` lines):

```rust
/// Pins how an update treats the main TOTP before `apply_totp` takes over
/// the inline match in `build_item` (spec 2026-09-30 §4.6).
#[test]
fn main_totp_updates_keep_clear_and_set() {
    use havenkeys_core::model::SecretUpdate;
    let (mut v, _) = activated_vault();
    let mut input = login("AWS", "root", "pw", "aws.amazon.com");
    input.totp = SecretUpdate::Set(secret("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"));
    let staged = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    assert_eq!(v.totp_code(&id, 59).unwrap().code.expose(), "287082");

    let with = |totp: SecretUpdate| {
        let mut i = login("AWS", "root", "pw", "aws.amazon.com");
        i.totp = totp;
        i
    };
    // Keep keeps.
    let staged = v.stage_update(&id, with(SecretUpdate::Keep), NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert_eq!(v.totp_code(&id, 59).unwrap().code.expose(), "287082");
    // A blank Set clears.
    let staged = v.stage_update(&id, with(SecretUpdate::Set(secret("   "))), NOW + 2).unwrap();
    let ov = v.commit_write(staged, 3).unwrap().unwrap();
    assert!(!ov.has_totp);
    // A bad Set is refused without echoing it.
    let err = v
        .stage_update(&id, with(SecretUpdate::Set(secret("not-base32-SECRETVALUE!"))), NOW + 3)
        .unwrap_err();
    assert!(!err.to_string().contains("SECRETVALUE"));
    // Set, then Clear.
    let staged = v
        .stage_update(&id, with(SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"))), NOW + 4)
        .unwrap();
    assert!(v.commit_write(staged, 4).unwrap().unwrap().has_totp);
    let staged = v.stage_update(&id, with(SecretUpdate::Clear), NOW + 5).unwrap();
    assert!(!v.commit_write(staged, 5).unwrap().unwrap().has_totp);
}
```

- [ ] **Step 3: Write the pin test for birth dates (passes on today's code)**

Look at `crates/havenkeys-core/tests/identity.rs` around line 150, which already refuses `2000-02-30`. Following that test's pattern for saving an identity, add `birth_dates_are_real_past_calendar_dates`. It should:
- accept `2000-02-29` and `1850-01-01`;
- refuse `1900-02-29`, `2000-13-01`, `2000-1-01`, `20000-01-01`, `1849-12-31`, `abcd-ef-gh` and a date after `NOW` (`2023-11-15` is after `NOW = 1_700_000_000_000`; use `2030-01-01`).

Run: `cargo test -p havenkeys-core --test writes main_totp_updates_keep_clear_and_set --test identity birth_dates_are_real_past_calendar_dates`
Expected: PASS on the unchanged code. If it fails, the test is wrong: fix the test, not the code.

- [ ] **Step 4: Add `apply_totp` and use it in `build_item`**

In `crates/havenkeys-core/src/model.rs`, add `use crate::totp::{self, TotpConfig};`; the existing `use crate::totp::TotpConfig;` becomes this. Then change the `impl SecretUpdate` block to:

```rust
impl SecretUpdate {
    pub(crate) fn is_keep(&self) -> bool {
        matches!(self, SecretUpdate::Keep)
    }

    /// Apply to an existing value; an empty `Set` is treated as `Clear`.
    pub(crate) fn apply(self, current: Option<SecretString>) -> Option<SecretString> {
        match self {
            SecretUpdate::Keep => current,
            SecretUpdate::Clear => None,
            SecretUpdate::Set(v) if v.is_empty() => None,
            SecretUpdate::Set(v) => Some(v),
        }
    }

    /// Apply to a one-time-password setup: `Set` takes an `otpauth://` URI
    /// or a bare Base32 secret; a blank `Set` clears. The one rule for the
    /// login's TOTP and for OTP custom fields.
    pub(crate) fn apply_totp(self, current: Option<TotpConfig>) -> Result<Option<TotpConfig>> {
        match self {
            SecretUpdate::Keep => Ok(current),
            SecretUpdate::Clear => Ok(None),
            SecretUpdate::Set(v) if v.expose().trim().is_empty() => Ok(None),
            SecretUpdate::Set(v) => totp::parse_totp_input(v.expose()).map(Some),
        }
    }
}
```

In `crates/havenkeys-core/src/vault.rs`, `build_item`, replace

```rust
            let totp = match totp {
                SecretUpdate::Keep => cur_totp,
                SecretUpdate::Clear => None,
                SecretUpdate::Set(v) if v.expose().trim().is_empty() => None,
                SecretUpdate::Set(v) => Some(totp::parse_totp_input(v.expose())?),
            };
```

with

```rust
            let totp = totp.apply_totp(cur_totp)?;
```

Remove any `use` that becomes unused (`cargo build` warns).

- [ ] **Step 5: Extract `parse_ymd` in `identity.rs`**

Make `enum Allow` and `fn clean_value` `pub(crate)`. Replace `check_birth_date` with:

```rust
/// `YYYY-MM-DD` as a real calendar date: `(year, month, day)`. The one
/// date check for identity birth dates and Date custom fields.
pub(crate) fn parse_ymd(value: &str) -> Option<(i64, i64, i64)> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = &value[range];
        if !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if y < 1 || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some((y, m, d))
}

/// `YYYY-MM-DD`, a real calendar date, from 1850 up to today (UTC).
fn check_birth_date(value: &str, now_ms: i64) -> Result<()> {
    const BAD: Error = Error::InvalidInput("birth date must be a past date as YYYY-MM-DD");
    let (y, m, d) = parse_ymd(value).ok_or(BAD)?;
    if y < MIN_BIRTH_YEAR || days_from_civil(y, m, d) > now_ms.div_euclid(86_400_000) {
        return Err(BAD);
    }
    Ok(())
}
```

- [ ] **Step 6: Run the whole core suite**

Run: `cargo test -p havenkeys-core`
Expected: all PASS, including both pin tests.

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-core
git commit -m "refactor(core): one TOTP update rule and one calendar-date check

SecretUpdate::apply_totp is the Keep/Clear/blank Set/Set→parse match that
build_item had inline for the login's TOTP; custom OTP fields will use it
too. parse_ymd is the YYYY-MM-DD calendar check behind check_birth_date,
for Date custom fields next. New tests pin the main TOTP's updates and the
birth-date rules; they pass on the code before this change."
```

---

### Task 2: Core `custom_field.rs` — types, views, validation

**Files:**
- Create: `crates/havenkeys-core/src/custom_field.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `pub mod custom_field;` after `pub mod crypto;`)

**Interfaces:**
- Consumes: from Task 1, `SecretUpdate::{is_keep, apply, apply_totp}`, `identity::{Allow, clean_value, parse_ymd}`; from `model`, `check_notes`, `check_password`, `normalize_url`, `MAX_TITLE_CHARS`; from `totp`, `generate`, `TotpCode`, `TotpConfig`.
- Produces (used by Tasks 3–5):
  - `pub struct FieldSection { pub id: Uuid, pub title: Option<SecretString>, pub fields: Vec<CustomField> }`
  - `pub struct CustomField { pub id: Uuid, pub label: SecretString, pub value: FieldValue }`
  - `pub enum FieldValue { Text(SecretString), Url(SecretString), Email(SecretString), Phone(SecretString), Date(SecretString), Address(Box<AddressValue>), Password(Option<SecretString>), Otp(Option<TotpConfig>) }`
  - `pub struct AddressValue { pub street, number, complement, neighborhood, city, state, postal_code, country: Option<SecretString> }`
  - `pub enum AddressPart { Street, Number, Complement, Neighborhood, City, State, PostalCode, Country }`
  - `pub enum FieldKind { Text, Url, Email, Phone, Date, Address, Password, Otp }`
  - `pub struct SectionInput<O = SecretUpdate> { pub id: Option<Uuid>, pub title: Option<SecretString>, pub fields: Vec<FieldInput<O>> }`
  - `pub struct FieldInput<O = SecretUpdate> { pub id: Option<Uuid>, pub label: SecretString, pub value: FieldValueInput<O> }`
  - `pub enum FieldValueInput<O = SecretUpdate> { Text(SecretString), Url(SecretString), Email(SecretString), Phone(SecretString), Date(SecretString), Address(Box<AddressValue>), Password(SecretUpdate), Otp(O) }`
  - `SectionInput::<O>::try_map_otp<P, E, F: FnMut(O) -> Result<P, E>>(self, f: &mut F) -> Result<SectionInput<P>, E>` and `SectionInput::<O>::otp_updates(&self) -> impl Iterator<Item = &O>`
  - `pub fn views(sections: Vec<FieldSection>) -> Vec<SectionView>`
  - `pub(crate) fn clean_plain(value: &FieldValueInput) -> Result<FieldValue>`
  - `pub(crate) fn apply_sections(input: Option<Vec<SectionInput>>, current: Vec<FieldSection>) -> Result<Vec<FieldSection>>`
  - `CustomField::{concealed(&self) -> Result<SecretString>, totp_code(&self, unix_seconds: u64) -> Result<TotpCode>, copy_value(&self, part: Option<AddressPart>, unix_seconds: u64) -> Result<SecretString>, url(&self) -> Result<String>}`
  - `FieldValue::kind(&self) -> FieldKind` and `FieldValueInput::<O>::kind(&self) -> FieldKind`
  - constants `MAX_SECTIONS = 20`, `MAX_FIELDS = 100`, `MAX_LABEL_CHARS = MAX_TITLE_CHARS`, `MAX_SHORT_VALUE_CHARS = 512`, `MAX_TOTAL_BYTES = 256 * 1024`

- [ ] **Step 1: Write the module with its tests**

Create `crates/havenkeys-core/src/custom_field.rs`:

```rust
//! A login's custom fields (spec 2026-09-30-login-custom-fields).
//!
//! Sections of labelled, typed fields inside the login's encrypted details;
//! nothing here is in the overview. Plain values reach the UI when a login
//! opens (as notes do). A Password value leaves the core only on an explicit
//! reveal or copy; an OTP secret never does, only its codes.

use crate::error::{Error, Result};
use crate::identity::{clean_value, parse_ymd, Allow};
use crate::model::{check_notes, check_password, normalize_url, SecretUpdate, MAX_TITLE_CHARS};
use crate::secret::SecretString;
use crate::totp::{self, TotpCode, TotpConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroize;

pub const MAX_SECTIONS: usize = 20;
pub const MAX_FIELDS: usize = 100;
pub const MAX_LABEL_CHARS: usize = MAX_TITLE_CHARS;
pub const MAX_SHORT_VALUE_CHARS: usize = 512;
/// Every value, label and title of one login together, in bytes.
pub const MAX_TOTAL_BYTES: usize = 256 * 1024;

/// An id that is not one of this login's fields or sections, sent twice, or
/// a kept value on a new field or with a changed type. Never names a value.
const BAD_LAYOUT: Error =
    Error::InvalidInput("the login's fields changed; reopen it and try again");

// ---------------------------------------------------------------- stored

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSection {
    pub id: Uuid,
    /// `None`: an untitled section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<SecretString>,
    #[serde(default)]
    pub fields: Vec<CustomField>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    pub id: Uuid,
    pub label: SecretString,
    pub value: FieldValue,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FieldValue {
    Text(SecretString),
    Url(SecretString),
    Email(SecretString),
    Phone(SecretString),
    /// `YYYY-MM-DD`, or empty.
    Date(SecretString),
    Address(Box<AddressValue>),
    /// `None`: cleared; the field and its label stay.
    Password(Option<SecretString>),
    /// `None`: cleared. The secret never leaves the core.
    Otp(Option<TotpConfig>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Url,
    Email,
    Phone,
    Date,
    Address,
    Password,
    Otp,
}

/// The same parts as the Identity's address, so a later "Copy street" menu
/// needs no format change.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddressValue {
    #[serde(default)]
    pub street: Option<SecretString>,
    #[serde(default)]
    pub number: Option<SecretString>,
    #[serde(default)]
    pub complement: Option<SecretString>,
    #[serde(default)]
    pub neighborhood: Option<SecretString>,
    #[serde(default)]
    pub city: Option<SecretString>,
    #[serde(default)]
    pub state: Option<SecretString>,
    #[serde(default)]
    pub postal_code: Option<SecretString>,
    #[serde(default)]
    pub country: Option<SecretString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddressPart {
    Street,
    Number,
    Complement,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
}

impl AddressValue {
    pub fn part(&self, part: AddressPart) -> Option<&SecretString> {
        match part {
            AddressPart::Street => self.street.as_ref(),
            AddressPart::Number => self.number.as_ref(),
            AddressPart::Complement => self.complement.as_ref(),
            AddressPart::Neighborhood => self.neighborhood.as_ref(),
            AddressPart::City => self.city.as_ref(),
            AddressPart::State => self.state.as_ref(),
            AddressPart::PostalCode => self.postal_code.as_ref(),
            AddressPart::Country => self.country.as_ref(),
        }
    }

    /// One line each: "street, number", complement, neighborhood,
    /// "city - state", postal code, country; empty ones left out. The one
    /// address format, for the detail view and for Copy.
    pub fn formatted(&self) -> SecretString {
        fn pair(a: &Option<SecretString>, sep: &str, b: &Option<SecretString>) -> Option<String> {
            match (a, b) {
                (Some(a), Some(b)) => Some(format!("{}{sep}{}", a.expose(), b.expose())),
                (Some(x), None) | (None, Some(x)) => Some(x.expose().to_owned()),
                (None, None) => None,
            }
        }
        let one = |v: &Option<SecretString>| v.as_ref().map(|s| s.expose().to_owned());
        let mut lines: Vec<String> = [
            pair(&self.street, ", ", &self.number),
            one(&self.complement),
            one(&self.neighborhood),
            pair(&self.city, " - ", &self.state),
            one(&self.postal_code),
            one(&self.country),
        ]
        .into_iter()
        .flatten()
        .collect();
        let out = SecretString::new(lines.join("\n"));
        lines.iter_mut().for_each(|l| l.zeroize());
        out
    }

    fn clean(&self) -> Result<Self> {
        let c = |v: &Option<SecretString>| {
            clean_value(
                v.clone(),
                MAX_SHORT_VALUE_CHARS,
                Allow::Line,
                "address is too long or contains control characters",
            )
        };
        Ok(Self {
            street: c(&self.street)?,
            number: c(&self.number)?,
            complement: c(&self.complement)?,
            neighborhood: c(&self.neighborhood)?,
            city: c(&self.city)?,
            state: c(&self.state)?,
            postal_code: c(&self.postal_code)?,
            country: c(&self.country)?,
        })
    }
}

impl FieldValue {
    pub fn kind(&self) -> FieldKind {
        match self {
            FieldValue::Text(_) => FieldKind::Text,
            FieldValue::Url(_) => FieldKind::Url,
            FieldValue::Email(_) => FieldKind::Email,
            FieldValue::Phone(_) => FieldKind::Phone,
            FieldValue::Date(_) => FieldKind::Date,
            FieldValue::Address(_) => FieldKind::Address,
            FieldValue::Password(_) => FieldKind::Password,
            FieldValue::Otp(_) => FieldKind::Otp,
        }
    }

    fn byte_len(&self) -> usize {
        match self {
            FieldValue::Text(v)
            | FieldValue::Url(v)
            | FieldValue::Email(v)
            | FieldValue::Phone(v)
            | FieldValue::Date(v) => v.expose().len(),
            FieldValue::Address(a) => a.formatted().expose().len(),
            FieldValue::Password(p) => p.as_ref().map_or(0, |p| p.expose().len()),
            FieldValue::Otp(c) => c.as_ref().map_or(0, |c| c.secret.expose().len()),
        }
    }
}

impl CustomField {
    /// A Password field's value, for an explicit reveal.
    pub fn concealed(&self) -> Result<SecretString> {
        match &self.value {
            FieldValue::Password(Some(p)) => Ok(p.clone()),
            FieldValue::Password(None) => Err(Error::NotFound),
            _ => Err(Error::InvalidInput("only a password field can be revealed")),
        }
    }

    /// The current code of an OTP field.
    pub fn totp_code(&self, unix_seconds: u64) -> Result<TotpCode> {
        match &self.value {
            FieldValue::Otp(Some(cfg)) => totp::generate(cfg, unix_seconds),
            FieldValue::Otp(None) => Err(Error::NotFound),
            _ => Err(Error::InvalidInput("not a one-time password field")),
        }
    }

    /// What Copy puts on the clipboard: the value, an OTP field's current
    /// code, an address formatted or one of its parts. `NotFound` when empty.
    pub fn copy_value(&self, part: Option<AddressPart>, unix_seconds: u64) -> Result<SecretString> {
        let value = match (&self.value, part) {
            (FieldValue::Address(a), Some(p)) => a.part(p).cloned(),
            (FieldValue::Address(a), None) => Some(a.formatted()),
            (_, Some(_)) => return Err(Error::InvalidInput("only an address has parts")),
            (
                FieldValue::Text(v)
                | FieldValue::Url(v)
                | FieldValue::Email(v)
                | FieldValue::Phone(v)
                | FieldValue::Date(v),
                None,
            ) => Some(v.clone()),
            (FieldValue::Password(p), None) => p.clone(),
            (FieldValue::Otp(_), None) => return self.totp_code(unix_seconds).map(|c| c.code),
        };
        value.filter(|v| !v.is_empty()).ok_or(Error::NotFound)
    }

    /// A URL field's address, normalised again before the OS opens it.
    pub fn url(&self) -> Result<String> {
        match &self.value {
            FieldValue::Url(v) if !v.is_empty() => normalize_url(v.expose()),
            FieldValue::Url(_) => Err(Error::NotFound),
            _ => Err(Error::InvalidInput("not a URL field")),
        }
    }
}

impl fmt::Debug for FieldSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldSection")
            .field("id", &self.id)
            .field("fields", &self.fields)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for CustomField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomField")
            .field("id", &self.id)
            .field("kind", &self.value.kind())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}(<redacted>)", self.kind())
    }
}

impl fmt::Debug for AddressValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AddressValue(<redacted>)")
    }
}

// ---------------------------------------------------------------- input

/// A section as the editor sends it. `O` is how an OTP field's setup is
/// sent: `SecretUpdate` in the core; the desktop's wire type adds scanned
/// QR tokens and maps them with `try_map_otp`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionInput<O = SecretUpdate> {
    /// `None`: a new section.
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub title: Option<SecretString>,
    #[serde(default = "Vec::new")]
    pub fields: Vec<FieldInput<O>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldInput<O = SecretUpdate> {
    /// `None`: a new field.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub label: SecretString,
    pub value: FieldValueInput<O>,
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldValueInput<O = SecretUpdate> {
    Text(SecretString),
    Url(SecretString),
    Email(SecretString),
    Phone(SecretString),
    Date(SecretString),
    Address(Box<AddressValue>),
    Password(SecretUpdate),
    Otp(O),
}

impl<O> SectionInput<O> {
    pub fn try_map_otp<P, E, F: FnMut(O) -> std::result::Result<P, E>>(
        self,
        f: &mut F,
    ) -> std::result::Result<SectionInput<P>, E> {
        let fields = self
            .fields
            .into_iter()
            .map(|field| {
                Ok(FieldInput {
                    id: field.id,
                    label: field.label,
                    value: field.value.try_map_otp(&mut *f)?,
                })
            })
            .collect::<std::result::Result<Vec<_>, E>>()?;
        Ok(SectionInput {
            id: self.id,
            title: self.title,
            fields,
        })
    }

    pub fn otp_updates(&self) -> impl Iterator<Item = &O> {
        self.fields.iter().filter_map(|f| match &f.value {
            FieldValueInput::Otp(o) => Some(o),
            _ => None,
        })
    }
}

impl<O> FieldValueInput<O> {
    pub fn kind(&self) -> FieldKind {
        match self {
            FieldValueInput::Text(_) => FieldKind::Text,
            FieldValueInput::Url(_) => FieldKind::Url,
            FieldValueInput::Email(_) => FieldKind::Email,
            FieldValueInput::Phone(_) => FieldKind::Phone,
            FieldValueInput::Date(_) => FieldKind::Date,
            FieldValueInput::Address(_) => FieldKind::Address,
            FieldValueInput::Password(_) => FieldKind::Password,
            FieldValueInput::Otp(_) => FieldKind::Otp,
        }
    }

    pub fn try_map_otp<P, E, F: FnMut(O) -> std::result::Result<P, E>>(
        self,
        f: &mut F,
    ) -> std::result::Result<FieldValueInput<P>, E> {
        Ok(match self {
            FieldValueInput::Text(v) => FieldValueInput::Text(v),
            FieldValueInput::Url(v) => FieldValueInput::Url(v),
            FieldValueInput::Email(v) => FieldValueInput::Email(v),
            FieldValueInput::Phone(v) => FieldValueInput::Phone(v),
            FieldValueInput::Date(v) => FieldValueInput::Date(v),
            FieldValueInput::Address(a) => FieldValueInput::Address(a),
            FieldValueInput::Password(u) => FieldValueInput::Password(u),
            FieldValueInput::Otp(o) => FieldValueInput::Otp(f(o)?),
        })
    }
}

impl<O> fmt::Debug for SectionInput<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionInput")
            .field("id", &self.id)
            .field("fields", &self.fields.len())
            .finish_non_exhaustive()
    }
}

impl<O> fmt::Debug for FieldInput<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldInput")
            .field("id", &self.id)
            .field("kind", &self.value.kind())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------- views

/// A section as the UI receives it when a login opens.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    pub id: Uuid,
    pub title: Option<SecretString>,
    pub fields: Vec<FieldView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    pub id: Uuid,
    pub label: SecretString,
    #[serde(flatten)]
    pub value: FieldValueView,
}

/// Plain values as they are; a Password or OTP field only says whether it
/// holds something. This type cannot carry a concealed value.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldValueView {
    Text { value: SecretString },
    Url { value: SecretString },
    Email { value: SecretString },
    Phone { value: SecretString },
    Date { value: SecretString },
    Address { parts: Box<AddressValue>, formatted: SecretString },
    Password {
        #[serde(rename = "hasValue")]
        has_value: bool,
    },
    Otp {
        #[serde(rename = "hasOtp")]
        has_otp: bool,
    },
}

pub fn views(sections: Vec<FieldSection>) -> Vec<SectionView> {
    sections
        .into_iter()
        .map(|s| SectionView {
            id: s.id,
            title: s.title,
            fields: s
                .fields
                .into_iter()
                .map(|f| FieldView {
                    id: f.id,
                    label: f.label,
                    value: match f.value {
                        FieldValue::Text(value) => FieldValueView::Text { value },
                        FieldValue::Url(value) => FieldValueView::Url { value },
                        FieldValue::Email(value) => FieldValueView::Email { value },
                        FieldValue::Phone(value) => FieldValueView::Phone { value },
                        FieldValue::Date(value) => FieldValueView::Date { value },
                        FieldValue::Address(parts) => {
                            let formatted = parts.formatted();
                            FieldValueView::Address { parts, formatted }
                        }
                        FieldValue::Password(p) => FieldValueView::Password {
                            has_value: p.is_some(),
                        },
                        FieldValue::Otp(c) => FieldValueView::Otp {
                            has_otp: c.is_some(),
                        },
                    },
                })
                .collect(),
        })
        .collect()
}

// ---------------------------------------------------------------- validation

/// Check one plain (not Password or OTP) value. Used by `apply_sections`,
/// and by the importer to decide whether a value keeps its type or becomes
/// Text.
pub(crate) fn clean_plain(value: &FieldValueInput) -> Result<FieldValue> {
    let short = |v: &SecretString, what: &'static str| -> Result<SecretString> {
        Ok(clean_value(Some(v.clone()), MAX_SHORT_VALUE_CHARS, Allow::Line, what)?
            .unwrap_or_default())
    };
    Ok(match value {
        FieldValueInput::Text(v) => {
            check_notes(v)?;
            FieldValue::Text(v.clone())
        }
        FieldValueInput::Url(v) if v.expose().trim().is_empty() => {
            FieldValue::Url(SecretString::default())
        }
        FieldValueInput::Url(v) => FieldValue::Url(SecretString::new(normalize_url(v.expose())?)),
        FieldValueInput::Email(v) => FieldValue::Email(short(
            v,
            "email is too long or contains control characters",
        )?),
        FieldValueInput::Phone(v) => FieldValue::Phone(short(
            v,
            "phone is too long or contains control characters",
        )?),
        FieldValueInput::Date(v) => {
            const BAD: Error = Error::InvalidInput("date must be YYYY-MM-DD");
            let v = short(v, "date must be YYYY-MM-DD")?;
            if !v.is_empty() && parse_ymd(v.expose()).is_none() {
                return Err(BAD);
            }
            FieldValue::Date(v)
        }
        FieldValueInput::Address(a) => FieldValue::Address(Box::new(a.clean()?)),
        FieldValueInput::Password(_) | FieldValueInput::Otp(_) => {
            return Err(Error::InvalidInput("not a plain value"))
        }
    })
}

/// A login's new fields from the editor's layout (spec §4.5). `None` keeps
/// `current` as it is. Ids are looked up only among `current`, so a save
/// can never reach another login's secrets.
pub(crate) fn apply_sections(
    input: Option<Vec<SectionInput>>,
    current: Vec<FieldSection>,
) -> Result<Vec<FieldSection>> {
    let Some(input) = input else {
        return Ok(current);
    };
    if input.len() > MAX_SECTIONS {
        return Err(Error::InvalidInput("too many sections"));
    }
    if input.iter().map(|s| s.fields.len()).sum::<usize>() > MAX_FIELDS {
        return Err(Error::InvalidInput("too many fields"));
    }
    let mut section_ids: HashSet<Uuid> = current.iter().map(|s| s.id).collect();
    let mut prior: HashMap<Uuid, FieldValue> = current
        .into_iter()
        .flat_map(|s| s.fields)
        .map(|f| (f.id, f.value))
        .collect();

    let mut out = Vec::with_capacity(input.len());
    for section in input {
        let id = match section.id {
            None => Uuid::new_v4(),
            Some(id) if section_ids.remove(&id) => id,
            Some(_) => return Err(BAD_LAYOUT),
        };
        let title = clean_value(
            section.title,
            MAX_LABEL_CHARS,
            Allow::Line,
            "section title is too long or contains control characters",
        )?;
        let mut fields = Vec::with_capacity(section.fields.len());
        for field in section.fields {
            let (id, before) = match field.id {
                None => (Uuid::new_v4(), None),
                // `remove`: an id sent twice is unknown the second time.
                Some(id) => (id, Some(prior.remove(&id).ok_or(BAD_LAYOUT)?)),
            };
            if before.as_ref().is_some_and(|b| b.kind() != field.value.kind()) {
                return Err(BAD_LAYOUT);
            }
            let label = clean_value(
                Some(field.label),
                MAX_LABEL_CHARS,
                Allow::Line,
                "field label is too long or contains control characters",
            )?
            .ok_or(Error::InvalidInput("a field needs a label"))?;
            let value = match field.value {
                FieldValueInput::Password(update) => {
                    if before.is_none() && update.is_keep() {
                        return Err(BAD_LAYOUT);
                    }
                    let current = match before {
                        Some(FieldValue::Password(p)) => p,
                        _ => None,
                    };
                    let p = update.apply(current);
                    if let Some(p) = &p {
                        check_password(p)?;
                    }
                    FieldValue::Password(p)
                }
                FieldValueInput::Otp(update) => {
                    if before.is_none() && update.is_keep() {
                        return Err(BAD_LAYOUT);
                    }
                    let current = match before {
                        Some(FieldValue::Otp(c)) => c,
                        _ => None,
                    };
                    FieldValue::Otp(update.apply_totp(current)?)
                }
                plain => clean_plain(&plain)?,
            };
            fields.push(CustomField { id, label, value });
        }
        out.push(FieldSection { id, title, fields });
    }

    let total: usize = out
        .iter()
        .map(|s| {
            s.title.as_ref().map_or(0, |t| t.expose().len())
                + s.fields
                    .iter()
                    .map(|f| f.label.expose().len() + f.value.byte_len())
                    .sum::<usize>()
        })
        .sum();
    if total > MAX_TOTAL_BYTES {
        return Err(Error::InvalidInput("the login's fields are too large"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    fn s(v: &str) -> SecretString {
        SecretString::from(v)
    }

    fn field(id: Option<Uuid>, label: &str, value: FieldValueInput) -> FieldInput {
        FieldInput {
            id,
            label: s(label),
            value,
        }
    }

    fn section(id: Option<Uuid>, title: Option<&str>, fields: Vec<FieldInput>) -> SectionInput {
        SectionInput {
            id,
            title: title.map(s),
            fields,
        }
    }

    /// One section "Bank" with a PIN (Password), an OTP and a Text field.
    fn bank() -> Vec<FieldSection> {
        apply_sections(
            Some(vec![section(
                None,
                Some("Bank"),
                vec![
                    field(None, "PIN", FieldValueInput::Password(SecretUpdate::Set(s("4321")))),
                    field(None, "Token", FieldValueInput::Otp(SecretUpdate::Set(s(RFC_SECRET)))),
                    field(None, "Account", FieldValueInput::Text(s("12345-6"))),
                ],
            )]),
            Vec::new(),
        )
        .unwrap()
    }

    fn ids(sections: &[FieldSection]) -> (Uuid, Uuid, Uuid, Uuid) {
        let sec = &sections[0];
        (sec.id, sec.fields[0].id, sec.fields[1].id, sec.fields[2].id)
    }

    #[test]
    fn none_keeps_the_current_fields() {
        let current = bank();
        let (sid, ..) = ids(&current);
        let kept = apply_sections(None, current).unwrap();
        assert_eq!(kept[0].id, sid);
        assert_eq!(kept[0].fields.len(), 3);
    }

    #[test]
    fn new_fields_get_ids_and_keep_order() {
        let out = bank();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title.as_ref().unwrap().expose(), "Bank");
        let labels: Vec<&str> = out[0].fields.iter().map(|f| f.label.expose()).collect();
        assert_eq!(labels, ["PIN", "Token", "Account"]);
        assert_ne!(out[0].fields[0].id, out[0].fields[1].id);
    }

    #[test]
    fn keep_keeps_and_reorder_is_the_list_order() {
        let (sid, pin, token, account) = ids(&bank());
        let out = apply_sections(
            Some(vec![section(
                Some(sid),
                Some("Bank"),
                vec![
                    field(Some(account), "Account", FieldValueInput::Text(s("12345-6"))),
                    field(Some(token), "Token", FieldValueInput::Otp(SecretUpdate::Keep)),
                    field(Some(pin), "PIN", FieldValueInput::Password(SecretUpdate::Keep)),
                ],
            )]),
            bank_with_ids(sid, pin, token, account),
        )
        .unwrap();
        let f = &out[0].fields;
        assert_eq!(f[0].id, account);
        assert_eq!(f[2].concealed().unwrap().expose(), "4321");
        assert_eq!(f[1].totp_code(59).unwrap().code.expose(), "287082");
    }

    /// `bank()` again with fixed ids, so a test can refer to them.
    fn bank_with_ids(sid: Uuid, pin: Uuid, token: Uuid, account: Uuid) -> Vec<FieldSection> {
        let mut b = bank();
        b[0].id = sid;
        b[0].fields[0].id = pin;
        b[0].fields[1].id = token;
        b[0].fields[2].id = account;
        b
    }

    #[test]
    fn a_field_moved_to_another_section_keeps_its_secret() {
        let (sid, pin, token, account) = ids(&bank());
        let current = bank_with_ids(sid, pin, token, account);
        let out = apply_sections(
            Some(vec![
                section(
                    Some(sid),
                    Some("Bank"),
                    vec![field(Some(account), "Account", FieldValueInput::Text(s("12345-6")))],
                ),
                section(
                    None,
                    Some("Secrets"),
                    vec![
                        field(Some(pin), "PIN", FieldValueInput::Password(SecretUpdate::Keep)),
                        field(Some(token), "Token", FieldValueInput::Otp(SecretUpdate::Keep)),
                    ],
                ),
            ]),
            current,
        )
        .unwrap();
        assert_eq!(out[1].fields[0].concealed().unwrap().expose(), "4321");
        assert!(out[1].fields[1].totp_code(59).is_ok());
    }

    #[test]
    fn unknown_duplicate_or_retyped_ids_are_refused() {
        let (sid, pin, token, account) = ids(&bank());
        let current = || bank_with_ids(sid, pin, token, account);
        let keep_pin = |id| field(Some(id), "PIN", FieldValueInput::Password(SecretUpdate::Keep));
        // A field id this login does not have (e.g. another login's).
        let other = Uuid::new_v4();
        let one = |f| Some(vec![section(Some(sid), None, vec![f])]);
        assert_eq!(apply_sections(one(keep_pin(other)), current()).err(), Some(BAD_LAYOUT));
        // The same field twice.
        let twice = Some(vec![section(Some(sid), None, vec![keep_pin(pin), keep_pin(pin)])]);
        assert_eq!(apply_sections(twice, current()).err(), Some(BAD_LAYOUT));
        // An OTP id sent as Password (would turn a hidden secret revealable).
        assert_eq!(apply_sections(one(keep_pin(token)), current()).err(), Some(BAD_LAYOUT));
        // A Password id sent as Text with a new value is still a type change.
        let retyped = field(Some(pin), "PIN", FieldValueInput::Text(s("x")));
        assert_eq!(apply_sections(one(retyped), current()).err(), Some(BAD_LAYOUT));
        // An unknown section id.
        let bad_section = Some(vec![section(Some(other), None, vec![])]);
        assert_eq!(apply_sections(bad_section, current()).err(), Some(BAD_LAYOUT));
        // Keep on a new field.
        let new_keep = field(None, "PIN", FieldValueInput::Password(SecretUpdate::Keep));
        assert_eq!(
            apply_sections(Some(vec![section(None, None, vec![new_keep])]), Vec::new()).err(),
            Some(BAD_LAYOUT)
        );
    }

    #[test]
    fn a_cleared_secret_field_keeps_its_label() {
        let (sid, pin, token, account) = ids(&bank());
        let out = apply_sections(
            Some(vec![section(
                Some(sid),
                None,
                vec![
                    field(Some(pin), "PIN", FieldValueInput::Password(SecretUpdate::Clear)),
                    field(Some(token), "Token", FieldValueInput::Otp(SecretUpdate::Set(s("  ")))),
                ],
            )]),
            bank_with_ids(sid, pin, token, account),
        )
        .unwrap();
        assert_eq!(out[0].fields[0].label.expose(), "PIN");
        assert_eq!(out[0].fields[0].concealed().err(), Some(Error::NotFound));
        assert_eq!(out[0].fields[1].totp_code(59).err(), Some(Error::NotFound));
        let v = serde_json::to_value(views(out)).unwrap();
        assert_eq!(v[0]["fields"][0]["hasValue"], false);
        assert_eq!(v[0]["fields"][1]["hasOtp"], false);
    }

    #[test]
    fn values_are_checked_per_type() {
        let one = |value| apply_sections(Some(vec![section(None, None, vec![field(None, "x", value)])]), Vec::new());
        assert!(one(FieldValueInput::Url(s("javascript:alert(1)"))).is_err());
        let url = one(FieldValueInput::Url(s("example.com"))).unwrap();
        assert_eq!(url[0].fields[0].url().unwrap(), "https://example.com/");
        assert!(one(FieldValueInput::Url(s(""))).is_ok());
        assert!(one(FieldValueInput::Date(s("2026-02-30"))).is_err());
        assert!(one(FieldValueInput::Date(s("2024-02-29"))).is_ok());
        assert!(one(FieldValueInput::Date(s(""))).is_ok());
        assert!(one(FieldValueInput::Email(s("a\u{0}b"))).is_err());
        assert!(one(FieldValueInput::Phone(s(&"9".repeat(513)))).is_err());
        assert!(one(FieldValueInput::Otp(SecretUpdate::Set(s("not base32!")))).is_err());
        assert!(one(FieldValueInput::Text(s(&"a".repeat(64 * 1024 + 1)))).is_err());
        let no_label = apply_sections(
            Some(vec![section(None, None, vec![field(None, "  ", FieldValueInput::Text(s("x")))])]),
            Vec::new(),
        );
        assert_eq!(no_label.err(), Some(Error::InvalidInput("a field needs a label")));
    }

    #[test]
    fn limits() {
        let text = || field(None, "x", FieldValueInput::Text(s("x")));
        let sections = (0..21).map(|_| section(None, None, vec![])).collect();
        assert!(apply_sections(Some(sections), Vec::new()).is_err());
        let many = vec![section(None, None, (0..101).map(|_| text()).collect())];
        assert!(apply_sections(Some(many), Vec::new()).is_err());
        let big = (0..5)
            .map(|_| field(None, "x", FieldValueInput::Text(s(&"a".repeat(60 * 1024)))))
            .collect();
        assert_eq!(
            apply_sections(Some(vec![section(None, None, big)]), Vec::new()).err(),
            Some(Error::InvalidInput("the login's fields are too large"))
        );
    }

    #[test]
    fn views_never_carry_concealed_values() {
        let json = serde_json::to_string(&views(bank())).unwrap();
        assert!(!json.contains("4321"));
        assert!(!json.contains(RFC_SECRET));
        assert!(json.contains("\"hasValue\":true"));
        assert!(json.contains("\"hasOtp\":true"));
        assert!(json.contains("12345-6"), "plain values are shown");
    }

    #[test]
    fn address_format_parts_and_copy() {
        let addr = AddressValue {
            street: Some(s("Rua A")),
            number: Some(s("10")),
            city: Some(s("São Paulo")),
            state: Some(s("SP")),
            postal_code: Some(s("01000-000")),
            ..AddressValue::default()
        };
        let out = apply_sections(
            Some(vec![section(None, None, vec![field(None, "Home", FieldValueInput::Address(Box::new(addr)))])]),
            Vec::new(),
        )
        .unwrap();
        let f = &out[0].fields[0];
        assert_eq!(
            f.copy_value(None, 0).unwrap().expose(),
            "Rua A, 10\nSão Paulo - SP\n01000-000"
        );
        assert_eq!(f.copy_value(Some(AddressPart::City), 0).unwrap().expose(), "São Paulo");
        assert_eq!(f.copy_value(Some(AddressPart::Country), 0).err(), Some(Error::NotFound));
    }

    #[test]
    fn wrong_operations_are_invalid_input() {
        let b = bank();
        let (pin, token, text) = (&b[0].fields[0], &b[0].fields[1], &b[0].fields[2]);
        assert!(matches!(text.concealed(), Err(Error::InvalidInput(_))));
        assert!(matches!(token.concealed(), Err(Error::InvalidInput(_))));
        assert!(matches!(pin.totp_code(59), Err(Error::InvalidInput(_))));
        assert!(matches!(pin.copy_value(Some(AddressPart::City), 0), Err(Error::InvalidInput(_))));
        assert!(matches!(text.url(), Err(Error::InvalidInput(_))));
        assert_eq!(token.copy_value(None, 59).unwrap().expose(), "287082");
        assert_eq!(pin.copy_value(None, 0).unwrap().expose(), "4321");
    }

    #[test]
    fn debug_never_prints_labels_or_values() {
        let b = bank();
        let dbg = format!("{b:?}");
        for secret in ["Bank", "PIN", "4321", "Token", RFC_SECRET, "Account", "12345-6"] {
            assert!(!dbg.contains(secret), "Debug printed {secret}");
        }
        let input = section(None, Some("Bank"), vec![field(None, "PIN", FieldValueInput::Text(s("4321")))]);
        let dbg = format!("{input:?}");
        assert!(!dbg.contains("Bank") && !dbg.contains("4321"));
    }

    #[test]
    fn stored_form_round_trips() {
        let json = serde_json::to_string(&bank()).unwrap();
        let back: Vec<FieldSection> = serde_json::from_str(&json).unwrap();
        assert_eq!(back[0].fields[0].concealed().unwrap().expose(), "4321");
        assert!(json.contains("\"type\":\"password\""));
    }

    #[test]
    fn input_json_shape() {
        let input: SectionInput = serde_json::from_value(serde_json::json!({
            "title": "Bank",
            "fields": [
                {"label": "PIN", "value": {"type": "password", "value": {"op": "set", "value": "1"}}},
                {"label": "Home", "value": {"type": "address", "value": {"street": "Rua A", "postalCode": "1"}}},
                {"label": "Day", "value": {"type": "date", "value": "2026-09-30"}}
            ]
        }))
        .unwrap();
        assert_eq!(input.fields.len(), 3);
        let bad = serde_json::from_value::<SectionInput>(serde_json::json!({
            "fields": [{"label": "x", "value": {"type": "text", "value": "y"}, "extra": 1}]
        }));
        assert!(bad.is_err(), "unknown keys are refused");
    }
}
```

Add `pub mod custom_field;` to `crates/havenkeys-core/src/lib.rs`.

- [ ] **Step 2: Run the module tests**

Run: `cargo test -p havenkeys-core --lib custom_field`
Expected: PASS. The module does not depend on `vault.rs` yet, so it compiles on its own. If the compiler asks for `check_notes`, `check_password` or `normalize_url` to be visible, they are already `pub(crate)`/`pub` in `model.rs`. If a `SecretString`-returning closure borrows, clone as shown.

- [ ] **Step 3: Lint**

Run: `cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: no warnings. `FieldValue` and `FieldValueInput` are `pub` but are not used outside tests yet. If the compiler warns about dead code in `clean_plain`/`apply_sections`, add `#[allow(dead_code)]` only until Task 3; Task 3 removes it.

- [ ] **Step 4: Commit**

```bash
git add crates/havenkeys-core/src/custom_field.rs crates/havenkeys-core/src/lib.rs
git commit -m "feat(core): custom field types, views and layout validation

custom_field.rs holds a login's sections of typed fields, the editor's
input (generic over how an OTP setup is sent), the view the UI gets
(Password and OTP reduced to flags) and apply_sections: ids from this
login only, each once, same type for Keep, per-type checks and limits."
```

---

### Task 3: Core — sections in the login, vault reads, save paths keep them

**Files:**
- Modify: `crates/havenkeys-core/src/model.rs` (`ItemDetails::Login`, `ItemInput`, `blank`, `check_shape`)
- Modify: `crates/havenkeys-core/src/vault.rs` (`build_item`, a new "custom fields" block after `card_value`)
- Modify: every file the compiler names (see Step 4)
- Create: `crates/havenkeys-core/tests/custom_fields.rs`
- Modify: `crates/havenkeys-core/tests/fuzz.rs`

**Interfaces:**
- Consumes: Task 2's `FieldSection`, `CustomField`, `SectionInput`, `apply_sections`.
- Produces (used by Tasks 4–5):
  - `ItemDetails::Login { …, sections: Vec<FieldSection> }` and `ItemInput.sections: Option<Vec<SectionInput>>`
  - `VaultService::login_sections(&self, id: &Uuid) -> Result<Vec<FieldSection>>`
  - `VaultService::login_field(&self, id: &Uuid, field_id: &Uuid) -> Result<CustomField>`

- [ ] **Step 1: Write the integration tests**

Create `crates/havenkeys-core/tests/custom_fields.rs`:

```rust
//! A login's custom fields in the vault (spec 2026-09-30-login-custom-fields).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::custom_field::{AddressPart, FieldInput, FieldValueInput, SectionInput};
use havenkeys_core::model::{ItemInput, SecretUpdate};
use havenkeys_core::vault::{SaveTarget, VaultService};
use havenkeys_core::Error;
use uuid::Uuid;

const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

fn field(id: Option<Uuid>, label: &str, value: FieldValueInput) -> FieldInput {
    FieldInput { id, label: secret(label), value }
}

fn bank_sections() -> Vec<SectionInput> {
    vec![SectionInput {
        id: None,
        title: Some(secret("Bank")),
        fields: vec![
            field(None, "PIN", FieldValueInput::Password(SecretUpdate::Set(secret("4321")))),
            field(None, "Token", FieldValueInput::Otp(SecretUpdate::Set(secret(RFC_SECRET)))),
            field(None, "Site", FieldValueInput::Url(secret("bank.example"))),
        ],
    }]
}

fn github() -> ItemInput {
    login("GitHub", "octo", "pw", "github.com")
}

fn create_with_fields(v: &mut VaultService) -> Uuid {
    let mut input = github();
    input.sections = Some(bank_sections());
    let staged = v.stage_create(input, NOW).unwrap();
    v.commit_write(staged, 1).unwrap().unwrap().id
}

/// (section id, PIN id, Token id, Site id)
fn field_ids(v: &VaultService, id: &Uuid) -> (Uuid, Uuid, Uuid, Uuid) {
    let s = v.login_sections(id).unwrap();
    (s[0].id, s[0].fields[0].id, s[0].fields[1].id, s[0].fields[2].id)
}

#[test]
fn fields_round_trip_through_the_encrypted_details() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let (_, pin, token, site) = field_ids(&v, &id);
    assert_eq!(v.login_field(&id, &pin).unwrap().concealed().unwrap().expose(), "4321");
    assert_eq!(v.login_field(&id, &token).unwrap().totp_code(59).unwrap().code.expose(), "287082");
    assert_eq!(v.login_field(&id, &site).unwrap().url().unwrap(), "https://bank.example/");
    // Nothing in the overview.
    let ov = serde_json::to_string(&v.get_item(&id).unwrap()).unwrap();
    assert!(!ov.contains("Bank") && !ov.contains("PIN"));
}

#[test]
fn none_keeps_the_fields() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v.stage_update(&id, github(), NOW + 1).unwrap(); // sections: None
    v.commit_write(staged, 2).unwrap();
    assert_eq!(v.login_sections(&id).unwrap()[0].fields.len(), 3);
}

#[test]
fn an_empty_layout_removes_them() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let mut input = github();
    input.sections = Some(Vec::new());
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert!(v.login_sections(&id).unwrap().is_empty());
}

#[test]
fn a_browser_password_update_keeps_the_fields() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v
        .stage_save_login(
            "https://github.com/login",
            None,
            Some("octo"),
            secret("new-pw"),
            SaveTarget::Update(&id),
            NOW + 1,
        )
        .unwrap();
    v.commit_write(staged.write, 2).unwrap();
    let (_, pin, ..) = field_ids(&v, &id);
    assert_eq!(v.login_field(&id, &pin).unwrap().concealed().unwrap().expose(), "4321");
}

#[test]
fn a_keep_cannot_reach_another_logins_field() {
    let (mut v, _) = activated_vault();
    let a = create_with_fields(&mut v);
    let b = create_with_fields(&mut v);
    let (_, a_pin, ..) = field_ids(&v, &a);
    // Save login B naming login A's PIN as one of B's fields.
    let mut input = github();
    input.sections = Some(vec![SectionInput {
        id: None,
        title: None,
        fields: vec![field(Some(a_pin), "PIN", FieldValueInput::Password(SecretUpdate::Keep))],
    }]);
    assert!(matches!(v.stage_update(&b, input, NOW + 1), Err(Error::InvalidInput(_))));
}

#[test]
fn reveal_and_copy_of_an_empty_field_are_not_found() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let (sid, pin, token, site) = field_ids(&v, &id);
    let mut input = github();
    input.sections = Some(vec![SectionInput {
        id: Some(sid),
        title: Some(secret("Bank")),
        fields: vec![
            field(Some(pin), "PIN", FieldValueInput::Password(SecretUpdate::Clear)),
            field(Some(token), "Token", FieldValueInput::Otp(SecretUpdate::Clear)),
            field(Some(site), "Site", FieldValueInput::Url(secret(""))),
        ],
    }]);
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    for f in [pin, token, site] {
        assert_eq!(v.login_field(&id, &f).unwrap().copy_value(None, 59).err(), Some(Error::NotFound));
    }
    assert_eq!(v.login_field(&id, &pin).unwrap().concealed().err(), Some(Error::NotFound));
}

#[test]
fn lookups_outside_a_login_are_not_found_and_locked_is_first() {
    let (mut v, _) = activated_vault();
    let id = create_with_fields(&mut v);
    let staged = v.stage_create(note("n", "c"), NOW).unwrap();
    let n = v.commit_write(staged, 2).unwrap().unwrap().id;
    assert_eq!(v.login_sections(&n).err(), Some(Error::NotFound));
    assert_eq!(v.login_field(&id, &Uuid::new_v4()).err(), Some(Error::NotFound));
    assert_eq!(v.login_field(&Uuid::new_v4(), &Uuid::new_v4()).err(), Some(Error::NotFound));
    let (_, pin, ..) = field_ids(&v, &id);
    v.lock();
    assert_eq!(v.login_field(&id, &pin).err(), Some(Error::Locked));
    assert_eq!(v.login_sections(&Uuid::new_v4()).err(), Some(Error::Locked));
}

#[test]
fn only_a_login_has_custom_fields() {
    let (v, _) = activated_vault();
    let mut input = note("n", "c");
    input.sections = Some(Vec::new());
    assert_eq!(
        v.stage_create(input, NOW).err(),
        Some(Error::InvalidInput("only a login has custom fields"))
    );
}

#[test]
fn page_bound_answers_hold_no_custom_field() {
    let (mut v, _) = activated_vault();
    create_with_fields(&mut v);
    let matches = v.find_matches("https://github.com/", None).unwrap();
    let json = format!("{matches:?}");
    assert!(!json.contains("4321") && !json.contains("Bank") && !json.contains("PIN"));
}

#[test]
fn address_part_copy() {
    let (mut v, _) = activated_vault();
    let mut input = github();
    input.sections = Some(vec![SectionInput {
        id: None,
        title: None,
        fields: vec![field(
            None,
            "Home",
            FieldValueInput::Address(Box::new(havenkeys_core::custom_field::AddressValue {
                city: Some(secret("Recife")),
                ..Default::default()
            })),
        )],
    }]);
    let staged = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(staged, 1).unwrap().unwrap().id;
    let f = v.login_sections(&id).unwrap()[0].fields[0].id;
    let field = v.login_field(&id, &f).unwrap();
    assert_eq!(field.copy_value(Some(AddressPart::City), 0).unwrap().expose(), "Recife");
}
```

If `find_matches` returns a type without `Debug`, replace the `format!` with `serde_json::to_string(&matches).unwrap()`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-core --test custom_fields`
Expected: compile errors: `no field sections on ItemInput`, `no method login_sections`.

- [ ] **Step 3: Add `sections` to the model**

In `crates/havenkeys-core/src/model.rs`:

1. Add `use crate::custom_field::{FieldSection, SectionInput};`.
2. In `ItemDetails::Login`, after `passkeys`:

```rust
        /// Custom fields, in sections (spec 2026-09-30-login-custom-fields).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sections: Vec<FieldSection>,
```

3. In `ItemInput`, after `card`:

```rust
    /// A login's custom fields. `None` keeps them as they are (every save
    /// path except the desktop editor); `Some` replaces the whole layout, in
    /// order (`custom_field::apply_sections`).
    #[serde(default)]
    pub sections: Option<Vec<SectionInput>>,
```

4. In `ItemInput::blank`, add `sections: None,`.
5. In `check_shape`, add this arm as the **first** arm of the `match input.item_type`:

```rust
        t if t != ItemType::Login && input.sections.is_some() => {
            Err(Error::InvalidInput("only a login has custom fields"))
        }
```

- [ ] **Step 4: Wire `build_item` and fix every construction**

In `crates/havenkeys-core/src/vault.rs`, `build_item`:
- add `sections,` to the `let ItemInput { … } = input;` destructure;
- in the `ItemType::Login` arm, bind the current sections:

```rust
            let (cur_pw, cur_totp, cur_notes, mut history, passkeys, cur_sections) = match current {
                Some(ItemDetails::Login {
                    password,
                    totp,
                    notes,
                    password_history,
                    passkeys,
                    sections,
                }) => (password, totp, notes, password_history, passkeys, sections),
                Some(_) => return Err(Error::Corrupted),
                None => (None, None, None, Vec::new(), Vec::new(), Vec::new()),
            };
```

- after `let totp = totp.apply_totp(cur_totp)?;`, add `let sections = custom_field::apply_sections(sections, cur_sections)?;` (with `use crate::custom_field;` at the top), and add `sections,` to the `ItemDetails::Login { … }` it builds.

Then run `cargo build -p havenkeys-core --all-targets 2>&1 | grep -E "^error" -A5` and fix each error:
- A **full `ItemInput { … }` literal** without `..`: add `sections: None,`.
- A **new** `ItemDetails::Login { … }` (a login created from nothing): add `sections: Vec::new(),`.
- A **rebuild of an existing login's details**: carry the existing `sections` through. **Never** use `Vec::new()` there, because that silently deletes the user's fields. The known in-place mutations (`passkey/vault.rs` `&mut details`) need nothing.
- A pattern without `..` that lists every field: add `sections` (or `..`).

Do the same in `crates/havenkeys-bridge/tests/bridge.rs`, `crates/havenkeys-sync-client/tests/round_trip.rs` and `apps/desktop/src-tauri/src/item_input.rs`. `item_input.rs` gets `sections: None` for now; Task 5 replaces it.

Remove any `#[allow(dead_code)]` added in Task 2.

- [ ] **Step 5: Add the vault reads**

In `crates/havenkeys-core/src/vault.rs`, after `card_value`:

```rust
    // ------------------------------------------------------------ custom fields

    /// A login's custom fields (spec 2026-09-30-login-custom-fields).
    /// `NotFound` for any other kind of item. Holds Password values and OTP
    /// secrets: callers hand out `custom_field::views` or one value.
    pub fn login_sections(&self, id: &Uuid) -> Result<Vec<FieldSection>> {
        match self.load_details(id)? {
            ItemDetails::Login { sections, .. } => Ok(sections),
            _ => Err(Error::NotFound),
        }
    }

    /// One custom field of a login: the one lookup behind reveal, code,
    /// copy and open. `load_details` checks the session first, so a locked
    /// vault answers `Locked` before any id is looked at.
    pub fn login_field(&self, id: &Uuid, field_id: &Uuid) -> Result<CustomField> {
        self.login_sections(id)?
            .into_iter()
            .flat_map(|s| s.fields)
            .find(|f| f.id == *field_id)
            .ok_or(Error::NotFound)
    }
```

Import `FieldSection, CustomField` from `crate::custom_field`.

- [ ] **Step 6: Fuzz the input**

Append to `crates/havenkeys-core/tests/fuzz.rs`. It uses the file's own `Rng` and `mutate_str`, the same way `fuzz_totp_input` does:

```rust
// ------------------------------------------------------------------ custom fields

/// Custom field layouts come from the renderer: arbitrary JSON must never
/// panic the parser or `stage_create`, and anything accepted must read back.
#[test]
fn fuzz_custom_field_input() {
    let mut rng = Rng::new(0xC0F1);
    let (v, _) = activated_vault();
    let seeds = [
        r#"[{"title":"Bank","fields":[{"label":"PIN","value":{"type":"password","value":{"op":"set","value":"1"}}}]}]"#,
        r#"[{"fields":[{"label":"x","value":{"type":"date","value":"2026-02-30"}}]}]"#,
        r#"[{"fields":[{"label":"x","value":{"type":"address","value":{"street":"a","postalCode":"1"}}}]}]"#,
        r#"[{"fields":[{"label":"t","value":{"type":"otp","value":{"op":"set","value":"JBSWY3DPEHPK3PXP"}}}]}]"#,
    ];
    for i in 0..3_000 {
        let text = if i % 4 == 0 {
            String::from_utf8_lossy(&rng.bytes(200)).into_owned()
        } else {
            let seed = *rng.pick(&seeds);
            mutate_str(&mut rng, seed)
        };
        let Ok(sections) =
            serde_json::from_str::<Vec<havenkeys_core::custom_field::SectionInput>>(&text)
        else {
            continue;
        };
        let mut input = login("x", "y", "z", "x.com");
        input.sections = Some(sections);
        let _ = v.stage_create(input, NOW);
    }
}
```

`stage_create` only seals the item and never commits it, so 3 000 iterations stay fast. If the file's `mutate_str` has a different signature, match the call in `fuzz_totp_input`.

- [ ] **Step 7: Run everything**

Run: `cargo test --workspace` (the desktop crate may need WebKit; if `cargo test --workspace` fails only on missing WebKit libraries, run `cargo test -p havenkeys-core -p havenkeys-bridge -p havenkeys-sync-client -p havenkeys-protocol -p havenkeys-native-host`).
Expected: all PASS.

Run: `cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add -A crates apps/desktop/src-tauri/src/item_input.rs
git commit -m "feat(core): logins hold custom field sections

ItemDetails::Login gains sections; ItemInput.sections None keeps them, so
the browser save, sign-in-with, passkey and import paths (all built on
ItemInput::blank) leave them alone. login_sections and login_field are the
reads; any other kind of item is NotFound, a locked vault Locked first."
```

---

### Task 4: Core — 1Password section fields become custom fields

**Files:**
- Modify: `crates/havenkeys-core/src/import/mod.rs` (`ImportReport.fields_to_notes`)
- Modify: `crates/havenkeys-core/src/import/onepux.rs`

**Interfaces:**
- Consumes: `custom_field::{AddressValue, FieldInput, FieldKind, FieldValueInput, SectionInput, clean_plain, MAX_FIELDS, MAX_SECTIONS, MAX_LABEL_CHARS}`, `model::check_password`, `totp::parse_totp_input`.
- Produces: `ImportReport.fields_to_notes: usize` (serialized `fieldsToNotes`), used by Task 6.

- [ ] **Step 1: Update `maps_items` and add the new tests (failing)**

In `onepux.rs` tests, `maps_items`, replace

```rust
        assert!(notes.contains("[Security]\nRecovery code: RC-123"));
```

with

```rust
        assert!(!notes.contains("RC-123"), "section fields are custom fields now");
        let sections = gh.input.sections.as_ref().expect("login sections");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title.as_ref().unwrap().expose(), "Security");
        assert_eq!(sections[0].fields.len(), 1, "TOTP and sso are taken first");
        assert_eq!(sections[0].fields[0].label.expose(), "Recovery code");
        assert!(matches!(
            &sections[0].fields[0].value,
            FieldValueInput::Password(SecretUpdate::Set(v)) if v.expose() == "RC-123"
        ));
```

Add these tests:

```rust
    fn login_with(fields: Value) -> Value {
        json!({"accounts": [{"attrs": {}, "vaults": [{"attrs": {"name": "P"}, "items": [{
            "uuid": "a", "categoryUuid": "001", "state": "active",
            "overview": {"title": "Bank"},
            "details": {
                "loginFields": [
                    {"value": "me", "name": "u", "fieldType": "T", "designation": "username"},
                    {"value": "branch-7", "name": "branch", "fieldType": "T"},
                    {"value": "4321", "name": "pin", "fieldType": "P"}
                ],
                "sections": [{"title": "More", "fields": fields}]
            }
        }]}]}]})
    }

    fn only_login(data: Value) -> (ImportedItem, ImportReport) {
        let mut parsed = parse(&make_1pux(&data, 0)).unwrap();
        (parsed.items.remove(0), parsed.report)
    }

    #[test]
    fn every_field_kind_maps_to_its_type() {
        let (item, _) = only_login(login_with(json!([
            {"title": "Note", "value": {"string": "hello"}},
            {"title": "Mail", "value": {"email": {"email_address": "a@b.c"}}},
            {"title": "Tel", "value": {"phone": "+55 11 5555"}},
            {"title": "Site", "value": {"url": "https://bank.example"}},
            {"title": "Opened", "value": {"date": 1_700_000_000}},
            {"title": "Home", "value": {"address": {"street": "Rua A", "city": "Recife", "zip": "50000"}}},
            {"title": "", "value": {"totp": "JBSWY3DPEHPK3PXP"}},
            {"title": "Backup", "value": {"totp": "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"}},
            {"title": "Card exp", "value": {"monthYear": 202612}},
            {"title": "Key", "value": {"sshKey": {"privateKey": "-----BEGIN KEY-----"}}}
        ])));
        let s = item.input.sections.as_ref().unwrap();
        let kinds: Vec<FieldKind> = s[0].fields.iter().map(|f| f.value.kind()).collect();
        assert_eq!(
            kinds,
            [
                FieldKind::Text, FieldKind::Email, FieldKind::Phone, FieldKind::Url,
                FieldKind::Date, FieldKind::Address, FieldKind::Otp, FieldKind::Text,
                FieldKind::Password,
            ],
            "the first TOTP stays the login's own"
        );
        assert!(set_value(&item.input.totp).is_some());
        assert!(matches!(&s[0].fields[4].value, FieldValueInput::Date(d) if d.expose() == "2023-11-14"));
        match &s[0].fields[5].value {
            FieldValueInput::Address(a) => {
                assert_eq!(a.postal_code.as_ref().unwrap().expose(), "50000");
                assert_eq!(a.city.as_ref().unwrap().expose(), "Recife");
            }
            _ => panic!("address"),
        }
        // Extra form fields: an untitled section after the 1Password ones.
        let form = &s[1];
        assert!(form.title.is_none());
        assert_eq!(form.fields[0].label.expose(), "branch");
        assert_eq!(form.fields[0].value.kind(), FieldKind::Text);
        assert_eq!(form.fields[1].value.kind(), FieldKind::Password);
    }

    #[test]
    fn odd_values_become_text_and_overflow_goes_to_notes() {
        let mut fields: Vec<Value> = vec![
            json!({"title": "Bad otp", "value": {"totp": "not base32 !!"}}),
            json!({"title": "Bad site", "value": {"url": "javascript:alert(1)"}}),
            json!({"title": "Far", "value": {"date": 400_000_000_000i64}}),
        ];
        for i in 0..110 {
            fields.push(json!({"title": format!("f{i}"), "value": {"string": format!("v{i}")}}));
        }
        let (item, report) = only_login(login_with(Value::Array(fields)));
        let s = item.input.sections.as_ref().unwrap();
        let first: Vec<FieldKind> = s[0].fields.iter().take(3).map(|f| f.value.kind()).collect();
        assert_eq!(first, [FieldKind::Text, FieldKind::Text, FieldKind::Text]);
        let total: usize = s.iter().map(|x| x.fields.len()).sum();
        assert_eq!(total, MAX_FIELDS);
        assert!(report.fields_to_notes > 0);
        let notes = set_value(&item.input.notes).unwrap();
        assert!(notes.contains("f109: v109"), "overflow is kept in the notes");
    }
```

Add the needed `use` lines in the test module: `crate::custom_field::{FieldKind, FieldValueInput, MAX_FIELDS}` and `crate::import::{ImportReport, ImportedItem}` if they are not already in scope via `super::*`.

Run: `cargo test -p havenkeys-core --lib import::onepux`
Expected: FAIL (`sections` is `None`; `fields_to_notes` does not exist).

- [ ] **Step 2: Add the report counter**

In `crates/havenkeys-core/src/import/mod.rs`, `ImportReport`, add after `urls_moved_to_notes`:

```rust
    /// Custom fields past a login's limits (100 fields, 20 sections), kept as
    /// text in the login's notes instead.
    pub fields_to_notes: usize,
```

If `ImportReport` has an explicit constructor or `Default` impl listing fields, add it there.

- [ ] **Step 3: Implement the mapping in `onepux.rs`**

Add the imports:

```rust
use crate::custom_field::{
    clean_plain, AddressValue, FieldInput, FieldKind, FieldValueInput, SectionInput,
    MAX_FIELDS, MAX_LABEL_CHARS, MAX_SECTIONS,
};
use crate::model::check_password;
```

(and `parse_totp_input` if not already imported).

Add after `Extras`:

```rust
/// A login's custom fields built from its 1Password sections and extra form
/// fields (spec 2026-09-30-login-custom-fields §6).
#[derive(Default)]
struct LoginSections {
    sections: Vec<SectionInput>,
    count: usize,
}

impl LoginSections {
    fn start(&mut self, title: Option<&str>) {
        let title = non_empty(title)
            .map(|t| clean_line(t, MAX_LABEL_CHARS))
            .filter(|t| !t.is_empty())
            .map(SecretString::new);
        self.sections.push(SectionInput { id: None, title, fields: Vec::new() });
    }

    /// Add a field to the current section; false when the login is full, so
    /// the caller keeps the value in the notes.
    fn push(&mut self, label: Option<&str>, value: FieldValueInput) -> bool {
        let used = self.sections.iter().filter(|s| !s.fields.is_empty()).count();
        let Some(section) = self.sections.last_mut() else {
            return false;
        };
        if self.count >= MAX_FIELDS || (section.fields.is_empty() && used >= MAX_SECTIONS) {
            return false;
        }
        let label = non_empty(label)
            .map(|l| clean_line(l, MAX_LABEL_CHARS))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| default_label(value.kind()).to_owned());
        section.fields.push(FieldInput { id: None, label: SecretString::new(label), value });
        self.count += 1;
        true
    }

    fn finish(self) -> Option<Vec<SectionInput>> {
        let sections: Vec<SectionInput> =
            self.sections.into_iter().filter(|s| !s.fields.is_empty()).collect();
        (!sections.is_empty()).then_some(sections)
    }
}

fn default_label(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "Text",
        FieldKind::Url => "URL",
        FieldKind::Email => "Email",
        FieldKind::Phone => "Phone",
        FieldKind::Date => "Date",
        FieldKind::Address => "Address",
        FieldKind::Password => "Password",
        FieldKind::Otp => "One-time password",
    }
}

/// A 1Password section value as a custom field. `None` for kinds with
/// nothing to keep (files are counted by `render_value`). A value that does
/// not pass its type's checks becomes Text; one that is not valid text
/// either is `None`, and the caller keeps it in the notes.
fn custom_field_from(value: &Value, report: &mut ImportReport) -> Option<FieldValueInput> {
    let obj = value.as_object()?;
    let (kind, v) = obj.iter().next()?;
    let text = |t: Option<&str>| non_empty(t).map(SecretString::from);
    let candidate = match kind.as_str() {
        "file" => return None,
        "concealed" => FieldValueInput::Password(SecretUpdate::Set(text(v.as_str())?)),
        "totp" => FieldValueInput::Otp(SecretUpdate::Set(text(v.as_str())?)),
        "email" => FieldValueInput::Email(text(str_at(v, &["email_address"]).or(v.as_str()))?),
        "phone" => FieldValueInput::Phone(text(v.as_str())?),
        "url" => FieldValueInput::Url(text(v.as_str())?),
        "date" => FieldValueInput::Date(SecretString::new(format_date(v.as_i64()?))),
        "address" => {
            let part = |k: &str| text(str_at(v, &[k]));
            let a = AddressValue {
                street: part("street"),
                city: part("city"),
                state: part("state"),
                postal_code: part("zip"),
                country: part("country"),
                ..AddressValue::default()
            };
            if a.street.is_none() && a.city.is_none() && a.state.is_none()
                && a.postal_code.is_none() && a.country.is_none()
            {
                return None;
            }
            FieldValueInput::Address(Box::new(a))
        }
        "sshKey" => FieldValueInput::Password(SecretUpdate::Set(SecretString::new(
            render_value(value, report)?,
        ))),
        _ => FieldValueInput::Text(SecretString::new(render_value(value, report)?)),
    };
    let passes = match &candidate {
        FieldValueInput::Otp(SecretUpdate::Set(t)) => parse_totp_input(t.expose()).is_ok(),
        FieldValueInput::Password(SecretUpdate::Set(p)) => check_password(p).is_ok(),
        other => clean_plain(other).is_ok(),
    };
    if passes {
        return Some(candidate);
    }
    let as_text = FieldValueInput::Text(SecretString::new(render_value(value, report)?));
    clean_plain(&as_text).is_ok().then_some(as_text)
}
```

The address arm builds `AddressValue` with named parts, so no part can be lost silently. The `"ssoLogin"` kind reaches the `_` arm; it is only reached for a second, or unknown, provider, because the first recognised one is taken before this is called.

In `convert_item`:
- declare `let mut fields = LoginSections::default();` next to `let mut extras = Extras::default();`;
- in the sections loop, next to `extras.section(str_at(section, &["title"]));`, add `if is_login { fields.start(str_at(section, &["title"])); }`;
- replace the tail of the per-field loop (the `if let Some(text) = render_value(value, report) { … }` block) with:

```rust
                if is_login {
                    if let Some(fv) = custom_field_from(value, report) {
                        if fields.push(label, fv) {
                            continue;
                        }
                        report.fields_to_notes += 1;
                    }
                }
                if let Some(text) = render_value(value, report) {
                    // (the existing body, unchanged)
                }
```

Change `login_fields` to take `sections: &mut LoginSections` and `report: &mut ImportReport`, and replace its last `else if` with:

```rust
        } else if !matches!(field_type, "C" | "R" | "B" | "I") {
            let name = non_empty(str_at(f, &["name"]));
            let v = SecretString::from(value);
            let fv = if field_type == "P" {
                FieldValueInput::Password(SecretUpdate::Set(v))
            } else {
                FieldValueInput::Text(v)
            };
            if !sections.push(name, fv) {
                report.fields_to_notes += 1;
                extras.push(name, value);
            }
        }
```

In the `"001" | "005"` arm, call `fields.start(None);` just before `login_fields(fields_json, &mut extras, &mut fields, report)`. Rename the local `fields` from `details.get("loginFields")` to `form` so the names don't clash. Add `sections: fields.finish(),` to the login's `ItemInput { … }`. For other categories `fields` stays empty and is unused. Add `let _ = fields;` only if the compiler warns; it should not, because `fields` is used in the loop.

The "file" kind still reaches `render_value`, which counts it, because `custom_field_from` returns `None` for it before counting anything.

- [ ] **Step 4: Run the importer tests**

Run: `cargo test -p havenkeys-core --lib import`
Expected: PASS, including `fuzz_never_panics`, `maps_items`, `dates_format` and `errors_do_not_echo_content`.

Run: `cargo test -p havenkeys-core`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/import
git commit -m "feat(import): 1Password section fields become custom fields

Each 1Password section of a login becomes a titled section and each
field keeps its type and label; extra form fields go to an untitled
section after them. A value that fails its type's checks becomes Text;
past 100 fields or 20 sections the rest stay in the notes, counted in
fields_to_notes."
```

---

### Task 5: Desktop Rust — commands and the save wire

**Files:**
- Create: `apps/desktop/src-tauri/src/custom_field.rs`
- Modify: `apps/desktop/src-tauri/src/item_input.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (`mod custom_field;` + `generate_handler!`)
- Modify: `apps/desktop/src-tauri/build.rs` (`COMMANDS`)
- Modify: `apps/desktop/src-tauri/capabilities/main.json`

**Interfaces:**
- Consumes: `VaultService::{login_sections, login_field}`, `custom_field::{views, SectionView, AddressPart, SectionInput}`, `commands::{copy_from_vault, CopyResult}`, `AppState::{touch, vault, unix_seconds}`, `CmdError::open_website`.
- Produces (called by Task 6's `api.ts`, argument names as JS sends them):
  - `login_fields({ id }) -> SectionView[]`
  - `reveal_login_field({ id, fieldId }) -> string`
  - `login_field_totp({ id, fieldId }) -> TotpCode`
  - `copy_login_field({ id, fieldId, part? }) -> CopyResult`
  - `open_login_field_url({ id, fieldId }) -> void`
  - `ItemInputWire.sections: Option<Vec<SectionInput<TotpUpdate>>>`

- [ ] **Step 1: Write the wire tests (failing)**

In `apps/desktop/src-tauri/src/item_input.rs` tests, add:

```rust
    fn wire_with_sections(otp: serde_json::Value) -> ItemInputWire {
        serde_json::from_value(json!({
            "itemType": "login",
            "title": "Bank",
            "sections": [{ "title": "More", "fields": [
                { "label": "Token", "value": { "type": "otp", "value": otp } },
                { "label": "PIN", "value": { "type": "password", "value": { "op": "set", "value": "1" } } }
            ]}]
        }))
        .unwrap()
    }

    #[test]
    fn a_scanned_token_in_an_otp_field_becomes_a_set() {
        let now = Instant::now();
        let (slot, token) = slot_with("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP", now);
        let input = wire_with_sections(json!({ "op": "scanned", "value": token }));
        assert!(input.uses_scan());
        let input = input.resolve(&slot, now).unwrap();
        let sections = input.sections.unwrap();
        match &sections[0].fields[0].value {
            havenkeys_core::custom_field::FieldValueInput::Otp(SecretUpdate::Set(v)) => {
                assert_eq!(v.expose(), "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP")
            }
            _ => panic!("expected Otp(Set)"),
        }
    }

    #[test]
    fn an_expired_token_in_an_otp_field_is_refused_and_no_sections_is_none() {
        let now = Instant::now();
        let slot = ScanSlot::default();
        let err = wire_with_sections(json!({ "op": "scanned", "value": "feed" }))
            .resolve(&slot, now)
            .err()
            .unwrap();
        assert_eq!(err.code, "scan_expired");
        let plain = wire(json!({ "op": "keep" }));
        assert!(plain.resolve(&slot, now).unwrap().sections.is_none());
    }
```

Run: `cargo test -p havenkeys-desktop item_input` (needs the WebKit dev libraries; see `docs/development.md`. If they are missing, say so in the task report and rely on `cargo check -p havenkeys-desktop` plus review).
Expected: FAIL, because `sections` is an unknown field.

- [ ] **Step 2: Add `sections` to the wire with one TOTP resolver**

In `item_input.rs`, add `use havenkeys_core::custom_field::SectionInput;` and to `ItemInputWire`:

```rust
    /// The core's `sections`, with OTP setups that may also be scanned
    /// tokens (resolved here, like `totp`).
    #[serde(default)]
    pub sections: Option<Vec<SectionInput<TotpUpdate>>>,
```

Replace `uses_scan` and `resolve` with:

```rust
impl ItemInputWire {
    /// Whether saving this uses a scanned code, so the caller can empty the
    /// slot once the save has gone through.
    pub fn uses_scan(&self) -> bool {
        let scanned = |u: &TotpUpdate| matches!(u, TotpUpdate::Scanned(_));
        scanned(&self.totp)
            || self
                .sections
                .iter()
                .flatten()
                .any(|s| s.otp_updates().any(scanned))
    }

    pub fn resolve(self, slot: &ScanSlot, now: Instant) -> CmdResult<ItemInput> {
        let mut resolve = |u| resolve_totp(u, slot, now);
        let totp = resolve(self.totp)?;
        let sections = self
            .sections
            .map(|list| {
                list.into_iter()
                    .map(|s| s.try_map_otp(&mut resolve))
                    .collect::<CmdResult<Vec<_>>>()
            })
            .transpose()?;
        Ok(ItemInput {
            item_type: self.item_type,
            title: self.title,
            username: self.username,
            urls: self.urls,
            password: self.password,
            totp,
            notes: self.notes,
            content: self.content,
            auto_sign_in: self.auto_sign_in,
            sign_in_with: self.sign_in_with,
            identity: self.identity,
            card: self.card,
            sections,
        })
    }
}

/// A TOTP update as the core takes it: a scanned token becomes a `Set` of
/// the URI Rust kept. The one rule for the login's TOTP and OTP fields.
fn resolve_totp(update: TotpUpdate, slot: &ScanSlot, now: Instant) -> CmdResult<SecretUpdate> {
    Ok(match update {
        TotpUpdate::Keep => SecretUpdate::Keep,
        TotpUpdate::Set(value) => SecretUpdate::Set(value),
        TotpUpdate::Clear => SecretUpdate::Clear,
        TotpUpdate::Scanned(token) => {
            SecretUpdate::Set(slot.get(&token, now).ok_or_else(scan_expired)?)
        }
    })
}
```

- [ ] **Step 3: Create the commands**

Create `apps/desktop/src-tauri/src/custom_field.rs`:

```rust
//! A login's custom fields in the desktop app (spec
//! 2026-09-30-login-custom-fields §5.1).
//!
//! The core owns the fields and their rules. The renderer gets the view
//! (Password and OTP reduced to flags) when a login opens; a Password value
//! comes one at a time on an explicit reveal; copies and URL opens happen
//! here, from the vault, and never take a value from the renderer.

use crate::commands::{copy_from_vault, CopyResult};
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::custom_field::{self, AddressPart, SectionView};
use havenkeys_core::totp::TotpCode;
use havenkeys_core::SecretString;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn login_fields(state: State<'_, AppState>, id: Uuid) -> CmdResult<Vec<SectionView>> {
    state.touch();
    Ok(custom_field::views(state.vault()?.login_sections(&id)?))
}

#[tauri::command]
pub fn reveal_login_field(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.login_field(&id, &field_id)?.concealed()?)
}

#[tauri::command]
pub fn login_field_totp(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
) -> CmdResult<TotpCode> {
    // No `touch()`: refreshed on a timer, like `get_totp_code`.
    Ok(state
        .vault()?
        .login_field(&id, &field_id)?
        .totp_code(AppState::unix_seconds())?)
}

#[tauri::command]
pub fn copy_login_field(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
    part: Option<AddressPart>,
) -> CmdResult<CopyResult> {
    copy_from_vault(&state, |v| {
        v.login_field(&id, &field_id)?
            .copy_value(part, AppState::unix_seconds())
    })
}

/// Open a URL field in the default browser. The renderer names the field;
/// Rust opens only that field's saved address, normalised again (http(s)
/// only), like `open_website`.
#[tauri::command]
pub fn open_login_field_url(state: State<'_, AppState>, id: Uuid, field_id: Uuid) -> CmdResult<()> {
    state.touch();
    let target = state.vault()?.login_field(&id, &field_id)?.url()?;
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())
}
```

Check the real paths: `TotpCode` may be re-exported elsewhere (see how `commands.rs` imports it), and `AppState::unix_seconds` is used in `commands.rs`. Match those `use` lines.

- [ ] **Step 4: Register and allow the commands**

- In `src-tauri/src/lib.rs`, add `mod custom_field;` next to `mod card;`. In `generate_handler!`, after `card::check_card_number,`, add:

```rust
            custom_field::login_fields,
            custom_field::reveal_login_field,
            custom_field::login_field_totp,
            custom_field::copy_login_field,
            custom_field::open_login_field_url,
```

- In `build.rs` `COMMANDS`, after `"check_card_number",`, add `"login_fields", "reveal_login_field", "login_field_totp", "copy_login_field", "open_login_field_url",`.
- In `capabilities/main.json`, after `"allow-check-card-number"`, add `"allow-login-fields"`, `"allow-reveal-login-field"`, `"allow-login-field-totp"`, `"allow-copy-login-field"`, `"allow-open-login-field-url"`.

- [ ] **Step 5: Verify**

Run: `cargo test -p havenkeys-desktop` and `cargo clippy -p havenkeys-desktop -- -D warnings`
Expected: PASS and clean. If the WebKit libraries are missing, run `cargo check -p havenkeys-desktop` if it can, and report the limitation.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri
git commit -m "feat(desktop): commands for a login's custom fields

login_fields returns the view (no Password values or OTP secrets);
reveal_login_field, login_field_totp, copy_login_field and
open_login_field_url go through VaultService::login_field. The save wire
carries sections, with scanned QR tokens resolved by the same resolve_totp
as the login's own TOTP."
```

---

### Task 6: Desktop TS — types, API, editor model, strings

**Files:**
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/api.ts`, `apps/desktop/src/lib/hooks.ts`, `apps/desktop/src/lib/format.ts`, `apps/desktop/src/lib/format.test.ts`
- Create: `apps/desktop/src/lib/customFields.ts`, `apps/desktop/src/lib/customFields.test.ts`
- Modify: `apps/desktop/src/lib/openItem.ts`, `apps/desktop/src/lib/openItem.test.ts`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts`

**Interfaces:**
- Consumes: Task 5's command names and argument names.
- Produces (used by Task 7):
  - types `FieldType`, `AddressPart`, `AddressValue`, `FieldView`, `SectionView`, `FieldValueInput`, `FieldInput`, `SectionInput`; `ItemInput.sections?: SectionInput[]`; `ImportResult.fieldsToNotes: number`
  - `api.loginFields(id)`, `api.revealLoginField(id, fieldId)`, `api.loginFieldTotp(id, fieldId)`, `api.copyLoginField(id, fieldId, part?)`, `api.openLoginFieldUrl(id, fieldId)`
  - `useTotp(load: () => Promise<TotpCode>, key: string, enabled: boolean)`
  - `formatDay(iso: string, locale: string): string`
  - `customFields.ts`: `FIELD_TYPES`, `MAX_SECTIONS`, `MAX_FIELDS`, `EditField`, `EditSection`, `fromViews`, `newField`, `newSection`, `addField`, `fieldCount`, `updateField`, `removeField`, `updateSection`, `removeSection`, `moveField`, `moveSection`, `placeField`, `sectionsInput`
  - `EditorSnapshot.sections: EditSection[] | null`
  - strings `t.fields.*` and `t.import.fieldsToNotes`

- [ ] **Step 1: Types**

In `lib/types.ts`, after `ItemInput`:

```ts
/** A custom field's kind (spec 2026-09-30-login-custom-fields). */
export type FieldType = "text" | "url" | "email" | "phone" | "date" | "address" | "password" | "otp";
export type AddressPart =
  | "street"
  | "number"
  | "complement"
  | "neighborhood"
  | "city"
  | "state"
  | "postalCode"
  | "country";
export type AddressValue = Partial<Record<AddressPart, string | null>>;

/** A custom field as the core shows it: no Password value, no OTP secret. */
export type FieldView = { id: string; label: string } & (
  | { type: "text" | "url" | "email" | "phone" | "date"; value: string }
  | { type: "address"; parts: AddressValue; formatted: string }
  | { type: "password"; hasValue: boolean }
  | { type: "otp"; hasOtp: boolean }
);
export interface SectionView {
  id: string;
  title: string | null;
  fields: FieldView[];
}

export type FieldValueInput =
  | { type: "text" | "url" | "email" | "phone" | "date"; value: string }
  | { type: "address"; value: AddressValue }
  | { type: "password" | "otp"; value: SecretUpdate };
export interface FieldInput {
  /** Absent: a new field. */
  id?: string;
  label: string;
  value: FieldValueInput;
}
export interface SectionInput {
  id?: string;
  title: string | null;
  fields: FieldInput[];
}
```

Add to `ItemInput`:

```ts
  /** A login's custom fields, whole layout in order. Absent: kept as they are. */
  sections?: SectionInput[];
```

Add `fieldsToNotes: number;` to `ImportResult`, next to `urlsMovedToNotes`.

- [ ] **Step 2: API wrappers**

In `lib/api.ts`, import `AddressPart, SectionView` and add after `openWebsite`:

```ts
  loginFields: (id: string) => call<SectionView[]>("login_fields", { id }),
  revealLoginField: (id: string, fieldId: string) => call<string>("reveal_login_field", { id, fieldId }),
  loginFieldTotp: (id: string, fieldId: string) => call<TotpCode>("login_field_totp", { id, fieldId }),
  copyLoginField: (id: string, fieldId: string, part?: AddressPart) =>
    call<CopyResult>("copy_login_field", { id, fieldId, part: part ?? null }),
  /** Rust opens only that field's saved http(s) address. */
  openLoginFieldUrl: (id: string, fieldId: string) => call<void>("open_login_field_url", { id, fieldId }),
```

- [ ] **Step 3: `useTotp` takes a loader**

In `lib/hooks.ts`, change the signature and fetch:

```ts
/** A live TOTP code; refreshes at the period boundary. `key` restarts it. */
export function useTotp(load: () => Promise<TotpCode>, key: string, enabled: boolean) {
```

Inside, `const c = await api.totp(itemId);` becomes `const c = await loadRef.current();`. Add before the effect:

```ts
  const loadRef = useRef(load);
  loadRef.current = load;
```

The effect deps become `[key, enabled]`. In `views/ItemDetail.tsx`, `TotpField` calls `useTotp(() => api.totp(item.id), item.id, true)`.

- [ ] **Step 4: `formatDay` with a test**

In `lib/format.test.ts` add:

```ts
describe("formatDay", () => {
  it("formats a calendar day without shifting it by time zone", () => {
    expect(formatDay("2026-09-30", "en-US")).toBe("Sep 30, 2026");
    expect(formatDay("2026-01-01", "pt-BR")).toBe("1 de jan. de 2026");
  });
  it("returns the text as it is when it is not a date", () => {
    expect(formatDay("", "en-US")).toBe("");
    expect(formatDay("soon", "en-US")).toBe("soon");
  });
});
```

(add `formatDay` to that file's import). In `lib/format.ts`:

```ts
/** A `YYYY-MM-DD` day in the UI locale; read as UTC so no zone moves it. */
export function formatDay(iso: string, locale: string): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return iso;
  const d = new Date(`${iso}T00:00:00Z`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(locale, { dateStyle: "medium", timeZone: "UTC" });
}
```

If the pt-BR expectation differs on the CI's ICU (e.g. `1 de jan. de 2026` vs `01/01/2026`), change the test to assert only that it contains `2026` and does not contain `2025`. The goal is "no zone shift".

- [ ] **Step 5: Write the editor-model tests (failing)**

Create `lib/customFields.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  addField,
  fromViews,
  moveField,
  moveSection,
  newField,
  newSection,
  placeField,
  removeField,
  sectionsInput,
  type EditSection,
} from "./customFields";
import { KEEP } from "./secretEdit";
import type { SectionView } from "./types";

const views: SectionView[] = [
  {
    id: "s1",
    title: "Bank",
    fields: [
      { id: "f1", label: "PIN", type: "password", hasValue: true },
      { id: "f2", label: "Token", type: "otp", hasOtp: false },
      { id: "f3", label: "Account", type: "text", value: "12345" },
    ],
  },
  {
    id: "s2",
    title: null,
    fields: [{ id: "f4", label: "Home", type: "address", parts: { city: "Recife" }, formatted: "Recife" }],
  },
];

const keys = (s: EditSection[]) => s.map((x) => x.fields.map((f) => f.id ?? f.label));

describe("customFields", () => {
  it("never sends sections that were not loaded, so Rust keeps them", () => {
    expect(sectionsInput(null)).toBeUndefined();
  });

  it("opens existing secrets as keep and empty ones as empty", () => {
    const s = fromViews(views);
    expect(s[0]!.fields[0]!.secret).toEqual(KEEP);
    expect(s[0]!.fields[1]!.secret).toEqual({ mode: "set", value: "" });
    expect(s[0]!.fields[2]!.text).toBe("12345");
    expect(s[1]!.fields[0]!.address).toEqual({ city: "Recife" });
    expect(s[1]!.title).toBe("");
  });

  it("builds the save payload with ids, trimmed labels and secret updates", () => {
    const s = fromViews(views);
    const out = sectionsInput(s)!;
    expect(out[0]).toEqual({
      id: "s1",
      title: "Bank",
      fields: [
        { id: "f1", label: "PIN", value: { type: "password", value: { op: "keep" } } },
        { id: "f2", label: "Token", value: { type: "otp", value: { op: "clear" } } },
        { id: "f3", label: "Account", value: { type: "text", value: "12345" } },
      ],
    });
    expect(out[1]).toEqual({
      id: "s2",
      title: null,
      fields: [{ id: "f4", label: "Home", value: { type: "address", value: { city: "Recife" } } }],
    });
  });

  it("new fields have no id and go to the last section", () => {
    const s = addField(fromViews(views), newField("email", " Mail "));
    const out = sectionsInput(s)!;
    expect(out[1]!.fields[1]).toEqual({ label: "Mail", value: { type: "email", value: "" } });
    expect(addField([], newField("text", "x"))).toHaveLength(1);
  });

  it("moves a field within a section and across section edges, keeping its secret", () => {
    const s = fromViews(views);
    const pin = s[0]!.fields[0]!.key;
    expect(keys(moveField(s, pin, 1))).toEqual([["f2", "f1", "f3"], ["f4"]]);
    const account = s[0]!.fields[2]!.key;
    const moved = moveField(s, account, 1);
    expect(keys(moved)).toEqual([["f1", "f2"], ["f3", "f4"]]);
    const back = moveField(moved, account, -1);
    expect(keys(back)).toEqual([["f1", "f2", "f3"], ["f4"]]);
    expect(moveField(s, pin, -1)).toBe(s);
    const pinToEnd = placeField(s, pin, s[1]!.key, null);
    expect(keys(pinToEnd)).toEqual([["f2", "f3"], ["f4", "f1"]]);
    expect(pinToEnd[1]!.fields[1]!.secret).toEqual(KEEP);
  });

  it("drops a field before another one", () => {
    const s = fromViews(views);
    const account = s[0]!.fields[2]!.key;
    const pin = s[0]!.fields[0]!.key;
    expect(keys(placeField(s, account, s[0]!.key, pin))).toEqual([["f3", "f1", "f2"], ["f4"]]);
  });

  it("moves and removes sections", () => {
    const s = [...fromViews(views), newSection("New")];
    expect(moveSection(s, s[2]!.key, -1).map((x) => x.title)).toEqual(["Bank", "New", ""]);
    expect(keys(removeField(s, s[0]!.fields[1]!.key))[0]).toEqual(["f1", "f3"]);
  });
});
```

Run: `pnpm --filter @havenkeys/desktop test customFields` (check the package name in `apps/desktop/package.json`; else run `pnpm vitest run src/lib/customFields.test.ts` inside `apps/desktop`).
Expected: FAIL, because the module does not exist.

- [ ] **Step 6: Implement `customFields.ts`**

```ts
/*
 * The editor's model of a login's custom fields (spec
 * 2026-09-30-login-custom-fields §5.3). Pure functions so the payload,
 * moves and drops are tested without React. Password and OTP values are
 * `SecretEdit`s: an existing one is `keep` and is never loaded to edit.
 */
import { EMPTY, KEEP, toUpdate, type SecretEdit } from "./secretEdit";
import type { AddressValue, FieldType, FieldValueInput, SectionInput, SectionView } from "./types";

/** The "add another field" menu, in 1Password's order. */
export const FIELD_TYPES: FieldType[] = ["text", "url", "email", "address", "date", "otp", "password", "phone"];
export const MAX_SECTIONS = 20;
export const MAX_FIELDS = 100;

export interface EditField {
  /** React key; never sent. */
  key: string;
  /** Absent for a field added in this edit. */
  id?: string;
  type: FieldType;
  label: string;
  /** text, url, email, phone, date */
  text: string;
  address: AddressValue;
  /** password, otp */
  secret: SecretEdit;
}

export interface EditSection {
  key: string;
  id?: string;
  title: string;
  fields: EditField[];
}

let counter = 0;
function newKey(): string {
  counter += 1;
  return `cf${counter}`;
}

export function fromViews(views: SectionView[]): EditSection[] {
  return views.map((s) => ({
    key: newKey(),
    id: s.id,
    title: s.title ?? "",
    fields: s.fields.map((f): EditField => {
      const base: EditField = { key: newKey(), id: f.id, type: f.type, label: f.label, text: "", address: {}, secret: EMPTY };
      switch (f.type) {
        case "password":
          return { ...base, secret: f.hasValue ? KEEP : EMPTY };
        case "otp":
          return { ...base, secret: f.hasOtp ? KEEP : EMPTY };
        case "address":
          return { ...base, address: { ...f.parts } };
        default:
          return { ...base, text: f.value };
      }
    }),
  }));
}

export function newField(type: FieldType, label: string): EditField {
  return { key: newKey(), type, label, text: "", address: {}, secret: EMPTY };
}

export function newSection(title = ""): EditSection {
  return { key: newKey(), title, fields: [] };
}

export function fieldCount(sections: EditSection[]): number {
  return sections.reduce((n, s) => n + s.fields.length, 0);
}

/** Adds to the last section, opening an untitled one when there is none. */
export function addField(sections: EditSection[], field: EditField): EditSection[] {
  if (sections.length === 0) return [{ ...newSection(), fields: [field] }];
  const last = sections.length - 1;
  return sections.map((s, i) => (i === last ? { ...s, fields: [...s.fields, field] } : s));
}

export function updateField(sections: EditSection[], key: string, change: Partial<EditField>): EditSection[] {
  return sections.map((s) =>
    s.fields.some((f) => f.key === key)
      ? { ...s, fields: s.fields.map((f) => (f.key === key ? { ...f, ...change } : f)) }
      : s,
  );
}

export function removeField(sections: EditSection[], key: string): EditSection[] {
  return sections.map((s) => ({ ...s, fields: s.fields.filter((f) => f.key !== key) }));
}

export function updateSection(sections: EditSection[], key: string, title: string): EditSection[] {
  return sections.map((s) => (s.key === key ? { ...s, title } : s));
}

export function removeSection(sections: EditSection[], key: string): EditSection[] {
  return sections.filter((s) => s.key !== key);
}

/**
 * One place up or down. At a section's edge the field moves into the
 * neighbouring section: its end going up, its start going down.
 * Returns `sections` itself when there is nowhere to go.
 */
export function moveField(sections: EditSection[], key: string, delta: -1 | 1): EditSection[] {
  const si = sections.findIndex((s) => s.fields.some((f) => f.key === key));
  if (si < 0) return sections;
  const fi = sections[si]!.fields.findIndex((f) => f.key === key);
  const target = fi + delta;
  const next = sections.map((s) => ({ ...s, fields: [...s.fields] }));
  const [field] = next[si]!.fields.splice(fi, 1);
  if (target >= 0 && target < sections[si]!.fields.length) {
    next[si]!.fields.splice(target, 0, field!);
    return next;
  }
  const ns = si + delta;
  if (ns < 0 || ns >= next.length) return sections;
  if (delta < 0) next[ns]!.fields.push(field!);
  else next[ns]!.fields.unshift(field!);
  return next;
}

export function moveSection(sections: EditSection[], key: string, delta: -1 | 1): EditSection[] {
  const i = sections.findIndex((s) => s.key === key);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= sections.length) return sections;
  const next = [...sections];
  [next[i], next[j]] = [next[j]!, next[i]!];
  return next;
}

/** Drag and drop: put the field before `beforeKey` in `sectionKey`, or at its end. */
export function placeField(
  sections: EditSection[],
  key: string,
  sectionKey: string,
  beforeKey: string | null,
): EditSection[] {
  const field = sections.flatMap((s) => s.fields).find((f) => f.key === key);
  if (!field || key === beforeKey) return sections;
  return removeField(sections, key).map((s) => {
    if (s.key !== sectionKey) return s;
    const at = beforeKey === null ? -1 : s.fields.findIndex((f) => f.key === beforeKey);
    const fields = [...s.fields];
    fields.splice(at < 0 ? fields.length : at, 0, field);
    return { ...s, fields };
  });
}

function valueInput(f: EditField): FieldValueInput {
  switch (f.type) {
    case "password":
    case "otp":
      return { type: f.type, value: toUpdate(f.secret) };
    case "address":
      return { type: "address", value: f.address };
    default:
      return { type: f.type, value: f.text };
  }
}

/** The save's `sections`. `null` (never loaded, or loading failed) sends nothing, so Rust keeps them. */
export function sectionsInput(sections: EditSection[] | null): SectionInput[] | undefined {
  if (sections === null) return undefined;
  return sections.map((s) => ({
    ...(s.id ? { id: s.id } : {}),
    title: s.title.trim() || null,
    fields: s.fields.map((f) => ({
      ...(f.id ? { id: f.id } : {}),
      label: f.label.trim(),
      value: valueInput(f),
    })),
  }));
}
```

Run the tests again. Expected: PASS.

- [ ] **Step 7: Dirty check includes sections**

In `lib/openItem.ts`, add to `EditorSnapshot`:

```ts
  /** `null` while a login's custom fields are loading or failed to load. */
  sections: EditSection[] | null;
```

(import the type from `./customFields`). In `lib/openItem.test.ts`, add `sections: null` to `base`, and add:

```ts
  it("sees a custom field edit", () => {
    const initial = { ...base, sections: [] };
    expect(isDirty(initial, { ...initial, sections: [] })).toBe(false);
    expect(isDirty(initial, { ...initial, sections: [{ key: "k", title: "x", fields: [] }] })).toBe(true);
  });
```

- [ ] **Step 8: Strings**

In `i18n/en.ts`, add a `fields` block after `editor`:

```ts
  fields: {
    types: {
      text: "Text",
      url: "URL",
      email: "Email",
      address: "Address",
      date: "Date",
      otp: "One-time password",
      password: "Password",
      phone: "Phone",
    },
    addField: "Add another field",
    addSection: "Add section",
    sectionTitle: "Section title",
    untitled: "Untitled section",
    label: "Label",
    removeField: (label: string) => `Remove ${label}`,
    removeSection: "Remove section",
    confirmRemoveSection: (n: number) =>
      n === 1 ? "Remove this section and its field?" : `Remove this section and its ${n} fields?`,
    move: (label: string) => `Move ${label} (Alt+Up / Alt+Down)`,
    moveSectionUp: "Move section up",
    moveSectionDown: "Move section down",
    copy: (label: string) => `Copy ${label}`,
    copied: (label: string, seconds: number) => `${label} copied. The clipboard clears in ${seconds} s.`,
    show: (label: string) => `Show ${label}`,
    hide: (label: string) => `Hide ${label}`,
    loadFailed: "Could not load this login's other fields.",
    limit: "This login has as many fields as it can hold.",
    set: "Saved.",
    removed: "Will be removed.",
  },
```

In `import`, next to `urlsMovedToNotes`:

```ts
    fieldsToNotes: (n: number) =>
      n === 1 ? "1 field over a login's limit was kept in its notes." : `${n} fields over a login's limit were kept in their notes.`,
```

In `i18n/pt-BR.ts`, the same keys:

```ts
  fields: {
    types: {
      text: "Texto",
      url: "URL",
      email: "E-mail",
      address: "Endereço",
      date: "Data",
      otp: "Código de verificação",
      password: "Senha",
      phone: "Telefone",
    },
    addField: "Adicionar outro campo",
    addSection: "Adicionar seção",
    sectionTitle: "Título da seção",
    untitled: "Seção sem título",
    label: "Rótulo",
    removeField: (label: string) => `Remover ${label}`,
    removeSection: "Remover seção",
    confirmRemoveSection: (n: number) =>
      n === 1 ? "Remover esta seção e o campo dela?" : `Remover esta seção e os ${n} campos dela?`,
    move: (label: string) => `Mover ${label} (Alt+↑ / Alt+↓)`,
    moveSectionUp: "Mover seção para cima",
    moveSectionDown: "Mover seção para baixo",
    copy: (label: string) => `Copiar ${label}`,
    copied: (label: string, seconds: number) =>
      `${label} copiado. A área de transferência será limpa em ${seconds} s.`,
    show: (label: string) => `Mostrar ${label}`,
    hide: (label: string) => `Ocultar ${label}`,
    loadFailed: "Não foi possível carregar os outros campos deste login.",
    limit: "Este login já tem o máximo de campos.",
    set: "Salvo.",
    removed: "Será removido.",
  },
```

and `fieldsToNotes: (n: number) => n === 1 ? "1 campo acima do limite de um login ficou nas notas dele." : \`${n} campos acima do limite de um login ficaram nas notas deles.\``.

Match the wording of the existing pt-BR `copied` strings (e.g. `t.detail.copied.password` in `pt-BR.ts`) so the toast reads the same way; if theirs says "Senha copiada. A área de transferência é limpa em…", mirror that verb tense.

- [ ] **Step 9: Verify**

Run (in `apps/desktop`): `pnpm typecheck && pnpm test`
Expected: typecheck errors only in `ItemEditor.tsx`, which does not pass `sections` in its snapshot yet. Task 7 fixes that; for now add `sections: null` to its `snapshot` literal so this task ends green. Re-run: PASS.

- [ ] **Step 10: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop-ui): custom field types, API, editor model and strings

customFields.ts is the editor's pure model: views to editable sections,
moves across section edges, drag drops and the save payload; sections
that never loaded send nothing, so Rust keeps them. useTotp takes a
loader so OTP fields reuse it; formatDay shows a date without a zone
shift."
```

---

### Task 7: Desktop UI — detail sections and the field editor

**Files:**
- Create: `apps/desktop/src/components/TotpEdit.tsx`
- Create: `apps/desktop/src/views/CustomSections.tsx`
- Create: `apps/desktop/src/views/CustomFieldsEditor.tsx`
- Modify: `apps/desktop/src/views/ItemDetail.tsx`, `apps/desktop/src/views/ItemEditor.tsx`, `apps/desktop/src/views/ImportSection.tsx`
- Modify: the stylesheet holding `.edit-row` (`grep -rln "\.edit-row" apps/desktop/src`)

**Interfaces:**
- Consumes: everything Task 6 produces; the existing `Field`, `IconButton` (exported from `ItemDetail.tsx`), `CopyButton`, `Icon`, `useToast`, `useI18n`, `errorMessage`, `useRevealedSecret`, `canScan`, `scanLabel`, `KEEP`, `EMPTY`.
- Produces: `<CustomSections itemId updatedAt />`, `<CustomFieldsEditor sections onChange disabled />` and `<TotpEdit edit onChange disabled ariaLabel />`.

- [ ] **Step 1: Extract `TotpEdit` from the editor**

Create `components/TotpEdit.tsx` by moving the one-time-code row out of `ItemEditor.tsx`. That is the JSX from `<div className="row edit-row">` under `t.editor.oneTimeCodes` through the `scanChoices` block, plus `scanQr`, `pickScan` and their `scanning`/`scanChoices` state. Keep the same markup and classes:

```tsx
import { useState } from "react";
import { Icon } from "./Icon";
import { api } from "../lib/api";
import { canScan, EMPTY, KEEP, scanLabel, type SecretEdit } from "../lib/secretEdit";
import type { ScannedTotp } from "../lib/types";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/**
 * A one-time-code setup: kept, removed, scanned or typed. The login's own
 * TOTP and every OTP custom field use this row. A scan keeps the secret in
 * Rust; the editor only holds its token.
 */
export function TotpEdit({
  edit,
  onChange,
  disabled,
  ariaLabel,
  onError,
}: {
  edit: SecretEdit;
  onChange: (edit: SecretEdit) => void;
  disabled: boolean;
  ariaLabel: string;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [scanning, setScanning] = useState(false);
  const [choices, setChoices] = useState<ScannedTotp[] | null>(null);

  async function scan() {
    setScanning(true);
    setChoices(null);
    try {
      const found = await api.scanTotpQr();
      if (found.length === 1) pick(found[0]!);
      else setChoices(found);
    } catch (e) {
      onError(errorMessage(e, t, t.editor.scanFailed));
    } finally {
      setScanning(false);
    }
  }

  function pick(code: ScannedTotp) {
    setChoices(null);
    onChange({ mode: "scanned", token: code.token, label: scanLabel(code, t.editor.scanUnnamed) });
  }

  return (
    <>
      <div className="row edit-row">
        {/* The four branches exactly as they are in ItemEditor today, with
            setTotp → onChange, scanQr → scan, the input's aria-label →
            ariaLabel and the scan button's disabled → scanning || disabled. */}
      </div>
      {choices && choices.length > 1 && (
        <div className="row scan-choices">
          <span className="muted">{t.editor.scanPick}</span>
          {choices.map((code) => (
            <button key={code.token} type="button" className="btn btn-small" onClick={() => pick(code)}>
              {scanLabel(code, t.editor.scanUnnamed)}
            </button>
          ))}
        </div>
      )}
    </>
  );
}
```

Paste the four branches (`keep`, `clear`, `scanned`, typed) verbatim from `ItemEditor.tsx` into the marked place, applying the renames in the comment. Delete the comment. In the typed branch's `onChange`, also call `setChoices(null)`. `KEEP` and `EMPTY` are used by those branches.

In `ItemEditor.tsx`, replace the moved JSX with:

```tsx
            <TotpEdit
              edit={totp}
              onChange={setTotp}
              disabled={saving || readOnly}
              ariaLabel={t.editor.totpLabel}
              onError={setError}
            />
```

and delete `scanning`, `scanChoices`, `scanQr` and `pickScan` from the editor.

Run: `pnpm typecheck`. Expected: PASS. Then run `pnpm dev` and check that the main TOTP row still keeps, replaces, removes, scans and undoes.

- [ ] **Step 2: Detail view `CustomSections.tsx`**

```tsx
import { Fragment, useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import { formatDay, groupCode } from "../lib/format";
import { useRevealedSecret, useTotp } from "../lib/hooks";
import type { FieldView, SectionView } from "../lib/types";
import { CopyButton } from "../components/CopyButton";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { Field, IconButton } from "./ItemDetail";

/**
 * A login's custom fields, under its built-in ones (spec
 * 2026-09-30-login-custom-fields §5.2). Loaded when the login opens or
 * changes; Password values only on the eye, copies done in Rust.
 */
export function CustomSections({ itemId, updatedAt }: { itemId: string; updatedAt: number }) {
  const { t } = useI18n();
  const [sections, setSections] = useState<SectionView[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let live = true;
    setFailed(false);
    api.loginFields(itemId).then(
      (s) => live && setSections(s),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
      setSections(null);
    };
  }, [itemId, updatedAt]);

  if (failed) return <p className="group-note">{t.fields.loadFailed}</p>;
  if (!sections) return null;
  return (
    <>
      {sections.map((s) => (
        <Fragment key={s.id}>
          {s.title && <h3 className="group-title">{s.title}</h3>}
          <div className="group">
            {s.fields.map((f) => (
              <FieldRow key={f.id} itemId={itemId} field={f} />
            ))}
          </div>
        </Fragment>
      ))}
    </>
  );
}

function FieldRow({ itemId, field }: { itemId: string; field: FieldView }) {
  const { t, dateLocale } = useI18n();
  const toast = useToast();
  const copy = useCallback(async () => {
    try {
      const r = await api.copyLoginField(itemId, field.id);
      toast(t.fields.copied(field.label, r.clearAfterSeconds));
      return true;
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
      return false;
    }
  }, [itemId, field.id, field.label, toast, t]);
  const copyButton = <CopyButton label={t.fields.copy(field.label)} onCopy={copy} />;

  switch (field.type) {
    case "password":
      return <PasswordRow itemId={itemId} field={field} copyButton={copyButton} />;
    case "otp":
      return field.hasOtp ? (
        <OtpRow itemId={itemId} fieldId={field.id} label={field.label} copyButton={copyButton} />
      ) : (
        <Field label={field.label}>
          <span className="muted">—</span>
        </Field>
      );
    case "address":
      return (
        <Field label={field.label} actions={field.formatted ? copyButton : undefined}>
          <span className="selectable multiline">{field.formatted || "—"}</span>
        </Field>
      );
    case "url":
      return (
        <Field label={field.label} actions={field.value ? copyButton : undefined}>
          {field.value ? (
            <button
              type="button"
              className="url-link"
              title={t.detail.openWebsite}
              onClick={() =>
                void api
                  .openLoginFieldUrl(itemId, field.id)
                  .catch((e) => toast(errorMessage(e, t, t.detail.openWebsiteFailed), "error"))
              }
            >
              {field.value}
            </button>
          ) : (
            <span className="muted">—</span>
          )}
        </Field>
      );
    default:
      return (
        <Field label={field.label} actions={field.value ? copyButton : undefined}>
          <span className="selectable multiline">
            {field.type === "date" ? formatDay(field.value, dateLocale) : field.value || "—"}
          </span>
        </Field>
      );
  }
}

function PasswordRow({
  itemId,
  field,
  copyButton,
}: {
  itemId: string;
  field: Extract<FieldView, { type: "password" }>;
  copyButton: JSX.Element;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const load = useCallback(() => api.revealLoginField(itemId, field.id), [itemId, field.id]);
  const secret = useRevealedSecret(load);
  if (!field.hasValue) {
    return (
      <Field label={field.label}>
        <span className="muted">—</span>
      </Field>
    );
  }
  return (
    <Field
      label={field.label}
      actions={
        <>
          <IconButton
            icon={secret.value === null ? "eye" : "eyeOff"}
            label={secret.value === null ? t.fields.show(field.label) : t.fields.hide(field.label)}
            onClick={() =>
              secret.value === null
                ? void secret.reveal().catch((e) => toast(errorMessage(e, t, t.common.couldNotReveal), "error"))
                : secret.hide()
            }
          />
          {copyButton}
        </>
      }
    >
      <span className="secret" data-revealed={secret.value !== null}>
        {secret.value === null ? (
          <span className="mono masked" aria-label={t.common.hiddenPassword}>
            ••••••••••••
          </span>
        ) : (
          <span className="mono selectable revealed">{secret.value}</span>
        )}
      </span>
    </Field>
  );
}

function OtpRow({
  itemId,
  fieldId,
  label,
  copyButton,
}: {
  itemId: string;
  fieldId: string;
  label: string;
  copyButton: JSX.Element;
}) {
  const { t } = useI18n();
  const load = useCallback(() => api.loginFieldTotp(itemId, fieldId), [itemId, fieldId]);
  const { code, remaining, failed } = useTotp(load, `${itemId}/${fieldId}`, true);
  const period = code?.period ?? 30;
  const progress = code ? remaining / period : 0;
  return (
    <Field label={label} actions={copyButton}>
      {failed ? (
        <span className="muted">{t.detail.codeFailed}</span>
      ) : (
        <span className="totp">
          <span className="mono totp-code">{code ? groupCode(code.code) : "••• •••"}</span>
          <svg className={`totp-ring${remaining <= 5 ? " totp-ring-low" : ""}`} viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="10" cy="10" r="8" className="totp-ring-track" />
            <circle
              cx="10"
              cy="10"
              r="8"
              className="totp-ring-fill"
              strokeDasharray="50.27"
              strokeDashoffset={50.27 * (1 - progress)}
            />
          </svg>
          <span className="totp-seconds" aria-label={t.detail.secondsRemaining(remaining)}>
            {remaining}s
          </span>
        </span>
      )}
    </Field>
  );
}
```

`OtpRow`'s markup is the same as `TotpField` in `ItemDetail.tsx`. Make `TotpField` the one component: give it props `{ label, copyLabel?, load, loadKey, onCopy }`, export it, and use it for both. Replace `OtpRow`'s body with `<TotpField label={label} load={load} loadKey={`${itemId}/${fieldId}`} actions={copyButton} />`, and update the existing call in `ItemDetail`. That keeps one TOTP ring in the codebase. Check that `t.common.couldNotReveal` exists (grep `en.ts`); if it does not, use the same error string `ItemDetail`'s `onRevealError` uses. If `JSX.Element` is not in scope with this React version, use `ReactNode` from `react`.

In `ItemDetail.tsx`, for logins only, render `<CustomSections itemId={item.id} updatedAt={item.updatedAt} />` after the websites group and before notes.

- [ ] **Step 3: Editor `CustomFieldsEditor.tsx`**

```tsx
import { useRef, useState } from "react";
import { Icon } from "../components/Icon";
import { TotpEdit } from "../components/TotpEdit";
import {
  addField,
  FIELD_TYPES,
  fieldCount,
  MAX_FIELDS,
  MAX_SECTIONS,
  moveField,
  moveSection,
  newField,
  newSection,
  placeField,
  removeField,
  removeSection,
  updateField,
  updateSection,
  type EditField,
  type EditSection,
} from "../lib/customFields";
import { EMPTY, KEEP } from "../lib/secretEdit";
import type { AddressPart } from "../lib/types";
import { useI18n } from "../i18n/context";

const ADDRESS_PARTS: AddressPart[] = [
  "street",
  "number",
  "complement",
  "neighborhood",
  "city",
  "state",
  "postalCode",
  "country",
];

/** A login's custom fields in the editor (spec §5.3). */
export function CustomFieldsEditor({
  sections,
  onChange,
  disabled,
  onError,
}: {
  sections: EditSection[];
  onChange: (next: EditSection[]) => void;
  disabled: boolean;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [menuOpen, setMenuOpen] = useState(false);
  const dragKey = useRef<string | null>(null);
  const full = fieldCount(sections) >= MAX_FIELDS;

  function add(type: (typeof FIELD_TYPES)[number]) {
    setMenuOpen(false);
    onChange(addField(sections, newField(type, t.fields.types[type])));
  }

  function drop(sectionKey: string, beforeKey: string | null) {
    const key = dragKey.current;
    dragKey.current = null;
    if (key) onChange(placeField(sections, key, sectionKey, beforeKey));
  }

  return (
    <>
      {sections.map((s, si) => (
        <div key={s.key} className="cf-section">
          <div className="cf-section-head">
            <input
              className="edit-input group-title-input"
              value={s.title}
              onChange={(e) => onChange(updateSection(sections, s.key, e.target.value))}
              placeholder={t.fields.untitled}
              aria-label={t.fields.sectionTitle}
              maxLength={256}
              disabled={disabled}
            />
            <button
              type="button"
              className="icon-btn"
              onClick={() => onChange(moveSection(sections, s.key, -1))}
              disabled={disabled || si === 0}
              aria-label={t.fields.moveSectionUp}
              title={t.fields.moveSectionUp}
            >
              <Icon name="chevronUp" size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              onClick={() => onChange(moveSection(sections, s.key, 1))}
              disabled={disabled || si === sections.length - 1}
              aria-label={t.fields.moveSectionDown}
              title={t.fields.moveSectionDown}
            >
              <Icon name="chevronDown" size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              onClick={() => {
                if (s.fields.length === 0 || window.confirm(t.fields.confirmRemoveSection(s.fields.length))) {
                  onChange(removeSection(sections, s.key));
                }
              }}
              disabled={disabled}
              aria-label={t.fields.removeSection}
              title={t.fields.removeSection}
            >
              <Icon name="x" size={16} />
            </button>
          </div>
          <div className="group" onDragOver={(e) => e.preventDefault()} onDrop={() => drop(s.key, null)}>
            {s.fields.map((f) => (
              <FieldEditRow
                key={f.key}
                field={f}
                disabled={disabled}
                onChange={(change) => onChange(updateField(sections, f.key, change))}
                onRemove={() => onChange(removeField(sections, f.key))}
                onMove={(delta) => onChange(moveField(sections, f.key, delta))}
                onDragStart={() => (dragKey.current = f.key)}
                onDropBefore={() => drop(s.key, f.key)}
                onError={onError}
              />
            ))}
          </div>
        </div>
      ))}

      <div className="group">
        <div className="cf-add">
          <button
            type="button"
            className="row row-button add-row"
            onClick={() => setMenuOpen((o) => !o)}
            disabled={disabled || full}
            aria-expanded={menuOpen}
          >
            <Icon name="plus" size={15} /> {t.fields.addField}
          </button>
          {menuOpen && (
            <div className="cf-menu" role="menu">
              {FIELD_TYPES.map((type) => (
                <button key={type} type="button" role="menuitem" className="cf-menu-item" onClick={() => add(type)}>
                  {t.fields.types[type]}
                </button>
              ))}
            </div>
          )}
        </div>
        <button
          type="button"
          className="row row-button add-row"
          onClick={() => onChange([...sections, newSection()])}
          disabled={disabled || sections.length >= MAX_SECTIONS}
        >
          <Icon name="plus" size={15} /> {t.fields.addSection}
        </button>
      </div>
      {full && <p className="group-note">{t.fields.limit}</p>}
    </>
  );
}

function FieldEditRow({
  field,
  disabled,
  onChange,
  onRemove,
  onMove,
  onDragStart,
  onDropBefore,
  onError,
}: {
  field: EditField;
  disabled: boolean;
  onChange: (change: Partial<EditField>) => void;
  onRemove: () => void;
  onMove: (delta: -1 | 1) => void;
  onDragStart: () => void;
  onDropBefore: () => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [shown, setShown] = useState(false);

  return (
    <div
      className="row edit-row cf-row"
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.stopPropagation();
        onDropBefore();
      }}
    >
      <button
        type="button"
        className="icon-btn cf-handle"
        draggable={!disabled}
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = "move";
          onDragStart();
        }}
        onKeyDown={(e) => {
          if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
            e.preventDefault();
            onMove(e.key === "ArrowUp" ? -1 : 1);
            // Focus follows the row: React keeps the button by key.
            requestAnimationFrame(() => (e.target as HTMLElement).focus());
          }
        }}
        aria-label={t.fields.move(field.label)}
        title={t.fields.move(field.label)}
        disabled={disabled}
      >
        <Icon name="grip" size={16} />
      </button>
      <div className="cf-body">
        <input
          className="edit-input cf-label"
          value={field.label}
          onChange={(e) => onChange({ label: e.target.value })}
          aria-label={t.fields.label}
          maxLength={256}
          disabled={disabled}
        />
        <FieldValueEdit field={field} shown={shown} disabled={disabled} onChange={onChange} onError={onError} />
      </div>
      {field.type === "password" && field.secret.mode === "set" && (
        <button
          type="button"
          className="icon-btn"
          onClick={() => setShown((v) => !v)}
          aria-label={shown ? t.fields.hide(field.label) : t.fields.show(field.label)}
          title={shown ? t.fields.hide(field.label) : t.fields.show(field.label)}
        >
          <Icon name={shown ? "eyeOff" : "eye"} size={16} />
        </button>
      )}
      <button
        type="button"
        className="icon-btn"
        onClick={onRemove}
        disabled={disabled}
        aria-label={t.fields.removeField(field.label)}
        title={t.fields.removeField(field.label)}
      >
        <Icon name="x" size={16} />
      </button>
    </div>
  );
}

function FieldValueEdit({
  field,
  shown,
  disabled,
  onChange,
  onError,
}: {
  field: EditField;
  shown: boolean;
  disabled: boolean;
  onChange: (change: Partial<EditField>) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  switch (field.type) {
    case "otp":
      return (
        <TotpEdit
          edit={field.secret}
          onChange={(secret) => onChange({ secret })}
          disabled={disabled}
          ariaLabel={field.label}
          onError={onError}
        />
      );
    case "password":
      if (field.secret.mode === "keep" || field.secret.mode === "clear") {
        return (
          <div className="edit-secret">
            <span className="muted">{field.secret.mode === "keep" ? "••••••••••••" : t.fields.removed}</span>
            <span className="edit-secret-actions">
              {field.secret.mode === "keep" ? (
                <>
                  <button type="button" className="btn btn-small" onClick={() => onChange({ secret: EMPTY })}>
                    {t.editor.replace}
                  </button>
                  <button
                    type="button"
                    className="btn btn-small btn-quiet-danger"
                    onClick={() => onChange({ secret: { mode: "clear" } })}
                  >
                    {t.common.remove}
                  </button>
                </>
              ) : (
                <button type="button" className="btn btn-small" onClick={() => onChange({ secret: KEEP })}>
                  {t.common.undo}
                </button>
              )}
            </span>
          </div>
        );
      }
      return (
        <input
          className="edit-input mono"
          type={shown ? "text" : "password"}
          value={field.secret.mode === "set" ? field.secret.value : ""}
          onChange={(e) => onChange({ secret: { mode: "set", value: e.target.value } })}
          autoComplete="off"
          spellCheck={false}
          aria-label={field.label}
          disabled={disabled}
        />
      );
    case "address":
      return (
        <div className="cf-address">
          {ADDRESS_PARTS.map((part) => (
            <input
              key={part}
              className={`edit-input cf-part-${part}`}
              value={field.address[part] ?? ""}
              onChange={(e) => onChange({ address: { ...field.address, [part]: e.target.value } })}
              placeholder={t.identity.fields[part]}
              aria-label={`${field.label}: ${t.identity.fields[part]}`}
              maxLength={512}
              disabled={disabled}
            />
          ))}
        </div>
      );
    case "text":
      return (
        <textarea
          className="edit-area"
          value={field.text}
          onChange={(e) => onChange({ text: e.target.value })}
          aria-label={field.label}
          rows={1}
          spellCheck={false}
          disabled={disabled}
        />
      );
    default:
      return (
        <input
          className="edit-input"
          type={field.type === "date" ? "date" : field.type === "email" ? "email" : field.type === "phone" ? "tel" : "text"}
          inputMode={field.type === "url" ? "url" : undefined}
          value={field.text}
          onChange={(e) => onChange({ text: e.target.value })}
          aria-label={field.label}
          maxLength={field.type === "url" ? 2048 : 512}
          spellCheck={false}
          autoCapitalize="off"
          disabled={disabled}
        />
      );
  }
}
```

Icons: check `components/Icon.tsx` for `grip` and `chevronUp`. If one is missing, add it in the same style as the existing icons (a 24×24 path). `grip` is six dots and `chevronUp` is `chevronDown` rotated. Close the add menu on Escape and on outside click in the same way other menus in the app do; find one with `grep -rn "role=\"menu\"" apps/desktop/src`. If there is none, add a `keydown` Escape handler and a `blur` on the wrapper.

- [ ] **Step 4: Wire into `ItemEditor.tsx`**

- State:

```ts
  const [sections, setSections] = useState<EditSection[] | null>(
    itemType === "login" && isNew ? [] : null,
  );
```

- Load once for an existing login (next to the notes-loading effect):

```ts
  useEffect(() => {
    if (!existing || itemType !== "login") return;
    let cancelled = false;
    api
      .loginFields(existing.id)
      .then((views) => {
        if (cancelled) return;
        const loaded = fromViews(views);
        setSections(loaded);
        setInitialSnapshot((s) => ({ ...s, sections: loaded }));
      })
      .catch(() => {
        if (!cancelled) setError(t.fields.loadFailed);
      });
    return () => {
      cancelled = true;
    };
    // `t` is left out, as for the notes: a language change must not reload.
  }, [existing, itemType]);
```

- `const [initialSnapshot] = useState(snapshot);` becomes `const [initialSnapshot, setInitialSnapshot] = useState(snapshot);`, and `snapshot` gets `sections` instead of the `sections: null` added in Task 6.
- In `submit`'s login input, add `sections: sectionsInput(sections),`.
- Render, for logins, after the one-time-codes group and before the Browser group:

```tsx
          {sections !== null && (
            <CustomFieldsEditor
              sections={sections}
              onChange={setSections}
              disabled={saving || readOnly}
              onError={setError}
            />
          )}
```

While `sections === null` for an existing login (loading or failed), nothing renders and nothing is sent, so Rust keeps the fields (Review Focus 1).

- [ ] **Step 5: Import report line**

In `views/ImportSection.tsx`, next to the `urlsMovedToNotes` line:

```ts
  if (r.fieldsToNotes) out.push(t.import.fieldsToNotes(r.fieldsToNotes));
```

- [ ] **Step 6: Styles**

In the stylesheet that holds `.edit-row`, add rules that reuse the existing tokens (look at `.url-edit` and `.edit-secret` next to it for spacing variables, and use the same ones):
- `.cf-section-head`: flex, gap, align center;
- `.group-title-input`: styled like `.group-title` but editable, with no border until hover or focus;
- `.cf-row`: grid `auto 1fr auto auto`, align start;
- `.cf-handle`: `cursor: grab`;
- `.cf-body`: flex column, small gap;
- `.cf-label`: the smaller label font of `.edit-label`;
- `.cf-address`: a two-column grid, with `.cf-part-street` and `.cf-part-complement` spanning both columns;
- `.cf-add`: `position: relative`;
- `.cf-menu`: absolute, the popover surface and shadow tokens used by existing popovers (grep `box-shadow` near `select` or menu styles), z-index above rows;
- `.cf-menu-item`: a full-width left-aligned row button;
- `.multiline`: `white-space: pre-wrap`, if it does not already exist.

No new colours: tokens only. It must work at the app's narrowest window width.

- [ ] **Step 7: Verify**

Run (in `apps/desktop`): `pnpm typecheck && pnpm test`
Expected: PASS.

Run the app (`pnpm dev` at the repo root; see `docs/development.md` for the server it needs) and check by hand:
1. A new login: add each of the eight types, fill them, save. The detail view shows them in order; Copy works for each (paste to check); the eye reveals the Password; the OTP ring counts down; the URL opens the browser; the Date shows localized.
2. Edit: drag a Password field into a new section, Alt+↓ a field across a section edge, save without touching secrets. The Password still reveals the same value.
3. Clear the OTP field and save. It shows "—" with no copy button.
4. Remove a section with fields: it asks first.
5. Language pt-BR: every new string is translated.
6. Import a 1PUX with sections: the fields appear in their sections.

Run: `pnpm ui:check` (layout check, both languages).
Expected: PASS. Look at the screenshots for the editor and detail with custom fields; if the check has a fixture list, add a login with custom fields to it the way the card and identity fixtures were added (`git show d2c32f3 -- tools` for the pattern).

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop-ui): custom fields in the login detail and editor

The detail shows each section with its fields: Password behind the eye,
OTP as a live code, URL opened by Rust, every field copied by Rust. The
editor adds any of the eight types, groups them in titled sections, and
reorders by drag or Alt+Up/Down across sections. TotpEdit is now the one
one-time-code setup row for the login's TOTP and OTP fields."
```

---

### Task 8: Docs, audit and final verification

**Files:**
- Modify: `docs/security-model.md`, `docs/crypto.md`, `docs/architecture.md`

- [ ] **Step 1: Documentation**

- `docs/security-model.md`: in the section on what the desktop renderer receives (search for `reveal_card` or "renderer"), add a paragraph covering:
  - custom fields are desktop-only;
  - `login_fields` returns plain values and flags;
  - Password values come only through `reveal_login_field`;
  - OTP secrets never leave Rust (codes only);
  - copies and URL opens read from the vault by field id;
  - a save's field ids are checked against the same login.

  In the extension/native-messaging part, state that no request can name a custom field.
- `docs/crypto.md`: in the item details description, add `sections` (titles, labels and values) inside the login's encrypted details blob, with nothing in the overview.
- `docs/architecture.md`: add the five commands to the command list, and `custom_field.rs` to the core module list.

- [ ] **Step 2: Secret-leak and audit pass**

Run:

```bash
grep -rnE "console\.(log|debug|info)" apps/desktop/src | grep -v test
cargo test --workspace
cargo clippy -p havenkeys-core --all-targets -- -D warnings
cargo audit
cargo deny check
pnpm typecheck
pnpm -r test
```

Expected:
- the grep finds nothing new;
- the tests, lints and typecheck are all PASS or clean;
- `cargo audit` and `cargo deny` report nothing new, since no dependency was added.

Read the diff of `custom_field.rs` (core and desktop) once more for any `format!`, error text or `Debug` output that includes a label or value.

- [ ] **Step 3: Commit**

```bash
git add docs
git commit -m "docs: login custom fields in the security model, crypto and architecture"
```

- [ ] **Step 4: Hand back**

Report to the user:
- the branch and its commits;
- the manual checks that passed;
- anything skipped (e.g. desktop crate tests without WebKit).

Ask whether to merge `login-custom-fields` into `main` with `--no-ff`.
