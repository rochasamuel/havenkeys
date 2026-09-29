# Sign in with … : complete the provider's login — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After the user picks a "Sign in with <provider> · account" login, HavenKeys finishes the provider's sign-in:
- clicks the account in the chooser;
- or clicks "Use another account";
- then hands the login form to the existing automatic sign-in run for the vault's provider login with that account.

**Architecture:** `SsoRun` gains a `login` phase. On the provider's top frame the content script:
1. clicks the chooser row;
2. or clicks "Use another account";
3. when a login form is present, sends `cs_sso_login`.

The background validates the run, finds exactly one provider login (`find_matches` + username equal to the account), calls `fill_item`, and passes the fill to the inline handler's `pickFill` with `auto` when `autoSubmit` is on. That starts the ordinary automatic sign-in run (username → password → OTP), bound to the provider origin. No Rust change.

**Tech Stack:** TypeScript (extension: vitest + jsdom), esbuild.

**Spec:** `docs/superpowers/specs/2026-09-29-sign-in-with-provider-login-design.md` (amends `docs/superpowers/specs/2026-09-28-sign-in-with-design.md`).

## Global Constraints

- Only top frames act (`frame.frameId === 0` in the background, `deps.isTop` in the content script), only on an origin in the run's `providerOrigins` (from Rust), only in the run's tab or a popup whose `openerTabId` is that tab.
- The provider login is the single `find_matches` result for the provider frame whose `username`, trimmed and lowercased, equals the run's account, trimmed and lowercased. Zero or more than one → end the run, fill nothing.
- Press only when all three switches are on:
  - the site login's `autoChoose`, which a run needs to reach `login` at all;
  - `fill_item.autoSubmit`, which Rust computes as the vault setting && the provider login's switch.
  - With `autoSubmit` false: fill, no press.
- `cs_sso_login` is honoured once per run. The run ends when it is consumed.
- `CHOOSE_WAIT_MS = 10_000` bounds the content script's wait on each provider document.
- Never press a consent screen. A visible CAPTCHA stops the run: the existing auto sign-in press refuses with `hasChallenge`.
- No `innerHTML` in sources. Nothing is logged. Every new user-visible string goes in `en` and `pt-BR`; this plan adds none.
- Commits: conventional, **no `Co-Authored-By` trailer**.
- Checks: `pnpm --filter @havenkeys/extension test` and `pnpm --filter @havenkeys/extension typecheck`.

## Review Focus

1. **Two vault logins for the provider with the same email.** Expected: nothing is filled (Task 3 test `two_provider_logins_fill_nothing`).
2. **A provider login whose username differs from the account.** Expected: never filled, even when it is the only provider login (Task 3 test `other_account_is_never_filled`).
3. **A `cs_sso_login` from a provider iframe, from an unrelated tab, or sent twice.** Expected: ignored (Task 2 `login_only_once_top_run_tab`, Task 3 `subframe_login_is_ignored`).
4. **The provider login's switch is off.** Expected: the email is filled and nothing is pressed (Task 3 test `provider_switch_off_fills_without_press`).
5. **A consent screen showing "Use another account"** (some providers list it on consent pages). Expected: nothing is clicked (Task 1 test `no_another_account_on_consent`, Task 4 test `consent_stops_before_another_account`).

---

## File map

- Modify `apps/extension/src/autofill/sso.ts` (+ `sso.test.ts`): `anotherAccountButton`.
- Modify `apps/extension/src/messaging/sso.ts` (+ test): `SsoReady` gains `{kind:"login"}`, and a new `cs_sso_login` request.
- Modify `apps/extension/src/background/sso-state.ts` (+ test): the `login` phase, `chooseFor` → `login`, `loginReady`, `loginFor`.
- Modify `apps/extension/src/background/sso-handler.ts` (+ test): `ready()` returns `login`, `cs_sso_login` completes the login, new dep `pickFill`.
- Modify `apps/extension/src/background/index.ts`: pass `pickFill` to the sso handler.
- Modify `apps/extension/src/content/sso.ts` (+ `sso-provider.test.ts`): the choose watcher gains "Use another account" and the login hand-off; `onReady({kind:"login"})`.
- Modify docs: `docs/autofill.md`, `docs/security-model.md`, `docs/threat-model.md`, `docs/security-review.md`, and `CLAUDE.md` (the §25 note).

---

### Task 1: Recognise "Use another account"

**Files:**
- Modify: `apps/extension/src/autofill/sso.ts`
- Test: `apps/extension/src/autofill/sso.test.ts`

