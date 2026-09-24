# Passkey Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** HavenKeys answers a site's automatic passkey upgrade (`create({mediation:"conditional"})`) right after a HavenKeys password fill, hints at the site's passkey help page for sites in the 2factorauth Passkeys Directory, and leads the field menu with passkeys when the user has one.

**Architecture:** The Rust core remembers recent password fills per unlocked session and alone decides whether a conditional create is `auto`, `ask` or `none`; `passkey_create {conditional:true}` is refused unless the core recomputes `auto` for that exact login. A new Lookup request `passkey_status` tells the extension whether a passkey exists for the page. The extension forwards conditional creates, shows a 4-second "saved" notice (or the save card titled "Add a passkey?"), and adds hint rows to the field menu from a committed directory snapshot.

**Tech Stack:** Rust (havenkeys-core, havenkeys-protocol, havenkeys-bridge), TypeScript (packages/protocol, apps/extension with esbuild + vitest/jsdom), React desktop (apps/desktop).

**Spec:** `docs/superpowers/specs/2026-09-24-passkey-upgrade-design.md`. Background: `docs/superpowers/specs/2026-09-23-passkeys-design.md`.

## Global Constraints

- `UPGRADE_WINDOW_MS = 5 * 60_000`; at most 16 recent fills; never persisted; dropped on lock.
- Vault setting `auto_passkey_upgrade: bool`, `#[serde(default = ...)]` **true**; desktop toggle text "Add passkeys automatically after I sign in", note "When off, HavenKeys asks first."
- Notice text: "Passkey saved to HavenKeys" with the site, and "Manage it in the HavenKeys app"; shown for 4 seconds.
- Ask card title: "Add a passkey?", filled login preselected.
- Menu hint rows: "<name> supports passkeys — how to add one" (last row, click opens `help` via `chrome.tabs.create`, trusted click only); "You have a passkey for <site> — use the site's 'Sign in with a passkey' option" (first row, no click action).
- All new protocol fields: `deny_unknown_fields` (Rust) and exact-key parsing (TS). No response carries a key or secret.
- `passkey_status` is Lookup-class (rate limiter).
- Directory data: snapshot committed at `apps/extension/src/data/passkey-sites.json`, never fetched at runtime; attribution "Passkeys Directory by 2factorauth", CC-BY-4.0, in `THIRD-PARTY-NOTICES.md` and `docs/autofill.md`. Only `https:` help links; directory text is shown via `textContent` only.
- No new extension permissions. No browser storage in the extension.
- Never log usernames, item titles, URLs or secrets. Match the surrounding comment density and idiom.
- Do not add Co-Authored-By trailers to commits (user rule).
- Vault format changes need no migration (single-owner vault), but old settings blobs must still parse (serde default).

**Deviation from the spec, decided while planning:** the Directory's public API (`passkeys-api.2fa.directory/v1/*.json`) is keyed by domain and carries **no site names** and no grouping of additional domains. The update script therefore reads the Directory's source repository (`github.com/2factorauth/passkeys`, `entries/*/*.json`, same CC-BY-4.0 data), where each file is `{ "<Name>": { "additional-domains"?: [...], "passwordless"?: "allowed", "mfa"?: "allowed", "documentation"?: url } }` and the file name is the primary domain. Record this in `docs/autofill.md`.

## Review Focus

1. **Clock skew in the fill memory**: a fill whose timestamp is after `now` (clock moved back) must not count as recent; `0 <= now - filled_at <= window`. Test in Task 2.
2. **Username folding**: site sends `"  Octo "`, login username `"octo"` → match; login with empty/absent username → match any name. Test in Task 2.
3. **Subdomain vs. look-alike sites**: fill on `https://github.com/login`, conditional create from `https://gist.github.com/` → same site (if the login is offered there); fill on `github.com`, create from `github.com.evil.com` → `none`. Test in Task 2.
4. **Directory host normalization**: page host with a trailing dot (`https://github.com./`) and the longest-domain winner when two entries match (`accounts.nintendo.com` vs `nintendo.com`); `github.com.evil.com` / `evilgithub.com` never match. Test in Task 7.
5. **Page abort during a silent save**: the page aborts or navigates while `passkey_create` runs → no notice frame, no hang, the page's own abort wins. Test in Task 6.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-core/src/model.rs` | `Settings.auto_passkey_upgrade` |
| `crates/havenkeys-core/src/origin.rs` | `site_of(&PageUrl)` |
| `crates/havenkeys-core/src/vault.rs` | `Session.recent_fills`, `fill_for_page` records fills |
| `crates/havenkeys-core/src/passkey/vault.rs` | `Upgrade`, `CreateQuery`, upgrade decision, conditional create check, `has_passkey_for_page` |
| `crates/havenkeys-core/tests/passkeys.rs`, `tests/vault.rs`, `tests/security.rs` | core tests |
| `crates/havenkeys-protocol/src/message.rs`, `tests/messages.rs` | wire types |
| `crates/havenkeys-bridge/src/{dispatch,server,ratelimit}.rs`, `tests/bridge.rs` | mapping, rate class, attack tests |
| `packages/protocol/src/index.ts`, `index.test.ts`, `fuzz.test.ts` | TS wire types |
| `apps/extension/src/webauthn/{page,messages,bridge}.ts` | conditional create forwarding, `saved` reply, notice frame |
| `apps/extension/src/background/webauthn-handler.ts` | upgrade flow (auto / ask / fallback), notices |
| `apps/extension/src/menu/passkey.ts` | "Add a passkey?" and "saved" views |
| `apps/extension/src/content/frames.ts` | `noticeBox` |
| `scripts/update-passkey-directory.mjs` | builds the snapshot |
| `apps/extension/src/data/passkey-sites.json` | snapshot |
| `apps/extension/src/background/passkey-sites.ts` | validate + match directory |
| `apps/extension/src/background/inline-handler.ts`, `messaging/inline.ts`, `menu/menu.ts`, `background/index.ts` | menu hints |
| `apps/desktop/src/lib/types.ts`, `views/SettingsView.tsx` | setting toggle |
| docs | threat model, autofill, native messaging, security model, spec amendment |

Test commands used throughout:
- Rust: `cargo test -p havenkeys-core`, `cargo test -p havenkeys-protocol`, `cargo test -p havenkeys-bridge`; lint `pnpm lint:rust`.
- TS: `pnpm --filter @havenkeys/protocol test`, `pnpm --filter @havenkeys/extension test`, `pnpm --filter @havenkeys/desktop test`; types `pnpm typecheck`.

---

### Task 1: Setting `auto_passkey_upgrade` (core + desktop)

**Files:**
- Modify: `crates/havenkeys-core/src/model.rs` (Settings ~236-274, tests ~440-470)
- Modify: `crates/havenkeys-core/tests/vault.rs` (`settings_are_encrypted_and_persisted` neighbourhood)
- Modify: `apps/desktop/src/lib/types.ts:88-93`, `apps/desktop/src/views/SettingsView.tsx:220-240`

**Interfaces:**
- Produces: `Settings { ..., pub auto_passkey_upgrade: bool }` (camelCase `autoPasskeyUpgrade` on the wire to the desktop UI), default `true`, and `true` when absent from an old blob.

- [ ] **Step 1: Write failing tests**

In `model.rs` tests, next to the existing "Settings blobs written before the theme field existed" assertions, add:

```rust
    #[test]
    fn auto_passkey_upgrade_defaults_on() {
        assert!(Settings::default().auto_passkey_upgrade);
        // Settings saved before the field existed.
        let old: Settings = serde_json::from_str(
            r#"{"autoLockMinutes":5,"clipboardClearSeconds":30,"theme":"dark","browserIntegration":true}"#,
        )
        .unwrap();
        assert!(old.auto_passkey_upgrade);
        let off: Settings = serde_json::from_str(
            r#"{"autoLockMinutes":5,"clipboardClearSeconds":30,"autoPasskeyUpgrade":false}"#,
        )
        .unwrap();
        assert!(!off.auto_passkey_upgrade);
    }
```

In `tests/vault.rs` add:

```rust
#[test]
fn auto_passkey_upgrade_setting_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.db");
    let sk = {
        let (mut v, sk) = activated_vault_at(&path);
        assert!(v.settings().unwrap().auto_passkey_upgrade);
        v.update_settings(Settings {
            auto_passkey_upgrade: false,
            ..v.settings().unwrap()
        })
        .unwrap();
        sk
    };
    let mut v = open_file(&path);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    assert!(!v.settings().unwrap().auto_passkey_upgrade);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-core auto_passkey_upgrade`
Expected: compile error, no field `auto_passkey_upgrade`.

- [ ] **Step 3: Implement**

In `model.rs`:

```rust
    /// Whether a site's automatic passkey upgrade (a conditional
    /// `create()` right after HavenKeys filled a password there) may save a
    /// passkey without asking. On by default, including for settings saved
    /// before this field existed; when off, the save card asks instead.
    #[serde(default = "default_true")]
    pub auto_passkey_upgrade: bool,
}

fn default_true() -> bool {
    true
}
```

and `auto_passkey_upgrade: true,` in `impl Default for Settings`.

Desktop `types.ts`: add `autoPasskeyUpgrade: boolean;` to `Settings`.

`SettingsView.tsx`, inside the "Browser extension" `.group`, after the existing row:

```tsx
            <div className="row">
              <span className="row-label-inline">Add passkeys automatically after I sign in</span>
              <Switch
                label="Add passkeys automatically after I sign in"
                checked={settings.autoPasskeyUpgrade}
                onChange={(checked) => void update({ autoPasskeyUpgrade: checked })}
              />
            </div>
```

and after the existing `<p className="group-note">…</p>` of that block add:

```tsx
          <p className="group-note">
            When a website offers to add a passkey right after HavenKeys fills your password there, HavenKeys saves it to
            that login. When off, HavenKeys asks first.
          </p>
```

- [ ] **Step 4: Verify**

