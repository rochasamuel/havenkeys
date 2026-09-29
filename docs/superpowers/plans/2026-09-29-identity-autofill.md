# Identity Autofill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The browser extension fills sign-up and checkout forms from the account's one Identity after the user's click, with a second click before any document number (CPF, RG, passport, CNH).

**Architecture:** Rust decides what leaves the vault: three new native-messaging requests (`find_identity`, `fill_identity`, `open_identity`) answered by a new core module that checks the page (http(s), same-site frames, https for documents) and returns only the requested roles. The extension gets a separate identity classifier (it does not touch the login classifier), a new menu kind `identity`, a writer that handles `<input>`, `<select>`, `<textarea>` and date inputs without overwriting existing values, and a popup button.

**Tech Stack:** Rust (havenkeys-core, havenkeys-protocol, havenkeys-bridge), TypeScript (packages/protocol, apps/extension, MV3), vitest + jsdom.

**Spec:** `docs/superpowers/specs/2026-09-29-identity-autofill-design.md` (read it first; this plan implements it). Background: `docs/superpowers/specs/2026-09-29-identity-item-design.md` (the Identity item, already shipped).

## Global Constraints

- Project rules in `CLAUDE.md` apply: never log secrets or identity values; validate every message; no `innerHTML` for user data; Rust enforces security, never the extension.
- Commits: **no `Co-Authored-By` trailer** (user preference). Work on a branch `identity-autofill`; merge to `main` with `--no-ff` at the end only if the user asks.
- Roles (exact wire strings, camelCase): `fullName firstName middleName lastName email phone birthDate birthDay birthMonth birthYear company street number complement addressLine1 addressLine2 neighborhood city state postalCode country username cpf rg passport driversLicense`.
- Document roles: `cpf rg passport driversLicense`.
- `fill_identity.roles`: 1–40 entries, no duplicates. Response values: 1–4096 bytes each, at most 40, no duplicate roles.
- Rate limit classes: `find_identity` = Lookup; `fill_identity`, `open_identity` = Secret.
- Documents only when `documents: true` **and** the page URL is https. Cross-site frame → `Denied`.
- Identity fills never submit and never overwrite a non-empty value unless HavenKeys wrote that exact value.
- Derived values: `fullName` = `IdentityFields::display_name()`; `phone` = mobile, else home, else work; `birthDay`/`birthMonth` unpadded (`"20"`, `"4"`), `birthYear` 4 digits; `addressLine1` = `"street, number"` when both, else whichever; `addressLine2` = complement.
- Every user-visible string in both `apps/extension/src/i18n/en.ts` and `pt-BR.ts` (the `Messages` type enforces the same shape).
- Server tests need Postgres: run `scripts/test-server.sh` (Docker) when running the whole workspace; unit/crate tests below do not need it.

## Review Focus

1. **A page that changes the form between menu open and pick** (a field hidden or a new field added): the writer re-classifies at fill time and re-checks `isFillable`; a field hidden since open must stay empty. Pinned in Task 7 (`skips a field hidden after the menu opened`).
2. **A `<select>` whose options use codes the identity doesn't** (state "Distrito Federal" vs option "DF", country "Brasil" vs option "BR", accents): must match both ways or be skipped, never set to a wrong option. Pinned in Task 7 (`select matching`).
3. **A checkout's CPF field that the login classifier calls a username**: must open the identity menu, not an empty login menu; a gov.br-style login ("Entrar", CPF only) must still open the login menu. Pinned in Task 10 (`menuKindFor`).
4. **A `maxlength` shorter than the stored phone** (`+55 61 99999-0000` into `maxlength=11`): retry without the country code, else skip; never write a truncated number. Pinned in Task 7.
5. **An identity with no values, or a vault without an identity yet** (created at connect): the menu shows the "fill it in HavenKeys" row, never an error or a silent no-op. Pinned in Task 9 (`empty identity row`).

---

## File Structure

**Rust**
- Modify `crates/havenkeys-core/src/identity.rs` — add `FillRole` and `IdentityFields::fill_value`.
- Create `crates/havenkeys-core/src/identity_page.rs` — `impl VaultService { identity_summary_for_page, identity_values_for_page, identity_id_for_page }` with the page checks.
- Modify `crates/havenkeys-core/src/lib.rs` — `mod identity_page;` and re-export `IdentitySummary`.
- Create `crates/havenkeys-core/tests/identity_page.rs`.
- Modify `crates/havenkeys-protocol/src/lib.rs` (constants), `src/message.rs` (role enum, requests, results, validation); test in `tests/messages.rs`.
- Modify `crates/havenkeys-bridge/src/dispatch.rs`, `src/server.rs`; test in `tests/bridge.rs`.

**TypeScript**
- Modify `packages/protocol/src/index.ts`; create `packages/protocol/src/identity.ts` (roles), `identity-parity.test.ts`; extend `index.test.ts`.
- Create `apps/extension/src/autofill/identity.ts` (classifier) + `identity.test.ts`.
- Create `apps/extension/src/autofill/identity-fill.ts` (writer) + `identity-fill.test.ts`.
- Modify `apps/extension/src/messaging/inline.ts` (+ tests in a new `inline-identity.test.ts`).
- Modify `apps/extension/src/background/inline-handler.ts` (+ `inline-handler.test.ts`).
- Modify `apps/extension/src/content/index.ts` (+ `content.test.ts`).
- Modify `apps/extension/src/menu/menu.ts`, `menu/inline.css` (+ `menu.test.ts`).
- Modify `apps/extension/src/messaging/popup.ts`, `background/popup-handler.ts`, `background/index.ts`, `popup/popup.ts` (+ `popup-handler.test.ts`).
- Modify `apps/extension/src/i18n/en.ts`, `pt-BR.ts`; `apps/extension/manifest/firefox.json`.

**Docs**: `docs/autofill.md`, `docs/native-messaging.md`, `docs/security-model.md`, `CLAUDE.md` (§25 note), `README.md`, `tools/ui-check/extension.mjs`.

---

### Task 1: Core — fill roles and their values

**Files:**
- Modify: `crates/havenkeys-core/src/identity.rs`
- Test: same file, `mod tests`

**Interfaces:**
- Produces: `pub enum FillRole` (26 variants, names as in Global Constraints, PascalCase in Rust), `FillRole::ALL: [FillRole; 26]`, `FillRole::is_document(self) -> bool`, `IdentityFields::fill_value(&self, role: FillRole) -> Option<SecretString>`.

- [ ] **Step 1: Write the failing tests** (append inside `mod tests` in `identity.rs`)

```rust
    #[test]
    fn fill_values_are_derived_as_the_spec_says() {
        let f = IdentityFields {
            first_name: s("Samuel"),
            last_name: s("Rocha"),
            birth_date: s("2000-04-20"),
            home_phone: s("61 3333-4444"),
            work_phone: s("61 2222-0000"),
            street: s("Quadra 02"),
            number: s("10"),
            complement: s("Apto 3"),
            cpf: s("123.456.789-00"),
            ..Default::default()
        };
        let v = |r| f.fill_value(r).map(|x| x.expose().to_owned());
        assert_eq!(v(FillRole::FullName).as_deref(), Some("Samuel Rocha"));
        assert_eq!(v(FillRole::Phone).as_deref(), Some("61 3333-4444"), "home when no mobile");
        assert_eq!(v(FillRole::BirthDay).as_deref(), Some("20"));
        assert_eq!(v(FillRole::BirthMonth).as_deref(), Some("4"));
        assert_eq!(v(FillRole::BirthYear).as_deref(), Some("2000"));
        assert_eq!(v(FillRole::AddressLine1).as_deref(), Some("Quadra 02, 10"));
        assert_eq!(v(FillRole::AddressLine2).as_deref(), Some("Apto 3"));
        assert_eq!(v(FillRole::Cpf).as_deref(), Some("123.456.789-00"));
        assert_eq!(v(FillRole::Email), None);
        assert_eq!(v(FillRole::MiddleName), None);
    }

    #[test]
    fn mobile_wins_and_address_line_takes_what_exists() {
        let f = IdentityFields {
            mobile_phone: s("+55 61 99999-0000"),
            home_phone: s("61 3333-4444"),
            number: s("10"),
            ..Default::default()
        };
        assert_eq!(f.fill_value(FillRole::Phone).unwrap().expose(), "+55 61 99999-0000");
        assert_eq!(f.fill_value(FillRole::AddressLine1).unwrap().expose(), "10");
        assert!(IdentityFields::default().fill_value(FillRole::FullName).is_none());
    }

    #[test]
    fn document_roles() {
        let docs: Vec<_> = FillRole::ALL.iter().filter(|r| r.is_document()).collect();
        assert_eq!(
            docs,
            [&FillRole::Cpf, &FillRole::Rg, &FillRole::Passport, &FillRole::DriversLicense]
        );
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-core --lib identity::tests`
Expected: compile error, `FillRole` not found.

- [ ] **Step 3: Implement** (add after the `IdentityField` enum in `identity.rs`)

```rust
/// What a web form field asks for, as the browser extension classifies it
/// (spec 2026-09-29-identity-autofill §4). Some roles are derived from
/// several identity values; the extension never gets more than a role's
/// value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FillRole {
    FullName,
    FirstName,
    MiddleName,
    LastName,
    Email,
    Phone,
    BirthDate,
    BirthDay,
    BirthMonth,
    BirthYear,
    Company,
    Street,
    Number,
    Complement,
    AddressLine1,
    AddressLine2,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
    Username,
    Cpf,
    Rg,
    Passport,
    DriversLicense,
}

impl FillRole {
    pub const ALL: [FillRole; 26] = [
        FillRole::FullName,
        FillRole::FirstName,
        FillRole::MiddleName,
        FillRole::LastName,
        FillRole::Email,
        FillRole::Phone,
        FillRole::BirthDate,
        FillRole::BirthDay,
        FillRole::BirthMonth,
        FillRole::BirthYear,
        FillRole::Company,
        FillRole::Street,
        FillRole::Number,
        FillRole::Complement,
        FillRole::AddressLine1,
        FillRole::AddressLine2,
        FillRole::Neighborhood,
        FillRole::City,
        FillRole::State,
        FillRole::PostalCode,
        FillRole::Country,
        FillRole::Username,
        FillRole::Cpf,
        FillRole::Rg,
        FillRole::Passport,
        FillRole::DriversLicense,
    ];

    /// Document numbers: filled only after the user confirms them, and only
    /// on https pages.
    pub fn is_document(self) -> bool {
        matches!(
            self,
            FillRole::Cpf | FillRole::Rg | FillRole::Passport | FillRole::DriversLicense
        )
    }
}
```

and inside `impl IdentityFields`:

```rust
    /// The value for a form field of `role`, derived as the spec says; `None`
    /// when the identity has nothing for it.
    pub fn fill_value(&self, role: FillRole) -> Option<SecretString> {
        let get = |v: &Option<SecretString>| v.as_ref().map(|s| s.expose().to_owned());
        let date_part = |i: usize| -> Option<String> {
            let d = self.birth_date.as_ref()?.expose().to_owned();
            let part = d.split('-').nth(i)?.to_owned();
            // Day and month unpadded ("04" → "4"); the year as is.
            Some(if i == 0 { part } else { part.trim_start_matches('0').to_owned() })
        };
        let value = match role {
            FillRole::FullName => Some(self.display_name()).filter(|n| !n.is_empty()),
            FillRole::FirstName => get(&self.first_name),
            FillRole::MiddleName => get(&self.middle_name),
            FillRole::LastName => get(&self.last_name),
            FillRole::Email => get(&self.email),
            FillRole::Phone => get(&self.mobile_phone)
                .or_else(|| get(&self.home_phone))
                .or_else(|| get(&self.work_phone)),
            FillRole::BirthDate => get(&self.birth_date),
            FillRole::BirthDay => date_part(2),
            FillRole::BirthMonth => date_part(1),
            FillRole::BirthYear => date_part(0),
            FillRole::Company => get(&self.company),
            FillRole::Street => get(&self.street),
            FillRole::Number => get(&self.number),
            FillRole::Complement | FillRole::AddressLine2 => get(&self.complement),
            FillRole::AddressLine1 => match (get(&self.street), get(&self.number)) {
                (Some(s), Some(n)) => Some(format!("{s}, {n}")),
                (s, n) => s.or(n),
            },
            FillRole::Neighborhood => get(&self.neighborhood),
            FillRole::City => get(&self.city),
            FillRole::State => get(&self.state),
            FillRole::PostalCode => get(&self.postal_code),
            FillRole::Country => get(&self.country),
            FillRole::Username => get(&self.username),
            FillRole::Cpf => get(&self.cpf),
            FillRole::Rg => get(&self.rg),
            FillRole::Passport => get(&self.passport),
            FillRole::DriversLicense => get(&self.drivers_license),
        };
        value.map(SecretString::new)
    }
```

Note: the intermediate `String`s are plain; they are moved into `SecretString` (wiped on drop). That matches how `formatted_address` already works.

- [ ] **Step 4: Run to see it pass**

Run: `cargo test -p havenkeys-core --lib identity::tests`
Expected: all identity tests pass (16).

- [ ] **Step 5: Commit**

```bash
git checkout -b identity-autofill
git add crates/havenkeys-core/src/identity.rs
git commit -m "feat(core): identity fill roles and their derived values"
```

---

### Task 2: Core — page-bound identity access

**Files:**
- Create: `crates/havenkeys-core/src/identity_page.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `mod identity_page;` and `pub use identity_page::IdentitySummary;`)
- Test: `crates/havenkeys-core/tests/identity_page.rs`

**Interfaces:**
- Consumes: `FillRole`, `IdentityFields::fill_value` (Task 1); `VaultService::identity_item_id`, `reveal_identity`, `get_item` (existing); `crate::origin::{PageUrl, site_of}` (`site_of` is `pub(crate)`).
- Produces (all on `VaultService`):
  - `pub fn identity_summary_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<IdentitySummary>`
  - `pub fn identity_values_for_page(&self, page_url: &str, top_url: Option<&str>, roles: &[FillRole], documents: bool) -> Result<Vec<(FillRole, SecretString)>>`
  - `pub fn identity_id_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<Uuid>`
  - `pub struct IdentitySummary { pub title: String, pub email: Option<String>, pub roles: Vec<FillRole> }` (`Debug` shows only the role count).
- Errors: locked → `Locked`; unparseable page or top URL, or cross-site frame → `Denied`; no identity item → `NotFound`.

- [ ] **Step 1: Write the failing tests** (`crates/havenkeys-core/tests/identity_page.rs`)

```rust
//! Identity access for web pages (spec 2026-09-29-identity-autofill §6.2).

mod common;

use common::{activated_vault, secret, NOW};
use havenkeys_core::identity::{FillRole, IdentityFields};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;

fn some(v: &str) -> Option<havenkeys_core::SecretString> {
    Some(secret(v))
}

/// A vault whose identity holds a name, a phone, an address and a CPF.
fn with_identity() -> VaultService {
    let (mut v, _) = activated_vault();
    let staged = v.stage_identity_if_missing("user@example.com", NOW).unwrap().unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 1).unwrap();
    let fields = IdentityFields {
        first_name: some("Samuel"),
        last_name: some("Rocha"),
        email: some("user@example.com"),
        mobile_phone: some("+55 61 99999-0000"),
        postal_code: some("71266-105"),
        cpf: some("123.456.789-00"),
        ..Default::default()
    };
    let input = ItemInput {
        item_type: ItemType::Identity,
        title: String::new(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: Some(fields),
    };
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 2).unwrap();
    v
}

const SHOP: &str = "https://shop.example.com/checkout";

fn values(v: &VaultService, url: &str, top: Option<&str>, roles: &[FillRole], docs: bool) -> Vec<(FillRole, String)> {
    v.identity_values_for_page(url, top, roles, docs)
        .unwrap()
        .into_iter()
        .map(|(r, s)| (r, s.expose().to_owned()))
        .collect()
}

