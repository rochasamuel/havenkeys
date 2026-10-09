// @vitest-environment jsdom
//
// The toolbar popup's identity row against a fake background: the
// documents question names the site, the answer carries the origin it was
// asked for, and no button can send a second request while one is in flight.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../i18n/en";

const asked: unknown[] = [];
/** Replies by request type; a function reply can hold the request open. */
let replies: Record<string, (m: unknown) => Promise<unknown>> = {};

function page(): void {
  document.body.innerHTML = `<span id="state" hidden></span><button id="options"></button>
    <main id="main"></main><button id="lock" hidden><span id="lock-label"></span></button>`;
}

async function load(): Promise<void> {
  vi.resetModules();
  await import("./popup");
  await vi.waitFor(() => expect(document.querySelector(".item.identity")).not.toBeNull());
}

const fillButton = () => document.querySelector<HTMLButtonElement>(".item.identity .actions .btn")!;
const statusButtons = () => Array.from(document.querySelectorAll<HTMLButtonElement>(".item.identity .row-status .btn"));
const identityRequests = () => asked.filter((m) => (m as { type: string }).type === "popup_fill_identity");
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  asked.length = 0;
  page();
  replies = {
    popup_state: async () => ({
      ok: true,
      value: { kind: "unlocked", site: "shop.com", matches: [], identity: { title: "Samuel" } },
    }),
  };
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      sendMessage: async (m: unknown) => {
        asked.push(m);
        const r = replies[(m as { type: string }).type];
        return r ? r(m) : { ok: false, message: "x" };
      },
      openOptionsPage: () => undefined,
    },
    permissions: { contains: async () => true, request: async () => true },
  };
  vi.spyOn(window, "close").mockImplementation(() => undefined);
});

describe("popup identity fill", () => {
  it("names the site in the documents question and sends the answer with its origin", async () => {
    replies.popup_fill_identity = async (m) =>
      (m as { documents: unknown }).documents === null
        ? { ok: true, value: { confirm: ["cpf"], origin: "https://shop.com" } }
        : { ok: true, value: null };
    await load();
    fillButton().click();
    await vi.waitFor(() => expect(statusButtons()).toHaveLength(2));
    expect(document.querySelector(".item.identity .who .user")?.textContent).toBe("shop.com also asks for CPF.");
    expect(statusButtons().map((b) => b.textContent)).toEqual(["Fill CPF too", "Without them"]);
    expect(fillButton().hidden).toBe(true);
    statusButtons()[0]!.click();
    await flush();
    expect(identityRequests()).toEqual([
      { type: "popup_fill_identity", documents: null },
      { type: "popup_fill_identity", documents: true, origin: "https://shop.com" },
    ]);
  });

  it("disables Fill while its request is in flight", async () => {
    let answer: (v: unknown) => void = () => undefined;
    replies.popup_fill_identity = () => new Promise((r) => (answer = r));
    await load();
    fillButton().click();
    await flush();
    expect(fillButton().disabled).toBe(true);
    fillButton().click();
    await flush();
    expect(identityRequests()).toHaveLength(1);
    answer({ ok: false, message: "No form for your identity on this page." });
    await flush();
    expect(fillButton().disabled).toBe(false);
  });

  it("disables Fill and both answers while an answer is in flight", async () => {
    let answer: (v: unknown) => void = () => undefined;
    replies.popup_fill_identity = (m) =>
      (m as { documents: unknown }).documents === null
        ? Promise.resolve({ ok: true, value: { confirm: ["cpf"], origin: "https://shop.com" } })
        : new Promise((r) => (answer = r));
    await load();
    fillButton().click();
    await vi.waitFor(() => expect(statusButtons()).toHaveLength(2));
    const [yes, no] = statusButtons();
    no!.click();
    await flush();
    expect([fillButton(), yes!, no!].map((b) => b.disabled)).toEqual([true, true, true]);
    yes!.click();
    fillButton().click();
    await flush();
    expect(identityRequests()).toHaveLength(2);
    answer({ ok: false, message: "The page changed to another site. Open HavenKeys again to fill your identity." });
    await flush();
    expect(fillButton().disabled).toBe(false);
    expect(document.querySelector(".item.identity .error")?.textContent).toContain("The page changed");
  });
});

