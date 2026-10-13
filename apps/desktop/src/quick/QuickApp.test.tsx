// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";

const status = vi.fn<() => Promise<VaultStatus>>();
const unlock = vi.fn<(password: string, secretKey?: string) => Promise<VaultStatus>>();
const hideQuickWindow = vi.fn<() => Promise<void>>();
const listItems = vi.fn();
let shown: (() => void) | null = null;
let locked: ((reason: string) => void) | null = null;

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    status: () => status(),
    unlock: (p: string, k?: string) => unlock(p, k),
    hideQuickWindow: () => hideQuickWindow(),
    listItems: (q?: string) => listItems(q),
    onQuickShown: (cb: () => void) => {
      shown = cb;
      return Promise.resolve(() => undefined);
    },
    onLocked: (cb: (r: string) => void) => {
      locked = cb;
      return Promise.resolve(() => undefined);
    },
  };
  return {
    ...original,
    api: new Proxy(known, {
      get: (target, name: string) =>
        name in target
          ? target[name]
          : name.startsWith("on")
            ? () => Promise.resolve(() => undefined)
            : () => Promise.resolve(null),
    }),
  };
});

import { en } from "../i18n/en";
import { QuickApp } from "./QuickApp";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const LOCKED: VaultStatus = { state: "locked", vaultExists: true, damagedItems: 0, damagedSettings: false, unreadableItems: 0 } as VaultStatus;
const UNLOCKED: VaultStatus = { ...LOCKED, state: "unlocked" } as VaultStatus;

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  status.mockReset().mockResolvedValue(LOCKED);
  unlock.mockReset().mockResolvedValue(UNLOCKED);
  hideQuickWindow.mockReset().mockResolvedValue(undefined);
  listItems.mockReset().mockResolvedValue([]);
  shown = null;
  locked = null;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function render() {
  await act(async () => root.render(<QuickApp />));
}
function type(selector: string, value: string) {
  const input = host.querySelector<HTMLInputElement>(selector)!;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}
async function submit() {
  await act(async () => {
    host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
}
async function key(target: Element, k: string, init: KeyboardEventInit = {}) {
  await act(async () => {
    target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...init }));
  });
}
const password = () => host.querySelector<HTMLInputElement>("input[type=password]");
const search = () => host.querySelector<HTMLInputElement>("input[role=combobox]");

describe("QuickApp", () => {
  it("asks for the master password while locked", async () => {
    await render();
    expect(password()).not.toBeNull();
    expect(search()).toBeNull();
    expect(host.textContent).toContain(en.quick.unlockLede);
  });

  it("unlocks with unlock_vault and then searches", async () => {
    await render();
    await act(async () => type("input[type=password]", "correct horse"));
    await submit();
    expect(unlock).toHaveBeenCalledWith("correct horse", undefined);
    expect(search()).not.toBeNull();
    expect(password()).toBeNull();
  });

  it("shows the generic error for a wrong password and empties the field", async () => {
    unlock.mockRejectedValue(new ApiError("unlock_failed", "Incorrect master password or damaged vault."));
    await render();
    await act(async () => type("input[type=password]", "wrong"));
    await submit();
    expect(host.textContent).toContain(en.errors.codes.unlock_failed);
    expect(password()!.value).toBe("");
  });

  it("sends the user to the main window when the Secret Key is needed", async () => {
    unlock.mockRejectedValue(new ApiError("secret_key_required", "This vault needs your Secret Key."));
    await render();
    await act(async () => type("input[type=password]", "pw"));
    await submit();
    expect(host.textContent).toContain(en.quick.secretKeyNeeded);
  });

  it("hides on Esc while locked", async () => {
    await render();
    await key(password()!, "Escape");
    expect(hideQuickWindow).toHaveBeenCalledTimes(1);
  });

  it("a lock clears the typed password", async () => {
    await render();
    await act(async () => type("input[type=password]", "half typed"));
    await act(async () => locked?.("idle"));
    expect(password()!.value).toBe("");
  });

  it("goes back to the password field when the vault locks", async () => {
    status.mockResolvedValue(UNLOCKED);
    await render();
    expect(search()).not.toBeNull();
    await act(async () => locked?.("idle"));
    expect(password()).not.toBeNull();
  });

  it("re-reads the lock state each time it is shown", async () => {
    await render();
    expect(password()).not.toBeNull();
    status.mockResolvedValue(UNLOCKED); // unlocked in the main window meanwhile
    await act(async () => shown?.());
    expect(search()).not.toBeNull();
  });
});