#[test]
fn the_summary_names_the_roles_with_a_value_and_no_values() {
    let v = with_identity();
    let s = v.identity_summary_for_page(SHOP, None).unwrap();
    assert_eq!(s.title, "Samuel Rocha");
    assert_eq!(s.email.as_deref(), Some("user@example.com"));
    for r in [FillRole::FullName, FillRole::Phone, FillRole::PostalCode, FillRole::Cpf] {
        assert!(s.roles.contains(&r), "{r:?}");
    }
    assert!(!s.roles.contains(&FillRole::City));
    assert!(!format!("{s:?}").contains("Samuel"));
}

#[test]
fn only_the_requested_roles_come_back() {
    let v = with_identity();
    let got = values(&v, SHOP, None, &[FillRole::FullName, FillRole::City], false);
    assert_eq!(got, vec![(FillRole::FullName, "Samuel Rocha".to_owned())]);
}

#[test]
fn documents_need_the_flag_and_https() {
    let v = with_identity();
    assert!(values(&v, SHOP, None, &[FillRole::Cpf], false).is_empty());
    assert_eq!(
        values(&v, SHOP, None, &[FillRole::Cpf], true),
        vec![(FillRole::Cpf, "123.456.789-00".to_owned())]
    );
    assert!(
        values(&v, "http://shop.example.com/", None, &[FillRole::Cpf], true).is_empty(),
        "never on http"
    );
    assert_eq!(
        values(&v, "http://shop.example.com/", None, &[FillRole::PostalCode], true).len(),
        1,
        "other values still fill on http"
    );
}

#[test]
fn frames_must_be_same_site_as_the_top_page() {
    let v = with_identity();
    let same = values(&v, "https://pay.example.com/f", Some(SHOP), &[FillRole::FullName], false);
    assert_eq!(same.len(), 1);
    assert!(matches!(
        v.identity_values_for_page(SHOP, Some("https://evil.com/"), &[FillRole::FullName], false),
        Err(Error::Denied)
    ));
    assert!(matches!(
        v.identity_summary_for_page(SHOP, Some("https://evil.com/")),
        Err(Error::Denied)
    ));
    assert!(matches!(
        v.identity_id_for_page(SHOP, Some("not a url")),
        Err(Error::Denied)
    ));
}

#[test]
fn non_web_pages_and_a_locked_vault_are_refused() {
    let mut v = with_identity();
    for bad in ["file:///etc/passwd", "chrome://settings", "javascript:alert(1)", ""] {
        assert!(matches!(v.identity_summary_for_page(bad, None), Err(Error::Denied)), "{bad}");
    }
    v.lock();
    assert!(matches!(v.identity_summary_for_page(SHOP, None), Err(Error::Locked)));
}

#[test]
fn a_vault_without_its_identity_says_not_found() {
    let (v, _) = activated_vault();
    assert!(matches!(v.identity_summary_for_page(SHOP, None), Err(Error::NotFound)));
    assert!(matches!(v.identity_id_for_page(SHOP, None), Err(Error::NotFound)));
}

#[test]
fn the_id_for_page_is_the_identity() {
    let v = with_identity();
    assert_eq!(v.identity_id_for_page(SHOP, None).unwrap(), v.identity_item_id().unwrap());
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-core --test identity_page`
Expected: compile error (`identity_summary_for_page` not found).

- [ ] **Step 3: Implement** (`crates/havenkeys-core/src/identity_page.rs`)

```rust
//! The Identity for web pages (spec 2026-09-29-identity-autofill §6.2).
//!
//! Unlike a login, the identity is not bound to a site: any page may ask.
//! What protects it is here and in the extension: only http(s) pages; a
//! frame only when it is the same site as the tab's page; only the roles
//! asked for; documents only when the user confirmed them and only on https.

use crate::error::{Error, Result};
use crate::identity::FillRole;
use crate::origin::{site_of, PageUrl};
use crate::secret::SecretString;
use crate::vault::VaultService;
use std::fmt;
use uuid::Uuid;

/// What the menu row needs: no values, only which roles have one.
pub struct IdentitySummary {
    pub title: String,
    pub email: Option<String>,
    pub roles: Vec<FillRole>,
}

impl fmt::Debug for IdentitySummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentitySummary")
            .field("roles", &self.roles.len())
            .finish_non_exhaustive()
    }
}

/// The frame's page, if it may be served at all.
fn checked_page(page_url: &str, top_url: Option<&str>) -> Result<PageUrl> {
    let page = PageUrl::parse(page_url).ok_or(Error::Denied)?;
    if let Some(top) = top_url {
        let top = PageUrl::parse(top).ok_or(Error::Denied)?;
        match (site_of(&page), site_of(&top)) {
            (Some(a), Some(b)) if a == b => {}
            _ => return Err(Error::Denied),
        }
    }
    Ok(page)
}

impl VaultService {
    /// The identity's ID, for a page that may use it.
    pub fn identity_id_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<Uuid> {
        let id = self.identity_item_id()?;
        checked_page(page_url, top_url)?;
        self.get_item(&id)?;
        Ok(id)
    }

    /// Title, email and the roles the identity has a value for.
    pub fn identity_summary_for_page(
        &self,
        page_url: &str,
        top_url: Option<&str>,
    ) -> Result<IdentitySummary> {
        let id = self.identity_id_for_page(page_url, top_url)?;
        let overview = self.get_item(&id)?;
        let fields = self.reveal_identity(&id)?;
        let roles = FillRole::ALL
            .into_iter()
            .filter(|r| fields.fill_value(*r).is_some())
            .collect();
        Ok(IdentitySummary {
            title: overview.title.clone(),
            email: overview.username.clone(),
            roles,
        })
    }

    /// The values for `roles`, in that order, skipping roles with no value.
    /// Documents only with `documents` and on an https page.
    pub fn identity_values_for_page(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        roles: &[FillRole],
        documents: bool,
    ) -> Result<Vec<(FillRole, SecretString)>> {
        let id = self.identity_item_id()?;
        let page = checked_page(page_url, top_url)?;
        let https = page.url().scheme() == "https";
        let fields = self.reveal_identity(&id)?;
        Ok(roles
            .iter()
            .filter(|r| !r.is_document() || (documents && https))
            .filter_map(|r| fields.fill_value(*r).map(|v| (*r, v)))
            .collect())
    }
}
```

`PageUrl::url()` is `pub(crate)`, so it is usable here; the `url` crate lowercases the scheme, so `HTTPS://…` counts as https.

`reveal_identity` returns `NotFound` when the item is missing because `load_details` does; `get_item` is called first in `identity_id_for_page` to answer `NotFound` before any decryption.

- [ ] **Step 4: Wire the module**: in `crates/havenkeys-core/src/lib.rs` add `pub mod identity_page;` after `pub mod identity;`. (Use `pub mod` so `IdentitySummary` is reachable as `havenkeys_core::identity_page::IdentitySummary`.)

- [ ] **Step 5: Run to see it pass**

Run: `cargo test -p havenkeys-core --test identity_page && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: 7 passed; no clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-core/src/identity_page.rs crates/havenkeys-core/src/lib.rs crates/havenkeys-core/tests/identity_page.rs
git commit -m "feat(core): identity access for web pages, same-site frames, documents on https only"
```

---

### Task 3: Protocol (Rust) — identity requests and results

**Files:**
- Modify: `crates/havenkeys-protocol/src/lib.rs`, `crates/havenkeys-protocol/src/message.rs`
- Test: `crates/havenkeys-protocol/tests/messages.rs`

**Interfaces:**
- Produces:
  - `pub const MAX_IDENTITY_ROLES: usize = 40;` `pub const MAX_IDENTITY_VALUE_BYTES: usize = 4096;`
  - `pub enum IdentityRole` — same 26 variants as `FillRole`, `#[serde(rename_all = "camelCase")]`, `Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`.
  - `Request::FindIdentity { url, top_url }`, `Request::FillIdentity { url, top_url, roles: Vec<IdentityRole>, documents: bool }`, `Request::OpenIdentity { url, top_url }` (kinds `find_identity`, `fill_identity`, `open_identity`).
  - `ResultBody::FindIdentity { title: String, email: Option<String>, roles: Vec<IdentityRole> }`, `ResultBody::FillIdentity { values: Vec<IdentityValue> }`, `ResultBody::OpenIdentity {}`.
  - `pub struct IdentityValue { pub role: IdentityRole, pub value: WireSecret }` (camelCase, `deny_unknown_fields`, `Debug` shows only the role).

- [ ] **Step 1: Write the failing tests** (append to `tests/messages.rs`)

```rust
#[test]
fn identity_requests_parse_and_are_bounded() {
    let ok = [
        r#"{"v":1,"id":1,"request":{"type":"find_identity","url":"https://shop.com/"}}"#,
        r#"{"v":1,"id":2,"request":{"type":"open_identity","url":"https://shop.com/","topUrl":"https://shop.com/"}}"#,
        r#"{"v":1,"id":3,"request":{"type":"fill_identity","url":"https://shop.com/","roles":["fullName","postalCode","cpf"],"documents":true}}"#,
    ];
    for s in ok {
        let env = parse(s).unwrap_or_else(|_| panic!("{s}"));
        assert!(matches!(
            env.request.kind(),
            "find_identity" | "open_identity" | "fill_identity"
        ));
    }
    let many = format!(
        r#"{{"v":1,"id":4,"request":{{"type":"fill_identity","url":"https://a.com/","roles":[{}],"documents":false}}}}"#,
        vec![r#""city""#; 41].join(",")
    );
    let bad = [
        r#"{"v":1,"id":5,"request":{"type":"fill_identity","url":"https://a.com/","roles":[],"documents":false}}"#.to_owned(),
        r#"{"v":1,"id":6,"request":{"type":"fill_identity","url":"https://a.com/","roles":["city","city"],"documents":false}}"#.to_owned(),
        r#"{"v":1,"id":7,"request":{"type":"fill_identity","url":"https://a.com/","roles":["password"],"documents":false}}"#.to_owned(),
        r#"{"v":1,"id":8,"request":{"type":"fill_identity","url":"https://a.com/","roles":["city"]}}"#.to_owned(),
        r#"{"v":1,"id":9,"request":{"type":"find_identity","url":"https://a.com/","itemId":"x"}}"#.to_owned(),
        many,
    ];
    for s in &bad {
        assert!(parse(s).is_err(), "{s}");
    }
}

#[test]
fn identity_results_are_validated() {
    let valid = [
        r#"{"v":1,"id":1,"result":{"type":"find_identity","title":"Samuel","email":null,"roles":["fullName","cpf"]}}"#,
        r#"{"v":1,"id":2,"result":{"type":"fill_identity","values":[{"role":"city","value":"Brasília"}]}}"#,
        r#"{"v":1,"id":3,"result":{"type":"fill_identity","values":[]}}"#,
        r#"{"v":1,"id":4,"result":{"type":"open_identity"}}"#,
    ];
    for s in valid {
        assert!(Outgoing::parse(s.as_bytes()).is_some(), "{s}");
    }
    let long = "x".repeat(MAX_IDENTITY_VALUE_BYTES + 1);
    let invalid = [
        r#"{"v":1,"id":1,"result":{"type":"fill_identity","values":[{"role":"city","value":""}]}}"#.to_owned(),
        r#"{"v":1,"id":1,"result":{"type":"fill_identity","values":[{"role":"city","value":"a"},{"role":"city","value":"b"}]}}"#.to_owned(),
        format!(r#"{{"v":1,"id":1,"result":{{"type":"fill_identity","values":[{{"role":"city","value":"{long}"}}]}}}}"#),
        r#"{"v":1,"id":1,"result":{"type":"find_identity","title":"x","email":null,"roles":["city","city"]}}"#.to_owned(),
    ];
    for s in &invalid {
        assert!(Outgoing::parse(s.as_bytes()).is_none(), "{s}");
    }
}

#[test]
fn identity_values_never_debug_print() {
    let v = IdentityValue {
        role: IdentityRole::Cpf,
        value: WireSecret::new("123.456.789-00".into()),
    };
    assert!(!format!("{v:?}").contains("123"));
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-protocol --test messages identity`
Expected: compile errors (`IdentityValue`, `MAX_IDENTITY_VALUE_BYTES` not found).

- [ ] **Step 3: Constants** (`src/lib.rs`, after `MAX_MATCHES`)

```rust
/// Roles one fill_identity may ask for (a form has fewer fields than this).
pub const MAX_IDENTITY_ROLES: usize = 40;
/// Largest identity value returned (the core's longest field is 4096 characters of a custom value; filled roles are far shorter).
pub const MAX_IDENTITY_VALUE_BYTES: usize = 4096;
```

- [ ] **Step 4: Types and requests** (`src/message.rs`)

Add to the `use crate::{…}` list: `MAX_IDENTITY_ROLES, MAX_IDENTITY_VALUE_BYTES`. Add `use std::collections::HashSet;`.

Add the role enum (after `SsoProvider`):

```rust
/// A form field's role for an identity fill. Mirrors
/// havenkeys_core::identity::FillRole; the bridge maps between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdentityRole {
    FullName,
    FirstName,
    MiddleName,
    LastName,
    Email,
    Phone,
    BirthDate,
    BirthDay,
    BirthMonth,
    BirthYear,
    Company,
    Street,
    Number,
    Complement,
    AddressLine1,
    AddressLine2,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
    Username,
    Cpf,
    Rg,
    Passport,
    DriversLicense,
}

/// One identity value for a fill.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityValue {
    pub role: IdentityRole,
    pub value: WireSecret,
}

impl fmt::Debug for IdentityValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdentityValue")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

fn roles_unique(roles: &[IdentityRole]) -> bool {
    let mut seen = HashSet::new();
    roles.iter().all(|r| seen.insert(*r))
}
```

Add variants to `enum Request` (before the closing brace):

```rust
    /// The account's Identity: title, email and which roles have a value.
    /// No values. Any http(s) page; a frame only when same-site as its tab.
    FindIdentity {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// The identity's values for `roles`. Documents only with `documents`
    /// and on an https page (the core decides).
    FillIdentity {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        roles: Vec<IdentityRole>,
        documents: bool,
    },
    /// Bring the desktop window forward on the identity. Returns nothing.
    OpenIdentity {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
```

In `kind()` add:

```rust
            Request::FindIdentity { .. } => "find_identity",
            Request::FillIdentity { .. } => "fill_identity",
            Request::OpenIdentity { .. } => "open_identity",
```

In `urls()` extend the big `|` arm with:

```rust
            | Request::FindIdentity { url, top_url }
            | Request::FillIdentity { url, top_url, .. }
            | Request::OpenIdentity { url, top_url }
```

In `field_sizes_ok`, add a third check and include it in the final `&&`:

```rust
        let identity_ok = match self {
            Request::FillIdentity { roles, .. } => {
                !roles.is_empty() && roles.len() <= MAX_IDENTITY_ROLES && roles_unique(roles)
            }
            _ => true,
        };
        urls_ok && login_ok && passkey_ok && identity_ok
```

Add to `enum ResultBody`:

```rust
    FindIdentity {
        title: String,
        email: Option<String>,
        /// Roles the identity has a value for; names only.
        roles: Vec<IdentityRole>,
    },
    FillIdentity {
        values: Vec<IdentityValue>,
    },
    OpenIdentity {},
```

In `ResultBody`'s `Debug`, add:

```rust
            ResultBody::FindIdentity { .. } => "find_identity",
            ResultBody::FillIdentity { .. } => "fill_identity",
            ResultBody::OpenIdentity {} => "open_identity",
```

In `Response::is_valid`, add arms before `_ => true`:

```rust
            Some(ResultBody::FindIdentity {
                title,
                email,
                roles,
            }) => {
                title.len() <= MAX_TITLE_BYTES
                    && email.as_ref().is_none_or(|e| e.len() <= MAX_USERNAME_BYTES)
                    && roles.len() <= MAX_IDENTITY_ROLES
                    && roles_unique(roles)
            }
            Some(ResultBody::FillIdentity { values }) => {
                let roles: Vec<IdentityRole> = values.iter().map(|v| v.role).collect();
                values.len() <= MAX_IDENTITY_ROLES
                    && roles_unique(&roles)
                    && values.iter().all(|v| {
                        !v.value.expose().is_empty()
                            && v.value.expose().len() <= MAX_IDENTITY_VALUE_BYTES
                    })
            }
```

