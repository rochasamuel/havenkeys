# Self-Service Signup — Stage 2: Website Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** havenkeys.net gets a `/signup` page (email and terms → six-digit code → setup code with Copy and downloads), a `/pricing` page, updated Terms and Privacy, and "Create account" in place of "Request an invite"; the invite lives only in page memory.

**Architecture:** A pure state machine in `src/lib/signup.ts` (actions in, state out, no DOM) drives a `Signup` page that renders one of three steps; the two API calls live in `src/lib/api.ts` with a fixed base URL and validated responses. The CSP gains `connect-src https://api.havenkeys.net`. Terms carry a `TERMS_VERSION` date that the signup request sends as `acceptedTerms`.

**Tech Stack:** Vite 8 + React 19 + react-router-dom 7, TypeScript strict, Vitest (node environment, `renderToString`), Vercel headers.

**Spec:** `docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md` (§7, §8 "Web"). Index: `docs/superpowers/plans/2026-10-09-self-signup-index.md`. Depends on Stage 1's routes: `POST /v1/signup/start { email, locale, acceptedTerms }` → `202 {}` and `POST /v1/signup/verify { email, code }` → `200 { invite }`, errors `{"error":{"code","message"}}` with `400 invalid_request`, `404`, `429 rate_limited`, `503 unavailable`.

## Global Constraints

