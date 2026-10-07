// @vitest-environment jsdom
//
// The content script against a fake `chrome` API. jsdom events dispatched
// from script are untrusted (isTrusted === false), exactly like events a
// hostile page would synthesize, so these tests also show that page script
// cannot open menus or trigger fills.

import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { markUserEdit } from "../autofill/fill";
import { BUTTON_WAIT_MS, OTP_SETTLE_MS } from "../autofill/submit";
import { SCAN_DEBOUNCE_MS } from "./sso";
import { frameToken } from "./frames";

/** Our frame holding `token`, if one is in the page. */
function frameWith(token: string): HTMLIFrameElement | null {
  return Array.from(document.querySelectorAll("iframe")).find((f) => frameToken(f) === token) ?? null;
}

/**
 * jsdom runs every test as the top frame. A test sets `childAncestry` to
 * play a subframe that reports this ancestry (content/ancestry.ts).
 */
const frames = vi.hoisted(() => ({ childAncestry: null as unknown }));
vi.mock("./ancestry", async (orig) => {
  const real = await orig<typeof import("./ancestry")>();
  return { ...real, frameAncestry: () => (frames.childAncestry as ReturnType<typeof real.frameAncestry>) ?? real.frameAncestry() };
});

type Listener = (msg: unknown, sender: { id?: string; tab?: unknown }, reply: (r: unknown) => void) => boolean | void;

const sent: unknown[] = [];
let listener: Listener | null = null;
/** The reply to the next cs_open_menu (a menu the background registered). */
let openReply: unknown = undefined;
/** The content script's window listeners, so a test can call one with a trusted event object. */
const windowListeners = new Map<string, EventListener[]>();
type StorageListener = (c: Record<string, { newValue?: unknown }>, area: string) => void;
let storageListener: StorageListener | null = null;

beforeAll(async () => {
  // jsdom has no layout: give every element a visible size.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 10,
    y: 10,
    top: 10,
    left: 10,
    bottom: 30,
    right: 210,
    width: 200,
    height: 20,
    toJSON: () => ({}),
  } as DOMRect);
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      id: "ext",
      getURL: (p: string) => `chrome-extension://ext/${p}`,
      sendMessage: async (m: unknown) => {
        sent.push(m);
        if ((m as { type?: string }).type === "cs_open_menu") return openReply;
        return { saveToken: null };
      },
      onMessage: { addListener: (l: Listener) => (listener = l) },
    },
    storage: {
      local: { get: async () => ({}) },
      onChanged: { addListener: (l: StorageListener) => (storageListener = l) },
    },
  };
  // jsdom never marks a dispatched event trusted, so record the listeners
  // the content script installs and call them with a trusted event object
  // (the same approach as menu/click-capture.test-helper.ts).
  const add = window.addEventListener.bind(window);
  vi.spyOn(window, "addEventListener").mockImplementation(((type: string, l: EventListener, o?: AddEventListenerOptions) => {
    windowListeners.set(type, [...(windowListeners.get(type) ?? []), l]);
    add(type, l, o);
  }) as typeof window.addEventListener);
  await import("./index");
});

beforeEach(() => {
  sent.length = 0;
  document.body.innerHTML = `<form><h1>Sign in</h1><input name="user" type="email"><input name="pw" type="password">
    <button type="submit">Sign in</button></form>`;
});

/** The listener's reply: as is when sent at once, a promise of it when the listener replies later (returns true). */
function deliver(msg: unknown, sender: { id?: string; tab?: unknown } = { id: "ext" }): unknown {
  let out: unknown;
  let resolve: (r: unknown) => void = () => undefined;
  const later = new Promise<unknown>((r) => (resolve = r));
  const async = listener?.(msg, sender, (r) => {
    out = r;
    resolve(r);
  });
  return async === true ? later : out;
}

const field = (n: string) => document.querySelector<HTMLInputElement>(`[name="${n}"]`) as HTMLInputElement;
const loginFill = (origin: string) => ({
  type: "bg_fill",
  origin,
  token: null,
  fill: { kind: "login", username: "octo", password: "pw-from-vault" },
  submit: false,
  totp: false,
});