Run: `cargo test -p havenkeys-core` then `pnpm --filter @havenkeys/desktop typecheck && pnpm --filter @havenkeys/desktop test`
Expected: all PASS. Fix any other `Settings { .. }` literal that fails to compile by adding `..Settings::default()` only if needed (existing literals already use it).

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/model.rs crates/havenkeys-core/tests/vault.rs apps/desktop/src/lib/types.ts apps/desktop/src/views/SettingsView.tsx
git commit -m "feat(core,desktop): auto_passkey_upgrade setting, on by default"
```

---

### Task 2: Recent fills and the upgrade decision (core)

**Files:**
- Modify: `crates/havenkeys-core/src/origin.rs` (add `site_of`)
- Modify: `crates/havenkeys-core/src/vault.rs` (`Session` ~57, its two constructors ~618 and ~718, `fill_for_page` ~1081)
- Modify: `crates/havenkeys-core/src/passkey/vault.rs`, `crates/havenkeys-core/src/passkey/mod.rs` (re-exports)
- Modify: `crates/havenkeys-core/tests/passkeys.rs`, `crates/havenkeys-core/tests/security.rs` (callers)
- Modify: `crates/havenkeys-bridge/src/dispatch.rs` (only so the workspace compiles: pass `now_ms(unix_seconds)`, `conditional: false`; the wire change is Task 4)

**Interfaces:**
- Consumes: `Settings.auto_passkey_upgrade` (Task 1).
- Produces:
  - `pub(crate) fn site_of(page: &PageUrl) -> Option<String>` in `origin.rs`.
  - `pub const UPGRADE_WINDOW_MS: i64 = 5 * 60_000;` and `pub const MAX_RECENT_FILLS: usize = 16;` in `vault.rs`, re-exported from `havenkeys_core::vault`.
  - `VaultService::fill_for_page(&mut self, id: &Uuid, page_url: &str, top_url: Option<&str>, now_ms: i64) -> Result<FillCredentials>`.
  - `havenkeys_core::passkey::Upgrade` — `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Upgrade { None, Ask(Uuid), Auto(Uuid) }`.
  - `havenkeys_core::passkey::CreateQuery<'a> { pub rp_id: &'a str, pub page_url: &'a str, pub top_url: Option<&'a str>, pub user_name: &'a str, pub exclude: &'a [Vec<u8>], pub conditional: bool }`.
  - `VaultService::check_passkey_create(&self, q: &CreateQuery<'_>, now_ms: i64) -> Result<CreateCheck>`; `CreateCheck` gains `pub upgrade: Upgrade` (`Upgrade::None` when `!conditional` or `excluded`).
  - `PasskeyCreate` gains `pub conditional: bool`.

- [ ] **Step 1: Write failing tests** (append to `tests/passkeys.rs`)

Update `create_req` to set `conditional: false`, and add helpers + tests:

```rust
use havenkeys_core::model::Settings;
use havenkeys_core::passkey::{CreateQuery, Upgrade};
use havenkeys_core::vault::UPGRADE_WINDOW_MS;

/// A GitHub login with a password, committed.
fn github_login(v: &mut VaultService, rev: &mut Rev, user: &str) -> Uuid {
    let s = v
        .stage_create(login("GitHub", user, "gh-pw", "https://github.com"), NOW)
        .unwrap();
    v.commit_write(s, rev.next()).unwrap().unwrap().id
}

fn query<'a>(page: &'a str, user: &'a str) -> CreateQuery<'a> {
    CreateQuery {
        rp_id: "github.com",
        page_url: page,
        top_url: None,
        user_name: user,
        exclude: &[],
        conditional: true,
    }
}

fn upgrade(v: &VaultService, page: &str, user: &str, now: i64) -> Upgrade {
    v.check_passkey_create(&query(page, user), now).unwrap().upgrade
}

fn set_auto(v: &mut VaultService, on: bool) {
    let s = v.settings().unwrap();
    v.update_settings(Settings { auto_passkey_upgrade: on, ..s }).unwrap();
}

#[test]
fn upgrade_is_auto_after_a_recent_fill_and_ask_when_the_setting_is_off() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW + 1000), Upgrade::Auto(gh));
    // Not conditional: never an upgrade.
    let q = CreateQuery { conditional: false, ..query(GH, "octo") };
    assert_eq!(v.check_passkey_create(&q, NOW).unwrap().upgrade, Upgrade::None);
    set_auto(&mut v, false);
    assert_eq!(upgrade(&v, GH, "octo", NOW + 1000), Upgrade::Ask(gh));
}

#[test]
fn upgrade_window_is_five_minutes_and_ignores_future_fills() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW + UPGRADE_WINDOW_MS), Upgrade::Auto(gh));
    assert_eq!(upgrade(&v, GH, "octo", NOW + UPGRADE_WINDOW_MS + 1), Upgrade::None);
    // Clock moved back: a fill "from the future" is not recent.
    assert_eq!(upgrade(&v, GH, "octo", NOW - 1), Upgrade::None);
}

#[test]
fn upgrade_needs_the_same_account_name_folded_or_a_login_without_one() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "  OCTO ", NOW), Upgrade::Auto(gh));
    assert_eq!(upgrade(&v, GH, "someone-else", NOW), Upgrade::None);

    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let mut input = login("GitHub", "x", "pw", "https://github.com");
    input.username = None;
    let s = v.stage_create(input, NOW).unwrap();
    let anon = v.commit_write(s, rev.next()).unwrap().unwrap().id;
    v.fill_for_page(&anon, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "anyone", NOW), Upgrade::Auto(anon));
}

#[test]
fn upgrade_is_per_site_and_never_for_look_alikes() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    // Same registrable domain, login offered there (domain rule): allowed.
    let q = CreateQuery { rp_id: "gist.github.com", ..query("https://gist.github.com/", "octo") };
    assert_eq!(v.check_passkey_create(&q, NOW).unwrap().upgrade, Upgrade::Auto(gh));
    // Look-alike: authorize_rp itself refuses.
    let evil = CreateQuery { rp_id: "github.com.evil.com", ..query("https://github.com.evil.com/", "octo") };
    assert!(!matches!(v.check_passkey_create(&evil, NOW), Ok(c) if c.upgrade != Upgrade::None));
}

#[test]
fn a_fill_without_a_password_is_not_remembered() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let mut input = login("GitHub", "octo", "x", "https://github.com");
    input.password = havenkeys_core::model::SecretUpdate::Keep;
    let s = v.stage_create(input, NOW).unwrap();
    let id = v.commit_write(s, rev.next()).unwrap().unwrap().id;
    v.fill_for_page(&id, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
}

#[test]
fn fill_memory_is_dropped_on_lock() {
    let (mut v, sk) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    v.lock();
    v.unlock_for_account(&secret(PASSWORD), &sk, &account()).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
}

#[test]
fn a_full_login_gets_no_upgrade() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    for i in 0..MAX_PASSKEYS_PER_LOGIN {
        let handle = [i as u8 + 10];
        let s = v.stage_passkey_create(create_req("octo", &handle, Some(gh)), NOW).unwrap();
        commit(&mut v, &mut rev, s);
    }
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    assert_eq!(upgrade(&v, GH, "octo", NOW), Upgrade::None);
}

#[test]
fn conditional_create_needs_auto_for_exactly_that_login() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    let other = github_login(&mut v, &mut rev, "work");
    let cond = |item| PasskeyCreate { conditional: true, ..create_req("octo", &[1], item) };
    // No fill yet.
    assert_eq!(v.stage_passkey_create(cond(Some(gh)), NOW).err(), Some(Error::Denied));
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    // Another login, no login, too late.
    assert_eq!(v.stage_passkey_create(cond(Some(other)), NOW).err(), Some(Error::Denied));
    assert_eq!(v.stage_passkey_create(cond(None), NOW).err(), Some(Error::Denied));
    assert_eq!(
        v.stage_passkey_create(cond(Some(gh)), NOW + UPGRADE_WINDOW_MS + 1).err(),
        Some(Error::Denied)
    );
    // Setting off: the extension must show the card (a non-conditional create).
    set_auto(&mut v, false);
    assert_eq!(v.stage_passkey_create(cond(Some(gh)), NOW).err(), Some(Error::Denied));
    set_auto(&mut v, true);
    let s = v.stage_passkey_create(cond(Some(gh)), NOW).unwrap();
    assert_eq!(s.item_id, gh);
}

#[test]
fn conditional_create_never_lands_in_another_login_that_holds_the_account() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    let gh = github_login(&mut v, &mut rev, "octo");
    let other = github_login(&mut v, &mut rev, "octo2");
    // `other` already holds the passkey for user handle [1].
    let s = v.stage_passkey_create(create_req("octo", &[1], Some(other)), NOW).unwrap();
    commit(&mut v, &mut rev, s);
    v.fill_for_page(&gh, GH, None, NOW).unwrap();
    let cond = PasskeyCreate { conditional: true, ..create_req("octo", &[1], Some(gh)) };
    assert_eq!(v.stage_passkey_create(cond, NOW).err(), Some(Error::Denied));
}
```

(Adjust helper imports — `login`, `secret`, `PASSWORD`, `account` come from `common`; `Error` is already imported. If `ItemInput.username`/`password` field names differ, read `common::login` and the `ItemInput` struct in `model.rs`.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-core --test passkeys`
Expected: compile errors (`CreateQuery`, `Upgrade`, `UPGRADE_WINDOW_MS`, new `fill_for_page` arity).

- [ ] **Step 3: Implement**

`origin.rs`, below `registrable_domain_of`:

```rust
/// The page's site: its registrable domain, or its host when it has none
/// (an IP address, an unknown suffix).
pub(crate) fn site_of(page: &PageUrl) -> Option<String> {
    let host = host_key(page.url())?;
    Some(registrable_domain_of(&host).unwrap_or(host))
}
```

`vault.rs`:

```rust
/// How long a password fill counts as consent for a site's automatic
/// passkey upgrade (see `passkey::Upgrade`).
pub const UPGRADE_WINDOW_MS: i64 = 5 * 60_000;
/// Recent fills kept per session.
pub const MAX_RECENT_FILLS: usize = 16;

/// A password HavenKeys filled: which login, on which site, when. In memory
/// only, inside the session, so it is gone when the vault locks.
pub(crate) struct RecentFill {
    pub(crate) item_id: Uuid,
    pub(crate) site: String,
    pub(crate) filled_at_ms: i64,
}

impl RecentFill {
    /// Within the window, and not from the future (a clock set back).
    pub(crate) fn is_recent(&self, now_ms: i64) -> bool {
        (0..=UPGRADE_WINDOW_MS).contains(&now_ms.saturating_sub(self.filled_at_ms))
    }
}
```

Add `pub(crate) recent_fills: Vec<RecentFill>,` to `Session` and `recent_fills: Vec::new(),` in both places a `Session` is built. Replace `fill_for_page`:

```rust
    /// Username and password for filling the page. Denied unless the item's
    /// own website rules match it. A returned password is remembered for
    /// `UPGRADE_WINDOW_MS` as consent for the site's passkey upgrade.
    pub fn fill_for_page(
        &mut self,
        id: &Uuid,
        page_url: &str,
        top_url: Option<&str>,
        now_ms: i64,
    ) -> Result<FillCredentials> {
        let username = self
            .authorize_for_page(id, page_url, top_url)?
            .username
            .clone();
        let password = match self.load_details(id)? {
            ItemDetails::Login { password, .. } => password,
            ItemDetails::SecureNote { .. } => return Err(Error::Denied),
        };
        if password.is_some() {
            self.record_fill(*id, page_url, now_ms)?;
        }
        Ok(FillCredentials { username, password })
    }

    fn record_fill(&mut self, item_id: Uuid, page_url: &str, now_ms: i64) -> Result<()> {
        let Some(site) = PageUrl::parse(page_url).as_ref().and_then(site_of) else {
            return Ok(());
        };
        let fills = &mut self.session_mut()?.recent_fills;
        fills.retain(|f| f.is_recent(now_ms) && !(f.item_id == item_id && f.site == site));
        fills.push(RecentFill { item_id, site, filled_at_ms: now_ms });
        let excess = fills.len().saturating_sub(MAX_RECENT_FILLS);
        fills.drain(..excess);
        Ok(())
    }
```

(import `site_of` from `crate::origin`.)

`passkey/vault.rs` — add types, split out the candidate list, and compute the decision:

```rust
/// A site's automatic passkey upgrade (`create()` with conditional
/// mediation), decided here and nowhere else. `Auto`: save to this login
/// without asking; `Ask`: offer the save card with it preselected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    None,
    Ask(Uuid),
    Auto(Uuid),
}

/// A site's `create()` request, as far as the checks before saving need it.
pub struct CreateQuery<'a> {
    pub rp_id: &'a str,
    pub page_url: &'a str,
    pub top_url: Option<&'a str>,
    pub user_name: &'a str,
    pub exclude: &'a [Vec<u8>],
    /// `mediation: "conditional"`: the site's automatic upgrade.
    pub conditional: bool,
}
```

`CreateCheck` gains `pub upgrade: Upgrade,`; `PasskeyCreate` gains

```rust
    /// The site's automatic upgrade, saved without a click in HavenKeys UI.
    /// Refused unless the upgrade decision is `Auto(item_id)`.
    pub conditional: bool,