- API base URL, verbatim: `https://api.havenkeys.net`. The site never calls any other API host; the CSP `connect-src` lists exactly `'self' https://api.github.com https://api.havenkeys.net`.
- The invite is held in React state only: never `localStorage`, `sessionStorage`, the URL, analytics or `document.title`; it is dropped when the component unmounts.
- `acceptedTerms` is the terms page's date in `YYYY-MM-DD`; the Terms "Last updated" line and `TERMS_VERSION` must agree (a test pins it).
- Every string exists in `en.tsx` and `pt-BR.tsx` (typed `Messages`, so the typecheck enforces it). Product names (HavenKeys, Secret Key, Emergency Kit) stay English.
- Controller name, verbatim: `SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA`. Contact: `samuelsilv.rocha@gmail.com`.
- Resend is offered 60 s after a code was sent. The code field accepts six ASCII digits only.
- `form-action 'none'` stays: forms submit through `onSubmit` with `preventDefault`, never a navigation.
- No external UI library; plain class names in `src/styles/global.css` following `apps/web/DESIGN.md`.
- **Beta, stated plainly (owner's instruction, 2026-10-09):** the site says HavenKeys is a beta. A `Beta` tag sits next to the brand in the nav on every page; the signup page's first step, the signup done step and the pricing page each carry one sentence saying so (`t.common.betaNotice`).
- **Installer warning, repeated where downloads are offered (owner's instruction, 2026-10-09):** wherever the site offers an installer, including the signup done step, it says the installers are not code-signed yet and that Windows SmartScreen and macOS Gatekeeper will warn on first run (`t.common.installerWarning`, same wording as the Download page's "Expect a warning on first run" card).
- No commit carries a `Co-Authored-By` trailer (user preference).
- Checks: `pnpm --filter @havenkeys/web typecheck`, `pnpm --filter @havenkeys/web test`, `pnpm --filter @havenkeys/web build`, `pnpm ui:check` for screenshots.

## Review Focus

1. **Back button or language switch mid-flow** must not leak the invite into history or storage, and the flow restarts cleanly. (Task 2: `the reducer forgets everything on reset`; Task 3: the page is keyed by locale so a switch unmounts it.)
2. **An existing account's email** gets the same "we sent you a code" screen; the user learns from the mail, not the page. (Task 2: the API layer treats `202` as success with no body inspection; Task 3 copy says "If this address already has an account, the email says so".)
3. **`429` and `503`** show a specific sentence each, not a generic failure. (Task 2: `errors are classified by code`.)
4. **Pasting a code with spaces** (`123 456`) must work: digits are kept, everything else dropped, capped at six. (Task 2: `code input is normalized`.)
5. **The page opened on the phone** must offer the Android download too and say the setup code is also in the mail. (Task 3: the download block reuses `AndroidDownload` plus the desktop platform links.)

---

## File Structure

| File | Responsibility |
|---|---|
| `apps/web/vercel.json` | CSP `connect-src` |
| `apps/web/src/lib/api.ts` (new) | `API_BASE`, `startSignup`, `verifySignup`, `ApiFailure` |
| `apps/web/src/lib/signup.ts` (new) | `SignupState`, `signupReducer`, `normalizeCode`, `TERMS_VERSION` |
| `apps/web/src/lib/legal.ts` (new) | `TERMS_VERSION`, `TERMS_UPDATED`, `PRIVACY_UPDATED` |
| `apps/web/src/pages/Signup.tsx` (new) | the three-step page |
| `apps/web/src/pages/Pricing.tsx` (new) | the plan page |
| `apps/web/src/components/Icon.tsx` | `copy` icon |
| `apps/web/src/i18n/en.tsx`, `pt-BR.tsx` | `signup`, `pricing`, `nav.pricing`, `footer.pricing`, `common.createAccount`; Terms and Privacy text |
| `apps/web/src/App.tsx` | routes |
| `apps/web/src/components/{Nav,Footer}.tsx`, `src/pages/{Home,SelfHost}.tsx` | "Create account" and "Pricing" links |
| `apps/web/src/lib/links.ts` | `inviteHref` removed |
| `apps/web/src/styles/global.css` | `.field`, `.field__input`, `.code-input`, `.signup`, `.pricing` |
| `tools/ui-check/web.mjs` | the two new pages |
| `apps/web/src/lib/{api,signup}.test.ts`, `src/pages/{Signup,Pricing}.test.tsx`, `src/i18n/messages.test.ts` | tests |
| `docs/website.md` | the new pages |

---

### Task 1: CSP, API client, legal constants

**Files:**
- Modify: `apps/web/vercel.json`
- Create: `apps/web/src/lib/api.ts`
- Create: `apps/web/src/lib/legal.ts`
- Test: `apps/web/src/lib/api.test.ts` (new)

**Interfaces:**
- Produces:
  - `API_BASE = "https://api.havenkeys.net"`.
  - `type ApiFailure = { kind: "invalid" | "rate_limited" | "unavailable" | "closed" | "network" }`.
  - `startSignup(input: { email: string; locale: "en" | "pt-BR"; acceptedTerms: string }, fetchFn?: typeof fetch): Promise<{ ok: true } | { ok: false; failure: ApiFailure }>`.
  - `verifySignup(input: { email: string; code: string }, fetchFn?: typeof fetch): Promise<{ ok: true; invite: string } | { ok: false; failure: ApiFailure }>`.
  - `TERMS_VERSION = "2026-10-20"` (the date Task 4 puts on the Terms page), `TERMS_UPDATED = { en: "Last updated October 20, 2026.", "pt-BR": "Atualizados em 20 de outubro de 2026." }`, `PRIVACY_UPDATED` likewise.

- [ ] **Step 1: Write the failing tests**

Create `apps/web/src/lib/api.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { API_BASE, startSignup, verifySignup } from "./api";

type Call = { url: string; init: RequestInit };

function fakeFetch(status: number, body: unknown, calls: Call[] = []): typeof fetch {
  return (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    return new Response(body === undefined ? null : JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;
}

describe("signup api", () => {
  it("posts start to the fixed base URL with exactly the three fields", async () => {
    const calls: Call[] = [];
    const result = await startSignup(
      { email: "a@example.com", locale: "pt-BR", acceptedTerms: "2026-10-20" },
      fakeFetch(202, {}, calls),
    );
    expect(result).toEqual({ ok: true });
    expect(calls[0]?.url).toBe(`${API_BASE}/v1/signup/start`);
    expect(calls[0]?.init.method).toBe("POST");
    expect(JSON.parse(String(calls[0]?.init.body))).toEqual({
      email: "a@example.com",
      locale: "pt-BR",
      acceptedTerms: "2026-10-20",
    });
    expect(calls[0]?.init.credentials).toBe("omit");
  });

  it("returns the invite from verify and rejects a malformed one", async () => {
    const ok = await verifySignup({ email: "a@example.com", code: "123456" }, fakeFetch(200, { invite: "HKINV1-abc" }));
    expect(ok).toEqual({ ok: true, invite: "HKINV1-abc" });
    const bad = await verifySignup({ email: "a@example.com", code: "123456" }, fakeFetch(200, { invite: "nope" }));
    expect(bad).toEqual({ ok: false, failure: { kind: "network" } });
    const missing = await verifySignup({ email: "a@example.com", code: "123456" }, fakeFetch(200, {}));
    expect(missing.ok).toBe(false);
  });

  it("classifies failures by status", async () => {
    const cases: Array<[number, string]> = [
      [400, "invalid"],
      [429, "rate_limited"],
      [503, "unavailable"],
      [404, "closed"],
      [500, "network"],
    ];
    for (const [status, kind] of cases) {
      const r = await startSignup(
        { email: "a@example.com", locale: "en", acceptedTerms: "2026-10-20" },
        fakeFetch(status, { error: { code: "x", message: "y" } }),
      );
      expect(r).toEqual({ ok: false, failure: { kind } });
    }
    const thrown = (async () => {
      throw new TypeError("offline");
    }) as unknown as typeof fetch;
    const r = await startSignup({ email: "a@example.com", locale: "en", acceptedTerms: "2026-10-20" }, thrown);
    expect(r).toEqual({ ok: false, failure: { kind: "network" } });
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `pnpm --filter @havenkeys/web test -- src/lib/api.test.ts`
Expected: FAIL, cannot resolve `./api`.

- [ ] **Step 3: Write the API client**

Create `apps/web/src/lib/api.ts`:

```ts
// The only API the site calls. The CSP in vercel.json allows exactly this
// host for connect-src; a second host needs both changed.
export const API_BASE = "https://api.havenkeys.net";

export type ApiFailure = {
  kind: "invalid" | "rate_limited" | "unavailable" | "closed" | "network";
};

export type Locale = "en" | "pt-BR";

type Outcome<T> = ({ ok: true } & T) | { ok: false; failure: ApiFailure };

function classify(status: number): ApiFailure["kind"] {
  if (status === 400) return "invalid";
  if (status === 429) return "rate_limited";
  if (status === 503) return "unavailable";
  if (status === 404) return "closed";
  return "network";
}

async function post(path: string, body: unknown, fetchFn: typeof fetch): Promise<Response | null> {
  try {
    return await fetchFn(`${API_BASE}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify(body),
      credentials: "omit",
      referrerPolicy: "no-referrer",
    });
  } catch {
    return null;
  }
}

export async function startSignup(
  input: { email: string; locale: Locale; acceptedTerms: string },
  fetchFn: typeof fetch = fetch,
): Promise<Outcome<Record<never, never>>> {
  const res = await post("/v1/signup/start", input, fetchFn);
  if (!res) return { ok: false, failure: { kind: "network" } };
  if (res.status === 202) return { ok: true };
  return { ok: false, failure: { kind: classify(res.status) } };
}

export async function verifySignup(
  input: { email: string; code: string },
  fetchFn: typeof fetch = fetch,
): Promise<Outcome<{ invite: string }>> {
  const res = await post("/v1/signup/verify", input, fetchFn);
  if (!res) return { ok: false, failure: { kind: "network" } };
  if (res.status !== 200) return { ok: false, failure: { kind: classify(res.status) } };
  let data: unknown;
  try {
    data = await res.json();
  } catch {
    return { ok: false, failure: { kind: "network" } };
  }
  const invite = (data as { invite?: unknown })?.invite;
  // The app accepts only this shape; anything else is not an invite.
  if (typeof invite !== "string" || !/^HKINV1-[A-Za-z0-9_-]{16,2048}$/.test(invite)) {
    return { ok: false, failure: { kind: "network" } };
  }
  return { ok: true, invite };
}
```

Create `apps/web/src/lib/legal.ts`:

```ts
// The terms version a signup accepts is the date on the Terms page. Change
// both together; messages.test.ts checks they agree.
export const TERMS_VERSION = "2026-10-20";
export const TERMS_UPDATED = {
  en: "Last updated October 20, 2026.",
  "pt-BR": "Atualizados em 20 de outubro de 2026.",
} as const;
export const PRIVACY_UPDATED = {
  en: "Last updated October 20, 2026.",
  "pt-BR": "Atualizada em 20 de outubro de 2026.",
} as const;
```

- [ ] **Step 4: CSP**

In `apps/web/vercel.json`, change the `Content-Security-Policy` value's `connect-src` to:

```
connect-src 'self' https://api.github.com https://api.havenkeys.net;
```

(the rest of the header unchanged).

- [ ] **Step 5: Run the test**

Run: `pnpm --filter @havenkeys/web test -- src/lib/api.test.ts && pnpm --filter @havenkeys/web typecheck`
Expected: PASS, no type errors.

- [ ] **Step 6: Commit**

```bash
git add apps/web/vercel.json apps/web/src/lib/api.ts apps/web/src/lib/api.test.ts apps/web/src/lib/legal.ts
git commit -m "feat(web): signup API client and the api.havenkeys.net CSP entry"
```

---

### Task 2: The signup state machine

**Files:**
- Create: `apps/web/src/lib/signup.ts`
- Test: `apps/web/src/lib/signup.test.ts` (new)

**Interfaces:**
- Consumes: `ApiFailure` from Task 1.
- Produces:
  - `type SignupState = { step: "email"; email: string; accepted: boolean; busy: boolean; error: SignupError | null } | { step: "code"; email: string; code: string; busy: boolean; error: SignupError | null; sentAt: number } | { step: "done"; invite: string }`.
  - `type SignupError = ApiFailure["kind"] | "email" | "terms" | "code"`.
  - `type SignupAction = { type: "email"; value: string } | { type: "accept"; value: boolean } | { type: "submit_email" } | { type: "sent"; at: number } | { type: "failed"; error: SignupError } | { type: "code"; value: string } | { type: "submit_code" } | { type: "verified"; invite: string } | { type: "resend" } | { type: "reset" }`.
  - `signupReducer(state, action): SignupState`, `initialSignup(): SignupState`, `normalizeCode(raw: string): string`, `canResend(state, now): boolean`, `RESEND_AFTER_MS = 60_000`.

- [ ] **Step 1: Write the failing tests**

Create `apps/web/src/lib/signup.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { canResend, initialSignup, normalizeCode, RESEND_AFTER_MS, signupReducer, type SignupState } from "./signup";

function run(actions: Parameters<typeof signupReducer>[1][], from: SignupState = initialSignup()): SignupState {
  return actions.reduce(signupReducer, from);
}

describe("signup state", () => {
  it("validates the email and the terms before submitting", () => {
    const s = run([{ type: "email", value: "not an email" }, { type: "accept", value: true }, { type: "submit_email" }]);
    expect(s.step).toBe("email");
    expect(s.step === "email" && s.error).toBe("email");
    const t = run([{ type: "email", value: "a@example.com" }, { type: "submit_email" }]);
    expect(t.step === "email" && t.error).toBe("terms");
    const u = run([{ type: "email", value: " A@Example.com " }, { type: "accept", value: true }, { type: "submit_email" }]);
    expect(u.step === "email" && u.busy).toBe(true);
    expect(u.step === "email" && u.email).toBe("a@example.com");
  });

  it("moves to the code step when the code was sent", () => {
    const s = run([
      { type: "email", value: "a@example.com" },
      { type: "accept", value: true },
      { type: "submit_email" },
      { type: "sent", at: 1000 },
    ]);
    expect(s).toEqual({ step: "code", email: "a@example.com", code: "", busy: false, error: null, sentAt: 1000 });
    expect(canResend(s, 1000 + RESEND_AFTER_MS - 1)).toBe(false);
    expect(canResend(s, 1000 + RESEND_AFTER_MS)).toBe(true);
  });

  it("code input is normalized to six digits", () => {
    expect(normalizeCode("123 456")).toBe("123456");
    expect(normalizeCode("12-34-56-78")).toBe("123456");
    expect(normalizeCode("abc")).toBe("");
    const s = run([{ type: "code", value: "12a3" }], { step: "code", email: "a@example.com", code: "", busy: false, error: null, sentAt: 0 });
    expect(s.step === "code" && s.code).toBe("123");
    const t = run([{ type: "submit_code" }], s);
    expect(t.step === "code" && t.error).toBe("code");
    expect(t.step === "code" && t.busy).toBe(false);
  });

  it("errors are classified by code and clear on the next edit", () => {
    const code: SignupState = { step: "code", email: "a@example.com", code: "123456", busy: true, error: null, sentAt: 0 };
    const limited = run([{ type: "failed", error: "rate_limited" }], code);
    expect(limited.step === "code" && limited.error).toBe("rate_limited");
    expect(limited.step === "code" && limited.busy).toBe(false);
    const cleared = run([{ type: "code", value: "1" }], limited);
    expect(cleared.step === "code" && cleared.error).toBeNull();
  });

  it("verified holds only the invite, and reset forgets everything", () => {
    const code: SignupState = { step: "code", email: "a@example.com", code: "123456", busy: true, error: null, sentAt: 0 };
    const done = run([{ type: "verified", invite: "HKINV1-x" }], code);
    expect(done).toEqual({ step: "done", invite: "HKINV1-x" });
    expect(run([{ type: "reset" }], done)).toEqual(initialSignup());
  });

  it("resend goes back to busy on the code step and keeps the email", () => {
    const code: SignupState = { step: "code", email: "a@example.com", code: "12", busy: false, error: "invalid", sentAt: 0 };
    const s = run([{ type: "resend" }], code);
    expect(s).toEqual({ step: "code", email: "a@example.com", code: "", busy: true, error: null, sentAt: 0 });
    const t = run([{ type: "sent", at: 5000 }], s);
    expect(t.step === "code" && t.sentAt).toBe(5000);
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `pnpm --filter @havenkeys/web test -- src/lib/signup.test.ts`
Expected: FAIL, cannot resolve `./signup`.

- [ ] **Step 3: Write the reducer**

Create `apps/web/src/lib/signup.ts`:

```ts
import type { ApiFailure } from "./api";

// Three screens, one reducer, no DOM: the page renders this and dispatches
// into it, and the API calls are effects that end in `sent`, `verified` or
// `failed`. The invite exists only in the `done` state, in memory.

export type SignupError = ApiFailure["kind"] | "email" | "terms" | "code";

export type SignupState =
  | { step: "email"; email: string; accepted: boolean; busy: boolean; error: SignupError | null }
  | { step: "code"; email: string; code: string; busy: boolean; error: SignupError | null; sentAt: number }
  | { step: "done"; invite: string };

export type SignupAction =
  | { type: "email"; value: string }
  | { type: "accept"; value: boolean }
  | { type: "submit_email" }
  | { type: "sent"; at: number }
  | { type: "failed"; error: SignupError }
  | { type: "code"; value: string }
  | { type: "submit_code" }
  | { type: "verified"; invite: string }
  | { type: "resend" }
  | { type: "reset" };

export const RESEND_AFTER_MS = 60_000;

export function initialSignup(): SignupState {
  return { step: "email", email: "", accepted: false, busy: false, error: null };
}

// The server normalizes too; this only keeps the obviously wrong from
// costing a round trip. One @, something on both sides, no spaces.
function looksLikeEmail(value: string): boolean {
  const at = value.indexOf("@");
  return at > 0 && at === value.lastIndexOf("@") && at < value.length - 1 && !/\s/.test(value) && value.length <= 254;
}

export function normalizeCode(raw: string): string {
  return raw.replace(/\D/g, "").slice(0, 6);
}

export function canResend(state: SignupState, now: number): boolean {
  return state.step === "code" && !state.busy && now - state.sentAt >= RESEND_AFTER_MS;
}

export function signupReducer(state: SignupState, action: SignupAction): SignupState {
  if (action.type === "reset") return initialSignup();
  switch (state.step) {
    case "email":
      switch (action.type) {
        case "email":
          return { ...state, email: action.value, error: null };
        case "accept":
          return { ...state, accepted: action.value, error: null };
        case "submit_email": {
          const email = state.email.trim().toLowerCase();
          if (!looksLikeEmail(email)) return { ...state, error: "email" };
          if (!state.accepted) return { ...state, error: "terms" };
          return { ...state, email, busy: true, error: null };
        }
        case "sent":
          return { step: "code", email: state.email, code: "", busy: false, error: null, sentAt: action.at };
        case "failed":
          return { ...state, busy: false, error: action.error };
        default:
          return state;
      }
    case "code":
      switch (action.type) {
        case "code":
          return { ...state, code: normalizeCode(action.value), error: null };
        case "submit_code":
          if (state.code.length !== 6) return { ...state, error: "code" };
          return { ...state, busy: true, error: null };
        case "verified":
          return { step: "done", invite: action.invite };
        case "failed":
          return { ...state, busy: false, error: action.error };
        case "resend":
          return { ...state, code: "", busy: true, error: null };
        case "sent":
          return { ...state, busy: false, error: null, sentAt: action.at };
        default:
          return state;
      }
    case "done":
      return state;
  }
}
```

- [ ] **Step 4: Run the test**

Run: `pnpm --filter @havenkeys/web test -- src/lib/signup.test.ts`
Expected: 6 PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/lib/signup.ts apps/web/src/lib/signup.test.ts
git commit -m "feat(web): signup state machine"
```

---

### Task 3: The `/signup` page

**Files:**
- Create: `apps/web/src/pages/Signup.tsx`
- Modify: `apps/web/src/components/Icon.tsx` (add `copy`)
- Modify: `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx` (`signup` section, `common.createAccount`)
- Modify: `apps/web/src/App.tsx` (route)
- Modify: `apps/web/src/styles/global.css`
- Modify: `tools/ui-check/web.mjs`
- Test: `apps/web/src/pages/Signup.test.tsx` (new)

**Interfaces:**
- Consumes: `startSignup`, `verifySignup` (Task 1), `signupReducer` and friends (Task 2), `TERMS_VERSION` (Task 1), `fetchLatestRelease`, `pickAsset`, `RELEASES_PAGE_URL` from `src/lib/releases.ts`, `AndroidDownload` component, `detectOs`.
- Produces: `Signup` page at `/signup` and `/pt-br/signup`; messages `t.signup.*`; `t.common.createAccount`.

- [ ] **Step 1: Write the failing test**

Create `apps/web/src/pages/Signup.test.tsx`:

```tsx
import type { ReactNode } from "react";
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { ptBR } from "../i18n/pt-BR";
import { Signup, SignupView } from "./Signup";

const html = (node: ReactNode) => renderToString(<MemoryRouter>{node}</MemoryRouter>);

describe("Signup", () => {
  it("starts on the email step with the terms checkbox unchecked", () => {
    const h = html(<Signup />);
    expect(h).toContain('type="email"');
    expect(h).toContain('type="checkbox"');
    expect(h).not.toContain("checked");
    expect(h).toContain(en.signup.create);
  });

  it("renders the code step with a six-digit field and no resend before a minute", () => {
    const h = html(
      <SignupView
        state={{ step: "code", email: "a@example.com", code: "", busy: false, error: null, sentAt: Date.now() }}
        now={Date.now()}
        dispatch={() => {}}
        t={en}
        locale="en"
      />,
    );
    expect(h).toContain('inputmode="numeric"');
    expect(h).toContain('maxlength="6"');
    expect(h).toContain("a@example.com");
    expect(h).toContain("disabled");
    expect(h).toContain(en.signup.resendIn.replace("{s}", "60"));
  });

  it("renders the invite on the done step in a read-only field with Copy", () => {
    const h = html(
      <SignupView state={{ step: "done", invite: "HKINV1-abcdef" }} now={0} dispatch={() => {}} t={ptBR} locale="pt-BR" />,
    );
    expect(h).toContain("HKINV1-abcdef");
    expect(h).toContain("readonly");
    expect(h).toContain(ptBR.signup.copy);
    expect(h).toContain(ptBR.signup.alsoEmailed);
    expect(h).toContain(ptBR.common.installerWarning);
    expect(h).toContain(ptBR.common.betaNotice);
    expect(h).not.toContain("href=\"HKINV1");
  });

  it("says it is a beta on the first step", () => {
    expect(html(<Signup />)).toContain(en.common.betaNotice);
  });

  it("names each failure", () => {
    for (const error of ["rate_limited", "unavailable", "closed", "invalid", "network"] as const) {
      const h = html(
        <SignupView
          state={{ step: "email", email: "a@example.com", accepted: true, busy: false, error }}
          now={0}
          dispatch={() => {}}
          t={en}
          locale="en"
        />,
      );
      expect(h).toContain(en.signup.errors[error]);
    }
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `pnpm --filter @havenkeys/web test -- src/pages/Signup.test.tsx`
Expected: FAIL, cannot resolve `./Signup`.

- [ ] **Step 3: Strings**

In `apps/web/src/i18n/en.tsx`, add to `common`:

```tsx
    createAccount: "Create account",
    beta: "Beta",
    betaNotice:
      "HavenKeys is a beta. It works and it is what we use every day, but expect rough edges, and keep your Emergency Kit somewhere safe.",
    installerWarning:
      "The installers aren’t code-signed yet, so Windows SmartScreen and macOS Gatekeeper will warn you on first run. Android verifies the APK’s signature itself.",
```

and in `pt-BR.tsx`:

```tsx
    createAccount: "Criar conta",
    beta: "Beta",
    betaNotice:
      "O HavenKeys está em beta. Funciona e é o que usamos todo dia, mas espere arestas, e guarde bem seu Emergency Kit.",
    installerWarning:
      "Os instaladores ainda não são assinados, então o Windows SmartScreen e o Gatekeeper do macOS vão avisar na primeira execução. No Android, o APK tem a assinatura verificada pelo próprio sistema.",
```

Then a new top-level section after `download`:

```tsx
  signup: {
    title: "Create your HavenKeys account",
    lede: "Three steps: confirm your email, then set up the app on your computer or phone.",
    emailLabel: "Email",
    emailHint: "We send a six-digit code to confirm it is yours.",
    terms: (terms: string, privacy: string) => (
      <>
        I have read and accept the <Link to={terms}>Terms</Link> and the{" "}
        <Link to={privacy}>Privacy Policy</Link>.
      </>
    ),
    create: "Create account",
    sending: "Sending…",
    codeTitle: "Check your email",
    codeLede: (email: string) => (
      <>
        We sent a six-digit code to <strong>{email}</strong>. It is valid for 15 minutes. If this address
        already has an account, the email says so instead.
      </>
    ),
    codeLabel: "Code",
    verify: "Continue",
    verifying: "Checking…",
    resend: "Resend code",
    resendIn: "Resend in {s} s",
    changeEmail: "Use another email",
    doneTitle: "Your setup code",
    doneLede:
      "Install HavenKeys, open it, choose “I have a setup code” and paste this code. Then pick a master password and keep your Emergency Kit safe.",
    copy: "Copy",
    copied: "Copied",
    alsoEmailed: "We also emailed it. It is valid for 24 hours and works once.",
    inviteAria: "Setup code",
    downloadsTitle: "Get the app",
    otherDownloads: "All downloads",
    errors: {
      email: "That does not look like an email address.",
      terms: "Please accept the Terms and the Privacy Policy to continue.",
      code: "Enter the six digits from the email.",
      invalid: "That code is not valid. Check the email, or request a new code.",
      rate_limited: "Too many attempts. Wait an hour and try again.",
      unavailable: "We could not send the email right now. Try again in a few minutes.",
      closed: "Sign-up is not open on this server.",
      network: "We could not reach the server. Check your connection and try again.",
    },
  },
```

`Link` is already imported in `en.tsx` (used by `notFound.body`). Add the same keys to `pt-BR.tsx`:

```tsx
  signup: {
    title: "Crie sua conta HavenKeys",
    lede: "Três passos: confirme seu e-mail e depois configure o app no computador ou no celular.",
    emailLabel: "E-mail",
    emailHint: "Enviamos um código de seis dígitos para confirmar que ele é seu.",
    terms: (terms: string, privacy: string) => (
      <>
        Li e aceito os <Link to={terms}>Termos</Link> e a <Link to={privacy}>Política de Privacidade</Link>.
      </>
    ),
    create: "Criar conta",
    sending: "Enviando…",
    codeTitle: "Veja seu e-mail",
    codeLede: (email: string) => (
      <>
        Enviamos um código de seis dígitos para <strong>{email}</strong>. Ele vale por 15 minutos. Se este
        endereço já tem uma conta, o e-mail diz isso.
      </>
    ),
    codeLabel: "Código",
    verify: "Continuar",
    verifying: "Verificando…",
    resend: "Reenviar código",
    resendIn: "Reenviar em {s} s",
    changeEmail: "Usar outro e-mail",
    doneTitle: "Seu código de configuração",
    doneLede:
      "Instale o HavenKeys, abra o app, escolha “Tenho um código de configuração” e cole este código. Depois escolha uma senha mestra e guarde bem seu Emergency Kit.",
    copy: "Copiar",
    copied: "Copiado",
    alsoEmailed: "Também enviamos por e-mail. Ele vale por 24 horas e funciona uma única vez.",
    inviteAria: "Código de configuração",
    downloadsTitle: "Baixe o app",
    otherDownloads: "Todos os downloads",
    errors: {
      email: "Isso não parece um endereço de e-mail.",
      terms: "Aceite os Termos e a Política de Privacidade para continuar.",
      code: "Digite os seis dígitos do e-mail.",
      invalid: "Esse código não é válido. Confira o e-mail ou peça um novo código.",
      rate_limited: "Muitas tentativas. Espere uma hora e tente de novo.",
      unavailable: "Não conseguimos enviar o e-mail agora. Tente de novo em alguns minutos.",
      closed: "O cadastro não está aberto neste servidor.",
      network: "Não conseguimos falar com o servidor. Verifique sua conexão e tente de novo.",
    },
  },
```

and `createAccount: "Criar conta",` in `common`.

- [ ] **Step 4: The `copy` icon**

In `apps/web/src/components/Icon.tsx`, add to the icon map (same stroke conventions as the others, 24-unit viewBox):

```tsx
  copy: (
    <>
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M5 15V5a2 2 0 0 1 2-2h10" />
    </>
  ),
```

and `"copy"` to the `IconName` union if it is an explicit union.

- [ ] **Step 5: The page**

Create `apps/web/src/pages/Signup.tsx`:

```tsx
import { useEffect, useReducer, useRef, useState, type Dispatch, type FormEvent } from "react";
import { AndroidDownload } from "../components/AndroidDownload";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import type { Messages } from "../i18n/en";
import type { Locale } from "../i18n/locale";
import { startSignup, verifySignup } from "../lib/api";
import { TERMS_VERSION } from "../lib/legal";
import { detectOs, type Os } from "../lib/os";
import { fetchLatestRelease, pickAsset, RELEASES_PAGE_URL, type LatestRelease, type Platform } from "../lib/releases";
import { canResend, initialSignup, RESEND_AFTER_MS, signupReducer, type SignupAction, type SignupState } from "../lib/signup";

/*
 * The invite is React state and nothing else. It is never written to
 * storage, the URL, the title or analytics, and it goes when this component
 * unmounts (a language switch remounts the page: App keys it by locale).
 */
export function Signup() {
  const { t, locale } = useI18n();
  const [state, dispatch] = useReducer(signupReducer, undefined, initialSignup);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (state.step !== "code") return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [state.step]);

  useEffect(() => {
    if (state.step === "email" && state.busy) {
      let cancelled = false;
      startSignup({ email: state.email, locale, acceptedTerms: TERMS_VERSION }).then((r) => {
        if (cancelled) return;
        dispatch(r.ok ? { type: "sent", at: Date.now() } : { type: "failed", error: r.failure.kind });
      });
      return () => {
        cancelled = true;
      };
    }
    if (state.step === "code" && state.busy && state.code === "") {
      let cancelled = false;
      startSignup({ email: state.email, locale, acceptedTerms: TERMS_VERSION }).then((r) => {
        if (cancelled) return;
        dispatch(r.ok ? { type: "sent", at: Date.now() } : { type: "failed", error: r.failure.kind });
      });
      return () => {
        cancelled = true;
      };
    }
    if (state.step === "code" && state.busy) {
      let cancelled = false;
      verifySignup({ email: state.email, code: state.code }).then((r) => {
        if (cancelled) return;
        dispatch(r.ok ? { type: "verified", invite: r.invite } : { type: "failed", error: r.failure.kind });
      });
      return () => {
        cancelled = true;
      };
    }
    return undefined;
  }, [state, locale]);

  return <SignupView state={state} now={now} dispatch={dispatch} t={t} locale={locale} />;
}

export function SignupView({
  state,
  now,
  dispatch,
  t,
  locale,
}: {
  state: SignupState;
  now: number;
  dispatch: Dispatch<SignupAction>;
  t: Messages;
  locale: Locale;
}) {
  const s = t.signup;
  const path = (p: string) => (locale === "pt-BR" ? `/pt-br${p}` : p);
  return (
    <section className="signup">
      <div className="signup__card">
        {state.step === "email" && (
          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              dispatch({ type: "submit_email" });
            }}
            noValidate
          >
            <h1>{s.title}</h1>
            <p className="signup__lede">{s.lede}</p>
            <p className="signup__note">{t.common.betaNotice}</p>
            <label className="field">
              <span className="field__label">{s.emailLabel}</span>
              <input
                className="field__input"
                type="email"
                autoComplete="email"
                inputMode="email"
                value={state.email}
                disabled={state.busy}
                onChange={(e) => dispatch({ type: "email", value: e.target.value })}
              />
              <span className="field__hint">{s.emailHint}</span>
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={state.accepted}
                disabled={state.busy}
                onChange={(e) => dispatch({ type: "accept", value: e.target.checked })}
              />
              <span>{s.terms(path("/terms"), path("/privacy"))}</span>
            </label>
            {state.error && (
              <p className="field__error" role="alert">
                {s.errors[state.error]}
              </p>
            )}
            <button className="btn btn--primary btn--lg" type="submit" disabled={state.busy}>
              {state.busy ? s.sending : s.create}
            </button>
          </form>
        )}

        {state.step === "code" && (
          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              dispatch({ type: "submit_code" });
            }}
            noValidate
          >
            <h1>{s.codeTitle}</h1>
            <p className="signup__lede">{s.codeLede(state.email)}</p>
            <label className="field">
              <span className="field__label">{s.codeLabel}</span>
              <input
                className="field__input code-input"
                type="text"
                inputMode="numeric"
                autoComplete="one-time-code"
                pattern="[0-9]*"
                maxLength={6}
                value={state.code}
                disabled={state.busy}
                onChange={(e) => dispatch({ type: "code", value: e.target.value })}
              />
            </label>
            {state.error && (
              <p className="field__error" role="alert">
                {s.errors[state.error]}
              </p>
            )}
            <div className="signup__actions">
              <button className="btn btn--primary btn--lg" type="submit" disabled={state.busy}>
                {state.busy ? s.verifying : s.verify}
              </button>
              <button
                className="btn btn--ghost"
                type="button"
                disabled={!canResend(state, now)}
                onClick={() => dispatch({ type: "resend" })}
              >
                {canResend(state, now)
                  ? s.resend
                  : s.resendIn.replace("{s}", String(Math.max(0, Math.ceil((state.sentAt + RESEND_AFTER_MS - now) / 1000))))}
              </button>
              <button className="btn btn--ghost" type="button" onClick={() => dispatch({ type: "reset" })}>
                {s.changeEmail}
              </button>
            </div>
          </form>
        )}

        {state.step === "done" && <Done invite={state.invite} t={t} />}
      </div>
    </section>
  );
}