**Interfaces:**
- Produces: `export function anotherAccountButton(root: ParentNode, env: Env): HTMLElement | null`.

- [ ] **Step 1: Write the failing tests** (append to `sso.test.ts`, reusing its `env` and `page()` helpers)

```ts
describe("use another account", () => {
  it("finds it in English and Portuguese", () => {
    for (const text of ["Use another account", "Usar outra conta", "Sign in with a different account", "Usar uma conta diferente"]) {
      const root = page(`<ul><li><div role="link">Me <div>me@gmail.com</div></div></li><li><div role="link">${text}</div></li></ul>`);
      expect(anotherAccountButton(root, env)?.textContent).toBe(text);
    }
  });
  it("no_another_account_on_consent", () => {
    const root = page(`<div role="link">Use another account</div><button>Allow</button><button>Cancel</button>`);
    expect(anotherAccountButton(root, env)).toBeNull();
  });
  it("ignores hidden or duplicated controls", () => {
    expect(anotherAccountButton(page(`<div role="link" hidden>Use another account</div>`), env)).toBeNull();
    expect(anotherAccountButton(page(`<div role="link">Use another account</div><a href="#">Use another account</a>`), env)).toBeNull();
  });
});
```

Add `anotherAccountButton` to the file's import from `./sso`.

- [ ] **Step 2: Run and confirm failure**

Run: `cd apps/extension && npx vitest run src/autofill/sso.test.ts`
Expected: FAIL (`anotherAccountButton` is not exported).

- [ ] **Step 3: Implement** (append to `autofill/sso.ts`, next to `chooserRow`)

```ts
/** "Use another account" on a provider's account chooser, in the languages HavenKeys ships. */
const ANOTHER_ACCOUNT = [
  "use another account", "usar outra conta",
  "sign in with a different account", "usar uma conta diferente",
  "use a different account", "entrar com outra conta",
];

/**
 * The chooser's "Use another account" control: exactly one visible,
 * enabled candidate whose whole label is one of the phrases, on a page
 * that is not a consent screen. Null otherwise.
 */
export function anotherAccountButton(root: ParentNode, env: Env): HTMLElement | null {
  if (isConsentScreen(root, env)) return null;
  const hits = candidates(root, env).filter((el) => ANOTHER_ACCOUNT.includes(label(el)));
  const outer = hits.filter((el) => !hits.some((o) => o !== el && o.contains(el)));
  return outer.length === 1 ? (outer[0] as HTMLElement) : null;
}
```

`candidates`, `label` and `isConsentScreen` already exist in this file. Keep the exact-label comparison: a row reading "Use another account for work" is not the control.

- [ ] **Step 4: Run the tests**

Run: `cd apps/extension && npx vitest run src/autofill/sso.test.ts && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/sso.ts apps/extension/src/autofill/sso.test.ts
git commit -m "feat(extension): recognise a provider chooser's \"use another account\""
```

---

### Task 2: Messages and run state for the `login` phase

**Files:**
- Modify: `apps/extension/src/messaging/sso.ts`, `apps/extension/src/messaging/sso.test.ts`
- Modify: `apps/extension/src/background/sso-state.ts`, `apps/extension/src/background/sso-state.test.ts`

**Interfaces:**
- Produces:
  - `SsoReady = { kind: "choose"; account: string } | { kind: "login" } | null`;
  - `SsoContentRequest` gains `{ type: "cs_sso_login" }`;
  - `SsoRun.phase: "press" | "choose" | "login"`;
  - `chooseFor(tab, origin): string | null` now moves the run to `login` instead of deleting it;
  - new `loginReady(tab: TabRef, origin: string): boolean`: a provider top frame of a run in `login`, not consuming;
  - new `loginFor(tab: TabRef, origin: string): SsoRun | null`: consumes the run and returns it.

- [ ] **Step 1: Write the failing tests**

In `messaging/sso.test.ts`:

```ts
it("parses the login phase and cs_sso_login", () => {
  expect(parseSsoReady({ kind: "login" })).toEqual({ kind: "login" });
  expect(parseSsoReady({ kind: "login", account: "a@b.co" })).toBeNull();
  expect(parseSsoContentRequest({ type: "cs_sso_login" })).toEqual({ type: "cs_sso_login" });
  expect(parseSsoContentRequest({ type: "cs_sso_login", itemId: "x" })).toBeNull();
});
```

In `background/sso-state.test.ts`, inside the `runs` describe, reusing its `start` helper and `G`:

