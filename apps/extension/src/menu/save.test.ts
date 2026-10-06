// @vitest-environment jsdom
//
// The save prompt against a fake background: it reports the height its
// content needs (long errors wrap to several lines) so the frame can grow.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { postToken } from "./token.test-helper";

const TOKEN = "b".repeat(32);
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
  const prompt = document.createElement("div");
  prompt.className = "prompt";
  const text = document.createElement("div");
  for (const id of ["question", "detail"]) {
    const p = document.createElement("p");
    p.id = id;
    text.append(p);
  }
  const field = document.createElement("label");
  field.id = "title-field";
  field.hidden = true;
  const label = document.createElement("span");
  label.id = "title-label";
  const input = document.createElement("input");
  input.id = "title";
  field.append(label, input);
  text.append(field);
  const actions = document.createElement("div");
  for (const id of ["dismiss", "confirm"]) {
    const b = document.createElement("button");
    b.id = id;
    actions.append(b);
  }
  prompt.append(text, actions);
  card.append(header, prompt);
  document.body.append(card);
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

async function openPrompt(view: object): Promise<void> {
  replies = [{ ok: true, value: view }];
  vi.resetModules();
  await import("./save");
  postToken(TOKEN);
  await vi.advanceTimersByTimeAsync(1000); // past the click guard
}

const titleField = () => document.getElementById("title-field") as HTMLElement;
const titleInput = () => document.getElementById("title") as HTMLInputElement;
const confirms = () => asked.filter((m) => (m as { type: string }).type === "save_confirm");

describe("save prompt name", () => {
  it("a new login: the suggested name, editable, not focused", async () => {
    await openPrompt({ action: "add", site: "github.com", username: "octo", title: "GitHub" });
    expect(titleField().hidden).toBe(false);
    expect(titleInput().value).toBe("GitHub");
    expect(titleInput().maxLength).toBe(256);
    // Opening the prompt must not take the focus from the page.
    expect(document.activeElement).not.toBe(titleInput());
  });

  it("an update keeps the login's title: no name field", async () => {
    await openPrompt({ action: "update", site: "github.com", username: "octo", title: null });
    expect(titleField().hidden).toBe(true);
  });

  it("the confirm carries the name only for a new login", async () => {
    const { confirmRequest } = await import("./save");
    postToken(TOKEN);
    expect(confirmRequest(TOKEN, "GitHub – work")).toEqual({ type: "save_confirm", token: TOKEN, title: "GitHub – work" });
    expect(confirmRequest(TOKEN, null)).toEqual({ type: "save_confirm", token: TOKEN });
  });

  it("synthetic clicks and Enter presses save nothing", async () => {
    await openPrompt({ action: "add", site: "github.com", username: "octo", title: "GitHub" });
    titleInput().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    (document.getElementById("confirm") as HTMLButtonElement).click();
    await vi.advanceTimersByTimeAsync(0);
    expect(confirms()).toEqual([]);
  });
});

describe("save prompt size", () => {
  it("reports the height its content needs, so a long error is not clipped", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const height = this.tagName === "HEADER" ? 34 : this.classList.contains("prompt") ? 150 : 0;
      return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: height, width: 0, height, toJSON: () => ({}) } as DOMRect;
    });
    replies = [{ ok: false, message: "A long error that wraps over several lines in the prompt." }];
    vi.resetModules();
    await import("./save");
    postToken(TOKEN);
    await vi.advanceTimersByTimeAsync(50);
    expect(asked).toContainEqual({ type: "save_resize", token: TOKEN, height: 34 + 150 });
  });
});

describe("card save prompt", () => {
  it("shows the card's logo, last four and expiry with an editable name", async () => {
    await openPrompt({ site: "shop.com", title: "Visa", card: { brand: "visa", last4: "5556", expiry: "01/30" } });
    expect(document.getElementById("question")!.textContent).toBe("Save this card to HavenKeys?");
    expect(document.getElementById("detail")!.textContent).toContain("•••• 5556 · 01/30");
    expect(document.querySelector("#detail svg")).not.toBeNull();
    expect(titleField().hidden).toBe(false);
    expect(titleInput().value).toBe("Visa");
  });
});