function Done({ invite, t }: { invite: string; t: Messages }) {
  const s = t.signup;
  const [copied, setCopied] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(invite);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      field.current?.select();
    }
  };
  return (
    <div>
      <h1>{s.doneTitle}</h1>
      <p className="signup__lede">{s.doneLede}</p>
      <div className="invite">
        <input
          ref={field}
          className="field__input invite__field"
          readOnly
          value={invite}
          aria-label={s.inviteAria}
          onFocus={(e) => e.target.select()}
        />
        <button className="btn btn--primary" type="button" onClick={copy}>
          <Icon name="copy" size={16} />
          {copied ? s.copied : s.copy}
        </button>
      </div>
      <p className="signup__note">{s.alsoEmailed}</p>
      <h2>{s.downloadsTitle}</h2>
      <Downloads t={t} />
      <p className="signup__note">{t.common.installerWarning}</p>
      <p className="signup__note">{t.common.betaNotice}</p>
    </div>
  );
}

const DESKTOP: Array<{ id: Platform; os: Os }> = [
  { id: "windows", os: "windows" },
  { id: "macos", os: "macos" },
  { id: "linux-appimage", os: "linux" },
];

function Downloads({ t }: { t: Messages }) {
  const [release, setRelease] = useState<LatestRelease | null>(null);
  const [os] = useState(() => (typeof navigator === "undefined" ? null : detectOs(navigator.userAgent)));
  useEffect(() => {
    let cancelled = false;
    fetchLatestRelease().then((r) => {
      if (!cancelled) setRelease(r);
    });
    return () => {
      cancelled = true;
    };
  }, []);
  return (
    <div className="signup__downloads">
      {DESKTOP.map((p) => {
        const asset = release ? pickAsset(release.assets, p.id) : null;
        const yours = p.os === os;
        return (
          <a
            key={p.id}
            className={`btn ${yours ? "btn--primary" : "btn--ghost"}`}
            href={asset?.browser_download_url ?? RELEASES_PAGE_URL}
            target="_blank"
            rel="noreferrer"
          >
            <Icon name="download" size={15} />
            {t.download.platforms[p.id].label}
          </a>
        );
      })}
      {os === "android" && <AndroidDownload />}
      <a className="text-link" href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
        {t.signup.otherDownloads}
      </a>
    </div>
  );
}
```

If `AndroidDownload` needs props the current component does not take, render it exactly as `Download.tsx` does. If `t.download.platforms[p.id].label` is not the shape in `en.tsx`, use the label key the Download page uses.

- [ ] **Step 6: Route, keyed by locale**

In `apps/web/src/App.tsx`: import `Signup` and add `{ path: "signup", element: <Signup /> },` after `download` in `PAGES`. In `Site`, give the English and Portuguese routes different `key`s (they already have `en-signup` / `pt-signup`), which remounts the page on a language switch, dropping the invite.

- [ ] **Step 7: Styles**

Append to `apps/web/src/styles/global.css`, in the forms position (after the buttons block):

```css
/* ------------------------------------------------------------- forms */

