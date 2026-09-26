// @vitest-environment jsdom
//
// The passkey card against a fake background: replies it cannot rely on
// (a restarting worker answers nothing at all) and the "already saved" state.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const TOKEN = "a".repeat(32);
const ITEM = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";
let replies: unknown[] = [];
const asked: unknown[] = [];
const resized: unknown[] = [];

function page(): void {
  document.body.replaceChildren();
  const card = document.createElement("div");
  card.className = "card";
  for (const [tag, id] of [
    ["span", "site"],
    ["p", "question"],
    ["p", "detail"],
    ["main", "main"],
    ["button", "cancel"],
    ["button", "fallback"],
    ["button", "confirm"],
  ] as const) {
    const el = document.createElement(tag);
    el.id = id;
    card.append(el);
  }
  document.body.append(card);
  (document.getElementById("confirm") as HTMLButtonElement).hidden = true;
  location.hash = TOKEN;
}

async function load(): Promise<void> {
  vi.resetModules();
  await import("./passkey");
  await vi.advanceTimersByTimeAsync(0);
}

const text = (id: string) => document.getElementById(id)?.textContent;

beforeEach(() => {
  vi.useFakeTimers();
  replies = [];
  asked.length = 0;
  resized.length = 0;
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      sendMessage: async (m: unknown) => {
        // Size reports are fire-and-forget; keep them out of the request log.
        if ((m as { type?: string }).type === "pk_resize") {
          resized.push(m);
          return { ok: true, value: null };
        }
        asked.push(m);
        return replies.length > 0 ? replies.shift() : undefined;
      },
    },
  };
  page();
});

afterEach(() => vi.useRealTimers());

describe("passkey card", () => {
  it("does not throw when the background answers nothing", async () => {
    await load();
    expect(asked).toEqual([{ type: "pk_state", token: TOKEN }]);
    expect(text("question")).toBe("HavenKeys could not be reached.");
  });

  it("keeps polling a locked card through empty replies", async () => {
    replies = [{ ok: true, value: { state: "locked", site: "github.com" } }, undefined];
    await load();
    expect(text("question")).toBe("HavenKeys is locked");
    await vi.advanceTimersByTimeAsync(1500);
    expect(asked).toHaveLength(2);
    expect(text("question")).toBe("HavenKeys is locked");
    replies = [{ ok: true, value: { state: "chooser", site: "github.com", passkeys: [{ itemId: ITEM, credentialId: CRED, title: "GitHub", userName: "octo" }] } }];
    await vi.advanceTimersByTimeAsync(1500);
    expect(asked).toHaveLength(3);
    expect(text("question")).toBe("Sign in with a passkey");
  });

  it("shows the already-saved card with a Close button and no Cancel", async () => {
    replies = [{ ok: true, value: { state: "exists", site: "github.com" } }];
    await load();
    expect(text("question")).toBe("This account already has a passkey in HavenKeys");
    const confirm = document.getElementById("confirm") as HTMLButtonElement;
    expect(confirm.hidden).toBe(false);
    expect(confirm.textContent).toBe("Close");
    expect((document.getElementById("cancel") as HTMLButtonElement).hidden).toBe(true);
  });

  it("titles the upgrade card and preselects the filled login", async () => {
    const OTHER = "11111111-2222-4333-8444-555555555555";
    replies = [{ ok: true, value: { state: "create", site: "github.com", userName: "octo", upgradeItemId: OTHER, candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }, { itemId: OTHER, title: "GitHub work", username: "work" }] } }];
    await load();
    expect(text("question")).toBe("Add a passkey?");
    const checked = [...document.querySelectorAll<HTMLInputElement>("input[type=radio]")].map((r) => r.checked);
    expect(checked).toEqual([false, true, false]);
  });

  it("keeps the ordinary save card for ordinary creates", async () => {
    replies = [{ ok: true, value: { state: "create", site: "github.com", userName: "octo", upgradeItemId: null, candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }] } }];
    await load();
    expect(text("question")).toBe("Save a passkey to HavenKeys?");
    const checked = [...document.querySelectorAll<HTMLInputElement>("input[type=radio]")].map((r) => r.checked);
    expect(checked).toEqual([true, false]);
  });

  it("shows the saved notice without buttons", async () => {
    replies = [{ ok: true, value: { state: "saved", site: "github.com" } }];
    await load();
    expect(text("question")).toBe("Passkey saved to HavenKeys");
    expect(text("detail")).toBe("Manage it in the HavenKeys app");
    expect(text("site")).toBe("github.com");
    for (const id of ["cancel", "fallback", "confirm"]) expect((document.getElementById(id) as HTMLButtonElement).hidden).toBe(true);
  });
});
