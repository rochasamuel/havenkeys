// @vitest-environment jsdom
//
// The "Sign in with" content logic on an ordinary site (not a provider
// origin). Provider-origin behaviour is in sso-provider.test.ts.

import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { SsoContentRequest } from "../messaging/sso";
import { createSsoContent, SCAN_DEBOUNCE_MS, SCAN_WINDOW_MS } from "./sso";

const TOKEN = "0123456789abcdef0123456789abcdef";

beforeAll(() => {
  // jsdom has no layout: give every element a visible size.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 10, y: 10, top: 10, left: 10, bottom: 30, right: 210, width: 200, height: 20, toJSON: () => ({}),
  } as DOMRect);
  (globalThis as { chrome?: unknown }).chrome = { runtime: { getURL: (p: string) => `chrome-extension://ext/${p}` } };
});

let content: ReturnType<typeof createSsoContent> | null = null;
let sent: SsoContentRequest[] = [];
let reply: unknown = { ok: false };
let suggestions = true;

function make(isTop = true) {
  content = createSsoContent({
    send: async (m) => {
      sent.push(m);
      return reply;
    },
    viewport: () => ({ width: 1000, height: 800 }),
    suggestions: () => suggestions,
    isTop,
  });
  return content;
}

function button(text: string): HTMLButtonElement {
  const b = document.createElement("button");
  b.textContent = text;
  document.body.append(b);
  return b;
}

const ssoFrames = () => Array.from(document.querySelectorAll("iframe")).filter((f) => f.src.includes("sso.html"));
const buttonsSent = () => sent.filter((m) => m.type === "cs_sso_buttons");

beforeEach(() => {
  vi.useFakeTimers();
  sent = [];
  reply = { ok: false };
  suggestions = true;
  document.body.replaceChildren();
  history.replaceState(null, "", "/login");
});

afterEach(() => {
  content?.teardown();
  content = null;
  vi.useRealTimers();
});

describe("scanning for provider buttons", () => {
  it("offers the providers found, and shows the balloon the background allows", async () => {
    button("Continue with Google");
    reply = { ok: true, token: TOKEN };
    make().watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    expect(sent).toEqual([{ type: "cs_sso_buttons", providers: ["google"] }]);
    const [frame] = ssoFrames();
    expect(frame?.src).toContain(`sso.html#${TOKEN}`);
    expect(frame?.title).toBe("Sign in with HavenKeys");
  });

  it("does not re-send the same providers on mutations; a new one sends the sorted set", async () => {
    button("Continue with Google");
    make().watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    document.body.append(document.createElement("div"));
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(buttonsSent()).toHaveLength(1);
    button("Sign in with GitHub");
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    expect(buttonsSent()).toEqual([
      { type: "cs_sso_buttons", providers: ["google"] },
      { type: "cs_sso_buttons", providers: ["github", "google"] },
    ]);
  });

  it("sends nothing while in-page suggestions are off", async () => {
    suggestions = false;
    button("Continue with Google");
    make().watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(sent).toEqual([]);
  });

  it("sends nothing from a subframe", async () => {
    button("Continue with Google");
    make(false).watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(sent).toEqual([]);
  });

  it("sends nothing on a page without provider buttons", async () => {
    button("Sign in");
    make().watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(sent).toEqual([]);
  });

  it("stops observing after the scan window", async () => {
    make().watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_WINDOW_MS + 1);
    button("Continue with Google");
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(sent).toEqual([]);
  });

  it("a closed balloon stays closed on this page, and may return after navigation", async () => {
    button("Continue with Google");
    reply = { ok: true, token: TOKEN };
    const c = make();
    c.watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    expect(ssoFrames()).toHaveLength(1);
    c.handleBackground({ type: "bg_sso_close", token: TOKEN });
    expect(ssoFrames()).toHaveLength(0);

    // The background would re-offer; the content side does not ask on this page.
    button("Sign in with GitHub");
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(buttonsSent()).toHaveLength(1);

    history.pushState(null, "", "/login/next");
    document.body.append(document.createElement("div"));
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    expect(buttonsSent()).toHaveLength(2);
    expect(ssoFrames()).toHaveLength(1);
  });

  it("a closed balloon stays closed on a query or hash change alone: only the pathname brings it back", async () => {
    button("Continue with Google");
    reply = { ok: true, token: TOKEN };
    const c = make();
    c.watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    c.handleBackground({ type: "bg_sso_close", token: TOKEN });
    expect(ssoFrames()).toHaveLength(0);

    history.pushState(null, "", "/login?next=%2Fsettings#top");
    document.body.append(document.createElement("div"));
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    expect(ssoFrames()).toHaveLength(0);
    expect(buttonsSent()).toHaveLength(1);
  });

  it("a balloon the page removes is not offered again on this page", async () => {
    button("Continue with Google");
    reply = { ok: true, token: TOKEN };
    make().watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    ssoFrames()[0]?.remove();
    await vi.advanceTimersByTimeAsync(0);
    expect(ssoFrames()).toHaveLength(0);
    button("Sign in with GitHub");
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS * 2);
    expect(buttonsSent()).toHaveLength(1);
  });

  it("ignores a close or resize for another balloon", async () => {
    button("Continue with Google");
    reply = { ok: true, token: TOKEN };
    const c = make();
    c.watchPage();
    await vi.advanceTimersByTimeAsync(SCAN_DEBOUNCE_MS);
    c.handleBackground({ type: "bg_sso_close", token: "f".repeat(32) });
    expect(ssoFrames()).toHaveLength(1);
    c.handleBackground({ type: "bg_sso_resize", token: TOKEN, height: 200 });
    expect(ssoFrames()[0]?.style.height).toBe("200px");
  });
});

