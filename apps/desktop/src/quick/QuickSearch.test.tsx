// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../lib/api";
import type { ItemOverview } from "../lib/types";

const listItems = vi.fn<(q?: string) => Promise<ItemOverview[]>>();
const copy = vi.fn();
const openWebsite = vi.fn();
const calls: string[] = [];

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    listItems: (q?: string) => listItems(q),
    copy: (id: string, field: string) => copy(id, field),
    openWebsite: (id: string, url: string) => openWebsite(id, url),
  };
  return {
    ...original,
    api: new Proxy(known, {
      get: (target, name: string) => {
        calls.push(name);
        return name in target
          ? target[name]
          : name.startsWith("on")
            ? () => Promise.resolve(() => undefined)
            : () => Promise.resolve(null);
      },
    }),
  };
});

import { en } from "../i18n/en";
import { QuickSearch } from "./QuickSearch";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// Every api call that hands a secret value (or a code) to JavaScript.
const SECRET_READS = /^(reveal\w*|totp|loginFieldTotp)$/;
const SECRET_READS_IN_SOURCE = /api\.(reveal\w*|totp|loginFieldTotp)\b/;

function login(id: string, title: string, extra: Partial<ItemOverview> = {}): ItemOverview {
  return {
    id,
    itemType: "login",
    title,
    username: `${title.toLowerCase()}@example.com`,
    urls: [{ url: `https://${title.toLowerCase()}.example`, matchType: "domain" }],
    hasPassword: true,
    hasTotp: false,
    hasNotes: false,
    hasPasskey: false,
    autoSignIn: true,
    tags: [],
    createdAt: 1,
    updatedAt: 1,
    ...extra,
  } as ItemOverview;
}
const GITHUB = login("a", "GitHub", { hasTotp: true });
const BANK = login("b", "Bank", { username: null, urls: [] });
const NOTE = { ...login("n", "Wi-Fi"), itemType: "secure_note" } as ItemOverview;

let host: HTMLElement;
let root: Root;
const onHide = vi.fn();

beforeEach(() => {
  document.documentElement.dataset.platform = "";
  listItems.mockReset().mockResolvedValue([GITHUB, NOTE, BANK]);
  copy.mockReset().mockResolvedValue({ clearAfterSeconds: 30 });
  openWebsite.mockReset().mockResolvedValue(undefined);
  onHide.mockReset();
  calls.length = 0;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function render() {
  await act(async () => root.render(<QuickSearch onHide={onHide} />));
}
const field = () => host.querySelector<HTMLInputElement>("input[role=combobox]")!;
const rows = () => [...host.querySelectorAll("[role=option]")];
const selected = () => host.querySelector("[role=option][aria-selected=true]")?.textContent ?? "";
async function type(value: string) {
  await act(async () => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    setter.call(field(), value);
    field().dispatchEvent(new Event("input", { bubbles: true }));
  });
}
async function key(k: string, init: KeyboardEventInit = {}) {
  let event!: KeyboardEvent;
  await act(async () => {
    event = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...init });
    field().dispatchEvent(event);
  });
  return event;
}

