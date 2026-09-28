# Sign in with Google / Microsoft / GitHub / Apple — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Logins can record "Sign in with <provider> · <account>". The extension offers them in a top-right balloon when the site shows that provider's button. After the user picks one, it presses the button and clicks the saved account on the provider's chooser. New ones are detected and saved after confirmation, and a 1Password re-import fills them in.

**Architecture:** Rust owns the closed provider list, the provider origins a run may continue into, and every item-to-page authorization (`start_sso_for_page`, `check_sso`, `stage_save_sso`). The native protocol gains `start_sso`, `check_sso` and `save_sso`, and `Match.provider`. The extension adds:
* a pure recogniser (`autofill/sso.ts`);
* a pure state module (`background/sso-state.ts`);
* a background handler (`background/sso-handler.ts`);
* a content-script module (`content/sso.ts`);
* one new extension frame page (`sso.html`) that shows the offer, the save prompt and a notice.

The desktop gets an editor section, a detail block, and a list subtitle.

**Tech Stack:** Rust (serde, uuid, url, psl), TypeScript (esbuild, vitest, jsdom), React (desktop), WebExtension MV3.

**Spec:** `docs/superpowers/specs/2026-09-28-sign-in-with-design.md`

## Global Constraints

- Providers are exactly `google`, `microsoft`, `github`, `apple` (lowercase on the wire). Display names are `Google`, `Microsoft`, `GitHub`, `Apple`.
- Provider origins (exact, https):

  | Provider | Origins |
  |---|---|
  | google | `https://accounts.google.com` |
  | microsoft | `https://login.microsoftonline.com`, `https://login.live.com` |
  | github | `https://github.com` |
  | apple | `https://appleid.apple.com` |

- Account: trimmed, empty → none, at most 254 characters, no control characters.
- Timers:

  | What | Duration |
  |---|---|
  | `PendingSso` | 5 minutes (`PENDING_TTL_MS = 300_000`) |
  | `SsoRun` | 2 minutes (`SSO_RUN_TTL_MS = 120_000`) |
  | Chooser wait on the provider page | 10 s (`CHOOSE_WAIT_MS = 10_000`) |

- Never press a consent/permissions screen. Never fill a password or TOTP on this path.
- `sign_in_with` is only valid on logins. Old vaults parse unchanged, with no migration (serde defaults).
- No `innerHTML` or similar HTML parsing of strings. Everything is inserted with `textContent` or `createElementNS`. `src/hygiene.test.ts` enforces this.
- Nothing new is logged. Account text is never put in errors.
- Every user-visible string goes in `en` **and** `pt-BR` (extension `apps/extension/src/i18n/{en,pt-BR}.ts`, desktop `apps/desktop/src/i18n/{en,pt-BR}.ts`). Both are typed, so a missing key fails typecheck.
- Commits: conventional style (`feat(core): …`). **No `Co-Authored-By` trailer** (user preference).
- Verification commands:
  - `cargo test -p <crate>`
  - `pnpm --filter <pkg> test`
  - `pnpm --filter <pkg> typecheck`
  - `pnpm lint:rust` before the last task's commit

## Review Focus

1. **A password update from the browser silently clears `sign_in_with`.** `stage_save_update` builds a full `ItemInput`. It must copy `existing.sign_in_with` (Task 1 test `password_update_keeps_sign_in_with`).
2. **A hostile page fakes a provider button whose `href` goes to `evil.com`.** Pressing it only navigates that page. The run must still refuse to act on `evil.com`, because `chooseFor` checks the frame's real origin against Rust's list (Task 11 test `choose_only_on_provider_origins`).
3. **The provider page shows the account twice**, for example Google's chooser plus "signed in as" text. This must not click. `chooserRow` requires exactly one clickable match (Task 9 test `chooser_row_requires_a_unique_clickable_match`).
4. **A site's own `/auth/google` interstitial on the site origin reloads the tab right after the click.** This must not raise the save prompt before the provider was reached (Task 11 test `return_before_provider_is_ignored`).
5. **A page with thousands of links and buttons.** The scan stays bounded to `MAX_SSO_CANDIDATES` (Task 9 test `scan_is_bounded`).

---

## File map

**Rust**
- Create `crates/havenkeys-core/src/sso.rs`: `SsoProvider`, `SignInWith`, `clean_sign_in_with`.
- Modify `crates/havenkeys-core/src/lib.rs`: `pub mod sso;`
- Modify `crates/havenkeys-core/src/model.rs`: overview and input fields, `check_shape`.
- Modify `crates/havenkeys-core/src/vault.rs`:
  - `build_item`, `stage_save_update`, `search`, `Suggestion`;
  - new `start_sso_for_page`, `check_sso`, `stage_save_sso`;
  - import upgrade.
- Modify `crates/havenkeys-core/src/import/{mod.rs,onepux.rs}`.
- Add `sign_in_with: None` to every `ItemInput { … }` literal:
  - `crates/havenkeys-core/src/{vault.rs,passkey/vault.rs,sync.rs,import/onepux.rs}`
  - `crates/havenkeys-core/tests/{common/mod.rs,security.rs}`
  - `crates/havenkeys-bridge/tests/bridge.rs`
  - `crates/havenkeys-sync-client/tests/round_trip.rs`
  - `apps/desktop/src-tauri/src/item_input.rs`
- Modify `crates/havenkeys-protocol/src/{lib.rs,message.rs}`, `crates/havenkeys-protocol/tests/messages.rs`.
- Modify `crates/havenkeys-bridge/src/{dispatch.rs,server.rs}`, `crates/havenkeys-bridge/tests/bridge.rs`.
- Modify `apps/desktop/src-tauri/src/import.rs`: imported count excludes upgrades.

**TypeScript shared**
- Create `packages/protocol/src/sso.ts` and `packages/protocol/src/sso-parity.test.ts`.
- Modify `packages/protocol/src/index.ts`, `index.test.ts`, `fuzz.test.ts`, `tsconfig.json`.
- Create `packages/ui/src/provider-icons.ts`. Modify `packages/ui/package.json` (export).

**Desktop**
- Create `apps/desktop/src/lib/sso.ts`, `apps/desktop/src/lib/sso.test.ts`, `apps/desktop/src/components/ProviderIcon.tsx`.
- Modify:
  - `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/openItem.ts`
  - `apps/desktop/src/views/{ItemEditor,ItemDetail,ItemList,VaultScreen,ImportSection}.tsx`
  - `apps/desktop/src/i18n/{en,pt-BR}.ts`

**Extension**
- Create:
  - `apps/extension/src/autofill/sso.ts` and `sso.test.ts`
  - `apps/extension/src/messaging/sso.ts` and `sso.test.ts`
  - `apps/extension/src/background/sso-state.ts` and `sso-state.test.ts`
  - `apps/extension/src/background/sso-handler.ts` and `sso-handler.test.ts`
  - `apps/extension/src/content/sso.ts` and `sso.test.ts`
  - `apps/extension/src/menu/sso.html`, `sso.ts` and `sso.test.ts`
  - `apps/extension/src/menu/icons.ts`
- Modify:
  - `apps/extension/src/background/{index.ts,inline-handler.ts,popup-handler.ts}` and their tests
  - `apps/extension/src/content/{index.ts,frames.ts}`
  - `apps/extension/src/messaging/inline.ts` (`MenuItemView.provider`)
  - `apps/extension/src/menu/{menu.ts,inline.css}`, `apps/extension/src/popup/popup.ts`
  - `apps/extension/src/i18n/{en,pt-BR}.ts`
  - `apps/extension/build.mjs`, `apps/extension/manifest/base.json`, `apps/extension/package.json`
  - `tools/ui-check/extension.mjs`

**Docs**
- `docs/autofill.md`, `docs/native-messaging.md`, `docs/security-model.md`, `docs/threat-model.md`, `docs/security-review.md`, `CLAUDE.md` (§25 amendment note).

---

### Task 1: Core model — providers and the `sign_in_with` field

**Files:**
- Create: `crates/havenkeys-core/src/sso.rs`
- Modify: `crates/havenkeys-core/src/lib.rs`, `crates/havenkeys-core/src/model.rs`, `crates/havenkeys-core/src/vault.rs` (`build_item` ~1573, `stage_save_login` ~1279, `stage_save_update` ~1310, `search` ~983)
- Modify: all `ItemInput { … }` literals listed in the file map (add `sign_in_with: None,`)
- Test: `crates/havenkeys-core/src/sso.rs` (unit), `crates/havenkeys-core/tests/security.rs`

**Interfaces:**
- Produces:
  - `havenkeys_core::sso::{SsoProvider, SignInWith, MAX_ACCOUNT_CHARS}`
  - `SsoProvider::{ALL, name(self) -> &'static str, origins(self) -> &'static [&'static str], from_name(&str) -> Option<Self>, allows_origin(self, &str) -> bool}`
  - `ItemOverview.sign_in_with: Option<SignInWith>`, `ItemInput.sign_in_with: Option<SignInWith>`
  - JSON: `"signInWith": {"provider": "google", "account": "a@b.c" | null}`. Omitted from the overview when none.

- [ ] **Step 1: Write the failing unit tests in the new module**

Create `crates/havenkeys-core/src/sso.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_wire_names_and_origins() {
        assert_eq!(serde_json::to_string(&SsoProvider::Github).unwrap(), "\"github\"");
        assert_eq!(SsoProvider::Github.name(), "GitHub");
        assert!(SsoProvider::Microsoft.allows_origin("https://login.live.com"));
        assert!(!SsoProvider::Google.allows_origin("https://accounts.google.com.evil.com"));
        assert!(!SsoProvider::Google.allows_origin("http://accounts.google.com"));
        assert_eq!(SsoProvider::from_name(" GitHub "), Some(SsoProvider::Github));
        assert_eq!(SsoProvider::from_name("Okta"), None);
    }

    #[test]
    fn account_rules() {
        let with = |a: &str| SignInWith { provider: SsoProvider::Google, account: Some(a.into()) };
        assert_eq!(clean_sign_in_with(Some(with("  me@x.com "))).unwrap().unwrap().account.as_deref(), Some("me@x.com"));
        assert_eq!(clean_sign_in_with(Some(with("   "))).unwrap().unwrap().account, None);
        assert!(clean_sign_in_with(Some(with("a\u{0007}b"))).is_err());
        assert!(clean_sign_in_with(Some(with(&"x".repeat(MAX_ACCOUNT_CHARS + 1)))).is_err());
        assert!(clean_sign_in_with(None).unwrap().is_none());
    }

    #[test]
    fn unknown_fields_and_providers_are_rejected() {
        assert!(serde_json::from_str::<SignInWith>(r#"{"provider":"okta"}"#).is_err());
        assert!(serde_json::from_str::<SignInWith>(r#"{"provider":"google","x":1}"#).is_err());
        let s: SignInWith = serde_json::from_str(r#"{"provider":"apple"}"#).unwrap();
        assert_eq!(s.account, None);
    }

    #[test]
    fn debug_hides_the_account() {
        let s = SignInWith { provider: SsoProvider::Google, account: Some("me@x.com".into()) };
        assert!(!format!("{s:?}").contains("me@x.com"));
    }
}
```

Add `pub mod sso;` to `crates/havenkeys-core/src/lib.rs` (alphabetical, after `pub mod secret;`).

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test -p havenkeys-core --lib sso`
Expected: FAIL. Compile errors, because `SsoProvider`, `SignInWith` and `clean_sign_in_with` do not exist.

- [ ] **Step 3: Implement the module (above the test module)**

```rust
//! "Sign in with …" providers. A closed list: each provider's display name
//! and the exact origins a sign-in run may continue into
//! (docs/superpowers/specs/2026-09-28-sign-in-with-design.md §3.1). The
//! extension holds a mirror for save detection only; parity is tested in
//! packages/protocol/src/sso-parity.test.ts.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

/// Longest account (an email address) accepted, in characters.
pub const MAX_ACCOUNT_CHARS: usize = 254;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SsoProvider {
    Google,
    Microsoft,
    Github,
    Apple,
}

impl SsoProvider {
    pub const ALL: [SsoProvider; 4] = [Self::Google, Self::Microsoft, Self::Github, Self::Apple];

    pub fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Microsoft => "Microsoft",
            Self::Github => "GitHub",
            Self::Apple => "Apple",
        }
    }

    /// Exact origins (scheme + host, no port, no trailing slash).
    pub fn origins(self) -> &'static [&'static str] {
        match self {
            Self::Google => &["https://accounts.google.com"],
            Self::Microsoft => &["https://login.microsoftonline.com", "https://login.live.com"],
            Self::Github => &["https://github.com"],
            Self::Apple => &["https://appleid.apple.com"],
        }
    }

    /// Exact string comparison: the caller passes a serialized origin.
    pub fn allows_origin(self, origin: &str) -> bool {
        self.origins().contains(&origin)
    }

    /// A provider named in an export (`"Google"`, `"github"`), case-insensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        Self::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(n))
    }
}

/// How a login signs in when it has no password of its own (or also has one).
/// Stored in the encrypted overview, like the username.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignInWith {
    pub provider: SsoProvider,
    #[serde(default)]
    pub account: Option<String>,
}

impl fmt::Debug for SignInWith {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignInWith")
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl Drop for SignInWith {
    fn drop(&mut self) {
        self.account.zeroize();
    }
}

pub(crate) fn clean_sign_in_with(value: Option<SignInWith>) -> Result<Option<SignInWith>> {
    let Some(v) = value else { return Ok(None) };
    let account = match v.account.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        None => None,
        Some(a) if a.chars().count() > MAX_ACCOUNT_CHARS || a.chars().any(char::is_control) => {
            return Err(Error::InvalidInput("account is too long or contains control characters"))
        }
        Some(a) => Some(a.to_owned()),
    };
    Ok(Some(SignInWith { provider: v.provider, account }))
}
```

- [ ] **Step 4: Run the module tests**

Run: `cargo test -p havenkeys-core --lib sso`
Expected: PASS (4 tests).

- [ ] **Step 5: Write the failing model and vault tests**

Append to the `tests` module in `crates/havenkeys-core/src/model.rs`:

```rust
    #[test]
    fn overviews_without_sign_in_with_parse_and_omit_it() {
        let json = r#"{"id":"7c9e6679-7425-40de-944b-e07fc1f90ae7","itemType":"login","title":"t",
            "hasPassword":false,"hasTotp":false,"hasNotes":false,"createdAt":1,"updatedAt":1}"#;
        let o: ItemOverview = serde_json::from_str(json).unwrap();
        assert!(o.sign_in_with.is_none());
        assert!(!serde_json::to_string(&o).unwrap().contains("signInWith"));
    }

    #[test]
    fn secure_notes_cannot_sign_in_with() {
        let input: ItemInput = serde_json::from_str(
            r#"{"itemType":"secure_note","title":"n","signInWith":{"provider":"google"}}"#,
        )
        .unwrap();
        assert!(check_shape(&input).is_err());
    }
```

Append to `crates/havenkeys-core/tests/security.rs`, next to the other `save_login_*` tests:

```rust
fn google_login(title: &str, account: Option<&str>, url: &str) -> havenkeys_core::model::ItemInput {
    let mut input = login(title, "", "", url);
    input.username = None;
    input.password = havenkeys_core::model::SecretUpdate::Keep;
    input.sign_in_with = Some(havenkeys_core::sso::SignInWith {
        provider: havenkeys_core::sso::SsoProvider::Google,
        account: account.map(str::to_owned),
    });
    input
}

#[test]
fn sign_in_with_round_trips_and_is_searchable() {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(google_login("Typeform", Some(" me@gmail.com "), "typeform.com"), NOW)
        .unwrap();
    let ov = v.commit_write(staged, 5).unwrap().unwrap();
    let s = ov.sign_in_with.as_ref().unwrap();
    assert_eq!(s.account.as_deref(), Some("me@gmail.com"));
    assert!(!ov.has_password);
    assert_eq!(v.search("me@gmail").unwrap().len(), 1);
    assert_eq!(v.search("google").unwrap().len(), 1);
}

#[test]
fn password_update_keeps_sign_in_with() {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(google_login("Typeform", Some("me@gmail.com"), "typeform.com"), NOW)
        .unwrap();
    let id = v.commit_write(staged, 5).unwrap().unwrap().id;
    let staged = v
        .stage_save_login(
            "https://typeform.com/",
            None,
            None,
            secret("also-a-password"),
            SaveTarget::Update(&id),
            NOW,
        )
        .unwrap();
    let ov = v.commit_write(staged.write, 6).unwrap().unwrap();
    assert!(ov.has_password);
    assert_eq!(ov.sign_in_with.as_ref().unwrap().account.as_deref(), Some("me@gmail.com"));
}
```

- [ ] **Step 6: Run and confirm failure**

Run: `cargo test -p havenkeys-core`
Expected: FAIL to compile (no field `sign_in_with`).

- [ ] **Step 7: Add the fields and validation**

In `model.rs`:
- Import `use crate::sso::SignInWith;`.
- Add to `ItemOverview`, after `auto_sign_in`:

```rust
    /// "Sign in with <provider>" and the account used there. `default`:
    /// overviews written before this field existed have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in_with: Option<SignInWith>,
```

- Add to `ItemInput`, after `auto_sign_in`:

```rust
    /// "Sign in with …" for a login. Sent in full like `username`: `None`
    /// on an update clears it.
    #[serde(default)]
    pub sign_in_with: Option<SignInWith>,
```

- In `check_shape`, add `|| input.sign_in_with.is_some()` to the `SecureNote` condition list.

In `vault.rs` `build_item`:
- Destructure `sign_in_with` from `ItemInput` along with the other fields.
- Set the overview field:

```rust
        sign_in_with: match item_type {
            ItemType::Login => crate::sso::clean_sign_in_with(sign_in_with)?,
            ItemType::SecureNote => None,
        },
```

In `stage_save_login`'s `ItemInput` literal add `sign_in_with: None,`. In `stage_save_update`'s literal add `sign_in_with: existing.sign_in_with.clone(),`.

In `search`, extend the filter closure:

```rust
                    || i.sign_in_with.as_ref().is_some_and(|s| {
                        s.provider.name().to_lowercase().contains(&q)
                            || s.account.as_deref().is_some_and(|a| a.to_lowercase().contains(&q))
                    })
```

Add `sign_in_with: None,` to every other `ItemInput { … }` literal in the file map. Run `grep -rn "ItemInput {" crates apps --include='*.rs'` to find them all; the destructuring in `build_item` is not a literal. In `apps/desktop/src-tauri/src/item_input.rs`, add the field to `ItemInputWire`:

```rust
    #[serde(default)]
    pub sign_in_with: Option<havenkeys_core::sso::SignInWith>,