```ts
it("choose moves to login; login_only_once_top_run_tab", () => {
  const { s } = setup();
  start(s);
  s.pressed(1);
  expect(s.chooseFor({ tabId: 1 }, G)).toBe("me@gmail.com");
  expect(s.run(1)?.phase).toBe("login");
  expect(s.chooseFor({ tabId: 1 }, G)).toBeNull(); // choose happens once
  expect(s.loginReady({ tabId: 1 }, G)).toBe(true);
  expect(s.loginReady({ tabId: 1 }, "https://evil.com")).toBe(false);
  expect(s.loginFor({ tabId: 7 }, G)).toBeNull(); // unrelated tab
  expect(s.loginFor({ tabId: 1 }, "https://accounts.google.com.evil.com")).toBeNull();
  expect(s.loginFor({ tabId: 9, openerTabId: 1 }, G)?.account).toBe("me@gmail.com"); // popup
  expect(s.loginFor({ tabId: 1 }, G)).toBeNull(); // consumed
  expect(s.run(1)).toBeNull();
});
it("login expires with the run", () => {
  const { s, advance } = setup();
  start(s);
  s.pressed(1);
  s.chooseFor({ tabId: 1 }, G);
  advance(SSO_RUN_TTL_MS + 1);
  expect(s.loginFor({ tabId: 1 }, G)).toBeNull();
});
```

Update the existing test `choose_only_on_provider_origins`. Its line `expect(s.run(1)).toBeNull(); // one action only` now reads:

```ts
expect(s.run(1)?.phase).toBe("login"); // choose is used once; the run waits for the login form
```

- [ ] **Step 2: Run and confirm failure**

Run: `cd apps/extension && npx vitest run src/messaging/sso.test.ts src/background/sso-state.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement the messages**

In `messaging/sso.ts`:
- Types:

```ts
  /** A login form is on the provider's page: complete the provider login (§3 step 3). */
  | { type: "cs_sso_login" };
```

  goes in `SsoContentRequest`, and

```ts
export type SsoReady = { kind: "choose"; account: string } | { kind: "login" } | null;
```

- `parseSsoContentRequest`: `case "cs_sso_login": return keysAre(o, ["type"]) ? { type: "cs_sso_login" } : null;`
- `parseSsoReady`:

```ts
export function parseSsoReady(v: unknown): SsoReady {
  const o = obj(v);
  if (!o) return null;
  if (o.kind === "login") return keysAre(o, ["kind"]) ? { kind: "login" } : null;
  if (o.kind !== "choose" || !keysAre(o, ["kind", "account"]) || !isAccount(o.account)) return null;
  return { kind: "choose", account: o.account };
}
```

- [ ] **Step 4: Implement the state**

In `background/sso-state.ts`:
- Header comment: "A run moves press → choose → login → end. On the provider's page it chooses the account once and completes one login …".
- `phase: "press" | "choose" | "login";`
- Replace `chooseFor` and add two functions:

```ts
    /** The run for this tab or its opener, if the origin is one of its provider origins. */
    // (helper, inside createSsoState)
```

```ts
  function runFor(tab: TabRef, origin: string): SsoRun | null {
    const r = run(tab.tabId) ?? (tab.openerTabId === undefined ? null : run(tab.openerTabId));
    return r && r.providerOrigins.includes(origin) ? r : null;
  }
```

```ts
    chooseFor(tab: TabRef, origin: string): string | null {
      const r = runFor(tab, origin);
      if (!r || r.phase !== "choose" || r.account === null) return null;
      r.phase = "login";
      return r.account;
    },
    /** A later provider document while the run waits for its login form. */
    loginReady(tab: TabRef, origin: string): boolean {
      return runFor(tab, origin)?.phase === "login";
    },
    /** cs_sso_login: the run to complete, consumed. */
    loginFor(tab: TabRef, origin: string): SsoRun | null {
      const r = runFor(tab, origin);
      if (!r || r.phase !== "login") return null;
      runs.delete(r.tabId);
      return r;
    },
```

- [ ] **Step 5: Run the tests**

Run: `cd apps/extension && npx vitest run src/messaging/sso.test.ts src/background/sso-state.test.ts && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS. Typecheck may flag `sso-handler.ts` or `content/sso.ts` narrowing on `SsoReady`. Fix them minimally: `onReady` keeps handling only `"choose"` until Task 4.

- [ ] **Step 6: Commit**