.field {
  display: grid;
  gap: 6px;
  margin: 20px 0;
}

.field__label {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-strong);
}

.field__input {
  height: 46px;
  padding: 0 14px;
  border: 1px solid var(--line-strong);
  border-radius: var(--r-md);
  background: var(--ink);
  color: var(--text);
  font: inherit;
  font-size: 16px;
}

.field__input:focus-visible {
  outline: 2px solid var(--brass-line);
  outline-offset: 2px;
}

.field__input:disabled {
  opacity: 0.6;
}

.field__hint {
  font-size: 13px;
  color: var(--muted);
}

.field__error {
  margin: 0 0 16px;
  color: var(--danger, #d85a4a);
  font-size: 14px;
}

.check {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  margin: 8px 0 20px;
  font-size: 15px;
  color: var(--muted-hi);
}

.check input {
  margin-top: 4px;
  width: 18px;
  height: 18px;
  accent-color: var(--primary-bg);
}

.code-input {
  font-family: "JetBrains Mono", monospace;
  font-size: 24px;
  letter-spacing: 0.4em;
  text-align: center;
}

/* ------------------------------------------------------------ signup */

.signup {
  max-width: 560px;
  margin: 0 auto;
  padding: clamp(72px, 10vw, 120px) var(--gutter) 0;
}

.signup__card h1 {
  font-size: clamp(2rem, 4vw, 2.6rem);
  margin-bottom: 8px;
}

.signup__lede {
  margin: 0 0 24px;
  color: var(--muted-hi);
}

.signup__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}

