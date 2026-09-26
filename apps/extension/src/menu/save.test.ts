// @vitest-environment jsdom
//
// The save prompt against a fake background: it reports the height its
// content needs (long errors wrap to several lines) so the frame can grow.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
  const actions = document.createElement("div");
  for (const id of ["dismiss", "confirm"]) {
    const b = document.createElement("button");
    b.id = id;
    actions.append(b);
  }
  prompt.append(text, actions);
  card.append(header, prompt);
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

describe("save prompt size", () => {
  it("reports the height its content needs, so a long error is not clipped", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const height = this.tagName === "HEADER" ? 34 : this.classList.contains("prompt") ? 150 : 0;
      return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: height, width: 0, height, toJSON: () => ({}) } as DOMRect;
    });
    replies = [{ ok: false, message: "A long error that wraps over several lines in the prompt." }];
    vi.resetModules();
    await import("./save");
    await vi.advanceTimersByTimeAsync(50);
    expect(asked).toContainEqual({ type: "save_resize", token: TOKEN, height: 34 + 150 });
  });
});