- [ ] **Step 5: Run the protocol tests**

Run: `cargo test -p havenkeys-protocol`
Expected: all pass, including the three new tests. If a `match` elsewhere in the crate (e.g. the fuzz test in `tests/messages.rs`) lists every request kind, add the new kinds there.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-protocol
git commit -m "feat(protocol): find_identity, fill_identity and open_identity"
```

---

### Task 4: Bridge — dispatch, rate limits, security tests

**Files:**
- Modify: `crates/havenkeys-bridge/src/dispatch.rs`, `crates/havenkeys-bridge/src/server.rs`
- Test: `crates/havenkeys-bridge/tests/bridge.rs`

**Interfaces:**
- Consumes: Task 2's three `VaultService` methods; Task 3's protocol types.
- Produces: `Dispatched::OpenIdentity(Uuid)`; the bridge answers `ResultBody::OpenIdentity {}` after calling the existing open-item hook with the identity's ID.

- [ ] **Step 1: Write the failing tests** (append to `tests/bridge.rs`)

```rust
use havenkeys_core::identity::IdentityFields;

/// Give the fixture's vault its identity, with a name, postal code and CPF.
fn add_identity(f: &Fixture) -> Uuid {
    let mut v = f.vault.lock().unwrap();
    let staged = v
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 50).unwrap();
    let input = ItemInput {
        item_type: ItemType::Identity,
        title: String::new(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: Some(IdentityFields {
            first_name: Some(SecretString::from("Samuel")),
            postal_code: Some(SecretString::from("71266-105")),
            cpf: Some(SecretString::from("123.456.789-00")),
            ..Default::default()
        }),
    };
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 51).unwrap();
    id
}

fn fill_identity(f: &Fixture, url: &str, top: Option<&str>, roles: &[&str], documents: bool) -> serde_json::Value {
    let mut req = serde_json::json!({"type": "fill_identity", "url": url, "roles": roles, "documents": documents});
    if let Some(t) = top {
        req["topUrl"] = serde_json::json!(t);
    }
    call(f, req)
}

#[test]
fn identity_is_found_and_filled_by_role() {
    let f = fixture();
    add_identity(&f);
    let found = call(&f, serde_json::json!({"type": "find_identity", "url": "https://shop.com/"}));
    assert_eq!(found["result"]["title"], "Samuel");
    let roles = found["result"]["roles"].as_array().unwrap();
    assert!(roles.contains(&serde_json::json!("cpf")), "names only");
    assert!(!found.to_string().contains("123.456"), "no values in find_identity");

    let r = fill_identity(&f, "https://shop.com/", None, &["firstName", "postalCode"], false);
    assert_eq!(
        r["result"]["values"],
        serde_json::json!([{"role": "firstName", "value": "Samuel"}, {"role": "postalCode", "value": "71266-105"}])
    );
}

#[test]
fn identity_documents_need_confirmation_and_https() {
    let f = fixture();
    add_identity(&f);
    let no = fill_identity(&f, "https://shop.com/", None, &["cpf"], false);
    assert_eq!(no["result"]["values"], serde_json::json!([]));
    let yes = fill_identity(&f, "https://shop.com/", None, &["cpf"], true);
    assert_eq!(yes["result"]["values"][0]["value"], "123.456.789-00");
    let http = fill_identity(&f, "http://shop.com/", None, &["cpf"], true);
    assert_eq!(http["result"]["values"], serde_json::json!([]));
}

/// Attack: evil.com embeds shop.com's checkout in an iframe to harvest the
/// identity through the user's click.
#[test]
fn identity_denied_to_a_cross_site_frame() {
    let f = fixture();
    add_identity(&f);
    let r = fill_identity(&f, "https://shop.com/checkout", Some("https://evil.com/"), &["firstName"], false);
    assert_eq!(error_code(&r), Some("denied"));
    let r = call(&f, serde_json::json!({"type": "find_identity", "url": "https://shop.com/", "topUrl": "https://evil.com/"}));
    assert_eq!(error_code(&r), Some("denied"));
}

#[test]
fn identity_requests_respect_lock_integration_and_absence() {
    let f = fixture();
    let r = call(&f, serde_json::json!({"type": "find_identity", "url": "https://shop.com/"}));
    assert_eq!(error_code(&r), Some("not_found"), "no identity yet");
    add_identity(&f);
    f.vault
        .lock()
        .unwrap()
        .update_settings(Settings { browser_integration: false, ..Settings::default() })
        .unwrap();
    let r = call(&f, serde_json::json!({"type": "find_identity", "url": "https://shop.com/"}));
    assert_eq!(error_code(&r), Some("integration_disabled"));
    f.vault.lock().unwrap().lock();
    let r = fill_identity(&f, "https://shop.com/", None, &["firstName"], false);
    assert_eq!(error_code(&r), Some("locked"));
}

#[test]
fn open_identity_opens_it_through_the_hook() {
    let f = fixture();
    let id = add_identity(&f);
    let r = call(&f, serde_json::json!({"type": "open_identity", "url": "https://shop.com/"}));
    assert_eq!(r["result"], serde_json::json!({"type": "open_identity"}));
    assert_eq!(*f.opened.lock().unwrap(), vec![id]);
}

#[test]
fn fill_identity_shares_the_secret_rate_limit() {
    let f = fixture();
    add_identity(&f);
    for _ in 0..10 {
        fill_identity(&f, "https://shop.com/", None, &["firstName"], false);
    }
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
}

/// Attack: a request for a thousand roles.
#[test]
fn fill_identity_with_too_many_roles_is_rejected() {
    let f = fixture();
    add_identity(&f);
    let roles = vec!["city"; 1000];
    let r = fill_identity(&f, "https://shop.com/", None, &roles, false);
    assert!(matches!(error_code(&r), Some("invalid_input") | Some("malformed") | Some("too_large")));
}
```

Also check `Settings` is already imported in `bridge.rs` (it is: `use havenkeys_core::model::{…, Settings, …}`).

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-bridge --test bridge identity`
Expected: failures: the dispatcher's `match` is non-exhaustive (compile error).

- [ ] **Step 3: Dispatch** (`src/dispatch.rs`)

Imports: add `use havenkeys_core::identity::FillRole;` and `IdentityRole, IdentityValue` to the `havenkeys_protocol::{…}` import.

Add mapping functions near `wire_provider`:

```rust
fn core_role(r: IdentityRole) -> FillRole {
    match r {
        IdentityRole::FullName => FillRole::FullName,
        IdentityRole::FirstName => FillRole::FirstName,
        IdentityRole::MiddleName => FillRole::MiddleName,
        IdentityRole::LastName => FillRole::LastName,
        IdentityRole::Email => FillRole::Email,
        IdentityRole::Phone => FillRole::Phone,
        IdentityRole::BirthDate => FillRole::BirthDate,
        IdentityRole::BirthDay => FillRole::BirthDay,
        IdentityRole::BirthMonth => FillRole::BirthMonth,
        IdentityRole::BirthYear => FillRole::BirthYear,
        IdentityRole::Company => FillRole::Company,
        IdentityRole::Street => FillRole::Street,
        IdentityRole::Number => FillRole::Number,
        IdentityRole::Complement => FillRole::Complement,
        IdentityRole::AddressLine1 => FillRole::AddressLine1,
        IdentityRole::AddressLine2 => FillRole::AddressLine2,
        IdentityRole::Neighborhood => FillRole::Neighborhood,
        IdentityRole::City => FillRole::City,
        IdentityRole::State => FillRole::State,
        IdentityRole::PostalCode => FillRole::PostalCode,
        IdentityRole::Country => FillRole::Country,
        IdentityRole::Username => FillRole::Username,
        IdentityRole::Cpf => FillRole::Cpf,
        IdentityRole::Rg => FillRole::Rg,
        IdentityRole::Passport => FillRole::Passport,
        IdentityRole::DriversLicense => FillRole::DriversLicense,
    }
}

fn wire_role(r: FillRole) -> IdentityRole {
    match r {
        FillRole::FullName => IdentityRole::FullName,
        FillRole::FirstName => IdentityRole::FirstName,
        FillRole::MiddleName => IdentityRole::MiddleName,
        FillRole::LastName => IdentityRole::LastName,
        FillRole::Email => IdentityRole::Email,
        FillRole::Phone => IdentityRole::Phone,
        FillRole::BirthDate => IdentityRole::BirthDate,
        FillRole::BirthDay => IdentityRole::BirthDay,
        FillRole::BirthMonth => IdentityRole::BirthMonth,
        FillRole::BirthYear => IdentityRole::BirthYear,
        FillRole::Company => IdentityRole::Company,
        FillRole::Street => IdentityRole::Street,
        FillRole::Number => IdentityRole::Number,
        FillRole::Complement => IdentityRole::Complement,
        FillRole::AddressLine1 => IdentityRole::AddressLine1,
        FillRole::AddressLine2 => IdentityRole::AddressLine2,
        FillRole::Neighborhood => IdentityRole::Neighborhood,
        FillRole::City => IdentityRole::City,
        FillRole::State => IdentityRole::State,
        FillRole::PostalCode => IdentityRole::PostalCode,
        FillRole::Country => IdentityRole::Country,
        FillRole::Username => IdentityRole::Username,
        FillRole::Cpf => IdentityRole::Cpf,
        FillRole::Rg => IdentityRole::Rg,
        FillRole::Passport => IdentityRole::Passport,
        FillRole::DriversLicense => IdentityRole::DriversLicense,
    }
}
```

Add a variant to `enum Dispatched`:

```rust
    /// The identity may be opened in the desktop; the caller runs the
    /// open-item hook once the vault lock is released.
    OpenIdentity(Uuid),
```

Add arms to `dispatch` (before the end of the `match`):

```rust
        Request::FindIdentity { url, top_url } => {
            require_enabled(v)?;
            let s = v
                .identity_summary_for_page(url, top_url.as_deref())
                .map_err(code)?;
            Ok(Dispatched::Done(ResultBody::FindIdentity {
                title: s.title,
                email: s.email,
                roles: s.roles.into_iter().map(wire_role).collect(),
            }))
        }
        Request::FillIdentity {
            url,
            top_url,
            roles,
            documents,
        } => {
            require_enabled(v)?;
            let roles: Vec<FillRole> = roles.iter().copied().map(core_role).collect();
            let values = v
                .identity_values_for_page(url, top_url.as_deref(), &roles, *documents)
                .map_err(code)?
                .into_iter()
                .map(|(role, value)| IdentityValue {
                    role: wire_role(role),
                    value: WireSecret::new(value.expose().to_owned()),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FillIdentity { values }))
        }
        Request::OpenIdentity { url, top_url } => {
            require_enabled(v)?;
            let id = v
                .identity_id_for_page(url, top_url.as_deref())
                .map_err(code)?;
            Ok(Dispatched::OpenIdentity(id))
        }
```

- [ ] **Step 4: Server** (`src/server.rs`)

In `handle`'s class `match`, add `| Request::FindIdentity { .. }` to the Lookup arm and `| Request::FillIdentity { .. } | Request::OpenIdentity { .. }` to the Secret arm (before `Some(RequestClass::Secret)`).

Where `Dispatched::OpenItem(id)` is handled (around line 210), add the identity arm with the same hook and its own result:

```rust
            Ok(Dispatched::OpenIdentity(id)) => {
                let hook = guard(&self.inner.on_open_item).clone();
                match hook {
                    Some(hook) => {
                        hook(id);
                        Ok(ResultBody::OpenIdentity {})
                    }
                    None => Err(ErrorCode::Internal),
                }
            }
```

