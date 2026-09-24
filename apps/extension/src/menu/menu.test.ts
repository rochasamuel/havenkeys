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

afterEach(() => vi.useRealTimers());

describe("field menu passkey hints", () => {
  it("renders the use-passkey hint first as plain text and the help row last as a button", async () => {
    replies = [
      {
        ok: true,
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo" }], passkeys: [], hint: { kind: "use_passkey" } },
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
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo" }], passkeys: [], hint: { kind: "add_passkey", name: "GitHub" } },
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
        value: { state: "ready", kind: "login", site: "github.com", items: [{ id: ITEM, title: "GitHub", username: "octo" }], passkeys: [], hint: null },
      },
    ];
    await load();
    const main = document.getElementById("main")!;
    expect(main.querySelectorAll("button.row")).toHaveLength(1);
    expect(main.querySelectorAll(".row.hint")).toHaveLength(0);
  });
});
