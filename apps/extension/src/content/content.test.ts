// @vitest-environment jsdom
//
// The content script against a fake `chrome` API. jsdom events dispatched
// from script are untrusted (isTrusted === false), exactly like events a
// hostile page would synthesize, so these tests also show that page script
// cannot open menus or trigger fills.

import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

type Listener = (msg: unknown, sender: { id?: string; tab?: unknown }, reply: (r: unknown) => void) => boolean | void;

const sent: unknown[] = [];
let listener: Listener | null = null;

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
        return { saveToken: null };
      },
      onMessage: { addListener: (l: Listener) => (listener = l) },
    },
  };
  await import("./index");
});

beforeEach(() => {
  sent.length = 0;
  document.body.innerHTML = `<form><h1>Sign in</h1><input name="user" type="email"><input name="pw" type="password">
    <button type="submit">Sign in</button></form>`;
});

function deliver(msg: unknown, sender: { id?: string; tab?: unknown } = { id: "ext" }): unknown {
  let out: unknown;
  listener?.(msg, sender, (r) => (out = r));
  return out;
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
});

describe("automatic sign-in", () => {
  // A valid email format: the beforeEach form's username field is type="email",
  // and a real press goes through the page's own form.requestSubmit(), which
  // (like a real browser) withholds the submit event for a value that fails
  // the field's own HTML validation ("octo" is not a valid email address).
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