(The desktop's hook emits `vault://open-item` with the ID; `VaultScreen` then opens the editor. For the identity it opens `IdentityEditor`, which the `selected?.itemType === "identity"` branch already handles.)

- [ ] **Step 5: Run the bridge and native-host tests**

Run: `cargo test -p havenkeys-bridge -p havenkeys-native-host && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass. If `havenkeys-native-host` has an exhaustive request list (grep `"open_item"` in `crates/havenkeys-native-host`), add the three kinds.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-bridge
git commit -m "feat(bridge): answer identity requests; same-site frames, documents confirmed, rate limited"
```

---

### Task 5: Protocol (TypeScript) — roles, requests, results, parity

**Files:**
- Create: `packages/protocol/src/identity.ts`, `packages/protocol/src/identity-parity.test.ts`
- Modify: `packages/protocol/src/index.ts`, `packages/protocol/src/index.test.ts`

**Interfaces:**
- Produces: `IDENTITY_ROLES: readonly IdentityRole[]` (the 26 strings in Global Constraints order), `type IdentityRole`, `DOCUMENT_ROLES`, `isIdentityRole(v): v is IdentityRole`, `MAX_IDENTITY_ROLES = 40`, `MAX_IDENTITY_VALUE_BYTES = 4096`; request types `find_identity`/`fill_identity`/`open_identity`; results `{type:"find_identity"; title; email; roles}`, `{type:"fill_identity"; values: Array<{role; value}>}`, `{type:"open_identity"}`.

- [ ] **Step 1: Write the failing tests**

`packages/protocol/src/identity-parity.test.ts`:

```ts
// Runs under Node in vitest; excluded from tsconfig because it uses Node APIs.
// The extension's role list must be the Rust protocol's (message.rs IdentityRole).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { IDENTITY_ROLES } from "./identity";

const RUST = join(__dirname, "../../../crates/havenkeys-protocol/src/message.rs");

describe("identity roles", () => {
  it("match havenkeys-protocol IdentityRole, in order", () => {
    const src = readFileSync(RUST, "utf8");
    const body = src.slice(src.indexOf("pub enum IdentityRole {"));
    const block = body.slice(0, body.indexOf("}"));
    const variants = [...block.matchAll(/^\s+([A-Z]\w*),$/gm)].map((m) => m[1] as string);
    const camel = variants.map((v) => v[0]!.toLowerCase() + v.slice(1));
    expect(camel).toEqual([...IDENTITY_ROLES]);
  });
});
```

Check `packages/protocol/tsconfig.json` excludes `sso-parity.test.ts`; add `identity-parity.test.ts` to the same `exclude` list.

Append to `index.test.ts`:

```ts
describe("identity results", () => {
  it("accepts well-formed identity results", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "find_identity", title: "Samuel", email: null, roles: ["fullName", "cpf"] } },
      { v: 1, id: 2, result: { type: "fill_identity", values: [{ role: "city", value: "Brasília" }] } },
      { v: 1, id: 3, result: { type: "fill_identity", values: [] } },
      { v: 1, id: 4, result: { type: "open_identity" } },
    ];
    for (const m of ok) expect(parseIncoming(m), JSON.stringify(m)).not.toBeNull();
  });

  it("rejects unknown roles, duplicates, empty and oversized values", () => {
    const bad = [
      { v: 1, id: 1, result: { type: "find_identity", title: "x", email: null, roles: ["password"] } },
      { v: 1, id: 1, result: { type: "find_identity", title: "x", email: null, roles: ["city", "city"] } },
      { v: 1, id: 1, result: { type: "fill_identity", values: [{ role: "city", value: "" }] } },
      { v: 1, id: 1, result: { type: "fill_identity", values: [{ role: "city", value: "x".repeat(4097) }] } },
      { v: 1, id: 1, result: { type: "fill_identity", values: [{ role: "city", value: "a", extra: 1 }] } },
      { v: 1, id: 1, result: { type: "open_identity", x: 1 } },
    ];
    for (const m of bad) expect(parseIncoming(m), JSON.stringify(m)).toBeNull();
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `pnpm --filter @havenkeys/protocol test` (or `cd packages/protocol && npx vitest run`)
Expected: failures (module `./identity` missing; parse returns null).

- [ ] **Step 3: Implement** `packages/protocol/src/identity.ts`

```ts
// Identity fill roles (spec 2026-09-29-identity-autofill §4). Mirrors
// IdentityRole in crates/havenkeys-protocol/src/message.rs, in the same
// order (identity-parity.test.ts checks).

export const IDENTITY_ROLES = [
  "fullName",
  "firstName",
  "middleName",
  "lastName",
  "email",
  "phone",
  "birthDate",
  "birthDay",
  "birthMonth",
  "birthYear",
  "company",
  "street",
  "number",
  "complement",
  "addressLine1",
  "addressLine2",
  "neighborhood",
  "city",
  "state",
  "postalCode",
  "country",
  "username",
  "cpf",
  "rg",
  "passport",
  "driversLicense",
] as const;

export type IdentityRole = (typeof IDENTITY_ROLES)[number];

/** Filled only after the user confirms them, and only on https pages. */
export const DOCUMENT_ROLES: readonly IdentityRole[] = ["cpf", "rg", "passport", "driversLicense"];

export const MAX_IDENTITY_ROLES = 40;
export const MAX_IDENTITY_VALUE_BYTES = 4096;

export function isIdentityRole(v: unknown): v is IdentityRole {
  return typeof v === "string" && (IDENTITY_ROLES as readonly string[]).includes(v);
}

export interface IdentityValue {
  role: IdentityRole;
  value: string;
}
```

In `index.ts`: `export * from "./identity";` and `import { isIdentityRole, MAX_IDENTITY_ROLES, MAX_IDENTITY_VALUE_BYTES, type IdentityRole, type IdentityValue } from "./identity";`.

Add to `Request`:

```ts
  | { type: "find_identity"; url: string; topUrl?: string }
  | { type: "fill_identity"; url: string; topUrl?: string; roles: IdentityRole[]; documents: boolean }
  | { type: "open_identity"; url: string; topUrl?: string }
```

Add to `Result`:

```ts
  | { type: "find_identity"; title: string; email: string | null; roles: IdentityRole[] }
  | { type: "fill_identity"; values: IdentityValue[] }
  | { type: "open_identity" }
```

Add parsers (above `parseResult`):

```ts
function parseRoles(v: unknown): IdentityRole[] | null {
  if (!Array.isArray(v) || v.length > MAX_IDENTITY_ROLES) return null;
  const out: IdentityRole[] = [];
  for (const r of v) {
    if (!isIdentityRole(r) || out.includes(r)) return null;
    out.push(r);
  }
  return out;
}

/** UTF-8 byte length, the unit the Rust side bounds values in. */
const utf8Length = (s: string) => new TextEncoder().encode(s).length;

function parseIdentityValue(v: unknown): IdentityValue | null {
  if (!isObj(v) || !hasExactKeys(v, ["role", "value"])) return null;
  if (!isIdentityRole(v.role) || !isStr(v.value) || v.value.length === 0 || utf8Length(v.value) > MAX_IDENTITY_VALUE_BYTES) return null;
  return { role: v.role, value: v.value };
}
```

and cases in `parseResult`:

```ts
    case "find_identity": {
      if (!hasExactKeys(v, ["type", "title", "email", "roles"]) || !isStr(v.title) || !isNullableStr(v.email)) return null;
      const roles = parseRoles(v.roles);
      return roles && { type: "find_identity", title: v.title, email: v.email, roles };
    }
    case "fill_identity": {
      if (!hasExactKeys(v, ["type", "values"]) || !Array.isArray(v.values) || v.values.length > MAX_IDENTITY_ROLES) return null;
      const values: IdentityValue[] = [];
      for (const x of v.values) {
        const p = parseIdentityValue(x);
        if (!p || values.some((y) => y.role === p.role)) return null;
        values.push(p);
      }
      return { type: "fill_identity", values };
    }
    case "open_identity":
      return hasExactKeys(v, ["type"]) ? { type: "open_identity" } : null;
```

- [ ] **Step 4: Run to see it pass**

Run: `cd packages/protocol && npx vitest run && npx tsc --noEmit -p .`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add packages/protocol
git commit -m "feat(protocol-ts): identity roles, requests and results, with a parity test"
```

---

### Task 6: Extension — identity field classifier

**Files:**
- Create: `apps/extension/src/autofill/identity.ts`
- Test: `apps/extension/src/autofill/identity.test.ts`

**Interfaces:**
- Consumes: `normalize`, `hasAny`, `MAX_HINT_CHARS` (`autofill/text.ts`); `groupRoot`, `MAX_GROUP_INPUTS`, `type Env` (`autofill/group.ts`); `IdentityRole`, `DOCUMENT_ROLES` (`@havenkeys/protocol`).
- Produces:
  - `type IdentityElement = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement`
  - `interface IdentityField { el: IdentityElement; role: IdentityRole; confidence: number; byAutocomplete: boolean }`
  - `interface IdentityGroup { root: ParentNode; fields: IdentityField[] }`
  - `identityRoleOf(el: IdentityElement): { role: IdentityRole; confidence: number; byAutocomplete: boolean } | null`
  - `isIdentityFillable(el: IdentityElement, env: Env): boolean`
  - `identityGroupFor(field: HTMLInputElement, env: Env): IdentityGroup | null` — null unless the group qualifies.
  - `findIdentityGroup(doc: Document, env: Env): IdentityGroup | null` — the first qualifying group, examining at most `MAX_GROUP_INPUTS` inputs.
  - `rolesOf(group: IdentityGroup): IdentityRole[]` — unique, in document order, at most 40.

- [ ] **Step 1: Write the failing tests** (`identity.test.ts`)

```ts
// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { findIdentityGroup, identityGroupFor, identityRoleOf, rolesOf } from "./identity";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}
const env: Env = { isVisible: visible, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends Element = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;
const role = (sel: string) => identityRoleOf($(sel))?.role ?? null;

beforeEach(() => page(""));

const BR_CHECKOUT = `<form>
  <label for="n">Nome completo</label><input id="n" name="nome">
  <label for="c">CPF</label><input id="c" name="cpf">
  <label for="t">Celular</label><input id="t" name="celular" type="tel">
  <label for="z">CEP</label><input id="z" name="cep">
  <label for="r">Logradouro</label><input id="r" name="logradouro">
  <label for="u">Número</label><input id="u" name="numero">
  <label for="m">Complemento</label><input id="m" name="complemento">
  <label for="b">Bairro</label><input id="b" name="bairro">
  <label for="d">Cidade</label><input id="d" name="cidade">
  <label for="s">UF</label><select id="s" name="uf"><option value="">--</option><option value="DF">DF</option></select>
  <button type="submit">Finalizar compra</button></form>`;

describe("identityRoleOf", () => {
  it("classifies a Brazilian checkout by its words", () => {
    page(BR_CHECKOUT);
    expect(role("#n")).toBe("fullName");
    expect(role("#c")).toBe("cpf");
    expect(role("#t")).toBe("phone");
    expect(role("#z")).toBe("postalCode");
    expect(role("#r")).toBe("street");
    expect(role("#u")).toBe("number");
    expect(role("#m")).toBe("complement");
    expect(role("#b")).toBe("neighborhood");
    expect(role("#d")).toBe("city");
    expect(role("#s")).toBe("state");
  });

  it("trusts autocomplete first, with section and shipping prefixes", () => {
    page(`<form>
      <input id="a" autocomplete="shipping given-name" name="x1">
      <input id="b" autocomplete="section-x billing family-name" name="x2">
      <input id="c" autocomplete="shipping address-line1" name="x3">
      <input id="d" autocomplete="postal-code" name="x4">
      <input id="e" autocomplete="bday-day" name="x5">
      <textarea id="f" autocomplete="street-address"></textarea>
      <input id="g" autocomplete="tel-national" name="x6">
    </form>`);
    expect(role("#a")).toBe("firstName");
    expect(role("#b")).toBe("lastName");
    expect(role("#c")).toBe("addressLine1");
    expect(role("#d")).toBe("postalCode");
    expect(role("#e")).toBe("birthDay");
    expect(role("#f")).toBe("addressLine1");
    expect(role("#g")).toBe("phone");
  });

  it("never classifies passwords, codes, cards or search boxes", () => {
    page(`<form>
      <input id="p" type="password" name="nome">
      <input id="o" autocomplete="one-time-code" name="cep">
      <input id="k" autocomplete="cc-name" name="nome">
      <input id="q" type="search" name="cidade">
      <input id="h" type="hidden" name="cpf">
    </form>`);
    for (const id of ["#p", "#o", "#k", "#q", "#h"]) expect(role(id), id).toBeNull();
  });

  it("reads a birth date input and English labels", () => {
    page(`<form><label>Date of birth<input id="b" type="date"></label><label>ZIP code<input id="z"></label></form>`);
    expect(role("#b")).toBe("birthDate");
    expect(role("#z")).toBe("postalCode");
  });
});

describe("identity groups", () => {
  it("qualify with two identity fields", () => {
    page(BR_CHECKOUT);
    const g = identityGroupFor($("#z"), env);
    expect(g).not.toBeNull();
    expect(rolesOf(g!)).toEqual(["fullName", "cpf", "phone", "postalCode", "street", "number", "complement", "neighborhood", "city", "state"]);
  });

  it("qualify with one field whose autocomplete names a non-email role", () => {
    page(`<form><input id="z" autocomplete="postal-code"></form>`);
    expect(identityGroupFor($("#z"), env)).not.toBeNull();
  });

  it("do not qualify for a newsletter's lone email box", () => {
    page(`<form><input id="e" type="email" autocomplete="email" name="email"><button>Subscribe</button></form>`);
    expect(identityGroupFor($("#e"), env)).toBeNull();
  });

  it("skip hidden and disabled fields", () => {
    page(`<form><input id="n" name="nome"><input id="c" name="cpf" hidden><input id="z" name="cep" disabled></form>`);
    expect(identityGroupFor($("#n"), env)).toBeNull();
  });

  it("findIdentityGroup picks the first qualifying form, bounded", () => {
    page(`<form><input name="q" type="search"></form>${BR_CHECKOUT}`);
    const g = findIdentityGroup(document, env);
    expect(g && rolesOf(g)[0]).toBe("fullName");
    page(`<form>${"<input name=x>".repeat(5000)}</form>`);
    expect(findIdentityGroup(document, env)).toBeNull();
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/autofill/identity.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement** `apps/extension/src/autofill/identity.ts`

```ts
// Identity field classification (spec 2026-09-29-identity-autofill §5.1).
//
// Separate from the login classifier: that one keeps treating these words as
// reasons *not* to fill a login. Pure reads of attributes compared against
// fixed keyword lists; nothing from the page is evaluated or inserted.
// Bounded like login groups: at most MAX_GROUP_INPUTS elements per group.

import { DOCUMENT_ROLES, MAX_IDENTITY_ROLES, type IdentityRole } from "@havenkeys/protocol";
import { groupRoot, MAX_GROUP_INPUTS, type Env } from "./group";
import { hasAny, MAX_HINT_CHARS, normalize } from "./text";

export type IdentityElement = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

export interface IdentityField {
  el: IdentityElement;
  role: IdentityRole;
  confidence: number;
  byAutocomplete: boolean;
}

export interface IdentityGroup {
  root: ParentNode;
  fields: IdentityField[];
}

/** Input types that can hold an identity value. */
const TEXT_TYPES = new Set(["text", "email", "tel", "number", "url", "date", ""]);
const THRESHOLD = 60;

/** autocomplete token → role. Prefixes (section-*, shipping, …) are stripped first. */
const AUTOCOMPLETE: Record<string, IdentityRole> = {
  name: "fullName",
  "given-name": "firstName",
  "additional-name": "middleName",
  "family-name": "lastName",
  email: "email",
  tel: "phone",
  "tel-national": "phone",
  bday: "birthDate",
  "bday-day": "birthDay",
  "bday-month": "birthMonth",
  "bday-year": "birthYear",
  organization: "company",
  "street-address": "addressLine1",
  "address-line1": "addressLine1",
  "address-line2": "addressLine2",
  "address-level3": "neighborhood",
  "address-level2": "city",
  "address-level1": "state",
  "postal-code": "postalCode",
  country: "country",
  "country-name": "country",
  username: "username",
};
const AC_PREFIX = /^(section-\S+|shipping|billing|home|work|mobile|fax|pager)$/;

/** Words per role, English and Portuguese, normalized (no accents). Order matters: first hit wins. */
const WORDS: Array<[IdentityRole, readonly string[]]> = [
  ["fullName", ["nome completo", "full name", "your name", "seu nome"]],
  ["firstName", ["first name", "given name", "primeiro nome", "firstname", "fname"]],
  ["middleName", ["middle name", "nome do meio"]],
  ["lastName", ["last name", "surname", "family name", "sobrenome", "lastname", "lname"]],
  ["cpf", ["cpf"]],
  ["rg", ["rg", "carteira de identidade", "registro geral"]],
  ["passport", ["passport", "passaporte"]],
  ["driversLicense", ["cnh", "driver license", "drivers license", "driving licence", "carteira de motorista"]],
  ["birthDate", ["data de nascimento", "nascimento", "date of birth", "birth date", "birthday", "birthdate", "dob"]],
  ["postalCode", ["cep", "zip", "zip code", "zipcode", "postal", "postal code", "postcode"]],
  ["neighborhood", ["bairro", "neighborhood", "district"]],
  ["complement", ["complemento", "apt", "apartment", "suite", "unit"]],
  ["number", ["numero", "number", "house number", "num"]],
  ["street", ["logradouro", "rua", "endereco", "street", "address", "address line 1"]],
  ["city", ["cidade", "city", "municipio", "town"]],
  ["state", ["estado", "uf", "state", "province", "region"]],
  ["country", ["pais", "country"]],
  ["phone", ["celular", "telefone", "phone", "mobile", "tel", "whatsapp"]],
  ["company", ["empresa", "company", "organization", "organizacao"]],
  ["email", ["email", "e mail"]],
  ["username", ["username", "user name", "nome de usuario"]],
  ["fullName", ["nome", "name"]],
];

/** Wording that means "not the person's data" even when a role word matches. */
const NEGATIVE = ["search", "busca", "pesquisar", "coupon", "cupom", "promo", "card", "cartao", "cvv", "cvc", "captcha", "quantity", "quantidade"];

const WORDS_ON_ATTRS = 70;
const WORDS_ON_TEXT = 60;

function attr(el: Element, n: string): string {
  return (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
}

function labelText(el: IdentityElement): string {
  const parts: string[] = [];
  for (const l of Array.from(el.labels ?? []).slice(0, 2)) parts.push((l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  return parts.join(" ");
}

function autocompleteRole(el: IdentityElement): IdentityRole | null {
  const tokens = attr(el, "autocomplete").toLowerCase().split(/\s+/).filter(Boolean).filter((t) => !AC_PREFIX.test(t));
  const last = tokens[tokens.length - 1];
  return last ? (AUTOCOMPLETE[last] ?? null) : null;
}

function kindOf(el: IdentityElement): "input" | "select" | "textarea" {
  return el instanceof HTMLSelectElement ? "select" : el instanceof HTMLTextAreaElement ? "textarea" : "input";
}

/** The field's identity role, or null. */
export function identityRoleOf(el: IdentityElement): { role: IdentityRole; confidence: number; byAutocomplete: boolean } | null {
  const tag = kindOf(el);
  const type = tag === "input" ? (el as HTMLInputElement).type.toLowerCase() : tag;
  if (tag === "input" && !TEXT_TYPES.has(type)) return null;
  const ac = attr(el, "autocomplete").toLowerCase();
  if (/\b(cc-|one-time-code|current-password|new-password)/.test(ac)) return null;
  const attrs = normalize(`${attr(el, "name")} ${attr(el, "id")}`);
  const text = normalize(`${attr(el, "placeholder")} ${attr(el, "aria-label")} ${attr(el, "title")} ${labelText(el)}`, MAX_HINT_CHARS * 3);
  if (hasAny(`${attrs} ${text}`, NEGATIVE) || attr(el, "role") === "search") return null;

  const fromAc = autocompleteRole(el);
  if (fromAc) return { role: fromAc, confidence: 1, byAutocomplete: true };
  if (type === "email") return { role: "email", confidence: 0.8, byAutocomplete: false };
  if (type === "tel") return { role: "phone", confidence: 0.8, byAutocomplete: false };

  for (const [role, words] of WORDS) {
    const score = hasAny(attrs, words) ? WORDS_ON_ATTRS : hasAny(text, words) ? WORDS_ON_TEXT : 0;
    if (score < THRESHOLD) continue;
    // A date input only takes a birth date.
    if (type === "date" && role !== "birthDate") return null;
    // A multi-line field only takes an address.
    if (tag === "textarea" && role !== "street" && role !== "addressLine1") return null;
    return { role: role === "street" && tag === "textarea" ? "addressLine1" : role, confidence: score / 100, byAutocomplete: false };
  }
  return null;
}

/** Visible, enabled, editable. */
export function isIdentityFillable(el: IdentityElement, env: Env): boolean {
  if (el.disabled || !env.isVisible(el)) return false;
  if (el instanceof HTMLSelectElement) return true;
  if (el.readOnly) return false;
  return !(el instanceof HTMLInputElement) || TEXT_TYPES.has(el.type.toLowerCase());
}

function classify(root: ParentNode, env: Env): IdentityField[] {
  const els = Array.from(root.querySelectorAll<IdentityElement>("input, select, textarea"))
    .slice(0, MAX_GROUP_INPUTS * 4)
    .filter((el) => isIdentityFillable(el, env))
    .slice(0, MAX_GROUP_INPUTS);
  const out: IdentityField[] = [];
  for (const el of els) {
    const r = identityRoleOf(el);
    if (r) out.push({ el, ...r });
  }
  return out;
}

/** Two identity fields, or one named by autocomplete that is not an email/username box. */
function qualifies(fields: IdentityField[]): boolean {
  if (fields.length >= 2) return true;
  const [only] = fields;
  return only !== undefined && only.byAutocomplete && only.role !== "email" && only.role !== "username";
}

/** The identity group around a field the user interacted with, if it qualifies. */
export function identityGroupFor(field: HTMLInputElement, env: Env): IdentityGroup | null {
  const root = groupRoot(field);
  const fields = classify(root, env);
  if (!fields.some((f) => f.el === field) || !qualifies(fields)) return null;
  return { root, fields };
}

/** The page's first qualifying group (popup fill), bounded. */
export function findIdentityGroup(doc: Document, env: Env): IdentityGroup | null {
  const candidates = Array.from(doc.querySelectorAll<HTMLInputElement>("input")).slice(0, MAX_GROUP_INPUTS);
  const seen = new Set<ParentNode>();
  for (const input of candidates) {
    const root = groupRoot(input);
    if (seen.has(root)) continue;
    seen.add(root);
    const fields = classify(root, env);
    if (qualifies(fields) && fields.length >= 2) return { root, fields };
  }
  return null;
}

/** The group's roles, unique, in document order. */
export function rolesOf(group: IdentityGroup): IdentityRole[] {
  const out: IdentityRole[] = [];
  for (const f of group.fields) if (!out.includes(f.role)) out.push(f.role);
  return out.slice(0, MAX_IDENTITY_ROLES);
}

export const isDocumentRole = (r: IdentityRole) => DOCUMENT_ROLES.includes(r);
```

Notes for the implementer:
- `groupRoot` accepts an `HTMLInputElement`. Keywords are whole normalized words (`hasAny`), so one- and two-letter tokens (`n`, `no`) are deliberately absent: an `id="n"` would otherwise read as a house number.
- The label test uses `<label for=…>`; jsdom supports `el.labels`.
- The 5000-input bound test must finish quickly; `classify` slices before filtering.

- [ ] **Step 4: Run to see it pass**

Run: `cd apps/extension && npx vitest run src/autofill/identity.test.ts && npx tsc --noEmit -p .`
Expected: all pass. Tune the keyword table (not the tests) if a fixture fails; every change must keep the other fixtures passing.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/identity.ts apps/extension/src/autofill/identity.test.ts
git commit -m "feat(extension): identity field classifier"
```

---

### Task 7: Extension — writing identity values

**Files:**
- Create: `apps/extension/src/autofill/identity-fill.ts`
- Test: `apps/extension/src/autofill/identity-fill.test.ts`

**Interfaces:**
- Consumes: `IdentityGroup`, `isIdentityFillable` (Task 6); `IdentityValue` (`@havenkeys/protocol`).
- Produces: `fillIdentity(group: IdentityGroup, values: readonly IdentityValue[], env: Env): number` (fields written); `matchOption(select: HTMLSelectElement, role: IdentityRole, value: string): number` (option index or −1).

- [ ] **Step 1: Write the failing tests** (`identity-fill.test.ts`)

```ts
// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { identityGroupFor } from "./identity";
import { fillIdentity, matchOption } from "./identity-fill";

let hiddenIds = new Set<string>();
const env: Env = { isVisible: (el) => !el.hidden && !hiddenIds.has(el.id), path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends HTMLElement = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;

beforeEach(() => {
  page("");
  hiddenIds = new Set();
});

const FORM = `<form>
  <input id="n" name="nome_completo" aria-label="Nome completo">
  <input id="c" name="cep" aria-label="CEP">
  <select id="uf" name="uf" aria-label="UF"><option value="">--</option><option value="SP">São Paulo</option><option value="DF">Distrito Federal</option></select>
  <select id="pais" name="pais" aria-label="País"><option value="US">United States</option><option value="BR">Brasil</option></select>
  <input id="t" name="celular" type="tel" maxlength="15">
  <input id="d" type="date" aria-label="Data de nascimento">
</form>`;

const group = () => identityGroupFor($("#n"), env)!;

describe("fillIdentity", () => {
  it("fills empty fields and fires input and change", () => {
    page(FORM);
    const events: string[] = [];
    $("#n").addEventListener("input", () => events.push("input"));
    $("#n").addEventListener("change", () => events.push("change"));
    const n = fillIdentity(group(), [{ role: "fullName", value: "Samuel Rocha" }, { role: "postalCode", value: "71266-105" }], env);
    expect(n).toBe(2);
    expect($("#n").value).toBe("Samuel Rocha");
    expect($("#c").value).toBe("71266-105");
    expect(events).toEqual(["input", "change"]);
    expect(document.body.innerHTML).not.toContain("Samuel");
  });

  it("never overwrites what the user typed, but updates its own values", () => {
    page(FORM);
    $("#n").value = "Typed by me";
    expect(fillIdentity(group(), [{ role: "fullName", value: "Samuel" }, { role: "postalCode", value: "1" }], env)).toBe(1);
    expect($("#n").value).toBe("Typed by me");
    expect(fillIdentity(group(), [{ role: "postalCode", value: "2" }], env)).toBe(1);
    expect($("#c").value).toBe("2");
  });

  it("skips a field hidden after the menu opened", () => {
    page(FORM);
    const g = group();
    hiddenIds.add("c");
    fillIdentity(g, [{ role: "postalCode", value: "71266-105" }], env);
    expect($("#c").value).toBe("");
  });

  it("puts a birth date into a date input as ISO", () => {
    page(FORM);
    fillIdentity(group(), [{ role: "birthDate", value: "2000-04-20" }], env);
    expect($("#d").value).toBe("2000-04-20");
  });

  it("retries a phone without the country code when maxlength is short", () => {
    page(FORM);
    fillIdentity(group(), [{ role: "phone", value: "+55 61 99999-0000" }], env);
    expect($("#t").value).toBe("61 99999-0000");
    page(FORM.replace('maxlength="15"', 'maxlength="5"'));
    expect(fillIdentity(group(), [{ role: "phone", value: "+55 61 99999-0000" }], env)).toBe(0);
    expect($("#t").value).toBe("");
  });
});

describe("select matching", () => {
  it("matches state names, UF codes and countries, ignoring case and accents", () => {
    page(FORM);
    const uf = $<HTMLSelectElement>("#uf");
    const pais = $<HTMLSelectElement>("#pais");
    expect(matchOption(uf, "state", "DF")).toBe(2);
    expect(matchOption(uf, "state", "distrito federal")).toBe(2);
    expect(matchOption(uf, "state", "Sao Paulo")).toBe(1);
    expect(matchOption(uf, "state", "Texas")).toBe(-1);
    expect(matchOption(pais, "country", "Brasil")).toBe(1);
    expect(matchOption(pais, "country", "Brazil")).toBe(1);
    expect(matchOption(pais, "country", "BR")).toBe(1);
  });

  it("selects the match and leaves an unmatched select alone", () => {
    page(FORM);
    fillIdentity(group(), [{ role: "state", value: "Distrito Federal" }, { role: "country", value: "Narnia" }], env);
    expect($<HTMLSelectElement>("#uf").value).toBe("DF");
    expect($<HTMLSelectElement>("#pais").value).toBe("US");
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/autofill/identity-fill.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement** `apps/extension/src/autofill/identity-fill.ts`

```ts
// Writing identity values into a form (spec 2026-09-29-identity-autofill §5.4).
//
// Only the group the user picked from; only empty fields or fields whose
// value we wrote; each field re-checked just before writing. Values go into
// `value` / `selectedIndex` only, never attributes, and nothing is logged.

import type { IdentityRole, IdentityValue } from "@havenkeys/protocol";
import type { Env } from "./group";
import { isIdentityFillable, type IdentityElement, type IdentityGroup } from "./identity";
import { normalize } from "./text";

/** What we last wrote into each element: a value equal to it is ours to replace. */
const written = new WeakMap<IdentityElement, string>();

const setters = {
  input: Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set,
  textarea: Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set,
};

/** The 27 Brazilian states: UF code and name. */
const UF: ReadonlyArray<readonly [string, string]> = [
  ["AC", "Acre"], ["AL", "Alagoas"], ["AP", "Amapa"], ["AM", "Amazonas"], ["BA", "Bahia"],
  ["CE", "Ceara"], ["DF", "Distrito Federal"], ["ES", "Espirito Santo"], ["GO", "Goias"],
  ["MA", "Maranhao"], ["MT", "Mato Grosso"], ["MS", "Mato Grosso do Sul"], ["MG", "Minas Gerais"],
  ["PA", "Para"], ["PB", "Paraiba"], ["PR", "Parana"], ["PE", "Pernambuco"], ["PI", "Piaui"],
  ["RJ", "Rio de Janeiro"], ["RN", "Rio Grande do Norte"], ["RS", "Rio Grande do Sul"],
  ["RO", "Rondonia"], ["RR", "Roraima"], ["SC", "Santa Catarina"], ["SP", "Sao Paulo"],
  ["SE", "Sergipe"], ["TO", "Tocantins"],
];
const BRAZIL = ["br", "bra", "brasil", "brazil"];

/** Everything the value may be written as in an option, normalized. */
function aliases(role: IdentityRole, value: string): string[] {
  const v = normalize(value);
  const out = new Set([v]);
  if (role === "state") {
    for (const [code, name] of UF) {
      const c = normalize(code);
      const n = normalize(name);
      if (v === c || v === n) {
        out.add(c);
        out.add(n);
      }
    }
  }
  if (role === "country" && BRAZIL.includes(v)) for (const b of BRAZIL) out.add(b);
  return [...out];
}

/** The index of the option matching `value` by value or text, or −1. */
export function matchOption(select: HTMLSelectElement, role: IdentityRole, value: string): number {
  const wanted = aliases(role, value);
  const options = Array.from(select.options).slice(0, 500);
  return options.findIndex((o) => wanted.includes(normalize(o.value)) || wanted.includes(normalize(o.textContent ?? "")));
}

function mine(el: IdentityElement): boolean {
  const current = el instanceof HTMLSelectElement ? String(el.selectedIndex) : el.value;
  return written.get(el) === current;
}

function isEmpty(el: IdentityElement): boolean {
  if (el instanceof HTMLSelectElement) return el.selectedIndex <= 0 || el.value === "";
  return el.value === "";
}

function announce(el: IdentityElement): void {
  el.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

/** A phone that fits the field's maxlength: as saved, then without +55. */
function phoneFor(el: HTMLInputElement, value: string): string | null {
  const max = el.maxLength;
  if (max < 0 || value.length <= max) return value;
  const national = value.replace(/^\+55\s*/, "");
  return national.length <= max ? national : null;
}

function write(el: IdentityElement, role: IdentityRole, value: string): boolean {
  if (el instanceof HTMLSelectElement) {
    const i = matchOption(el, role, value);
    if (i < 0) return false;
    el.selectedIndex = i;
    written.set(el, String(i));
    announce(el);
    return true;
  }
  let v = value;
  if (el instanceof HTMLInputElement) {
    if (el.type === "date" && !/^\d{4}-\d{2}-\d{2}$/.test(v)) return false;
    if (role === "phone") {
      const fit = phoneFor(el, v);
      if (fit === null) return false;
      v = fit;
    } else if (el.maxLength >= 0 && v.length > el.maxLength) return false;
  }
  el.focus({ preventScroll: true });
  const setter = el instanceof HTMLTextAreaElement ? setters.textarea : setters.input;
  if (setter) setter.call(el, v);
  else el.value = v;
  written.set(el, v);
  announce(el);
  return true;
}

/** Fill the group's fields from `values`. Returns how many were written. */
export function fillIdentity(group: IdentityGroup, values: readonly IdentityValue[], env: Env): number {
  const byRole = new Map(values.map((v) => [v.role, v.value] as const));
  let n = 0;
  for (const f of group.fields) {
    const value = byRole.get(f.role);
    if (value === undefined || !f.el.isConnected || !isIdentityFillable(f.el, env)) continue;
    if (!isEmpty(f.el) && !mine(f.el)) continue;
    if (write(f.el, f.role, value)) n++;
  }
  return n;
}
```

Note: the street-address `<textarea>` gets `addressLine1`'s value from Rust (street and number); the complement is a separate role. If the group has no `addressLine2` field, that's acceptable (spec §5.4 describes the multi-line value; keep it simple and document the limitation in `docs/autofill.md` in Task 13).

- [ ] **Step 4: Run to see it pass**

Run: `cd apps/extension && npx vitest run src/autofill/identity-fill.test.ts`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/identity-fill.ts apps/extension/src/autofill/identity-fill.test.ts
git commit -m "feat(extension): write identity values: selects, dates, phones; never overwrite"
```

---

### Task 8: Extension messaging — identity menu, fill payload, parsers

**Files:**
- Modify: `apps/extension/src/messaging/inline.ts`
- Test: create `apps/extension/src/messaging/inline-identity.test.ts`

**Interfaces:**
- Produces (in `inline.ts`):
  - `MenuKind = "login" | "otp" | "new_password" | "identity"`
  - `ContentRequest` `cs_open_menu` gains optional `roles?: IdentityRole[]` (1–40, unique, known roles). Required non-empty for kind `identity`; allowed for `login`; refused for other kinds.
  - `FillPayload` gains `{ kind: "identity"; values: IdentityValue[] }`.
  - `InlineRequest` gains `{ type: "menu_pick_identity"; token; documents: boolean }` and `{ type: "menu_open_identity"; token }`.
  - `IdentityRowView = { title: string; fills: number; documents: IdentityRole[]; documentsAllowed: boolean; empty: boolean }`.
  - `MenuView` `ready` state gains `identity: IdentityRowView | null`.
  - `BackgroundToContent` gains `{ type: "bg_identity_roles" }` (popup asks the top frame which roles its first identity form has); reply shape `IdentityRolesReply = { roles: IdentityRole[] }`, parsed by `parseIdentityRolesReply(v): IdentityRole[]`.

- [ ] **Step 1: Write the failing tests** (`inline-identity.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { parseBackgroundMessage, parseContentRequest, parseIdentityRolesReply, parseInlineRequest } from "./inline";

const TOKEN = "a".repeat(32);

describe("identity messages", () => {
  it("cs_open_menu carries the form's roles", () => {
    expect(parseContentRequest({ type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })).toEqual({
      type: "cs_open_menu",
      kind: "identity",
      roles: ["fullName", "cpf"],
    });
    expect(parseContentRequest({ type: "cs_open_menu", kind: "login", roles: ["email"] })).not.toBeNull();
    for (const bad of [
      { type: "cs_open_menu", kind: "identity" },
      { type: "cs_open_menu", kind: "identity", roles: [] },
      { type: "cs_open_menu", kind: "identity", roles: ["password"] },
      { type: "cs_open_menu", kind: "identity", roles: ["city", "city"] },
      { type: "cs_open_menu", kind: "otp", roles: ["city"] },
      { type: "cs_open_menu", kind: "identity", roles: Array(41).fill("city") },
    ]) {
      expect(parseContentRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("menu requests for the identity", () => {
    expect(parseInlineRequest({ type: "menu_pick_identity", token: TOKEN, documents: true })).toEqual({
      type: "menu_pick_identity",
      token: TOKEN,
      documents: true,
    });
    expect(parseInlineRequest({ type: "menu_open_identity", token: TOKEN })).not.toBeNull();
    expect(parseInlineRequest({ type: "menu_pick_identity", token: TOKEN })).toBeNull();
    expect(parseInlineRequest({ type: "menu_pick_identity", token: TOKEN, documents: "yes" })).toBeNull();
  });

  it("identity fills and role scans from the background", () => {
    const fill = {
      type: "bg_fill",
      origin: "https://shop.com",
      token: TOKEN,
      fill: { kind: "identity", values: [{ role: "city", value: "Brasília" }] },
      submit: false,
      totp: false,
    };
    expect(parseBackgroundMessage(fill)).toEqual(fill);
    expect(parseBackgroundMessage({ ...fill, submit: true })).toBeNull(); // identity never submits
    expect(parseBackgroundMessage({ ...fill, fill: { kind: "identity", values: [{ role: "x", value: "y" }] } })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_identity_roles" })).toEqual({ type: "bg_identity_roles" });
    expect(parseIdentityRolesReply({ roles: ["cpf", "city"] })).toEqual(["cpf", "city"]);
    expect(parseIdentityRolesReply({ roles: ["nope"] })).toEqual([]);
    expect(parseIdentityRolesReply(undefined)).toEqual([]);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/messaging/inline-identity.test.ts`
Expected: FAIL (identity kinds rejected / `parseIdentityRolesReply` missing).

- [ ] **Step 3: Implement** (`inline.ts`)

Imports: `import { CREDENTIAL_ID_BYTES, isB64Url, isIdentityRole, isUuid, MAX_IDENTITY_ROLES, MAX_IDENTITY_VALUE_BYTES, type IdentityRole, type IdentityValue, type SsoProvider } from "@havenkeys/protocol";`

Type changes:

```ts
export type MenuKind = "login" | "otp" | "new_password" | "identity";

// in ContentRequest:
  | { type: "cs_open_menu"; kind: MenuKind; explicit?: true; roles?: IdentityRole[] }

// in FillPayload:
  | { kind: "identity"; values: IdentityValue[] }

// in BackgroundToContent:
  /** Popup: which identity roles has this (top) frame's first identity form? */
  | { type: "bg_identity_roles" }

// in InlineRequest:
  /** Fill the identity; `documents`: the user confirmed the document fields. */
  | { type: "menu_pick_identity"; token: string; documents: boolean }
  /** Open the identity in the desktop app (the menu's empty-identity row). */
  | { type: "menu_open_identity"; token: string }

/** The menu's identity row. No values: counts and role names only. */
export interface IdentityRowView {
  title: string;
  /** Fields in the form the identity has a value for. */
  fills: number;
  /** Document roles the form asks for and the identity has. */
  documents: IdentityRole[];
  /** false on http pages: documents will not be filled. */
  documentsAllowed: boolean;
  /** The identity has no values at all (or does not exist yet). */
  empty: boolean;
}

// MenuView ready state:
  | { state: "ready"; kind: MenuKind; site: string; items: MenuItemView[]; passkeys: PasskeyRow[]; hint: MenuHint | null; identity: IdentityRowView | null };

export type IdentityRolesReply = { roles: IdentityRole[] };
```

`MENU_KINDS` gains `"identity"`.

Helpers and parser changes:

```ts
function parseRoleList(v: unknown): IdentityRole[] | null {
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_IDENTITY_ROLES) return null;
  const out: IdentityRole[] = [];
  for (const r of v) {
    if (!isIdentityRole(r) || out.includes(r)) return null;
    out.push(r);
  }
  return out;
}

export function parseIdentityRolesReply(v: unknown): IdentityRole[] {
  const o = obj(v);
  return (o && keysAre(o, ["roles"]) && parseRoleList(o.roles)) || [];
}
```

Replace the `cs_open_menu` case:

```ts
    case "cs_open_menu": {
      if (!MENU_KINDS.includes(o.kind as MenuKind)) return null;
      const kind = o.kind as MenuKind;
      const keys = Object.keys(o).filter((k) => k !== "type" && k !== "kind");
      if (keys.some((k) => k !== "explicit" && k !== "roles")) return null;
      if ("explicit" in o && o.explicit !== true) return null;
      let roles: IdentityRole[] | undefined;
      if ("roles" in o) {
        if (kind !== "identity" && kind !== "login") return null;
        const parsed = parseRoleList(o.roles);
        if (!parsed) return null;
        roles = parsed;
      } else if (kind === "identity") return null;
      return {
        type: "cs_open_menu",
        kind,
        ...(o.explicit === true ? { explicit: true as const } : {}),
        ...(roles ? { roles } : {}),
      };
    }
```

In `parseInlineRequest` add:

```ts
    case "menu_pick_identity":
      return keysAre(o, ["type", "token", "documents"]) && typeof o.documents === "boolean"
        ? { type: "menu_pick_identity", token, documents: o.documents }
        : null;
    case "menu_open_identity":
      return keysAre(o, ["type", "token"]) ? { type: "menu_open_identity", token } : null;
```

In `parseFill` add:

```ts
    case "identity": {
      if (!keysAre(o, ["kind", "values"]) || !Array.isArray(o.values) || o.values.length > MAX_IDENTITY_ROLES) return null;
      const values: IdentityValue[] = [];
      for (const x of o.values) {
        const v = obj(x);
        if (!v || !keysAre(v, ["role", "value"]) || !isIdentityRole(v.role) || typeof v.value !== "string") return null;
        if (v.value.length === 0 || v.value.length > MAX_IDENTITY_VALUE_BYTES || values.some((y) => y.role === v.role)) return null;
        values.push({ role: v.role, value: v.value });
      }
      return { kind: "identity", values };
    }
```

In `parseBackgroundMessage`'s `bg_fill` case, after parsing the fill, refuse an identity fill that submits:

```ts
      if (fill?.kind === "identity" && (o.submit || o.totp)) return null;
```

and add:

```ts
    case "bg_identity_roles":
      return keysAre(o, ["type"]) ? { type: "bg_identity_roles" } : null;
```

- [ ] **Step 4: Run to see it pass**

Run: `cd apps/extension && npx vitest run src/messaging && npx tsc --noEmit -p .`
Expected: the new test passes; `tsc` now reports errors in `menu.ts`, `inline-handler.ts` and `content/index.ts` for the new union members (`identity` missing from `MenuView`, non-exhaustive switches). Those are fixed in Tasks 9–11; to keep this commit green, add `identity: null` where `MenuView` `ready` values are built in `inline-handler.ts` (`menu_state`) and a `case "identity": return { filled: 0, pressing: null };` placeholder in `content/index.ts`'s `handleFill` switch, which Task 10 replaces.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/messaging apps/extension/src/background/inline-handler.ts apps/extension/src/content/index.ts
git commit -m "feat(extension): identity menu and fill messages, strictly parsed"
```

---

### Task 9: Background — the identity menu session

**Files:**
- Modify: `apps/extension/src/background/inline-handler.ts`
- Test: `apps/extension/src/background/inline-handler.test.ts`

**Interfaces:**
- Consumes: Task 5 protocol (`find_identity`, `fill_identity`, `open_identity`, `DOCUMENT_ROLES`); Task 8 messages.
- Produces: `openMenu(frame, kind, explicit, roles?)`; `MenuSession` gains `roles: IdentityRole[]` and `identity: IdentityRowView | null`; `menu_state` returns `identity`; `menu_pick_identity` fills; `menu_open_identity` opens.

Rules (spec §5.2, §7):
- kind `identity`: call `find_identity`. Locked → locked menu. `not_found` → row with `empty: true` (the identity is created at connect; the row opens the desktop). Any other error → `{ ok: false }`. `fills` = number of `roles` (from the content script) that are in the summary's roles; `documents` = those roles ∩ `DOCUMENT_ROLES`; `documentsAllowed` = `frame.url` starts with `https:`; `empty` = summary roles empty. If `fills === 0` and not empty and not explicit → `{ ok: false }` (nothing to fill).
- kind `login` with `roles` (a sign-up form's email field): the login flow as today, then the same `find_identity`; attach the row only when `fills > 0`; an error there leaves the login menu unchanged. Rows count +1 when attached.
- `menu_pick_identity`: session must have `identity` and not be `empty`. Roles to request: session roles minus documents, plus documents only when `req.documents && documentsAllowed`. `fill_identity`, then `pickFill(frame, token, { kind: "identity", values }, null)`. A result of 0 filled → `{ ok: false, message: t.menu.identityNothing }`.
- `menu_open_identity`: `open_identity`, close the menu.

- [ ] **Step 1: Write the failing tests** (append to `inline-handler.test.ts`; reuse that file's fake client and `frame` helpers — read its top ~80 lines first and follow them; the snippets below assume a `setup(replies)` helper returning `{ handler, sent, requests }` like the existing tests use. If the file's helper has another name, adapt the calls, not the assertions.)

```ts
describe("identity menu", () => {
  const summary = { type: "find_identity", title: "Samuel Rocha", email: "me@x.com", roles: ["fullName", "postalCode", "cpf"] };

  it("offers the identity with a count and the documents the form asks for", async () => {
    const { handler, requests } = setup({ find_identity: summary });
    const open = await handler.handleContent(frame("https://shop.com/checkout"), {
      type: "cs_open_menu",
      kind: "identity",
      roles: ["fullName", "cpf", "city"],
    });
    expect(open).toMatchObject({ ok: true, rows: 1 });
    const token = (open as { token: string }).token;
    const view = await handler.handleInline(TAB, { type: "menu_state", token });
    expect(view).toMatchObject({
      ok: true,
      value: { state: "ready", kind: "identity", identity: { title: "Samuel Rocha", fills: 2, documents: ["cpf"], documentsAllowed: true, empty: false } },
    });
    expect(requests.map((r) => r.type)).toEqual(["find_identity"]);
  });

  it("fills without documents, then with them after confirmation", async () => {
    const { handler, requests, sent } = setup({
      find_identity: summary,
      fill_identity: { type: "fill_identity", values: [{ role: "fullName", value: "Samuel Rocha" }] },
    }, { fillReply: { filled: 1, pressing: null } });
    const open = (await handler.handleContent(frame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })) as { token: string };
    await handler.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: false });
    expect(requests.at(-1)).toMatchObject({ type: "fill_identity", roles: ["fullName"], documents: false });
    expect(sent.at(-1)).toMatchObject({ type: "bg_fill", fill: { kind: "identity" }, submit: false, totp: false });

    const again = (await handler.handleContent(frame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })) as { token: string };
    await handler.handleInline(TAB, { type: "menu_pick_identity", token: again.token, documents: true });
    expect(requests.at(-1)).toMatchObject({ type: "fill_identity", roles: ["fullName", "cpf"], documents: true });
  });

  it("never asks for documents on an http page", async () => {
    const { handler, requests } = setup({ find_identity: summary, fill_identity: { type: "fill_identity", values: [] } });
    const open = (await handler.handleContent(frame("http://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })) as { token: string };
    const view = await handler.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { identity: { documentsAllowed: false } } });
    await handler.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: true });
    expect(requests.at(-1)).toMatchObject({ roles: ["fullName"] });
  });

  it("shows the empty identity row when there is none, and opens it", async () => {
    const { handler, requests } = setup({ find_identity: new BridgeError("not_found", "x"), open_identity: { type: "open_identity" } });
    const open = (await handler.handleContent(frame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "city"] })) as { token: string };
    const view = await handler.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { identity: { empty: true } } });
    expect(await handler.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: false })).toMatchObject({ ok: false });
    await handler.handleInline(TAB, { type: "menu_open_identity", token: open.token });
    expect(requests.at(-1)?.type).toBe("open_identity");
  });

  it("adds the identity row under a sign-up form's logins", async () => {
    const { handler } = setup({ find_matches: { type: "find_matches", matches: [] }, find_identity: summary });
    const open = (await handler.handleContent(frame("https://shop.com/signup"), { type: "cs_open_menu", kind: "login", roles: ["email", "fullName"] })) as { ok: boolean; token: string };
    expect(open.ok).toBe(true);
    const view = await handler.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { kind: "login", items: [], identity: { fills: 1 } } });
  });

  it("stays out of the page when the identity has nothing for the form", async () => {
    const { handler } = setup({ find_identity: { ...summary, roles: ["cpf"] } });
    const open = await handler.handleContent(frame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["city", "state"] });
    expect(open).toEqual({ ok: false });
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/background/inline-handler.test.ts -t "identity menu"`
Expected: FAIL.

- [ ] **Step 3: Implement** (`inline-handler.ts`)

Imports: `import { DOCUMENT_ROLES, SSO_PROVIDERS, type IdentityRole } from "@havenkeys/protocol";` and `type IdentityRowView` from `../messaging/inline`.

`MenuSession` gains:

```ts
  /** The form's identity roles (from the content script). */
  roles: IdentityRole[];
  identity: IdentityRowView | null;
```

Add a helper:

```ts
  /**
   * The identity row for a form with `roles`, or null when there is nothing
   * to offer. Throws BridgeError("locked") through for the caller.
   */
  async function identityRow(frame: FrameRef, roles: IdentityRole[]): Promise<IdentityRowView | null> {
    let summary: ResultFor<"find_identity">;
    try {
      summary = await deps.client.request({ type: "find_identity", ...frameFields(frame) });
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked") throw e;
      if (e instanceof BridgeError && e.code === "not_found") {
        return { title: "", fills: 0, documents: [], documentsAllowed: false, empty: true };
      }
      return null;
    }
    const has = roles.filter((r) => summary.roles.includes(r));
    return {
      title: summary.title,
      fills: has.length,
      documents: has.filter((r) => DOCUMENT_ROLES.includes(r)),
      documentsAllowed: frame.url.startsWith("https:"),
      empty: summary.roles.length === 0,
    };
  }
```

Change `openMenu`'s signature to `openMenu(frame, kind, explicit, roles: IdentityRole[] = [])`, and at its top (after the suggestions check) add the identity branch:

```ts
    if (kind === "identity") {
      let row: IdentityRowView | null;
      try {
        row = await identityRow(frame, roles);
      } catch {
        return register(frame, kind, true, [], [], { hint: null, help: null }, roles, null);
      }
      if (!row || (!row.empty && row.fills === 0 && !explicit)) return { ok: false };
      return register(frame, kind, false, [], [], { hint: null, help: null }, roles, row);
    }
```

In the login path, after computing `items`/`passkeys`/`extra` and before `register`, attach the row for sign-up forms:

```ts
    let identity: IdentityRowView | null = null;
    if (kind === "login" && !locked && roles.length > 0) {
      identity = await identityRow(frame, roles).catch(() => null);
      if (identity && identity.fills === 0) identity = null;
    }
    if (!locked && kind !== "new_password" && items.length === 0 && passkeys.length === 0 && !identity && !explicit) return { ok: false };
```

(replace the existing "Nothing to offer" line with the last one), and pass `roles, identity` to `register`.

`register` gains `roles: IdentityRole[] = [], identity: IdentityRowView | null = null` parameters, stores them in the session, and counts the row:

```ts
    const offered = items.length + passkeys.length + (hint ? 1 : 0) + (identity ? 1 : 0);
    const rows = locked || kind === "new_password" || kind === "identity" ? 1 : Math.max(1, Math.min(offered, MENU_MAX_ROWS));
```

(Also update the suggestions-off early `register(...)` call: it passes no roles.)

`handleContent`'s `cs_open_menu`: `return openMenu(frame, req.kind, req.explicit === true, req.roles ?? []);`

`menu_state`: include `identity: m.identity` in the ready view.

New `handleInline` cases:

```ts
      case "menu_pick_identity": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !m.identity) return { ok: false, message: t.errors.menuExpired };
        if (m.identity.empty) return { ok: false, message: t.menu.identityEmptyBody };
        const docs = req.documents && m.identity.documentsAllowed;
        const roles = m.roles.filter((r) => docs || !DOCUMENT_ROLES.includes(r));
        if (roles.length === 0) return { ok: false, message: t.menu.identityNothing };
        closeMenu(tabId);
        try {
          const r = await deps.client.request({ type: "fill_identity", ...frameFields(m.frame), roles, documents: docs });
          const filled = await pickFill(m.frame, m.token, { kind: "identity", values: r.values }, null);
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.menu.identityNothing };
        } catch (e) {
          return fail(e);
        }
      }
      case "menu_open_identity": {
        const m = liveMenu(tabId, req.token);
        if (!m || !m.identity) return { ok: false, message: t.errors.menuExpired };
        closeMenu(tabId);
        try {
          await deps.client.request({ type: "open_identity", ...frameFields(m.frame) });
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
```

`pickFill`: an identity payload must never submit — it already passes `auto === null`, so `submit` and `totp` are false.

Strings used here (`t.menu.identityNothing`, `t.menu.identityEmptyBody`) are added in Task 11; add them now to `en.ts` and `pt-BR.ts` under `menu` so this task compiles:

```ts
    identityNothing: "Nothing to fill in this form.",
    identityEmptyBody: "Add your details in the HavenKeys app first.",
```

```ts
    identityNothing: "Nada a preencher neste formulário.",
    identityEmptyBody: "Adicione seus dados no app HavenKeys primeiro.",
```

- [ ] **Step 4: Run to see it pass**

Run: `cd apps/extension && npx vitest run src/background && npx tsc --noEmit -p .`
Expected: all background tests pass (old ones too).

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/background/inline-handler.ts apps/extension/src/background/inline-handler.test.ts apps/extension/src/i18n
git commit -m "feat(extension): identity menu sessions: counts, documents step, empty identity"
```

---

### Task 10: Content script — choosing the identity menu and filling

**Files:**
- Modify: `apps/extension/src/content/index.ts`
- Test: `apps/extension/src/content/content.test.ts`

**Interfaces:**
- Consumes: Tasks 6–8.
- Produces: `menuKindFor(field)` returns `{ kind: MenuKind; roles?: IdentityRole[] } | null`; `handleFill` handles `identity`; the listener answers `bg_identity_roles`.

Decision (spec §5.2, amended):

```text
login kind (username/password/current-password):
    group has a password field, or intent is "login"  → login (+ roles when intent is "signup" and an identity group qualifies)
    else, if the field has an identity role in a qualifying group → identity
    else → login
new-password roles → new_password
otp → otp
otherwise, an identity role in a qualifying group whose login intent is not "login" → identity
otherwise → null
```

- [ ] **Step 1: Write the failing tests** (append to `content.test.ts`; `send` calls land in `sent`; follow how existing tests trigger a menu — look for a helper that dispatches a trusted-looking open, e.g. clicking the icon via `toggleFromIcon`, or the tests' own approach. If the existing tests exercise `maybeOpen` through the field icon's click, do the same.)

Because menus only open on trusted input (and jsdom events are untrusted), test the decision function directly by exporting it for tests: in `index.ts` export `menuKindFor` (add `export` — it has no side effects). Then:

```ts
import { menuKindFor } from "./index";

describe("menuKindFor", () => {
  const set = (html: string) => (document.body.innerHTML = html);
  const f = (sel: string) => document.querySelector(sel) as HTMLInputElement;

  it("opens the identity menu on a checkout's CPF field", () => {
    set(`<form><input id="n" name="nome" aria-label="Nome completo"><input id="c" name="cpf" aria-label="CPF"><button>Finalizar compra</button></form>`);
    expect(menuKindFor(f("#c"))).toEqual({ kind: "identity", roles: ["fullName", "cpf"] });
  });

  it("keeps the login menu on a gov.br-style CPF login", () => {
    set(`<form><h1>Entrar</h1><input id="c" name="cpf" aria-label="CPF"><button type="submit">Entrar</button></form>`);
    expect(menuKindFor(f("#c"))?.kind).toBe("login");
  });

  it("adds identity roles to a sign-up form's email field", () => {
    set(`<form><h1>Create account</h1><input id="n" name="first_name" aria-label="First name"><input id="e" type="email" name="email"><input type="password" name="pw" autocomplete="new-password"><button type="submit">Sign up</button></form>`);
    const k = menuKindFor(f("#e"));
    expect(k?.kind).toBe("login");
    expect(k?.roles).toContain("firstName");
  });

  it("never offers the identity on a login form", () => {
    set(`<form><h1>Sign in</h1><input id="e" type="email" name="email"><input type="password" name="pw"><button type="submit">Sign in</button></form>`);
    expect(menuKindFor(f("#e"))).toEqual({ kind: "login" });
  });
});

describe("identity fills", () => {
  it("fills the picked form's identity fields on the matched origin", () => {
    document.body.innerHTML = `<form><input name="nome" aria-label="Nome completo"><input name="cep" aria-label="CEP"></form>`;
    const reply = deliver({
      type: "bg_fill",
      origin: location.origin,
      token: null,
      fill: { kind: "identity", values: [{ role: "fullName", value: "Samuel" }, { role: "postalCode", value: "70000-000" }] },
      submit: false,
      totp: false,
    });
    expect(reply).toEqual({ filled: 2, pressing: null });
    expect(field("nome").value).toBe("Samuel");
  });

  it("refuses an identity fill for another origin", () => {
    document.body.innerHTML = `<form><input name="nome" aria-label="Nome completo"><input name="cep" aria-label="CEP"></form>`;
    const reply = deliver({
      type: "bg_fill",
      origin: "https://evil.example",
      token: null,
      fill: { kind: "identity", values: [{ role: "fullName", value: "Samuel" }] },
      submit: false,
      totp: false,
    });
    expect(reply).toEqual({ filled: 0, pressing: null });
    expect(field("nome").value).toBe("");
  });

  it("answers the popup's role scan", () => {
    document.body.innerHTML = `<form><input name="nome" aria-label="Nome completo"><input name="cpf" aria-label="CPF"></form>`;
    expect(deliver({ type: "bg_identity_roles" })).toEqual({ roles: ["fullName", "cpf"] });
  });
});
```

(`field(n)` in that file returns an `HTMLInputElement` by name; it works for these inputs.)

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/content/content.test.ts`
Expected: FAIL (`menuKindFor` not exported / returns strings).

- [ ] **Step 3: Implement** (`content/index.ts`)

Imports: `import { findIdentityGroup, identityGroupFor, rolesOf } from "../autofill/identity";`, `import { fillIdentity } from "../autofill/identity-fill";`, `import type { IdentityRole } from "@havenkeys/protocol";`.

Replace `menuKindFor`:

```ts
/** Which menu a field gets, and the identity roles of its form when they matter. */
export function menuKindFor(field: HTMLInputElement): { kind: MenuKind; roles?: IdentityRole[] } | null {
  const env = defaultEnv();
  const { group, kind } = groupFor(field, env);
  const loginIntent = group.intent === "login";
  const identity = loginIntent ? null : identityGroupFor(field, env);
  const roles = identity ? rolesOf(identity) : undefined;
  if (isLoginRole(kind)) {
    const hasPassword = group.fields.some((f) => f.el.type === "password");
    if (hasPassword || loginIntent || !roles) {
      return group.intent === "signup" && roles ? { kind: "login", roles } : { kind: "login" };
    }
    return { kind: "identity", roles };
  }
  if (isNewPasswordRole(kind)) return { kind: "new_password" };
  if (kind === "otp") return { kind: "otp" };
  return roles ? { kind: "identity", roles } : null;
}
```

In `maybeOpen`, adapt to the object:

```ts
    const choice = menuKindFor(field);
    if (!choice) return;
    const kind = choice.kind;
    if (!suggestions && (explicit || kind !== "login")) return;
    opening = true;
    const req: ContentRequest = {
      type: "cs_open_menu",
      kind,
      ...(explicit ? { explicit: true as const } : {}),
      ...(choice.roles ? { roles: choice.roles } : {}),
    };
```

`showIcon` uses `!menuKindFor(field)` — unchanged semantics.

In `handleFill`, compute the identity group for identity payloads before the login group (identity must not use `groupFor`'s login group):

```ts
    if (m.fill.kind === "identity") {
      const target = m.token !== null ? (picked && picked.token === m.token && picked.until > Date.now() ? picked : null) : null;
      if (m.token !== null) picked = null;
      if (m.token !== null && (!target || !target.field.isConnected)) return none;
      const group = target ? identityGroupFor(target.field, env) : findIdentityGroup(document, env);
      return { filled: group ? fillIdentity(group, m.fill.values, env) : 0, pressing: null };
    }
```

placed right after `const env = defaultEnv();` (the origin check stays first). Remove the Task 8 placeholder `case "identity"`, or keep it returning `none` as an unreachable fallback so the `switch` stays exhaustive.

In the `chrome.runtime.onMessage` listener, where parsed background messages are dispatched (find `parseBackgroundMessage(` in `index.ts`), add:

```ts
      case "bg_identity_roles": {
        if (window.top !== window) return reply({ roles: [] });
        const g = findIdentityGroup(document, defaultEnv());
        reply({ roles: g ? rolesOf(g) : [] });
        return;
      }
```

following the listener's existing reply convention (synchronous `reply(...)` then return; check how `bg_fill` replies and do the same).

- [ ] **Step 4: Run to see it pass**

Run: `cd apps/extension && npx vitest run src/content src/autofill && npx tsc --noEmit -p .`
Expected: all pass (existing content tests included).

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/content
git commit -m "feat(extension): identity menu choice and fills in the content script"
```

---

### Task 11: Menu page — the identity row and the document step

**Files:**
- Modify: `apps/extension/src/menu/menu.ts`, `apps/extension/src/menu/inline.css` (only if a new class needs styling), `apps/extension/src/i18n/en.ts`, `apps/extension/src/i18n/pt-BR.ts`
- Test: `apps/extension/src/menu/menu.test.ts`

**Interfaces:**
- Consumes: `IdentityRowView`, `menu_pick_identity`, `menu_open_identity` (Task 8).

Behaviour:
- `kind === "identity"`: one row: ID-card icon, title = `view.identity.title || msg.menu.identityFallback`, detail = `msg.menu.identityFills(fills)`.
- `kind === "login"` with `view.identity`: the same row after the login rows.
- Empty identity: row title `msg.menu.identityEmptyTitle`, detail `msg.menu.identityEmptyBody`; picking sends `menu_open_identity`.
- Picking the row when `documents.length > 0 && documentsAllowed`: replace the list with a step: text `msg.menu.identityAlsoAsks(site, labels)`, a primary button `msg.menu.identityFillWithDocs(labels)` (sends `documents: true`), and a secondary button `msg.menu.identityFillWithoutDocs` (sends `documents: false`). Both are real `<button>`s under the same click guard.
- When `documents.length > 0 && !documentsAllowed`: pick fills directly with `documents: false`, and the row's detail says `msg.menu.identityNoDocsHttp`.
- Labels: `msg.menu.documentLabels[role]` joined with `msg.menu.and`.

- [ ] **Step 1: Write the failing tests** (append to `menu.test.ts`, using its `replies`/`asked`/`load()` pattern and `captureClicks` for trusted, armed clicks as the existing tests do)

```ts
describe("identity row", () => {
  const ready = (identity: object, kind = "identity") => ({
    ok: true,
    value: { state: "ready", kind, site: "shop.com", items: [], passkeys: [], hint: null, identity },
  });

  it("asks before filling documents, naming them", async () => {
    replies = [ready({ title: "Samuel Rocha", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false }), { ok: true, value: null }];
    await load();
    const row = document.querySelector<HTMLButtonElement>("button.row")!;
    expect(row.textContent).toContain("Samuel Rocha");
    await captureClicks(row); // the file's helper for an armed, trusted click
    expect(document.body.textContent).toContain("CPF");
    const [withDocs, withoutDocs] = Array.from(document.querySelectorAll<HTMLButtonElement>("#main button"));
    await captureClicks(withoutDocs!);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: false });
    expect(withDocs).toBeDefined();
  });

  it("fills straight away when the form asks for no documents", async () => {
    replies = [ready({ title: "Samuel", fills: 2, documents: [], documentsAllowed: true, empty: false }), { ok: true, value: null }];
    await load();
    await captureClicks(document.querySelector<HTMLButtonElement>("button.row")!);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: false });
  });

  it("opens the desktop app for an empty identity", async () => {
    replies = [ready({ title: "", fills: 0, documents: [], documentsAllowed: true, empty: true }), { ok: true, value: null }];
    await load();
    await captureClicks(document.querySelector<HTMLButtonElement>("button.row")!);
    expect(asked.at(-1)).toEqual({ type: "menu_open_identity", token: TOKEN });
  });

  it("shows the row under a sign-up form's logins", async () => {
    replies = [ready({ title: "Samuel", fills: 1, documents: [], documentsAllowed: true, empty: false }, "login")];
    await load();
    expect(document.querySelectorAll("button.row")).toHaveLength(1);
  });
});
```

Read `click-capture.test-helper.ts` and the existing "help row" test before writing these: use exactly the helper and arming sequence those tests use (they advance fake timers past `ARM_DELAY_MS`). Adjust the helper call, not the assertions.

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/menu/menu.test.ts -t "identity row"`
Expected: FAIL.

- [ ] **Step 3: Strings** — add under `menu` in `en.ts`:

```ts
    identityFallback: "Your identity",
    identityFills: (n: number) => (n === 1 ? "Fills 1 field" : `Fills ${n} fields`),
    identityEmptyTitle: "Your identity is empty",
    identityAlsoAsks: (site: string, docs: string) => `${site} also asks for: ${docs}`,
    identityFillWithDocs: (docs: string) => `Fill ${docs} too`,
    identityFillWithoutDocs: "Fill without documents",
    identityNoDocsHttp: "Documents are not filled on http pages",
    and: " and ",
    documentLabels: { cpf: "CPF", rg: "RG", passport: "passport", driversLicense: "driver’s license" },
```

and `pt-BR.ts`:

```ts
    identityFallback: "Sua identidade",
    identityFills: (n: number) => (n === 1 ? "Preenche 1 campo" : `Preenche ${n} campos`),
    identityEmptyTitle: "Sua identidade está vazia",
    identityAlsoAsks: (site: string, docs: string) => `${site} também pede: ${docs}`,
    identityFillWithDocs: (docs: string) => `Preencher ${docs} também`,
    identityFillWithoutDocs: "Preencher sem documentos",
    identityNoDocsHttp: "Documentos não são preenchidos em páginas http",
    and: " e ",
    documentLabels: { cpf: "CPF", rg: "RG", passport: "passaporte", driversLicense: "CNH" },
```

(`identityNothing` and `identityEmptyBody` were added in Task 9.)

- [ ] **Step 4: Implement** (`menu.ts`)

Add an ID-card glyph next to `sparkle()`:

```ts
/** The identity row's glyph: an ID card in the 1.6-stroke icon set. */
function idCard(): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "16");
  svg.setAttribute("height", "16");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", "M4.5 6h15a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM9 12a1.8 1.8 0 1 0 0-3.6A1.8 1.8 0 0 0 9 12zM6 15.5c.5-1.3 1.6-2 3-2s2.5.7 3 2M14 10h3.5M14 13.5h3.5");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.6");
  path.setAttribute("stroke-linecap", "round");
  svg.append(path);
  return svg;
}
```

Add the row and the step:

```ts
function documentList(roles: readonly IdentityRole[]): string {
  return roles.map((r) => msg.menu.documentLabels[r as keyof typeof msg.menu.documentLabels] ?? r).join(msg.menu.and);
}

