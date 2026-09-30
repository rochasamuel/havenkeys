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

  /** Let the documents step's own click guard arm. */
  const armStep = () => vi.advanceTimersByTimeAsync(400);

  it("asks before filling documents, naming them", async () => {
    const handlers = await setup({ title: "Samuel Rocha", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false });
    const row = document.querySelector<HTMLButtonElement>("button.row")!;
    expect(row.textContent).toContain("Samuel Rocha");
    await click(handlers, row);
    await armStep();
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
    await armStep();
    await click(handlers, document.querySelector("#main button")!);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_identity", token: TOKEN, documents: true });
  });

  it("re-arms the guard when the step appears: a fast second click confirms nothing", async () => {
    const handlers = await setup({ title: "Samuel", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false });
    await click(handlers, document.querySelector("button.row")!);
    const [withDocs, withoutDocs] = Array.from(document.querySelectorAll<HTMLButtonElement>("#main button"));
    const picks = () => asked.filter((m) => (m as { type: string }).type === "menu_pick_identity");
    await vi.advanceTimersByTimeAsync(100);
    await click(handlers, withDocs!);
    await click(handlers, withoutDocs!);
    expect(picks()).toEqual([]);
    await vi.advanceTimersByTimeAsync(300);
    await click(handlers, withDocs!);
    expect(picks()).toEqual([{ type: "menu_pick_identity", token: TOKEN, documents: true }]);
  });

  it("moves focus to the step's safe button, never the documents one", async () => {
    const handlers = await setup({ title: "Samuel", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false });
    const row = document.querySelector<HTMLButtonElement>("button.row")!;
    row.focus();
    await click(handlers, row);
    const [withDocs, withoutDocs] = Array.from(document.querySelectorAll<HTMLButtonElement>("#main button"));
    expect(document.activeElement).toBe(withoutDocs);
    expect(document.activeElement).not.toBe(withDocs);
  });

  it("ignores untrusted clicks on the step's buttons", async () => {
    const handlers = await setup({ title: "Samuel", fills: 3, documents: ["cpf"], documentsAllowed: true, empty: false });
    await click(handlers, document.querySelector("button.row")!);
    await armStep();
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

  it("says to open HavenKeys, with no button, when the identity does not exist yet", async () => {
    await setup({ title: "", fills: 0, documents: [], documentsAllowed: false, empty: true, missing: true });
    expect(document.querySelectorAll("#main button")).toHaveLength(0);
    expect(document.getElementById("main")?.textContent).toContain("Open the HavenKeys app to add your details.");
    expect(asked.some((m) => (m as { type: string }).type === "menu_open_identity")).toBe(false);
  });

  it("opens the desktop app for an empty identity", async () => {
    const handlers = await setup({ title: "", fills: 0, documents: [], documentsAllowed: true, empty: true, missing: false });
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

describe("card rows", () => {
  const trusted = { isTrusted: true } as MouseEvent;
  const view = (cards: object[], insecure = false) => ({ ok: true, value: { state: "cards", site: "shop.com", cards, insecure } });
  const visa = { id: ITEM, title: "Visa", brand: "visa", last4: "1111", expiry: "04/33", expired: false };

  async function setup(v: object) {
    const handlers = captureClicks();
    replies = [v];
    await load();
    (globalThis as unknown as { chrome: { runtime: { sendMessage: unknown } } }).chrome.runtime.sendMessage = async (m: unknown) => {
      asked.push(m);
      return { ok: true, value: null };
    };
    await vi.advanceTimersByTimeAsync(1000);
    return handlers;
  }

  it("lists cards with their logo, last four and expiry, and names the page", async () => {
    const handlers = await setup(view([visa, { ...visa, id: "11111111-2222-4333-8444-555555555555", title: "Old", expiry: "01/20", expired: true }]));
    expect(document.getElementById("site")!.textContent).toBe("Fill on");
    expect(document.querySelector(".dest-host")!.textContent).toBe("shop.com");
    const rows = Array.from(document.querySelectorAll<HTMLButtonElement>("button.row"));
    expect(rows[0]!.textContent).toContain("•••• 1111 · 04/33");
    expect(rows[0]!.querySelector("svg")).not.toBeNull();
    expect(rows[1]!.classList.contains("expired")).toBe(true);
    expect(rows[1]!.textContent).toContain("Expired");
    handlers.get(rows[0]!)?.(trusted);
    await vi.advanceTimersByTimeAsync(0);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_card", token: TOKEN, itemId: ITEM });
  });

  it("keeps the end of a long look-alike host visible, on its own line", async () => {
    const evil = "checkout.magazineluiza.com.br.pagamento-seguro.evil.xyz";
    await setup({ ok: true, value: { state: "cards", site: evil, cards: [visa], insecure: false } });
    const host = document.querySelector<HTMLElement>(".dest-host")!;
    // Its own element, not inside the header's one-line label.
    expect(document.getElementById("site")!.contains(host)).toBe(false);
    const shown = host.textContent!;
    expect(shown.startsWith("…")).toBe(true);
    expect(shown.endsWith(".evil.xyz")).toBe(true);
    expect(shown.length).toBeLessThanOrEqual(33);
    // Cut at a label boundary: whole labels after the ellipsis.
    expect(evil.endsWith(shown.slice(1))).toBe(true);
    expect(evil[evil.length - shown.length]).toBe(".");
    expect(host.title).toBe(evil);
  });

  it("keeps a long last label rather than snapping to a short tail", async () => {
    const long = `shop.com.br.${"m".repeat(40)}.xyz`;
    await setup({ ok: true, value: { state: "cards", site: long, cards: [visa], insecure: false } });
    const shown = document.querySelector(".dest-host")!.textContent!;
    expect(shown).toBe(`…${long.slice(long.length - 31)}`);
  });

  it("shows a short host whole", async () => {
    await setup({ ok: true, value: { state: "cards", site: "magazineluiza.com.br", cards: [visa], insecure: false } });
    expect(document.querySelector(".dest-host")!.textContent).toBe("magazineluiza.com.br");
  });

  it("a synthetic click on a card row sends nothing", async () => {
    await setup(view([visa]));
    document.querySelector<HTMLButtonElement>("button.row")!.click();
    await vi.advanceTimersByTimeAsync(0);
    expect(asked.some((m) => (m as { type: string }).type === "menu_pick_card")).toBe(false);
  });

  it("draws the logo on a white tile, at the card's aspect ratio; other brands get the generic card", async () => {
    await setup(view([visa, { ...visa, id: "11111111-2222-4333-8444-555555555555", brand: "other" }, { ...visa, id: "11111111-2222-4333-8444-666666666666", brand: null }]));
    const svgs = Array.from(document.querySelectorAll<SVGSVGElement>("button.row svg"));
    expect(svgs).toHaveLength(3);
    for (const s of svgs) {
      expect(s.getAttribute("viewBox")).toBe("0 0 780 500");
      expect(s.getAttribute("width")).toBe("24");
      expect(s.getAttribute("height")).toBe(String(Math.round((24 * 500) / 780)));
      const tile = s.firstElementChild!;
      expect(tile.tagName).toBe("rect");
      expect(tile.getAttribute("fill")).toBe("#ffffff");
      expect(tile.getAttribute("width")).toBe("780");
    }
    const { CARD_BRAND_ICONS, GENERIC_CARD_ICON } = await import("@havenkeys/ui/card-brand-icons");
    expect(svgs[0]!.childElementCount).toBe(CARD_BRAND_ICONS.visa.shapes.length + 1);
    expect(svgs[1]!.childElementCount).toBe(GENERIC_CARD_ICON.shapes.length + 1);
    expect(svgs[2]!.innerHTML).toBe(svgs[1]!.innerHTML);
  });

  it("explains http pages without buttons", async () => {
    await setup(view([], true));
    expect(document.body.textContent).toContain("HavenKeys fills cards only on secure (https) pages.");
    expect(document.querySelector("button.row")).toBeNull();
  });

  it("says when no card is saved, without buttons", async () => {
    await setup(view([]));
    expect(document.body.textContent).toContain("No cards saved");
    expect(document.querySelector("button.row")).toBeNull();
  });

  it("shows the locked message when the vault is locked", async () => {
    await setup({ ok: true, value: { state: "locked" } });
    expect(document.querySelector("button.row")).toBeNull();
    expect(document.getElementById("main")!.textContent).not.toBe("");
  });
});
