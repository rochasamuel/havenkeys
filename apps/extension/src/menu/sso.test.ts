// @vitest-environment jsdom
//
// The "sign in with" balloon against a fake background: offering a saved
// login for a provider's button, confirming the account to save, and the
// "couldn't find the button" notice.
//
// jsdom never marks a script-dispatched event isTrusted (real for every DOM
// implementation, not a jsdom quirk — see content.test.ts), so the guard's
// arm delay is exercised here by capturing the click listeners a row/button
// registers and invoking them directly with a trusted event object. That
// covers the guard's own timing; a genuine untrusted click sending nothing,
// the security-relevant case, is covered with a real dispatched click.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const TOKEN = "c".repeat(32);
const ID1 = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
let replies: unknown[] = [];
const asked: unknown[] = [];

function page(): void {
  document.body.replaceChildren();
  const card = document.createElement("div");
  card.className = "card save sso";
  const header = document.createElement("header");
  const site = document.createElement("span");
  site.id = "site";
  const close = document.createElement("button");
  close.id = "close";
  header.append(site, close);
  const main = document.createElement("div");
  main.id = "main";
  main.className = "prompt";
  const heading = document.createElement("p");
  heading.id = "heading";
  main.append(heading);
  card.append(header, main);
  document.body.append(card);
  location.hash = TOKEN;
}

beforeEach(() => {
  vi.useFakeTimers();
  replies = [];
  asked.length = 0;
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      sendMessage: async (m: unknown) => {
        asked.push(m);
        return replies.length > 0 ? replies.shift() : undefined;
      },
    },
  };
  page();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

async function openPrompt(view: object, advanceMs = 1000): Promise<void> {
  replies = [{ ok: true, value: view }];
  vi.resetModules();
  await import("./sso");
  await vi.advanceTimersByTimeAsync(advanceMs);
}

/** Captures 'click' listeners as they're registered, so a handler gated on
 * `e.isTrusted` can be exercised directly without jsdom's permanent false. */
function captureClicks(): Map<Element, (e: Partial<MouseEvent>) => void> {
  const handlers = new Map<Element, (e: Partial<MouseEvent>) => void>();
  const orig = EventTarget.prototype.addEventListener;
  vi.spyOn(EventTarget.prototype, "addEventListener").mockImplementation(function (
    this: EventTarget,
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions,
  ) {
    if (type === "click" && listener && this instanceof Element) handlers.set(this, listener as (e: Partial<MouseEvent>) => void);
    return orig.call(this, type, listener, options);
  });
  return handlers;
}

describe("sso offer view", () => {
  it("renders one row per candidate, with the provider name and account", async () => {
    await openPrompt({
      mode: "offer",
      site: "github.com",
      rows: [
        { id: ID1, provider: "google", title: "Personal", account: "me@example.com" },
        { id: "11111111-2222-4333-8444-555555555555", provider: "github", title: "Work", account: null },
      ],
    });
    const rows = document.querySelectorAll("button.row");
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain("Personal");
    expect(rows[0]!.textContent).toContain("Google");
    expect(rows[0]!.textContent).toContain("me@example.com");
    expect(rows[1]!.textContent).toContain("Sign in with GitHub");
  });

  it("ignores a click before the guard arms, then sends sso_pick once it does", async () => {
    const handlers = captureClicks();
    await openPrompt(
      { mode: "offer", site: "github.com", rows: [{ id: ID1, provider: "google", title: "Personal", account: "me@example.com" }] },
      0, // resolve init() only; the guard's own delay has not elapsed
    );
    const row = document.querySelector("button.row") as HTMLButtonElement;
    const handler = handlers.get(row)!;
    const picks = () => asked.filter((m) => (m as { type?: string }).type === "sso_pick");

    handler({ isTrusted: true } as MouseEvent);
    await vi.advanceTimersByTimeAsync(0);
    expect(picks()).toEqual([]);

    await vi.advanceTimersByTimeAsync(500);
    replies.push({ ok: true, value: null }); // the sso_pick reply; the background closes the frame on success
    handler({ isTrusted: true } as MouseEvent);
    await vi.advanceTimersByTimeAsync(0);
    expect(picks()).toEqual([{ type: "sso_pick", token: TOKEN, itemId: ID1 }]);

    // A synthetic (untrusted) click, armed or not, must never send.
    row.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await vi.advanceTimersByTimeAsync(0);
    expect(picks()).toEqual([{ type: "sso_pick", token: TOKEN, itemId: ID1 }]);
  });
});

describe("sso save view", () => {
  it("shows the account, and hides the title field for an update", async () => {
    await openPrompt({ mode: "save", site: "github.com", provider: "google", account: "me@example.com", title: null, action: "update" });
    const inputs = document.querySelectorAll("input");
    expect(inputs).toHaveLength(1);
    expect(inputs[0]!.value).toBe("me@example.com");
  });

  it("shows an empty account and a title field for a new login", async () => {
    await openPrompt({ mode: "save", site: "github.com", provider: "github", account: null, title: "GitHub", action: "add" });
    const inputs = document.querySelectorAll("input");
    expect(inputs).toHaveLength(2);
    expect(inputs[0]!.value).toBe("");
    expect(inputs[1]!.value).toBe("GitHub");
  });

  it("builds the save request from the token, the account field and the title (add only)", async () => {
    const { saveRequest } = await import("./sso");
    expect(saveRequest(TOKEN, "me@example.com", "GitHub")).toEqual({ type: "sso_save", token: TOKEN, account: "me@example.com", title: "GitHub" });
    expect(saveRequest(TOKEN, "", null)).toEqual({ type: "sso_save", token: TOKEN, account: "", title: null });
  });
});

describe("sso notice view", () => {
  it("shows the no-button message and closes on an armed, trusted click", async () => {
    const handlers = captureClicks();
    await openPrompt({ mode: "notice", site: "github.com", provider: "google" });
    expect(document.getElementById("heading")?.textContent).toBe('Couldn’t find the “Sign in with Google” button on this page.');
    const buttons = Array.from(document.querySelectorAll("button")).filter((b) => b.textContent === "Close");
    expect(buttons).toHaveLength(1);
    const handler = handlers.get(buttons[0]!)!;

    buttons[0]!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await vi.advanceTimersByTimeAsync(0);
    expect(asked.some((m) => (m as { type?: string }).type === "sso_dismiss")).toBe(false);

    handler({ isTrusted: true } as MouseEvent);
    await vi.advanceTimersByTimeAsync(0);
    expect(asked).toContainEqual({ type: "sso_dismiss", token: TOKEN });
  });
});