/** A plain action button for the documents step, under the click guard. */
function action(text: string, primary: boolean, onPick: () => Promise<void>): HTMLButtonElement {
  const b = h("button", { className: primary ? "step-btn primary" : "step-btn", text });
  b.type = "button";
  b.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed() || b.disabled) return;
    b.disabled = true;
    void onPick().finally(() => (b.disabled = false));
  });
  return b;
}

function documentsStep(t: string, site: string, docs: readonly IdentityRole[]): void {
  const list = documentList(docs);
  main.replaceChildren(
    h(
      "div",
      { className: "message step" },
      h("strong", { text: msg.menu.identityAlsoAsks(site, list) }),
      action(msg.menu.identityFillWithDocs(list), true, () => pick({ type: "menu_pick_identity", token: t, documents: true })),
      action(msg.menu.identityFillWithoutDocs, false, () => pick({ type: "menu_pick_identity", token: t, documents: false })),
    ),
  );
}

function identityRow(t: string, site: string, v: IdentityRowView): HTMLButtonElement {
  if (v.empty) {
    return row(idCard(), msg.menu.identityEmptyTitle, msg.menu.identityEmptyBody, () => pick({ type: "menu_open_identity", token: t }), {
      title: true,
      detail: true,
    });
  }
  const askFirst = v.documents.length > 0 && v.documentsAllowed;
  const detail = v.documents.length > 0 && !v.documentsAllowed ? msg.menu.identityNoDocsHttp : msg.menu.identityFills(v.fills);
  return row(
    idCard(),
    v.title || msg.menu.identityFallback,
    detail,
    async () => {
      if (askFirst) documentsStep(t, site, v.documents);
      else await pick({ type: "menu_pick_identity", token: t, documents: false });
    },
    { title: v.title === "", detail: true },
  );
}
```

In `render`, after the `new_password` branch:

```ts
  if (view.kind === "identity") {
    main.replaceChildren(view.identity ? identityRow(t, view.site, view.identity) : message(msg.menu.unavailable, msg.menu.identityNothing));
    return;
  }