```

In `impl VaultService`:

```rust
    /// Logins saved for the page that can take another passkey, in
    /// `find_matches` order.
    fn passkey_homes(&self, page_url: &str, top_url: Option<&str>) -> Result<Vec<Suggestion>> {
        let mut out = Vec::new();
        for s in self.find_matches(page_url, top_url)? {
            let full = match self.load_details(&s.id) {
                Ok(ItemDetails::Login { passkeys, .. }) => passkeys.len() >= MAX_PASSKEYS_PER_LOGIN,
                _ => true,
            };
            if !full {
                out.push(s);
            }
        }
        Ok(out)
    }

    /// The newest recent password fill on the page's site whose login is
    /// among `homes` and has the site's account name (folded) or none.
    /// `Auto` or `Ask` by the vault setting.
    fn upgrade_for(
        &self,
        page_url: &str,
        user_name: &str,
        homes: &[Suggestion],
        now_ms: i64,
    ) -> Result<Upgrade> {
        let session = self.session()?;
        let Some(site) = PageUrl::parse(page_url).as_ref().and_then(site_of) else {
            return Ok(Upgrade::None);
        };
        let wanted = fold(user_name);
        let same_account = |s: &Suggestion| {
            s.username
                .as_deref()
                .map(fold)
                .filter(|u| !u.is_empty())
                .is_none_or(|u| u == wanted)
        };
        let found = session
            .recent_fills
            .iter()
            .rev()
            .filter(|f| f.site == site && f.is_recent(now_ms))
            .find_map(|f| homes.iter().find(|s| s.id == f.item_id && same_account(s)))
            .map(|s| s.id);
        Ok(match found {
            None => Upgrade::None,
            Some(id) if session.settings.auto_passkey_upgrade => Upgrade::Auto(id),
            Some(id) => Upgrade::Ask(id),
        })
    }
```

Rewrite `check_passkey_create`:

```rust
    /// Before asking the user: is one of the site's `excludeCredentials`
    /// already here, which logins could hold the new passkey, and, for a
    /// conditional request, is this the site's automatic upgrade after a
    /// HavenKeys password fill?
    pub fn check_passkey_create(&self, q: &CreateQuery<'_>, now_ms: i64) -> Result<CreateCheck> {
        self.session()?;
        authorize_rp(q.rp_id, q.page_url, q.top_url)?;
        let excluded = !q.exclude.is_empty()
            && !self
                .find_passkeys(q.rp_id, q.page_url, q.top_url, q.exclude)?
                .is_empty();
        let wanted = fold(q.user_name);
        let mut candidates = self.passkey_homes(q.page_url, q.top_url)?;
        // Stable: same username first, otherwise find_matches' order.
        candidates.sort_by_key(|s| s.username.as_deref().map(fold) != Some(wanted.clone()));
        let upgrade = if q.conditional && !excluded {
            self.upgrade_for(q.page_url, q.user_name, &candidates, now_ms)?
        } else {
            Upgrade::None
        };
        Ok(CreateCheck { excluded, candidates, upgrade })
    }
```

In `stage_passkey_create`, right after `self.session()?;` and the challenge check:

```rust
        // The site's automatic upgrade: the consent is a HavenKeys password
        // fill of this very login on this site within the window, checked
        // here, never taken from the extension.
        if req.conditional {
            let item = req.item_id.ok_or(Error::Denied)?;
            let homes = self.passkey_homes(req.page_url, req.top_url)?;
            if self.upgrade_for(req.page_url, req.user_name, &homes, now_ms)? != Upgrade::Auto(item) {
                return Err(Error::Denied);
            }
        }
```

and after `let holder = self.find_passkey_holder(...)?;`:

```rust
        // An upgrade adds to the login that was filled, never elsewhere.
        if req.conditional && holder.is_some_and(|h| Some(h) != req.item_id) {
            return Err(Error::Denied);
        }
```

`passkey/mod.rs`: `pub use vault::{CreateCheck, CreateQuery, PasskeyCreate, PasskeyInfo, PasskeyMatch, StagedPasskey, Upgrade};`. `vault.rs` must make `UPGRADE_WINDOW_MS` public (it is, at module level in `crate::vault`).

Update existing callers:
- `tests/passkeys.rs`: every `check_passkey_create("github.com", GH, None, "octo", &[...])` → `check_passkey_create(&CreateQuery { rp_id: "github.com", page_url: GH, top_url: None, user_name: "octo", exclude: &[...], conditional: false }, NOW)` (a small local helper `plain(user, exclude)` is fine).
- `tests/security.rs`: `v.fill_for_page(&id, page, None)` → `v.fill_for_page(&id, page, None, NOW)`; make `v` `mut` where needed.
- `havenkeys-bridge/src/dispatch.rs`: `fill_for_page(item_id, url, top_url.as_deref(), now_ms(unix_seconds))`; `check_passkey_create(&CreateQuery { rp_id, page_url: url, top_url: top_url.as_deref(), user_name, exclude: &exclude, conditional: false }, now_ms(unix_seconds))`; `PasskeyCreate { ..., conditional: false }`. (Task 4 wires the real fields.)
- grep for any other caller: `grep -rn "fill_for_page\|check_passkey_create\|PasskeyCreate {" crates apps --include='*.rs'`.

- [ ] **Step 4: Verify**

Run: `cargo test -p havenkeys-core && cargo test -p havenkeys-bridge && pnpm lint:rust`
Expected: all PASS, no clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core crates/havenkeys-bridge/src/dispatch.rs
git commit -m "feat(core): remember recent password fills and decide the passkey upgrade in Rust"
```

---

### Task 3: `has_passkey_for_page` (core)

**Files:**
- Modify: `crates/havenkeys-core/src/passkey/vault.rs`
- Test: `crates/havenkeys-core/tests/passkeys.rs`