.signup__note {
  color: var(--muted);
  font-size: 14px;
}

.invite {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
  margin: 16px 0;
}

.invite__field {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  text-overflow: ellipsis;
}

.signup__downloads {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
}

.signup h2 {
  margin-top: 40px;
  font-size: 1.3rem;
}

@media (max-width: 480px) {
  .invite {
    grid-template-columns: minmax(0, 1fr);
  }
}
```

Use the existing `--danger` token if `apps/web/DESIGN.md` names one; otherwise keep the fallback.

In `tools/ui-check/web.mjs`, add `"/signup"` to `webPages`.

- [ ] **Step 8: Run the tests, the typecheck and a build**

Run: `pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web typecheck && pnpm --filter @havenkeys/web build`
Expected: all PASS; the build succeeds (the typecheck is what catches a missing pt-BR key).

- [ ] **Step 9: Look at it**

Run: `pnpm dev:web` and open `http://localhost:5173/signup` and `/pt-br/signup`. The page talks to `api.havenkeys.net`, so the real flow needs the hosted server at Stage 1; check the three steps by temporarily rendering `SignupView` with each state in a scratch route if needed, then remove it. Check phone width (390px): the invite field and the Copy button stack.

- [ ] **Step 10: Commit**

```bash
git add apps/web tools/ui-check/web.mjs
git commit -m "feat(web): /signup page: email and terms, six-digit code, setup code with Copy and downloads"
```