describe("the user's clicks", () => {
  it("records a click on a provider button, not on a plain sign-in button", () => {
    const c = make();
    c.onTrustedClick(button("Continue with Google"));
    c.onTrustedClick(button("Sign in"));
    expect(sent).toEqual([{ type: "cs_sso_click", provider: "google" }]);
  });

  it("recognises a click inside the button, and a bare name the user chose", () => {
    const c = make();
    const b = button("");
    const span = document.createElement("span");
    span.textContent = "GitHub";
    b.append(span);
    c.onTrustedClick(span);
    expect(sent).toEqual([{ type: "cs_sso_click", provider: "github" }]);
  });

  it("does not report accounts off the provider's origins", () => {
    const c = make();
    const row = document.createElement("div");
    row.setAttribute("role", "link");
    row.textContent = "Me me@gmail.com";
    document.body.append(row);
    c.onTrustedClick(row);
    expect(sent).toEqual([]);
  });
});

describe("pressing after a pick", () => {
  it("presses the provider's button, and the user's next input ends the run once", () => {
    const b = button("Continue with Google");
    const click = vi.spyOn(b, "click");
    const c = make();
    expect(c.handleBackground({ type: "bg_sso_press", provider: "google", origin: location.origin })).toEqual({ pressed: true });
    expect(click).toHaveBeenCalledOnce();
    c.onTrustedInput();
    c.onTrustedInput();
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });

  it("does not press without a button, and input then sends nothing", () => {
    button("Continue with GitHub");
    const c = make();
    expect(c.handleBackground({ type: "bg_sso_press", provider: "google", origin: location.origin })).toEqual({ pressed: false });
    c.onTrustedInput();
    expect(sent).toEqual([]);
  });

  it("does not press with a visible challenge on the page", () => {
    const b = button("Continue with Google");
    const click = vi.spyOn(b, "click");
    const captcha = document.createElement("iframe");
    captcha.src = "https://challenges.cloudflare.com/turnstile";
    document.body.append(captcha);
    expect(make().handleBackground({ type: "bg_sso_press", provider: "google", origin: location.origin })).toEqual({ pressed: false });
    expect(click).not.toHaveBeenCalled();
  });

  it("does not press when the button is ambiguous", () => {
    button("Continue with Google");
    button("Sign in with Google");
    expect(make().handleBackground({ type: "bg_sso_press", provider: "google", origin: location.origin })).toEqual({ pressed: false });
  });

  it("A1: does not press when the message's origin does not match this page (a stale press after the frame navigated)", () => {
    const b = button("Continue with Google");
    const click = vi.spyOn(b, "click");
    expect(make().handleBackground({ type: "bg_sso_press", provider: "google", origin: "https://evil.com" })).toEqual({ pressed: false });
    expect(click).not.toHaveBeenCalled();
  });

  it("a choose instruction off the provider's origins does nothing", async () => {
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    const row = document.createElement("div");
    row.setAttribute("role", "link");
    row.textContent = "me@gmail.com";
    const click = vi.spyOn(row, "click");
    document.body.append(row);
    await vi.advanceTimersByTimeAsync(1000);
    expect(click).not.toHaveBeenCalled();
    c.onReady(null);
  });
});