**Interfaces:**
- Produces: `VaultService::has_passkey_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<bool>`; `Err(Locked)` when locked.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn passkey_status_sees_only_passkeys_the_page_may_use() {
    let (mut v, _) = activated_vault();
    let mut rev = Rev(0);
    assert!(!v.has_passkey_for_page(GH, None).unwrap());
    let s = v.stage_passkey_create(create_req("octo", &[1], None), NOW).unwrap();
    commit(&mut v, &mut rev, s);
    assert!(v.has_passkey_for_page(GH, None).unwrap());
    assert!(v.has_passkey_for_page("https://gist.github.com/", None).unwrap());
    assert!(!v.has_passkey_for_page("https://gitlab.com/", None).unwrap());
    assert!(!v.has_passkey_for_page("https://github.com.evil.com/", None).unwrap());
    assert!(!v.has_passkey_for_page("http://github.com/", None).unwrap());
    // A github.com frame inside another site.
    assert!(!v.has_passkey_for_page(GH, Some("https://evil.com/")).unwrap());
    v.lock();
    assert_eq!(v.has_passkey_for_page(GH, None).err(), Some(Error::Locked));
}
```

- [ ] **Step 2: Run** `cargo test -p havenkeys-core --test passkeys passkey_status` — expect compile failure.

- [ ] **Step 3: Implement** in `passkey/vault.rs`:

```rust
    /// Does the vault hold any passkey this page may use (its rpId passes
    /// `authorize_rp` for the page)? Nothing else about it is returned. A
    /// login whose details do not open is skipped.
    pub fn has_passkey_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<bool> {
        let session = self.session()?;
        let holders: Vec<Uuid> = session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login && o.has_passkey)
            .map(|o| o.id)
            .collect();
        for id in holders {
            let Ok(ItemDetails::Login { passkeys, .. }) = self.load_details(&id) else {
                continue;
            };
            if passkeys
                .iter()
                .any(|p| authorize_rp(&p.rp_id, page_url, top_url).is_ok())
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
```

- [ ] **Step 4: Verify** `cargo test -p havenkeys-core && pnpm lint:rust` — PASS.

- [ ] **Step 5: Commit** `git commit -am "feat(core): has_passkey_for_page for the field menu"`

---

### Task 4: Wire protocol and bridge (Rust)

**Files:**
- Modify: `crates/havenkeys-protocol/src/message.rs`, `crates/havenkeys-protocol/tests/messages.rs`
- Modify: `crates/havenkeys-bridge/src/dispatch.rs`, `server.rs` (`handle` class match), `ratelimit.rs` (doc comment)
- Test: `crates/havenkeys-bridge/tests/bridge.rs`
- Modify (if they enumerate request kinds): `crates/havenkeys-native-host` tests, `crates/havenkeys-core/tests/fuzz.rs` — grep for `check_passkey_create` / `"passkey_create"` across `crates/` and update every JSON fixture to include `"conditional":false`.

**Interfaces:**
- Consumes: Task 2/3 core API.
- Produces (wire, camelCase):
  - `check_passkey_create` request: `+ conditional: bool` (required). Result: `+ upgrade: {"kind":"none"} | {"kind":"ask","itemId":uuid} | {"kind":"auto","itemId":uuid}`.
  - `passkey_create` request: `+ conditional: bool` (required).
  - `passkey_status` request `{ url, topUrl? }` → result `{ type: "passkey_status", hasPasskey: bool }`.
  - Rust: `pub enum UpgradeHint { None {}, Ask { item_id: Uuid }, Auto { item_id: Uuid } }` in `havenkeys_protocol` (re-exported like `PasskeyCandidate`).

- [ ] **Step 1: Failing tests**

`tests/messages.rs` (follow the file's existing style for accepted/rejected fixtures):

```rust
#[test]
fn passkey_upgrade_fields_parse_exactly() {
    let ok = [
        r#"{"v":1,"id":1,"request":{"type":"check_passkey_create","url":"https://github.com/","rpId":"github.com","userName":"octo","excludeCredentials":[],"conditional":true}}"#,
        r#"{"v":1,"id":2,"request":{"type":"passkey_status","url":"https://github.com/"}}"#,
        r#"{"v":1,"id":3,"request":{"type":"passkey_status","url":"https://github.com/","topUrl":"https://github.com/"}}"#,
    ];
    for m in ok {
        assert!(parse_request(m.as_bytes()).is_ok(), "{m}");
    }
    let bad = [
        // `conditional` is required.
        r#"{"v":1,"id":1,"request":{"type":"check_passkey_create","url":"https://github.com/","rpId":"github.com","userName":"octo","excludeCredentials":[]}}"#,
        r#"{"v":1,"id":1,"request":{"type":"check_passkey_create","url":"https://github.com/","rpId":"github.com","userName":"octo","excludeCredentials":[],"conditional":"yes"}}"#,
        r#"{"v":1,"id":2,"request":{"type":"passkey_status","url":"https://github.com/","rpId":"github.com"}}"#,
        r#"{"v":1,"id":2,"request":{"type":"passkey_status","url":""}}"#,
    ];
    for m in bad {
        assert!(parse_request(m.as_bytes()).is_err(), "{m}");
    }
}

#[test]
fn passkey_upgrade_results_validate() {
    let good = [
        format!(r#"{{"v":1,"id":1,"result":{{"type":"check_passkey_create","excluded":false,"candidates":[],"upgrade":{{"kind":"auto","itemId":"{ITEM}"}}}}}}"#),
        r#"{"v":1,"id":1,"result":{"type":"check_passkey_create","excluded":true,"candidates":[],"upgrade":{"kind":"none"}}}"#.to_owned(),
        r#"{"v":1,"id":1,"result":{"type":"passkey_status","hasPasskey":true}}"#.to_owned(),
    ];
    for m in &good {
        assert!(Outgoing::parse(m.as_bytes()).is_some(), "{m}");
    }
    let bad = [
        // Excluded wins: no upgrade then.
        format!(r#"{{"v":1,"id":1,"result":{{"type":"check_passkey_create","excluded":true,"candidates":[],"upgrade":{{"kind":"ask","itemId":"{ITEM}"}}}}}}"#),
        r#"{"v":1,"id":1,"result":{"type":"check_passkey_create","excluded":false,"candidates":[]}}"#.to_owned(),
        r#"{"v":1,"id":1,"result":{"type":"check_passkey_create","excluded":false,"candidates":[],"upgrade":{"kind":"auto"}}}"#.to_owned(),
        r#"{"v":1,"id":1,"result":{"type":"passkey_status","hasPasskey":true,"rpId":"x"}}"#.to_owned(),
    ];
    for m in &bad {
        assert!(Outgoing::parse(m.as_bytes()).is_none(), "{m}");
    }
}
```

(`ITEM` is the UUID constant the file already uses; add `"conditional":false` to every existing `check_passkey_create`/`passkey_create` fixture and `"upgrade":{"kind":"none"}` to existing `check_passkey_create` result fixtures.)

`tests/bridge.rs` — using the fixture (GitHub login "octo" on `https://github.com`, bank login on `https://bank.example`). Read how existing passkey tests send frames (`passkey_attacks_through_the_bridge` near line 641) and add:

```rust
fn ask(f: &Fixture, body: String) -> serde_json::Value {
    let out = f.bridge.handle_frame(format!(r#"{{"v":1,"id":1,"request":{body}}}"#).as_bytes());
    serde_json::to_value(match out { Outgoing::Response(r) => r, _ => panic!() }).unwrap()
}

#[test]
fn passkey_upgrade_through_the_bridge() {
    let f = build_fixture(Some(()));
    let check = |url: &str, rp: &str| {
        ask(&f, format!(r#"{{"type":"check_passkey_create","url":"{url}","rpId":"{rp}","userName":"octo","excludeCredentials":[],"conditional":true}}"#))
    };
    let create = |url: &str, rp: &str, item: Uuid| {
        ask(&f, format!(r#"{{"type":"passkey_create","url":"{url}","rpId":"{rp}","challenge":"AQ","userHandle":"AQ","userName":"octo","displayName":null,"itemId":"{item}","conditional":true}}"#))
    };
    // No fill yet: nothing, and a silent create is refused.
    assert_eq!(check("https://github.com/", "github.com")["result"]["upgrade"]["kind"], "none");
    assert_eq!(create("https://github.com/", "github.com", f.github)["error"]["code"], "denied");
    // A fill of the GitHub login.
    let fill = ask(&f, format!(r#"{{"type":"fill_item","itemId":"{}","url":"https://github.com/login"}}"#, f.github));
    assert!(fill["result"].is_object());
    assert_eq!(check("https://github.com/", "github.com")["result"]["upgrade"]["kind"], "auto");
    // A1: another site gets nothing and cannot create.
    assert!(check("https://evil.com/", "github.com")["error"].is_object());
    assert_eq!(create("https://evil.com/", "evil.com", f.github)["error"]["code"], "denied");
    // A2: another item.
    assert_eq!(create("https://github.com/", "github.com", f.bank)["error"]["code"], "denied");
    // Allowed.
    assert!(create("https://github.com/", "github.com", f.github)["result"].is_object());
    // A3: locked.
    f.vault.lock().unwrap().lock();
    assert_eq!(check("https://github.com/", "github.com")["error"]["code"], "locked");
}

#[test]
fn passkey_status_through_the_bridge() {
    let f = build_fixture(Some(()));
    let status = |url: &str| ask(&f, format!(r#"{{"type":"passkey_status","url":"{url}"}}"#));
    assert_eq!(status("https://github.com/")["result"]["hasPasskey"], false);
    f.vault.lock().unwrap().lock();
    assert_eq!(status("https://github.com/")["error"]["code"], "locked");
}
```

(If `build_fixture(Some(()))` wires a writer that accepts writes, the create succeeds; if not, use the variant the existing passkey-create bridge test uses. Adapt the `ask` helper to however existing tests read responses.)

- [ ] **Step 2: Run** `cargo test -p havenkeys-protocol && cargo test -p havenkeys-bridge` — expect failures.

- [ ] **Step 3: Implement**

`message.rs` requests: add `conditional: bool,` as the last field of `CheckPasskeyCreate` and of `PasskeyCreate` with doc comments (`/// mediation: "conditional" (the site's automatic upgrade).`), and

```rust
    /// Does HavenKeys hold a passkey `url` may use? A yes/no only.
    PasskeyStatus {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
```

Add `"passkey_status"` to `kind()`, and to the `urls()` arm list. Results:

```rust
    CheckPasskeyCreate {
        excluded: bool,
        candidates: Vec<PasskeyCandidate>,
        upgrade: UpgradeHint,
    },
    PasskeyStatus {
        has_passkey: bool,
    },
```

with the `Debug` kind names, and in `Response::is_valid`:

```rust
            Some(ResultBody::CheckPasskeyCreate {
                excluded,
                candidates,
                upgrade,
            }) => {
                candidates.len() <= MAX_MATCHES
                    && !(*excluded && !candidates.is_empty())
                    && !(*excluded && *upgrade != UpgradeHint::None {})
            }
```

and the type:

```rust
/// The core's decision on a site's automatic passkey upgrade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UpgradeHint {
    None {},
    Ask { item_id: Uuid },
    Auto { item_id: Uuid },
}
```

Export it from `lib.rs` with the other message types.

`dispatch.rs`:

```rust
        Request::CheckPasskeyCreate { url, top_url, rp_id, user_name, exclude_credentials, conditional } => {
            require_enabled(v)?;
            let exclude = byte_list(exclude_credentials)?;
            let check = v
                .check_passkey_create(
                    &CreateQuery {
                        rp_id,
                        page_url: url,
                        top_url: top_url.as_deref(),
                        user_name,
                        exclude: &exclude,
                        conditional: *conditional,
                    },
                    now_ms(unix_seconds),
                )
                .map_err(code)?;
            let candidates = /* unchanged */;
            let upgrade = match check.upgrade {
                Upgrade::None => UpgradeHint::None {},
                Upgrade::Ask(item_id) => UpgradeHint::Ask { item_id },
                Upgrade::Auto(item_id) => UpgradeHint::Auto { item_id },
            };
            Ok(Dispatched::Done(ResultBody::CheckPasskeyCreate { excluded: check.excluded, candidates, upgrade }))
        }
        Request::PasskeyStatus { url, top_url } => {
            require_enabled(v)?;
            let has_passkey = v.has_passkey_for_page(url, top_url.as_deref()).map_err(code)?;
            Ok(Dispatched::Done(ResultBody::PasskeyStatus { has_passkey }))
        }
```

and pass `conditional: *conditional` into `PasskeyCreate`. Update the module doc list (`has_passkey_for_page`). `server.rs`: add `| Request::PasskeyStatus { .. }` to the Lookup arm. `ratelimit.rs`: mention `passkey_status` in the Lookup doc.

- [ ] **Step 4: Verify** `cargo test --workspace && pnpm lint:rust` — PASS.

- [ ] **Step 5: Commit** `git commit -am "feat(protocol,bridge): upgrade decision, conditional create and passkey_status on the wire"`

---

### Task 5: TypeScript protocol

**Files:**
- Modify: `packages/protocol/src/index.ts`, `index.test.ts`, `fuzz.test.ts` (if it lists request/result shapes)

**Interfaces:**
- Produces:
  ```ts
  export type UpgradeHint = { kind: "none" } | { kind: "ask"; itemId: string } | { kind: "auto"; itemId: string };
  // Request additions
  | { type: "check_passkey_create"; url; topUrl?; rpId; userName; excludeCredentials: string[]; conditional: boolean }
  | { type: "passkey_create"; ...; itemId: string | null; conditional: boolean }
  | { type: "passkey_status"; url: string; topUrl?: string }
  // Result additions
  | { type: "check_passkey_create"; excluded: boolean; candidates: PasskeyCandidate[]; upgrade: UpgradeHint }
  | { type: "passkey_status"; hasPasskey: boolean }
  ```

- [ ] **Step 1: Failing tests** in `index.test.ts`'s "passkey results" block — update the two existing `check_passkey_create` fixtures to carry `upgrade: { kind: "none" }`, and add:

```ts
  it("parses the upgrade decision and passkey_status exactly", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "auto", itemId: ID } } },
      { v: 1, id: 2, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "ask", itemId: ID } } },
      { v: 1, id: 3, result: { type: "passkey_status", hasPasskey: false } },
    ];
    for (const m of ok) expect(parseIncoming(m)).not.toBeNull();
    const bad = [
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [] } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "auto", itemId: ID } } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "auto" } } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "none", itemId: ID } } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "maybe" } } },
      { v: 1, id: 1, result: { type: "passkey_status", hasPasskey: "yes" } },
      { v: 1, id: 1, result: { type: "passkey_status", hasPasskey: true, rpId: "x" } },
    ];
    for (const m of bad) expect(parseIncoming(m)).toBeNull();
  });
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/protocol test` — FAIL.

- [ ] **Step 3: Implement** in `index.ts`: the types above, and

```ts
function parseUpgrade(v: unknown): UpgradeHint | null {
  if (!isObj(v)) return null;
  if (v.kind === "none") return hasExactKeys(v, ["kind"]) ? { kind: "none" } : null;
  if (v.kind === "ask" || v.kind === "auto") {
    return hasExactKeys(v, ["kind", "itemId"]) && isUuid(v.itemId) ? { kind: v.kind, itemId: v.itemId } : null;
  }
  return null;
}
```

```ts
    case "check_passkey_create": {
      if (!hasExactKeys(v, ["type", "excluded", "candidates", "upgrade"]) || !isBool(v.excluded)) return null;
      const candidates = parseList(v.candidates, parseCandidate);
      const upgrade = parseUpgrade(v.upgrade);
      if (!candidates || !upgrade || (v.excluded && (candidates.length > 0 || upgrade.kind !== "none"))) return null;
      return { type: "check_passkey_create", excluded: v.excluded, candidates, upgrade };
    }
    case "passkey_status":
      if (!hasExactKeys(v, ["type", "hasPasskey"]) || !isBool(v.hasPasskey)) return null;
      return { type: "passkey_status", hasPasskey: v.hasPasskey };
```

- [ ] **Step 4: Verify** `pnpm --filter @havenkeys/protocol test && pnpm --filter @havenkeys/protocol typecheck`. The extension will not typecheck until Task 6 adds `conditional` to its requests; that is expected here — do not "fix" it by making fields optional.

- [ ] **Step 5: Commit** `git commit -am "feat(protocol): TS types for the passkey upgrade and passkey_status"`

---

### Task 6: Extension — conditional create, auto save with notice, ask card

**Files:**
- Modify: `apps/extension/src/webauthn/messages.ts`, `page.ts`, `bridge.ts`
- Modify: `apps/extension/src/background/webauthn-handler.ts`
- Modify: `apps/extension/src/menu/passkey.ts`, `apps/extension/src/content/frames.ts`, `apps/extension/src/menu/inline.css` (only if the notice needs a style)
- Tests: `webauthn/messages.test.ts`, `webauthn/page.test.ts`, `webauthn/bridge.test.ts`, `background/webauthn-handler.test.ts`, `menu/passkey.test.ts`

**Interfaces:**
- Consumes: Task 5 protocol types.
- Produces:
  - `CreateOptions.conditional: boolean` (exact-key parsed).
  - `export const NOTICE_MS = 4_000;` in `messages.ts`.
  - `WaReply` gains `{ ok: true; token: string; ui: "saved"; credential: CreatedCredential }` — the passkey was already saved; the bridge answers the page and shows the notice for `token`.
  - `PkView` create gains `upgradeItemId: string | null`; new `{ state: "saved"; site: string }`.
  - `noticeBox(viewport: { width: number }): Box` in `frames.ts` (`NOTICE_HEIGHT = 112`, width `PASSKEY_WIDTH`).

Behavior (background, `beginCreate` when `options.conditional`):
1. `check_passkey_create {..., conditional: true}`; any error → `FALLBACK`.
2. `excluded` or `upgrade.kind === "none"` → `FALLBACK`.
3. `ask` → open a normal create session with `upgradeItemId = upgrade.itemId`, reply `ui: "create"`. The card's save sends `passkey_create {conditional: false}` (a click happened).
4. `auto` → `passkey_create {..., itemId: upgrade.itemId, conditional: true}`; error → `FALLBACK`. On success: `token = newToken()`, remember `notices.set(tabId, { token, site: displayHost(frame.url), timer })` for `NOTICE_MS + 10_000`, reply `{ ok: true, token, ui: "saved", credential }`. No session is opened.
5. `handleFrame` with no live session: `pk_state` whose token equals the tab's notice → `{ ok: true, value: { state: "saved", site } }`. `forgetTab` drops the notice.
6. Non-conditional creates: every `passkey_create`/`check_passkey_create` now sends `conditional: false`; otherwise unchanged. Conditional + locked → `FALLBACK` (no locked card).

Bridge: on a `saved` reply for a still-pending request, `respond(req.id, { outcome: "credential", credential })`, then show `new InlineFrame("passkey.html", token, noticeBox(viewport()), ...)` removed after `NOTICE_MS` (or at once if the page touches it). If the page cancelled meanwhile (`pending.get(req.id) !== entry`), show nothing.

- [ ] **Step 1: Failing tests**

`webauthn-handler.test.ts` — extend `createOpts` with `conditional: false`, make `defaults` return `upgrade: { kind: "none" }` for `check_passkey_create`, and add:

```ts
const cond: CreateOptions = { ...createOpts, conditional: true };
const withUpgrade = (upgrade: unknown) => (r: Request) =>
  r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: false, candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }], upgrade } : defaults(r);

describe("automatic passkey upgrade", () => {
  it("saves silently on auto and returns the credential with a notice token", async () => {
    const { h, requests } = setup(withUpgrade({ kind: "auto", itemId: ITEM }));
    const reply = await h.handleContent(frame(), { type: "wa_create", options: cond });
    expect(reply).toMatchObject({ ok: true, token: T1, ui: "saved", credential: { type: "create", credentialId: CRED } });
    expect(requests[0]).toMatchObject({ type: "check_passkey_create", conditional: true });
    expect(requests[1]).toMatchObject({ type: "passkey_create", itemId: ITEM, conditional: true, url: "https://github.com/login" });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "saved", site: "github.com" } });
    expect((await h.handleFrame(2, { type: "pk_state", token: T1 })).ok).toBe(false);
  });

  it("shows the Add a passkey card on ask, and the click saves non-conditionally", async () => {
    const { h, requests } = setup(withUpgrade({ kind: "ask", itemId: ITEM }));
    expect(await h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual({ ok: true, token: T1, ui: "create" });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toMatchObject({ ok: true, value: { state: "create", upgradeItemId: ITEM } });
    expect(requests.some((r) => r.type === "passkey_create")).toBe(false);
    await h.handleFrame(1, { type: "pk_save", token: T1, itemId: ITEM });
    expect(requests.at(-1)).toMatchObject({ type: "passkey_create", itemId: ITEM, conditional: false });
  });

  it("falls back on none, excluded, locked, offline and failed saves", async () => {
    const FB = { ok: false, outcome: { outcome: "fallback" } };
    expect(await setup(withUpgrade({ kind: "none" })).h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    const excluded = setup((r) => (r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "none" } } : defaults(r)));
    expect(await excluded.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    const locked = setup(() => {
      throw new BridgeError("locked", "x");
    });
    expect(await locked.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    const offline = setup((r) => {
      if (r.type === "passkey_create") throw new BridgeError("offline", "x");
      return withUpgrade({ kind: "auto", itemId: ITEM })(r);
    });
    expect(await offline.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
  });

  it("sends conditional: false for ordinary creates", async () => {
    const { h, requests } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    expect(requests[0]).toMatchObject({ type: "check_passkey_create", conditional: false });
  });
});
```

`messages.test.ts`: `parseCreateOptions` via `parsePageRequest`/`parseWaRequest` requires `conditional` (a create without it → null; `conditional: "yes"` → null). `parseWaReply` accepts `{ ok: true, token, ui: "saved", credential: <valid create credential> }`, rejects `ui: "saved"` without a credential, with a `get` credential, or with extra keys; rejects a `credential` key on the other `ui` values.

`page.test.ts`: a `create({ mediation: "conditional", publicKey })` is now forwarded as a request whose `options.conditional === true` (read how existing tests capture the dispatched `REQUEST_EVENT`); a create without `mediation` sends `conditional: false`; when the bridge answers `fallback`, the original `create` is called with the original options (including `mediation`).

`bridge.test.ts` (Review Focus 5):

```ts
  it("answers a silent save with the credential and shows the notice for NOTICE_MS", async () => {
    vi.useFakeTimers();
    const credential = { type: "create", credentialId: "AQEBAQEBAQEBAQEBAQEBAQ", clientDataJson: "e30", attestationObject: "oA", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -7 };
    reply = (m) => ((m as { type: string }).type === "wa_create" ? { ok: true, token: TOKEN, ui: "saved", credential } : undefined);
    const id = hex(901);
    request({ kind: "create", id, options: { ...create, conditional: true } });
    await vi.advanceTimersByTimeAsync(0);
    expect(responses.find((r) => r.id === id)).toMatchObject({ outcome: "credential", credential: { credentialId: credential.credentialId } });
    const frame = () => document.querySelector(`iframe[src$="#${TOKEN}"]`);
    expect(frame()?.getAttribute("src")).toContain("passkey.html");
    await vi.advanceTimersByTimeAsync(NOTICE_MS);
    expect(frame()).toBeNull();
  });

  it("shows no notice when the page cancelled during a silent save", async () => {
    let answer: (v: unknown) => void = () => undefined;
    reply = (m) => ((m as { type: string }).type === "wa_create" ? new Promise((r) => (answer = r)) : undefined);
    const id = hex(902);
    request({ kind: "create", id, options: { ...create, conditional: true } });
    await flush();
    request({ kind: "cancel", id });
    answer({ ok: true, token: hex(903), ui: "saved", credential: { type: "create", credentialId: "AQEBAQEBAQEBAQEBAQEBAQ", clientDataJson: "e30", attestationObject: "oA", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -7 } });
    await flush();
    expect(document.querySelector(`iframe[src$="#${hex(903)}"]`)).toBeNull();
    expect(responses.some((r) => r.id === id)).toBe(false);
  });
```

(`create` is a `CreateOptions` fixture — define it next to `get` if the file has none: `{ rpId: null, challenge: "AQ", userId: "AQ", userName: "octo", userDisplayName: null, algs: [-7], excludeCredentials: [], timeoutMs: 1000, conditional: false }`. Import `NOTICE_MS`.)

`passkey.test.ts`:

```ts
  it("titles the upgrade card and preselects the filled login", async () => {
    const OTHER = "11111111-2222-4333-8444-555555555555";
    replies = [{ ok: true, value: { state: "create", site: "github.com", userName: "octo", upgradeItemId: OTHER, candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }, { itemId: OTHER, title: "GitHub work", username: "work" }] } }];
    await load();
    expect(text("question")).toBe("Add a passkey?");
    const checked = [...document.querySelectorAll<HTMLInputElement>("input[type=radio]")].map((r) => r.checked);
    expect(checked).toEqual([false, true, false]);
  });

  it("shows the saved notice without buttons", async () => {
    replies = [{ ok: true, value: { state: "saved", site: "github.com" } }];
    await load();
    expect(text("question")).toBe("Passkey saved to HavenKeys");
    expect(text("detail")).toBe("Manage it in the HavenKeys app");
    expect(text("site")).toBe("github.com");
    for (const id of ["cancel", "fallback", "confirm"]) expect((document.getElementById(id) as HTMLButtonElement).hidden).toBe(true);
  });
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/extension test` — FAIL.

- [ ] **Step 3: Implement**

`messages.ts`: add `conditional: boolean` to `CreateOptions` (doc: "`mediation: \"conditional\"`: the site's automatic passkey upgrade"), add `"conditional"` to `parseCreateOptions`' key list with `typeof conditional !== "boolean"` → null; `NOTICE_MS`; the new `WaReply` member; `PkView` changes. `parseWaReply`:

```ts
  if (o.ok === true) {
    if (!isToken(o.token)) return null;
    if (o.ui === "saved") {
      if (!keysAre(o, ["ok", "token", "ui", "credential"])) return null;
      const c = parseCredential(o.credential);
      return c && c.type === "create" ? { ok: true, token: o.token, ui: "saved", credential: c } : null;
    }
    if (!keysAre(o, ["ok", "token", "ui"])) return null;
    if (o.ui !== "chooser" && o.ui !== "create" && o.ui !== "none") return null;
    return { ok: true, token: o.token, ui: o.ui };
  }
```

`page.ts` `wrappedCreate`:

```ts
  function wrappedCreate(options?: CredentialCreationOptions): Promise<Credential | null> {
    const fallback = () => origCreate(options as never);
    const pk = options?.publicKey;
    if (!pk) return fallback();
    // Automatic passkey upgrade (not yet in lib.dom's CredentialCreationOptions):
    // HavenKeys answers only right after it filled this site's password; the
    // desktop decides, and anything else ends in the browser's own create().
    const conditional = (options as { mediation?: string }).mediation === "conditional";
    const opts = createOptions(pk, conditional);
    if (!opts) return fallback();
    const signal = options?.signal ?? undefined;
    return ask({ kind: "create", id: newId(), options: opts }, signal).then((o) => settle(o, fallback, signal));
  }
```

and `createOptions(pk, conditional: boolean)` puts `conditional` in the returned object. (A conditional create stays "modal" in `ask`: it is bounded by the site's timeout like any create.)

`webauthn-handler.ts`: add to the create session `upgradeItemId: string | null` (null for ordinary creates); `view()` returns `{ state: "create", site, userName, candidates, upgradeItemId: s.upgradeItemId }`; `save()` and `lookAgain()` send `conditional: false`; `beginCreate`'s ordinary path sends `conditional: false`. New:

```ts
export const NOTICE_TTL_MS = NOTICE_MS + 10_000;
...
  // Tabs showing the "passkey saved" notice: the notice frame asks for its
  // site by token, after the silent save finished and no session is left.
  const notices = new Map<number, { token: string; site: string; timer: ReturnType<typeof setTimeout> }>();

  function dropNotice(tabId: number): void {
    const n = notices.get(tabId);
    if (!n) return;
    notices.delete(tabId);
    clearTimeout(n.timer);
  }

  /**
   * The site's automatic upgrade. The desktop decides (`upgrade`), from a
   * HavenKeys password fill on this site in the last few minutes; anything
   * but a usable decision falls back to the browser, silently.
   */
  async function beginUpgrade(frame: FrameRef, options: CreateOptions, rpId: string): Promise<WaReply> {
    let check: ResultFor<"check_passkey_create">;
    try {
      check = await deps.client.request({
        type: "check_passkey_create",
        ...frameFields(frame),
        rpId,
        userName: options.userName,
        excludeCredentials: options.excludeCredentials,
        conditional: true,
      });
    } catch {
      return FALLBACK;
    }
    const upgrade = check.upgrade;
    if (check.excluded || upgrade.kind === "none") return FALLBACK;
    if (upgrade.kind === "ask") {
      const token = open(
        frame.tabId,
        (token, timer) => ({ kind: "create", token, frame, rpId, options, locked: false, refreshing: null, busy: false, candidates: check.candidates, exists: false, upgradeItemId: upgrade.itemId, timer }),
        clampTimeout(options.timeoutMs),
      );
      return { ok: true, token, ui: "create" };
    }
    let r: ResultFor<"passkey_create">;
    try {
      r = await deps.client.request({
        type: "passkey_create",
        ...frameFields(frame),
        rpId,
        challenge: options.challenge,
        userHandle: options.userId,
        userName: options.userName,
        displayName: options.userDisplayName,
        itemId: upgrade.itemId,
        conditional: true,
      });
    } catch {
      return FALLBACK;
    }
    dropNotice(frame.tabId);
    const token = deps.newToken();
    const timer = setTimeout(() => {
      if (notices.get(frame.tabId)?.token === token) notices.delete(frame.tabId);
    }, NOTICE_TTL_MS);
    notices.set(frame.tabId, { token, site: displayHost(frame.url) ?? "", timer });
    return {
      ok: true,
      token,
      ui: "saved",
      credential: {
        type: "create",
        credentialId: r.credentialId,
        clientDataJson: r.clientDataJson,
        attestationObject: r.attestationObject,
        authenticatorData: r.authenticatorData,
        publicKey: r.publicKey,
        publicKeyAlgorithm: r.publicKeyAlgorithm,
      },
    };
  }
```

At the top of `beginCreate`, after the ES256 check and `rpId`: `if (options.conditional) return beginUpgrade(frame, options, rpId);`. Extract the credential mapping shared with `save()` into a small `created(r)` helper rather than duplicating it. In `handleFrame`'s `if (!s)` branch, first:

```ts
      const n = notices.get(tabId);
      if (req.type === "pk_state" && n && n.token === req.token) return { ok: true, value: { state: "saved", site: n.site } };
```

`forgetTab` also calls `dropNotice(tabId)`. Update the file header's trust-model bullet "Nothing is signed or created without a pick/save from the frame" to: "…, except the site's automatic upgrade, which the desktop allows only after a HavenKeys password fill of that login on that site within the last 5 minutes (`upgrade: auto`)."

`frames.ts`:

```ts
export const NOTICE_HEIGHT = 112;

/** The "passkey saved" notice: where the passkey card would be, shorter. */
export function noticeBox(viewport: { width: number }): Box {
  return { ...passkeyBox(viewport), height: NOTICE_HEIGHT };
}
```

`bridge.ts` in `begin()`, replacing the lines after `if (!reply.ok) return respond(...)`:

```ts
    if (reply.ui === "saved") {
      respond(req.id, { outcome: "credential", credential: reply.credential });
      showNotice(reply.token);
      return;
    }
    entry.token = reply.token;
    ...
```

and

```ts
  /** "Passkey saved", for NOTICE_MS; gone at once if the page touches it. */
  function showNotice(token: string): void {
    const frame: InlineFrame = new InlineFrame("passkey.html", token, noticeBox(viewport()), () => frame.remove());
    setTimeout(() => frame.remove(), NOTICE_MS);
  }
```

(`respond` already does nothing if the page cancelled; the earlier `pending.get(req.id) !== entry` check returns before this, so no notice then.) Update the header comment to mention the notice.

`passkey.ts`: `choices(candidates, userName, preselect: string | null)` — the preferred candidate is `candidates.find((c) => c.itemId === preselect)` when `preselect` is set, else the existing username match. In `render` "create": `question.textContent = view.upgradeItemId ? "Add a passkey?" : "Save a passkey to HavenKeys?";` and pass `view.upgradeItemId`. New case:

```ts
    case "saved":
      question.textContent = "Passkey saved to HavenKeys";
      detail.textContent = "Manage it in the HavenKeys app";
      main.replaceChildren();
      cancelBtn.hidden = fallbackBtn.hidden = confirmBtn.hidden = true;
      return;
```

If the empty `.actions` row leaves visible padding in the notice, hide it with `document.querySelector(".actions")?.setAttribute("hidden", "")` in that case, and check `NOTICE_HEIGHT` against the card by building the extension (`pnpm build:extension`) and opening `dist/chrome/passkey.html#<32 hex>` is not possible without a background; instead keep `NOTICE_HEIGHT` = header (34) + prompt (two lines) + padding and adjust only if a manual run shows clipping.

- [ ] **Step 4: Verify** `pnpm --filter @havenkeys/extension test && pnpm typecheck` — PASS (the hygiene tests included).

- [ ] **Step 5: Commit** `git commit -am "feat(extension): answer the automatic passkey upgrade (silent save with notice, or ask)"`

---

### Task 7: Passkeys Directory snapshot and matcher

**Files:**
- Create: `scripts/update-passkey-directory.mjs`
- Create: `apps/extension/src/data/passkey-sites.json` (generated by the script)
- Create: `apps/extension/src/background/passkey-sites.ts`, `apps/extension/src/background/passkey-sites.test.ts`
- Modify: `apps/extension/tsconfig.json` (`"resolveJsonModule": true`)
- Modify: `THIRD-PARTY-NOTICES.md`

**Interfaces:**
- Produces:
  ```ts
  export interface PasskeySite { name: string; domains: string[]; passwordless: boolean; mfa: boolean; help: string | null }
  export function parsePasskeySites(data: unknown): PasskeySite[] | null;
  export function findPasskeySite(pageUrl: string, sites?: readonly PasskeySite[]): PasskeySite | null; // defaults to PASSKEY_SITES
  export const PASSKEY_SITES: readonly PasskeySite[]; // the committed file, validated; [] if invalid
  ```

- [ ] **Step 1: Write the script**

```js
#!/usr/bin/env node
// Rebuilds apps/extension/src/data/passkey-sites.json from the Passkeys
// Directory by 2factorauth (https://github.com/2factorauth/passkeys,
// CC-BY-4.0). Run by hand; the output is committed and reviewed like code,
// and the extension never fetches anything at runtime.
//
// The public API (passkeys-api.2fa.directory) carries no site names, so this
// reads the repository's entries/*/*.json: { "<Name>": { "additional-domains"?,
// "passwordless"?, "mfa"?, "documentation"? } }, named after the primary domain.
//
// Usage: node scripts/update-passkey-directory.mjs

import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "https://github.com/2factorauth/passkeys.git";
const OUT = join(dirname(fileURLToPath(import.meta.url)), "../apps/extension/src/data/passkey-sites.json");
const MAX_NAME = 100;

function hostname(d) {
  if (typeof d !== "string" || d.length === 0 || d.length > 253 || !d.includes(".") || d.endsWith(".")) return null;
  try {
    const h = new URL(`https://${d}/`).hostname;
    return h === d.toLowerCase() && h === d ? h : null;
  } catch {
    return null;
  }
}

function httpsUrl(u) {
  try {
    return typeof u === "string" && new URL(u).protocol === "https:" ? new URL(u).href : null;
  } catch {
    return null;
  }
}

function cleanName(n) {
  const t = typeof n === "string" ? n.trim() : "";
  // eslint-disable-next-line no-control-regex
  return t.length > 0 && t.length <= MAX_NAME && !/[\u0000-\u001f\u007f]/.test(t) ? t : null;
}

const dir = mkdtempSync(join(tmpdir(), "passkeys-dir-"));
try {
  execFileSync("git", ["clone", "--depth", "1", "--quiet", REPO, dir], { stdio: "inherit" });
  const commit = execFileSync("git", ["-C", dir, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const sites = [];
  const entries = join(dir, "entries");
  for (const letter of readdirSync(entries)) {
    for (const file of readdirSync(join(entries, letter))) {
      if (!file.endsWith(".json")) continue;
      const primary = hostname(file.slice(0, -".json".length));
      if (!primary) continue;
      const data = JSON.parse(readFileSync(join(entries, letter, file), "utf8"));
      for (const [rawName, e] of Object.entries(data)) {
        const name = cleanName(rawName);
        const passwordless = e?.passwordless === "allowed";
        const mfa = e?.mfa === "allowed";
        if (!name || !(passwordless || mfa)) continue;
        const extra = Array.isArray(e["additional-domains"]) ? e["additional-domains"].map(hostname).filter(Boolean) : [];
        const domains = [...new Set([primary, ...extra])];
        sites.push({ name, domains, passwordless, mfa, help: httpsUrl(e.documentation) });
      }
    }
  }
  sites.sort((a, b) => a.name.localeCompare(b.name, "en") || a.domains[0].localeCompare(b.domains[0]));
  writeFileSync(OUT, JSON.stringify(sites, null, 2) + "\n");
  console.error(`Wrote ${sites.length} sites from 2factorauth/passkeys@${commit}`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
```

Run: `node scripts/update-passkey-directory.mjs` (needs network and git). Expected stderr: `Wrote N sites from 2factorauth/passkeys@<sha>` with N in the hundreds. Record `<sha>` for the notice. Skim the diff of the generated JSON for anything odd (non-https help, weird names).

- [ ] **Step 2: Failing tests** (`passkey-sites.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import raw from "../data/passkey-sites.json";
import { findPasskeySite, parsePasskeySites, PASSKEY_SITES, type PasskeySite } from "./passkey-sites";

const site = (name: string, domains: string[], help: string | null = `https://${domains[0]}/help`): PasskeySite => ({ name, domains, passwordless: true, mfa: false, help });
const SITES = [site("GitHub", ["github.com"]), site("Nintendo", ["nintendo.com"]), site("Nintendo Account", ["accounts.nintendo.com"]), site("Microsoft", ["microsoft.com", "live.com"])];

describe("committed directory", () => {
  it("validates and is what the extension loads", () => {
    const parsed = parsePasskeySites(raw);
    expect(parsed).not.toBeNull();
    expect(parsed!.length).toBeGreaterThan(50);
    expect(PASSKEY_SITES).toEqual(parsed);
    expect(parsed!.every((s) => s.help === null || s.help.startsWith("https://"))).toBe(true);
  });
});

describe("parsePasskeySites", () => {
  it("rejects anything off-shape", () => {
    const good = { name: "A", domains: ["a.com"], passwordless: true, mfa: false, help: null };
    expect(parsePasskeySites([good])).toEqual([good]);
    for (const bad of [
      { ...good, extra: 1 },
      { ...good, help: "http://a.com/" },
      { ...good, help: "javascript:alert(1)" },
      { ...good, domains: [] },
      { ...good, domains: ["A.com"] },
      { ...good, domains: ["a.com."] },
      { ...good, domains: ["localhost"] },
      { ...good, name: "" },
      { ...good, name: "a\u0007" },
      { ...good, mfa: "allowed" },
    ]) {
      expect(parsePasskeySites([bad])).toBeNull();
    }
    expect(parsePasskeySites({})).toBeNull();
  });
});

describe("findPasskeySite", () => {
  it("matches the host or a subdomain, never a look-alike", () => {
    expect(findPasskeySite("https://github.com/login", SITES)?.name).toBe("GitHub");
    expect(findPasskeySite("https://gist.github.com/", SITES)?.name).toBe("GitHub");
    expect(findPasskeySite("https://GitHub.com./login", SITES)?.name).toBe("GitHub");
    expect(findPasskeySite("https://login.live.com/", SITES)?.name).toBe("Microsoft");
    for (const u of ["https://github.com.evil.com/", "https://evilgithub.com/", "https://github.co/", "not a url", "https://127.0.0.1/"]) {
      expect(findPasskeySite(u, SITES)).toBeNull();
    }
  });

  it("prefers the longest matching domain", () => {
    expect(findPasskeySite("https://accounts.nintendo.com/login", SITES)?.name).toBe("Nintendo Account");
    expect(findPasskeySite("https://www.nintendo.com/", SITES)?.name).toBe("Nintendo");
  });
});
```

- [ ] **Step 3: Run** `pnpm --filter @havenkeys/extension test passkey-sites` — FAIL (module missing).

- [ ] **Step 4: Implement** `passkey-sites.ts`:

```ts
// Sites known to support passkeys, from the Passkeys Directory by
// 2factorauth (CC-BY-4.0; see THIRD-PARTY-NOTICES.md). A snapshot committed
// with the extension (scripts/update-passkey-directory.mjs), never fetched.
//
// Only a UI hint: a match shows a help link in the field menu, nothing more,
// so matching is plain host-suffix comparison (no Public Suffix List).
// The data is third-party text: validated here, shown with textContent only.

import raw from "../data/passkey-sites.json";

export interface PasskeySite {
  name: string;
  /** Primary domain first. Lowercase hostnames. */
  domains: string[];
  passwordless: boolean;
  mfa: boolean;
  /** The site's passkey help article, https only. */
  help: string | null;
}

const MAX_NAME = 100;
const MAX_SITES = 10_000;

type Obj = Record<string, unknown>;

function isHostname(v: unknown): v is string {
  if (typeof v !== "string" || v.length > 253 || !v.includes(".") || v.endsWith(".")) return false;
  try {
    return new URL(`https://${v}/`).hostname === v;
  } catch {
    return false;
  }
}

function isHttps(v: unknown): v is string {
  try {
    return typeof v === "string" && new URL(v).protocol === "https:";
  } catch {
    return false;
  }
}

function parseSite(v: unknown): PasskeySite | null {
  if (typeof v !== "object" || v === null || Array.isArray(v)) return null;
  const o = v as Obj;
  const keys = ["name", "domains", "passwordless", "mfa", "help"];
  if (Object.keys(o).length !== keys.length || !keys.every((k) => Object.prototype.hasOwnProperty.call(o, k))) return null;
  const { name, domains, passwordless, mfa, help } = o;
  // eslint-disable-next-line no-control-regex
  if (typeof name !== "string" || name.length === 0 || name.length > MAX_NAME || /[\u0000-\u001f\u007f]/.test(name)) return null;
  if (!Array.isArray(domains) || domains.length === 0 || !domains.every(isHostname)) return null;
  if (typeof passwordless !== "boolean" || typeof mfa !== "boolean") return null;
  if (help !== null && !isHttps(help)) return null;
  return { name, domains: [...domains], passwordless, mfa, help };
}

/** The whole list, or null if any entry is off-shape. */
export function parsePasskeySites(data: unknown): PasskeySite[] | null {
  if (!Array.isArray(data) || data.length > MAX_SITES) return null;
  const out: PasskeySite[] = [];
  for (const v of data) {
    const s = parseSite(v);
    if (!s) return null;
    out.push(s);
  }
  return out;
}

export const PASSKEY_SITES: readonly PasskeySite[] = parsePasskeySites(raw) ?? [];

/** The entry for the page's host: equal to one of its domains or a subdomain of one; the longest domain wins. */
export function findPasskeySite(pageUrl: string, sites: readonly PasskeySite[] = PASSKEY_SITES): PasskeySite | null {
  let host: string;
  try {
    host = new URL(pageUrl).hostname.replace(/\.$/, "");
  } catch {
    return null;
  }
  let best: { site: PasskeySite; len: number } | null = null;
  for (const site of sites) {
    for (const d of site.domains) {
      if ((host === d || host.endsWith(`.${d}`)) && d.length > (best?.len ?? 0)) best = { site, len: d.length };
    }
  }
  return best?.site ?? null;
}
```

Add `"resolveJsonModule": true` to `apps/extension/tsconfig.json` compilerOptions. esbuild bundles the JSON import natively; run `pnpm build:extension` to confirm.

`THIRD-PARTY-NOTICES.md`: a new top-level section after the fonts:

```markdown
## Data

### Passkeys Directory

`apps/extension/src/data/passkey-sites.json` is derived from the
[Passkeys Directory by 2factorauth](https://github.com/2factorauth/passkeys)
(site names, domains, passkey support and documentation links; snapshot of
commit `<sha>`), licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Changes: entries
without a valid hostname or passkey support are dropped, non-https
documentation links are removed, and fields not used by HavenKeys are left
out. It is embedded in the browser extension package.
```

Also update the intro sentence that says only fonts need notices ("The fonts below are different…") to cover the data too.

- [ ] **Step 5: Verify** `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck && pnpm build:extension` — PASS.

- [ ] **Step 6: Commit**

```bash
git add scripts/update-passkey-directory.mjs apps/extension/src/data apps/extension/src/background/passkey-sites.ts apps/extension/src/background/passkey-sites.test.ts apps/extension/tsconfig.json THIRD-PARTY-NOTICES.md
git commit -m "feat(extension): Passkeys Directory snapshot and host matcher"
```

---

### Task 8: Field menu — passkeys first and the directory hint

**Files:**
- Modify: `apps/extension/src/messaging/inline.ts` (types, `parseInlineRequest`)
- Modify: `apps/extension/src/background/inline-handler.ts`
- Modify: `apps/extension/src/background/index.ts` (deps)
- Modify: `apps/extension/src/menu/menu.ts`, `apps/extension/src/menu/inline.css` (hint row style)
- Tests: `apps/extension/src/background/inline-handler.test.ts`; create `apps/extension/src/menu/menu.test.ts` (jsdom, same pattern as `passkey.test.ts`)

**Interfaces:**
- Consumes: `passkey_status` (Task 5), `findPasskeySite`/`PasskeySite` (Task 7).
- Produces:
  - `MenuHint = { kind: "use_passkey" } | { kind: "add_passkey"; name: string }`; `MenuView` ready gains `hint: MenuHint | null`.
  - `InlineRequest` gains `{ type: "menu_open_help"; token: string }`.
  - `InlineDeps` gains optional `passkeySite?(url: string): PasskeySite | null` and `openTab?(url: string): void`.

Rules (login menus only, vault unlocked, `items.length > 0`, and no conditional passkeys already listed):
- Ask `passkey_status` for the frame; any error → `false`.
- `hasPasskey` → hint `use_passkey` (shown first, not clickable).
- otherwise, `passkeySite(frame.url)` with a non-null `help` → hint `add_passkey` (shown last, clickable; the help URL stays in the background session).
- otherwise no hint. `rows` counts the hint row (still capped at 5).
- `menu_open_help`: live, unlocked menu with an `add_passkey` hint → `closeMenu`, `openTab(help)`, `{ ok: true, value: null }`; else `{ ok: false, message: "This menu has expired." }`.
- OTP and new-password menus never ask `passkey_status`.

- [ ] **Step 1: Failing tests** (`inline-handler.test.ts`; `setup` gains optional deps — extend it to accept `extra: Partial<InlineDeps>` and spread it into `createInlineHandler`)

```ts
const GH_SITE = { name: "GitHub", domains: ["github.com"], passwordless: true, mfa: true, help: "https://docs.github.com/passkeys" };

describe("passkey hints in the login menu", () => {
  const status = (has: boolean) => (r: Request) => (r.type === "passkey_status" ? { type: "passkey_status", hasPasskey: has } : defaultAnswer(r));

  it("leads with a use-your-passkey hint when a passkey exists", async () => {
    const { h, requests } = setup(status(true), { passkeySite: () => GH_SITE });
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 2 });
    expect(requests.find((r) => r.type === "passkey_status")).toEqual({ type: "passkey_status", url: "https://github.com/login" });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ ok: true, value: { hint: { kind: "use_passkey" } } });
  });

  it("offers the directory help link when there is no passkey, and opens it only through the menu", async () => {
    const opened: string[] = [];
    const { h } = setup(status(false), { passkeySite: () => GH_SITE, openTab: (u) => void opened.push(u) });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ ok: true, value: { hint: { kind: "add_passkey", name: "GitHub" } } });
    expect((await h.handleInline(2, { type: "menu_open_help", token: T1 })).ok).toBe(false);
    expect(opened).toEqual([]);
    expect(await h.handleInline(1, { type: "menu_open_help", token: T1 })).toEqual({ ok: true, value: null });
    expect(opened).toEqual(["https://docs.github.com/passkeys"]);
    // The menu closed: a second click does nothing.
    expect((await h.handleInline(1, { type: "menu_open_help", token: T1 })).ok).toBe(false);
  });

  it("shows no hint without a help link, when the status fails, for unknown sites, or in OTP menus", async () => {
    const noHelp = setup(status(false), { passkeySite: () => ({ ...GH_SITE, help: null }) });
    await noHelp.h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await noHelp.h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ value: { hint: null } });

    const failing = setup(defaultAnswer, { passkeySite: () => null }); // passkey_status throws in defaultAnswer
    expect(await failing.h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 1 });

    const otp = setup(status(true), { passkeySite: () => GH_SITE });
    await otp.h.handleContent(frame(), { type: "cs_open_menu", kind: "otp" });
    expect(otp.requests.some((r) => r.type === "passkey_status")).toBe(false);
  });

  it("does not ask when the site offers passkey autofill (passkey rows already lead)", async () => {
    const row = { itemId: GH, credentialId: "AQEBAQEBAQEBAQEBAQEBAQ", title: "GitHub", userName: "octo" };
    const { h, requests } = setup(status(true), {
      passkeySite: () => GH_SITE,
      passkeys: { conditionalFor: () => [row], pickConditional: async () => ({ ok: true, value: null }) },
    });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(requests.some((r) => r.type === "passkey_status")).toBe(false);
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ value: { hint: null } });
  });
});
```

Also: `parseInlineRequest({ type: "menu_open_help", token: T1 })` parses; with an extra key it is null. Update existing tests that assert the exact `requests` list or `menu_state` value to include `passkey_status` / `hint: null` as needed.

`menu.test.ts` (jsdom; build the DOM `menu.html` has — `#site`, `#main`, `.card` — fake `chrome.runtime.sendMessage` like `passkey.test.ts`):