```

and for login menus append the row after items (before `tail`):

```ts
  const identity = view.identity ? [identityRow(t, view.site, view.identity)] : [];
  const rows = [...lead, ...passkeyRows, ...view.items.map((i) => itemRow(t, i, kind)), ...identity, ...tail];
```

`kind` is narrowed to `"login" | "otp"` for `itemRow`; after the two early returns (`new_password`, `identity`) TypeScript narrows it. Import `type IdentityRowView` and `type IdentityRole`.

Arrow-key navigation selects `button.row`; the step's buttons use `step-btn`, so extend the keydown selector to `"button.row, button.step-btn"`.

CSS (`menu/inline.css`, follow existing tokens):

```css
.step { gap: 8px; }
.step-btn {
  font: inherit;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}
.step-btn.primary { background: var(--accent); color: var(--accent-ink); border-color: transparent; }
```

Check `inline.css`'s variable names first and use the ones it defines (the names above are placeholders only if they don't exist — pick the file's accent/border tokens).

- [ ] **Step 5: Run to see it pass**

Run: `cd apps/extension && npx vitest run src/menu src/i18n && npx tsc --noEmit -p .`
Expected: all pass (the i18n tests check both languages have the same keys).

- [ ] **Step 6: Commit**

```bash
git add apps/extension/src/menu apps/extension/src/i18n
git commit -m "feat(extension): identity row and the documents step in the field menu"
```

---

### Task 12: Popup — "Fill identity"

**Files:**
- Modify: `apps/extension/src/messaging/popup.ts`, `apps/extension/src/background/popup-handler.ts`, `apps/extension/src/background/index.ts`, `apps/extension/src/popup/popup.ts`, `apps/extension/src/i18n/en.ts`, `pt-BR.ts`
- Test: `apps/extension/src/background/popup-handler.test.ts`

**Interfaces:**
- `PopupState` `unlocked` gains `identity: { title: string } | null` (null when `find_identity` fails or the identity has no roles).
- `PopupRequest` gains `{ type: "popup_fill_identity"; documents: boolean | null }` (`null`: not asked yet).
- Reply value for `popup_fill_identity`: `null` (filled; popup closes) or `{ confirm: IdentityRole[] }` (ask about these documents, then resend with `documents: true|false`).
- `createPopupHandler(client, activeTab, fillTab, startSso?, scanIdentity?)` where `scanIdentity: (tabId: number, pageUrl: string) => Promise<IdentityRole[]>` injects the content script into frame 0 and sends `bg_identity_roles`.

- [ ] **Step 1: Write the failing tests** (append to `popup-handler.test.ts`, using its fake client pattern)

```ts
describe("popup identity fill", () => {
  it("offers the identity when it has values", async () => {
    const h = handlerWith({ status: unlocked, find_matches: { type: "find_matches", matches: [] }, find_identity: { type: "find_identity", title: "Samuel", email: null, roles: ["fullName"] } });
    const r = await h.handle({ type: "popup_state" });
    expect(r).toMatchObject({ ok: true, value: { kind: "unlocked", identity: { title: "Samuel" } } });
  });

  it("asks about documents first, then fills with the answer", async () => {
    const { h, requests, filled } = identitySetup(["fullName", "cpf"], { roles: ["fullName", "cpf"] });
    expect(await h.handle({ type: "popup_fill_identity", documents: null })).toEqual({ ok: true, value: { confirm: ["cpf"] } });
    expect(requests.some((x) => x.type === "fill_identity")).toBe(false);
    expect(await h.handle({ type: "popup_fill_identity", documents: false })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toMatchObject({ type: "fill_identity", roles: ["fullName"], documents: false });
    expect(filled.at(-1)).toMatchObject({ kind: "identity" });
  });

  it("says so when the page has no identity form", async () => {
    const { h } = identitySetup([], { roles: ["fullName"] });
    expect(await h.handle({ type: "popup_fill_identity", documents: null })).toMatchObject({ ok: false });
  });
});
```

Define `handlerWith` / `identitySetup` in the test file following the file's existing fake-client setup: `identitySetup(pageRoles, summary)` builds a client answering `find_identity` with `summary` and `fill_identity` with `{ type: "fill_identity", values: [{ role: "fullName", value: "Samuel" }] }`, an active tab `https://shop.com/`, a `fillTab` that records payloads and returns 1, and a `scanIdentity` that returns `pageRoles`.

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/background/popup-handler.test.ts -t "popup identity"`
Expected: FAIL.

- [ ] **Step 3: Implement**

`messaging/popup.ts`: add the request and `identity` to the unlocked state; parser case:

```ts
    case "popup_fill_identity":
      return keys.length === 2 && (o.documents === null || typeof o.documents === "boolean")
        ? { type: "popup_fill_identity", documents: o.documents }
        : null;
```

`popup-handler.ts`:

```ts
export type IdentityScanner = (tabId: number, pageUrl: string) => Promise<IdentityRole[]>;
```

`state()`: after `find_matches`, add

```ts
      let identity: { title: string } | null = null;
      try {
        const s = await client.request({ type: "find_identity", url });
        if (s.roles.length > 0) identity = { title: s.title };
      } catch {
        identity = null;
      }
      return { kind: "unlocked", site: displayHost(url), matches: found.matches, identity };
```

(and `identity: null` in the no-URL branch).

New handler case:

```ts
      case "popup_fill_identity": {
        const tab = await activeTab();
        const url = pageUrlForRequest(tab?.url);
        if (!tab || !url || !scanIdentity) return { ok: false, message: t.errors.pageNotSupported };
        try {
          const pageRoles = await scanIdentity(tab.id, url);
          if (pageRoles.length === 0) return { ok: false, message: t.errors.noIdentityForm };
          const summary = await client.request({ type: "find_identity", url });
          const has = pageRoles.filter((r) => summary.roles.includes(r));
          const docs = url.startsWith("https:") ? has.filter((r) => DOCUMENT_ROLES.includes(r)) : [];
          if (req.documents === null && docs.length > 0) return { ok: true, value: { confirm: docs } };
          const withDocs = req.documents === true && docs.length > 0;
          const roles = has.filter((r) => withDocs || !DOCUMENT_ROLES.includes(r));
          if (roles.length === 0) return { ok: false, message: t.menu.identityNothing };
          const r = await client.request({ type: "fill_identity", url, roles, documents: withDocs });
          const filled = await fillTab(tab.id, url, { kind: "identity", values: r.values });
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.menu.identityNothing };
        } catch (e) {
          return fail(e);
        }
      }
