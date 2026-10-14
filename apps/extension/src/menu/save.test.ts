// @vitest-environment jsdom
//
// The save prompt against a fake background: it reports the height its
// content needs (long errors wrap to several lines) so the frame can grow.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { captureClicks } from "./click-capture.test-helper";
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
  const warning = document.createElement("p");
  warning.id = "warning";
  warning.hidden = true;
  text.prepend(warning);
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
    b.className = id === "confirm" ? "btn primary" : "btn";
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

describe("look-alike warning", () => {
  const view = { action: "add", site: "xn--gthub-n2e.com", username: "octo", title: "xn--gthub-n2e.com", lookalikeOf: "github.com" };
  const enter = { key: "Enter", isComposing: false };

  it("leads with the warning and offers Save anyway as the secondary action", async () => {
    await openPrompt(view);
    const warning = document.getElementById("warning")!;
    expect(warning.hidden).toBe(false);
    expect(warning.textContent).toContain("github.com");
    const confirm = document.getElementById("confirm") as HTMLButtonElement;
    expect(confirm.textContent).toBe("Save anyway");
    expect(confirm.disabled).toBe(false);
    expect(confirm.classList.contains("primary")).toBe(false);
    expect(document.getElementById("dismiss")!.classList.contains("primary")).toBe(true);
    // The primary ("Not now") sits last, where a reflexive click lands; focus order follows.
    expect(confirm.nextElementSibling?.id).toBe("dismiss");
  });

  it("the saved site is text, never markup", async () => {
    await openPrompt({ ...view, lookalikeOf: "<img src=x>.com" });
    const warning = document.getElementById("warning")!;
    expect(warning.querySelector("img")).toBeNull();
    expect(warning.textContent).toContain("<img src=x>.com");
  });

  it("Enter does not save on a look-alike page", async () => {
    // A synthetic KeyboardEvent is never trusted (confirm() ignores it), so
    // the Enter rule is checked directly: a reflexive Enter must not save.
    await openPrompt(view);
    const { enterSaves } = await import("./save");
    expect(enterSaves(enter)).toBe(false);
  });

  it("Enter saves as usual elsewhere", async () => {
    await openPrompt({ action: "add", site: "github.com", username: "octo", title: "github.com", lookalikeOf: null });
    const { enterSaves } = await import("./save");
    expect(enterSaves(enter)).toBe(true);
    expect(enterSaves({ key: "Enter", isComposing: true })).toBe(false);
    expect(enterSaves({ key: "a", isComposing: false })).toBe(false);
  });

  it("a view without lookalikeOf shows no warning", async () => {
    await openPrompt({ action: "add", site: "github.com", username: "octo", title: "github.com" });
    expect(document.getElementById("warning")!.hidden).toBe(true);
    expect(document.getElementById("warning")!.textContent).toBe("");
    expect(document.getElementById("confirm")!.textContent).toBe("Save");
    expect(document.getElementById("confirm")!.classList.contains("primary")).toBe(true);
    expect(document.getElementById("dismiss")!.classList.contains("primary")).toBe(false);
    expect(document.getElementById("dismiss")!.nextElementSibling?.id).toBe("confirm");
  });

  it("an update never shows the warning", async () => {
    await openPrompt({ action: "update", site: "github.com", username: "octo", title: null, lookalikeOf: null });
    expect(document.getElementById("warning")!.hidden).toBe(true);
    expect(document.getElementById("confirm")!.textContent).toBe("Update");
  });
});

describe("save prompt waiting for the desktop", () => {
  /** Holds the next save_confirm reply until released. */
  function holdConfirm(): (reply: unknown) => void {
    let release!: (reply: unknown) => void;
    const held = new Promise((r) => (release = r));
    const runtime = (globalThis as unknown as { chrome: { runtime: { sendMessage: (m: unknown) => Promise<unknown> } } }).chrome.runtime;
    const next = runtime.sendMessage;
    runtime.sendMessage = async (m: unknown) => {
      if ((m as { type?: string }).type !== "save_confirm") return next(m);
      asked.push(m);
      return held;
    };
    return release;
  }
  const question = () => document.getElementById("question") as HTMLElement;

  it("an update says to confirm in HavenKeys after 2 s, then shows the answer", async () => {
    const handlers = captureClicks();
    await openPrompt({ action: "update", site: "github.com", username: "octo", title: null });
    const release = holdConfirm();
    handlers.get(document.getElementById("confirm")!)!({ isTrusted: true } as MouseEvent);
    const dismiss = document.getElementById("dismiss") as HTMLButtonElement;
    await vi.advanceTimersByTimeAsync(1_900);
    expect(question().textContent).toBe("Update the saved password?");
    expect(dismiss.textContent).toBe("Not now");
    await vi.advanceTimersByTimeAsync(200);
    expect(question().textContent).toBe("Confirm in HavenKeys…");
    // Closing the card does not withdraw the desktop's question: it stays possible, and says Close.
    expect(dismiss.textContent).toBe("Close");
    expect(dismiss.disabled).toBe(false);
    release({ ok: false, message: "Not saved" });
    await vi.advanceTimersByTimeAsync(0);
    expect(question().textContent).toBe("Not saved");
    expect(question().className).toBe("error");
    expect(dismiss.textContent).toBe("Close");
  });

  it("a quick answer never shows the hint", async () => {
    const handlers = captureClicks();
    await openPrompt({ action: "update", site: "github.com", username: "octo", title: null });
    const release = holdConfirm();
    handlers.get(document.getElementById("confirm")!)!({ isTrusted: true } as MouseEvent);
    release({ ok: false, message: "Not saved — HavenKeys is waiting for another confirmation" });
    await vi.advanceTimersByTimeAsync(5_000);
    expect(question().textContent).toBe("Not saved — HavenKeys is waiting for another confirmation");
  });

  it("a new login never shows the desktop hint", async () => {
    const handlers = captureClicks();
    await openPrompt({ action: "add", site: "github.com", username: "octo", title: "GitHub" });
    holdConfirm();
    handlers.get(document.getElementById("confirm")!)!({ isTrusted: true } as MouseEvent);
    await vi.advanceTimersByTimeAsync(5_000);
    expect(question().textContent).toBe("Save this login to HavenKeys?");
  });
});