```ts
  it("renders the use-passkey hint first as plain text and the help row last as a button", async () => {
    replies = [{ ok: true, value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo" }], passkeys: [], hint: { kind: "use_passkey" } } }];
    await load();
    const main = document.getElementById("main")!;
    expect(main.firstElementChild?.tagName).toBe("DIV");
    expect(main.firstElementChild?.textContent).toContain("You have a passkey for github.com");
    expect(main.querySelectorAll("button.row")).toHaveLength(1);
  });

  it("renders the directory row last and sends menu_open_help only for a trusted, armed click", async () => {
    replies = [{ ok: true, value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo" }], passkeys: [], hint: { kind: "add_passkey", name: "GitHub" } } }];
    await load();
    const rows = document.querySelectorAll<HTMLButtonElement>("button.row");
    const last = rows[rows.length - 1]!;
    expect(last.textContent).toContain("GitHub supports passkeys");
    last.click(); // untrusted
    expect(asked.some((m) => (m as { type: string }).type === "menu_open_help")).toBe(false);
  });
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/extension test` — FAIL.

- [ ] **Step 3: Implement**

`inline.ts`: add `MenuHint`, `hint` to the ready `MenuView`, `menu_open_help` to `InlineRequest` and to the `parseInlineRequest` token-only case list.