```bash
git add apps/extension/src/messaging apps/extension/src/background/sso-state.ts apps/extension/src/background/sso-state.test.ts
git commit -m "feat(extension): sign-in-with runs wait for the provider's login form"
```

---

### Task 3: The background completes the provider login

**Files:**
- Modify: `apps/extension/src/background/sso-handler.ts`, `apps/extension/src/background/sso-handler.test.ts`, `apps/extension/src/background/index.ts`

**Interfaces:**
- Consumes:
  - Task 2's `loginReady` and `loginFor`;
  - `AutoRun` and `FillPayload` types (`inline-handler.ts`, `messaging/inline.ts`);
  - `inline.pickFill(frame, token, payload, auto): Promise<number>`.
- Produces:
  - `SsoDeps.pickFill?(frame: FrameRef, payload: FillPayload, auto: AutoRun | null): Promise<number>`;
  - `ready()` returns `{kind:"login"}` for a provider top frame of a run in `login`;
  - `handleContent` handles `cs_sso_login`.

- [ ] **Step 1: Write the failing tests** (in `sso-handler.test.ts`)

Extend `setup()` with a recorder for fills:

```ts
  const fills: { frame: unknown; payload: unknown; auto: unknown }[] = [];
  // … in createSsoHandler deps:
    pickFill: async (frame, payload, auto) => {
      fills.push({ frame, payload, auto });
      return 1;
    },
  // … return { h, requests, sent, fills };
```

Tests (`G` = `googleFrame()`, `startRunTo(h)` = the existing sequence that picks and presses, reaching `choose`):

```ts
describe("completing the provider login", () => {
  const google = (over: object = {}) => ({ id: "9c9e6679-7425-40de-944b-e07fc1f90ae7", title: "Google", username: "Me@Gmail.com", hasTotp: true, strength: "same_host", provider: null, ...over });
  const startAnswers = { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true };

  async function toLogin(answers: Record<string, unknown>) {
    const env = setup({ start_sso: startAnswers, ...answers });
    await env.h.start(top, ID);
    expect(env.h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
    return env;
  }

  it("fills the one provider login with that account and presses", async () => {
    const { h, fills, requests } = await toLogin({
      find_matches: { matches: [google()] },
      fill_item: { type: "fill_item", username: "Me@Gmail.com", password: "pw", autoSubmit: true },
    });
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "login" }); // the password page after a chooser click
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(requests.map((r) => r.type)).toContain("fill_item");
    expect(fills).toEqual([{ frame: googleFrame(), payload: { kind: "login", username: "Me@Gmail.com", password: "pw" }, auto: { itemId: google().id, hasTotp: true } }]);
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(1); // once
  });
  it("two_provider_logins_fill_nothing", async () => {
    const { h, fills, requests } = await toLogin({ find_matches: { matches: [google(), google({ id: "8c9e6679-7425-40de-944b-e07fc1f90ae7" })] } });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(requests.some((r) => r.type === "fill_item")).toBe(false);
  });
  it("other_account_is_never_filled", async () => {
    const { h, fills } = await toLogin({ find_matches: { matches: [google({ username: "you@gmail.com" })] } });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
  });
  it("provider_switch_off_fills_without_press", async () => {
    const { h, fills } = await toLogin({
      find_matches: { matches: [google()] },
      fill_item: { type: "fill_item", username: "Me@Gmail.com", password: "pw", autoSubmit: false },
    });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills[0]?.auto).toBeNull();
  });
  it("subframe_login_is_ignored", async () => {
    const { h, fills } = await toLogin({ find_matches: { matches: [google()] }, fill_item: { type: "fill_item", username: "u", password: "p", autoSubmit: true } });
    await h.handleContent({ ...googleFrame(), frameId: 4, topUrl: "https://typeform.com/" }, { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(h.ready({ ...googleFrame(), frameId: 4, topUrl: "https://typeform.com/" }, { tabId: 1 })).toBeNull();
  });
  it("a denied fill ends the run quietly", async () => {
    const { h, fills } = await toLogin({ find_matches: { matches: [google()] }, fill_item: new BridgeError("denied", "x") });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
});
```

Adapt to the file's actual `setup()` answer mechanism (an `Error` answer throws). `BridgeError`'s constructor signature is in `messaging/native.ts`.

- [ ] **Step 2: Run and confirm failure**

