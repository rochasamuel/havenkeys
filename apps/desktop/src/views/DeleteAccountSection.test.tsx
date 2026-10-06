// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const deleteAccount = vi.fn<(confirmation: string, master: string) => Promise<void>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    accountStatus: () => Promise.resolve({ email: "user@example.com" }),
    deleteAccount: (c: string, m: string) => deleteAccount(c, m),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { en } from "../i18n/en";
import { DeleteAccountSection } from "./DeleteAccountSection";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  deleteAccount.mockReset().mockResolvedValue(undefined);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function type(selector: string, value: string) {
  const input = host.querySelector<HTMLInputElement>(selector)!;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function button(label: string): HTMLButtonElement {
  return [...host.querySelectorAll("button")].find((b) => b.textContent === label)!;
}

async function openConfirm() {
  await act(async () => root.render(<DeleteAccountSection online />));
  await act(async () => button(en.deleteAccount.start).click());
  await act(async () => button(en.deleteAccount.continueWithout).click());
}

describe("DeleteAccountSection", () => {
  it("needs the account's email and the master password before deleting", async () => {
    await openConfirm();
    expect(button(en.deleteAccount.confirm).disabled).toBe(true);
    await act(async () => type('input[name="confirm-email"]', " User@Example.com "));
    expect(button(en.deleteAccount.confirm).disabled).toBe(true);
    await act(async () => type('input[name="master-password"]', "pw"));
    expect(button(en.deleteAccount.confirm).disabled).toBe(false);
    await act(async () => button(en.deleteAccount.confirm).click());
    expect(deleteAccount).toHaveBeenCalledWith(" User@Example.com ", "pw");
  });

  it("keeps the button disabled for another email", async () => {
    await openConfirm();
    await act(async () => type('input[name="confirm-email"]', "other@example.com"));
    await act(async () => type('input[name="master-password"]', "pw"));
    expect(button(en.deleteAccount.confirm).disabled).toBe(true);
  });

  it("clears the password and shows why after a failed attempt", async () => {
    deleteAccount.mockRejectedValue({ code: "unlock_failed", message: "x" });
    await openConfirm();
    await act(async () => type('input[name="confirm-email"]', "user@example.com"));
    await act(async () => type('input[name="master-password"]', "wrong"));
    await act(async () => button(en.deleteAccount.confirm).click());
    expect(host.querySelector<HTMLInputElement>('input[name="master-password"]')!.value).toBe("");
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
  });

  it("cannot start while offline", async () => {
    await act(async () => root.render(<DeleteAccountSection online={false} />));
    expect(button(en.deleteAccount.start).disabled).toBe(true);
  });
});