`inline-handler.ts`: `MenuSession` gains `hint: MenuHint | null; help: string | null`. In `openMenu`, after computing `passkeys`:

```ts
    const { hint, help } = kind === "login" && !locked && items.length > 0 && passkeys.length === 0 ? await passkeyHint(frame) : { hint: null, help: null };
```

with

```ts
  /**
   * Passkeys first: a hint to use the site's own passkey sign-in when
   * HavenKeys holds one for the page, otherwise, for a site in the Passkeys
   * Directory, a link to its help article. The answer only shapes our menu,
   * which the page cannot read.
   */
  async function passkeyHint(frame: FrameRef): Promise<{ hint: MenuHint | null; help: string | null }> {
    let has = false;
    try {
      has = (await deps.client.request({ type: "passkey_status", ...frameFields(frame) })).hasPasskey;
    } catch {
      // Locked meanwhile, desktop gone, rate limited: no hint.
    }
    if (has) return { hint: { kind: "use_passkey" }, help: null };
    const site = deps.passkeySite?.(frame.url);
    return site?.help ? { hint: { kind: "add_passkey", name: site.name }, help: site.help } : { hint: null, help: null };
  }
```

`rows = locked || kind === "new_password" ? 1 : Math.min(items.length + passkeys.length + (hint ? 1 : 0), MAX_ROWS)`. `menu_state` returns `hint: m.hint`. New case:

