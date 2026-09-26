# Automatic Sign-in Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After the user picks a login, HavenKeys fills it, presses the site's sign-in button, and continues through a following password page and TOTP step on the same origin, with a global and a per-login switch, both on by default.

**Architecture:** Rust computes `autoSubmit` (global setting AND the login's switch) and returns it with every `fill_item` / `get_totp`. The extension's background worker keeps a memory-only *sign-in run* per tab (item, frame, origin, step). The content script presses buttons (`autofill/submit.ts`) and watches for the next step (`autofill/watch.ts`), and asks the background to continue; every continuation value still comes from Rust's origin-checked calls.

**Tech Stack:** Rust (havenkeys-core, havenkeys-protocol, havenkeys-bridge), TypeScript (WebExtension MV3, vitest + jsdom), React (Tauri desktop).

**Spec:** `docs/superpowers/specs/2026-09-26-auto-sign-in-design.md`

## Global Constraints

- Setting `Settings.auto_sign_in` (`autoSignIn` on the wire), default **true**, also true when absent from an old blob.
- Per-login `ItemOverview.auto_sign_in` (`autoSignIn`), default **true**, stored in the encrypted overview blob; true when absent.
- `autoSubmit = settings.auto_sign_in && item.auto_sign_in`, computed in Rust only.
- Run lifetime 2 minutes (`RUN_TTL_MS = 120_000`); next-step watch timeout 30 s (`WATCH_TIMEOUT_MS = 30_000`); mutation debounce 150 ms; OTP settle wait 500 ms; button enable wait up to 1 s polled every 100 ms.
- Button press only if best score ≥ 60 and beats the runner-up by ≥ 20.
- Run bound to exact origin (scheme+host+port) and frame of the pick; steps forward only `username → password → otp`; each step once; never retry.
- Nothing about runs is persisted. Never log values. No `innerHTML`.
- Never synthesize key events; press with `form.requestSubmit(button)` when the button is a form submitter, else `button.click()`.
- Commits: **no `Co-Authored-By` trailer** (user preference).
- Desktop copy: Settings toggle "Sign in automatically after filling"; login editor switch "Sign in automatically on this site".

## Review Focus

1. **Invisible reCAPTCHA badges** (v3 / `size=invisible`) are on many login pages; treating them as a challenge would silently disable auto sign-in there. Expected: ignored. Test in Task 5.
2. **SPA where the pressed step's field stays on screen while the request is in flight** (same element, still holding our value). Expected: keep waiting, not "came back". Test in Task 6.
3. **Page re-renders the password page after a wrong password** (field emptied or replaced). Expected: run stops, no second fill. Test in Task 6.
4. **A button that is disabled until the input validates** and never enables. Expected: no press, run ends. Test in Task 5.
5. **A late `cs_run_step` after the run expired or after another pick in the same tab.** Expected: ignored, no native request. Test in Task 8.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-core/src/model.rs` | `Settings.auto_sign_in`, `ItemOverview.auto_sign_in`, `ItemInput.auto_sign_in` |
| `crates/havenkeys-core/src/vault.rs` | build/update carry the switch; `VaultService::auto_sign_in_for` |
| `crates/havenkeys-protocol/src/message.rs` | `auto_submit` on `FillItem` / `GetTotp` results |
| `crates/havenkeys-bridge/src/dispatch.rs` | fills `auto_submit` from the core |
| `packages/protocol/src/index.ts` | TS result types + strict parser |
| `apps/desktop/src/lib/types.ts`, `views/SettingsView.tsx`, `views/ItemEditor.tsx` | toggle and per-login switch |
| `apps/extension/src/autofill/submit.ts` (new) | button scoring, challenge detection, pressing |
| `apps/extension/src/autofill/watch.ts` (new) | next-step detection and debounced watcher |
| `apps/extension/src/messaging/inline.ts` | run message types and parsers |
| `apps/extension/src/background/signin-run.ts` (new) | pure run registry |
| `apps/extension/src/background/inline-handler.ts`, `popup-handler.ts`, `index.ts` | start/continue/end runs |
| `apps/extension/src/content/index.ts` | wiring: press after fill, watch, report |
| `docs/*.md`, `CLAUDE.md` | documentation, accepted risk |

---

### Task 1: Core settings, per-login switch, `auto_sign_in_for`

**Files:**
- Modify: `crates/havenkeys-core/src/model.rs` (`ItemOverview` ~line 53, `ItemInput` ~line 188, `Settings` ~line 236, tests module)
- Modify: `crates/havenkeys-core/src/vault.rs` (`stage_update` ~1302, `build_item` ~1510–1620, new method after `totp_for_page` ~1155)
- Modify: every `ItemInput { .. }` literal (21 sites: `crates/havenkeys-bridge/tests/bridge.rs`, `crates/havenkeys-sync-client/tests/round_trip.rs`, `crates/havenkeys-core/tests/security.rs`, `crates/havenkeys-core/tests/common/mod.rs`, `crates/havenkeys-core/src/{model.rs,sync.rs,passkey/vault.rs,import/onepux.rs,vault.rs}`) and the `ItemOverview { .. }` literal in `crates/havenkeys-core/src/origin.rs:379`
- Test: `crates/havenkeys-core/src/model.rs` (unit), `crates/havenkeys-core/tests/security.rs`

**Interfaces:**
- Produces: `Settings { .., pub auto_sign_in: bool }`; `ItemOverview { .., pub auto_sign_in: bool }`; `ItemInput { .., pub auto_sign_in: Option<bool> }`; `VaultService::auto_sign_in_for(&self, id: &Uuid) -> Result<bool>` (`Err(Locked)` when locked, `Err(NotFound)` for unknown IDs).

- [ ] **Step 1: Write the failing unit tests** (append inside `mod tests` in `model.rs`, next to `auto_passkey_upgrade_defaults_on`)

```rust
    #[test]
    fn auto_sign_in_defaults_on() {
        assert!(Settings::default().auto_sign_in);
        let old: Settings =
            serde_json::from_str(r#"{"autoLockMinutes":5,"clipboardClearSeconds":30}"#).unwrap();
        assert!(old.auto_sign_in);
        let off: Settings = serde_json::from_str(
            r#"{"autoLockMinutes":5,"clipboardClearSeconds":30,"autoSignIn":false}"#,
        )
        .unwrap();
        assert!(!off.auto_sign_in);
    }

    #[test]
    fn overviews_saved_before_auto_sign_in_are_on() {
        let json = r#"{"id":"7c9e6679-7425-40de-944b-e07fc1f90ae7","itemType":"login","title":"t",
            "hasPassword":true,"hasTotp":false,"hasNotes":false,"createdAt":1,"updatedAt":1}"#;
        let o: ItemOverview = serde_json::from_str(json).unwrap();
        assert!(o.auto_sign_in);
    }
```

- [ ] **Step 2: Write the failing integration test** (append to `crates/havenkeys-core/tests/security.rs`)

```rust
/// Automatic sign-in needs both the vault setting and the login's switch;
/// an edit that does not mention the switch keeps it.
#[test]
fn auto_sign_in_needs_the_setting_and_the_login_switch() {
    let (mut v, gh) = github_vault();
    assert!(v.auto_sign_in_for(&gh).unwrap(), "both default on");

    let edit = |title: &str, switch: Option<bool>| {
        let mut input = login(title, "octo", "gh-secret", "github.com");
        input.password = SecretUpdate::Keep;
        input.auto_sign_in = switch;
        input
    };
    let staged = v.stage_update(&gh, edit("GitHub", Some(false)), NOW).unwrap();
    v.commit_write(staged, 3).unwrap();
    assert!(!v.auto_sign_in_for(&gh).unwrap());

    let staged = v.stage_update(&gh, edit("GitHub (work)", None), NOW).unwrap();
    v.commit_write(staged, 4).unwrap();
    assert!(!v.auto_sign_in_for(&gh).unwrap(), "None keeps the switch");

    let staged = v.stage_update(&gh, edit("GitHub", Some(true)), NOW).unwrap();
    v.commit_write(staged, 5).unwrap();
    assert!(v.auto_sign_in_for(&gh).unwrap());

    v.update_settings(Settings {
        auto_sign_in: false,
        ..v.settings().unwrap()
    })
    .unwrap();
    assert!(!v.auto_sign_in_for(&gh).unwrap(), "global off wins");

    assert_eq!(v.auto_sign_in_for(&Uuid::new_v4()).err(), Some(Error::NotFound));
    v.lock();
    assert_eq!(v.auto_sign_in_for(&gh).err(), Some(Error::Locked));
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p havenkeys-core auto_sign_in`
Expected: compile errors: no field `auto_sign_in`, no method `auto_sign_in_for`.

- [ ] **Step 4: Add the fields in `model.rs`**

In `ItemOverview`, after `has_passkey`:

```rust
    /// Whether automatic sign-in may press this login's sign-in button and
    /// continue to later steps. `default`: logins saved before this field
    /// existed are on.
    #[serde(default = "default_true")]
    pub auto_sign_in: bool,
```

In `ItemInput`, after `content`:

```rust
    /// Automatic sign-in switch for a login. `None` keeps the current value
    /// on update and means on for a new item.
    #[serde(default)]
    pub auto_sign_in: Option<bool>,
```

In `Settings`, after `auto_passkey_upgrade`:

```rust
    /// Whether HavenKeys may press a site's sign-in button after filling and
    /// continue through later steps (password page, one-time code). On by
    /// default, including for settings saved before this field existed. Each
    /// login has its own switch too (`ItemOverview::auto_sign_in`).
    #[serde(default = "default_true")]
    pub auto_sign_in: bool,
```

In `impl Default for Settings`, add `auto_sign_in: true,`.

- [ ] **Step 5: Carry the switch in `vault.rs`**

In `build_item`, add `auto_sign_in,` to the `let ItemInput { item_type, username, urls, password, totp, notes, content, .. } = input;` destructuring, and in the `ItemOverview { .. }` literal at the end add:

```rust
        auto_sign_in: auto_sign_in.unwrap_or(true),
```

In `stage_update`, make `input` mutable and keep the existing switch when the edit does not mention it:

```rust
    pub fn stage_update(&self, id: &Uuid, mut input: ItemInput, now_ms: i64) -> Result<StagedWrite> {
        let existing = self.get_item(id)?;
        if existing.item_type != input.item_type {
            return Err(Error::InvalidInput("item type cannot change"));
        }
        input.auto_sign_in.get_or_insert(existing.auto_sign_in);
        let current = self.load_details(id)?;
```

After `totp_for_page`, add:

```rust
    /// Whether a fill of this login may press the page's sign-in button: the
    /// vault's `auto_sign_in` setting and the login's own switch. Asked only
    /// after `fill_for_page` / `totp_for_page` authorized the item.
    pub fn auto_sign_in_for(&self, id: &Uuid) -> Result<bool> {
        let session = self.session()?;
        let overview = session.overviews.get(id).ok_or(Error::NotFound)?;
        Ok(session.settings.auto_sign_in && overview.auto_sign_in)
    }
```

- [ ] **Step 6: Fix the struct literals**

Run: `cargo build --workspace --all-targets 2>&1 | grep -E "^error|missing" | head -40`
Add `auto_sign_in: None,` to every `ItemInput { .. }` literal it reports, and `auto_sign_in: true,` to the `ItemOverview` literal in `origin.rs`. Repeat until it builds.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p havenkeys-core`
Expected: all pass, including `auto_sign_in_defaults_on`, `overviews_saved_before_auto_sign_in_are_on`, `auto_sign_in_needs_the_setting_and_the_login_switch`.

- [ ] **Step 8: Commit**

```bash
git add crates
git commit -m "feat(core): auto sign-in setting and per-login switch"
```

---

### Task 2: `autoSubmit` on the native wire (Rust protocol + bridge)

**Files:**
- Modify: `crates/havenkeys-protocol/src/message.rs:428-436`
- Modify: `crates/havenkeys-bridge/src/dispatch.rs:135-166`
- Modify: `crates/havenkeys-protocol/tests/messages.rs:286,348,357` (literals)
- Test: `crates/havenkeys-bridge/tests/bridge.rs`

**Interfaces:**
- Consumes: `VaultService::auto_sign_in_for` (Task 1).
- Produces: JSON results `{"type":"fill_item","username":..,"password":..,"autoSubmit":bool}` and `{"type":"get_totp","code":..,"period":..,"secondsRemaining":..,"autoSubmit":bool}`.

- [ ] **Step 1: Write the failing bridge test** (append to `bridge.rs`)

```rust
/// autoSubmit comes from the core: the vault setting AND the login's switch.
#[test]
fn fills_carry_auto_submit_from_the_core() {
    let f = fixture();
    let url = "https://github.com/login";
    assert_eq!(fill(&f, f.github, url)["result"]["autoSubmit"], true);
    assert_eq!(totp(&f, f.github, url)["result"]["autoSubmit"], true);
    {
        let mut v = f.vault.lock().unwrap();
        let s = v.settings().unwrap();
        v.update_settings(Settings {
            auto_sign_in: false,
            ..s
        })
        .unwrap();
    }
    assert_eq!(fill(&f, f.github, url)["result"]["autoSubmit"], false);
    assert_eq!(totp(&f, f.github, url)["result"]["autoSubmit"], false);
    // A wrong origin is still denied before anything else.
    assert_eq!(
        error_code(&fill(&f, f.github, "https://evil.com/")),
        Some("denied")
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p havenkeys-bridge fills_carry_auto_submit_from_the_core`
Expected: FAIL, `autoSubmit` is `null`.

- [ ] **Step 3: Add the field to the results** (`message.rs`)

```rust
    FillItem {
        username: Option<String>,
        password: Option<WireSecret>,
        /// Rust's decision that the extension may press the sign-in button.
        auto_submit: bool,
    },
    GetTotp {
        code: WireSecret,
        period: u32,
        seconds_remaining: u32,
        auto_submit: bool,
    },
```

- [ ] **Step 4: Fill it in `dispatch.rs`**

```rust
        Request::FillItem { item_id, url, top_url } => {
            require_enabled(v)?;
            let creds = v
                .fill_for_page(item_id, url, top_url.as_deref(), now_ms(unix_seconds))
                .map_err(item_code)?;
            let auto_submit = v.auto_sign_in_for(item_id).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::FillItem {
                username: creds.username.clone(),
                password: creds
                    .password
                    .as_ref()
                    .map(|p| WireSecret::new(p.expose().to_owned())),
                auto_submit,
            }))
        }
        Request::GetTotp { item_id, url, top_url } => {
            require_enabled(v)?;
            let totp = v
                .totp_for_page(item_id, url, top_url.as_deref(), unix_seconds)
                .map_err(item_code)?;
            let auto_submit = v.auto_sign_in_for(item_id).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::GetTotp {
                code: WireSecret::new(totp.code.expose().to_owned()),
                period: totp.period,
                seconds_remaining: totp.seconds_remaining,
                auto_submit,
            }))
        }
```

Add `auto_submit: false,` to the three `ResultBody::FillItem` / `GetTotp` literals in `crates/havenkeys-protocol/tests/messages.rs`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat(bridge): return autoSubmit with fill_item and get_totp"
```

---

### Task 3: TypeScript protocol parses `autoSubmit`

**Files:**
- Modify: `packages/protocol/src/index.ts:106-107` (types), `:279-286` (parser)
- Modify: `packages/protocol/src/index.test.ts:13-15,49`, `packages/protocol/src/fuzz.test.ts:23-24`
- Modify (fixtures only): `apps/extension/src/background/inline-handler.test.ts` `defaultAnswer`, `apps/extension/src/background/popup-handler.test.ts` (every `fill_item` / `get_totp` answer)

**Interfaces:**
- Produces: `Result` members `{ type: "fill_item"; username: string | null; password: string | null; autoSubmit: boolean }` and `{ type: "get_totp"; code: string; period: number; secondsRemaining: number; autoSubmit: boolean }`.

- [ ] **Step 1: Write the failing test** (in `index.test.ts`, inside the result-parsing `describe`)

```ts
  it("requires a boolean autoSubmit on fills", () => {
    const ok = parseIncoming({ v: 1, id: 1, result: { type: "fill_item", username: "u", password: "p", autoSubmit: true } });
    expect(ok && "result" in ok && ok.result).toEqual({ type: "fill_item", username: "u", password: "p", autoSubmit: true });
    for (const result of [
      { type: "fill_item", username: "u", password: "p" },
      { type: "fill_item", username: "u", password: "p", autoSubmit: "yes" },
      { type: "get_totp", code: "123456", period: 30, secondsRemaining: 1 },
      { type: "get_totp", code: "123456", period: 30, secondsRemaining: 1, autoSubmit: 1 },
    ]) {
      expect(parseIncoming({ v: 1, id: 1, result })).toBeNull();
    }
  });
```

(`parseIncoming` takes the already-decoded object, as in the file's other tests.)

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @havenkeys/protocol test`
Expected: FAIL (missing `autoSubmit` still accepted / extra key rejected).

- [ ] **Step 3: Update types and parser**

```ts
  | { type: "fill_item"; username: string | null; password: string | null; autoSubmit: boolean }
  | { type: "get_totp"; code: string; period: number; secondsRemaining: number; autoSubmit: boolean }
```

```ts
    case "fill_item":
      if (!hasExactKeys(v, ["type", "username", "password", "autoSubmit"])) return null;
      if (!isNullableStr(v.username) || !isNullableStr(v.password) || !isBool(v.autoSubmit)) return null;
      return { type: "fill_item", username: v.username, password: v.password, autoSubmit: v.autoSubmit };
    case "get_totp":
      if (!hasExactKeys(v, ["type", "code", "period", "secondsRemaining", "autoSubmit"])) return null;
      if (!isStr(v.code) || !/^[0-9]{6,8}$/.test(v.code) || !isU32(v.period) || !isU32(v.secondsRemaining)) return null;
      if (!isBool(v.autoSubmit)) return null;
      return { type: "get_totp", code: v.code, period: v.period, secondsRemaining: v.secondsRemaining, autoSubmit: v.autoSubmit };
```

Add `autoSubmit: false` to the existing valid `fill_item` / `get_totp` fixtures in `index.test.ts` and `fuzz.test.ts`, and to every `fill_item` / `get_totp` answer in `apps/extension/src/background/inline-handler.test.ts` (`defaultAnswer`) and `popup-handler.test.ts`.

- [ ] **Step 4: Run the tests and type checks**

Run: `pnpm --filter @havenkeys/protocol test && pnpm --filter @havenkeys/extension test && pnpm -r typecheck`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add packages apps/extension/src/background
git commit -m "feat(protocol): parse autoSubmit on fill_item and get_totp"
```

---

### Task 4: Desktop toggle and per-login switch

**Files:**
- Modify: `apps/desktop/src/lib/types.ts` (`ItemOverview`, `ItemInput`, `Settings`)
- Modify: `apps/desktop/src/views/SettingsView.tsx:222-249`
- Modify: `apps/desktop/src/views/ItemEditor.tsx`

**Interfaces:**
- Consumes: `autoSignIn` on overviews and settings (Task 1); `ItemInput.autoSignIn` accepted by `create_item` / `update_item`.

- [ ] **Step 1: Types** (`types.ts`)

`ItemOverview`: add `autoSignIn: boolean;` after `hasPasskey`.
`ItemInput`: add `autoSignIn?: boolean;` after `content`.
`Settings`: add `autoSignIn: boolean;` after `autoPasskeyUpgrade`.

- [ ] **Step 2: Settings toggle** (`SettingsView.tsx`, a new row after the passkey row inside the *Browser extension* group, and a note after the passkey note)

```tsx
            <div className="row">
              <span className="row-label-inline">Sign in automatically after filling</span>
              <Switch
                label="Sign in automatically after filling"
                checked={settings.autoSignIn}
                onChange={(checked) => void update({ autoSignIn: checked })}
              />
            </div>
```

```tsx
          <p className="group-note">
            After you choose a login, HavenKeys presses the sign-in button and continues through email, password and
            two-factor steps on the same site. Sign-ins that span several pages need in-page suggestions. Each login
            can turn this off.
          </p>
```

- [ ] **Step 3: Per-login switch** (`ItemEditor.tsx`)

Add `Switch` to the component imports (`import { Switch } from "../components/Switch";`) and state after the other `useState` calls:

```tsx
  const [autoSignIn, setAutoSignIn] = useState(existing?.autoSignIn ?? true);
  const [globalAutoSignIn, setGlobalAutoSignIn] = useState(true);

  useEffect(() => {
    let live = true;
    api
      .getSettings()
      .then((s) => live && setGlobalAutoSignIn(s.autoSignIn))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);
```

In `submit`, add `autoSignIn,` to the login branch of `input`.

Render, inside the login-only fragment, right after the *One-time codes* group's closing `</div>`:

```tsx
          <h3 className="group-title">Browser</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">Sign in automatically on this site</span>
              <Switch
                label="Sign in automatically on this site"
                checked={autoSignIn && globalAutoSignIn}
                disabled={!globalAutoSignIn || readOnly}
                onChange={setAutoSignIn}
              />
            </div>
          </div>
          {!globalAutoSignIn && <p className="group-note">Turned off in Settings.</p>}
```

- [ ] **Step 4: Type check and tests**

Run: `pnpm --filter @havenkeys/desktop typecheck && pnpm --filter @havenkeys/desktop test`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop): auto sign-in toggle and per-login switch"
```

---

### Task 5: `autofill/submit.ts` — find and press the sign-in button

**Files:**
- Create: `apps/extension/src/autofill/submit.ts`
- Modify: `apps/extension/src/autofill/text.ts` (move `SUBMIT_WORDS` here), `apps/extension/src/content/index.ts` (import it from `../autofill/text`)
- Test: `apps/extension/src/autofill/submit.test.ts`

**Interfaces:**
- Consumes: `Env`, `isRendered` from `./group`; `hasAny`, `normalize`, `MAX_HINT_CHARS` from `./text`.
- Produces:
  - `type PressStep = "username" | "password" | "otp"`
  - `findSubmitButton(root: ParentNode, field: HTMLInputElement, step: PressStep, env: Env): HTMLElement | null`
  - `hasChallenge(doc: Document, env: Env): boolean`
  - `pressWhenReady(o: { button: HTMLElement; field: HTMLInputElement; step: PressStep; env: Env; doc?: Document; sleep?: (ms: number) => Promise<void> }): Promise<"pressed" | "site_submitted" | "gave_up">`
  - `export const SUBMIT_WORDS` from `./text` (moved, unchanged contents).

- [ ] **Step 1: Move `SUBMIT_WORDS`**

Cut the `SUBMIT_WORDS` array (with its doc comment) from `content/index.ts` into `autofill/text.ts` as `export const SUBMIT_WORDS = [...]`, and add `SUBMIT_WORDS` to content's import from `"../autofill/text"`. Run `pnpm --filter @havenkeys/extension test` — expected: pass (pure move).

- [ ] **Step 2: Write the failing tests** (`submit.test.ts`)

```ts
// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Env } from "./group";
import { findSubmitButton, hasChallenge, pressWhenReady } from "./submit";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}
const env: Env = { isVisible: visible, path: "/" };
const $ = <T extends Element = HTMLInputElement>(sel: string) => document.querySelector(sel) as unknown as T;
const noSleep = async () => undefined;

beforeEach(() => (document.body.innerHTML = ""));

describe("findSubmitButton", () => {
  it("prefers the form's sign-in button over social and recovery buttons", () => {
    document.body.innerHTML = `<form><input name="u"><input name="p" type="password">
      <button type="button">Sign in with Google</button><a role="button">Forgot password?</a>
      <button type="submit" id="go">Sign in</button></form>`;
    expect(findSubmitButton($("form"), $("[name=p]"), "password", env)?.id).toBe("go");
  });

  it("picks Continue over Create account on a username step", () => {
    document.body.innerHTML = `<div id="root"><input name="email" type="email">
      <button id="next">Continue</button><button>Create account</button></div>`;
    expect(findSubmitButton($("#root"), $("[name=email]"), "username", env)?.id).toBe("next");
  });

  it("finds a form-less button just outside the field's container", () => {
    document.body.innerHTML = `<div><div id="root"><input name="email" type="email"></div>
      <div><button id="next">Next</button></div></div>`;
    expect(findSubmitButton($("#root"), $("[name=email]"), "username", env)?.id).toBe("next");
  });

  it("refuses when two candidates are equally good", () => {
    document.body.innerHTML = `<div id="root"><input name="email" type="email">
      <button>Next</button><button>Continue</button></div>`;
    expect(findSubmitButton($("#root"), $("[name=email]"), "username", env)).toBeNull();
  });

  it("ignores hidden buttons and returns null with nothing that qualifies", () => {
    document.body.innerHTML = `<div id="root"><input name="p" type="password">
      <button hidden>Sign in</button><button>Show</button></div>`;
    expect(findSubmitButton($("#root"), $("[name=p]"), "password", env)).toBeNull();
  });

  it("scores an input[type=submit] by its value", () => {
    document.body.innerHTML = `<form><input name="otp"><input type="submit" id="v" value="Verify"></form>`;
    expect(findSubmitButton($("form"), $("[name=otp]"), "otp", env)?.id).toBe("v");
  });
});

describe("hasChallenge", () => {
  it("sees a visible reCAPTCHA checkbox, hCaptcha and Turnstile", () => {
    for (const html of [
      `<iframe src="https://www.google.com/recaptcha/api2/anchor?k=x&size=normal"></iframe>`,
      `<iframe src="https://newassets.hcaptcha.com/captcha/v1/x"></iframe>`,
      `<iframe src="https://challenges.cloudflare.com/cdn-cgi/challenge-platform/x"></iframe>`,
      `<div class="cf-turnstile"></div>`,
    ]) {
      document.body.innerHTML = html;
      expect(hasChallenge(document, env), html).toBe(true);
    }
  });

  it("ignores invisible reCAPTCHA badges, hidden widgets and look-alike hosts", () => {
    for (const html of [
      `<iframe src="https://www.google.com/recaptcha/api2/anchor?k=x&size=invisible"></iframe>`,
      `<div class="g-recaptcha" data-size="invisible"></div>`,
      `<iframe hidden src="https://www.google.com/recaptcha/api2/anchor?k=x"></iframe>`,
      `<iframe src="https://hcaptcha.com.evil.example/x"></iframe>`,
      `<iframe src="https://www.google.com/maps"></iframe>`,
    ]) {
      document.body.innerHTML = html;
      expect(hasChallenge(document, env), html).toBe(false);
    }
  });
});

describe("pressWhenReady", () => {
  it("submits the form through requestSubmit with the button as submitter", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"><button id="go">Sign in</button></form>`;
    let submitter: unknown = null;
    $("form").addEventListener("submit", (e) => {
      e.preventDefault();
      submitter = (e as SubmitEvent).submitter;
    });
    const out = await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep });
    expect(out).toBe("pressed");
    expect(submitter).toBe($("#go"));
  });

  it("clicks a form-less button", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><div role="button" id="go">Log in</div></div>`;
    const click = vi.fn();
    $("#go").addEventListener("click", click);
    await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep });
    expect(click).toHaveBeenCalledOnce();
  });

  it("waits for a disabled button to enable, and gives up after a second", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><button id="go" disabled>Sign in</button></div>`;
    const go = $<HTMLButtonElement>("#go");
    let slept = 0;
    const sleep = async (ms: number) => {
      slept += ms;
      if (slept === 300) go.disabled = false;
    };
    expect(await pressWhenReady({ button: go, field: $("[name=p]"), step: "password", env, sleep })).toBe("pressed");

    go.disabled = true;
    const never = async () => undefined;
    expect(await pressWhenReady({ button: go, field: $("[name=p]"), step: "password", env, sleep: never })).toBe("gave_up");
  });

  it("treats aria-disabled like disabled", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><button id="go" aria-disabled="true">Sign in</button></div>`;
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("gave_up");
  });

  it("does not press when the site submitted the code by itself", async () => {
    document.body.innerHTML = `<form><input name="otp"><button id="v">Verify</button></form>`;
    const click = vi.fn();
    $("#v").addEventListener("click", click);
    const sleep = async () => $("[name=otp]").remove();
    expect(await pressWhenReady({ button: $("#v"), field: $("[name=otp]"), step: "otp", env, sleep })).toBe("site_submitted");
    expect(click).not.toHaveBeenCalled();
  });

  it("does not press while a challenge is showing", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"><div class="h-captcha"></div><button id="go">Sign in</button></form>`;
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("gave_up");
  });
});
```

- [ ] **Step 3: Run to verify failure**

Run: `pnpm --filter @havenkeys/extension test -- submit`
Expected: FAIL, cannot resolve `./submit`.

- [ ] **Step 4: Implement `submit.ts`**

```ts
// Pressing a login form's submit button, for automatic sign-in.
//
// Conservative by design: a button is pressed only when one candidate
// clearly wins; otherwise the fields stay filled and the user presses.
// The DOM is untrusted: its strings are only compared against keyword
// lists. Nothing here reads or writes field values.

import type { Env } from "./group";
import { hasAny, MAX_HINT_CHARS, normalize, SUBMIT_WORDS } from "./text";

export type PressStep = "username" | "password" | "otp";

/** Candidate buttons examined per scope. */
const MAX_BUTTONS = 30;
/** Ancestor levels searched above a form-less group for its button. */
const MAX_SCOPE_CLIMB = 3;
export const PRESS_MIN_SCORE = 60;
export const PRESS_MIN_MARGIN = 20;
export const ENABLE_WAIT_MS = 1000;
export const ENABLE_POLL_MS = 100;
export const OTP_SETTLE_MS = 500;

const CANDIDATES = 'button, input[type="submit"], input[type="image"], [role="button"]';

const STEP_WORDS: Record<PressStep, readonly string[]> = {
  username: ["continue", "next", "proximo", "continuar", "avancar", "seguinte"],
  password: ["sign in", "log in", "login", "signin", "entrar", "acessar", "iniciar sesion"],
  otp: ["verify", "confirm", "submit", "verificar", "confirmar", "enviar"],
};

/** Any of these disqualifies a button: it goes somewhere else. */
const NEGATIVE_WORDS = [
  "forgot", "reset", "create account", "sign up", "signup", "register", "cadastrar", "criar conta",
  "cancel", "cancelar", "back", "voltar", "resend", "reenviar", "show", "mostrar", "another", "outra",
  "passkey", "with google", "with apple", "with facebook", "with microsoft", "with github",
  "com google", "com apple", "com facebook", "com microsoft", "com github",
];

function label(el: Element): string {
  const attr = (n: string) => (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
  const own = el instanceof HTMLInputElement ? el.value.slice(0, MAX_HINT_CHARS) : (el.textContent ?? "").slice(0, MAX_HINT_CHARS);
  return normalize(`${own} ${attr("aria-label")} ${attr("title")}`, MAX_HINT_CHARS * 3);
}

function isSubmitter(el: Element): el is HTMLButtonElement | HTMLInputElement {
  return (
    (el instanceof HTMLButtonElement && el.type === "submit") ||
    (el instanceof HTMLInputElement && (el.type === "submit" || el.type === "image"))
  );
}

function score(el: HTMLElement, field: HTMLInputElement, step: PressStep): number {
  const text = label(el);
  if (hasAny(text, NEGATIVE_WORDS)) return -1;
  let s = 0;
  if (field.form && isSubmitter(el) && el.form === field.form) s += 60;
  if (hasAny(text, STEP_WORDS[step])) s += 50;
  if (hasAny(text, SUBMIT_WORDS)) s += 20;
  if (field.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING) s += 10;
  return s;
}

/** The group's root, then (outside a form) a few ancestors: SPAs often put the button beside the fields' container. */
function scopes(root: ParentNode): ParentNode[] {
  const out: ParentNode[] = [root];
  if (root instanceof HTMLFormElement || !(root instanceof Element)) return out;
  let node: Element | null = root.parentElement;
  for (let i = 0; node && node !== document.body && i < MAX_SCOPE_CLIMB; i++, node = node.parentElement) out.push(node);
  return out;
}

/**
 * The button that submits `field`'s step, or null when none clearly wins.
 * Disabled buttons are candidates: many sites enable theirs only once the
 * input validates, and pressWhenReady waits for that.
 */
export function findSubmitButton(root: ParentNode, field: HTMLInputElement, step: PressStep, env: Env): HTMLElement | null {
  for (const scope of scopes(root)) {
    const buttons = Array.from(scope.querySelectorAll<HTMLElement>(CANDIDATES))
      .slice(0, MAX_BUTTONS)
      .filter((b) => env.isVisible(b));
    if (buttons.length === 0) continue;
    const ranked = buttons.map((b) => ({ b, s: score(b, field, step) })).sort((x, y) => y.s - x.s);
    const [best, second] = ranked;
    if (!best || best.s < PRESS_MIN_SCORE) return null;
    if (second && best.s - second.s < PRESS_MIN_MARGIN) return null;
    return best.b;
  }
  return null;
}

function isChallengeFrame(src: string, base: string): boolean {
  let u: URL;
  try {
    u = new URL(src, base);
  } catch {
    return false;
  }
  if (u.searchParams.get("size") === "invisible") return false;
  const h = u.hostname;
  return (
    ((h === "www.google.com" || h === "www.recaptcha.net") && u.pathname.startsWith("/recaptcha/")) ||
    h === "hcaptcha.com" ||
    h.endsWith(".hcaptcha.com") ||
    h === "challenges.cloudflare.com"
  );
}

/** A visible CAPTCHA or bot challenge. Invisible reCAPTCHA badges do not count. */
export function hasChallenge(doc: Document, env: Env): boolean {
  for (const f of Array.from(doc.querySelectorAll("iframe")).slice(0, 50)) {
    if (env.isVisible(f) && isChallengeFrame(f.getAttribute("src") ?? "", doc.baseURI)) return true;
  }
  return Array.from(doc.querySelectorAll<HTMLElement>(".g-recaptcha, .h-captcha, .cf-turnstile"))
    .slice(0, 10)
    .some((el) => el.getAttribute("data-size") !== "invisible" && env.isVisible(el));
}

function isDisabled(b: HTMLElement): boolean {
  return (b as HTMLButtonElement).disabled === true || b.getAttribute("aria-disabled") === "true";
}

const realSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Press `button` once, when it is usable:
 * * OTP step: first give the site OTP_SETTLE_MS to submit on its own.
 * * Wait up to ENABLE_WAIT_MS for the button to enable.
 * * Never with a challenge on the page.
 * * `form.requestSubmit(button)` for a form's submit button, so the site's
 *   validation and submit handlers run; `click()` otherwise.
 */
export async function pressWhenReady(o: {
  button: HTMLElement;
  field: HTMLInputElement;
  step: PressStep;
  env: Env;
  doc?: Document;
  sleep?: (ms: number) => Promise<void>;
}): Promise<"pressed" | "site_submitted" | "gave_up"> {
  const sleep = o.sleep ?? realSleep;
  const doc = o.doc ?? document;
  if (o.step === "otp") {
    await sleep(OTP_SETTLE_MS);
    if (!o.field.isConnected || !o.env.isVisible(o.field)) return "site_submitted";
  }
  for (let waited = 0; isDisabled(o.button); waited += ENABLE_POLL_MS) {
    if (waited >= ENABLE_WAIT_MS) return "gave_up";
    await sleep(ENABLE_POLL_MS);
  }
  if (!o.button.isConnected || !o.env.isVisible(o.button) || hasChallenge(doc, o.env)) return "gave_up";
  const form = isSubmitter(o.button) ? o.button.form : null;
  if (form && typeof form.requestSubmit === "function") form.requestSubmit(o.button);
  else o.button.click();
  return "pressed";
}
```

- [ ] **Step 5: Run the tests**

Run: `pnpm --filter @havenkeys/extension test -- submit && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS. If the "waits for a disabled button" case fails on the exact 300 ms count, check the loop increments `waited` after each sleep (it re-checks `isDisabled` before sleeping again).

- [ ] **Step 6: Commit**

```bash
git add apps/extension/src/autofill apps/extension/src/content/index.ts
git commit -m "feat(extension): sign-in button scoring and pressing"
```

---

### Task 6: `autofill/watch.ts` — detect the next step

**Files:**
- Create: `apps/extension/src/autofill/watch.ts`
- Test: `apps/extension/src/autofill/watch.test.ts`

**Interfaces:**
- Consumes: `findLoginGroup`, `findOtpGroup` (`./page`); `fieldsOf`, `Env` (`./group`); `valueSource`, `setValue` (`./fill`); `hasChallenge` (`./submit`, Task 5).
- Produces:
  - `type WatchKind = "password" | "otp"`
  - `type WatchResult = { found: WatchKind; field: HTMLInputElement } | { stop: "timeout" | "came_back" | "challenge" }`
  - `detectStep(doc: Document, env: Env, want: WatchKind, filled: HTMLInputElement | null): Seen | null`
  - `watchNext(o: { want: WatchKind; filled: HTMLInputElement | null; env: () => Env; onResult: (r: WatchResult) => void; doc?: Document; timeoutMs?: number }): () => void` (returns cancel)
  - `WATCH_TIMEOUT_MS = 30_000`, `CHECK_DEBOUNCE_MS = 150`

- [ ] **Step 1: Write the failing tests** (`watch.test.ts`)

```ts
// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setValue } from "./fill";
import type { Env } from "./group";
import { CHECK_DEBOUNCE_MS, detectStep, watchNext, WATCH_TIMEOUT_MS, type WatchResult } from "./watch";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}
const env: Env = { isVisible: visible, path: "/" };
const $ = (sel: string) => document.querySelector(sel) as HTMLInputElement;

beforeEach(() => {
  vi.useFakeTimers();
  document.body.innerHTML = "";
});
afterEach(() => vi.useRealTimers());

function watch(want: "password" | "otp", filled: HTMLInputElement | null = null) {
  const results: WatchResult[] = [];
  let checks = 0;
  const cancel = watchNext({ want, filled, env: () => (checks++, env), onResult: (r) => results.push(r) });
  return { results, cancel, checks: () => checks };
}

describe("detectStep", () => {
  it("finds a password page and an OTP page", () => {
    document.body.innerHTML = `<form><input name="p" type="password"><button>Sign in</button></form>`;
    expect(detectStep(document, env, "password", null)?.kind).toBe("password");
    document.body.innerHTML = `<form><input name="otp" autocomplete="one-time-code"></form>`;
    expect(detectStep(document, env, "otp", null)?.kind).toBe("otp");
  });

  it("keeps waiting while the pressed field is still on screen with our value (SPA in flight)", () => {
    document.body.innerHTML = `<form><input name="email" type="email" autocomplete="username"><button>Continue</button></form>`;
    setValue($("[name=email]"), "a@b.c", env);
    expect(detectStep(document, env, "password", $("[name=email]"))).toBeNull();
  });

  it("reports came_back when the password page returns emptied or on a fresh page", () => {
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    const pw = $("[name=p]");
    setValue(pw, "secret", env);
    expect(detectStep(document, env, "otp", pw)).toBeNull();
    pw.value = ""; // the site cleared it after "wrong password"
    expect(detectStep(document, env, "otp", pw)?.kind).toBe("came_back");
    expect(detectStep(document, env, "otp", null)?.kind).toBe("came_back");
  });

  it("reports a visible challenge", () => {
    document.body.innerHTML = `<div class="h-captcha"></div><form><input name="p" type="password"></form>`;
    expect(detectStep(document, env, "password", null)?.kind).toBe("challenge");
  });
});

describe("watchNext", () => {
  it("reports a field present at start once it is stable", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    const w = watch("password");
    expect(w.results).toEqual([]);
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    expect(w.results).toEqual([{ found: "password", field: $("[name=p]") }]);
  });

  it("reports a field inserted later and one revealed by a class change", async () => {
    const w = watch("otp");
    await vi.advanceTimersByTimeAsync(1000);
    document.body.innerHTML = `<style>.off{display:none}</style><form class="off"><input name="otp" autocomplete="one-time-code"></form>`;
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS * 3);
    expect(w.results).toEqual([]);
    document.querySelector("form")?.classList.remove("off");
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS * 3);
    expect(w.results).toEqual([{ found: "otp", field: $("[name=otp]") }]);
  });

  it("ignores a field that shows for one check only", async () => {
    const w = watch("password");
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    document.body.innerHTML = "";
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS * 3);
    expect(w.results).toEqual([]);
  });

  it("stops on came_back, on a challenge, and after the timeout", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    const back = watch("otp", null);
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    expect(back.results).toEqual([{ stop: "came_back" }]);

    document.body.innerHTML = `<div class="cf-turnstile"></div>`;
    const ch = watch("password");
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    expect(ch.results).toEqual([{ stop: "challenge" }]);

    document.body.innerHTML = "";
    const idle = watch("password");
    await vi.advanceTimersByTimeAsync(WATCH_TIMEOUT_MS);
    expect(idle.results).toEqual([{ stop: "timeout" }]);
  });

  it("reports nothing after cancel", async () => {
    const w = watch("password");
    w.cancel();
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    await vi.advanceTimersByTimeAsync(WATCH_TIMEOUT_MS);
    expect(w.results).toEqual([]);
  });

  it("stays bounded under a mutation storm (attack 7)", async () => {
    const w = watch("password");
    for (let i = 0; i < 50; i++) {
      const frag = document.createDocumentFragment();
      for (let j = 0; j < 100; j++) frag.appendChild(document.createElement("input"));
      document.body.appendChild(frag);
      await vi.advanceTimersByTimeAsync(10);
    }
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    // 5,000 inputs over ~500 ms: one check per debounce window, not per mutation.
    expect(w.checks()).toBeLessThanOrEqual(1 + Math.ceil(700 / CHECK_DEBOUNCE_MS) + 1);
    w.cancel();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @havenkeys/extension test -- watch`
Expected: FAIL, cannot resolve `./watch`.

- [ ] **Step 3: Implement `watch.ts`**

```ts
// Watching for the next step of an automatic sign-in: the password page
// after a username step, or the one-time-code page after the password.
//
// Runs only during a sign-in run, for at most WATCH_TIMEOUT_MS. One
// MutationObserver, debounced to one bounded check per CHECK_DEBOUNCE_MS;
// each check uses the same capped page lookups as the popup fill, so a page
// with thousands of nodes costs the same as a small one (attack 7). A field
// counts only when the same element is found by two consecutive checks.

import { valueSource } from "./fill";
import { fieldsOf, type Env } from "./group";
import { findLoginGroup, findOtpGroup } from "./page";
import { hasChallenge } from "./submit";

export type WatchKind = "password" | "otp";
export type WatchResult = { found: WatchKind; field: HTMLInputElement } | { stop: "timeout" | "came_back" | "challenge" };

export const WATCH_TIMEOUT_MS = 30_000;
export const CHECK_DEBOUNCE_MS = 150;

export type Seen =
  | { kind: WatchKind; el: HTMLInputElement }
  | { kind: "came_back"; el: HTMLInputElement }
  | { kind: "challenge"; el: null };

/**
 * What the page shows now, for a run waiting for `want`. `filled` is the
 * field we filled for the previous step on this page (null on a freshly
 * loaded page). The previous step's field "came back" when it is a
 * different element, or no longer holds our value: the site re-rendered it
 * after rejecting the submission.
 */
export function detectStep(doc: Document, env: Env, want: WatchKind, filled: HTMLInputElement | null): Seen | null {
  if (hasChallenge(doc, env)) return { kind: "challenge", el: null };
  const login = findLoginGroup(doc, env);
  const [pw] = login ? fieldsOf(login, "current-password", "password") : [];
  const [user] = login ? fieldsOf(login, "username") : [];
  const cameBack = (el: HTMLInputElement) => el !== filled || valueSource(el) !== "vault";
  if (want === "password") {
    if (pw) return { kind: "password", el: pw };
    if (user && cameBack(user)) return { kind: "came_back", el: user };
    return null;
  }
  const otp = findOtpGroup(doc, env);
  const [box] = otp ? fieldsOf(otp, "otp") : [];
  if (box) return { kind: "otp", el: box };
  if (pw && cameBack(pw)) return { kind: "came_back", el: pw };
  return null;
}

/** Watch for `want`; calls `onResult` once. Returns a cancel function that suppresses it. */
export function watchNext(o: {
  want: WatchKind;
  filled: HTMLInputElement | null;
  env: () => Env;
  onResult: (r: WatchResult) => void;
  doc?: Document;
  timeoutMs?: number;
}): () => void {
  const doc = o.doc ?? document;
  let last: Seen | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let done = false;

  const observer = new MutationObserver(() => schedule());
  const deadline = setTimeout(() => finish({ stop: "timeout" }), o.timeoutMs ?? WATCH_TIMEOUT_MS);

  function cancel(): void {
    done = true;
    observer.disconnect();
    clearTimeout(deadline);
    if (timer) clearTimeout(timer);
    timer = null;
  }

  function finish(r: WatchResult): void {
    if (done) return;
    cancel();
    o.onResult(r);
  }

  function schedule(): void {
    if (done || timer) return;
    timer = setTimeout(() => {
      timer = null;
      check();
    }, CHECK_DEBOUNCE_MS);
  }

  function check(): void {
    if (done) return;
    const seen = detectStep(doc, o.env(), o.want, o.filled);
    if (seen && last && seen.kind === last.kind && seen.el === last.el) {
      if (seen.kind === "challenge") return finish({ stop: "challenge" });
      if (seen.kind === "came_back") return finish({ stop: "came_back" });
      return finish({ found: seen.kind, field: seen.el });
    }
    last = seen;
    // Confirm on the next check even if nothing else mutates.
    if (seen) schedule();
  }

  observer.observe(doc.documentElement, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["class", "style", "hidden", "disabled", "type"],
  });
  check();
  return cancel;
}
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test -- watch && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS. If the class-change case never fires, confirm `getComputedStyle` in jsdom honours the `<style>` rule (it does for `display`); if not, use `hidden` toggling in that test instead of a class.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/watch.ts apps/extension/src/autofill/watch.test.ts
git commit -m "feat(extension): next-step watcher for automatic sign-in"
```

---

### Task 7: Run messages (`messaging/inline.ts`)

**Files:**
- Modify: `apps/extension/src/messaging/inline.ts`
- Modify: `apps/extension/src/background/inline-handler.ts` `fill()` (pass the two new `bg_fill` fields so it type-checks: `submit: false, totp: false` for now)
- Modify: `apps/extension/src/content/content.test.ts` (every `bg_fill` literal gains `submit: false, totp: false`)
- Test: `apps/extension/src/background/inline-handler.test.ts` (`describe("message validation")`)

**Interfaces:**
- Produces (exported from `messaging/inline.ts`):
  - `type RunStep = "username" | "password" | "otp"`, `type NextStep = "password" | "otp"`
  - `ContentRequest` adds `{ type: "cs_run_step"; kind: NextStep }` and `{ type: "cs_run_stop" }`
  - `ReadyReply = { saveToken: string | null; watch: NextStep | null }`
  - `bg_fill` becomes `{ type: "bg_fill"; origin: string; token: string | null; fill: FillPayload; submit: boolean; totp: boolean }` (`submit`: press after filling; `totp`: an OTP step follows the password step)
  - `BackgroundToContent` adds `{ type: "bg_run_end" }`
  - `FillReply = { filled: number; pressing: RunStep | null }`
  - `parseFillReply(v: unknown): FillReply` (invalid → `{ filled: 0, pressing: null }`)

- [ ] **Step 1: Write the failing tests** (append inside `describe("message validation")`)

```ts
  it("validates run messages", () => {
    expect(parseContentRequest({ type: "cs_run_step", kind: "password" })).toEqual({ type: "cs_run_step", kind: "password" });
    expect(parseContentRequest({ type: "cs_run_step", kind: "otp" })).toEqual({ type: "cs_run_step", kind: "otp" });
    expect(parseContentRequest({ type: "cs_run_stop" })).toEqual({ type: "cs_run_stop" });
    for (const bad of [
      { type: "cs_run_step", kind: "username" },
      { type: "cs_run_step", kind: "password", itemId: GH },
      { type: "cs_run_step" },
      { type: "cs_run_stop", reason: "x" },
    ]) {
      expect(parseContentRequest(bad)).toBeNull();
    }
    const fillMsg = { type: "bg_fill", origin: "https://github.com", token: null, fill: { kind: "otp", code: "123456" }, submit: true, totp: false };
    expect(parseBackgroundMessage(fillMsg)).toEqual(fillMsg);
    expect(parseBackgroundMessage({ ...fillMsg, submit: "yes" })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://github.com", token: null, fill: fillMsg.fill })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_run_end" })).toEqual({ type: "bg_run_end" });
    expect(parseBackgroundMessage({ type: "bg_run_end", token: T1 })).toBeNull();
  });

  it("reads fill replies defensively", () => {
    expect(parseFillReply({ filled: 2, pressing: "password" })).toEqual({ filled: 2, pressing: "password" });
    expect(parseFillReply({ filled: 1, pressing: null })).toEqual({ filled: 1, pressing: null });
    for (const bad of [undefined, null, { filled: "2" }, { filled: 2, pressing: "everything" }, { filled: -1, pressing: null }]) {
      expect(parseFillReply(bad)).toEqual({ filled: 0, pressing: null });
    }
  });
```

Add `parseFillReply` to the test file's import from `"../messaging/inline"`.

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @havenkeys/extension test -- inline-handler`
Expected: FAIL (`parseFillReply` missing; new types rejected).

- [ ] **Step 3: Implement**

Types (near the top, after `MenuKind`):

```ts
/** Steps of an automatic sign-in, in order. */
export type RunStep = "username" | "password" | "otp";
/** Steps a run can continue to. */
export type NextStep = "password" | "otp";
const NEXT_STEPS: readonly NextStep[] = ["password", "otp"];
const RUN_STEPS: readonly RunStep[] = ["username", "password", "otp"];
```

`ContentRequest`: add

```ts
  /** The next step's field appeared in this frame during a sign-in run. */
  | { type: "cs_run_step"; kind: NextStep }
  /** The run should end here: the user took over, a stop condition, or nothing appeared. */
  | { type: "cs_run_stop" }
```

`ReadyReply`: `export type ReadyReply = { saveToken: string | null; watch: NextStep | null };`

`BackgroundToContent`: change `bg_fill` to

```ts
  | { type: "bg_fill"; origin: string; token: string | null; fill: FillPayload; submit: boolean; totp: boolean }
```

(extend its doc comment: "`submit`: press the page's button after filling (Rust's autoSubmit). `totp`: after a password step, watch for a one-time-code step.") and add `| { type: "bg_run_end" }`.

`FillReply`: `export type FillReply = { filled: number; pressing: RunStep | null };`

In `parseContentRequest`:

```ts
    case "cs_run_step":
      return keysAre(o, ["type", "kind"]) && NEXT_STEPS.includes(o.kind as NextStep)
        ? { type: "cs_run_step", kind: o.kind as NextStep }
        : null;
    case "cs_run_stop":
      return keysAre(o, ["type"]) ? { type: "cs_run_stop" } : null;
```

In `parseBackgroundMessage`, replace the `bg_fill` case and add `bg_run_end`:

```ts
    case "bg_fill": {
      if (!keysAre(o, ["type", "origin", "token", "fill", "submit", "totp"]) || typeof o.origin !== "string") return null;
      if (o.token !== null && !isToken(o.token)) return null;
      if (typeof o.submit !== "boolean" || typeof o.totp !== "boolean") return null;
      const fill = parseFill(o.fill);
      return fill && { type: "bg_fill", origin: o.origin, token: o.token, fill, submit: o.submit, totp: o.totp };
    }
    case "bg_run_end":
      return keysAre(o, ["type"]) ? { type: "bg_run_end" } : null;
```

New export:

```ts
/** A content script's reply to bg_fill; anything malformed reads as "nothing filled". */
export function parseFillReply(v: unknown): FillReply {
  const o = obj(v);
  if (!o || typeof o.filled !== "number" || !Number.isInteger(o.filled) || o.filled < 0) return { filled: 0, pressing: null };
  if (o.pressing !== null && !RUN_STEPS.includes(o.pressing as RunStep)) return { filled: 0, pressing: null };
  return { filled: o.filled, pressing: o.pressing as RunStep | null };
}
```

In `inline-handler.ts` `fill()`, send `submit: false, totp: false` and read the reply with `parseFillReply(reply).filled` (temporary; Task 9 replaces this). Update `ready()` to return `{ saveToken: ..., watch: null }` in all branches, and update existing `cs_ready` expectations in `inline-handler.test.ts` to include `watch: null`. Update the `bg_fill` literals in `content.test.ts` with `submit: false, totp: false`, and in `content/index.ts` return `{ filled: handleFill(m), pressing: null }` so the reply shape matches (Task 10 replaces this).

- [ ] **Step 4: Run tests and type check**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src
git commit -m "feat(extension): message types for sign-in runs"
```

---

### Task 8: `background/signin-run.ts` — the run registry

**Files:**
- Create: `apps/extension/src/background/signin-run.ts`
- Test: `apps/extension/src/background/signin-run.test.ts`

**Interfaces:**
- Consumes: `RunStep`, `NextStep` from `../messaging/inline` (Task 7).
- Produces:
  - `RUN_TTL_MS = 120_000`
  - `nextStep(step: RunStep, hasTotp: boolean): NextStep | null`
  - `interface RunFrame { tabId: number; frameId: number; origin: string }`
  - `interface Run { tabId: number; frameId: number; origin: string; itemId: string; step: RunStep; hasTotp: boolean; expires: number }`
  - `createRuns(now: () => number)` returning `{ start(frame, itemId, pressed, hasTotp): Run | null; accept(frame, kind): Run | null; watchFor(frame): NextStep | null; stop(frame): Run | null; end(tabId): Run | null; clear(): Run[]; get(tabId): Run | null }`
  - `type Runs = ReturnType<typeof createRuns>`

- [ ] **Step 1: Write the failing tests** (`signin-run.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { createRuns, nextStep, RUN_TTL_MS, type RunFrame } from "./signin-run";

const ITEM = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const f = (over: Partial<RunFrame> = {}): RunFrame => ({ tabId: 1, frameId: 0, origin: "https://www.amazon.com", ...over });

function setup() {
  let clock = 1_000;
  return { runs: createRuns(() => clock), advance: (ms: number) => (clock += ms) };
}

describe("nextStep", () => {
  it("goes username → password → otp (only with TOTP) → end", () => {
    expect(nextStep("username", false)).toBe("password");
    expect(nextStep("password", true)).toBe("otp");
    expect(nextStep("password", false)).toBeNull();
    expect(nextStep("otp", true)).toBeNull();
  });
});

describe("runs", () => {
  it("walks an Amazon-style flow", () => {
    const { runs } = setup();
    expect(runs.start(f(), ITEM, "username", true)).not.toBeNull();
    expect(runs.watchFor(f())).toBe("password"); // the password page loaded
    expect(runs.accept(f(), "password")?.itemId).toBe(ITEM);
    expect(runs.watchFor(f())).toBe("otp");
    expect(runs.accept(f(), "otp")?.step).toBe("otp");
  });

  it("keeps no run when the pressed step was the last", () => {
    const { runs } = setup();
    expect(runs.start(f(), ITEM, "password", false)).toBeNull();
    expect(runs.start(f(), ITEM, "otp", true)).toBeNull();
    expect(runs.get(1)).toBeNull();
  });

  it("ignores out-of-order, repeated, and foreign steps", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", true);
    expect(runs.accept(f(), "otp")).toBeNull(); // skips password
    expect(runs.accept(f({ tabId: 2 }), "password")).toBeNull(); // another tab
    expect(runs.accept(f({ frameId: 3 }), "password")).toBeNull(); // another frame
    expect(runs.accept(f({ origin: "https://amazon.com.evil.example" }), "password")).toBeNull();
    expect(runs.accept(f({ origin: "https://smile.amazon.com" }), "password")).toBeNull(); // same site, other origin
    expect(runs.accept(f(), "password")).not.toBeNull();
    expect(runs.accept(f(), "password")).toBeNull(); // once
  });

  it("expires after two minutes", () => {
    const { runs, advance } = setup();
    runs.start(f(), ITEM, "username", false);
    advance(RUN_TTL_MS);
    expect(runs.watchFor(f())).toBeNull();
    expect(runs.accept(f(), "password")).toBeNull();
  });

  it("ends when the run's frame loads another origin", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", false);
    expect(runs.watchFor(f({ origin: "https://evil.example" }))).toBeNull();
    expect(runs.get(1)).toBeNull();
  });

  it("does not let other frames watch or stop the run", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", false);
    expect(runs.watchFor(f({ frameId: 7, origin: "https://ads.example" }))).toBeNull();
    expect(runs.stop(f({ frameId: 7 }))).toBeNull();
    expect(runs.get(1)).not.toBeNull();
    expect(runs.stop(f())).not.toBeNull();
    expect(runs.get(1)).toBeNull();
  });

  it("a new start replaces the run; clear drops all", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", false);
    runs.start(f(), "11111111-2222-4333-8444-555555555555", "username", false);
    expect(runs.get(1)?.itemId).toBe("11111111-2222-4333-8444-555555555555");
    runs.start(f({ tabId: 2 }), ITEM, "username", false);
    expect(runs.clear()).toHaveLength(2);
    expect(runs.get(1)).toBeNull();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @havenkeys/extension test -- signin-run`
Expected: FAIL, cannot resolve `./signin-run`.

- [ ] **Step 3: Implement `signin-run.ts`**

```ts
// Automatic sign-in runs: which login the user picked in a tab, and which
// step of its sign-in comes next. Pure state: no chrome.*, no secrets.
//
// A run starts only from a pick (menu or popup) whose fill Rust marked
// autoSubmit, and only once the content script said it is pressing the
// page's button. It is bound to the tab, frame and exact origin of that
// pick, moves forward only (username → password → otp), accepts each step
// once, and lasts at most RUN_TTL_MS. Every value a continuation fills
// still comes from the desktop's origin-checked fill_item / get_totp.

import type { NextStep, RunStep } from "../messaging/inline";

export const RUN_TTL_MS = 2 * 60_000;

export interface RunFrame {
  tabId: number;
  frameId: number;
  origin: string;
}

export interface Run extends RunFrame {
  itemId: string;
  step: RunStep;
  hasTotp: boolean;
  expires: number;
}

export function nextStep(step: RunStep, hasTotp: boolean): NextStep | null {
  if (step === "username") return "password";
  if (step === "password") return hasTotp ? "otp" : null;
  return null;
}

export function createRuns(now: () => number) {
  const runs = new Map<number, Run>(); // by tab

  function get(tabId: number): Run | null {
    const r = runs.get(tabId);
    if (!r) return null;
    if (r.expires <= now()) {
      runs.delete(tabId);
      return null;
    }
    return r;
  }

  const sameFrame = (r: Run, f: RunFrame) => r.frameId === f.frameId && r.origin === f.origin;

  /** A pick's first step was pressed. Replaces the tab's run; null when that step was the last. */
  function start(frame: RunFrame, itemId: string, pressed: RunStep, hasTotp: boolean): Run | null {
    runs.delete(frame.tabId);
    if (nextStep(pressed, hasTotp) === null) return null;
    const run: Run = { ...frame, itemId, step: pressed, hasTotp, expires: now() + RUN_TTL_MS };
    runs.set(frame.tabId, run);
    return run;
  }

  /** cs_run_step: the run to continue (now at `kind`), or null to ignore the message. */
  function accept(frame: RunFrame, kind: NextStep): Run | null {
    const r = get(frame.tabId);
    if (!r || !sameFrame(r, frame) || nextStep(r.step, r.hasTotp) !== kind) return null;
    r.step = kind;
    return r;
  }

  /** cs_ready: what a (re)loaded frame should watch for. The run's frame on another origin ends it. */
  function watchFor(frame: RunFrame): NextStep | null {
    const r = get(frame.tabId);
    if (!r || r.frameId !== frame.frameId) return null;
    if (r.origin !== frame.origin) {
      runs.delete(frame.tabId);
      return null;
    }
    return nextStep(r.step, r.hasTotp);
  }

  /** cs_run_stop from the run's own frame. */
  function stop(frame: RunFrame): Run | null {
    const r = runs.get(frame.tabId);
    if (!r || !sameFrame(r, frame)) return null;
    runs.delete(frame.tabId);
    return r;
  }

  function end(tabId: number): Run | null {
    const r = runs.get(tabId) ?? null;
    runs.delete(tabId);
    return r;
  }

  function clear(): Run[] {
    const all = [...runs.values()];
    runs.clear();
    return all;
  }

  return { start, accept, watchFor, stop, end, clear, get };
}

export type Runs = ReturnType<typeof createRuns>;
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test -- signin-run && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/background/signin-run.ts apps/extension/src/background/signin-run.test.ts
git commit -m "feat(extension): sign-in run registry"
```

---

### Task 9: Background wiring (inline handler, popup, index)

**Files:**
- Modify: `apps/extension/src/background/inline-handler.ts`
- Modify: `apps/extension/src/background/popup-handler.ts`
- Modify: `apps/extension/src/background/index.ts:78-88`
- Test: `apps/extension/src/background/inline-handler.test.ts`, `apps/extension/src/background/popup-handler.test.ts`

**Interfaces:**
- Consumes: `createRuns`, `nextStep` (Task 8); `parseFillReply`, `FillReply`, `RunStep`, `NextStep` (Task 7); `autoSubmit` on results (Task 3).
- Produces:
  - `InlineHandler.pickFill(frame: FrameRef, token: string | null, payload: FillPayload, auto: AutoRun | null): Promise<number>` where `export interface AutoRun { itemId: string; hasTotp: boolean }` (exported from `inline-handler.ts`)
  - `TabFiller = (tabId: number, pageUrl: string, payload: FillPayload, auto?: AutoRun | null) => Promise<number>`
  - `handleContent` answers `cs_run_step`, `cs_run_stop`; `cs_ready` reply includes `watch`.

- [ ] **Step 1: Write the failing tests** (append to `inline-handler.test.ts`)

Change `setup()`'s `sendToFrame` so tests can script the content script's reply to `bg_fill`:

```ts
function setup(answer: (r: Request) => unknown = defaultAnswer, extra: Partial<InlineDeps> = {}, pressing: (msg: BackgroundToContent) => string | null = () => null) {
  // ...unchanged...
    sendToFrame: async (to, msg) => {
      sent.push({ to: { tabId: to.tabId, frameId: to.frameId }, msg: msg as BackgroundToContent });
      return msg.type === "bg_fill" ? { filled: 2, pressing: pressing(msg as BackgroundToContent) } : undefined;
    },
```

Tests:

```ts
describe("automatic sign-in", () => {
  const auto = (r: Request): unknown => {
    const base = defaultAnswer(r) as Record<string, unknown>;
    return r.type === "fill_item" || r.type === "get_totp" ? { ...base, autoSubmit: true } : base;
  };
  const pressWhenSubmit = (msg: BackgroundToContent) =>
    msg.type === "bg_fill" && msg.submit ? (msg.fill.kind === "otp" ? "otp" : msg.fill.username !== null ? "username" : "password") : null;

  async function pick(h: ReturnType<typeof setup>["h"]) {
    const open = (await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })) as { token: string };
    await h.handleInline(1, { type: "menu_pick", token: open.token, itemId: GH });
  }

  it("asks the content script to press only when Rust says autoSubmit", async () => {
    const off = setup();
    await pick(off.h);
    expect(off.sent.find((s) => s.msg.type === "bg_fill")?.msg).toMatchObject({ submit: false, totp: true });

    const on = setup(auto);
    await pick(on.h);
    expect(on.sent.find((s) => s.msg.type === "bg_fill")?.msg).toMatchObject({ submit: true, totp: true });
  });

  it("continues username → password → otp on the same origin, with Rust checks each step", async () => {
    const { h, requests, sent } = setup(auto, {}, pressWhenSubmit);
    await pick(h); // username-only page: content reports pressing "username"
    expect(await h.handleContent(frame({ url: "https://github.com/login/password" }), { type: "cs_ready" })).toEqual({
      saveToken: null,
      watch: "password",
    });
    await h.handleContent(frame({ url: "https://github.com/login/password" }), { type: "cs_run_step", kind: "password" });
    const pwFill = sent.filter((s) => s.msg.type === "bg_fill").at(-1)?.msg;
    expect(pwFill).toMatchObject({ token: null, submit: true, fill: { kind: "login", username: null, password: "pw" } });
    expect(requests.filter((r) => r.type === "fill_item").at(-1)).toMatchObject({ url: "https://github.com/login/password" });

    await h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(requests.at(-1)).toMatchObject({ type: "get_totp", itemId: GH });
    expect(sent.filter((s) => s.msg.type === "bg_fill").at(-1)?.msg).toMatchObject({ fill: { kind: "otp", code: "123456" }, submit: true });
    // The run is over: a repeat is ignored without asking the desktop.
    const before = requests.length;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(requests.length).toBe(before);
  });

  it("ignores steps from another origin, another tab, or with no run", async () => {
    const { h, requests } = setup(auto, {}, pressWhenSubmit);
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" }); // no pick yet
    await pick(h);
    const before = requests.length;
    await h.handleContent(frame({ url: "https://gist.github.com/", origin: "https://gist.github.com" }), { type: "cs_run_step", kind: "password" });
    await h.handleContent(frame({ tabId: 2 }), { type: "cs_run_step", kind: "password" });
    expect(requests.length).toBe(before);
  });

  it("drops a late step after expiry or after another pick in the tab", async () => {
    const { h, requests, advance } = setup(auto, {}, pressWhenSubmit);
    await pick(h);
    advance(2 * 60_000);
    const before = requests.length;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    expect(requests.length).toBe(before);
  });

  it("stops on cs_run_stop, on lock, and when the content script did not press", async () => {
    const a = setup(auto, {}, pressWhenSubmit);
    await pick(a.h);
    await a.h.handleContent(frame(), { type: "cs_run_stop" });
    expect(await a.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });

    const b = setup(auto, {}, pressWhenSubmit);
    await pick(b.h);
    b.h.reset();
    expect(await b.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });
    expect(b.sent.some((s) => s.msg.type === "bg_run_end")).toBe(true);

    const c = setup(auto); // content never presses
    await pick(c.h);
    expect(await c.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });
  });

  it("fills without pressing and ends when autoSubmit turns off mid-run", async () => {
    let on = true;
    const answer = (r: Request): unknown => {
      const base = defaultAnswer(r) as Record<string, unknown>;
      return r.type === "fill_item" || r.type === "get_totp" ? { ...base, autoSubmit: on } : base;
    };
    const { h, sent } = setup(answer, {}, pressWhenSubmit);
    await pick(h);
    on = false;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    expect(sent.filter((s) => s.msg.type === "bg_fill").at(-1)?.msg).toMatchObject({ submit: false });
    expect(await h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });
  });
});
```

Note: `pick()` fills `{ username: "octo", password: "pw" }`, so `pressWhenSubmit` reports `"username"` for the first fill (username !== null). That stands in for a username-only page.

In `popup-handler.test.ts`, the existing "fills the active tab using the tab's own URL" test's fake answer gains `autoSubmit: false`, and its expectation becomes `[[7, "https://github.com/login", { kind: "login", username: "octo", password: "pw" }, null]]` (the 4th argument is now always passed). Append inside `describe("popup handler")`:

```ts
  it("passes a run to the tab filler when Rust says autoSubmit, with the item's TOTP flag", async () => {
    const c = fakeClient((r) =>
      r.type === "fill_item"
        ? { type: "fill_item", username: "octo", password: "pw", autoSubmit: true }
        : { type: "find_matches", matches: [{ id: ID, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host" }] },
    );
    const fills: unknown[][] = [];
    const h = createPopupHandler(c, async () => ({ id: 7, url: "https://github.com/login" }), async (...a) => {
      fills.push(a);
      return 2;
    });
    await h.handle({ type: "popup_fill", itemId: ID });
    expect(fills[0]?.[3]).toEqual({ itemId: ID, hasTotp: true });
    expect(c.seen.map((r) => r.type)).toEqual(["fill_item", "find_matches"]);
  });

  it("passes a code-only run for a popup TOTP fill", async () => {
    const c = fakeClient(() => ({ type: "get_totp", code: "123456", period: 30, secondsRemaining: 9, autoSubmit: true }));
    const fills: unknown[][] = [];
    const h = createPopupHandler(c, async () => ({ id: 7, url: "https://github.com/login" }), async (...a) => {
      fills.push(a);
      return 1;
    });
    await h.handle({ type: "popup_fill_totp", itemId: ID });
    expect(fills[0]?.[3]).toEqual({ itemId: ID, hasTotp: true });
  });
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @havenkeys/extension test -- inline-handler popup-handler`
Expected: FAIL (`submit` always false, `cs_run_step` unknown in `handleContent`).

- [ ] **Step 3: Implement in `inline-handler.ts`**

Imports: add `parseFillReply`, `type NextStep`, `type RunStep` from `"../messaging/inline"`; `import { createRuns, nextStep } from "./signin-run";`.

Export near `FrameRef`:

```ts
/** A pick whose fill Rust marked autoSubmit: the run it may start. */
export interface AutoRun {
  itemId: string;
  hasTotp: boolean;
}
```

Extend the header comment's trust-model list with:

```ts
// * A sign-in run (signin-run.ts) starts only from a pick whose fill Rust
//   marked autoSubmit, is bound to that tab, frame and origin, and every
//   continuation fill is re-requested from the desktop for the frame's URL.
```

Inside `createInlineHandler`, after the maps:

```ts
  const runs = createRuns(deps.now);

  function endRun(tabId: number): void {
    const r = runs.end(tabId);
    if (r) void deps.sendToFrame({ tabId, frameId: r.frameId }, { type: "bg_run_end" });
  }
```

Replace `fill` with a low-level sender and a pick entry point:

```ts
  async function sendFill(frame: FrameRef, token: string | null, payload: FillPayload, submit: boolean, totp: boolean) {
    const reply = await deps.sendToFrame(frame, { type: "bg_fill", origin: frame.origin, token, fill: payload, submit, totp });
    return parseFillReply(reply);
  }

  /**
   * A fill the user picked (menu or popup). Ends any run in the tab; with
   * `auto`, asks the content script to press, and starts a run if it did.
   */
  async function pickFill(frame: FrameRef, token: string | null, payload: FillPayload, auto: AutoRun | null): Promise<number> {
    endRun(frame.tabId);
    const r = await sendFill(frame, token, payload, auto !== null, auto?.hasTotp ?? false);
    if (auto && r.pressing) runs.start(frame, auto.itemId, r.pressing, auto.hasTotp);
    return r.filled;
  }
```

In `menu_pick`, replace the try block:

```ts
        try {
          const offered = m.items.find((i) => i.id === req.itemId);
          if (m.kind === "otp") {
            const t = await deps.client.request({ type: "get_totp", itemId: req.itemId, ...frameFields(m.frame) });
            await pickFill(m.frame, m.token, { kind: "otp", code: t.code }, t.autoSubmit ? { itemId: req.itemId, hasTotp: true } : null);
          } else {
            const c = await deps.client.request({ type: "fill_item", itemId: req.itemId, ...frameFields(m.frame) });
            const auto = c.autoSubmit ? { itemId: req.itemId, hasTotp: offered?.hasTotp ?? false } : null;
            await pickFill(m.frame, m.token, { kind: "login", username: c.username, password: c.password }, auto);
          }
        } catch (e) {
          return fail(e);
        }
```

In `menu_generate`: `await pickFill(m.frame, m.token, { kind: "generated", password: g.password }, null);`

Continuation:

```ts
  /** cs_run_step: the next step's field appeared in the run's frame. */
  async function continueRun(frame: FrameRef, kind: NextStep): Promise<void> {
    const run = runs.accept(frame, kind);
    if (!run) return;
    let payload: FillPayload;
    let auto: boolean;
    try {
      if (kind === "password") {
        const c = await deps.client.request({ type: "fill_item", itemId: run.itemId, ...frameFields(frame) });
        if (c.password === null) return endRun(frame.tabId);
        payload = { kind: "login", username: null, password: c.password };
        auto = c.autoSubmit;
      } else {
        const t = await deps.client.request({ type: "get_totp", itemId: run.itemId, ...frameFields(frame) });
        payload = { kind: "otp", code: t.code };
        auto = t.autoSubmit;
      }
    } catch {
      return endRun(frame.tabId); // locked, denied, gone: stop quietly
    }
    const r = await sendFill(frame, null, payload, auto, run.hasTotp);
    if (!auto || r.pressing !== kind || nextStep(kind, run.hasTotp) === null) endRun(frame.tabId);
  }
```

`ready(frame)`:

```ts
  function ready(frame: FrameRef): ReadyReply {
    const watch = runs.watchFor(frame);
    if (frame.frameId !== 0) return { saveToken: null, watch };
    const s = saves.get(frame.tabId);
    if (!s || s.expires <= deps.now()) {
      dropSave(frame.tabId, false);
      return { saveToken: null, watch };
    }
    return { saveToken: s.token, watch };
  }
```

`handleContent` new cases:

```ts
      case "cs_run_step":
        await continueRun(frame, req.kind);
        return {};
      case "cs_run_stop":
        runs.stop(frame);
        return {};
```

`reset()`: add `for (const r of runs.clear()) void deps.sendToFrame({ tabId: r.tabId, frameId: r.frameId }, { type: "bg_run_end" });`
`forgetTab()`: add `runs.end(tabId);`
Return: `return { handleContent, handleInline, pickFill, reset, forgetTab };`

- [ ] **Step 4: Popup and index**

`popup-handler.ts`: import `type AutoRun` from `"./inline-handler"`; change `TabFiller` to

```ts
export type TabFiller = (tabId: number, pageUrl: string, payload: FillPayload, auto?: AutoRun | null) => Promise<number>;
```

In `fillFromPopup`:

```ts
      if (totp) {
        const t = await client.request({ type: "get_totp", itemId, url });
        filled = await fillTab(tab.id, url, { kind: "otp", code: t.code }, t.autoSubmit ? { itemId, hasTotp: true } : null);
      } else {
        const c = await client.request({ type: "fill_item", itemId, url });
        const auto = c.autoSubmit ? { itemId, hasTotp: await hasTotp(itemId, url) } : null;
        filled = await fillTab(tab.id, url, { kind: "login", username: c.username, password: c.password }, auto);
      }
```

with, inside `createPopupHandler`:

```ts
  /** Whether a login has TOTP, for the run's OTP step. A lookup, no secrets. */
  async function hasTotp(itemId: string, url: string): Promise<boolean> {
    try {
      const { matches } = await client.request({ type: "find_matches", url });
      return matches.find((m) => m.id === itemId)?.hasTotp ?? false;
    } catch {
      return false;
    }
  }
```

`index.ts` `fillTab`:

```ts
async function fillTab(tabId: number, pageUrl: string, payload: FillPayload, auto: AutoRun | null = null): Promise<number> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
  } catch {
    return 0;
  }
  const frame: FrameRef = { tabId, frameId: 0, url: pageUrl, origin: new URL(pageUrl).origin };
  return inline.pickFill(frame, null, payload, auto);
}
```

and import `type FillPayload` from `"../messaging/inline"` and `type AutoRun` from `"./inline-handler"`.

Update any remaining `inline.fill` / `h.fill` references in tests to `pickFill(frame, token, payload, null)`.

- [ ] **Step 5: Run tests and type check**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/extension/src/background
git commit -m "feat(extension): start, continue and end sign-in runs in the background"
```

---

### Task 10: Content script wiring

**Files:**
- Modify: `apps/extension/src/content/index.ts`
- Test: `apps/extension/src/content/content.test.ts`

**Interfaces:**
- Consumes: `findSubmitButton`, `hasChallenge`, `pressWhenReady`, `PressStep` (Task 5); `watchNext`, `WatchKind` (Task 6); `FillReply`, `RunStep`, `NextStep` (Task 7); `fieldsOf` (`../autofill/group`), `valueSource` (`../autofill/fill`).
- Produces: content replies to `bg_fill` with `FillReply`; sends `cs_run_step` / `cs_run_stop`; handles `bg_run_end`; `cs_ready` from every frame.

- [ ] **Step 1: Write the failing tests** (append to `content.test.ts`; the beforeEach form has an email, a password and a "Sign in" submit button)

```ts
describe("automatic sign-in", () => {
  const autoFill = (over: Record<string, unknown> = {}) => ({ ...loginFill(location.origin), submit: true, totp: false, ...over });

  it("fills, reports the step it presses, then submits the form", async () => {
    vi.useFakeTimers();
    const submitted = vi.fn((e: Event) => e.preventDefault());
    document.querySelector("form")?.addEventListener("submit", submitted);
    expect(deliver(autoFill())).toEqual({ filled: 2, pressing: "password" });
    await vi.advanceTimersByTimeAsync(0);
    expect(submitted).toHaveBeenCalledOnce();
    vi.useRealTimers();
  });

  it("does not press without submit, or when two buttons tie", async () => {
    vi.useFakeTimers();
    const submitted = vi.fn((e: Event) => e.preventDefault());
    document.querySelector("form")?.addEventListener("submit", submitted);
    expect(deliver({ ...autoFill(), submit: false })).toEqual({ filled: 2, pressing: null });

    document.body.innerHTML = `<div><input name="user" type="email" autocomplete="username"><input name="pw" type="password">
      <button>Log in</button><button>Sign in</button></div>`;
    expect(deliver(autoFill())).toEqual({ filled: 2, pressing: null });
    await vi.advanceTimersByTimeAsync(2000);
    expect(submitted).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("reports a username step and then asks to continue when the password field appears", async () => {
    vi.useFakeTimers();
    document.body.innerHTML = `<form><h1>Sign in</h1><input name="user" type="email" autocomplete="username"><button type="submit">Continue</button></form>`;
    document.querySelector("form")?.addEventListener("submit", (e) => {
      e.preventDefault();
      document.body.innerHTML = `<form><input name="pw" type="password"><button type="submit">Sign in</button></form>`;
    });
    expect(deliver(autoFill())).toEqual({ filled: 1, pressing: "username" });
    await vi.advanceTimersByTimeAsync(1000);
    expect(sent).toContainEqual({ type: "cs_run_step", kind: "password" });
    vi.useRealTimers();
  });

  it("stops watching on bg_run_end", async () => {
    vi.useFakeTimers();
    document.body.innerHTML = `<form><input name="user" type="email" autocomplete="username"><button type="submit">Continue</button></form>`;
    document.querySelector("form")?.addEventListener("submit", (e) => e.preventDefault());
    deliver(autoFill());
    await vi.advanceTimersByTimeAsync(0);
    deliver({ type: "bg_run_end" });
    document.body.innerHTML = `<form><input name="pw" type="password"></form>`;
    await vi.advanceTimersByTimeAsync(1000);
    expect(sent).not.toContainEqual({ type: "cs_run_step", kind: "password" });
    vi.useRealTimers();
  });
});
```

Note: jsdom events dispatched from tests are untrusted, so "the user takes over" (trusted `keydown` / `input` / `pointerdown`) cannot be simulated here; it is covered by code review of the three listener lines in Step 3.

- [ ] **Step 2: Run to verify failure**

Run: `pnpm --filter @havenkeys/extension test -- content`
Expected: FAIL (`pressing` is always null; no `cs_run_step`).

- [ ] **Step 3: Implement in `content/index.ts`**

Header comment, add a rule:

```ts
// * Automatic sign-in: after a fill the background marked `submit` (Rust's
//   autoSubmit), press the page's button once, then watch for the next step
//   and ask the background to continue. Any trusted key, input or pointer
//   event from the user ends the run.
```

Imports:

```ts
import { fillLogin, fillNewPassword, fillOtp, markUserEdit, valueSource } from "../autofill/fill";
import { classifyGroup, defaultEnv, fieldsOf, groupFor, groupRoot, isFillable } from "../autofill/group";
import { findSubmitButton, hasChallenge, pressWhenReady, type PressStep } from "../autofill/submit";
import { watchNext, type WatchKind } from "../autofill/watch";
```

and add `type FillReply`, `type NextStep` to the `../messaging/inline` import.

State inside `start()`:

```ts
  /** A sign-in run is active in this frame (we pressed, or the background said to watch). */
  let runActive = false;
  let stopWatch: (() => void) | null = null;
```

Helpers (new section `// ---- automatic sign-in`):

```ts
  function cancelWatch(): void {
    stopWatch?.();
    stopWatch = null;
  }

  /** End the run here; `tell`: let the background know (not for bg_run_end). */
  function endLocalRun(tell: boolean): void {
    cancelWatch();
    if (runActive && tell) void send({ type: "cs_run_stop" });
    runActive = false;
  }

  function startWatch(want: WatchKind, filled: HTMLInputElement | null): void {
    cancelWatch();
    runActive = true;
    stopWatch = watchNext({
      want,
      filled,
      env: defaultEnv,
      onResult: (r) => {
        stopWatch = null;
        if ("found" in r) void send({ type: "cs_run_step", kind: r.found });
        else endLocalRun(true);
      },
    });
  }

  /** The step a fill just completed, and the field it ended in. */
  function filledStep(
    group: ReturnType<typeof groupFor>["group"],
    kind: "login" | "otp",
  ): { step: PressStep; last: HTMLInputElement } | null {
    if (kind === "otp") {
      const boxes = fieldsOf(group, "otp");
      const last = boxes[boxes.length - 1];
      return last && valueSource(last) === "vault" ? { step: "otp", last } : null;
    }
    const [pw] = fieldsOf(group, "current-password", "password");
    if (pw) return valueSource(pw) === "vault" ? { step: "password", last: pw } : null;
    const [user] = fieldsOf(group, "username");
    return user && valueSource(user) === "vault" ? { step: "username", last: user } : null;
  }

  /** Press after a fill, then watch for the next step. Returns the step being pressed, or null. */
  function pressAfterFill(group: ReturnType<typeof groupFor>["group"], kind: "login" | "otp", totp: boolean): PressStep | null {
    const done = filledStep(group, kind);
    if (!done) return null;
    const env = defaultEnv();
    const button = findSubmitButton(group.root, done.last, done.step, env);
    if (!button || hasChallenge(document, env)) return null;
    const next: NextStep | null = done.step === "username" ? "password" : done.step === "password" && totp ? "otp" : null;
    runActive = true;
    cancelWatch();
    void pressWhenReady({ button, field: done.last, step: done.step, env }).then((outcome) => {
      if (!runActive) return;
      if (outcome === "gave_up") return endLocalRun(true);
      if (next) startWatch(next, done.last);
      else runActive = false;
    });
    return done.step;
  }
```

`handleFill` now returns `FillReply`:

```ts
  function handleFill(m: Extract<BackgroundToContent, { type: "bg_fill" }>): FillReply {
    const none: FillReply = { filled: 0, pressing: null };
    // The frame may have navigated since the desktop matched its URL.
    if (m.origin !== location.origin) return none;
    const env = defaultEnv();
    let group;
    if (m.token !== null) {
      const target = picked && picked.token === m.token && picked.until > Date.now() ? picked : null;
      picked = null;
      if (!target || !target.field.isConnected) return none;
      group = groupFor(target.field, env).group;
    } else {
      group = m.fill.kind === "otp" ? findOtpGroup(document, env) : findLoginGroup(document, env);
    }
    if (!group) return none;
    switch (m.fill.kind) {
      case "login": {
        const filled = fillLogin(group, m.fill, env);
        return { filled, pressing: filled > 0 && m.submit ? pressAfterFill(group, "login", m.totp) : null };
      }
      case "otp": {
        const filled = fillOtp(group, m.fill.code, env);
        return { filled, pressing: filled > 0 && m.submit ? pressAfterFill(group, "otp", m.totp) : null };
      }
      case "generated": {
        const n = fillNewPassword(group, m.fill.password, env);
        if (n > 0) generatedIn = group.root;
        return { filled: n, pressing: null };
      }
    }
  }
```

Listeners — first line after each `if (!e.isTrusted) return;` in the `pointerdown`, `keydown` and `input` listeners:

```ts
      if (runActive) endLocalRun(true); // the user took over
```

In the `pagehide` listener, add `cancelWatch(); runActive = false;` (navigation is expected mid-run; the next page asks with `cs_ready`).

Background messages:

```ts
      case "bg_fill":
        sendResponse(handleFill(m));
        return false;
      case "bg_run_end":
        endLocalRun(false);
        return false;
```

`cs_ready` from every frame (replace the `if (window.top === window) { ... }` block at the end of `start()`):

```ts
  // A save prompt for a login submitted just before this page loaded (top
  // frame only), and the next step of a sign-in run in progress.
  void send({ type: "cs_ready" }).then((r) => {
    const reply = r as { saveToken?: unknown; watch?: unknown } | undefined;
    if (window.top === window && typeof reply?.saveToken === "string" && TOKEN.test(reply.saveToken)) showSave(reply.saveToken);
    if (reply?.watch === "password" || reply?.watch === "otp") startWatch(reply.watch, null);
  });
```

- [ ] **Step 4: Run tests, type check, build**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck && pnpm --filter @havenkeys/extension build`
Expected: PASS; build succeeds. Also run the existing secret-hygiene test (`hygiene.test.ts`, included in the suite) — it must still pass (no `console.*`, no `innerHTML` added).

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/content
git commit -m "feat(extension): press after fill and follow sign-in steps in the page"
```

---

### Task 11: Documentation, security review, CLAUDE.md amendment

**Files:**
- Modify: `docs/autofill.md`, `docs/security-model.md`, `docs/threat-model.md`, `docs/security-review.md`, `docs/native-messaging.md`, `CLAUDE.md`
- Modify: `docs/superpowers/specs/2026-09-26-auto-sign-in-design.md` §6.3 (record the refined messages)

- [ ] **Step 1: `docs/autofill.md`** — add a section `## Automatic sign-in` after `## Saving logins`, covering, in this order: what it does (pick → fill → press → next page → password → press → OTP → press); the two switches and defaults; the run (tab, frame, exact origin, 2 minutes, forward only, once per step, memory only); next-step detection (cs_ready `watch` on load; watcher after pressing; 150 ms debounce; two-check stability; 30 s); button scoring table and the ambiguity rule (copy from spec §5.2); pressing (`requestSubmit` / `click`, 1 s enable wait, 500 ms OTP settle); stop conditions table (spec §4.3); popup-only limit (spec §6.5). Add Limitations bullets: host-hopping flows stop at the hop; sites that ignore scripted clicks; CAPTCHAs stop the press; heuristics may pick no button (then the user presses). In the "User flow" section add one line: "With automatic sign-in on, the content script then presses the form's button (see Automatic sign-in)."

- [ ] **Step 2: `docs/native-messaging.md`** — in the `fill_item` and `get_totp` rows, add `autoSubmit` (boolean: vault setting `auto_sign_in` AND the login's `auto_sign_in`, computed by `VaultService::auto_sign_in_for` after the origin check).

- [ ] **Step 3: `docs/security-model.md` and `docs/threat-model.md`** — describe the run and its binding, that `autoSubmit` is decided in Rust, that run state holds no secrets and is memory-only, and the accepted risk (spec §7). In the threat model's malicious-webpage section, add: a page cannot start or extend a run; continuation never crosses origins.

- [ ] **Step 4: `docs/security-review.md`** — add a finding row after PK22, using the table's columns:

```text
| AS1 | Medium | Extension / core (automatic sign-in) | For up to 2 minutes after a pick, a page on the same origin receives the password and the current TOTP code without further clicks, relaxing rule #6 for the rest of that one sign-in | Accepted, documented: starts only from a trusted pick; bound to tab, frame and exact origin; forward-only single-use steps; every value re-requested from Rust with the origin check; global and per-login off switches (`auto_sign_in`). Script on that origin could already obtain the password after the single manual pick; the new part is that the OTP no longer needs its own click. Not a boundary against a compromised extension, which can call fill_item / get_totp directly |
```

- [ ] **Step 5: `CLAUDE.md`** — under §25 "Explicit user interaction", after the flow block, add:

```markdown
> Amended on 2026-09-26 by
> `docs/superpowers/specs/2026-09-26-auto-sign-in-design.md`: after the user
> picks a login, HavenKeys may finish that one sign-in (press the button,
> fill a following password step and the TOTP code) on the same origin
> within 2 minutes, when the vault setting and the login's switch allow it.
> Nothing is ever filled or submitted without that pick.
```

- [ ] **Step 6: Spec §6.3** — replace the message table with the implemented shapes: `bg_fill` adds `submit: boolean` and `totp: boolean` (an OTP step follows the password step), and the content script replies `{ filled, pressing: "username" | "password" | "otp" | null }`, which the background uses to start the run; `ReadyReply` adds `watch`; `cs_run_step`, `cs_run_stop`, `bg_run_end` as before. One sentence of rationale: the background does not know which step a picked group is until the content script fills it.

- [ ] **Step 7: Full verification**

Run: `pnpm -r typecheck && pnpm -r test && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all pass. Then: `pnpm audit --prod` and `cargo audit` (no new dependencies were added; confirm nothing new is reported).

- [ ] **Step 8: Commit**

```bash
git add docs CLAUDE.md
git commit -m "docs: automatic sign-in"
```