---

### Task 4: Pricing page, links, legal text

**Files:**
- Create: `apps/web/src/pages/Pricing.tsx`
- Modify: `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx` (`pricing`, `nav.pricing`, `footer.pricing`, Terms and Privacy bodies and `updated`)
- Modify: `apps/web/src/components/Nav.tsx`, `apps/web/src/components/Footer.tsx`
- Modify: `apps/web/src/pages/Home.tsx`, `apps/web/src/pages/SelfHost.tsx`
- Modify: `apps/web/src/pages/Terms.tsx`, `apps/web/src/pages/Privacy.tsx` (read `updated` from `legal.ts`)
- Modify: `apps/web/src/lib/links.ts` (remove `INVITE_EMAIL`, `inviteHref`), `apps/web/src/lib/links.test.ts`
- Modify: `apps/web/src/App.tsx`, `tools/ui-check/web.mjs`
- Modify: `docs/website.md`
- Test: `apps/web/src/pages/Pricing.test.tsx` (new), `apps/web/src/i18n/messages.test.ts`

**Interfaces:**
- Consumes: `TERMS_VERSION`, `TERMS_UPDATED`, `PRIVACY_UPDATED` (Task 1).
- Produces: `/pricing`; `t.pricing.*`; `t.nav.pricing`, `t.footer.pricing`; Home and SelfHost link to `/signup`.

- [ ] **Step 1: Write the failing tests**

Create `apps/web/src/pages/Pricing.test.tsx`:

```tsx
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { Pricing } from "./Pricing";

describe("Pricing", () => {
  it("shows the Personal plan, the trial and what a frozen account keeps", () => {
    const h = renderToString(
      <MemoryRouter>
        <Pricing />
      </MemoryRouter>,
    );
    expect(h).toContain(en.pricing.plan);
    expect(h).toContain(en.pricing.trial);
    expect(h).toContain(en.pricing.priceSoon);
    for (const line of en.pricing.afterTrial) expect(h).toContain(line);
    expect(h).toContain('href="/signup"');
    expect(h).toContain(en.common.betaNotice);
  });
});
```