describe("content script", () => {
  it("announces itself once in the top frame, and installs once", async () => {
    expect(listener).not.toBeNull();
    const again = listener;
    await import("./index");
    expect(listener).toBe(again);
  });

  it("refuses_a_popup_fill_into_an_opaque_origin_document", () => {
    // A document served with `Content-Security-Policy: sandbox` keeps its
    // URL (and location.origin) but runs with an opaque origin.
    const real = Object.getOwnPropertyDescriptor(window, "origin");
    Object.defineProperty(window, "origin", { value: "null", configurable: true });
    try {
      expect(deliver(loginFill(location.origin))).toEqual({ filled: 0, pressing: null });
      expect(field("pw").value).toBe("");
    } finally {
      if (real) Object.defineProperty(window, "origin", real);
      else delete (window as { origin?: string }).origin;
    }
    // The same page without the sandbox still fills.
    expect(deliver(loginFill(location.origin))).toEqual({ filled: 2, pressing: null });
  });

  it("fills the page's login form for a popup fill on the matched origin", () => {
    expect(deliver(loginFill(location.origin))).toEqual({ filled: 2, pressing: null });
    expect(field("user").value).toBe("octo");
    expect(field("pw").value).toBe("pw-from-vault");
    expect(document.documentElement.outerHTML).not.toContain("pw-from-vault");
  });

  it("refuses to fill if the frame is on another origin than the desktop matched", () => {
    expect(deliver(loginFill("https://github.com"))).toEqual({ filled: 0, pressing: null });
    expect(field("pw").value).toBe("");
  });

  it("ignores messages that are not from the background worker", () => {
    // From a content script or extension page inside a tab.
    expect(deliver(loginFill(location.origin), { id: "ext", tab: { id: 1 } })).toBeUndefined();
    // From another extension.
    expect(deliver(loginFill(location.origin), { id: "other" })).toBeUndefined();
    // Malformed.
    expect(
      deliver({ type: "bg_fill", origin: location.origin, token: null, fill: { kind: "script" }, submit: false, totp: false }),
    ).toBeUndefined();
    expect(field("pw").value).toBe("");
  });

  it("refuses a menu fill for a menu it never opened", () => {
    const msg = { ...loginFill(location.origin), token: "a".repeat(32) };
    expect(deliver(msg)).toEqual({ filled: 0, pressing: null });
    expect(field("pw").value).toBe("");
  });

  it("page script cannot open menus or report submissions with synthetic events", async () => {
    const pw = field("pw");
    pw.focus();
    pw.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, composed: true }));
    pw.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    pw.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await new Promise((r) => setTimeout(r, 10));
    expect(sent.filter((m) => (m as { type: string }).type === "cs_open_menu")).toEqual([]);
    expect(document.querySelector("iframe")).toBeNull();
  });

  it("shows the HavenKeys icon in a focused login field, and page script cannot click it", async () => {
    const pw = field("pw");
    pw.focus();
    const icon = document.documentElement.querySelector(":scope > div[title='HavenKeys']") as HTMLElement | null;
    expect(icon).not.toBeNull();
    icon?.dispatchEvent(new MouseEvent("click", { bubbles: true, composed: true })); // untrusted
    await new Promise((r) => setTimeout(r, 10));
    expect(sent.filter((m) => (m as { type: string }).type === "cs_open_menu")).toEqual([]);
    pw.blur();
    await new Promise((r) => setTimeout(r, 10));
    expect(document.documentElement.querySelector(":scope > div[title='HavenKeys']")).toBeNull();
  });

  it("hides the field icon while suggestions are off, without a reload", async () => {
    const setPref = (v: boolean) => storageListener?.({ inlineSuggestions: { newValue: v } }, "local");
    const iconEl = () => document.documentElement.querySelector(":scope > div[title='HavenKeys']");
    const pw = field("pw");
    pw.focus();
    expect(iconEl()).not.toBeNull();
    setPref(false);
    expect(iconEl()).toBeNull();
    pw.blur();
    pw.focus();
    expect(iconEl()).toBeNull();
    setPref(true);
    expect(iconEl()).not.toBeNull(); // the focused field gets it back
    pw.blur();
    await new Promise((r) => setTimeout(r, 10));
  });

  it("still shows the save prompt and offers a generated password while suggestions are off", async () => {
    const setPref = (v: boolean) => storageListener?.({ inlineSuggestions: { newValue: v } }, "local");
    setPref(false);
    try {
      const token = "a".repeat(32);
      deliver({ type: "bg_show_save", token });
      const frame = document.documentElement.querySelector("iframe");
      expect(frame?.getAttribute("src") ?? "").toContain("save.html");
      deliver({ type: "bg_close_save", token });

      document.body.innerHTML = `<form><h1>Create account</h1><input name="user" type="email">
        <input name="new" type="password" autocomplete="new-password"><button type="submit">Sign up</button></form>`;
      const reply = deliver({
        type: "bg_fill",
        origin: location.origin,
        token: null,
        fill: { kind: "generated", password: "Gen-pw-1!" },
        submit: false,
        totp: false,
      }) as { filled: number };
      expect(reply.filled).toBeGreaterThan(0);
      sent.length = 0;
      window.dispatchEvent(new Event("pagehide"));
      expect(sent).toContainEqual(expect.objectContaining({ type: "cs_submit", password: "Gen-pw-1!" }));
    } finally {
      setPref(true);
    }
  });

  it("Riot: clicking a form-less, icon-only sign-in button offers the typed login", async () => {
    await new Promise((r) => setTimeout(r, 1100)); // submit debounce
    document.body.innerHTML = `<div><div><input name="username" type="text"></div>
      <div><input name="password" type="password" autocomplete="off"><button type="button" tabindex="-1"><svg aria-label="visibilityOff"></svg></button></div>
      <div><button data-testid="btn-signin-submit"><div><svg role="img" aria-label="forward"><path d="M0"></path></svg></div></button></div></div>`;
    field("username").value = "octo";
    field("password").value = "new-typed-pw";
    markUserEdit(field("username"));
    markUserEdit(field("password"));
    const arrow = document.querySelector("path") as Element;
    for (const l of windowListeners.get("click") ?? []) l({ isTrusted: true, composedPath: () => [arrow] } as unknown as Event);
    expect(sent).toContainEqual({ type: "cs_submit", username: "octo", password: "new-typed-pw" });
  });

  it("the password toggle beside it is not a submit", async () => {
    await new Promise((r) => setTimeout(r, 1100)); // submit debounce
    document.body.innerHTML = `<div><input name="username" type="text"><input name="password" type="password">
      <button type="button" id="eye"><svg aria-label="visibilityOff"></svg></button></div>`;
    field("username").value = "octo";
    field("password").value = "half";
    markUserEdit(field("password"));
    const eye = document.querySelector("#eye") as Element;
    for (const l of windowListeners.get("click") ?? []) l({ isTrusted: true, composedPath: () => [eye] } as unknown as Event);
    expect(sent.filter((m) => (m as { type: string }).type === "cs_submit")).toEqual([]);
  });

  it("page script cannot open menus by synthesizing typing", async () => {
    const pw = field("pw");
    pw.focus();
    pw.dispatchEvent(new InputEvent("input", { bubbles: true, composed: true, data: "x" }));
    await new Promise((r) => setTimeout(r, 10));
    expect(sent.filter((m) => (m as { type: string }).type === "cs_open_menu")).toEqual([]);
    pw.blur();
  });

  it("page script cannot plant a password and forge a submit (no save-prompt oracle)", () => {
    field("user").value = "me@example.com";
    field("pw").value = "guess";
    field("pw").dispatchEvent(new Event("input", { bubbles: true })); // untrusted
    document.querySelector("form")?.dispatchEvent(new Event("submit", { bubbles: true }));
    expect(sent.filter((m) => (m as { type: string }).type === "cs_submit")).toEqual([]);
  });

  it("does not report a password it filled from the vault", async () => {
    await new Promise((r) => setTimeout(r, 1100)); // submit debounce
    deliver(loginFill(location.origin));
    document.querySelector("form")?.dispatchEvent(new Event("submit", { bubbles: true }));
    expect(sent.filter((m) => (m as { type: string }).type === "cs_submit")).toEqual([]);
  });

  it("a page restored from the back/forward cache offers its provider buttons again", async () => {
    const ssoButtons = () => sent.filter((m) => (m as { type: string }).type === "cs_sso_buttons");
    document.body.replaceChildren();
    const b = document.createElement("button");
    b.textContent = "Continue with Google";
    document.body.append(b);
    await new Promise((r) => setTimeout(r, SCAN_DEBOUNCE_MS + 100));
    expect(ssoButtons()).toEqual([{ type: "cs_sso_buttons", providers: ["google"] }]);
    sent.length = 0;
    window.dispatchEvent(new Event("pagehide"));
    window.dispatchEvent(Object.assign(new Event("pageshow"), { persisted: true }));
    await new Promise((r) => setTimeout(r, SCAN_DEBOUNCE_MS + 100));
    expect(ssoButtons()).toEqual([{ type: "cs_sso_buttons", providers: ["google"] }]);
  });
});