```ts
      case "menu_open_help": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !m.help || !deps.openTab) return { ok: false, message: "This menu has expired." };
        closeMenu(tabId);
        deps.openTab(m.help);
        return { ok: true, value: null };
      }
```

`index.ts`: `import { findPasskeySite } from "./passkey-sites";` and pass `passkeySite: (url) => findPasskeySite(url), openTab: (url) => void chrome.tabs.create({ url }).catch(() => undefined)` to `createInlineHandler`. (`chrome.tabs.create` needs no permission.)

`menu.ts` `render`, ready branch:

```ts
  const hint = view.hint;
  const lead = hint?.kind === "use_passkey" ? [hintNote(`You have a passkey for ${view.site}`, "Use the site’s “Sign in with a passkey” option")] : [];
  const tail =
    hint?.kind === "add_passkey"
      ? [row(sparkle(), `${hint.name} supports passkeys`, "How to add one", () => pick({ type: "menu_open_help", token: t }))]
      : [];
  main.replaceChildren(...lead, ...passkeyRows, ...view.items.map((i) => itemRow(t, i, kind)), ...tail);
```

with

```ts
/** A row that only informs: no button, not in the arrow-key order. */
function hintNote(title: string, detail: string): HTMLElement {
  return h(
    "div",
    { className: "row hint" },
    h("span", { className: "who" }, h("span", { className: "title", text: title }), h("span", { className: "user", text: detail })),
  );
}
```