```

and pass `sign_in_with: self.sign_in_with,` in `resolve`.

- [ ] **Step 8: Run all core and dependent tests**

Run: `cargo test -p havenkeys-core && cargo test -p havenkeys-bridge && cargo test -p havenkeys-sync-client && cargo test -p havenkeys-desktop`
Expected: PASS. If the desktop crate has a different package name, find it with `grep ^name apps/desktop/src-tauri/Cargo.toml`.

- [ ] **Step 9: Commit**

```bash
git add crates apps/desktop/src-tauri/src/item_input.rs
git commit -m "feat(core): logins can sign in with Google, Microsoft, GitHub or Apple"
```

---

### Task 2: Core page operations — start, check and save

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (`Suggestion` ~144, `find_matches` ~1071, new methods after `auto_sign_in_for` ~1150)
- Test: `crates/havenkeys-core/tests/security.rs`

**Interfaces:**
- Consumes: Task 1 types.
- Produces (`havenkeys_core::vault`):
  - `Suggestion { …, account: Option<String>, provider: Option<SsoProvider> }`
  - `pub struct SsoStart { pub provider: SsoProvider, pub account: Option<String>, pub auto_choose: bool }`
  - `VaultService::start_sso_for_page(&self, id: &Uuid, page_url: &str, top_url: Option<&str>) -> Result<SsoStart>`
  - `VaultService::check_sso(&self, page_url: &str, top_url: Option<&str>, provider: SsoProvider, account: Option<&str>) -> Result<SaveAction>`
  - `VaultService::stage_save_sso(&self, page_url: &str, top_url: Option<&str>, provider: SsoProvider, account: Option<&str>, target: SaveTarget<'_>, now_ms: i64) -> Result<StagedSave>`

- [ ] **Step 1: Write the failing tests**

Append to `crates/havenkeys-core/tests/security.rs` (reuses `google_login` from Task 1). Add `use havenkeys_core::sso::SsoProvider;` and `use havenkeys_core::vault::SaveAction;` if they are not imported yet:

```rust
fn sso_vault() -> (havenkeys_core::vault::VaultService, Uuid) {
    let (mut v, _) = github_vault();
    let staged = v
        .stage_create(google_login("Typeform", Some("me@gmail.com"), "typeform.com"), NOW)
        .unwrap();
    let id = v.commit_write(staged, 5).unwrap().unwrap().id;
    (v, id)
}

#[test]
fn start_sso_is_origin_bound_and_returns_no_secret() {
    let (mut v, id) = sso_vault();
    let s = v.start_sso_for_page(&id, "https://admin.typeform.com/login", None).unwrap();
    assert_eq!(s.provider, SsoProvider::Google);
    assert_eq!(s.account.as_deref(), Some("me@gmail.com"));
    assert!(s.auto_choose);
    for page in ["https://evil.com/", "https://typeform.com.evil.com/", "http://typeform.com/", "javascript:x"] {
        assert_eq!(v.start_sso_for_page(&id, page, None).err(), Some(Error::Denied), "{page}");
    }
    // A frame of typeform.com embedded in evil.com gets nothing.
    assert_eq!(
        v.start_sso_for_page(&id, "https://typeform.com/", Some("https://evil.com/")).err(),
        Some(Error::Denied)
    );
    // A login without sign_in_with is not an SSO item.
    let gh = v.find_matches("https://github.com/", None).unwrap()[0].id;
    assert_eq!(v.start_sso_for_page(&gh, "https://github.com/", None).err(), Some(Error::Denied));
    assert_eq!(v.start_sso_for_page(&Uuid::new_v4(), "https://typeform.com/", None).err(), Some(Error::NotFound));
    v.lock();
    assert_eq!(v.start_sso_for_page(&id, "https://typeform.com/", None).err(), Some(Error::Locked));
}

#[test]
fn start_sso_auto_choose_follows_both_switches() {
    let (mut v, id) = sso_vault();
    let mut settings = v.settings().unwrap();
    settings.auto_sign_in = false;
    v.update_settings(settings).unwrap();
    assert!(!v.start_sso_for_page(&id, "https://typeform.com/", None).unwrap().auto_choose);
}

#[test]
fn find_matches_reports_provider_and_account() {
    let (v, id) = sso_vault();
    let m = v.find_matches("https://typeform.com/", None).unwrap();
    let s = m.iter().find(|s| s.id == id).unwrap();
    assert_eq!(s.provider, Some(SsoProvider::Google));
    assert_eq!(s.account.as_deref(), Some("me@gmail.com"));
    assert_eq!(s.username, None);
}

#[test]
fn check_sso_actions() {
    let (mut v, id) = sso_vault();
    let page = "https://typeform.com/";
    let check = |v: &havenkeys_core::vault::VaultService, p, a| v.check_sso(page, None, p, a).unwrap();
    assert_eq!(check(&v, SsoProvider::Google, Some("ME@gmail.com")), SaveAction::Unchanged);
    assert_eq!(check(&v, SsoProvider::Google, None), SaveAction::Unchanged);
    assert_eq!(check(&v, SsoProvider::Google, Some("other@gmail.com")), SaveAction::Add);
    assert_eq!(check(&v, SsoProvider::Github, None), SaveAction::Add);
    // An imported login with the provider but no account is offered the account.
    let staged = v.stage_create(google_login("Notion", None, "notion.so"), NOW).unwrap();
    let notion = v.commit_write(staged, 6).unwrap().unwrap().id;
    assert_eq!(
        v.check_sso("https://notion.so/", None, SsoProvider::Google, Some("me@gmail.com")).unwrap(),
        SaveAction::Update(notion)
    );
    let _ = id;
}

#[test]
fn save_sso_adds_without_a_password_and_updates_only_the_account() {
    let (mut v, _) = sso_vault();
    let staged = v
        .stage_save_sso("https://www.canva.com/login", None, SsoProvider::Apple, Some("me@icloud.com"),
            SaveTarget::New { title: Some("Canva") }, NOW)
        .unwrap();
    let ov = v.commit_write(staged.write, 7).unwrap().unwrap();
    assert_eq!(ov.title, "Canva");
    assert!(!ov.has_password);
    assert_eq!(ov.urls[0].url, "https://www.canva.com");
    assert_eq!(ov.sign_in_with.as_ref().unwrap().provider, SsoProvider::Apple);

    let staged = v.stage_create(google_login("Notion", None, "notion.so"), NOW).unwrap();
    let notion = v.commit_write(staged, 8).unwrap().unwrap().id;
    let staged = v
        .stage_save_sso("https://notion.so/", None, SsoProvider::Google, Some("me@gmail.com"),
            SaveTarget::Update(&notion), NOW)
        .unwrap();
    let ov = v.commit_write(staged.write, 9).unwrap().unwrap();
    assert_eq!(ov.title, "Notion");
    assert_eq!(ov.sign_in_with.as_ref().unwrap().account.as_deref(), Some("me@gmail.com"));

    // Another site's login, or a different provider, cannot be touched.
    assert_eq!(
        v.stage_save_sso("https://evil.com/", None, SsoProvider::Google, Some("x@y.z"), SaveTarget::Update(&notion), NOW).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.stage_save_sso("https://notion.so/", None, SsoProvider::Github, Some("x"), SaveTarget::Update(&notion), NOW).err(),
        Some(Error::Denied)
    );
    assert!(matches!(
        v.stage_save_sso("https://notion.so/", None, SsoProvider::Google, Some("a\u{7}b"), SaveTarget::New { title: None }, NOW),
        Err(Error::InvalidInput(_))
    ));
}
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test -p havenkeys-core --test security sso`
Expected: FAIL (missing methods and fields).

- [ ] **Step 3: Implement**

In `vault.rs`:

1. Add to `Suggestion`:
   ```rust
       /// The account of a "Sign in with" login (never a secret).
       pub account: Option<String>,
       pub provider: Option<crate::sso::SsoProvider>,
   ```
   and fill them in `find_matches`:
   ```rust
                       account: o.sign_in_with.as_ref().and_then(|s| s.account.clone()),
                       provider: o.sign_in_with.as_ref().map(|s| s.provider),
   ```
   Fix any other `Suggestion { … }` construction the compiler reports.

2. Add after `auto_sign_in_for`:

```rust
    /// Start a "Sign in with" pick: the provider, account and whether the
    /// run may click the account on the provider's chooser. Denied unless
    /// the item is a login saved for the page (and embedding page) that
    /// signs in with a provider. Nothing secret is returned.
    pub fn start_sso_for_page(
        &self,
        id: &Uuid,
        page_url: &str,
        top_url: Option<&str>,
    ) -> Result<SsoStart> {
        let overview = self.authorize_for_page(id, page_url, top_url)?;
        let s = overview.sign_in_with.as_ref().ok_or(Error::Denied)?;
        let settings = &self.session()?.settings;
        Ok(SsoStart {
            provider: s.provider,
            account: s.account.clone(),
            auto_choose: settings.auto_sign_in && overview.auto_sign_in,
        })
    }

    /// What saving a "Sign in with" the user just used on the page would do.
    /// See the spec §4: `Unchanged` when a login for the page already has
    /// this provider and account (or this provider and no account was
    /// learned); `Update(id)` when exactly one has this provider and no
    /// account; `Add` otherwise.
    pub fn check_sso(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        provider: SsoProvider,
        account: Option<&str>,
    ) -> Result<SaveAction> {
        let wanted = normalize_username(account);
        let mut without_account = Vec::new();
        for s in self.find_matches(page_url, top_url)? {
            if s.provider != Some(provider) {
                continue;
            }
            let have = normalize_username(s.account.as_deref());
            if wanted.is_none() || have == wanted {
                return Ok(SaveAction::Unchanged);
            }
            if have.is_none() {
                without_account.push(s.id);
            }
        }
        Ok(match without_account.as_slice() {
            [id] => SaveAction::Update(*id),
            _ => SaveAction::Add,
        })
    }

    /// Seal a "Sign in with" login the user confirmed in the save prompt.
    /// A new login is stored for the frame's own site with no password; an
    /// update must name a login saved for the page that already signs in
    /// with `provider`, and changes only its account.
    pub fn stage_save_sso(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        provider: SsoProvider,
        account: Option<&str>,
        target: SaveTarget<'_>,
        now_ms: i64,
    ) -> Result<StagedSave> {
        self.session()?;
        let sign_in_with = Some(SignInWith { provider, account: account.map(str::to_owned) });
        let input = match target {
            SaveTarget::New { title } => {
                let page = PageContext::parse(page_url, top_url).ok_or(Error::Denied)?;
                let (host, origin) = page.site_title_and_origin().ok_or(Error::Denied)?;
                let title = match title {
                    Some(t) => clean_title(t)?,
                    None => host,
                };
                ItemInput {
                    item_type: ItemType::Login,
                    title,
                    username: None,
                    urls: vec![UrlRule { url: origin, match_type: MatchType::Domain }],
                    password: SecretUpdate::Keep,
                    totp: SecretUpdate::Keep,
                    notes: SecretUpdate::Keep,
                    content: SecretUpdate::Keep,
                    auto_sign_in: None,
                    sign_in_with,
                }
            }
            SaveTarget::Update(id) => {
                let existing = self.authorize_for_page(id, page_url, top_url)?.clone();
                if existing.sign_in_with.as_ref().map(|s| s.provider) != Some(provider) {
                    return Err(Error::Denied);
                }
                let input = ItemInput {
                    item_type: ItemType::Login,
                    title: existing.title.clone(),
                    username: existing.username.clone(),
                    urls: existing.urls.clone(),
                    password: SecretUpdate::Keep,
                    totp: SecretUpdate::Keep,
                    notes: SecretUpdate::Keep,
                    content: SecretUpdate::Keep,
                    auto_sign_in: None,
                    sign_in_with,
                };
                return Ok(StagedSave { item_id: *id, write: self.stage_update(id, input, now_ms)? });
            }
        };
        let write = self.stage_create(input, now_ms)?;
        Ok(StagedSave { item_id: write.item_id, write })
    }
```

3. Add near `FillCredentials`:

```rust
/// Result of [`VaultService::start_sso_for_page`]. No secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SsoStart {
    pub provider: SsoProvider,
    pub account: Option<String>,
    pub auto_choose: bool,
}
```

The derived `Debug` would print the account. Implement `Debug` by hand instead, printing only `provider` and `auto_choose`, and drop `Debug` from the derive list.

4. Imports at the top of `vault.rs`: `use crate::sso::{SignInWith, SsoProvider};`.

An account that fails validation surfaces as `InvalidInput` from `build_item` through `stage_create`/`stage_update`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core
git commit -m "feat(core): origin-bound start, check and save for sign-in-with logins"
```

---

### Task 3: Rust protocol — `Match.provider`, `start_sso`, `check_sso`, `save_sso`

**Files:**
- Modify: `crates/havenkeys-protocol/src/message.rs`, `crates/havenkeys-protocol/src/lib.rs`
- Test: `crates/havenkeys-protocol/tests/messages.rs`

**Interfaces:**
- Produces (`havenkeys_protocol`):
  - `SsoProvider` (lowercase serde, own copy: this crate has no core dependency)
  - `MAX_ACCOUNT_BYTES = 4 * 254`
  - `Match.provider: Option<SsoProvider>` (always serialized, `null` when none)
  - `Request::StartSso { item_id, url, top_url }`
  - `Request::CheckSso { url, top_url, provider, account: Option<String> }`
  - `Request::SaveSso { url, top_url, provider, account: Option<String>, item_id: Option<Uuid>, title: Option<String> }`
  - `ResultBody::StartSso { provider, account: Option<String>, provider_origins: Vec<String>, auto_choose: bool }`
  - `ResultBody::CheckSso { action: SaveAction, item_id: Option<Uuid> }`
  - `ResultBody::SaveSso { item_id: Uuid }`
  - Wire kinds: `start_sso`, `check_sso`, `save_sso`

- [ ] **Step 1: Write the failing tests**

Append to `crates/havenkeys-protocol/tests/messages.rs`:

```rust
#[test]
fn sso_requests_parse() {
    let ok = [
        format!(r#"{{"v":1,"id":1,"request":{{"type":"start_sso","itemId":"{ITEM}","url":"https://typeform.com/"}}}}"#),
        r#"{"v":1,"id":2,"request":{"type":"check_sso","url":"https://typeform.com/","provider":"google","account":null}}"#.to_owned(),
        r#"{"v":1,"id":3,"request":{"type":"save_sso","url":"https://typeform.com/","provider":"github","account":"octo","itemId":null,"title":"Typeform"}}"#.to_owned(),
        format!(r#"{{"v":1,"id":4,"request":{{"type":"save_sso","url":"https://a.com/","topUrl":"https://b.com/","provider":"apple","account":null,"itemId":"{ITEM}"}}}}"#),
    ];
    for s in &ok {
        assert!(parse(s).is_ok(), "{s}");
    }
}

#[test]
fn sso_requests_are_bounded_and_strict() {
    let long = "a".repeat(4 * 254 + 1);
    let bad = [
        (r#"{"v":1,"id":1,"request":{"type":"check_sso","url":"https://a.com/","provider":"okta","account":null}}"#.to_owned(), ErrorCode::Malformed),
        (r#"{"v":1,"id":1,"request":{"type":"check_sso","url":"https://a.com/","provider":"google"}}"#.to_owned(), ErrorCode::Malformed),
        (format!(r#"{{"v":1,"id":1,"request":{{"type":"check_sso","url":"https://a.com/","provider":"google","account":"{long}"}}}}"#), ErrorCode::InvalidInput),
        (r#"{"v":1,"id":1,"request":{"type":"check_sso","url":"https://a.com/","provider":"google","account":""}}"#.to_owned(), ErrorCode::InvalidInput),
        // A title only names a new login.
        (format!(r#"{{"v":1,"id":1,"request":{{"type":"save_sso","url":"https://a.com/","provider":"google","account":null,"itemId":"{ITEM}","title":"x"}}}}"#), ErrorCode::InvalidInput),
        (r#"{"v":1,"id":1,"request":{"type":"start_sso","url":"https://a.com/"}}"#.to_owned(), ErrorCode::Malformed),
        (format!(r#"{{"v":1,"id":1,"request":{{"type":"start_sso","itemId":"{ITEM}","url":"https://a.com/","extra":1}}}}"#), ErrorCode::Malformed),
    ];
    for (s, code) in bad {
        assert_eq!(parse(&s).unwrap_err().code, code, "{s}");
    }
}

#[test]
fn sso_results_validate() {
    let ok = format!(r#"{{"v":1,"id":1,"result":{{"type":"start_sso","provider":"google","account":"me@gmail.com","providerOrigins":["https://accounts.google.com"],"autoChoose":true}}}}"#);
    assert!(Outgoing::parse(ok.as_bytes()).is_some());
    let too_many = r#"{"v":1,"id":1,"result":{"type":"start_sso","provider":"google","account":null,"providerOrigins":["a","b","c","d","e"],"autoChoose":true}}"#;
    assert!(Outgoing::parse(too_many.as_bytes()).is_none());
    let inconsistent = r#"{"v":1,"id":1,"result":{"type":"check_sso","action":"update","itemId":null}}"#;
    assert!(Outgoing::parse(inconsistent.as_bytes()).is_none());
    let m = format!(r#"{{"v":1,"id":1,"result":{{"type":"find_matches","matches":[{{"id":"{ITEM}","title":"t","username":null,"hasTotp":false,"strength":"same_site","provider":"github"}}]}}}}"#);
    assert!(Outgoing::parse(m.as_bytes()).is_some());
}
```

Also add these seeds to `fuzz_parse_request_never_panics`:

```rust
        format!(r#"{{"v":1,"id":17,"request":{{"type":"start_sso","itemId":"{ITEM}","url":"https://a.com/"}}}}"#),
        r#"{"v":1,"id":18,"request":{"type":"save_sso","url":"https://a.com/","provider":"google","account":"a","itemId":null}}"#.to_string(),
```

Existing `find_matches` result fixtures in this file carry a `Match` without `provider`. After Step 3 they fail parsing: add `,"provider":null` to each (`grep -n hasTotp crates/havenkeys-protocol/tests/messages.rs`).

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test -p havenkeys-protocol`
Expected: FAIL (unknown variants `start_sso` / `check_sso` / `save_sso`).

- [ ] **Step 3: Implement**

In `lib.rs`:

```rust
/// Largest account sent with check_sso/save_sso (the core allows 254 characters).
pub const MAX_ACCOUNT_BYTES: usize = 4 * 254;
/// Most provider origins a start_sso result may carry.
pub const MAX_PROVIDER_ORIGINS: usize = 4;
```

In `message.rs`:
- Import the two constants.
- Add the enum after `MatchStrength`:

```rust
/// A "Sign in with" provider. Mirrors havenkeys_core::sso::SsoProvider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SsoProvider {
    Google,
    Microsoft,
    Github,
    Apple,
}
```

- `Match`: add `pub provider: Option<SsoProvider>,` after `strength` (no `serde(default)`: always present).
- `Request`, after `OpenItem`:

```rust
    /// Start signing in with `item_id`'s provider, only if it is a login
    /// saved for `url` that signs in with one. No secrets come back.
    StartSso {
        item_id: Uuid,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// Would saving this "Sign in with" add a login, add the account to one,
    /// or do nothing?
    CheckSso {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        provider: SsoProvider,
        account: Option<String>,
    },
    /// Save a "Sign in with" after the user confirmed it. With `item_id`,
    /// set that login's account (it must be saved for `url`).
    SaveSso {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        provider: SsoProvider,
        account: Option<String>,
        item_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
```

- `kind()`: `Request::StartSso { .. } => "start_sso"`, `CheckSso` → `"check_sso"`, `SaveSso` → `"save_sso"`.
- `urls()`: add the three variants to the `[Some(url), top_url.as_deref()]` arm.
- `field_sizes_ok()`: extend `login_ok`:

```rust
            Request::CheckSso { account, .. } => account_ok(account),
            Request::SaveSso { account, title, item_id, .. } => {
                account_ok(account)
                    && title.as_ref().is_none_or(|t| {
                        item_id.is_none() && !t.is_empty() && t.len() <= MAX_TITLE_BYTES
                    })
            }
```

  with a helper next to `name_ok`:

```rust
fn account_ok(a: &Option<String>) -> bool {
    a.as_ref().is_none_or(|a| !a.is_empty() && a.len() <= MAX_ACCOUNT_BYTES)
}
```

- `ResultBody`, after `OpenItem {}`:

```rust
    StartSso {
        provider: SsoProvider,
        account: Option<String>,
        /// Where the run may click the account (exact origins, from Rust).
        provider_origins: Vec<String>,
        /// Rust's decision that the run may click the saved account.
        auto_choose: bool,
    },
    CheckSso {
        action: SaveAction,
        item_id: Option<Uuid>,
    },
    SaveSso {
        item_id: Uuid,
    },
```

- `Response::is_valid()` arms:

```rust
            Some(ResultBody::StartSso { provider_origins, .. }) => {
                !provider_origins.is_empty()
                    && provider_origins.len() <= MAX_PROVIDER_ORIGINS
                    && provider_origins.iter().all(|o| o.len() <= MAX_URL_BYTES)
            }
            Some(ResultBody::CheckSso { action, item_id }) => {
                (*action == SaveAction::Update) == item_id.is_some()
            }
```

- `ResultBody` `Debug` kinds: `"start_sso"`, `"check_sso"`, `"save_sso"`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-protocol`
Expected: PASS. The bridge crate will not compile yet (`Match` has a new field); Task 4 fixes that.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-protocol
git commit -m "feat(protocol): start_sso, check_sso, save_sso and Match.provider"
```

---

### Task 4: Bridge — dispatch, rate classes, attack tests

**Files:**
- Modify: `crates/havenkeys-bridge/src/dispatch.rs`, `crates/havenkeys-bridge/src/server.rs`
- Test: `crates/havenkeys-bridge/tests/bridge.rs`

**Interfaces:**
- Consumes: Task 2 core methods, Task 3 protocol types.
- Produces: `Dispatched::SaveSso(StagedSave)`. Requests answer with the typed results from Task 3.

- [ ] **Step 1: Write the failing tests**

In `bridge.rs`:
- Add a fourth fixture item. In `build_fixture`, after the note, create a Google login for `https://typeform.com`, account `me@gmail.com`, no password. Store its id as `Fixture.typeform`.
- Update `item()` and the note literal to include `sign_in_with: None`.

Then add:

```rust
fn start_sso(f: &Fixture, id: Uuid, url: &str) -> serde_json::Value {
    call(f, serde_json::json!({"type": "start_sso", "itemId": id, "url": url}))
}

#[test]
fn sso_flow_returns_no_secrets() {
    let f = fixture();
    let m = find(&f, "https://typeform.com/login");
    let row = &m["result"]["matches"][0];
    assert_eq!(row["provider"], "google");
    assert_eq!(row["username"], "me@gmail.com", "the account stands in for a missing username");
    let github_row = &find(&f, "https://github.com/")["result"]["matches"][0];
    assert!(github_row["provider"].is_null());

    let r = start_sso(&f, f.typeform, "https://typeform.com/login");
    assert_eq!(
        r["result"],
        serde_json::json!({"type": "start_sso", "provider": "google", "account": "me@gmail.com",
            "providerOrigins": ["https://accounts.google.com"], "autoChoose": true})
    );
}

/// A1/A2 for sign-in-with: another site, another item, a locked vault.
#[test]
fn start_sso_attacks_are_denied() {
    let f = fixture();
    for (url, id) in [
        ("https://evil.com/", f.typeform),
        ("https://typeform.com.evil.com/", f.typeform),
        ("https://github.com/", f.github), // a login without sign-in-with
        ("https://typeform.com/", Uuid::new_v4()),
        ("https://typeform.com/", f.note),
    ] {
        assert_eq!(error_code(&start_sso(&f, id, url)), Some("denied"), "{url}");
    }
    f.vault.lock().unwrap().lock();
    assert_eq!(error_code(&start_sso(&f, f.typeform, "https://typeform.com/")), Some("locked"));
}

#[test]
fn save_sso_flow() {
    let f = online_fixture();
    let r = call(&f, serde_json::json!({"type": "check_sso", "url": "https://typeform.com/", "provider": "google", "account": "ME@gmail.com"}));
    assert_eq!(r["result"], serde_json::json!({"type": "check_sso", "action": "unchanged", "itemId": null}));
    let r = call(&f, serde_json::json!({"type": "save_sso", "url": "https://canva.com/", "provider": "apple", "account": null, "itemId": null, "title": "Canva"}));
    assert_eq!(r["result"]["type"], "save_sso");
    assert_eq!(f.changes.load(Ordering::SeqCst), 1);
    // Cannot retarget another site's login.
    let r = call(&f, serde_json::json!({"type": "save_sso", "url": "https://evil.com/", "provider": "google", "account": "x", "itemId": f.typeform}));
    assert_eq!(error_code(&r), Some("denied"));
}
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test -p havenkeys-bridge`
Expected: FAIL (compile errors on the new `Match` field, unhandled variants).

- [ ] **Step 3: Implement dispatch**

In `dispatch.rs`:
- Import `SsoProvider as WireProvider` from the protocol and `havenkeys_core::sso::SsoProvider as CoreProvider`.
- Add two mappers:

```rust
fn wire_provider(p: CoreProvider) -> WireProvider {
    match p {
        CoreProvider::Google => WireProvider::Google,
        CoreProvider::Microsoft => WireProvider::Microsoft,
        CoreProvider::Github => WireProvider::Github,
        CoreProvider::Apple => WireProvider::Apple,
    }
}

fn core_provider(p: WireProvider) -> CoreProvider {
    match p {
        WireProvider::Google => CoreProvider::Google,
        WireProvider::Microsoft => CoreProvider::Microsoft,
        WireProvider::Github => CoreProvider::Github,
        WireProvider::Apple => CoreProvider::Apple,
    }
}
```

- `Match` construction in `FindMatches`:

```rust
                .map(|s| Match {
                    id: s.id,
                    title: s.title,
                    // A sign-in-with login shows its account where a username would go.
                    username: s.username.or(s.account),
                    has_totp: s.has_totp,
                    strength: strength(s.strength),
                    provider: s.provider.map(wire_provider),
                })
```

- Add `Dispatched::SaveSso(StagedSave)` with a doc comment mirroring `Save`.
- New arms:

```rust
        Request::StartSso { item_id, url, top_url } => {
            require_enabled(v)?;
            let s = v.start_sso_for_page(item_id, url, top_url.as_deref()).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::StartSso {
                provider: wire_provider(s.provider),
                account: s.account.clone(),
                provider_origins: s.provider.origins().iter().map(|o| (*o).to_owned()).collect(),
                auto_choose: s.auto_choose,
            }))
        }
        Request::CheckSso { url, top_url, provider, account } => {
            require_enabled(v)?;
            let (action, item_id) = match v
                .check_sso(url, top_url.as_deref(), core_provider(*provider), account.as_deref())
                .map_err(code)?
            {
                CoreSaveAction::Add => (SaveAction::Add, None),
                CoreSaveAction::Update(id) => (SaveAction::Update, Some(id)),
                CoreSaveAction::Unchanged => (SaveAction::Unchanged, None),
            };
            Ok(Dispatched::Done(ResultBody::CheckSso { action, item_id }))
        }
        Request::SaveSso { url, top_url, provider, account, item_id, title } => {
            require_enabled(v)?;
            let staged = v
                .stage_save_sso(
                    url,
                    top_url.as_deref(),
                    core_provider(*provider),
                    account.as_deref(),
                    match item_id {
                        Some(id) => SaveTarget::Update(id),
                        None => SaveTarget::New { title: title.as_deref() },
                    },
                    now_ms(unix_seconds),
                )
                .map_err(item_code)?;
            Ok(Dispatched::SaveSso(staged))
        }
```

Update the module doc comment's list of core methods.

- [ ] **Step 4: Implement the server changes**

In `server.rs` `handle`:
- Add `| Request::StartSso { .. } | Request::CheckSso { .. } | Request::SaveSso { .. }` to the `Secret` class arm. Each follows a user action; `start_sso` has a visible effect, and `check_sso` reveals which sites have logins.
- Extend the per-item update limiter: `if let Request::SaveLogin { item_id: Some(id), .. } | Request::SaveSso { item_id: Some(id), .. } = req`.
- Result mapping:

```rust
            Ok(Dispatched::SaveSso(staged)) => {
                let item_id = staged.item_id;
                (self.inner.save)(staged.write).map(|()| ResultBody::SaveSso { item_id })
            }
```

- `on_items_changed`: add `| Request::SaveSso { .. }` to the `matches!`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-bridge && cargo test -p havenkeys-native-host`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-bridge
git commit -m "feat(bridge): answer start_sso, check_sso and save_sso behind the core's origin checks"
```

---

### Task 5: TypeScript protocol — types, strict parsers, provider table

**Files:**
- Create: `packages/protocol/src/sso.ts`, `packages/protocol/src/sso-parity.test.ts`
- Modify: `packages/protocol/src/index.ts`, `packages/protocol/src/index.test.ts`, `packages/protocol/src/fuzz.test.ts`, `packages/protocol/tsconfig.json`, `packages/protocol/package.json` (devDependency `@types/node`, same version as `packages/ui`)

**Interfaces:**
- Produces (`@havenkeys/protocol`):
  - `type SsoProvider = "google" | "microsoft" | "github" | "apple"`
  - `SSO_PROVIDERS: Readonly<Record<SsoProvider, { name: string; origins: readonly string[] }>>`
  - `isSsoProvider(v: unknown): v is SsoProvider`
  - `providersForOrigin(origin: string): SsoProvider[]`
  - `MAX_ACCOUNT_CHARS = 254`
  - `Match.provider: SsoProvider | null`
  - Request variants:

    ```ts
    | { type: "start_sso"; itemId: string; url: string; topUrl?: string }
    | { type: "check_sso"; url: string; topUrl?: string; provider: SsoProvider; account: string | null }
    | { type: "save_sso"; url: string; topUrl?: string; provider: SsoProvider; account: string | null; itemId: string | null; title?: string }
    ```

  - Result variants:

    ```ts
    | { type: "start_sso"; provider: SsoProvider; account: string | null; providerOrigins: string[]; autoChoose: boolean }
    | { type: "check_sso"; action: SaveAction; itemId: string | null }
    | { type: "save_sso"; itemId: string }
    ```

- [ ] **Step 1: Write the failing tests**

`packages/protocol/src/sso-parity.test.ts`:

```ts
// Runs under Node in vitest; excluded from tsconfig because it uses Node APIs.
// The extension's provider table must be the core's: same providers, same
// names, same origins (crates/havenkeys-core/src/sso.rs).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { SSO_PROVIDERS } from "./sso";

const RUST = join(__dirname, "../../../crates/havenkeys-core/src/sso.rs");

function arms(src: string, fn: string): Record<string, string> {
  const body = src.slice(src.indexOf(`pub fn ${fn}(self)`));
  const block = body.slice(0, body.indexOf("\n    }\n"));
  const out: Record<string, string> = {};
  for (const m of block.matchAll(/Self::(\w+)\s*=>\s*(&\[[^\]]*\]|"[^"]*")/g)) out[(m[1] as string).toLowerCase()] = m[2] as string;
  return out;
}

describe("provider table", () => {
  it("matches havenkeys-core sso.rs", () => {
    const src = readFileSync(RUST, "utf8");
    const names = arms(src, "name");
    const origins = arms(src, "origins");
    expect(Object.keys(names).sort()).toEqual(Object.keys(SSO_PROVIDERS).sort());
    for (const [p, v] of Object.entries(SSO_PROVIDERS)) {
      expect(`"${v.name}"`).toBe(names[p]);
      expect([...(origins[p] as string).matchAll(/"([^"]+)"/g)].map((m) => m[1])).toEqual([...v.origins]);
    }
  });
});
```

Append to `packages/protocol/src/index.test.ts` (use the file's existing `ID` constant, or declare one):

```ts
describe("sign in with", () => {
  const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
  it("parses start_sso, check_sso, save_sso and Match.provider", () => {
    expect(parseIncoming({ v: 1, id: 1, result: { type: "start_sso", provider: "google", account: null, providerOrigins: ["https://accounts.google.com"], autoChoose: true } }))
      .toEqual({ kind: "result", id: 1, result: { type: "start_sso", provider: "google", account: null, providerOrigins: ["https://accounts.google.com"], autoChoose: true } });
    expect(parseIncoming({ v: 1, id: 1, result: { type: "check_sso", action: "update", itemId: ID } })?.kind).toBe("result");
    expect(parseIncoming({ v: 1, id: 1, result: { type: "save_sso", itemId: ID } })?.kind).toBe("result");
    const m = { id: ID, title: "t", username: null, hasTotp: false, strength: "same_site", provider: "github" };
    expect(parseIncoming({ v: 1, id: 1, result: { type: "find_matches", matches: [m] } })?.kind).toBe("result");
  });
  it("rejects wrong shapes", () => {
    const bad = [
      { type: "start_sso", provider: "okta", account: null, providerOrigins: ["x"], autoChoose: true },
      { type: "start_sso", provider: "google", account: null, providerOrigins: [], autoChoose: true },
      { type: "start_sso", provider: "google", account: null, providerOrigins: ["a", "b", "c", "d", "e"], autoChoose: true },
      { type: "check_sso", action: "update", itemId: null },
      { type: "find_matches", matches: [{ id: ID, title: "t", username: null, hasTotp: false, strength: "same_site" }] },
    ];
    for (const result of bad) expect(parseIncoming({ v: 1, id: 1, result })).toBeNull();
  });
  it("knows which providers an origin belongs to", () => {
    expect(providersForOrigin("https://github.com")).toEqual(["github"]);
    expect(providersForOrigin("https://accounts.google.com.evil.com")).toEqual([]);
  });
});
```

Import `providersForOrigin` from `./index` at the top of the file. In `fuzz.test.ts`, add `provider: null` to the existing `find_matches` seed and add seeds:

```ts
  { v: 1, id: 9, result: { type: "start_sso", provider: "google", account: "a@b.c", providerOrigins: ["https://accounts.google.com"], autoChoose: false } },
  { v: 1, id: 10, result: { type: "check_sso", action: "add", itemId: null } },
  { v: 1, id: 11, result: { type: "save_sso", itemId: ID } },
```

Also add inside the loop:

```ts
      if (msg.kind === "result" && msg.result.type === "check_sso") {
        expect(msg.result.action === "update").toBe(msg.result.itemId !== null);
      }
```

Add `"exclude": ["src/sso-parity.test.ts"]` to `packages/protocol/tsconfig.json`.

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/protocol test`
Expected: FAIL (`./sso` missing, parsers reject `provider`).

- [ ] **Step 3: Implement `sso.ts`**

```ts
// "Sign in with" providers. Mirrors crates/havenkeys-core/src/sso.rs, which
// is the authority (sso-parity.test.ts keeps them equal). The extension uses
// this table only to recognise provider pages while saving; a sign-in run
// continues only into the origins the desktop returns with start_sso.

export type SsoProvider = "google" | "microsoft" | "github" | "apple";

export const SSO_PROVIDERS: Readonly<Record<SsoProvider, { name: string; origins: readonly string[] }>> = {
  google: { name: "Google", origins: ["https://accounts.google.com"] },
  microsoft: { name: "Microsoft", origins: ["https://login.microsoftonline.com", "https://login.live.com"] },
  github: { name: "GitHub", origins: ["https://github.com"] },
  apple: { name: "Apple", origins: ["https://appleid.apple.com"] },
};

export const SSO_PROVIDER_IDS = Object.keys(SSO_PROVIDERS) as readonly SsoProvider[];

export const MAX_ACCOUNT_CHARS = 254;

export function isSsoProvider(v: unknown): v is SsoProvider {
  return typeof v === "string" && Object.prototype.hasOwnProperty.call(SSO_PROVIDERS, v);
}

/** Providers whose sign-in pages live on exactly this origin. */
export function providersForOrigin(origin: string): SsoProvider[] {
  return SSO_PROVIDER_IDS.filter((p) => SSO_PROVIDERS[p].origins.includes(origin));
}
```

- [ ] **Step 4: Implement `index.ts` changes**

- `export * from "./sso";` near the top, and `import { isSsoProvider, type SsoProvider } from "./sso";`.
- `Match`: add `provider: SsoProvider | null;`.
- `parseMatch`:

```ts
function parseMatch(v: unknown): Match | null {
  if (!isObj(v) || !hasExactKeys(v, ["id", "title", "username", "hasTotp", "strength", "provider"])) return null;
  const { id, title, username, hasTotp, strength, provider } = v;
  if (!isStr(id) || !UUID.test(id) || !isStr(title) || !isNullableStr(username) || !isBool(hasTotp)) return null;
  if (!STRENGTHS.includes(strength as MatchStrength)) return null;
  if (provider !== null && !isSsoProvider(provider)) return null;
  return { id, title, username, hasTotp, strength: strength as MatchStrength, provider };
}
```

- Add the three `Request` and three `Result` variants from **Interfaces**.
- `parseResult` cases:

```ts
    case "start_sso": {
      if (!hasExactKeys(v, ["type", "provider", "account", "providerOrigins", "autoChoose"])) return null;
      const { provider, account, providerOrigins, autoChoose } = v;
      if (!isSsoProvider(provider) || !isNullableStr(account) || !isBool(autoChoose)) return null;
      if (!Array.isArray(providerOrigins) || providerOrigins.length === 0 || providerOrigins.length > 4) return null;
      if (!providerOrigins.every((o) => isStr(o) && o.length <= MAX_URL_BYTES)) return null;
      return { type: "start_sso", provider, account, providerOrigins: [...providerOrigins] as string[], autoChoose };
    }
    case "check_sso": {
      if (!hasExactKeys(v, ["type", "action", "itemId"])) return null;
      const { action, itemId } = v;
      if (!SAVE_ACTIONS.includes(action as SaveAction)) return null;
      if (itemId !== null && !isUuid(itemId)) return null;
      if ((action === "update") !== (itemId !== null)) return null;
      return { type: "check_sso", action: action as SaveAction, itemId };
    }
    case "save_sso":
      if (!hasExactKeys(v, ["type", "itemId"]) || !isUuid(v.itemId)) return null;
      return { type: "save_sso", itemId: v.itemId };
```

- [ ] **Step 5: Run the protocol tests, then fix every consumer**

Run: `pnpm --filter @havenkeys/protocol test && pnpm --filter @havenkeys/protocol typecheck`
Expected: PASS.

Then `pnpm --filter @havenkeys/extension typecheck`. Every `Match` literal in extension tests now needs `provider: null`: `grep -rln "hasTotp:" apps/extension/src`. Add it until typecheck and `pnpm --filter @havenkeys/extension test` pass.

- [ ] **Step 6: Commit**

```bash
git add packages/protocol apps/extension/src
git commit -m "feat(protocol-ts): sign-in-with requests, results and provider table"
```

---

### Task 6: 1Password import — map `ssoLogin`, upgrade on re-import

**Files:**
- Modify: `crates/havenkeys-core/src/import/onepux.rs`, `crates/havenkeys-core/src/import/mod.rs`, `crates/havenkeys-core/src/vault.rs` (`stage_import`, `dedupe_keys`)
- Modify: `apps/desktop/src-tauri/src/import.rs`, `apps/desktop/src/lib/types.ts` (`ImportReport`), `apps/desktop/src/views/ImportSection.tsx`, `apps/desktop/src/i18n/{en,pt-BR}.ts`
- Test: `onepux.rs` unit tests, `crates/havenkeys-core/tests/writes.rs`

**Interfaces:**
- Consumes: `SsoProvider::from_name`, `SignInWith` (Task 1).
- Produces: `ImportReport.sso_upgraded: usize` (JSON `ssoUpgraded`). Legacy notes text constant `onepux::legacy_sso_note(provider_name) -> String` = `format!("Sign in with {name}")`.

- [ ] **Step 1: Write the failing tests**

In `onepux.rs` tests, update the existing test around line 560/667. It builds an item with `{"ssoLogin": {"provider": "Google"}}`. Change the assertion `notes.contains("Sign in with Google")` to:

```rust
        let sso = item.input.sign_in_with.as_ref().expect("ssoLogin becomes sign_in_with");
        assert_eq!(sso.provider, crate::sso::SsoProvider::Google);
        assert_eq!(sso.account, None);
```

Adapt the variable names to the test as written. Add:

```rust
    #[test]
    fn sso_login_with_account_and_unknown_provider() {
        let field = |v: serde_json::Value| json!({"title": "", "id": "sso", "value": {"ssoLogin": v}});
        for (value, expect) in [
            (json!({"provider": "GitHub", "username": "octo"}), Some(("github", Some("octo")))),
            (json!({"provider": "Okta"}), None),
        ] {
            let data = json!({"accounts": [{"attrs": {}, "vaults": [{"attrs": {"name": "V"}, "items": [{
                "uuid": "a", "categoryUuid": "001", "state": "active",
                "overview": {"title": "Site", "url": "https://site.example"},
                "details": {"loginFields": [], "sections": [{"title": "", "fields": [field(value)]}]}
            }]}]}]});
            let parsed = parse(&make_1pux(&data, 0)).unwrap();
            let item = &parsed.items[0];
            match expect {
                Some((p, account)) => {
                    let s = item.input.sign_in_with.as_ref().unwrap();
                    assert_eq!(serde_json::to_value(s.provider).unwrap(), p);
                    assert_eq!(s.account.as_deref(), account);
                }
                None => assert!(item.input.sign_in_with.is_none()),
            }
        }
    }
```

Before writing this test, copy the exact 1PUX JSON wrapper that `make_1pux` expects from the existing tests in the file; the `accounts/vaults/items` nesting above must match theirs.

In `crates/havenkeys-core/tests/writes.rs`, add a re-import test. Take the setup of `stage_import` from the existing test at line ~103.
1. Store a login titled `Typeform`, no username, URL `https://typeform.com`, notes exactly `Sign in with Google`, no `sign_in_with`. This is what the old importer wrote.
2. Call `stage_import` with an `ImportedItem` whose input has the same title and URLs plus `sign_in_with: Some(Google, None)` and notes `Keep`.
3. Assert:
   - `report.sso_upgraded == 1`, `report.skipped_duplicates == 0`, `report.imported == 0`;
   - after committing the single staged write (`commit_write(write, rev)`), the item has `sign_in_with` set and `has_notes == false`;
   - a second item whose notes are `Sign in with Google\nmore` keeps its notes after the same upgrade.

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test -p havenkeys-core import && cargo test -p havenkeys-core --test writes`
Expected: FAIL.

- [ ] **Step 3: Implement the mapping**

In `convert_item`'s section loop, before `render_value`, when `is_login` and the value has `ssoLogin`, capture it into a local `sso: Option<SignInWith>` (first one wins) and `continue`:

```rust
                if is_login && sso.is_none() {
                    if let Some(p) = value.get("ssoLogin").and_then(|s| str_at(s, &["provider"])).and_then(SsoProvider::from_name) {
                        let account = non_empty(str_at(&value["ssoLogin"], &["username"]))
                            .map(|a| clean_line(a, crate::sso::MAX_ACCOUNT_CHARS))
                            .filter(|a| !a.is_empty());
                        sso = Some(SignInWith { provider: p, account });
                        continue;
                    }
                }
```

An unknown provider falls through to `render_value`, which keeps writing `Sign in with {p}`. Set `sign_in_with: sso` in the login `ItemInput` and `sign_in_with: None` in the other two. Add to `onepux.rs`:

```rust
/// The notes line the importer wrote for a "Sign in with" login before
/// sign-in-with existed. A re-import clears notes that are exactly this.
pub(crate) fn legacy_sso_note(name: &str) -> String {
    format!("Sign in with {name}")
}
```

Use it in the `render_value` `ssoLogin` arm too, so the two texts cannot drift.

- [ ] **Step 4: Implement the re-import upgrade**

In `import/mod.rs` add to `ImportReport`:

```rust
    /// Logins already in the vault (same title, username and websites) that
    /// lacked "Sign in with" and got it from this import.
    pub sso_upgraded: usize,
```

In `vault.rs`:
- Replace `dedupe_keys` with `dedupe_index(&self) -> Result<(HashSet<[u8; 32]>, HashMap<[u8; 32], Uuid>)>`. The second map covers **logins without `sign_in_with`**, key → id. When a key has two such logins, remove it from the map: the importer does not guess.
- In `stage_import`'s loop, before the duplicate check:

```rust
            let key = dedupe_key(&overview, &details);
            if let Some(sso) = overview.sign_in_with.clone() {
                if let Some(target) = upgrade.remove(&key) {
                    if let Some(write) = self.stage_sso_upgrade(&target, sso, now_ms)? {
                        writes.push(write);
                        report.sso_upgraded += 1;
                        continue;
                    }
                }
            }
            if existing.contains(&key) { report.skipped_duplicates += 1; continue; }
```

- The helper:

```rust
    /// Give an existing login the "Sign in with" a re-import carries. Notes
    /// that are exactly the old importer's line are cleared; nothing else
    /// changes. `None` when the login no longer opens.
    fn stage_sso_upgrade(&self, id: &Uuid, sso: SignInWith, now_ms: i64) -> Result<Option<StagedWrite>> {
        let Ok(existing) = self.get_item(id) else { return Ok(None) };
        let legacy = crate::import::onepux::legacy_sso_note(sso.provider.name());
        let clear_notes = matches!(
            self.load_details(id),
            Ok(ItemDetails::Login { notes: Some(n), .. }) if n.expose().trim().eq_ignore_ascii_case(&legacy)
        );
        let input = ItemInput {
            item_type: ItemType::Login,
            title: existing.title.clone(),
            username: existing.username.clone(),
            urls: existing.urls.clone(),
            password: SecretUpdate::Keep,
            totp: SecretUpdate::Keep,
            notes: if clear_notes { SecretUpdate::Clear } else { SecretUpdate::Keep },
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: Some(sso),
        };
        self.stage_update(id, input, now_ms).map(Some)
    }
```

  The `legacy` comparison uses the provider's display name, case-insensitively, because the export's own spelling (`"Google"`, `"google"`) may differ.

- `report.imported = writes.len()` must now exclude upgrades: `report.imported = writes.len() - report.sso_upgraded;`.

In `apps/desktop/src-tauri/src/import.rs`: `report.imported = committed.saturating_sub(report.sso_upgraded);`.

Desktop UI:
- `ImportReport` in `types.ts`: add `ssoUpgraded: number`.
- `caveats()` in `ImportSection.tsx`: `if (r.ssoUpgraded) out.push(t.import.ssoUpgraded(r.ssoUpgraded));`.
- `en.ts` `import`: `ssoUpgraded: (n: number) => \`${n} ${n === 1 ? "login already in your vault now signs" : "logins already in your vault now sign"} in with Google, Microsoft, GitHub or Apple.\``
- `pt-BR.ts`: `ssoUpgraded: (n: number) => \`${n} ${n === 1 ? "login que já estava no cofre agora entra" : "logins que já estavam no cofre agora entram"} com Google, Microsoft, GitHub ou Apple.\``

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-core && pnpm --filter @havenkeys/desktop typecheck && pnpm --filter @havenkeys/desktop test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-core apps/desktop
git commit -m "feat(import): 1Password sign-in-with logins, and a re-import fills them in"
```

---

### Task 7: Provider icons in `@havenkeys/ui`

**Files:**
- Create: `packages/ui/src/provider-icons.ts`, `packages/ui/src/provider-icons.test.ts`
- Modify: `packages/ui/package.json` (export `"./provider-icons": "./src/provider-icons.ts"`), `apps/extension/package.json` (dependency `"@havenkeys/ui": "workspace:*"`), then run `pnpm install`

**Interfaces:**
- Produces: `PROVIDER_ICONS: Record<"google" | "microsoft" | "github" | "apple", { viewBox: string; shapes: ReadonlyArray<{ d?: string; rect?: [number, number, number, number]; fill: string }> }>`. `fill: "currentColor"` means "text colour".

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { PROVIDER_ICONS } from "./provider-icons";

describe("provider icons", () => {
  it("has one icon per provider, paths only", () => {
    expect(Object.keys(PROVIDER_ICONS).sort()).toEqual(["apple", "github", "google", "microsoft"]);
    for (const icon of Object.values(PROVIDER_ICONS)) {
      expect(icon.viewBox).toMatch(/^0 0 \d+ \d+$/);
      for (const s of icon.shapes) {
        expect(Boolean(s.d) !== Boolean(s.rect)).toBe(true);
        if (s.d) expect(s.d).toMatch(/^[MmLlHhVvCcSsQqTtAaZz0-9.,\s-]+$/);
        expect(s.fill).toMatch(/^(#[0-9A-F]{6}|currentColor)$/);
      }
    }
  });
});
```

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/ui test`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement**

```ts
// Provider marks for "Sign in with" rows. Path data only, rendered with
// createElementNS (extension) or JSX (desktop); never parsed from strings.
// Used to identify the provider next to its name, as the providers' own
// sign-in buttons do.

type Shape = { d?: string; rect?: [number, number, number, number]; fill: string };
export type ProviderIcon = { viewBox: string; shapes: readonly Shape[] };

export const PROVIDER_ICONS: Record<"google" | "microsoft" | "github" | "apple", ProviderIcon> = {
  google: {
    viewBox: "0 0 48 48",
    shapes: [
      { fill: "#EA4335", d: "M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z" },
      { fill: "#4285F4", d: "M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z" },
      { fill: "#FBBC05", d: "M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z" },
      { fill: "#34A853", d: "M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z" },
    ],
  },
  microsoft: {
    viewBox: "0 0 21 21",
    shapes: [
      { fill: "#F25022", rect: [0, 0, 10, 10] },
      { fill: "#7FBA00", rect: [11, 0, 10, 10] },
      { fill: "#00A4EF", rect: [0, 11, 10, 10] },
      { fill: "#FFB900", rect: [11, 11, 10, 10] },
    ],
  },
  github: {
    viewBox: "0 0 16 16",
    shapes: [
      {
        fill: "currentColor",
        d: "M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z",
      },
    ],
  },
  apple: {
    viewBox: "0 0 24 24",
    shapes: [
      {
        fill: "currentColor",
        d: "M16.37 12.7c-.02-2.3 1.88-3.4 1.96-3.46-1.07-1.56-2.73-1.78-3.32-1.8-1.41-.14-2.76.83-3.47.83-.72 0-1.82-.81-2.99-.79-1.54.02-2.96.9-3.75 2.27-1.6 2.78-.41 6.89 1.15 9.14.76 1.1 1.67 2.34 2.86 2.3 1.15-.05 1.58-.74 2.97-.74 1.38 0 1.77.74 2.98.72 1.23-.02 2.01-1.12 2.76-2.23.87-1.28 1.23-2.52 1.25-2.58-.03-.01-2.39-.92-2.4-3.66zM14.1 5.95c.63-.77 1.06-1.83.94-2.9-.91.04-2.01.61-2.66 1.37-.58.67-1.1 1.76-.96 2.8 1.01.08 2.05-.52 2.68-1.27z",
      },
    ],
  },
};
```

- [ ] **Step 4: Run the tests**

Run: `pnpm install && pnpm --filter @havenkeys/ui test && pnpm --filter @havenkeys/ui typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/ui apps/extension/package.json pnpm-lock.yaml
git commit -m "feat(ui): provider marks for sign-in-with rows"
```

---

### Task 8: Desktop — editor, detail, list

**Files:**
- Create: `apps/desktop/src/lib/sso.ts`, `apps/desktop/src/lib/sso.test.ts`, `apps/desktop/src/components/ProviderIcon.tsx`
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/openItem.ts`, `apps/desktop/src/views/ItemEditor.tsx`, `apps/desktop/src/views/ItemDetail.tsx`, `apps/desktop/src/views/ItemList.tsx`, `apps/desktop/src/views/VaultScreen.tsx`, `apps/desktop/src/i18n/{en,pt-BR}.ts`, the desktop stylesheet that holds `.row`/`.edit-row` (find it with `grep -rln "edit-row" apps/desktop/src --include=*.css`)

**Interfaces:**
- Consumes: `signInWith` in overview JSON (Task 1), `PROVIDER_ICONS` (Task 7).
- Produces:
  - `types.ts`:
    - `export type SsoProvider = "google" | "microsoft" | "github" | "apple";`
    - `export interface SignInWith { provider: SsoProvider; account: string | null }`
    - `ItemOverview.signInWith?: SignInWith`, `ItemInput.signInWith?: SignInWith | null`
  - `lib/sso.ts`:
    - `PROVIDER_NAMES: Record<SsoProvider, string>`
    - `PROVIDER_ORDER: SsoProvider[]`
    - `providerLogin(items: ItemOverview[], item: ItemOverview): ItemOverview | null`
    - `ssoSubtitle(s: SignInWith): string`

- [ ] **Step 1: Write the failing test**

`apps/desktop/src/lib/sso.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { ItemOverview } from "./types";
import { providerLogin, ssoSubtitle } from "./sso";

const base = { itemType: "login", hasPassword: true, hasTotp: false, hasNotes: false, hasPasskey: false, autoSignIn: true, createdAt: 0, updatedAt: 0 } as const;
const item = (id: string, over: Partial<ItemOverview>): ItemOverview => ({ id, title: id, username: null, urls: [], ...base, ...over });

describe("providerLogin", () => {
  const typeform = item("typeform", { urls: [{ url: "https://typeform.com/", matchType: "domain" }], signInWith: { provider: "google", account: "Me@gmail.com" } });
  it("finds the vault's login for the provider with the same account", () => {
    const google = item("google", { username: "me@gmail.com", urls: [{ url: "https://accounts.google.com/", matchType: "domain" }] });
    const other = item("other", { username: "you@gmail.com", urls: [{ url: "https://accounts.google.com/", matchType: "domain" }] });
    expect(providerLogin([typeform, other, google], typeform)?.id).toBe("google");
  });
  it("never matches look-alike hosts or a missing account", () => {
    const evil = item("evil", { username: "me@gmail.com", urls: [{ url: "https://google.com.evil.com/", matchType: "domain" }] });
    expect(providerLogin([typeform, evil], typeform)).toBeNull();
    const noAccount = { ...typeform, signInWith: { provider: "google" as const, account: null } };
    expect(providerLogin([noAccount], noAccount)).toBeNull();
  });
  it("formats the subtitle", () => {
    expect(ssoSubtitle({ provider: "github", account: null })).toBe("GitHub");
    expect(ssoSubtitle({ provider: "google", account: "me@gmail.com" })).toBe("Google · me@gmail.com");
  });
});
```

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/desktop test -- sso`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement `lib/sso.ts` and the types**

```ts
// "Sign in with" display helpers. The desktop only shows these; which pages
// a login may be used on is decided in Rust.

import type { ItemOverview, SignInWith, SsoProvider } from "./types";

export const PROVIDER_ORDER: SsoProvider[] = ["google", "microsoft", "github", "apple"];
export const PROVIDER_NAMES: Record<SsoProvider, string> = { google: "Google", microsoft: "Microsoft", github: "GitHub", apple: "Apple" };

/** Sites where the provider's own login would be saved. Display only. */
const PROVIDER_DOMAINS: Record<SsoProvider, string[]> = {
  google: ["google.com"],
  microsoft: ["microsoft.com", "microsoftonline.com", "live.com"],
  github: ["github.com"],
  apple: ["apple.com"],
};

function hostOf(url: string): string | null {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return null;
  }
}

/** The vault's login for this item's provider and account, to open from the detail view. */
export function providerLogin(items: ItemOverview[], item: ItemOverview): ItemOverview | null {
  const sso = item.signInWith;
  const account = sso?.account?.trim().toLowerCase();
  if (!sso || !account) return null;
  const domains = PROVIDER_DOMAINS[sso.provider];
  return (
    items.find(
      (o) =>
        o.id !== item.id &&
        o.itemType === "login" &&
        o.username?.trim().toLowerCase() === account &&
        o.urls.some((u) => {
          const h = hostOf(u.url);
          return h !== null && domains.some((d) => h === d || h.endsWith(`.${d}`));
        }),
    ) ?? null
  );
}

export function ssoSubtitle(s: SignInWith): string {
  return s.account ? `${PROVIDER_NAMES[s.provider]} · ${s.account}` : PROVIDER_NAMES[s.provider];
}
```

Add the types listed in **Interfaces** to `types.ts`, and `signInWith: SignInWith | null` to `EditorSnapshot` in `openItem.ts`.

- [ ] **Step 4: `ProviderIcon` component**

```tsx
import { PROVIDER_ICONS } from "@havenkeys/ui/provider-icons";
import type { SsoProvider } from "../lib/types";

export function ProviderIcon({ provider, size = 18 }: { provider: SsoProvider; size?: number }) {
  const icon = PROVIDER_ICONS[provider];
  return (
    <svg className="provider-icon" width={size} height={size} viewBox={icon.viewBox} aria-hidden="true">
      {icon.shapes.map((s, i) =>
        s.rect ? <rect key={i} x={s.rect[0]} y={s.rect[1]} width={s.rect[2]} height={s.rect[3]} fill={s.fill} /> : <path key={i} d={s.d} fill={s.fill} />,
      )}
    </svg>
  );
}
```

- [ ] **Step 5: Strings**

`en.ts`:

| Section | Key | Text |
|---|---|---|
| `editor` | `signInWith` | "Sign in with" |
| `editor` | `providerNone` | "Nothing (password only)" |
| `editor` | `providerPick` | "How you sign in" |
| `editor` | `account` | "Account" |
| `editor` | `accountPlaceholder` | "Email used there (optional)" |
| `editor` | `alsoPassword` | "Also has a password" |
| `detail` | `signInWith` | "Sign in with" |
| `detail` | `openProviderLogin` | `(provider: string) => \`Open the ${provider} login\`` |

`pt-BR.ts`:

| Section | Key | Text |
|---|---|---|
| `editor` | `signInWith` | "Entrar com" |
| `editor` | `providerNone` | "Nada (só senha)" |
| `editor` | `providerPick` | "Como você entra" |
| `editor` | `account` | "Conta" |
| `editor` | `accountPlaceholder` | "E-mail usado lá (opcional)" |
| `editor` | `alsoPassword` | "Também tem senha" |
| `detail` | `signInWith` | "Entrar com" |
| `detail` | `openProviderLogin` | `(provider: string) => \`Abrir o login do ${provider}\`` |

- [ ] **Step 6: Editor**

In `ItemEditor.tsx`:
- State: `const [signIn, setSignIn] = useState<SignInWith | null>(existing?.signInWith ?? null);` and `const [passwordOpen, setPasswordOpen] = useState(!existing?.signInWith || existing.hasPassword);`.
- Add `signInWith: signIn` to `snapshot`.
- Add `signInWith: signIn ? { provider: signIn.provider, account: signIn.account?.trim() || null } : null,` to the login `input`.
- Rendering: in the first `.group` of a login, **before** the username row, a "Sign in with" row:

```tsx
            <div className="row edit-row">
              <span className="edit-label">{t.editor.signInWith}</span>
              <span className="select-wrap">
                <select
                  value={signIn?.provider ?? ""}
                  aria-label={t.editor.providerPick}
                  onChange={(e) => {
                    const p = e.target.value as SsoProvider | "";
                    setSignIn(p ? { provider: p, account: signIn?.account ?? null } : null);
                    if (!p) setPasswordOpen(true);
                  }}
                >
                  <option value="">{t.editor.providerNone}</option>
                  {PROVIDER_ORDER.map((p) => (
                    <option key={p} value={p}>{PROVIDER_NAMES[p]}</option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
            </div>
            {signIn && (
              <label className="row edit-row">
                <span className="edit-label">{t.editor.account}</span>
                <input
                  className="edit-input"
                  value={signIn.account ?? ""}
                  onChange={(e) => setSignIn({ provider: signIn.provider, account: e.target.value })}
                  placeholder={t.editor.accountPlaceholder}
                  autoComplete="off"
                  spellCheck={false}
                  autoCapitalize="off"
                  maxLength={254}
                />
              </label>
            )}
```

- Wrap the existing username and password rows in `{passwordOpen ? (<>…</>) : (<button type="button" className="row row-button add-row" onClick={() => setPasswordOpen(true)}><Icon name="plus" size={15} /> {t.editor.alsoPassword}</button>)}`.

- [ ] **Step 7: Detail, list, wiring**

In `ItemDetail.tsx`, add props `items: ItemOverview[]` and `onOpen: (id: string) => void`. As the first child of the login `.group`:

```tsx
            {item.signInWith && (() => {
              const linked = providerLogin(items, item);
              const name = PROVIDER_NAMES[item.signInWith.provider];
              return (
                <Field
                  label={t.detail.signInWith}
                  actions={linked ? <IconButton icon="arrowRight" label={t.detail.openProviderLogin(name)} onClick={() => onOpen(linked.id)} /> : undefined}
                >
                  <span className="sso-value">
                    <ProviderIcon provider={item.signInWith.provider} />
                    <span>{name}</span>
                    {item.signInWith.account && <span className="muted selectable" data-truncate="">{item.signInWith.account}</span>}
                  </span>
                </Field>
              );
            })()}
```

Check `Icon`'s name union in `components/Icon.tsx`. If there is no `arrowRight`, use an existing arrow or chevron icon name.

In `VaultScreen.tsx`, pass `items={items}` and `onOpen={(id) => setPane({ kind: "view", id })}` to `ItemDetail`. The selection state may be a separate `selectedId` setter; follow how `ItemList`'s `onSelect` does it at line ~318.

In `ItemList.tsx` row data: `const data = item.itemType === "login" ? (item.username ?? (item.signInWith ? ssoSubtitle(item.signInWith) : null) ?? primaryHost(item)) : null;`.

CSS, next to the `.row` rules: `.sso-value { display: inline-flex; align-items: center; gap: 8px; min-width: 0; } .provider-icon { flex: none; }`.

- [ ] **Step 8: Run the checks**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: PASS.

Then run `pnpm ui:check` if it covers the desktop editor or detail. If a fixture's `ItemOverview` literal now fails typecheck, add nothing: `signInWith` is optional.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop
git commit -m "feat(desktop): edit and show how a login signs in (Google, Microsoft, GitHub, Apple)"
```

---

### Task 9: Extension — recognising provider buttons and chooser rows

**Files:**
- Create: `apps/extension/src/autofill/sso.ts`, `apps/extension/src/autofill/sso.test.ts`

**Interfaces:**
- Consumes: `Env` (`autofill/group.ts`), `normalize`, `hasPhrase`, `hasAny`, `MAX_HINT_CHARS` (`autofill/text.ts`), `SSO_PROVIDERS`, `SsoProvider` (`@havenkeys/protocol`).
- Produces:
  - `SSO_CANDIDATES: string`
  - `MAX_SSO_CANDIDATES = 400`
  - `SSO_MIN_SCORE = 60`
  - `providerOf(el: Element, context: boolean): { provider: SsoProvider; score: number } | null`
  - `findProviderButtons(root: ParentNode, env: Env): Map<SsoProvider, HTMLElement>`
  - `providerButton(root: ParentNode, provider: SsoProvider, env: Env): HTMLElement | null`
  - `emailIn(el: Element): string | null`
  - `chooserRow(root: ParentNode, account: string, env: Env): HTMLElement | null`
  - `isConsentScreen(root: ParentNode, env: Env): boolean`

- [ ] **Step 1: Write the failing tests**

```ts
// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Env } from "./group";
import { chooserRow, emailIn, findProviderButtons, isConsentScreen, providerButton, MAX_SSO_CANDIDATES } from "./sso";

const env: Env = { isVisible: (el) => !el.hidden && el.style.display !== "none", path: "/login" };
const page = (html: string) => {
  document.body.innerHTML = html; // test fixture only; hygiene.test.ts scans src excluding tests
  return document.body;
};

describe("provider buttons", () => {
  it("recognises common English and Portuguese labels", () => {
    const root = page(`
      <button>Continue with Google</button>
      <a href="https://github.com/login/oauth/authorize?client_id=x">Sign in with GitHub</a>
      <div role="button" aria-label="Entrar com a Microsoft"></div>
      <button><img alt="Apple"> Continuar com Apple</button>`);
    expect([...findProviderButtons(root, env).keys()].sort()).toEqual(["apple", "github", "google", "microsoft"]);
  });
  it("accepts a bare name only with context", () => {
    expect(findProviderButtons(page(`<button>Google</button>`), env).size).toBe(0);
    expect(findProviderButtons(page(`<button>Google</button><button>GitHub</button>`), env).size).toBe(2);
    expect(findProviderButtons(page(`<input type="email"><button>Google</button>`), env).size).toBe(1);
  });
  it("ignores look-alikes, hidden and disabled buttons", () => {
    const root = page(`
      <a href="https://drive.google.com">Open in Google Drive</a>
      <a href="https://github.com/x/y">Star on GitHub</a>
      <button hidden>Sign in with Google</button>
      <button disabled>Sign in with Apple</button>
      <p>Sign in with Google to continue reading this very long paragraph that is not a button</p>`);
    expect(findProviderButtons(root, env).size).toBe(0);
  });
  it("picks the button to press", () => {
    const root = page(`<button>Continue with Google</button><button>Continue with GitHub</button>`);
    expect(providerButton(root, "github", env)?.textContent).toBe("Continue with GitHub");
    expect(providerButton(root, "apple", env)).toBeNull();
  });
  it("scan_is_bounded", () => {
    const filler = Array.from({ length: MAX_SSO_CANDIDATES + 50 }, (_, i) => `<a href="/p${i}">Page ${i}</a>`).join("");
    const root = page(`${filler}<button>Continue with Google</button>`);
    const t0 = performance.now();
    expect(findProviderButtons(root, env).size).toBe(0);
    expect(performance.now() - t0).toBeLessThan(200);
  });
});

describe("account chooser", () => {
  it("finds the email in a row", () => {
    expect(emailIn(page(`<li><div>Me</div><div>Me@Gmail.com</div></li>`).firstElementChild as Element)).toBe("me@gmail.com");
    expect(emailIn(page(`<li>Use another account</li>`).firstElementChild as Element)).toBeNull();
  });
  it("chooser_row_requires_a_unique_clickable_match", () => {
    const one = page(`<ul><li><div role="link" data-identifier="me@gmail.com">Me <div>me@gmail.com</div></div></li>
      <li><div role="link">You <div>you@gmail.com</div></div></li></ul>`);
    expect(chooserRow(one, "ME@gmail.com", env)?.getAttribute("data-identifier")).toBe("me@gmail.com");
    const two = page(`<div role="link">me@gmail.com</div><div role="link">Signed in as me@gmail.com</div>`);
    expect(chooserRow(two, "me@gmail.com", env)).toBeNull();
    const none = page(`<p>me@gmail.com</p>`);
    expect(chooserRow(none, "me@gmail.com", env)).toBeNull();
  });
  it("sees consent screens", () => {
    expect(isConsentScreen(page(`<p>Typeform wants to access your account</p><button>Allow</button><button>Cancel</button>`), env)).toBe(true);
    expect(isConsentScreen(page(`<button>Continuar</button>`), env)).toBe(true);
    expect(isConsentScreen(page(`<div role="link">me@gmail.com</div><div role="link">Use another account</div>`), env)).toBe(false);
  });
});
```

`hygiene.test.ts` may flag `innerHTML` in test files. Check its file filter; if it scans tests, build the fixtures with `document.createElement` helpers instead, as `submit.test.ts` does.

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- autofill/sso`
Expected: FAIL (module missing).

- [ ] **Step 3: Implement**

```ts
// Recognising "Sign in with <provider>" buttons, and the saved account's row
// on a provider's account chooser.
//
// The DOM is untrusted: its strings are only compared against fixed lists,
// and the work per scan is bounded (MAX_SSO_CANDIDATES). A match only lets
// the extension *offer* a sign-in the desktop already matched to this page;
// what a run may do on the provider's page is bounded by origins from Rust.

import { SSO_PROVIDERS, type SsoProvider } from "@havenkeys/protocol";
import type { Env } from "./group";
import { hasAny, hasPhrase, MAX_HINT_CHARS, normalize } from "./text";

export const SSO_CANDIDATES = 'button, a[href], [role="button"], [role="link"], input[type="submit"], input[type="button"]';
export const MAX_SSO_CANDIDATES = 400;
export const SSO_MIN_SCORE = 60;
/** A label longer than this (normalized) is prose, not a button. */
const MAX_LABEL_CHARS = 60;

const NAMES: Record<SsoProvider, string> = { google: "google", microsoft: "microsoft", github: "github", apple: "apple" };
const JOINERS = ["with", "using", "via", "com", "com a", "com o", "pelo", "pela"];
const NEGATIVE = [
  "drive", "docs", "play", "store", "maps", "calendar", "repository", "repo", "star", "fork", "sponsor",
  "download", "app store", "teams", "office", "outlook", "music", "pay", "wallet", "podcasts", "tv",
];
const CONSENT_WORDS = ["allow", "authorize", "authorise", "grant", "accept", "continue", "permitir", "autorizar", "aceitar", "continuar", "confirmar"];
const EMAIL = /[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/i;

function label(el: Element): string {
  const attr = (n: string) => (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
  const own = el instanceof HTMLInputElement ? el.value.slice(0, MAX_HINT_CHARS) : (el.textContent ?? "").slice(0, MAX_HINT_CHARS);
  const img = el.querySelector("img[alt]")?.getAttribute("alt")?.slice(0, 60) ?? "";
  return normalize(`${own} ${attr("aria-label")} ${attr("title")} ${img}`, MAX_HINT_CHARS * 3);
}

function hrefProvider(el: Element): SsoProvider | null {
  const href = el instanceof HTMLAnchorElement ? el.getAttribute("href") : null;
  if (!href) return null;
  let origin: string;
  try {
    origin = new URL(href, document.baseURI).origin;
  } catch {
    return null;
  }
  for (const [p, v] of Object.entries(SSO_PROVIDERS)) if (v.origins.includes(origin)) return p as SsoProvider;
  return null;
}

function usable(el: HTMLElement, env: Env): boolean {
  return env.isVisible(el) && !el.matches(":disabled") && el.getAttribute("aria-disabled") !== "true";
}

/**
 * The provider this element signs in with, and how sure we are. `context`:
 * the page also has a login field or another provider button, which lets a
 * bare "Google" label count.
 */
export function providerOf(el: Element, context: boolean): { provider: SsoProvider; score: number } | null {
  const text = label(el);
  if (!text || text.length > MAX_LABEL_CHARS || hasAny(text, NEGATIVE)) return null;
  const linked = hrefProvider(el);
  let best: { provider: SsoProvider; score: number } | null = null;
  for (const [p, name] of Object.entries(NAMES) as [SsoProvider, string][]) {
    if (!hasPhrase(text, name)) continue;
    let s = JOINERS.some((j) => hasPhrase(text, `${j} ${name}`)) ? 80 : text === name || text === `${name} account` ? 40 : 0;
    if (s === 40 && context) s += 20;
    if (linked === p) s += 30;
    if (s > 0 && (!best || s > best.score)) best = { provider: p, score: s };
  }
  return best;
}

function candidates(root: ParentNode, env: Env): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(SSO_CANDIDATES))
    .slice(0, MAX_SSO_CANDIDATES)
    .filter((el) => usable(el, env));
}

function hasLoginField(root: ParentNode): boolean {
  return root.querySelector('input[type="password"], input[type="email"], input[autocomplete~="username"]') !== null;
}

function scored(root: ParentNode, env: Env): { el: HTMLElement; provider: SsoProvider; score: number }[] {
  const els = candidates(root, env);
  const first = els.map((el) => ({ el, hit: providerOf(el, false) }));
  const providers = new Set(first.flatMap((x) => (x.hit ? [x.hit.provider] : [])));
  const context = providers.size >= 2 || hasLoginField(root);
  const out: { el: HTMLElement; provider: SsoProvider; score: number }[] = [];
  for (const { el } of first) {
    const hit = providerOf(el, context);
    if (hit && hit.score >= SSO_MIN_SCORE) out.push({ el, ...hit });
  }
  return out;
}

/** Each provider with a qualifying button on the page, and its best button. */
export function findProviderButtons(root: ParentNode, env: Env): Map<SsoProvider, HTMLElement> {
  const out = new Map<SsoProvider, { el: HTMLElement; score: number }>();
  for (const c of scored(root, env)) {
    const cur = out.get(c.provider);
    if (!cur || c.score > cur.score) out.set(c.provider, c);
  }
  return new Map([...out].map(([p, c]) => [p, c.el]));
}

/** The button to press for `provider`: the highest score, first in document order on a tie. */
export function providerButton(root: ParentNode, provider: SsoProvider, env: Env): HTMLElement | null {
  return findProviderButtons(root, env).get(provider) ?? null;
}

/** The email address in an element's text, lowercased, or null. */
export function emailIn(el: Element): string | null {
  const m = EMAIL.exec((el.textContent ?? "").slice(0, 400)) ?? EMAIL.exec(el.getAttribute("data-identifier") ?? el.getAttribute("data-email") ?? "");
  return m ? m[0].toLowerCase() : null;
}

/** The chooser row for `account`: exactly one clickable, visible element whose text holds it. */
export function chooserRow(root: ParentNode, account: string, env: Env): HTMLElement | null {
  const want = account.trim().toLowerCase();
  const hits = candidates(root, env).filter((el) => emailIn(el) === want);
  // A row nested in another row (link inside list item) counts once: keep the outermost.
  const outer = hits.filter((el) => !hits.some((o) => o !== el && o.contains(el)));
  return outer.length === 1 ? (outer[0] as HTMLElement) : null;
}

/** A permissions or confirmation screen: HavenKeys never presses these. */
export function isConsentScreen(root: ParentNode, env: Env): boolean {
  return candidates(root, env).some((el) => {
    const t = label(el);
    return t.length <= 30 && hasAny(t, CONSENT_WORDS);
  });
}
```

`hasPhrase` works on normalized text, so `"entrar com a microsoft"` contains `"com a microsoft"`. The `providerOf` loop takes the highest score across providers, so a label naming two providers keeps the stronger one.

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test -- autofill/sso && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS. If a fixture fails because jsdom lacks `:disabled` on `div`s, keep the fixture and fix `usable`; the check must stay.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/sso.ts apps/extension/src/autofill/sso.test.ts
git commit -m "feat(extension): recognise sign-in-with buttons and the account chooser"
```

---

### Task 10: Extension — SSO message types and validators

**Files:**
- Create: `apps/extension/src/messaging/sso.ts`, `apps/extension/src/messaging/sso.test.ts`

**Interfaces:**
- Consumes: `TOKEN`, `newToken` pattern (`messaging/inline.ts`), `isSsoProvider`, `isUuid`, `MAX_ACCOUNT_CHARS`, `SsoProvider` (`@havenkeys/protocol`).
- Produces:

```ts
export type SsoContentRequest =
  | { type: "cs_sso_buttons"; providers: SsoProvider[] }   // top frame: provider buttons visible
  | { type: "cs_sso_click"; provider: SsoProvider }        // trusted click on a provider button
  | { type: "cs_sso_account"; account: string }            // trusted click on a chooser row (provider origins)
  | { type: "cs_sso_stop" };                               // the user took over after we pressed
export type SsoButtonsReply = { ok: true; token: string } | { ok: false };
export type SsoReady = { kind: "choose"; account: string } | null;
export type BackgroundToSso =
  | { type: "bg_sso_show"; token: string }
  | { type: "bg_sso_close"; token: string }
  | { type: "bg_sso_resize"; token: string; height: number }
  | { type: "bg_sso_press"; provider: SsoProvider };
export type SsoPressReply = { pressed: boolean };
export type SsoFrameRequest =
  | { type: "sso_state"; token: string }
  | { type: "sso_pick"; token: string; itemId: string }
  | { type: "sso_save"; token: string; account: string; title: string | null }
  | { type: "sso_dismiss"; token: string }
  | { type: "sso_resize"; token: string; height: number };
export interface SsoRowView { id: string; provider: SsoProvider; title: string; account: string | null }
export type SsoView =
  | { mode: "offer"; site: string; rows: SsoRowView[] }
  | { mode: "save"; site: string; provider: SsoProvider; account: string | null; title: string | null; action: "add" | "update" }
  | { mode: "notice"; site: string; provider: SsoProvider };
export const SSO_MIN_HEIGHT = 90;
export const SSO_MAX_HEIGHT = 360;
export function parseSsoContentRequest(msg: unknown): SsoContentRequest | null;
export function parseSsoFrameRequest(msg: unknown): SsoFrameRequest | null;
export function parseSsoBackgroundMessage(msg: unknown): BackgroundToSso | null;
export function parseSsoReady(v: unknown): SsoReady;
export function parsePressReply(v: unknown): SsoPressReply;
export function isAccount(v: unknown): v is string;
```

- [ ] **Step 1: Write the failing tests**

```ts
import { describe, expect, it } from "vitest";
import { isAccount, parsePressReply, parseSsoBackgroundMessage, parseSsoContentRequest, parseSsoFrameRequest, parseSsoReady } from "./sso";

const T = "0123456789abcdef0123456789abcdef";
const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

describe("sso messages", () => {
  it("accepts exact shapes", () => {
    expect(parseSsoContentRequest({ type: "cs_sso_buttons", providers: ["google", "github"] })).toEqual({ type: "cs_sso_buttons", providers: ["google", "github"] });
    expect(parseSsoContentRequest({ type: "cs_sso_click", provider: "apple" })).not.toBeNull();
    expect(parseSsoContentRequest({ type: "cs_sso_account", account: "me@gmail.com" })).not.toBeNull();
    expect(parseSsoContentRequest({ type: "cs_sso_stop" })).not.toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_pick", token: T, itemId: ID })).not.toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_save", token: T, account: "", title: null })).not.toBeNull();
    expect(parseSsoBackgroundMessage({ type: "bg_sso_press", provider: "google" })).not.toBeNull();
    expect(parseSsoReady({ kind: "choose", account: "me@gmail.com" })).toEqual({ kind: "choose", account: "me@gmail.com" });
  });
  it("rejects everything else", () => {
    for (const bad of [
      { type: "cs_sso_buttons", providers: [] },
      { type: "cs_sso_buttons", providers: ["google", "google"] },
      { type: "cs_sso_buttons", providers: ["okta"] },
      { type: "cs_sso_click", provider: "google", extra: 1 },
      { type: "cs_sso_account", account: "not an email" },
      { type: "cs_sso_account", account: `${"a".repeat(250)}@b.co` },
    ]) expect(parseSsoContentRequest(bad)).toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_pick", token: "short", itemId: ID })).toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_save", token: T, account: "x".repeat(255), title: null })).toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_resize", token: T, height: 9999 })).toBeNull();
    expect(parseSsoReady({ kind: "choose", account: 5 })).toBeNull();
    expect(parsePressReply(undefined)).toEqual({ pressed: false });
    expect(isAccount("me@x.io")).toBe(true);
  });
});
```

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- messaging/sso`
Expected: FAIL.

- [ ] **Step 3: Implement**

Follow `messaging/inline.ts`: the same `obj` and `keysAre` helpers (copy them), and `TOKEN` imported from `./inline`. Rules:
- `providers`: array of 1–4 distinct `isSsoProvider` values.
- `isAccount`: string, 3–254 chars, matches `/^[^\s@]+@[^\s@]+\.[^\s@]+$/`. The content script only ever sends email-shaped text.
- `sso_save.account`: string of 0–254 chars; empty means none.
- `sso_save.title`: `null` or a string ≤ `MAX_TITLE_CHARS` (from `./inline`).
- Heights: integers within `SSO_MIN_HEIGHT..=SSO_MAX_HEIGHT`.
- `parsePressReply`: anything but `{pressed: true}` reads as `{pressed: false}`.
- `parseSsoReady`: anything but a valid `{kind:"choose", account: isAccount}` → `null`.

Write each parser as a `switch (o.type)` with `keysAre` checks, like `parseContentRequest`.

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test -- messaging/sso && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/messaging/sso.ts apps/extension/src/messaging/sso.test.ts
git commit -m "feat(extension): strictly validated messages for sign-in-with"
```

---

### Task 11: Background state — pending captures and runs (pure)

**Files:**
- Create: `apps/extension/src/background/sso-state.ts`, `apps/extension/src/background/sso-state.test.ts`

**Interfaces:**
- Consumes: `SSO_PROVIDERS`, `SsoProvider`.
- Produces:

```ts
export const PENDING_TTL_MS = 5 * 60_000;
export const SSO_RUN_TTL_MS = 2 * 60_000;
export interface TabRef { tabId: number; openerTabId?: number }
export interface PendingSso { tabId: number; url: string; topUrl?: string; provider: SsoProvider; account: string | null; sawProvider: boolean; popupTabId: number | null; expires: number }
export interface SsoRun { tabId: number; frameId: number; siteOrigin: string; provider: SsoProvider; account: string | null; providerOrigins: string[]; autoChoose: boolean; phase: "press" | "choose"; expires: number }
export function createSsoState(now: () => number): {
  click(tabId: number, url: string, topUrl: string | undefined, provider: SsoProvider): void;
  visit(tab: TabRef, origin: string): void;                    // any frame load: marks sawProvider / popupTabId
  account(tab: TabRef, origin: string, account: string): boolean;
  takeReturn(tabId: number, origin: string): PendingSso | null;  // top-frame load back on a non-provider origin
  takeOnTabClosed(tabId: number): PendingSso | null;            // the provider popup closed
  pending(tabId: number): PendingSso | null;
  dropPending(tabId: number): void;
  startRun(r: Omit<SsoRun, "phase" | "expires">): SsoRun;
  run(tabId: number): SsoRun | null;
  pressed(tabId: number): SsoRun | null;                       // → "choose" if autoChoose && account, else ends; returns the live run or null
  chooseFor(tab: TabRef, origin: string): string | null;       // ends the run on success
  topLoad(tabId: number, origin: string): void;                // ends a run whose tab left site + provider origins
  endRun(tabId: number): void;
  forgetTab(tabId: number): void;
  clear(): void;
};
```

- [ ] **Step 1: Write the failing tests**

```ts
import { describe, expect, it } from "vitest";
import { createSsoState, PENDING_TTL_MS, SSO_RUN_TTL_MS } from "./sso-state";

function setup() {
  let t = 1_000;
  const s = createSsoState(() => t);
  return { s, advance: (ms: number) => (t += ms) };
}
const G = "https://accounts.google.com";

describe("pending captures", () => {
  it("prompts when the tab comes back after the provider", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/login", undefined, "google");
    s.visit({ tabId: 1 }, G);
    expect(s.account({ tabId: 1 }, G, "me@gmail.com")).toBe(true);
    const p = s.takeReturn(1, "https://admin.typeform.com");
    expect(p?.account).toBe("me@gmail.com");
    expect(s.pending(1)).toBeNull();
  });
  it("return_before_provider_is_ignored", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/login", undefined, "google");
    expect(s.takeReturn(1, "https://typeform.com")).toBeNull();
    expect(s.pending(1)).not.toBeNull();
  });
  it("learns the account only on the provider's own origins, also in a popup", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/", undefined, "google");
    expect(s.account({ tabId: 1 }, "https://evil.com", "x@y.z")).toBe(false);
    expect(s.account({ tabId: 9, openerTabId: 1 }, "https://login.live.com", "x@y.z")).toBe(false);
    s.visit({ tabId: 9, openerTabId: 1 }, G);
    expect(s.account({ tabId: 9, openerTabId: 1 }, G, "me@gmail.com")).toBe(true);
    expect(s.takeOnTabClosed(9)?.account).toBe("me@gmail.com");
  });
  it("expires", () => {
    const { s, advance } = setup();
    s.click(1, "https://typeform.com/", undefined, "google");
    s.visit({ tabId: 1 }, G);
    advance(PENDING_TTL_MS + 1);
    expect(s.takeReturn(1, "https://typeform.com")).toBeNull();
  });
});

describe("runs", () => {
  const start = (s: ReturnType<typeof createSsoState>, over: Partial<{ account: string | null; autoChoose: boolean }> = {}) =>
    s.startRun({ tabId: 1, frameId: 0, siteOrigin: "https://typeform.com", provider: "google", account: "me@gmail.com", providerOrigins: [G], autoChoose: true, ...over });

  it("choose_only_on_provider_origins", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    expect(s.chooseFor({ tabId: 1 }, "https://evil.com")).toBeNull();
    expect(s.chooseFor({ tabId: 1 }, "https://accounts.google.com.evil.com")).toBeNull();
    expect(s.chooseFor({ tabId: 1 }, G)).toBe("me@gmail.com");
    expect(s.run(1)).toBeNull(); // one action only
  });
  it("follows a popup opened by the run's tab, nothing else", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    expect(s.chooseFor({ tabId: 7 }, G)).toBeNull();
    expect(s.chooseFor({ tabId: 7, openerTabId: 1 }, G)).toBe("me@gmail.com");
  });
  it("ends after pressing without account or auto choose", () => {
    const { s } = setup();
    start(s, { account: null });
    expect(s.pressed(1)).toBeNull();
    start(s, { autoChoose: false });
    expect(s.pressed(1)).toBeNull();
  });
  it("ends on leaving, on expiry, and cannot choose before pressing", () => {
    const { s, advance } = setup();
    start(s);
    expect(s.chooseFor({ tabId: 1 }, G)).toBeNull(); // still "press"
    s.pressed(1);
    s.topLoad(1, "https://typeform.com"); // site origin: fine
    expect(s.run(1)).not.toBeNull();
    s.topLoad(1, "https://elsewhere.com");
    expect(s.run(1)).toBeNull();
    start(s);
    s.pressed(1);
    advance(SSO_RUN_TTL_MS + 1);
    expect(s.chooseFor({ tabId: 1 }, G)).toBeNull();
  });
});
```

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- sso-state`
Expected: FAIL.

- [ ] **Step 3: Implement**

```ts
// Sign-in-with state in the background: what the user clicked (to offer
// saving it) and which pick is running. Pure: no chrome.*, no secrets, in
// memory only; cleared when the vault locks.
//
// A run moves press → choose → end, performs at most one action on the
// provider's page, only on an origin Rust returned with start_sso, only in
// the run's tab or a popup that tab opened, and lasts SSO_RUN_TTL_MS.

import { SSO_PROVIDERS, type SsoProvider } from "@havenkeys/protocol";

export const PENDING_TTL_MS = 5 * 60_000;
export const SSO_RUN_TTL_MS = 2 * 60_000;

export interface TabRef { tabId: number; openerTabId?: number }
export interface PendingSso { tabId: number; url: string; topUrl?: string; provider: SsoProvider; account: string | null; sawProvider: boolean; popupTabId: number | null; expires: number }
export interface SsoRun { tabId: number; frameId: number; siteOrigin: string; provider: SsoProvider; account: string | null; providerOrigins: string[]; autoChoose: boolean; phase: "press" | "choose"; expires: number }

export function createSsoState(now: () => number) {
  const pendings = new Map<number, PendingSso>(); // by the tab clicked in
  const runs = new Map<number, SsoRun>(); // by tab

  function pending(tabId: number): PendingSso | null {
    const p = pendings.get(tabId);
    if (!p) return null;
    if (p.expires <= now()) {
      pendings.delete(tabId);
      return null;
    }
    return p;
  }

  /** The pending capture this tab belongs to: its own, or its opener's. */
  function pendingOf(tab: TabRef): PendingSso | null {
    return pending(tab.tabId) ?? (tab.openerTabId === undefined ? null : pending(tab.openerTabId));
  }

  const isProviderOrigin = (p: PendingSso, origin: string) => SSO_PROVIDERS[p.provider].origins.includes(origin);

  function run(tabId: number): SsoRun | null {
    const r = runs.get(tabId);
    if (!r) return null;
    if (r.expires <= now()) {
      runs.delete(tabId);
      return null;
    }
    return r;
  }

  return {
    click(tabId: number, url: string, topUrl: string | undefined, provider: SsoProvider): void {
      const p: PendingSso = { tabId, url, provider, account: null, sawProvider: false, popupTabId: null, expires: now() + PENDING_TTL_MS };
      if (topUrl !== undefined) p.topUrl = topUrl;
      pendings.set(tabId, p);
    },
    visit(tab: TabRef, origin: string): void {
      const p = pendingOf(tab);
      if (!p || !isProviderOrigin(p, origin)) return;
      p.sawProvider = true;
      if (tab.tabId !== p.tabId) p.popupTabId = tab.tabId;
    },
    account(tab: TabRef, origin: string, account: string): boolean {
      const p = pendingOf(tab);
      if (!p || !isProviderOrigin(p, origin)) return false;
      p.account = account;
      p.sawProvider = true;
      if (tab.tabId !== p.tabId) p.popupTabId = tab.tabId;
      return true;
    },
    takeReturn(tabId: number, origin: string): PendingSso | null {
      const p = pending(tabId);
      if (!p || !p.sawProvider || isProviderOrigin(p, origin)) return null;
      pendings.delete(tabId);
      return p;
    },
    takeOnTabClosed(tabId: number): PendingSso | null {
      for (const p of pendings.values()) {
        if (p.popupTabId === tabId && p.sawProvider && p.expires > now()) {
          pendings.delete(p.tabId);
          return p;
        }
      }
      return null;
    },
    pending,
    dropPending(tabId: number): void {
      pendings.delete(tabId);
    },
    startRun(r: Omit<SsoRun, "phase" | "expires">): SsoRun {
      const full: SsoRun = { ...r, providerOrigins: [...r.providerOrigins], phase: "press", expires: now() + SSO_RUN_TTL_MS };
      runs.set(r.tabId, full);
      return full;
    },
    run,
    pressed(tabId: number): SsoRun | null {
      const r = run(tabId);
      if (!r || r.phase !== "press") return null;
      if (!r.autoChoose || r.account === null) {
        runs.delete(tabId);
        return null;
      }
      r.phase = "choose";
      return r;
    },
    chooseFor(tab: TabRef, origin: string): string | null {
      const own = run(tab.tabId);
      const r = own ?? (tab.openerTabId === undefined ? null : run(tab.openerTabId));
      if (!r || r.phase !== "choose" || r.account === null || !r.providerOrigins.includes(origin)) return null;
      runs.delete(r.tabId);
      return r.account;
    },
    topLoad(tabId: number, origin: string): void {
      const r = run(tabId);
      if (r && origin !== r.siteOrigin && !r.providerOrigins.includes(origin)) runs.delete(tabId);
    },
    endRun(tabId: number): void {
      runs.delete(tabId);
    },
    forgetTab(tabId: number): void {
      pendings.delete(tabId);
      runs.delete(tabId);
    },
    clear(): void {
      pendings.clear();
      runs.clear();
    },
  };
}

export type SsoState = ReturnType<typeof createSsoState>;
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test -- sso-state && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/background/sso-state.ts apps/extension/src/background/sso-state.test.ts
git commit -m "feat(extension): background state for sign-in-with saves and runs"
```

---

### Task 12: Background handler and wiring

**Files:**
- Create: `apps/extension/src/background/sso-handler.ts`, `apps/extension/src/background/sso-handler.test.ts`
- Modify: `apps/extension/src/background/index.ts`, `apps/extension/src/background/inline-handler.ts` (+ test), `apps/extension/src/background/popup-handler.ts` (+ test), `apps/extension/src/i18n/{en,pt-BR}.ts`

**Interfaces:**
- Consumes: Tasks 5, 10, 11. `FrameRef`, `InlineReply` from `inline-handler.ts` / `messaging/inline.ts`. `displayHost` (`shared/url`). `BridgeError` (`messaging/native`).
- Produces:

```ts
export interface SsoDeps {
  client: Client; // same shape as inline-handler's
  sendToFrame(frame: Pick<FrameRef, "tabId" | "frameId" | "documentId">, msg: BackgroundToSso): Promise<unknown>;
  now(): number;
  newToken(): string;
  suggestionsOn?(): Promise<boolean>;
  siteName?(url: string): string | null; // passkey directory name, as inline-handler uses
}
export function createSsoHandler(deps: SsoDeps): {
  handleContent(frame: FrameRef, tab: TabRef, req: SsoContentRequest): Promise<unknown>;
  handleFrame(tabId: number, req: SsoFrameRequest): Promise<InlineReply<SsoView | null>>;
  ready(frame: FrameRef, tab: TabRef): SsoReady;
  noteUsername(frame: FrameRef, tab: TabRef, username: string): void;
  start(frame: FrameRef, itemId: string): Promise<InlineReply<null>>;
  tabRemoved(tabId: number): void;
  reset(): void;
};
```

- `InlineDeps` gains `startSso?(frame: FrameRef, itemId: string): Promise<InlineReply<null>>`.
- `createPopupHandler(client, activeTab, fillTab, startSso?)` with `startSso?: (tabId: number, pageUrl: string, itemId: string) => Promise<PopupReply<null>>`.

- [ ] **Step 1: Strings**

Extension `en.ts`, new section `sso`:

```ts
  sso: {
    pageTitle: "Sign in with HavenKeys",
    offerTitle: "Sign in",
    row: (provider: string, account: string | null) => (account ? `${provider} · ${account}` : `Sign in with ${provider}`),
    saveQuestion: "Save this login to HavenKeys?",
    updateQuestion: "Add this account to your saved login?",
    signInWith: (provider: string) => `Sign in with ${provider}`,
    accountLabel: "Account",
    accountPlaceholder: "Email (optional)",
    titleLabel: "Name",
    save: "Save",
    update: "Add",
    notNow: "Not now",
    close: "Close",
    noButton: (provider: string) => `Couldn’t find the “Sign in with ${provider}” button on this page.`,
  },
```

Also add `menu.ssoRow: (provider: string, account: string | null) => account ? \`${provider} · ${account}\` : \`Sign in with ${provider}\`` and `popup.signIn: "Sign in"`, `popup.signInTitle: "Sign in with this login’s provider"`.

pt-BR:

```ts
  sso: {
    pageTitle: "Entrar com HavenKeys",
    offerTitle: "Entrar",
    row: (provider: string, account: string | null) => (account ? `${provider} · ${account}` : `Entrar com ${provider}`),
    saveQuestion: "Salvar este login no HavenKeys?",
    updateQuestion: "Adicionar esta conta ao login salvo?",
    signInWith: (provider: string) => `Entrar com ${provider}`,
    accountLabel: "Conta",
    accountPlaceholder: "E-mail (opcional)",
    titleLabel: "Nome",
    save: "Salvar",
    update: "Adicionar",
    notNow: "Agora não",
    close: "Fechar",
    noButton: (provider: string) => `Não encontrei o botão “Entrar com ${provider}” nesta página.`,
  },
```

Also `menu.ssoRow` (`Entrar com …`), `popup.signIn: "Entrar"`, `popup.signInTitle: "Entrar com o provedor deste login"`.

- [ ] **Step 2: Write the failing handler tests**

`sso-handler.test.ts` uses a fake client that records requests and answers from a table, like `inline-handler.test.ts`; copy its `fakeClient` helper. Cases:

```ts
import { describe, expect, it, vi } from "vitest";
import { BridgeError } from "../messaging/native";
import { createSsoHandler } from "./sso-handler";
import type { FrameRef } from "./inline-handler";

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const top: FrameRef = { tabId: 1, frameId: 0, url: "https://typeform.com/login", origin: "https://typeform.com" };
const googleFrame = (tabId = 1): FrameRef => ({ tabId, frameId: 0, url: "https://accounts.google.com/o/oauth2/v2", origin: "https://accounts.google.com" });
const match = { id: ID, title: "Typeform", username: "me@gmail.com", hasTotp: false, strength: "same_site", provider: "google" } as const;

function setup(answers: Record<string, unknown>, opts: { suggestions?: boolean } = {}) {
  const requests: { type: string }[] = [];
  const sent: { frame: unknown; msg: { type: string } }[] = [];
  const client = {
    request: vi.fn(async (r: { type: string }) => {
      requests.push(r);
      const a = answers[r.type];
      if (a instanceof Error) throw a;
      return a;
    }),
  };
  let n = 0;
  const h = createSsoHandler({
    client: client as never,
    sendToFrame: async (frame, msg) => {
      sent.push({ frame, msg });
      return msg.type === "bg_sso_press" ? (answers.press ?? { pressed: true }) : undefined;
    },
    now: () => 1_000,
    newToken: () => (n++).toString(16).padStart(32, "0"),
    suggestionsOn: async () => opts.suggestions ?? true,
  });
  return { h, requests, sent };
}

describe("offer", () => {
  it("offers only this site's items for the providers on the page", async () => {
    const other = { ...match, id: "8c9e6679-7425-40de-944b-e07fc1f90ae7", provider: "github" };
    const plain = { ...match, id: "9c9e6679-7425-40de-944b-e07fc1f90ae7", provider: null };
    const { h } = setup({ find_matches: { matches: [match, other, plain] } });
    const r = (await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })) as { ok: boolean; token: string };
    expect(r.ok).toBe(true);
    const view = await h.handleFrame(1, { type: "sso_state", token: r.token });
    expect(view).toEqual({ ok: true, value: { mode: "offer", site: "typeform.com", rows: [{ id: ID, provider: "google", title: "Typeform", account: "me@gmail.com" }] } });
  });
  it("stays out when suggestions are off, in iframes, or with nothing to offer", async () => {
    expect(await setup({ find_matches: { matches: [match] } }, { suggestions: false }).h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
    expect(await setup({ find_matches: { matches: [match] } }).h.handleContent({ ...top, frameId: 3, topUrl: top.url }, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
    expect(await setup({ find_matches: { matches: [] } }).h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
    expect(await setup({ find_matches: new BridgeError("locked", "x") }).h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
  });
  it("a pick starts a run: start_sso, then press, then choose on the provider", async () => {
    const { h, requests, sent } = setup({
      find_matches: { matches: [match] },
      start_sso: { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    const r = (await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })) as { token: string };
    expect(await h.handleFrame(1, { type: "sso_pick", token: r.token, itemId: ID })).toEqual({ ok: true, value: null });
    expect(requests.map((q) => q.type)).toEqual(["find_matches", "start_sso"]);
    expect(sent.map((s) => s.msg.type)).toContain("bg_sso_press");
    expect(h.ready({ ...googleFrame(), url: "https://evil.com/", origin: "https://evil.com" }, { tabId: 1 })).toBeNull();
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull(); // once
  });
  it("rejects picks the offer did not make", async () => {
    const { h } = setup({ find_matches: { matches: [match] } });
    const r = (await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })) as { token: string };
    const res = await h.handleFrame(1, { type: "sso_pick", token: r.token, itemId: "8c9e6679-7425-40de-944b-e07fc1f90ae7" });
    expect(res.ok).toBe(false);
    expect((await h.handleFrame(2, { type: "sso_pick", token: r.token, itemId: ID })).ok).toBe(false); // another tab
  });
  it("shows a notice when the button is gone", async () => {
    const { h, sent } = setup({
      press: { pressed: false },
      start_sso: { type: "start_sso", provider: "google", account: null, providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    const r = await h.start(top, ID);
    expect(r.ok).toBe(false);
    const show = sent.find((s) => s.msg.type === "bg_sso_show")!.msg as unknown as { token: string };
    expect((await h.handleFrame(1, { type: "sso_state", token: show.token })).ok && true).toBe(true);
  });
});

describe("save", () => {
  it("asks on return, with the learned account, and saves for the clicked site", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null }, save_sso: { type: "save_sso", itemId: ID } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_account", account: "me@gmail.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://admin.typeform.com/", origin: "https://admin.typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    const token = (sent.find((s) => s.msg.type === "bg_sso_show")!.msg as unknown as { token: string }).token;
    const view = await h.handleFrame(1, { type: "sso_state", token });
    expect(view).toEqual({ ok: true, value: { mode: "save", site: "typeform.com", provider: "google", account: "me@gmail.com", title: "typeform.com", action: "add" } });
    expect(await h.handleFrame(1, { type: "sso_save", token, account: " me@gmail.com ", title: "Typeform" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_sso", url: "https://typeform.com/login", provider: "google", account: "me@gmail.com", itemId: null, title: "Typeform" });
  });
  it("says nothing when unchanged", async () => {
    const { h, sent } = setup({ check_sso: { type: "check_sso", action: "unchanged", itemId: null } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 });
    await new Promise((r) => setTimeout(r, 0));
    expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(false);
  });
  it("reset forgets everything", async () => {
    const { h } = setup({});
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.reset();
    h.ready(googleFrame(), { tabId: 1 });
    expect(h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 })).toBeNull();
  });
});
```

- [ ] **Step 3: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- sso-handler`
Expected: FAIL.

- [ ] **Step 4: Implement `sso-handler.ts`**

Structure (complete the bodies exactly as described; every branch appears in the tests above):

```ts
// Background side of "Sign in with": the offer balloon, the run a pick
// starts, and the save prompt after the user signed in with a provider.
//
// Trust model (docs/superpowers/specs/2026-09-28-sign-in-with-design.md §7):
// * Frames come from the browser's sender data. A pick names only an item
//   this tab's offer listed, and start_sso re-checks it against the page.
// * The run's provider origins come from start_sso (Rust). A provider page
//   gets the account to click only in the run's tab or a popup it opened.
// * Save prompts hold no secret. Everything is dropped on lock.

import type { Match, Request, RequestType, ResultFor, SsoProvider } from "@havenkeys/protocol";
import { SSO_PROVIDERS } from "@havenkeys/protocol";
import { t } from "../i18n";
import type { InlineReply } from "../messaging/inline";
import { BridgeError } from "../messaging/native";
import { parsePressReply, type BackgroundToSso, type SsoContentRequest, type SsoFrameRequest, type SsoReady, type SsoView } from "../messaging/sso";
import { displayHost } from "../shared/url";
import type { FrameRef } from "./inline-handler";
import { createSsoState, type PendingSso, type TabRef } from "./sso-state";

type Client = { request<T extends RequestType>(r: Extract<Request, { type: T }>): Promise<ResultFor<T>> };

export const FRAME_TTL_MS = 5 * 60_000;

type Session =
  | { kind: "offer"; token: string; frame: FrameRef; rows: Match[]; expires: number }
  | { kind: "notice"; token: string; frame: FrameRef; provider: SsoProvider; expires: number }
  | { kind: "save"; token: string; pending: PendingSso; action: "add" | "update"; itemId: string | null; title: string | null; expires: number };
```

Behaviour:
- `frameFields(f)`: identical to `inline-handler.ts`.
- `sessions: Map<tabId, Session>`, with `live(tabId, token)` expiring like `liveMenu`. `close(tabId)` deletes the session and sends `bg_sso_close` to `{tabId, frameId: 0}`.
- `handleContent`:
  - `cs_sso_buttons`:
    - `frame.frameId !== 0` → `{ok:false}`;
    - `suggestionsOn` false → `{ok:false}`;
    - the tab has a `save` session → `{ok:false}`;
    - otherwise `find_matches(frameFields(frame))` (any throw → `{ok:false}`), then `rows = matches.filter(m => m.provider !== null && req.providers.includes(m.provider))`. None → `{ok:false}`. Else replace any session with an `offer` and return `{ok:true, token}`.
  - `cs_sso_click`: `state.click(frame.tabId, frame.url, frame.topUrl, req.provider)`. Returns `{}`.
  - `cs_sso_account`: `state.account(tab, frame.origin, req.account)`. Returns `{}`.
  - `cs_sso_stop`: if `state.run(frame.tabId)?.frameId === frame.frameId`, `state.endRun(frame.tabId)`. Returns `{}`.
- `ready(frame, tab)`:
  1. `state.visit(tab, frame.origin)`.
  2. If `frame.frameId === 0`:
     - `state.topLoad(frame.tabId, frame.origin)`;
     - `const back = state.takeReturn(frame.tabId, frame.origin)`; if `back`, `void ask(back)`.
  3. Return `(() => { const a = state.chooseFor(tab, frame.origin); return a === null ? null : { kind: "choose", account: a }; })()`.
- `noteUsername(frame, tab, username)`: if `isAccount(username.trim())`, `state.account(tab, frame.origin, username.trim().toLowerCase())`.
- `tabRemoved(tabId)`: `const back = state.takeOnTabClosed(tabId)`; if `back`, `void ask(back)`. Then `state.forgetTab(tabId)`, and delete the tab's session.
- `ask(p: PendingSso)`:
  - `check_sso({url: p.url, ...(p.topUrl ? {topUrl} : {}), provider: p.provider, account: p.account})`; catch → return.
  - `unchanged` → return.
  - Otherwise create a `save` session with `title = action === "add" ? (deps.siteName?.(p.url) ?? displayHost(p.url) ?? null) : null` and send `bg_sso_show` to `{tabId: p.tabId, frameId: 0}`.
- `start(frame, itemId)`:
  1. `s = start_sso(frameFields + itemId)`; on error → `{ok:false, message: e instanceof BridgeError ? e.message : t.errors.generic}`.
  2. `state.startRun({tabId, frameId, siteOrigin: frame.origin, provider: s.provider, account: s.account, providerOrigins: s.providerOrigins, autoChoose: s.autoChoose})`.
  3. `pressed = parsePressReply(await deps.sendToFrame(frame, {type:"bg_sso_press", provider: s.provider})).pressed`.
  4. If pressed → `state.pressed(tabId)` and return `{ok:true, value:null}`.
  5. Else → `state.endRun(tabId)`; replace the session with a `notice` (top frame, provider) and send `bg_sso_show`; return `{ok:false, message: t.sso.noButton(SSO_PROVIDERS[s.provider].name)}`.
- `handleFrame(tabId, req)`:
  - `sso_state`:
    - offer → `{mode:"offer", site: displayHost(frame.url) ?? "", rows: rows.map(m => ({id, provider: m.provider!, title, account: m.username}))}`;
    - notice → `{mode:"notice", site, provider}`;
    - save → `{mode:"save", site: displayHost(p.url) ?? "", provider, account: p.account, title, action}`.
    - Missing/expired → `{ok:false, message: t.errors.promptExpired}`.
  - `sso_pick`: offer session only, and `rows.some(r => r.id === req.itemId)` else `{ok:false, message: t.errors.unknownItem}`. Then `close(tabId)` and `return start(session.frame, req.itemId)`.
  - `sso_save`: save session only.
    - `account = req.account.trim() || null`, title for `add` = `req.title?.trim() || session.title`.
    - `save_sso({url, topUrl?, provider, account, itemId: session.itemId, ...(add && title ? {title} : {})})`.
    - Errors → `fail(e)`. `finally close(tabId)`.
  - `sso_dismiss`: `close(tabId)` when live.
  - `sso_resize`: send `bg_sso_resize` to top.
- `reset()`: close every session, `state.clear()`.

- [ ] **Step 5: Wire it in `background/index.ts`**

1. Create the handler:
   ```ts
   const sso = createSsoHandler({ client, sendToFrame, now: Date.now, newToken, suggestionsOn: getInlineSuggestions, siteName: (url) => findPasskeySite(url)?.name ?? null });
   ```
   Widen `sendToFrame`'s message type to include `BackgroundToSso`.
2. Pass `startSso: (frame, itemId) => sso.start(frame, itemId)` in the inline handler deps.
3. `onEvent`: on `locked`/`disconnected`, also call `sso.reset()`.
4. `inlineFrame`: accept `"/sso.html"`. In the embedded branch: `if (embedded.page === "/sso.html") { const req = parseSsoFrameRequest(msg); if (!req) {…invalid…} return reply(sso.handleFrame(embedded.tabId, req)); }`.
5. Content branch, before `parseContentRequest`:
   ```ts
       const tab: TabRef = sender.tab?.openerTabId === undefined ? { tabId: frame.tabId } : { tabId: frame.tabId, openerTabId: sender.tab.openerTabId };
       const ssoReq = parseSsoContentRequest(msg);
       if (ssoReq) return reply(sso.handleContent(frame, tab, ssoReq));
   ```
   Then, for `req.type === "cs_ready"`:
   ```ts
       if (req.type === "cs_ready") return reply(inline.handleContent(frame, req).then((r) => ({ ...(r as object), sso: sso.ready(frame, tab) })));
   ```
   For `cs_submit` with `password === null && username !== null`, also call `sso.noteUsername(frame, tab, req.username)` before delegating.
6. `chrome.tabs.onRemoved`: add `sso.tabRemoved(tabId)`.
7. Popup SSO starter:
   ```ts
   async function ssoTab(tabId: number, pageUrl: string, itemId: string) {
     try {
       await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
     } catch {
       return { ok: false as const, message: t.errors.pageNotSupported };
     }
     const r = await sso.start({ tabId, frameId: 0, url: pageUrl, origin: new URL(pageUrl).origin }, itemId);
     return r.ok ? { ok: true as const, value: null } : r;
   }
   const popup = createPopupHandler(client, activeTab, fillTab, ssoTab);
   ```

- [ ] **Step 6: Inline handler and popup delegation**

`inline-handler.ts`:
- `InlineDeps.startSso?(frame: FrameRef, itemId: string): Promise<InlineReply<null>>`.
- In `menu_pick`, after validating the item, add this before the fill:
  ```ts
          if (offered?.provider && m.kind === "login" && deps.startSso) return deps.startSso(m.frame, req.itemId);
  ```
  It goes after `closeMenu(tabId)` and outside the `try`, since `start` never throws.
- `menu_state` items: `{ id: i.id, title: i.title, username: i.username, provider: i.provider }`.

Add a test in `inline-handler.test.ts`: a `find_matches` row with `provider: "google"`, `menu_pick` → `startSso` called with the frame and id, and **no** `fill_item` request.

`popup-handler.ts`, in `fillFromPopup` for `!totp`: first `find_matches` once (reuse the lookup):
```ts
        const { matches } = await client.request({ type: "find_matches", url });
        const m = matches.find((x) => x.id === itemId);
        if (m?.provider && startSso) return startSso(tab.id, url, itemId);
```
Then the existing `fill_item` path. Replace the `hasTotp()` helper with `m?.hasTotp ?? false`, so there is still one lookup. Add a popup test: an SSO match → `startSso` called, no `fill_item`.

- [ ] **Step 7: Run all extension tests**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS. Existing `MenuItemView` fixtures need `provider: null`; that type change lands in Task 15 Step 3, so do it now if typecheck asks.

- [ ] **Step 8: Commit**

```bash
git add apps/extension/src
git commit -m "feat(extension): offer, run and save sign-in-with logins from the background"
```

---

### Task 13: The `sso.html` frame page

**Files:**
- Create: `apps/extension/src/menu/sso.html`, `apps/extension/src/menu/sso.ts`, `apps/extension/src/menu/sso.test.ts`, `apps/extension/src/menu/icons.ts`
- Modify: `apps/extension/src/menu/inline.css`, `apps/extension/build.mjs`, `apps/extension/manifest/base.json`, `tools/ui-check/extension.mjs`

**Interfaces:**
- Consumes: `SsoView`, `SsoFrameRequest`, `SSO_MIN_HEIGHT/MAX` (Task 10), `ask`, `createClickGuard`, `tokenFromHash`, `h`, `userData` (`menu/common.ts`), `PROVIDER_ICONS` (Task 7), `SSO_PROVIDERS`.
- Produces:
  - `menu/icons.ts`: `providerIcon(p: SsoProvider, size?: number): SVGSVGElement`
  - `menu/sso.ts`: `export function render(view: SsoView, token: string): void`, exported for tests like `save.ts`'s `confirmRequest`
  - `export function saveRequest(token: string, account: string, title: string | null): SsoFrameRequest`

- [ ] **Step 1: Write the failing tests**

Read `apps/extension/src/menu/save.test.ts` first and copy its jsdom setup: a fake `chrome.runtime.sendMessage` and loading the HTML into `document`. Then test:
1. The offer view renders one `button.row` per row, with the provider's display name and the account text.
2. Clicking a row before the guard arms sends nothing; after `vi.advanceTimersByTime(500)` a click sends `{type:"sso_pick", token, itemId}`.
3. The save view: the account input holds `account ?? ""`. The title field is hidden for `update`. Confirm sends `saveRequest(token, input.value, titleOrNull)`.
4. The notice view shows `t.sso.noButton("Google")` and a close button that sends `sso_dismiss`.
5. No `innerHTML` anywhere (already covered by `hygiene.test.ts`).

Mock `IntersectionObserver` as `save.test.ts` does, so the guard is time-based.

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- menu/sso`
Expected: FAIL.

- [ ] **Step 3: Implement `icons.ts`**

```ts
// Provider marks as DOM nodes (createElementNS only; path data is ours).
import { PROVIDER_ICONS } from "@havenkeys/ui/provider-icons";
import type { SsoProvider } from "@havenkeys/protocol";

const NS = "http://www.w3.org/2000/svg";

export function providerIcon(p: SsoProvider, size = 18): SVGSVGElement {
  const icon = PROVIDER_ICONS[p];
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", icon.viewBox);
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("provider-icon");
  for (const s of icon.shapes) {
    const el = document.createElementNS(NS, s.rect ? "rect" : "path");
    if (s.rect) {
      const [x, y, w, hgt] = s.rect;
      el.setAttribute("x", String(x));
      el.setAttribute("y", String(y));
      el.setAttribute("width", String(w));
      el.setAttribute("height", String(hgt));
    } else el.setAttribute("d", s.d as string);
    el.setAttribute("fill", s.fill);
    svg.append(el);
  }
  return svg;
}
```

- [ ] **Step 4: Implement `sso.html` and `sso.ts`**

`sso.html` copies `save.html`'s head (fonts, theme, inline css, `<script src="sso.js" defer>`), with title "Sign in with HavenKeys". Body:

```html
    <div class="card save sso" role="dialog" aria-labelledby="heading">
      <header class="head">
        <!-- the same HavenKeys mark svg as save.html -->
        <span class="brand">HavenKeys</span>
        <span id="site" class="site" data-truncate></span>
        <button id="close" class="icon-close" type="button" aria-label="Close">×</button>
      </header>
      <div id="main" class="prompt"><p id="heading">…</p></div>
    </div>
```

`sso.ts`:
- Mirror `save.ts`'s structure: `token = tokenFromHash()`, `guard = createClickGuard(card)`, `applyDocumentLang()`, `document.title = t.sso.pageTitle`, the size reporting block (copy it, with `sso_resize` and `SSO_MIN_HEIGHT/SSO_MAX_HEIGHT`), and `init()` asking `sso_state`.
- `render(view, token)`, per mode:
  - **offer**: `#heading` gets `t.sso.offerTitle`. `#main` gets a `ul.list` of `button.row` elements, each containing `providerIcon(row.provider)` and a `span.who` with a title line (`userData(h("span",{className:"title",text:row.title}))`) and a detail line with `t.sso.row(SSO_PROVIDERS[row.provider].name, row.account)`. Click: `if (!e.isTrusted || !guard.armed()) return;` then `ask({type:"sso_pick", token, itemId: row.id})`. On `!ok`, replace `#main` with an error paragraph.
  - **save**:
    - `#heading` gets `view.action === "add" ? t.sso.saveQuestion : t.sso.updateQuestion`.
    - A provider line: `providerIcon` plus `t.sso.signInWith(name)`.
    - A label with an account input: `maxLength = 254`, `value = view.account ?? ""`, `placeholder = t.sso.accountPlaceholder`, `autocomplete="off"`, `spellcheck=false`.
    - For `add`, a label with a title input: `maxLength = MAX_TITLE_CHARS`, `value = view.title ?? ""`.
    - Buttons Not now (`sso_dismiss`) and Save/Add (`ask(saveRequest(token, account.value, titleInput ? titleInput.value : null))`), with the same guard, `disabled` and error handling as `save.ts`'s `confirm`.
  - **notice**: `#heading` gets `t.sso.noButton(name)`, plus one "Close" button → `sso_dismiss`.
- `#site` gets `view.site`. `#close` → `sso_dismiss`, trusted clicks only. Set `#close`'s `aria-label` to `t.sso.close`.

`inline.css`: `.sso .row` reuses the menu row styles. Copy the `.row`, `.who`, `.title`, `.user` rules the menu uses into `.card.sso` scope if they are scoped under `.menu`. Add `.provider-icon { flex: none; }`, `.icon-close` (a 24px square, transparent background, `color: var(--text-muted)`, `border-radius: 6px`, `:hover` background from the theme tokens the other buttons use), and `.sso .provider { display:flex; gap:8px; align-items:center; }`.

- [ ] **Step 5: Build, manifest, ui-check**

- `build.mjs`: add entry `sso: join(root, "src/menu/sso.ts")` and copy `"menu/sso.html"`.
- `manifest/base.json` `web_accessible_resources.resources`: add `"sso.html"` and `"sso.js"`.
- `tools/ui-check/extension.mjs`: copy the six `save.html` scenarios as `sso-offer`, `sso-offer-long` (long title and account), `sso-save-add`, `sso-save-update`, `sso-notice`, `sso-expired` with `page: "sso.html"` and `frame: { kind: "sso" }`. Add a `sso` frame kind wherever `kind === "save"` is handled (`grep -n '"save"' tools/ui-check/*.mjs`), with the same size as `save`.

- [ ] **Step 6: Run the checks**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck && pnpm build:extension && pnpm ui:check`
Expected: PASS. `ui:check` needs Playwright browsers. If they are missing, run `pnpm exec playwright install chromium` once, and say so in the report.

- [ ] **Step 7: Commit**

```bash
git add apps/extension tools/ui-check
git commit -m "feat(extension): the sign-in-with balloon: offer, save and notice"
```

---

### Task 14: Content script — scan, capture, press, choose

**Files:**
- Create: `apps/extension/src/content/sso.ts`, `apps/extension/src/content/sso.test.ts`
- Modify: `apps/extension/src/content/frames.ts` (`InlineFrame` page union, title, `ssoBox`), `apps/extension/src/content/index.ts`, `apps/extension/src/content/frames.test.ts`

**Interfaces:**
- Consumes: Task 9 recogniser, Task 10 messages, `InlineFrame`, `defaultEnv`, `hasChallenge` (`autofill/submit.ts`), `providersForOrigin`.
- Produces:

```ts
export const SCAN_DEBOUNCE_MS = 750;
export const SCAN_WINDOW_MS = 60_000;
export const CHOOSE_WAIT_MS = 10_000;
export function ssoBox(viewport: { width: number; height?: number }, height?: number): Box; // frames.ts, like saveBox, SSO_HEIGHT = 150
export function createSsoContent(deps: {
  send(msg: SsoContentRequest): Promise<unknown>;
  viewport(): { width: number; height: number };
  suggestions(): boolean;
  isTop: boolean;
}): {
  watchPage(): void;
  onTrustedClick(target: Element): void;
  onTrustedInput(): void;
  handleBackground(m: BackgroundToSso): SsoPressReply | undefined;
  onReady(sso: SsoReady): void;
  teardown(): void;
};
```

- [ ] **Step 1: Write the failing tests**

`content/sso.test.ts` (jsdom). Stub `chrome.runtime.getURL` for `InlineFrame`, and `vi.useFakeTimers()`. Cases:
1. `watchPage()` on a page with `<button>Continue with Google</button>`: after `SCAN_DEBOUNCE_MS`, `send` got `{type:"cs_sso_buttons", providers:["google"]}`. The reply `{ok:true, token}` puts an iframe with `src` containing `sso.html#<token>` into the DOM.
2. The same providers are not re-sent on mutations. Adding a GitHub button later sends `["github","google"]`, sorted.
3. `suggestions()` false → nothing is sent.
4. `onTrustedClick(googleButton)` → sends `cs_sso_click {provider:"google"}`. A plain "Sign in" button sends nothing.
5. On a provider origin, `onTrustedClick(rowWithEmail)` → sends `cs_sso_account`. On other origins it does not. jsdom's `location.origin` is fixed by the test URL: use `// @vitest-environment-options {"url": "https://accounts.google.com/"}` in a second test file `sso-provider.test.ts` for the provider-origin cases.
6. `handleBackground({type:"bg_sso_press", provider:"google"})` clicks the button (spy on `click`) and returns `{pressed:true}`. With no button, or a visible challenge, it returns `{pressed:false}`. After pressing, `onTrustedInput()` sends `cs_sso_stop` once.
7. In `sso-provider.test.ts`, `onReady({kind:"choose", account:"me@gmail.com"})`:
   - the row appears 1 s later (added by the test) and gets clicked;
   - with a consent button present it never clicks;
   - after `CHOOSE_WAIT_MS` with no row it stops (the observer is disconnected; adding a row later does not click);
   - `onTrustedInput()` before the row appears cancels.

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- content/sso`
Expected: FAIL.

- [ ] **Step 3: Implement `frames.ts` additions**

- Page union: `"menu.html" | "save.html" | "passkey.html" | "sso.html"`.
- Title: `page === "sso.html" ? t.sso.pageTitle`.
- Box:

```ts
export const SSO_HEIGHT = 150;

/** Top right, like the save prompt. */
export function ssoBox(viewport: { width: number; height?: number }, height = SSO_HEIGHT): Box {
  return saveBox(viewport, height);
}
```

Add a `frames.test.ts` case: `ssoBox({ width: 1000 })` equals `saveBox({ width: 1000 }, SSO_HEIGHT)`.

- [ ] **Step 4: Implement `content/sso.ts`**

```ts
// "Sign in with" in the page: notice the site's provider buttons (to offer a
// saved sign-in), notice the user's own clicks on them (to offer saving),
// press the button after the user picked, and click the saved account on
// the provider's chooser when the background says so.
//
// Bounded work: one scan per debounce, at most MAX_SSO_CANDIDATES elements,
// observation for SCAN_WINDOW_MS after load or a URL change. Page strings
// are only compared to fixed lists; nothing is logged or stored.

import { providersForOrigin } from "@havenkeys/protocol";
import { defaultEnv } from "../autofill/group";
import { chooserRow, emailIn, findProviderButtons, isConsentScreen, providerButton, providerOf, SSO_CANDIDATES, SSO_MIN_SCORE } from "../autofill/sso";
import { hasChallenge } from "../autofill/submit";
import { TOKEN } from "../messaging/inline";
import type { BackgroundToSso, SsoContentRequest, SsoPressReply, SsoReady } from "../messaging/sso";
import { InlineFrame, ssoBox } from "./frames";

export const SCAN_DEBOUNCE_MS = 750;
export const SCAN_WINDOW_MS = 60_000;
export const CHOOSE_WAIT_MS = 10_000;
const CHOOSE_DEBOUNCE_MS = 300;

export function createSsoContent(deps: {
  send(msg: SsoContentRequest): Promise<unknown>;
  viewport(): { width: number; height: number };
  suggestions(): boolean;
  isTop: boolean;
}) {
  let frame: InlineFrame | null = null;
  let dismissedHref: string | null = null;
  let lastSent = "";
  let href = location.href;
  let scanUntil = 0;
  let scanTimer: ReturnType<typeof setTimeout> | null = null;
  let observer: MutationObserver | null = null;
  let pressed = false;
  let chooseStop: (() => void) | null = null;

  function closeFrame(): void {
    frame?.remove();
    frame = null;
  }

  function show(token: string): void {
    if (!deps.isTop) return;
    closeFrame();
    frame = new InlineFrame("sso.html", token, ssoBox(deps.viewport()), () => {
      if (frame?.token === token) {
        closeFrame();
        dismissedHref = location.href; // the page removed it: do not offer again here
      }
    });
  }

  async function scan(): Promise<void> {
    scanTimer = null;
    if (!deps.isTop || !deps.suggestions() || frame || dismissedHref === location.href) return;
    const providers = [...findProviderButtons(document, defaultEnv()).keys()].sort();
    const key = providers.join(",");
    if (key === "" || key === lastSent) return;
    lastSent = key;
    const reply = (await deps.send({ type: "cs_sso_buttons", providers })) as { ok?: unknown; token?: unknown } | undefined;
    if (reply?.ok === true && typeof reply.token === "string" && TOKEN.test(reply.token)) show(reply.token);
  }

  function schedule(): void {
    if (location.href !== href) {
      href = location.href;
      lastSent = "";
      scanUntil = Date.now() + SCAN_WINDOW_MS;
    }
    if (Date.now() > scanUntil) {
      observer?.disconnect();
      observer = null;
      return;
    }
    if (scanTimer === null) scanTimer = setTimeout(() => void scan(), SCAN_DEBOUNCE_MS);
  }

  function watchPage(): void {
    if (!deps.isTop) return;
    scanUntil = Date.now() + SCAN_WINDOW_MS;
    schedule();
    observer = new MutationObserver(schedule);
    observer.observe(document.documentElement, { childList: true, subtree: true });
    window.addEventListener("popstate", () => {
      if (!observer) watchPage();
      else schedule();
    });
  }

  function cancelChoose(): void {
    chooseStop?.();
    chooseStop = null;
  }

  function choose(account: string): void {
    cancelChoose();
    const env = defaultEnv;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const attempt = () => {
      timer = null;
      if (isConsentScreen(document, env())) return cancelChoose();
      const row = chooserRow(document, account, env());
      if (!row) return;
      cancelChoose();
      row.click();
    };
    const mo = new MutationObserver(() => {
      if (timer === null) timer = setTimeout(attempt, CHOOSE_DEBOUNCE_MS);
    });
    mo.observe(document.documentElement, { childList: true, subtree: true });
    const giveUp = setTimeout(cancelChoose, CHOOSE_WAIT_MS);
    chooseStop = () => {
      mo.disconnect();
      clearTimeout(giveUp);
      if (timer !== null) clearTimeout(timer);
    };
    attempt();
  }

  return {
    watchPage,
    onTrustedClick(target: Element): void {
      const button = target.closest(SSO_CANDIDATES);
      const hit = button ? providerOf(button, true) : null;
      if (hit && hit.score >= SSO_MIN_SCORE) void deps.send({ type: "cs_sso_click", provider: hit.provider });
      if (providersForOrigin(location.origin).length > 0) {
        const row = target.closest(SSO_CANDIDATES) ?? target;
        const email = emailIn(row);
        if (email) void deps.send({ type: "cs_sso_account", account: email });
      }
    },
    onTrustedInput(): void {
      cancelChoose();
      if (pressed) {
        pressed = false;
        void deps.send({ type: "cs_sso_stop" });
      }
    },
    handleBackground(m: BackgroundToSso): SsoPressReply | undefined {
      switch (m.type) {
        case "bg_sso_show":
          show(m.token);
          return undefined;
        case "bg_sso_close":
          if (frame?.token === m.token) {
            closeFrame();
            dismissedHref = location.href;
          }
          return undefined;
        case "bg_sso_resize":
          if (frame?.token === m.token) frame.place(ssoBox(deps.viewport(), m.height));
          return undefined;
        case "bg_sso_press": {
          const env = defaultEnv();
          const button = providerButton(document, m.provider, env);
          if (!button || hasChallenge(document, env)) return { pressed: false };
          pressed = true;
          button.click();
          return { pressed: true };
        }
      }
    },
    onReady(sso: SsoReady): void {
      if (sso?.kind === "choose" && providersForOrigin(location.origin).length > 0) choose(sso.account);
    },
    teardown(): void {
      closeFrame();
      cancelChoose();
      observer?.disconnect();
      observer = null;
    },
  };
}
```

`providerOf(button, true)` passes `context = true` for a click: the user chose the element, so a bare "Google" counts.

- [ ] **Step 5: Wire it into `content/index.ts`**

1. In `start()`:
   ```ts
   const sso = createSsoContent({ send: (m) => chrome.runtime.sendMessage(m).catch(() => undefined), viewport, suggestions: () => suggestions, isTop: window.top === window });
   ```
   `viewport` is already defined inside `start`.
2. `pointerdown`, `keydown` and `input` listeners, after their `isTrusted` checks: `sso.onTrustedInput();`.
3. `click` listener, after `if (!e.isTrusted) return;` and the icon check:
   ```ts
      const target = e.composedPath()[0];
      if (target instanceof Element) sso.onTrustedClick(target);
   ```
   Put this before the submit-like logic, which returns early.
4. `pagehide`: `sso.teardown();`.
5. Background listener: before `parseBackgroundMessage`,
   ```ts
    const s = parseSsoBackgroundMessage(raw);
    if (s) {
      const r = sso.handleBackground(s);
      if (r) sendResponse(r);
      return false;
    }
   ```
6. `cs_ready` reply handling: `sso.onReady(parseSsoReady((reply as { sso?: unknown } | undefined)?.sso));`.
7. `applySuggestions(on)`: when turning on, call `sso.watchPage()` the first time only (keep a `watching` boolean); when off, `sso.teardown()`.

Update the header comment's Rules list: "Sign in with: provider buttons are looked for (bounded, see content/sso.ts) only while suggestions are on; the balloon they allow is an offer, and nothing is pressed without the user's pick."

- [ ] **Step 6: Run the checks**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS, including the existing `content.test.ts`. Its fake `sendMessage` returns `{ saveToken: null }` for `cs_ready`, which gives `sso` undefined and therefore null. That path must not throw.

- [ ] **Step 7: Commit**

```bash
git add apps/extension/src/content
git commit -m "feat(extension): content script offers, presses and chooses for sign-in-with"
```

---

### Task 15: Field menu and popup show sign-in-with rows

**Files:**
- Modify: `apps/extension/src/messaging/inline.ts` (`MenuItemView.provider`), `apps/extension/src/menu/menu.ts` (`itemRow`), `apps/extension/src/menu/menu.test.ts`, `apps/extension/src/popup/popup.ts` (`matchRow`), `apps/extension/src/popup/popup.css`, `apps/extension/src/menu/inline.css`

**Interfaces:**
- Consumes: `providerIcon` (Task 13), `menu.ssoRow`, `popup.signIn`, `popup.signInTitle` strings (Task 12).
- Produces: `MenuItemView { id; title; username; provider: SsoProvider | null }`.

- [ ] **Step 1: Write the failing test**

In `menu.test.ts`, render a `ready` view with an item `{ id, title: "Typeform", username: "me@gmail.com", provider: "google" }`. Expect:
- the row's detail text is `t.menu.ssoRow("Google", "me@gmail.com")`;
- the row contains an `svg.provider-icon` instead of the monogram;
- the pick still sends `{type:"menu_pick", token, itemId}`.

- [ ] **Step 2: Run and confirm failure**

Run: `pnpm --filter @havenkeys/extension test -- menu/menu`
Expected: FAIL.

- [ ] **Step 3: Implement**

`inline.ts`: `export interface MenuItemView { id: string; title: string; username: string | null; provider: SsoProvider | null }` (import the type from `@havenkeys/protocol`).

`menu.ts` `itemRow`, for `kind === "login"` and `item.provider`:

```ts
  if (kind === "login" && item.provider) {
    const name = SSO_PROVIDERS[item.provider].name;
    return row(providerIcon(item.provider), item.title, msg.menu.ssoRow(name, item.username), () => pick({ type: "menu_pick", token: t, itemId: item.id }), { title: false, detail: item.username === null });
  }
```

`row()` takes the monogram string as its first argument today. Widen it to `string | Node`: when a Node, append it into the avatar span instead of setting text. Check `row()`'s body near line 40 and adapt.

`popup.ts` `matchRow`:
- When `m.provider`, the `who` block's user line becomes `t.menu.ssoRow`'s equivalent. Add `popup.ssoUser = (provider, account) => …` if popup strings are separate from menu strings; otherwise reuse the menu function through `t`.
- The action button text becomes `t.popup.signIn` with title `t.popup.signInTitle`. It still sends `popup_fill`: the background routes SSO items in Task 12.
- Put `providerIcon(m.provider, 16)` inside the edit avatar next to the initial, or replace the initial; keep the avatar's click behaviour.
- CSS: `.provider-icon { flex: none; }` in both stylesheets.

- [ ] **Step 4: Run the checks**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck && pnpm build:extension && pnpm ui:check`
Expected: PASS. Add one `menu.html` ui-check scenario with an SSO row, `menu-sso-${width}`, copying an existing menu scenario's `menuView()` with `provider: "google"`. Add one popup scenario if the popup is covered there.

- [ ] **Step 5: Commit**

```bash
git add apps/extension tools/ui-check
git commit -m "feat(extension): field menu and popup show sign-in-with logins"
```

---

### Task 16: Docs, amendment, security review, full verification

**Files:**
- Modify: `docs/autofill.md`, `docs/native-messaging.md`, `docs/security-model.md`, `docs/threat-model.md`, `docs/security-review.md`, `CLAUDE.md`

- [ ] **Step 1: Documentation**

- `docs/native-messaging.md`: in the request and result tables, add `start_sso`, `check_sso`, `save_sso` with their exact fields (from Task 3) and `Match.provider`. Rate class: all three are "secret". Per-item limiter: `save_sso` with `itemId`.
- `docs/autofill.md`: a new section "Sign in with Google, Microsoft, GitHub, Apple" covering:
  - recognition signals and bounds (`MAX_SSO_CANDIDATES`, 60 s observation window, 750 ms debounce);
  - when the balloon appears;
  - the run (press → choose, one action, 2 minutes, provider origins from Rust, popup via `openerTabId`);
  - stop conditions (consent, ambiguity, password, user input, lock);
  - save detection (trusted click, account only on provider origins, prompt on return or on popup close, never before the provider was reached);
  - **known limitation:** Google Identity Services iframe buttons and One Tap are not recognised or pressed.
- `docs/security-model.md`:
  - the cross-origin step is limited to the Rust list and to one click on an exact, unique email match;
  - consent is never granted;
  - page-sourced account text is untrusted and validated in Rust;
  - no new permissions.
- `docs/threat-model.md`: add attacks and mitigations:
  - a fake provider button (only navigates the page; the run acts only on real provider origins);
  - a hostile page triggering the balloon (it shows only that page's own items; pressing needs the user's pick through a guarded extension frame);
  - a provider look-alike origin (exact origin list);
  - a chooser spoof inside a provider page (not in scope: a compromised provider origin is outside the model).
- `CLAUDE.md` §25: after the auto-sign-in amendment note, add:

```markdown
> Amended on 2026-09-28 by
> `docs/superpowers/specs/2026-09-28-sign-in-with-design.md`: after the user
> picks a "Sign in with Google/Microsoft/GitHub/Apple" login, HavenKeys may
> press that site's provider button and, on the provider's own origin (from
> a fixed list in Rust), click the saved account in its chooser. It never
> fills a password there and never presses a consent screen.
```

- [ ] **Step 2: Full verification**

Run each and read the output:

```bash
cargo test
pnpm lint:rust
pnpm typecheck
pnpm test
pnpm build:extension
pnpm ui:check
pnpm audit && cargo audit && cargo deny check
```

Expected: all pass. For audit findings unrelated to this change, record them in the report without fixing.

Then grep for leaks:
- `grep -rn "console\.\|dbg!\|println!" apps/extension/src crates/havenkeys-core/src | grep -i sso` → no output.
- Confirm no new `innerHTML` (`hygiene.test.ts` passes).
- Confirm no new manifest permission (`git diff main -- apps/extension/manifest` shows only `web_accessible_resources`).

- [ ] **Step 3: Security review entry**

In `docs/security-review.md`, add a dated "2026-09-28 — Sign in with" section. For each of the following, give severity, component, attack scenario, mitigation and remaining limitation:

1. **A trusted click on a page element that merely *says* "Sign in with Google" arms a save prompt.**
   - Severity: low.
   - Only a prompt: nothing is saved without confirmation, and the prompt names the site.
2. **The account text comes from the provider page.**
   - Severity: low.
   - It is validated in Rust and editable.
3. **Consent detection is keyword-based.**
   - Severity: medium.
   - Failure mode: an unrecognised consent screen that also shows an email row. Mitigation: a click needs a unique, clickable row whose email equals the saved account, and consent keywords stop the run.
   - Limitation: provider page redesigns can change behaviour. Documented.
4. **`openerTabId` trust.**
   - Severity: low.
   - The browser supplies it, not the page. A popup the site opened to a provider origin is exactly the flow; the run still needs the provider origin.
5. **GIS iframe not supported.**
   - Severity: info, a functional limitation.

- [ ] **Step 4: Commit**

```bash
git add docs CLAUDE.md
git commit -m "docs: sign in with Google, Microsoft, GitHub and Apple — protocol, autofill, security"
```

- [ ] **Step 5: Manual check in a real browser (report results; do not claim success without it)**

Load `apps/extension/dist/chrome` unpacked and run the desktop app with a test vault:

1. On a site whose login page has "Continue with Google" (for example `https://www.notion.so/login` or `https://admin.typeform.com/login`), click it and sign in with a Google account. Back on the site, the save balloon shows the account. Save it.
2. Sign out and reopen the login page. The top-right balloon offers "Google · account". Pick it: the button is pressed, and the Google chooser row is clicked.
3. A site that opens Google in a popup: the balloon appears after the popup closes.
4. Desktop: the item shows "Sign in with Google · account", and the editor round-trips.
5. Re-import a `.1pux` with SSO logins: the report says "N logins … now sign in with …".

Write down which sites were tested and anything that did not work.
