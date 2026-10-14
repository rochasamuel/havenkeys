// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateConfirmation } from "../lib/types";

let ask: ((c: UpdateConfirmation) => void) | null = null;
let closed: ((id: string) => void) | null = null;
let locked: ((reason: string) => void) | null = null;
const answer = vi.fn<(id: string, allow: boolean) => Promise<void>>(() => Promise.resolve());
// Only the confirmation's own calls exist: any secret fetch would throw.
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    onConfirmUpdate: (h: (c: UpdateConfirmation) => void) => ((ask = h), Promise.resolve(() => void (ask = null))),
    onConfirmUpdateClosed: (h: (id: string) => void) => ((closed = h), Promise.resolve(() => void (closed = null))),
    onLocked: (h: (reason: string) => void) => ((locked = h), Promise.resolve(() => void (locked = null))),
    answerUpdateConfirmation: (id: string, allow: boolean) => answer(id, allow),
  },
}));

// Each render of the dialog first notes what the last commit put on screen:
// whether Allow was enabled and which login the body named.
const seen: Array<{ allowEnabled: boolean; body: string }> = [];
vi.mock("../i18n/context", async (importOriginal) => {
  const real = await importOriginal<typeof import("../i18n/context")>();
  return {
    ...real,
    useI18n: () => {
      const allow = Array.from(document.querySelectorAll("button")).find((b) => b.textContent === "Allow");
      if (allow) {
        seen.push({
          allowEnabled: !allow.disabled,
          body: document.getElementById("confirm-update-body")?.textContent ?? "",
        });
      }
      return real.useI18n();
    },
  };
});

import { UpdateConfirmDialog } from "./UpdateConfirmDialog";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const PENDING: UpdateConfirmation = {
  id: "c0ffee00-0000-4000-8000-000000000001",
  title: "GitHub",
  username: "octo",
  site: "github.com",
  timeoutMs: 60_000,
};

let appRoot: HTMLElement;
let host: HTMLElement;
let root: Root;

beforeEach(async () => {
  vi.useFakeTimers();
  answer.mockReset();
  answer.mockImplementation(() => Promise.resolve());
  appRoot = document.createElement("div");
  appRoot.id = "root";
  host = document.createElement("div");
  appRoot.append(host);
  document.body.append(appRoot);
  root = createRoot(host);
  await act(async () => root.render(<UpdateConfirmDialog />));
});

afterEach(async () => {
  await act(async () => root.unmount());
  document.body.replaceChildren();
  vi.useRealTimers();
});

const dialog = () => document.querySelector<HTMLElement>("[role=alertdialog]");
const button = (name: string) =>
  Array.from(document.querySelectorAll("button")).find((b) => b.textContent === name) as HTMLButtonElement;

async function open(c: UpdateConfirmation = PENDING) {
  await act(async () => ask?.(c));
}

/** Past the delay that keeps Allow off right after the dialog opens. */
async function armed() {
  await act(async () => vi.advanceTimersByTime(500));
}