Run: `cd apps/extension && npx vitest run src/background/sso-handler.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

In `sso-handler.ts`:
- Imports: `import type { FillPayload } from "../messaging/inline";` and `import type { AutoRun, FrameRef } from "./inline-handler";`.
- `SsoDeps` gains:

```ts
  /** The inline handler's fill-and-maybe-press (starts an automatic sign-in run when `auto` is set). */
  pickFill?(frame: FrameRef, payload: FillPayload, auto: AutoRun | null): Promise<number>;
```

- Update the header comment's trust-model bullets with: "On the provider's top frame the run completes one login: the single provider login with the run's account, filled by `fill_item` (Rust re-checks the page) and pressed only when Rust's autoSubmit allows (spec 2026-09-29)."
- `ready()`: replace the tail with

```ts
    if (!isTop) return null;
    const account = state.chooseFor(tab, frame.origin);
    if (account !== null) return { kind: "choose", account };
    return state.loginReady(tab, frame.origin) ? { kind: "login" } : null;
```

- New function:

```ts
  const sameAccount = (a: string | null, b: string | null) => a !== null && b !== null && a.trim().toLowerCase() === b.trim().toLowerCase();

  /** cs_sso_login: the provider's login form is up; fill the provider login with the run's account. */
  async function completeLogin(frame: FrameRef, tab: TabRef): Promise<void> {
    if (frame.frameId !== 0 || !deps.pickFill) return;
    const run = state.loginFor(tab, frame.origin);
    if (!run) return;
    const gen = generation;
    try {
      const { matches } = await deps.client.request({ type: "find_matches", ...frameFields(frame) });
      const mine = matches.filter((m) => sameAccount(m.username, run.account));
      if (mine.length !== 1 || gen !== generation) return;
      const login = mine[0] as Match;
      const c = await deps.client.request({ type: "fill_item", itemId: login.id, ...frameFields(frame) });
      if (gen !== generation) return;
      await deps.pickFill(frame, { kind: "login", username: c.username, password: c.password }, c.autoSubmit ? { itemId: login.id, hasTotp: login.hasTotp } : null);
    } catch {
      // Locked, denied, desktop gone: the run is already over; the menu stays available.
    }
  }
```

  and in `handleContent`: `case "cs_sso_login": await completeLogin(frame, tab); return {};`.

In `background/index.ts`, add to the `createSsoHandler` deps:

```ts
  // The inline handler is created below; it exists by the time a run reaches the provider page.
  pickFill: (frame, payload, auto) => inline.pickFill(frame, null, payload, auto),
```

`inline` is a `const` declared later in the module. The arrow only reads it when called, after module init. If TypeScript or esbuild complains about use-before-define, move the sso handler creation below `inline` and keep `inline`'s `startSso: (f, id) => sso.start(f, id)` as an arrow.

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/background
git commit -m "feat(extension): a sign-in-with run completes the provider's login with the vault's provider login"
```

---

### Task 4: The content script clicks "Use another account" and hands off the login form

**Files:**
- Modify: `apps/extension/src/content/sso.ts`
- Test: `apps/extension/src/content/sso-provider.test.ts` (its jsdom URL is `https://accounts.google.com/`)

**Interfaces:**
- Consumes: `anotherAccountButton` (Task 1), `findLoginGroup` (`autofill/page.ts`), `SsoReady` `{kind:"login"}` (Task 2), and the request `{type:"cs_sso_login"}`.

- [ ] **Step 1: Write the failing tests** (in `sso-provider.test.ts`, following its existing `onReady` tests and fake timers)

1. **The row is missing:** `onReady({kind:"choose", account:"me@gmail.com"})` on a page whose chooser lists only `you@gmail.com` plus a "Use another account" link.
   - "Use another account" is clicked exactly once.
   - When the test then renders `<input type="email" name="identifier">`, `send` receives `{type:"cs_sso_login"}` once.
2. **The account row is clicked:** the row is clicked. A password field appearing later in the same document (an SPA password step) → `cs_sso_login`.
3. **A password page in a new document:** `onReady({kind:"login"})` on a page with `<input type="password">` → `cs_sso_login` at once.
4. **`consent_stops_before_another_account`:** a consent screen (an Allow button) with "Use another account" → nothing clicked, nothing sent.
5. **User input:** `onTrustedInput()` before the email field appears → no `cs_sso_login`.
6. **A non-top instance:** `isTop: false` + `onReady({kind:"login"})` → nothing.
7. **Nothing appears:** after `CHOOSE_WAIT_MS` with no form, nothing is sent and the observer is disconnected.

- [ ] **Step 2: Run and confirm failure**

Run: `cd apps/extension && npx vitest run src/content/sso-provider.test.ts`
Expected: FAIL.

