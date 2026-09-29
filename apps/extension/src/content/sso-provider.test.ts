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

function make(isTop = true) {
  content = createSsoContent({
    send: async (m) => {
      sent.push(m);
      return undefined;
    },
    viewport: () => ({ width: 1000, height: 800 }),
    suggestions: () => true,
    isTop,
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

  it("does nothing in a subframe (a provider widget embedded in another page)", async () => {
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    make(false).onReady({ kind: "choose", account: "me@gmail.com" });
    document.body.append(document.createElement("div"));
    await vi.advanceTimersByTimeAsync(1000);
    expect(click).not.toHaveBeenCalled();
  });

  it("never clicks on a consent screen", async () => {
    button("Allow");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
    expect(sent).toEqual([{ type: "cs_sso_stop" }]); // the run is over
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
    expect(sent).toEqual([{ type: "cs_sso_stop" }]); // giving up ends the run
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
  });

  it("the user's input before the row appears cancels and ends the run, once", async () => {
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    c.onTrustedInput();
    c.onTrustedInput();
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });

  it("the user's input after the row was clicked still ends the run", async () => {
    row("me@gmail.com");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    c.onTrustedInput();
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });
});

/** Google's "Use another account" entry on the chooser. */
function anotherAccount(): HTMLElement {
  const a = document.createElement("div");
  a.setAttribute("role", "link");
  a.textContent = "Use another account";
  document.body.append(a);
  return a;
}

function input(type: string, name: string): HTMLInputElement {
  const i = document.createElement("input");
  i.type = type;
  i.name = name;
  document.body.append(i);
  return i;
}

describe("handing the provider's login form to the background", () => {
  it("clicks \"Use another account\" once when the row is missing, then hands the email step over", async () => {
    row("you@gmail.com");
    const other = anotherAccount();
    const click = vi.spyOn(other, "click");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    expect(click).not.toHaveBeenCalled(); // the rows may still be rendering
    await vi.advanceTimersByTimeAsync(1000);
    expect(click).toHaveBeenCalledOnce();
    expect(sent).toEqual([]);
    input("email", "identifier");
    await vi.advanceTimersByTimeAsync(1000);
    expect(click).toHaveBeenCalledOnce();
    expect(sent).toEqual([{ type: "cs_sso_login" }]);
    document.body.append(document.createElement("div"));
    await vi.advanceTimersByTimeAsync(CHOOSE_WAIT_MS);
    c.onTrustedInput(); // the background consumed the run: nothing to stop
    expect(sent).toEqual([{ type: "cs_sso_login" }]);
  });

  it("waits for the rows: a saved row rendered after \"Use another account\" wins", async () => {
    const other = anotherAccount();
    const clickOther = vi.spyOn(other, "click");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    await vi.advanceTimersByTimeAsync(100); // within one debounce
    const r = row("me@gmail.com");
    const clickRow = vi.spyOn(r, "click");
    await vi.advanceTimersByTimeAsync(2000);
    expect(clickRow).toHaveBeenCalledOnce();
    expect(clickOther).not.toHaveBeenCalled();
  });

  it("clicks the row, not the hand-off, on a chooser that also shows a login field", () => {
    const r = row("me@gmail.com");
    input("email", "identifier");
    const click = vi.spyOn(r, "click");
    make().onReady({ kind: "choose", account: "me@gmail.com" });
    expect(click).toHaveBeenCalledOnce();
    expect(sent).toEqual([]);
  });

  it("clicks the account row, then hands over a password step in the same document", async () => {
    const r = row("me@gmail.com");
    const click = vi.spyOn(r, "click");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    expect(click).toHaveBeenCalledOnce();
    r.remove();
    input("password", "Passwd");
    await vi.advanceTimersByTimeAsync(1000);
    expect(click).toHaveBeenCalledOnce();
    expect(sent).toEqual([{ type: "cs_sso_login" }]);
  });

  it("hands over a password page in a new document at once; no cs_sso_stop afterwards", async () => {
    input("password", "Passwd");
    const c = make();
    c.onReady({ kind: "login" });
    expect(sent).toEqual([{ type: "cs_sso_login" }]);
    await vi.advanceTimersByTimeAsync(CHOOSE_WAIT_MS);
    c.onTrustedInput();
    expect(sent).toEqual([{ type: "cs_sso_login" }]);
  });

  it("the user's input on a login document before the form appears ends the run, once", async () => {
    const c = make();
    c.onReady({ kind: "login" });
    c.onTrustedInput();
    c.onTrustedInput();
    input("password", "Passwd");
    await vi.advanceTimersByTimeAsync(2000);
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });

  it("a consent screen on a login document ends the run", () => {
    button("Allow");
    input("password", "Passwd");
    make().onReady({ kind: "login" });
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });

  it("consent_stops_before_another_account", async () => {
    button("Allow");
    const other = anotherAccount();
    const click = vi.spyOn(other, "click");
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    input("email", "identifier");
    await vi.advanceTimersByTimeAsync(2000);
    expect(click).not.toHaveBeenCalled();
    expect(sent).toEqual([{ type: "cs_sso_stop" }]); // the run is over; no hand-off
  });

  it("the user's input before the email field appears cancels", async () => {
    row("you@gmail.com");
    anotherAccount();
    const c = make();
    c.onReady({ kind: "choose", account: "me@gmail.com" });
    c.onTrustedInput();
    input("email", "identifier");
    await vi.advanceTimersByTimeAsync(2000);
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });

  it("does nothing in a subframe", async () => {
    input("password", "Passwd");
    const c = make(false);
    c.onReady({ kind: "login" });
    document.body.append(document.createElement("div"));
    await vi.advanceTimersByTimeAsync(CHOOSE_WAIT_MS);
    c.onTrustedInput();
    expect(sent).toEqual([]);
  });

  it("gives up when no form appears within CHOOSE_WAIT_MS and ends the run", async () => {
    const disconnect = vi.spyOn(MutationObserver.prototype, "disconnect");
    const c = make();
    c.onReady({ kind: "login" });
    await vi.advanceTimersByTimeAsync(CHOOSE_WAIT_MS);
    expect(disconnect).toHaveBeenCalled();
    disconnect.mockRestore();
    input("password", "Passwd");
    await vi.advanceTimersByTimeAsync(2000);
    c.onTrustedInput(); // already ended: not sent twice
    expect(sent).toEqual([{ type: "cs_sso_stop" }]);
  });
});