Add to `apps/web/src/App.test.tsx` (it renders the site through `MemoryRouter`; follow its existing pattern for rendering a route):

```tsx
  it("shows the Beta tag in the nav on every page", () => {
    for (const path of ["/", "/download", "/signup", "/pricing", "/pt-br"]) {
      expect(render(path)).toContain('class="tag tag--beta"');
    }
  });
```

where `render(path)` is the file's existing helper that renders `<Site />` at a path (create one with `renderToString(<MemoryRouter initialEntries={[path]}><Site /></MemoryRouter>)` if the file has none).

```tsx
```

Replace the invite-subject test in `apps/web/src/i18n/messages.test.ts` with:

```ts
  it("sends the terms version that is on the Terms page", async () => {
    const { TERMS_VERSION, TERMS_UPDATED, PRIVACY_UPDATED } = await import("../lib/legal");
    expect(TERMS_VERSION).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(en.terms.updated).toBe(TERMS_UPDATED.en);
    expect(ptBR.terms.updated).toBe(TERMS_UPDATED["pt-BR"]);
    expect(en.privacy.updated).toBe(PRIVACY_UPDATED.en);
    expect(ptBR.privacy.updated).toBe(PRIVACY_UPDATED["pt-BR"]);
    // "October 20, 2026" ↔ "2026-10-20": the day and year must appear.
    const [y, , d] = TERMS_VERSION.split("-");
    expect(en.terms.updated).toContain(`${Number(d)}, ${y}`);
  });
  it("has no request-an-invite copy left", () => {
    for (const m of [en, ptBR]) {
      expect(JSON.stringify(m)).not.toMatch(/invite@havenkeys\.net|Request an invite|Pedir um convite/);
    }
  });
```

In `apps/web/src/lib/links.test.ts`, delete the `inviteHref` test(s).

- [ ] **Step 2: Run them to see them fail**

Run: `pnpm --filter @havenkeys/web test`
Expected: `Pricing.test.tsx` fails to resolve; the two new message tests FAIL.

- [ ] **Step 3: Pricing strings**

In `en.tsx`, add `pricing: "Pricing",` to `nav` and `footer`, and a `pricing` section:

```tsx
  pricing: {
    title: "One plan, your keys",
    lede: "HavenKeys is open source and free to self-host. The hosted service pays for the server and the work.",
    plan: "Personal",
    priceSoon: "Price coming soon",
    trial: "14 days free, no card",
    includes: [
      "Unlimited logins, cards, identities, notes and passkeys",
      "Desktop, browser extension and Android",
      "Phone-approved sign-in on a new computer",
      "Export at any time, in plain or encrypted form",
    ],
    afterTitle: "After the trial",
    afterTrial: [
      "Your vault stays readable on every device.",
      "Export keeps working, in plain or encrypted form.",
      "Changes, new devices and autofill pause until you subscribe.",
      "Signing in with an existing passkey keeps working.",
    ],
    subscribeSoon: "Subscriptions open soon. Until then, write to samuelsilv.rocha@gmail.com and we will keep your account open.",
    cta: "Create account",
    selfHost: "Or run your own server for free",
  },
```

and the Portuguese:

```tsx
  pricing: {
    title: "Um plano, suas chaves",
    lede: "O HavenKeys é código aberto e gratuito para hospedar por conta própria. O serviço hospedado paga o servidor e o trabalho.",
    plan: "Pessoal",
    priceSoon: "Preço em breve",
    trial: "14 dias grátis, sem cartão",
    includes: [
      "Logins, cartões, identidades, notas e passkeys sem limite",
      "Desktop, extensão do navegador e Android",
      "Entrada em um computador novo aprovada pelo celular",
      "Exportação a qualquer momento, aberta ou criptografada",
    ],
    afterTitle: "Depois do período de teste",
    afterTrial: [
      "Seu cofre continua legível em todos os dispositivos.",
      "A exportação continua funcionando, aberta ou criptografada.",
      "Alterações, dispositivos novos e preenchimento automático ficam pausados até você assinar.",
      "Entrar com uma passkey já salva continua funcionando.",
    ],
    subscribeSoon: "As assinaturas abrem em breve. Até lá, escreva para samuelsilv.rocha@gmail.com e mantemos sua conta aberta.",
    cta: "Criar conta",
    selfHost: "Ou rode seu próprio servidor de graça",
  },
```

with `pricing: "Planos",` in `nav` and `footer`.

- [ ] **Step 4: The page**

Create `apps/web/src/pages/Pricing.tsx`:

```tsx
import { Link } from "react-router-dom";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";

export function Pricing() {
  const { t, path } = useI18n();
  const p = t.pricing;
  return (
    <section className="pricing">
      <div className="section__head">
        <h1>{p.title}</h1>
        <p>{p.lede}</p>
      </div>
      <div className="pricing__card">
        <h2>{p.plan}</h2>
        <p className="pricing__price">{p.priceSoon}</p>
        <p className="pricing__trial">{p.trial}</p>
        <ul className="plain-list plain-list--ok">
          {p.includes.map((line) => (
            <li key={line}>
              <Icon name="check" size={16} />
              {line}
            </li>
          ))}
        </ul>
        <Link to={path("/signup")} className="btn btn--primary btn--lg">
          {p.cta}
        </Link>
        <p className="pricing__note">{p.subscribeSoon}</p>
        <p className="pricing__note">{t.common.betaNotice}</p>
      </div>
      <h2 className="pricing__after">{p.afterTitle}</h2>
      <ul className="plain-list">
        {p.afterTrial.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
      <p>
        <Link to={path("/self-host")} className="text-link">
          {p.selfHost}
        </Link>
      </p>
    </section>
  );
}
```

Styles, appended to `global.css`:

```css
/* ----------------------------------------------------------- pricing */

.pricing {
  max-width: 720px;
  margin: 0 auto;
  padding: clamp(72px, 10vw, 120px) var(--gutter) 0;
}

.pricing__card {
  margin: 32px 0;
  padding: 28px;
  border: 1px solid var(--line-strong);
  border-radius: var(--r-lg, 16px);
  background: var(--ink);
}

.pricing__card h2 {
  margin: 0 0 4px;
}

.pricing__price {
  margin: 0;
  font-size: 1.6rem;
  font-weight: 600;
  color: var(--text-strong);
}

.pricing__trial {
  margin: 4px 0 20px;
  color: var(--muted-hi);
}

.pricing__card .btn {
  margin-top: 20px;
}

.pricing__note,
.pricing__after {
  margin-top: 24px;
}

.pricing__note {
  color: var(--muted);
  font-size: 14px;
}
```

Route: `{ path: "pricing", element: <Pricing /> },` in `App.tsx` `PAGES`; `"/pricing"` in `tools/ui-check/web.mjs`.

- [ ] **Step 5: Links**

