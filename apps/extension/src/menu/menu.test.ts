// @vitest-environment jsdom
//
// The suggestion menu against a fake background: passkey hints leading and
// trailing the item list, and that the directory help row only asks the
// background on a trusted, armed click (see common.ts's click guard).

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const TOKEN = "a".repeat(32);
const ITEM = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
let replies: unknown[] = [];
const asked: unknown[] = [];

function page(): void {
  document.body.replaceChildren();
  const card = document.createElement("div");
  card.className = "card";
  const header = document.createElement("header");
  const site = document.createElement("span");
  site.id = "site";
  header.append(site);
  const main = document.createElement("main");
  main.id = "main";
  card.append(header, main);
  document.body.append(card);
  location.hash = TOKEN;
}

async function load(): Promise<void> {
  vi.resetModules();
  await import("./menu");
  await vi.advanceTimersByTimeAsync(0);
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

describe("field menu passkey hints", () => {
  it("renders the use-passkey hint first as plain text and the help row last as a button", async () => {
    replies = [
      {
        ok: true,
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo", provider: null }], passkeys: [], hint: { kind: "use_passkey" } },
      },
    ];
    await load();
    const main = document.getElementById("main")!;
    expect(main.firstElementChild?.tagName).toBe("DIV");
    expect(main.firstElementChild?.textContent).toContain("You have a passkey for github.com");
    expect(main.querySelectorAll("button.row")).toHaveLength(1);
  });

  it("renders the directory row last and sends menu_open_help only for a trusted, armed click", async () => {
    replies = [
      {
        ok: true,
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo", provider: null }], passkeys: [], hint: { kind: "add_passkey", name: "GitHub" } },
      },
    ];
    await load();
    const rows = document.querySelectorAll<HTMLButtonElement>("button.row");
    const last = rows[rows.length - 1]!;
    expect(last.textContent).toContain("GitHub supports passkeys");
    last.click(); // untrusted
    expect(asked.some((m) => (m as { type: string }).type === "menu_open_help")).toBe(false);
  });

  it("shows no hint row when the menu has none", async () => {
    replies = [
      {
        ok: true,
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo", provider: null }], passkeys: [], hint: null },
      },
    ];
    await load();
    const main = document.getElementById("main")!;
    expect(main.querySelectorAll("button.row")).toHaveLength(1);
    expect(main.querySelectorAll(".row.hint")).toHaveLength(0);
  });
});

describe("field menu copy and size", () => {
  it("lets its own copy wrap but truncates user data", async () => {
    replies = [
      {
        ok: true,
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: null, provider: null }], passkeys: [], hint: null },
      },
    ];
    await load();
    const row = document.querySelector("button.row")!;
    expect(row.querySelector(".title")?.classList.contains("copy")).toBe(false);
    expect(row.querySelector(".user")?.textContent).toBe("No username");
    expect(row.querySelector(".user")?.classList.contains("copy")).toBe(true);
  });

  it("reports the height its rows need so the frame can grow for wrapped rows", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const height = this.tagName === "HEADER" ? 34 : this.classList.contains("row") ? 64 : 0;
      return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: height, width: 0, height, toJSON: () => ({}) } as DOMRect;
    });
    replies = [
      {
        ok: true,
        value: {
          state: "ready",
          kind: "login",
          site: "github.com",
          items: [
            { id: ITEM, title: "GitHub", username: "octo" },
            { id: "11111111-2222-4333-8444-555555555555", title: "GitHub work", username: "work" },
          ],
          passkeys: [],
          hint: null,
        },
      },
    ];
    await load();
    await vi.advanceTimersByTimeAsync(50);
    expect(asked).toContainEqual({ type: "menu_resize", token: TOKEN, height: 34 + 2 * 64 });
  });
});

describe("field menu sign-in-with rows", () => {
  it("shows the provider icon and ssoRow detail, and picks by itemId", async () => {
    const handlers = captureClicks();
    replies = [
      {
        ok: true,
        value: {
          state: "ready",
          kind: "login",
          site: "typeform.com",
          items: [{ id: ITEM, title: "Typeform", username: "me@gmail.com", provider: "google" }],
          passkeys: [],
          hint: null,
        },
      },
    ];
    await load();
    await vi.advanceTimersByTimeAsync(1000); // let the click guard arm
    const row = document.querySelector("button.row") as HTMLButtonElement;
    expect(row.querySelector(".user")?.textContent).toBe("Google · me@gmail.com");
    expect(row.querySelector("svg.provider-icon")).not.toBeNull();
    expect(row.querySelector(".avatar")?.textContent).toBe("");

    replies.push({ ok: true, value: null });
    handlers.get(row)?.({ isTrusted: true } as MouseEvent);
    await vi.advanceTimersByTimeAsync(0);
    expect(asked).toContainEqual({ type: "menu_pick", token: TOKEN, itemId: ITEM });
  });
});