```

Update the reply type of `handle` to include `{ confirm: IdentityRole[] }`.

`background/index.ts`: implement the scanner and pass it:

```ts
async function scanIdentity(tabId: number, pageUrl: string): Promise<IdentityRole[]> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
    const reply = await chrome.tabs.sendMessage(tabId, { type: "bg_identity_roles" }, { frameId: 0 });
    return parseIdentityRolesReply(reply);
  } catch {
    return [];
  }
}
const popup = createPopupHandler(client, activeTab, fillTab, ssoTab, scanIdentity);
```

(`pageUrl` is unused by the scan itself; keep the parameter for symmetry with `fillTab`, or drop it from both the type and the call — pick one and keep the type consistent.)

`popup/popup.ts`: when `state.identity` is non-null, append after the list/notices:

```ts
function identityRow(title: string): HTMLElement {
  const status = h("div", { className: "row-status" });
  const fill = smallButton(t.popup.fillIdentity, t.popup.fillIdentityTitle);
  const row = h(
    "div",
    { className: "item identity" },
    h("div", { className: "who" }, truncates(h("div", { className: "title", text: title || t.menu.identityFallback })), h("div", { className: "user copy", text: t.popup.identityKind })),
    h("div", { className: "actions" }, fill),
    status,
  );
  async function run(documents: boolean | null): Promise<void> {
    fill.disabled = true;
    const r = await send<{ confirm: IdentityRole[] } | null>({ type: "popup_fill_identity", documents });
    fill.disabled = false;
    if (!r.ok) return void status.replaceChildren(h("span", { className: "error", text: r.message }));
    if (r.value === null) return window.close();
    const list = r.value.confirm.map((x) => t.menu.documentLabels[x as keyof typeof t.menu.documentLabels] ?? x).join(t.menu.and);
    const yes = smallButton(t.menu.identityFillWithDocs(list), "");
    const no = smallButton(t.menu.identityFillWithoutDocs, "");
    yes.addEventListener("click", () => void run(true));
    no.addEventListener("click", () => void run(false));
    status.replaceChildren(h("p", { text: t.popup.identityAsks(list) }), yes, no);
  }
  fill.addEventListener("click", () => void run(null));
  return row;
}
```

and in `render`'s `unlocked` case, `if (state.identity) parts.push(identityRow(state.identity.title));` before `main.replaceChildren(...parts)`.

Strings: `popup.fillIdentity` "Fill identity" / "Preencher identidade"; `popup.fillIdentityTitle` "Fill your identity into this page's form" / "Preencher sua identidade no formulário desta página"; `popup.identityKind` "Identity" / "Identidade"; `popup.identityAsks(docs)` "This page also asks for {docs}." / "Esta página também pede {docs}."; `errors.noIdentityForm` "No form for your identity on this page." / "Nenhum formulário para sua identidade nesta página."

- [ ] **Step 4: Run to see it pass**

Run: `cd apps/extension && npx vitest run && npx tsc --noEmit -p .`
Expected: the whole extension suite passes.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src
git commit -m "feat(extension): Fill identity from the toolbar popup, with the documents question"
```