The arrow-key handler selects `button.row` only — change its selector from `.row` to `button.row` (and the focus handler too). `inline.css`: `.row.hint { cursor: default; }` plus whatever keeps it from showing hover styles (read the existing `.row:hover` rule and exclude `.hint`). The `pick` error message "Could not fill" is fine for the help row too.

- [ ] **Step 4: Verify** `pnpm --filter @havenkeys/extension test && pnpm typecheck && pnpm build:extension` — PASS.

- [ ] **Step 5: Commit** `git commit -am "feat(extension): passkeys-first menu hint and the Passkeys Directory help row"`

---

### Task 9: Documentation

**Files:**
- Modify: `docs/threat-model.md` (passkeys section ~216-280; attack table ~335)
- Modify: `docs/autofill.md` (Passkeys section: add "Automatic upgrade", "Passkey hints in the field menu"; Errors and states table; Limitations)
- Modify: `docs/native-messaging.md` (request/response list)
- Modify: `docs/security-model.md` (vault settings, in-memory state, extension data)
- Modify: `docs/superpowers/specs/2026-09-23-passkeys-design.md` §5.3 (a one-paragraph amendment note pointing to the upgrade spec)
- Modify: `README.md` only if it lists passkey features

- [ ] **Step 1: Write the docs.** Content to cover, in each file's existing voice:
  - **threat-model.md**: the four §8 items from the upgrade spec (relaxed click rule with its bound — a compromised extension can at most add a passkey to the login the user just filled on that site within 5 minutes, and the user can turn it off; fill memory — IDs and sites only, never persisted, cleared on lock; `passkey_status` oracle — Lookup-class, asked only on menu open, answer never reaches the page, residual frame-size signal; directory data — untrusted, `textContent`, https-only, reviewed, never fetched). Add rows A1u–A3u (or extend A1p–A3p) to the attack table naming `tests/passkeys.rs` (`conditional_create_needs_auto_for_exactly_that_login`, `upgrade_is_per_site_and_never_for_look_alikes`) and `tests/bridge.rs` (`passkey_upgrade_through_the_bridge`).
  - **autofill.md**: the automatic upgrade flow (auto → notice; ask → "Add a passkey?" card; everything else → the browser); the setting; the field-menu hints and their order; Passkeys Directory attribution ("Passkeys Directory by 2factorauth", CC-BY-4.0), how the snapshot is built (`node scripts/update-passkey-directory.mjs`, reads the repository because the API has no names), that it is never fetched; new rows in "Errors and states" (spec §7); limitations: a site that checks `PublicKeyCredential.getClientCapabilities().conditionalCreate` sees the browser's own answer (HavenKeys does not change it); a page that aborts during a silent save leaves the passkey in the vault (visible in the desktop app) with no notice.
  - **native-messaging.md**: `conditional` on `check_passkey_create`/`passkey_create`, the `upgrade` result, `passkey_status`, and its rate-limit class.
  - **security-model.md**: `auto_passkey_upgrade` in the encrypted settings; recent fills in the unlocked session only.

- [ ] **Step 2: Verify** no claims beyond what the code does: `grep -rn "upgrade\|passkey_status" docs | head -50` and reread each paragraph against the implementation.

- [ ] **Step 3: Commit** `git commit -am "docs: passkey upgrade, passkey_status and the Passkeys Directory hint"`

---

### Task 10: Full verification and security self-review

- [ ] **Step 1:** `pnpm typecheck && pnpm test && pnpm lint:rust && pnpm build:extension` — all PASS. Paste failures verbatim if any and fix them.
- [ ] **Step 2:** `pnpm audit && cargo audit && cargo deny check` — no new findings (no dependencies were added; if the tools are missing, say so).
- [ ] **Step 3:** Secret-logging audit of the diff: `git diff main --stat` then read every new `console.*`, `eprintln!`, `format!`, `Debug` impl and error message; nothing may include a username, title, URL, credential, or site name. `RecentFill` must not derive `Debug`.
- [ ] **Step 4:** Add findings (if any) to `docs/security-review.md` as new PK entries with severity, component, scenario, mitigation, residual — at minimum record the relaxed click rule as an accepted residual.
- [ ] **Step 5:** Commit `git commit -am "docs(security-review): passkey upgrade review"` if anything changed.

Manual checks (for the human, not blocking): Google's automatic upgrade in Chrome with the setting on and off; a directory site (e.g. github.com with a saved password and no passkey) showing the help row.