describe("automatic sign-in", () => {
  // A valid email format: the beforeEach form's username field is type="email",
  // and a real press clicks the form's button, which (like a real browser)
  // withholds the submit event for a value that fails the field's own HTML
  // validation ("octo" is not a valid email address).
  const autoFill = (over: Record<string, unknown> = {}) => ({
    ...loginFill(location.origin),
    fill: { kind: "login", username: "octo@example.com", password: "pw-from-vault" },
    submit: true,
    totp: false,
    ...over,
  });

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
    const tie = deliver(autoFill());
    await vi.advanceTimersByTimeAsync(BUTTON_WAIT_MS + 500);
    expect(await tie).toEqual({ filled: 2, pressing: null });
    expect(submitted).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("waits for a button the page renders after the fill (Google's password view), then presses it", async () => {
    vi.useFakeTimers();
    document.body.innerHTML = `<main><div><input name="pw" type="password" autocomplete="current-password"></div><div id="footer"></div></main>`;
    const reply = deliver({ ...autoFill(), fill: { kind: "login", username: null, password: "pw-from-vault" } });
    expect(reply).toBeInstanceOf(Promise);
    const next = document.createElement("button");
    next.type = "button";
    next.textContent = "Next";
    const click = vi.fn();
    next.addEventListener("click", click);
    await vi.advanceTimersByTimeAsync(300);
    document.getElementById("footer")?.append(next);
    await vi.advanceTimersByTimeAsync(BUTTON_WAIT_MS);
    expect(await reply).toEqual({ filled: 1, pressing: "password" });
    expect(click).toHaveBeenCalledOnce();
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

  it("presses Verify after an OTP fill once the settle wait is over", async () => {
    vi.useFakeTimers();
    document.body.innerHTML = `<form><label for="c">Authentication code</label>
      <input id="c" name="code" autocomplete="one-time-code" inputmode="numeric"><button type="submit">Verify</button></form>`;
    const submitted = vi.fn((e: Event) => e.preventDefault());
    document.querySelector("form")?.addEventListener("submit", submitted);
    expect(deliver(autoFill({ fill: { kind: "otp", code: "123456" }, totp: true }))).toEqual({ filled: 1, pressing: "otp" });
    await vi.advanceTimersByTimeAsync(OTP_SETTLE_MS - 1);
    expect(submitted).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(submitted).toHaveBeenCalledOnce();
    expect(sent).not.toContainEqual({ type: "cs_run_stop" });
    vi.useRealTimers();
  });

  it("ends the run when the press itself throws", async () => {
    vi.useFakeTimers();
    const button = document.querySelector("form button") as HTMLButtonElement;
    button.click = () => {
      throw new Error("press failed");
    };
    expect(deliver(autoFill({ totp: true }))).toEqual({ filled: 2, pressing: "password" });
    await vi.advanceTimersByTimeAsync(0);
    expect(sent).toContainEqual({ type: "cs_run_stop" });
    vi.useRealTimers();
  });

  it("does not press a button that only enables after bg_run_end ended the run", async () => {
    vi.useFakeTimers();
    document.body.innerHTML = `<form><input name="user" type="email" autocomplete="username"><input name="pw" type="password">
      <button type="submit" disabled>Sign in</button></form>`;
    const submitted = vi.fn((e: Event) => e.preventDefault());
    document.querySelector("form")?.addEventListener("submit", submitted);
    expect(deliver(autoFill())).toEqual({ filled: 2, pressing: "password" });
    // The run ends while the press is still waiting for the button to enable...
    deliver({ type: "bg_run_end" });
    sent.length = 0;
    // ...and only then (as on a real page reacting late) does it enable.
    (document.querySelector("button") as HTMLButtonElement).disabled = false;
    await vi.advanceTimersByTimeAsync(1000);
    expect(submitted).not.toHaveBeenCalled();
    expect(sent).not.toContainEqual({ type: "cs_run_stop" });
    vi.useRealTimers();
  });
});

describe("menuKindFor", () => {
  const set = (html: string) => (document.body.innerHTML = html);
  const f = (sel: string) => document.querySelector(sel) as HTMLInputElement;
  const kindFor = async (el: HTMLInputElement) => (await import("./index")).menuKindFor(el);

  it("opens the identity menu on a checkout's CPF field", async () => {
    set(`<form><input id="n" name="nome" aria-label="Nome completo"><input id="c" name="cpf" aria-label="CPF"><button>Finalizar compra</button></form>`);
    expect(await kindFor(f("#c"))).toEqual({ kind: "identity", roles: ["fullName", "cpf"] });
  });

  // Yahoo's "Create a Yahoo account" (login.yahoo.com/account/create, 2026-10).
  // Its only intent words are "Sign in with Google" (submit) and "Sign in
  // instead": the fields themselves (given-name, family-name, bday-*) say signup.
  const YAHOO_SIGNUP = `<form id="challenge-form" method="post">
    <label for="fn">First name</label><input id="fn" type="text" autocomplete="given-name" name="firstName">
    <label for="ln">Last name</label><input id="ln" type="text" autocomplete="family-name" name="lastName">
    <label for="uid">New Yahoo email</label><input id="uid" type="text" autocomplete="off" name="userId"><span>@yahoo.com</span>
    <label for="pw">Password</label><input id="pw" type="password" autocomplete="new-password" name="password">
    <button type="button" aria-label="Show password"></button>
    <fieldset><legend>Date of birth</legend>
      <label for="mm">Month (MM)</label><input id="mm" type="tel" autocomplete="bday-month" maxlength="2" aria-label="Birthday month" name="mm">
      <label for="dd">Day (DD)</label><input id="dd" type="tel" autocomplete="bday-day" maxlength="2" aria-label="Birthday day" name="dd">
      <label for="yyyy">Year (YYYY)</label><input id="yyyy" type="tel" autocomplete="bday-year" maxlength="4" aria-label="Birthday year" name="yyyy">
    </fieldset>
    <button type="submit">Next</button>
    <button type="submit">Sign in with Google</button>
    <a href="/">Sign in instead</a>
  </form>`;

  it("offers the identity on a signup whose only intent words say sign in (Yahoo)", async () => {
    set(YAHOO_SIGNUP);
    // "New Yahoo email" picks a new address: the saved email does not go there.
    const identity = { kind: "identity", roles: ["firstName", "lastName", "birthMonth", "birthDay", "birthYear"] };
    for (const id of ["#fn", "#ln", "#mm", "#dd", "#yyyy"]) expect(await kindFor(f(id))).toEqual(identity);
    expect(await kindFor(f("#pw"))).toEqual({ kind: "new_password" });
  });

  it("keeps a sign-in form a login when it asks only for login fields", async () => {
    set(`<form><input id="u" autocomplete="username"><input id="p" type="password" autocomplete="current-password"><button type="submit">Sign in with Google</button><a href="/">Sign in instead</a></form>`);
    expect(await kindFor(f("#u"))).toEqual({ kind: "login" });
  });

  it("keeps the login menu on a gov.br-style CPF login", async () => {
    set(`<form><h1>Entrar</h1><input id="c" name="cpf" aria-label="CPF"><button type="submit">Entrar</button></form>`);
    expect((await kindFor(f("#c")))?.kind).toBe("login");
  });

  it("adds identity roles to a sign-up form's email field", async () => {
    set(`<form><h1>Create account</h1><input id="n" name="first_name" aria-label="First name"><input id="e" type="email" name="email"><input type="password" name="pw" autocomplete="new-password"><button type="submit">Sign up</button></form>`);
    const k = await kindFor(f("#e"));
    expect(k?.kind).toBe("login");
    expect(k?.roles).toContain("firstName");
  });

  it("never offers the identity on a login form", async () => {
    set(`<form><h1>Sign in</h1><input id="e" type="email" name="email"><input type="password" name="pw"><button type="submit">Sign in</button></form>`);
    expect(await kindFor(f("#e"))).toEqual({ kind: "login" });
  });

  it("offers nothing when every identity field already holds the user's own value", async () => {
    set(`<form><input id="n" name="nome" aria-label="Nome completo"><input id="c" name="cpf" aria-label="CPF"><button>Finalizar compra</button></form>`);
    for (const el of [f("#n"), f("#c")]) {
      el.value = "typed";
      markUserEdit(el);
    }
    // The name field is no login field: nothing to offer. (The CPF field falls
    // back to what it would get without an identity: the login menu.)
    expect(await kindFor(f("#n"))).toBeNull();
    expect(await kindFor(f("#c"))).toEqual({ kind: "login" });
    expect(deliver({ type: "bg_identity_roles" })).toEqual({ roles: [] });
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

  const idFill = (token: string | null, values = [{ role: "fullName", value: "Samuel" }, { role: "postalCode", value: "70000-000" }]) => ({
    type: "bg_fill",
    origin: location.origin,
    token,
    fill: { kind: "identity", values },
    submit: false,
    totp: false,
  });
  const byId = (id: string) => document.getElementById(id) as HTMLInputElement;
  let seq = 0;

  /**
   * Opens the field menu on `el` as a trusted click does, then closes it the
   * way a pick does (bg_close_menu): the content script keeps it as `picked`.
   */
  async function pick(el: HTMLInputElement): Promise<string> {
    const token = (++seq).toString(16).padStart(32, "c");
    openReply = { ok: true, token, rows: 1 };
    el.focus();
    for (const l of windowListeners.get("pointerdown") ?? []) l({ isTrusted: true, composedPath: () => [el] } as unknown as Event);
    await new Promise((r) => setTimeout(r, 10));
    expect(sent).toContainEqual(expect.objectContaining({ type: "cs_open_menu", kind: "identity" }));
    deliver({ type: "bg_close_menu", token });
    openReply = undefined;
    return token;
  }

  const TWO_FORMS = `<form id="a"><input id="an" name="nome" aria-label="Nome completo"><input id="ac" name="cep" aria-label="CEP"></form>
    <form id="b"><input id="bn" name="nome" aria-label="Nome completo"><input id="bc" name="cep" aria-label="CEP"></form>`;

  it("fills the picked form, and only that one, for its menu's token", async () => {
    document.body.innerHTML = TWO_FORMS;
    const token = await pick(byId("bc"));
    expect(deliver(idFill(token))).toEqual({ filled: 2, pressing: null });
    expect(byId("bn").value).toBe("Samuel");
    expect(byId("bc").value).toBe("70000-000");
    expect(byId("an").value).toBe("");
    expect(byId("ac").value).toBe("");
  });

  it("fills nothing for another menu's token", async () => {
    document.body.innerHTML = TWO_FORMS;
    await pick(byId("bc"));
    expect(deliver(idFill("f".repeat(32)))).toEqual({ filled: 0, pressing: null });
    for (const id of ["an", "ac", "bn", "bc"]) expect(byId(id).value, id).toBe("");
  });

  it("fills nothing once the pick has expired", async () => {
    document.body.innerHTML = TWO_FORMS;
    const token = await pick(byId("bc"));
    const now = Date.now();
    const clock = vi.spyOn(Date, "now").mockReturnValue(now + 31_000);
    try {
      expect(deliver(idFill(token))).toEqual({ filled: 0, pressing: null });
    } finally {
      clock.mockRestore();
    }
    expect(byId("bn").value).toBe("");
  });

  it("fills nothing when the picked field left the page", async () => {
    document.body.innerHTML = TWO_FORMS;
    const token = await pick(byId("bc"));
    const b = document.getElementById("b") as HTMLFormElement;
    b.remove();
    expect(deliver(idFill(token))).toEqual({ filled: 0, pressing: null });
    expect(byId("an").value).toBe("");
    expect((b.querySelector("#bn") as HTMLInputElement).value).toBe("");
  });

  it("leaves a login form's fields alone", async () => {
    document.body.innerHTML = `<form><h1>Sign in</h1><input id="le" name="user" type="email"><input id="lp" name="pw" type="password">
      <button type="submit">Sign in</button></form>
      <form id="b"><input id="bn" name="nome" aria-label="Nome completo"><input id="bc" name="cep" aria-label="CEP"></form>`;
    const token = await pick(byId("bc"));
    const values = [
      { role: "fullName", value: "Samuel" },
      { role: "postalCode", value: "70000-000" },
      { role: "email", value: "me@example.com" },
      { role: "username", value: "samuel" },
    ];
    expect(deliver(idFill(token, values))).toEqual({ filled: 2, pressing: null });
    expect(byId("le").value).toBe("");
    expect(byId("lp").value).toBe("");
  });

  it("answers the popup's role scan", () => {
    document.body.innerHTML = `<form><input name="nome" aria-label="Nome completo"><input name="cpf" aria-label="CPF"></form>`;
    expect(deliver({ type: "bg_identity_roles" })).toEqual({ roles: ["fullName", "cpf"] });
  });
});

describe("card fields", () => {
  const set = (html: string) => (document.body.innerHTML = html);
  const f = (sel: string) => document.querySelector(sel) as HTMLInputElement;
  const kindFor = async (el: HTMLInputElement) => (await import("./index")).menuKindFor(el);
  const CHECKOUT = `<form><input id="num" autocomplete="cc-number"><input id="exp" autocomplete="cc-exp" placeholder="MM/AA"><input id="cvv" autocomplete="cc-csc" maxlength="4"><button type="submit">Pagar</button></form>`;
  const cardFill = (token: string | null) => ({
    type: "bg_fill",
    origin: location.origin,
    token,
    fill: { kind: "card", values: [{ role: "number", value: "4111111111111111" }, { role: "expiryMonth", value: "4" }, { role: "expiryYear", value: "2033" }] },
    submit: false,
    totp: false,
  });
  const host = (token: string, frameId: number, url: string, rows = 1) => ({
    type: "bg_host_menu",
    token,
    frameId,
    url,
    anchor: { top: 0, left: 0, width: 200, height: 20 },
    rows,
  });
  const runtime = () => chrome.runtime as unknown as { getFrameId?: (el: Element) => number };
  const CHILD = { ancestors: ["https://shop.example"] };

  it("opens the card menu on a card field, with the roles it can fill", async () => {
    set(CHECKOUT);
    expect(await kindFor(f("#num"))).toEqual({ kind: "card", cardRoles: ["number", "expiryMonth", "expiryYear", "verificationNumber"] });
  });

  it("fills a card for a fill on this origin, without a token (another frame of the tab)", () => {
    set(CHECKOUT);
    const reply = deliver(cardFill(null));
    expect(reply).toEqual({ filled: 2, pressing: null });
    expect(f("#exp").value).toBe("04/33");
    expect(document.documentElement.outerHTML).not.toContain("4111");
  });

  it("refuses a card fill for another origin, or for a menu it never opened", () => {
    set(CHECKOUT);
    expect(deliver({ ...cardFill(null), origin: "https://evil.example" })).toEqual({ filled: 0, pressing: null });
    expect(deliver(cardFill("a".repeat(32)))).toEqual({ filled: 0, pressing: null });
    expect(f("#num").value).toBe("");
  });

  it("reports its card fields to a scan", () => {
    set(CHECKOUT);
    sent.length = 0;
    deliver({ type: "bg_card_scan", scan: "c".repeat(32) });
    expect(sent).toContainEqual({ type: "cs_card_fields", scan: "c".repeat(32), roles: ["number", "expiryMonth", "expiryYear", "verificationNumber"] });
  });

  it("says nothing to a scan without card fields", () => {
    sent.length = 0;
    deliver({ type: "bg_card_scan", scan: "c".repeat(32) });
    expect(sent).toEqual([]);
  });

  it("hosts a child frame's menu over that frame's iframe", () => {
    set(`<iframe id="stripe"></iframe>`);
    runtime().getFrameId = (el) => (el.id === "stripe" ? 7 : -1);
    try {
      const reply = deliver(host("d".repeat(32), 7, "https://js.stripe.com/v3/card.html", 2));
      expect(reply).toEqual({ ok: true });
      expect(frameWith("d".repeat(32))).not.toBeNull();
      expect(deliver(host("e".repeat(32), 9, "https://js.stripe.com/v3/card.html"))).toEqual({ ok: false });
      deliver({ type: "bg_close_menu", token: "d".repeat(32) });
      expect(frameWith("d".repeat(32))).toBeNull();
    } finally {
      delete runtime().getFrameId;
    }
  });

  it("without getFrameId (Chromium), hosts over the one iframe whose src is the frame's URL", () => {
    set(`<iframe id="stripe" src="https://js.stripe.com/v3/card.html#frame"></iframe><iframe src="https://other.example/"></iframe>`);
    expect(deliver(host("d".repeat(32), 7, "https://js.stripe.com/v3/card.html"))).toEqual({ ok: true });
    expect(frameWith("d".repeat(32))).not.toBeNull();
    deliver({ type: "bg_close_menu", token: "d".repeat(32) });
    expect(frameWith("d".repeat(32))).toBeNull();

    // Same path twice: the exact full URL picks one; a URL matching neither exactly finds none.
    set(`<iframe src="https://js.stripe.com/v3/card.html"></iframe><iframe src="https://js.stripe.com/v3/card.html?b"></iframe>`);
    expect(deliver(host("e".repeat(32), 7, "https://js.stripe.com/v3/card.html?b"))).toEqual({ ok: true });
    deliver({ type: "bg_close_menu", token: "e".repeat(32) });
    expect(deliver(host("f".repeat(32), 7, "https://js.stripe.com/v3/card.html?c"))).toEqual({ ok: false });
    expect(frameWith("f".repeat(32))).toBeNull();
  });

  describe("in a processor subframe", () => {
    beforeEach(() => {
      frames.childAncestry = CHILD;
    });
    afterEach(() => {
      frames.childAncestry = null;
      openReply = undefined;
    });

    it("reports its ancestry with its card fields", () => {
      set(CHECKOUT);
      sent.length = 0;
      deliver({ type: "bg_card_scan", scan: "c".repeat(32) });
      expect(sent).toContainEqual({
        type: "cs_card_fields",
        scan: "c".repeat(32),
        roles: ["number", "expiryMonth", "expiryYear", "verificationNumber"],
        ancestry: CHILD,
      });
    });

    it("asks with its anchor and ancestry, lets the top frame draw a hosted menu, and fills the picked field", async () => {
      set(CHECKOUT);
      sent.length = 0;
      const token = "9".repeat(32);
      openReply = { ok: true, token, rows: 1, hosted: true };
      const num = f("#num");
      num.focus();
      for (const l of windowListeners.get("pointerdown") ?? []) l({ isTrusted: true, composedPath: () => [num] } as unknown as Event);
      await new Promise((r) => setTimeout(r, 10));
      expect(sent).toContainEqual({
        type: "cs_open_menu",
        kind: "card",
        cardRoles: ["number", "expiryMonth", "expiryYear", "verificationNumber"],
        anchor: { top: 10, left: 10, width: 200, height: 20 },
        ancestry: CHILD,
        viewport: { width: window.innerWidth, height: window.innerHeight },
      });
      // The top frame draws it: no menu iframe here.
      expect(document.querySelector("iframe")).toBeNull();
      // Focus leaving for the top frame's menu does not close it.
      num.blur();
      for (const l of windowListeners.get("focusout") ?? []) l({ isTrusted: true, composedPath: () => [num] } as unknown as Event);
      await new Promise((r) => setTimeout(r, 10));
      expect(sent.some((m) => (m as { type: string }).type === "cs_close_menu")).toBe(false);
      // The pick closes it here (bg_close_menu); the token-bound fill finds the field.
      deliver({ type: "bg_close_menu", token });
      expect(deliver(cardFill(token))).toEqual({ filled: 2, pressing: null });
      expect(f("#num").value).toBe("4111111111111111");
    });
  });

  it("a top-frame card menu asks without an anchor or ancestry", async () => {
    set(CHECKOUT);
    sent.length = 0;
    const token = "8".repeat(32);
    openReply = { ok: true, token, rows: 1 };
    const num = f("#num");
    num.focus();
    for (const l of windowListeners.get("pointerdown") ?? []) l({ isTrusted: true, composedPath: () => [num] } as unknown as Event);
    await new Promise((r) => setTimeout(r, 10));
    openReply = undefined;
    expect(sent).toContainEqual({ type: "cs_open_menu", kind: "card", cardRoles: ["number", "expiryMonth", "expiryYear", "verificationNumber"] });
    expect(frameWith(token)).not.toBeNull();
    deliver({ type: "bg_close_menu", token });
    expect(frameWith(token)).toBeNull();
  });

  it("offers to save a masked number the user typed, and not one a page script wrote", async () => {
    set(CHECKOUT);
    sent.length = 0;
    const typed = (sel: string, v: string) => {
      f(sel).value = v;
      markUserEdit(f(sel));
    };
    typed("#num", "4000056655665556");
    f("#num").value = "4000 0566 5566 5556"; // the site's mask
    typed("#exp", "01/30");
    document.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true }));
    expect(sent).toContainEqual({
      type: "cs_card_submit",
      card: { number: "4000056655665556", expiry: "2030-01", verificationNumber: null, cardholderName: null },
    });

    set(CHECKOUT);
    sent.length = 0;
    f("#num").value = "4000056655665556"; // page script
    typed("#exp", "01/30");
    await new Promise((r) => setTimeout(r, 1100)); // past the 1 s submit debounce
    document.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true }));
    expect(sent.some((m) => (m as { type: string }).type === "cs_card_submit")).toBe(false);
  });
});