- [ ] **Step 3: Implement**

In `content/sso.ts`:
- Imports: add `anotherAccountButton` from `../autofill/sso` and `findLoginGroup` from `../autofill/page`.
- Replace `choose(account)` with one watcher, `providerStep(account: string | null)`:
  - `account` set: a `choose` document;
  - `null`: a `login` document.

```ts
  /**
   * On the provider's page during a run: choose the saved account (or "Use
   * another account"), then hand the login form to the background, which
   * fills the provider login and signs in (spec 2026-09-29 §3). One click of
   * each kind, one hand-off, within CHOOSE_WAIT_MS; any user input ends it.
   */
  function providerStep(account: string | null): void {
    cancelChoose();
    let timer: ReturnType<typeof setTimeout> | null = null;
    let chose = account === null;
    let anotherClicked = false;
    const attempt = () => {
      timer = null;
      const env = defaultEnv();
      if (isConsentScreen(document, env)) return cancelChoose();
      if (!chose && account !== null) {
        const row = chooserRow(document, account, env);
        if (row) {
          chose = true;
          row.click();
          return; // the provider moves on: a new document, or a password step here
        }
        const other = anotherAccountButton(document, env);
        if (other && !anotherClicked) {
          anotherClicked = true;
          chose = true;
          other.click();
          return;
        }
      }
      if (findLoginGroup(document, env)) {
        cancelChoose();
        void deps.send({ type: "cs_sso_login" });
      }
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
```

  Note on the `!chose` branch: a page with neither the row nor "Use another account" but with a login form (the provider signed out entirely) falls through to the hand-off. That is intended.
- `onReady`:

```ts
    onReady(sso: SsoReady): void {
      if (!deps.isTop || !sso || providersForOrigin(location.origin).length === 0) return;
      providerStep(sso.kind === "choose" ? sso.account : null);
    },
```

- Update the file header comment ("… click the saved account on the provider's chooser, or 'Use another account', and hand the login form to the background").

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck && pnpm build:extension`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/content
git commit -m "feat(extension): on the provider's page, use another account and hand the login form over"
```

---

### Task 5: Docs, amendment note, verification

**Files:**
- Modify: `docs/autofill.md`, `docs/security-model.md`, `docs/threat-model.md`, `docs/security-review.md`, `CLAUDE.md`

- [ ] **Step 1: Docs** (describe what the code does. Read the spec's §3–§5 and the code from Tasks 1–4.)
- `docs/autofill.md`, in the Sign in with section's run subsection:
  - replace "only click the saved account" with the §3 steps (chooser row → "Use another account" → login-form hand-off to the automatic sign-in run of the single provider login with that account);
  - the three switches;
  - the `login` phase;
  - the stop conditions.
- `docs/security-model.md`, in the Sign in with section: the new cross-origin fill and its bounds (spec §5). Remove any sentence saying HavenKeys never fills a password on the provider's page.
- `docs/threat-model.md`, in the Sign in with threat entry:
  - site X can trigger a provider login only after the user's pick, and only with the user's own single matching provider login, on the Rust-listed origin;
  - two matching logins → nothing.
- `docs/security-review.md`: an SSO6 entry.
  - Severity: Medium.
  - Component: extension background/content.
  - Scenario: a pick on site X fills the provider password without a second click.
  - Mitigation: the §5 bounds.
  - Limitation: relies on provider-page structure; phone and passkey prompts stop it.
- `CLAUDE.md` §25: add after the 2026-09-28 note:

```markdown
> Amended on 2026-09-29 by
> `docs/superpowers/specs/2026-09-29-sign-in-with-provider-login-design.md`:
> after that pick, if the account is not signed in at the provider,
> HavenKeys may click "Use another account" and sign in with the vault's
> one login for that provider and account (email, password, TOTP) on the
> provider's own origin, under the automatic sign-in rules and switches.
```

- [ ] **Step 2: Verify**

```bash
pnpm --filter @havenkeys/extension test
pnpm -r typecheck
pnpm build:extension
pnpm ui:check
```

Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add docs CLAUDE.md
git commit -m "docs: sign-in-with completes the provider's login"
```

- [ ] **Step 4: Manual check (report, don't claim)**

With the desktop app and the unpacked extension, test both cases and report the results:
- Google **signed out**: pick the site's "Sign in with Google" login. Expected: "Use another account" or the email step, then the email and password are filled and signed in.
- Google **signed in**: the account is clicked, as before.