- `Nav.tsx`: inside the brand link, after `<span>HavenKeys</span>`, add `<span className="tag tag--beta">{t.common.beta}</span>` (style: `.tag--beta { margin-left: 8px; font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; }` appended to `global.css` next to `.tag--opt`); add `<NavLink to={path("/pricing")} className="nav__hide-sm">{t.nav.pricing}</NavLink>` after Security; change the primary button to `<Link to={path("/signup")} className="btn btn--primary btn--sm">{t.common.createAccount}</Link>` and keep Download as a plain `NavLink` before it.
- `Footer.tsx`: add `<Link to={path("/pricing")}>{t.footer.pricing}</Link>` after Download.
- `Home.tsx`: after `<p className="hero__meta">{h.heroMeta}</p>` add `<p className="hero__meta">{t.common.betaNotice}</p>`; delete `inviteHref` import and `const invite`; replace both `<a href={invite} …>{t.common.requestInvite}</a>` with `<Link to={path("/signup")} className="btn btn--ghost btn--lg">{t.common.createAccount}</Link>`.
- `SelfHost.tsx`: replace the `inviteHref` anchor with `<Link to={path("/signup")} className="btn btn--ghost">{s.inviteInstead}</Link>` and change `selfHost.inviteInstead` to `"Rather not run a server? Create an account on ours."` / `"Prefere não rodar um servidor? Crie uma conta no nosso."`.
- `links.ts`: delete `INVITE_EMAIL` and `inviteHref`. Delete `common.requestInvite` and `common.inviteSubject` from both message files (the typecheck finds every use).

- [ ] **Step 6: Terms and Privacy**

`Terms.tsx` and `Privacy.tsx`: replace `{t.terms.updated}` / `{t.privacy.updated}` with the same values read through the messages (they stay in the message files; `legal.ts` is the pinned copy that the test compares). Set `terms.updated` and `privacy.updated` in both message files to the `TERMS_UPDATED` / `PRIVACY_UPDATED` strings from Task 1.

In `en.tsx` `terms.body`, replace the "Our server and yours" section with:

```tsx
        <h2>Our server and yours</h2>
        <p>
          SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA (“we”) runs a{" "}
          <code>havenkeys-server</code> at api.havenkeys.net. Anyone can instead run their own
          server; downloading the software creates no account with us.
        </p>

        <h2>Accounts, trial and plans</h2>
        <p>
          Creating an account on our server starts a 14-day free trial with no card. After it, the
          account continues on the Personal plan once you subscribe; until subscriptions open, we
          keep accounts open on request. Accounts we invited directly are complimentary with no end
          date unless we tell you otherwise.
        </p>
        <p>
          An account whose trial has ended, or whose payment has lapsed, is <strong>frozen</strong>:
          the server refuses changes, new devices and new passkeys, and the apps stop filling. You
          keep reading your vault on every device, signing in with saved passkeys, and exporting it,
          in plain or encrypted form, at any time. We never delete a frozen vault for being frozen;
          you can delete the account yourself from the app at any time.
        </p>
        <p>
          The service is provided on a best-effort basis, with no service-level agreement, and may
          change or end with notice. Nothing here limits your right to export and leave.
        </p>
```

and the Portuguese equivalent in `pt-BR.tsx`:

```tsx
        <h2>Nosso servidor e o seu</h2>
        <p>
          SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA (“nós”) mantém um{" "}
          <code>havenkeys-server</code> em api.havenkeys.net. Qualquer pessoa pode, em vez disso,
          rodar o próprio servidor; baixar o software não cria conta conosco.
        </p>

        <h2>Contas, período de teste e planos</h2>
        <p>
          Criar uma conta no nosso servidor inicia um período de teste gratuito de 14 dias, sem
          cartão. Depois dele, a conta continua no plano Pessoal quando você assina; até as
          assinaturas abrirem, mantemos contas abertas a pedido. Contas que convidamos diretamente
          são cortesia sem data de término, salvo aviso.
        </p>
        <p>
          Uma conta cujo período de teste terminou, ou cujo pagamento falhou, fica{" "}
          <strong>congelada</strong>: o servidor recusa alterações, dispositivos novos e passkeys
          novas, e os apps param de preencher. Você continua lendo seu cofre em todos os
          dispositivos, entrando com passkeys já salvas e exportando, aberto ou criptografado, a
          qualquer momento. Nunca apagamos um cofre por estar congelado; você pode apagar a conta
          pelo app quando quiser.
        </p>
        <p>
          O serviço é prestado com o melhor esforço, sem acordo de nível de serviço, e pode mudar
          ou terminar com aviso. Nada aqui limita seu direito de exportar e sair.
        </p>
```

In `privacy.body`, "This website" section (`en.tsx`): replace the two paragraphs with:

```tsx
        <p>
          havenkeys.net sets no cookies. It is hosted on Vercel, which processes standard request
          logs (such as IP address and browser user agent) to serve the site, and it uses Vercel
          Analytics, which counts page views in aggregate without cookies or cross-site tracking.
          The Download page asks GitHub's public API for the latest release directly from your
          browser, so GitHub also sees that request. If you pick a language with the switcher, the
          site remembers that choice in your browser's local storage; it is never sent anywhere.
          There is no advertising and no other third-party script on the site.
        </p>
        <p>
          The Create account page sends your email address, your language and the version of the
          Terms you accepted to our server at api.havenkeys.net, which emails you a code and, once
          you confirm it, a setup code. The setup code is shown on the page and kept only in the
          page's memory; it is not stored in your browser or in our analytics. Our server records
          your email address and when you accepted the Terms, and sends account notices (the code,
          the setup code, and when a trial is about to end or has ended) through an email provider
          acting as an operator for us; your address is used for nothing else. We never send
          marketing email.
        </p>
```

and in the Portuguese file the equivalent two paragraphs, naming "Criar conta", "api.havenkeys.net", "código de configuração" and "operador". Replace the "Keeping and deleting your data" section's reference to invites, if any, with "accounts never activated within 7 days of sign-up are deleted automatically". Add one sentence to the "The desktop app and browser extension" server paragraph: "It also stores your account's plan status (trial, active, frozen) and when it changed."

Delete the remaining ImprovMX mentions in both files (there is no invite mailbox any more); keep the controller name and contact in "Contact".

`docs/website.md`: add `/signup` and `/pricing` to the page list with one line each, and the note that `/signup` is the only page calling `api.havenkeys.net`.

- [ ] **Step 7: Run everything**

Run: `pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web typecheck && pnpm --filter @havenkeys/web build && pnpm ui:check`
Expected: all PASS; look at the `/signup` and `/pricing` screenshots in both languages and both widths.

- [ ] **Step 8: Commit**

```bash
git add apps/web tools/ui-check docs/website.md
git commit -m "feat(web): pricing page, Create account replaces Request an invite, Terms and Privacy cover trial and signup"
```

---

## Self-review notes

- Spec §7.1 (three steps, Copy, per-OS downloads, "also emailed", memory-only invite, CSP) → Tasks 1, 2, 3.
- Spec §7.2 (`/pricing`, "coming soon") → Task 4.
- Spec §7.3 (Terms: trial, freeze, complimentary, export; Privacy: SMTP operator, email use; `/delete-account` unchanged) → Task 4.
- Spec §7.4 (emails) is Stage 1.
- Spec §8 Web ("the three-step flow; the invite never reaches storage or the URL") → the reducer tests and `Signup.test.tsx`; the storage rule is structural (no `localStorage` call in `Signup.tsx`; `grep -n "Storage" apps/web/src/pages/Signup.tsx` must print nothing, which the reviewer checks).
- Removing "Request an invite" is an addition the spec did not spell out; it follows from the site's purpose in §2 ("the website could not capture the signup" was the reason to reject app-only signup). Keep `deploy/compose` and the self-host docs unchanged: self-hosters still use `admin new-account`.
