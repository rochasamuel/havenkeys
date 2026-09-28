// @vitest-environment jsdom
// @vitest-environment-options {"url": "https://accounts.google.com/"}
//
// The "Sign in with" content logic on a provider's own origin: learning the
// account the user clicks, and clicking the saved account on the chooser.

import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { SsoContentRequest } from "../messaging/sso";
import { CHOOSE_WAIT_MS, createSsoContent } from "./sso";

beforeAll(() => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 10, y: 10, top: 10, left: 10, bottom: 30, right: 210, width: 200, height: 20, toJSON: () => ({}),
  } as DOMRect);
  (globalThis as { chrome?: unknown }).chrome = { runtime: { getURL: (p: string) => `chrome-extension://ext/${p}` } };
});

let content: ReturnType<typeof createSsoContent> | null = null;
let sent: SsoContentRequest[] = [];

function make() {
  content = createSsoContent({
    send: async (m) => {
      sent.push(m);
      return undefined;
    },
    viewport: () => ({ width: 1000, height: 800 }),
    suggestions: () => true,
    isTop: true,
  });
  return content;
}

/** A Google-style chooser row: a role=link div holding a name and an email. */
function row(email: string): HTMLElement {
  const r = document.createElement("div");
  r.setAttribute("role", "link");
  r.setAttribute("data-identifier", email);
  const name = document.createElement("div");
  name.textContent = "Me";
  const addr = document.createElement("div");
  addr.textContent = email;
  r.append(name, addr);
  document.body.append(r);
  return r;
}

function button(text: string): HTMLButtonElement {
  const b = document.createElement("button");
  b.textContent = text;
  document.body.append(b);
  return b;
}

beforeEach(() => {
  vi.useFakeTimers();
  sent = [];
  document.body.replaceChildren();
});

afterEach(() => {
  content?.teardown();
  content = null;
  vi.useRealTimers();
});

describe("learning the account", () => {
  it("reports the email in the chooser row the user clicked", () => {
    expect(location.origin).toBe("https://accounts.google.com");
    const r = row("Me@Gmail.com");
    make().onTrustedClick(r.lastElementChild as Element);
    expect(sent).toEqual([{ type: "cs_sso_account", account: "me@gmail.com" }]);
  });

  it("does not report text from outside a row", () => {
    const p = document.createElement("p");
    p.textContent = "Signed in as someone@gmail.com";
    document.body.append(p);
    make().onTrustedClick(p);
    expect(sent).toEqual([]);
  });
});

describe("choosing the saved account", () => {
  it("clicks the row once it appears", async () => {
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    await vi.advanceTimersByTimeAsync(1000);
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(1000);
    expect(click).toHaveBeenCalledOnce();
  });

  it("clicks a row already on the page", () => {
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    make().onReady({ kind: "choose", account: "me@gmail.com" });
    expect(click).toHaveBeenCalledOnce();
  });

  it("never clicks on a consent screen", async () => {
    button("Allow");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
  });

  it("does not click when two rows hold the account", async () => {
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    const a = row("me@gmail.com");
    const b = row("me@gmail.com");
    const ca = vi.spyOn(a, "click");
    const cb = vi.spyOn(b, "click");
    await vi.advanceTimersByTimeAsync(1000);
    expect(ca).not.toHaveBeenCalled();
    expect(cb).not.toHaveBeenCalled();
  });

  it("gives up after CHOOSE_WAIT_MS", async () => {
    const disconnect = vi.spyOn(MutationObserver.prototype, "disconnect");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    await vi.advanceTimersByTimeAsync(CHOOSE_WAIT_MS);
    expect(disconnect).toHaveBeenCalled();
    disconnect.mockRestore();
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
  });

  it("the user's input before the row appears cancels", async () => {
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    c.onTrustedInput();
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
    expect(sent).toEqual([]);
  });
});