describe("popup unlock", () => {
  async function loadLocked(): Promise<HTMLButtonElement> {
    replies.popup_state = async () => ({ ok: true, value: { kind: "locked" } });
    vi.resetModules();
    await import("./popup");
    await vi.waitFor(() => expect(document.querySelector(".notice .btn")).not.toBeNull());
    return document.querySelector<HTMLButtonElement>(".notice .btn")!;
  }
  const unlockRequests = () => asked.filter((m) => (m as { type: string }).type === "popup_show_unlock");

  it("offers Unlock while locked and closes once the desktop is raised", async () => {
    replies.popup_show_unlock = async () => ({ ok: true, value: null });
    const unlock = await loadLocked();
    expect(unlock.textContent).toBe("Unlock");
    unlock.click();
    await flush();
    expect(unlockRequests()).toEqual([{ type: "popup_show_unlock" }]);
    expect(window.close).toHaveBeenCalled();
  });

  it("shows the error and stays open when the desktop refuses", async () => {
    replies.popup_show_unlock = async () => ({ ok: false, message: "Too many requests." });
    const unlock = await loadLocked();
    unlock.click();
    await vi.waitFor(() => expect(document.querySelector(".notice .error")?.textContent).toBe("Too many requests."));
    expect(window.close).not.toHaveBeenCalled();
    expect(unlock.disabled).toBe(false);
  });

  it("has no password field: the password is typed in the desktop app", async () => {
    await loadLocked();
    expect(document.querySelector("input")).toBeNull();
  });
});

describe("popup cards", () => {
  it("draws the logins first, then the cards above the in-page offer", async () => {
    const VISA = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
    let answer: (v: unknown) => void = () => undefined;
    replies.popup_cards = () => new Promise((r) => (answer = r));
    (globalThis as { chrome: { permissions: unknown } }).chrome.permissions = { contains: async () => false, request: async () => true };
    await load();
    expect(document.querySelector(".item.card")).toBeNull();
    answer({ ok: true, value: { cards: [{ id: VISA, title: "Visa", brand: "visa", last4: "1111", expiry: "04/33", expired: false }], origin: "https://shop.com" } });
    await vi.waitFor(() => expect(document.querySelector(".item.card")).not.toBeNull());
    const order = Array.from(document.querySelectorAll(".item.identity, .item.card, .notice.offer")).map((e) => e.className);
    expect(order).toEqual(["item identity", "item card", "notice offer"]);
  });
});

describe("popup tags", () => {
  const match = (tags: string[]) => ({ id: "11111111-1111-4111-8111-111111111111", title: "Acme", username: "admin@acme.example.com", hasTotp: false, provider: null, tags });
  const open = async (tags: string[]) => {
    replies.popup_state = async () => ({ ok: true, value: { kind: "unlocked", site: "acme.example.com", matches: [match(tags)], identity: null } });
    vi.resetModules();
    await import("./popup");
    await vi.waitFor(() => expect(document.querySelector(".item .who")).not.toBeNull());
  };

  it("shows the username first and the tags after it", async () => {
    await open(["staging"]);
    const user = document.querySelector(".item .who .user")!;
    expect(user.querySelector(".user-name")!.textContent).toBe("admin@acme.example.com");
    expect(user.querySelector(".tags")!.textContent).toBe("\u00b7 staging");
    expect(user.firstElementChild!.className).toContain("user-name");
  });

  it("renders no tags element without tags", async () => {
    await open([]);
    expect(document.querySelector(".item .who .user")!.textContent).toBe("admin@acme.example.com");
    expect(document.querySelector(".tags")).toBeNull();
  });

  it("shows the no-username copy with the tags", async () => {
    replies.popup_state = async () => ({
      ok: true,
      value: { kind: "unlocked", site: "acme.example.com", matches: [{ ...match(["staging"]), username: null }], identity: null },
    });
    vi.resetModules();
    await import("./popup");
    await vi.waitFor(() => expect(document.querySelector(".item .who")).not.toBeNull());
    expect(document.querySelector(".user-name")!.textContent).toBe("No username");
    expect(document.querySelector(".tags")!.textContent).toBe("\u00b7 staging");
  });
});

describe("popup frozen", () => {
  it("shows the notice, Subscribe, and Code for a login with TOTP but no Fill", async () => {
    replies = {
      popup_state: async () => ({ ok: true, value: { kind: "frozen", site: "github.com", matches: [{ id: "11111111-1111-4111-8111-111111111111", title: "GitHub", username: "octo", hasTotp: true, strength: "same_host", provider: null, tags: [] }] } }),
      popup_open_pricing: async () => ({ ok: true, value: null }),
    };
    vi.resetModules();
    await import("./popup");
    await vi.waitFor(() => expect(document.querySelector(".notice.frozen")).not.toBeNull());
    const texts = [...document.querySelectorAll("button")].map((b) => b.textContent);
    expect(texts).toContain(en.popup.frozen.subscribe);
    expect(texts).toContain(en.popup.frozen.code);
    expect(texts).not.toContain(en.popup.fill);
    expect(document.querySelector(".item.identity")).toBeNull();
    [...document.querySelectorAll("button")].find((b) => b.textContent === en.popup.frozen.subscribe)!.click();
    await vi.waitFor(() => expect(asked).toContainEqual({ type: "popup_open_pricing" }));
  });
});