describe("UpdateConfirmDialog", () => {
  it("shows nothing until the browser asks", () => {
    expect(dialog()).toBeNull();
  });

  it("names the login and the site, never a secret, with Deny focused", async () => {
    await open();
    expect(dialog()?.getAttribute("aria-modal")).toBe("true");
    expect(document.getElementById("confirm-update-title")?.textContent).toBe("Change this password?");
    expect(document.getElementById("confirm-update-body")?.textContent).toBe(
      "The browser wants to change the password of “GitHub” (octo) on github.com. It was already changed in the last 24 hours.",
    );
    expect(dialog()?.querySelector("input")).toBeNull();
    expect(document.activeElement).toBe(button("Deny"));
    expect(appRoot.hasAttribute("inert")).toBe(true);
    // Portalled outside the inert app root, so it stays usable.
    expect(appRoot.contains(dialog())).toBe(false);
  });

  it("leaves out the username when the login has none", async () => {
    await open({ ...PENDING, username: null });
    expect(document.getElementById("confirm-update-body")?.textContent).toBe(
      "The browser wants to change the password of “GitHub” on github.com. It was already changed in the last 24 hours.",
    );
  });

  it("Deny is the primary, first answer; Allow is a plain button", async () => {
    await open();
    const [first, second] = Array.from(dialog()!.querySelectorAll("button"));
    expect(first).toBe(button("Deny"));
    expect(first?.className).toBe("btn btn-primary");
    expect(second).toBe(button("Allow"));
    expect(second?.className).toBe("btn");
  });

  it("keeps Allow off for a moment after opening, against a click already on its way", async () => {
    await open();
    expect(button("Allow").disabled).toBe(true);
    await act(async () => button("Allow").click());
    expect(answer).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTime(499));
    expect(button("Allow").disabled).toBe(true);
    await act(async () => vi.advanceTimersByTime(1));
    expect(button("Allow").disabled).toBe(false);
  });

  it("Allow answers yes for this confirmation and closes", async () => {
    await open();
    await armed();
    await act(async () => button("Allow").click());
    expect(answer).toHaveBeenCalledWith(PENDING.id, true);
    expect(dialog()).toBeNull();
    expect(appRoot.hasAttribute("inert")).toBe(false);
  });

  it("Deny and Escape answer no", async () => {
    await open();
    await act(async () => button("Deny").click());
    expect(answer).toHaveBeenLastCalledWith(PENDING.id, false);
    await open();
    await act(async () => {
      dialog()?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(answer).toHaveBeenLastCalledWith(PENDING.id, false);
    expect(answer).toHaveBeenCalledTimes(2);
    expect(dialog()).toBeNull();
  });

  it("Escape answers no even when the focus left the dialog", async () => {
    await open();
    (document.activeElement as HTMLElement | null)?.blur();
    await act(async () => {
      document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
    expect(answer).toHaveBeenCalledExactlyOnceWith(PENDING.id, false);
    expect(dialog()).toBeNull();
  });

  it("closes quietly when Rust refuses the answer", async () => {
    answer.mockImplementation(() => Promise.reject(new Error("not_pending")));
    await open();
    await armed();
    await act(async () => button("Allow").click());
    expect(dialog()).toBeNull();
    expect(appRoot.hasAttribute("inert")).toBe(false);
  });

  it("closes without answering on Rust's closed event for it, and on lock", async () => {
    await open();
    await act(async () => closed?.("someone-else"));
    expect(dialog()).not.toBeNull();
    await act(async () => closed?.(PENDING.id));
    expect(dialog()).toBeNull();
    expect(appRoot.hasAttribute("inert")).toBe(false);
    await open();
    await act(async () => locked?.("auto"));
    expect(dialog()).toBeNull();
    expect(answer).not.toHaveBeenCalled();
  });

  it("a newer confirmation is never on screen armed by the one it replaced", async () => {
    const B: UpdateConfirmation = { ...PENDING, id: "c0ffee00-0000-4000-8000-000000000002", title: "GitLab" };
    await open();
    await armed();
    expect(button("Allow").disabled).toBe(false);
    seen.length = 0;
    await open(B);
    await act(async () => button("Allow").click());
    expect(answer).not.toHaveBeenCalled();
    // No commit ever showed B's text with Allow enabled before its own delay.
    expect(seen.filter((s) => s.body.includes("“GitLab”") && s.allowEnabled)).toEqual([]);
    expect(button("Allow").disabled).toBe(true);
    await armed();
    expect(button("Allow").disabled).toBe(false);
  });

  it("returns the focus where it was on close", async () => {
    const before = document.createElement("button");
    host.append(before);
    before.focus();
    await open();
    expect(document.activeElement).toBe(button("Deny"));
    await act(async () => button("Deny").click());
    expect(document.activeElement).toBe(before);
  });

  it("a newer confirmation replaces the old; the old one's close leaves it open", async () => {
    const B: UpdateConfirmation = { ...PENDING, id: "c0ffee00-0000-4000-8000-000000000002", title: "GitLab" };
    await open();
    await open(B);
    await act(async () => closed?.(PENDING.id));
    expect(dialog()).not.toBeNull();
    expect(appRoot.hasAttribute("inert")).toBe(true);
    expect(document.getElementById("confirm-update-body")?.textContent).toContain("“GitLab”");
    await act(async () => button("Deny").click());
    expect(answer).toHaveBeenCalledExactlyOnceWith(B.id, false);
  });
});
