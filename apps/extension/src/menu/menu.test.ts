// @vitest-environment jsdom
//
// The suggestion menu against a fake background: passkey hints leading and
// trailing the item list, and that the directory help row only asks the
// background on a trusted, armed click (see common.ts's click guard).

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { captureClicks } from "./click-capture.test-helper";

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

describe("field menu focus", () => {
  const view = (n: number) => ({
    ok: true,
    value: {
      state: "ready",
      kind: "login",
      site: "accounts.google.com",
      items: Array.from({ length: n }, (_, i) => ({ id: ITEM, title: `Login ${i}`, username: `u${i}@x.io`, provider: null })),
      passkeys: [],
      hint: null,
    },
  });

  it("starts on the first row when focused from the page's keyboard (ArrowDown)", async () => {
    replies = [view(8)];
    await load();
    window.dispatchEvent(new FocusEvent("focus"));
    expect((document.activeElement as HTMLElement).textContent).toContain("Login 0");
  });

  it("leaves focus and scroll alone when a click focuses the frame", async () => {
    // A click into a scrolled list: pointerdown, then the frame gains focus.
    // Moving focus to the first row would scroll the list, so the click would
    // land on another row and pick nothing.
    replies = [view(8)];
    await load();
    const rows = document.querySelectorAll<HTMLButtonElement>("button.row");
    rows[6]?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    window.dispatchEvent(new FocusEvent("focus"));
    expect(document.activeElement).not.toBe(rows[0]);
  });
});

describe("identity row", () => {
  const ready = (identity: object, kind = "identity") => ({
    ok: true,
    value: { state: "ready", kind, site: "shop.com", items: [], passkeys: [], hint: null, identity },
  });
  const trusted = { isTrusted: true } as MouseEvent;

  /** Loads with the click guard armed; returns the captured click handlers. */
  async function setup(identity: object, kind = "identity") {
    const handlers = captureClicks();
    replies = [ready(identity, kind)];
    await load();
    // Every later request (resize, pick) succeeds, whatever order they arrive in.
    (globalThis as unknown as { chrome: { runtime: { sendMessage: unknown } } }).chrome.runtime.sendMessage = async (m: unknown) => {
      asked.push(m);
      return { ok: true, value: null };
    };
    await vi.advanceTimersByTimeAsync(1000); // let the click guard arm
    return handlers;
  }

  async function click(handlers: ReturnType<typeof captureClicks>, el: Element): Promise<void> {
    handlers.get(el)?.(trusted);
    await vi.advanceTimersByTimeAsync(0);
  }

  it("asks before filling documents, naming them", async () => {
    const handlers = await setup({ title: "Samuel Rocha", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false });
    const row = document.querySelector<HTMLButtonElement>("button.row")!;
    expect(row.textContent).toContain("Samuel Rocha");
    await click(handlers, row);
    expect(asked.some((m) => (m as { type: string }).type === "menu_pick_identity")).toBe(false);
    expect(document.body.textContent).toContain("shop.com also asks for: CPF");
    const [withDocs, withoutDocs] = Array.from(document.querySelectorAll<HTMLButtonElement>("#main button"));
    expect(withDocs!.textContent).toBe("Fill CPF too");
    await click(handlers, withoutDocs!);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: false });
  });

  it("fills documents from the step's primary button", async () => {
    const handlers = await setup({ title: "Samuel", fills: 3, documents: ["cpf", "rg"], documentsAllowed: true, empty: false });
    await click(handlers, document.querySelector("button.row")!);
    expect(document.body.textContent).toContain("CPF and RG");
    await click(handlers, document.querySelector("#main button")!);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: true });
  });

  it("ignores untrusted clicks on the step's buttons", async () => {
    const handlers = await setup({ title: "Samuel", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false });
    await click(handlers, document.querySelector("button.row")!);
    const before = asked.length;
    handlers.get(document.querySelector("#main button")!)?.({ isTrusted: false } as MouseEvent);
    await vi.advanceTimersByTimeAsync(0);
    expect(asked.length).toBe(before);
  });

  it("says documents are not filled on http and fills without them, no step", async () => {
    const handlers = await setup({ title: "Samuel", fills: 2, documents: ["cpf"], documentsAllowed: false, empty: false });
    const row = document.querySelector<HTMLButtonElement>("button.row")!;
    expect(row.textContent).toContain("Documents are not filled on http pages");
    await click(handlers, row);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: false });
  });

  it("fills straight away when the form asks for no documents", async () => {
    const handlers = await setup({ title: "Samuel", fills: 2, documents: [], documentsAllowed: true, empty: false });
    await click(handlers, document.querySelector("button.row")!);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: false });
  });

  it("opens the desktop app for an empty identity", async () => {
    const handlers = await setup({ title: "", fills: 0, documents: [], documentsAllowed: true, empty: true });
    const row = document.querySelector<HTMLButtonElement>("button.row")!;
    expect(row.textContent).toContain("Your identity is empty");
    await click(handlers, row);
    expect(asked.at(-1)).toEqual({ type: "menu_open_identity", token: TOKEN });
  });

  it("shows the row under a sign-up form's logins", async () => {
    replies = [ready({ title: "Samuel", fills: 1, documents: [], documentsAllowed: true, empty: false }, "login")];
    await load();
    expect(document.querySelectorAll("button.row")).toHaveLength(1);
  });

  it("says so when an identity menu has no identity", async () => {
    replies = [ready(null as unknown as object)];
    await load();
    expect(document.body.textContent).toContain("Nothing to fill in this form.");
  });
});