describe("QuickSearch", () => {
  it("lists logins only, with title and username, and focuses the field", async () => {
    await render();
    expect(rows().map((r) => r.textContent)).toEqual([
      expect.stringContaining("GitHub"),
      expect.stringContaining("Bank"),
    ]);
    expect(rows()[0]!.textContent).toContain("github@example.com");
    expect(document.activeElement).toBe(field());
  });

  it("searches with list_items as the user types", async () => {
    await render();
    listItems.mockResolvedValue([BANK]);
    await type("ban");
    expect(listItems).toHaveBeenLastCalledWith("ban");
    expect(rows()).toHaveLength(1);
    expect(selected()).toContain("Bank");
  });

  it("ignores an answer for an older query", async () => {
    let slow!: (items: ItemOverview[]) => void;
    await render();
    listItems.mockImplementationOnce(() => new Promise((r) => (slow = r)));
    listItems.mockResolvedValueOnce([BANK]);
    await type("g");
    await type("ba");
    await act(async () => slow([GITHUB]));
    expect(rows().map((r) => r.textContent)).toEqual([expect.stringContaining("Bank")]);
  });

  it("moves the selection with the arrows and stops at the ends", async () => {
    await render();
    expect(selected()).toContain("GitHub");
    await key("ArrowDown");
    expect(selected()).toContain("Bank");
    await key("ArrowDown");
    expect(selected()).toContain("Bank");
    await key("ArrowUp");
    await key("ArrowUp");
    expect(selected()).toContain("GitHub");
  });

  it("copies the password on Enter, in Rust, and hides", async () => {
    await render();
    await key("Enter");
    expect(copy).toHaveBeenCalledWith("a", "password");
    expect(onHide).toHaveBeenCalledTimes(1);
  });

  it("copies the username, the code and opens the website with Ctrl", async () => {
    await render();
    await key("c", { ctrlKey: true });
    expect(copy).toHaveBeenLastCalledWith("a", "username");
    await key("t", { ctrlKey: true });
    expect(copy).toHaveBeenLastCalledWith("a", "totp");
    await key("o", { ctrlKey: true });
    expect(openWebsite).toHaveBeenCalledWith("a", "https://github.example");
    expect(onHide).toHaveBeenCalledTimes(3);
  });

  it("uses Cmd on macOS", async () => {
    document.documentElement.dataset.platform = "mac";
    await render();
    await key("c", { metaKey: true });
    expect(copy).toHaveBeenCalledWith("a", "username");
  });

  it("does nothing for a code, username or website the login does not have", async () => {
    await render();
    await key("ArrowDown"); // Bank: no TOTP, no username, no website
    await key("t", { ctrlKey: true });
    await key("c", { ctrlKey: true });
    await key("o", { ctrlKey: true });
    expect(copy).not.toHaveBeenCalled();
    expect(openWebsite).not.toHaveBeenCalled();
    expect(onHide).not.toHaveBeenCalled();
  });

  it("stays open and says so when copying fails", async () => {
    copy.mockRejectedValue(new ApiError("clipboard", "Could not access the clipboard."));
    await render();
    await key("Enter");
    expect(onHide).not.toHaveBeenCalled();
    expect(host.querySelector("[role=alert]")?.textContent).toBe(en.errors.codes.clipboard);
  });

  it("leaves Cmd/Ctrl+C alone with no results", async () => {
    listItems.mockResolvedValue([]);
    await render();
    expect(host.textContent).toContain(en.quick.noLogins);
    const event = await key("c", { ctrlKey: true });
    expect(event.defaultPrevented).toBe(false);
    expect(copy).not.toHaveBeenCalled();
    await type("zzz");
    expect(host.textContent).toContain(en.quick.noResults);
    await key("Enter");
    expect(onHide).not.toHaveBeenCalled();
  });

  it("offers only the keys the selected login has", async () => {
    await render();
    const hints = () => host.querySelector(".quick-hints")!.textContent ?? "";
    expect(hints()).toContain(en.quick.hints.password);
    expect(hints()).toContain(en.quick.hints.totp);
    await key("ArrowDown"); // Bank
    expect(hints()).not.toContain(en.quick.hints.totp);
    expect(hints()).not.toContain(en.quick.hints.username);
    expect(hints()).not.toContain(en.quick.hints.open);
    listItems.mockResolvedValue([]);
    await type("zzz");
    expect(hints()).not.toContain(en.quick.hints.password);
    expect(hints()).toContain(en.quick.hints.close);
  });

  it("keeps focus in the field", async () => {
    await render();
    const tab = await key("Tab");
    expect(tab.defaultPrevented).toBe(true);
    const down = new MouseEvent("mousedown", { bubbles: true, cancelable: true });
    host.querySelector(".quick-hints")!.dispatchEvent(down);
    expect(down.defaultPrevented).toBe(true);
  });

  it("drops the old rows when a search fails", async () => {
    await render();
    listItems.mockRejectedValue(new ApiError("vault_locked", "The vault is locked."));
    await type("x");
    expect(rows()).toHaveLength(0);
    await key("Enter");
    await key("c", { ctrlKey: true });
    expect(copy).not.toHaveBeenCalled();
    expect(host.querySelector("[role=alert]")?.textContent).not.toBe("");
  });

  it("leaves Cmd/Ctrl+C to text copy when the query is selected", async () => {
    await render();
    await type("git");
    field().setSelectionRange(0, 3);
    const event = await key("c", { ctrlKey: true });
    expect(event.defaultPrevented).toBe(false);
    expect(copy).not.toHaveBeenCalled();
    field().setSelectionRange(3, 3);
    await key("c", { ctrlKey: true });
    expect(copy).toHaveBeenCalledWith("a", "username");
  });

  it("hides on Esc", async () => {
    await render();
    await key("Escape");
    expect(onHide).toHaveBeenCalledTimes(1);
  });

  it("copies the password when a row is clicked", async () => {
    await render();
    await act(async () => (rows()[1]!.querySelector("button") ?? rows()[1]!).dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(copy).toHaveBeenCalledWith("b", "password");
  });

  it("never asks Rust for a secret value", async () => {
    await render();
    await key("Enter");
    await key("t", { ctrlKey: true });
    expect(calls.filter((name) => SECRET_READS.test(name))).toEqual([]);
    // And the source never mentions them, so a later edit cannot either.
    for (const file of ["./QuickSearch.tsx", "./QuickApp.tsx", "./QuickUnlock.tsx"]) {
      const source = readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8");
      expect(source, file).not.toMatch(SECRET_READS_IN_SOURCE);
    }
  });

  it("the secret-read pattern covers every reveal API", () => {
    for (const name of ["reveal", "revealCard", "revealIdentity", "revealCardField", "revealAccountSecretKey", "revealLoginField", "revealPreviousPassword", "totp", "loginFieldTotp"]) {
      expect(name).toMatch(SECRET_READS);
      expect(`api.${name}(`).toMatch(SECRET_READS_IN_SOURCE);
    }
    expect("copy").not.toMatch(SECRET_READS);
  });

  it("says so when a clicked login has no password, and stays open", async () => {
    listItems.mockResolvedValue([login("p", "Pinless", { hasPassword: false })]);
    await render();
    await act(async () => rows()[0]!.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    expect(host.querySelector("[role=alert]")!.textContent).toBe(en.quick.noPassword);
    expect(copy).not.toHaveBeenCalled();
    expect(onHide).not.toHaveBeenCalled();
  });

  it("announces the keys the selected login answers to", async () => {
    await render();
    expect(field().getAttribute("aria-keyshortcuts")).toBe("Enter Control+C Control+T Control+O");
    document.documentElement.dataset.platform = "mac";
    await act(async () => root.render(<QuickSearch key="m" onHide={onHide} />));
    expect(field().getAttribute("aria-keyshortcuts")).toBe("Enter Meta+C Meta+T Meta+O");
  });
});