---

### Task 13: Manifest, docs, UI check, full verification

**Files:**
- Modify: `apps/extension/manifest/firefox.json`, `docs/autofill.md`, `docs/native-messaging.md`, `docs/security-model.md`, `CLAUDE.md`, `README.md`, `tools/ui-check/extension.mjs`, `docs/superpowers/specs/2026-09-29-identity-autofill-design.md` (Status → accepted)

- [ ] **Step 1: Firefox data collection** — the extension now handles personal data. In `manifest/firefox.json` change `"required": ["authenticationInfo"]` to `"required": ["authenticationInfo", "personallyIdentifyingInfo"]`. Run `cd apps/extension && npx vitest run src/i18n/manifest.test.ts` and update that test if it pins the list.

- [ ] **Step 2: Docs**
  - `docs/native-messaging.md`: add the three requests to the request table (fields, result, rate class, what Rust checks — copy from spec §6).
  - `docs/autofill.md`: a new "Identity" section: signals (autocomplete table, words, types), qualifying forms, the menu decision (Task 10's table), writing rules (empty-only, selects with the UF table, date, phone retry, textarea gets address line 1).
  - `docs/security-model.md`: the not-site-bound decision and its seven rules (spec §2), the three requests in the IPC section, and the Firefox data-collection declaration.
  - `CLAUDE.md` §25: append an amendment note in the same style as the others:

```markdown
> Amended on 2026-09-29 by
> `docs/superpowers/specs/2026-09-29-identity-autofill-design.md`: after the
> user picks their Identity in the menu or popup, HavenKeys fills that form's
> identity fields on any site (not bound to a saved website). Document
> numbers need a second click naming them and only fill on https pages; a
> cross-site frame gets nothing; nothing is overwritten or submitted.
```

  - `README.md`: in "What the browser extension does", one bullet: filling sign-up and checkout forms from the Identity, documents only after confirming.
  - Spec status line → `Status: accepted, 2026-09-29.`

- [ ] **Step 3: UI check scenarios** — in `tools/ui-check/extension.mjs`, follow the existing menu scenarios (search for `"menu"`), and add: identity menu (`kind: "identity"`, a row with 5 fields), the documents step (click the row), the empty identity row, a sign-up login menu with the identity row, and the popup with the Fill identity button and its documents question. Stubbed `menu_state` replies need `identity`, and existing ready replies need `identity: null`.

Run: `pnpm ui:check --app=extension`
Expected: `no clipped text, no horizontal overflow.` Look at the identity screenshots in `ui-check-output/extension/…` in both languages and themes.

- [ ] **Step 4: Full verification**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
scripts/test-server.sh            # server + sync-client against Postgres
cargo test --workspace --exclude havenkeys-server --exclude havenkeys-sync-client
(cd packages/protocol && npx vitest run && npx tsc --noEmit -p .)
(cd apps/extension && npx vitest run && npx tsc --noEmit -p .)
(cd apps/desktop && npx vitest run && npx tsc --noEmit -p .)
cargo audit
pnpm --filter extension build     # or the repo's extension build script; check package.json
```

Expected: everything passes; `cargo audit` shows only the two pre-existing allowed warnings.

- [ ] **Step 5: Secret-leak review** — `grep -rn "console\.\(log\|debug\|info\)" apps/extension/src/autofill/identity*.ts apps/extension/src/content/index.ts` returns nothing new; no identity value is written to an attribute (the fill tests assert `innerHTML` has no values); `IdentityValue`, `IdentitySummary` and `ResultBody` `Debug`s show no values (tests in Tasks 2–3).

- [ ] **Step 6: Commit**

```bash
git add -A apps/extension/manifest docs CLAUDE.md README.md tools/ui-check
git commit -m "docs: identity autofill: native messaging, autofill rules, security model, CLAUDE.md note"
```

Then report to the user and ask whether to merge `identity-autofill` into `main` and push (previous features were merged with `git merge --no-ff` and pushed on request).
