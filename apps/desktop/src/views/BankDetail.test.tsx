// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const revealBank = vi.fn();
const revealBankField = vi.fn();
const copyBankField = vi.fn();

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    revealBank: (...a: unknown[]) => revealBank(...a),
    revealBankField: (...a: unknown[]) => revealBankField(...a),
    copyBankField: (...a: unknown[]) => copyBankField(...a),
    itemFields: () => Promise.resolve([]),
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
import { ToastProvider } from "../components/Toast";
import type { BankView, ItemOverview } from "../lib/types";
import { BankDetail } from "./BankDetail";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const f = en.bank.fields;
const item: ItemOverview = {
  id: "b1", itemType: "bank_account", title: "Itaú", username: null, urls: [], hasPassword: true, hasTotp: false,
  hasNotes: false, hasPasskey: false, autoSignIn: false, tags: [], createdAt: 1, updatedAt: 1,
  bank: { bank: "Itaú", branch: "1234", accountLast3: "45-6" },
};
const view: BankView = {
  bank: "Itaú", holderName: "Maria", accountType: "checking", branch: "1234", branchDigit: "5", notes: null,
  sections: [], hasAccount: true, hasPassword: true,
};

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  revealBank.mockReset().mockResolvedValue(view);
  revealBankField.mockReset().mockImplementation((_id: string, field: string) =>
    Promise.resolve(field === "account" ? "12345" : field === "accountDigit" ? "6" : "s3cret"),
  );
  copyBankField.mockReset().mockResolvedValue({ clearAfterSeconds: 30 });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.useRealTimers();
});

const render = () =>
  act(async () =>
    root.render(
      <ToastProvider>
        <BankDetail item={item} readOnly={false} onEdit={() => undefined} onDelete={() => undefined} />
      </ToastProvider>,
    ),
  );
const button = (label: string) => host.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
const click = (label: string) => act(async () => button(label).click());

describe("BankDetail", () => {
  it("shows the account masked, and the eye reveals it with its digit", async () => {
    await render();
    expect(host.textContent).toContain("•••45-6");
    expect(host.textContent).toContain("1234-5");
    expect(host.textContent).not.toContain("12345");
    await click(en.bank.show(f.account));
    expect(revealBankField).toHaveBeenCalledWith("b1", "account");
    expect(revealBankField).toHaveBeenCalledWith("b1", "accountDigit");
    expect(host.textContent).toContain("12345-6");
  });

  it("hides a revealed value after 30 s", async () => {
    vi.useFakeTimers();
    await render();
    await click(en.bank.show(f.password));
    expect(revealBankField).toHaveBeenCalledWith("b1", "password");
    expect(host.textContent).toContain("s3cret");
    await act(async () => vi.advanceTimersByTime(30_000));
    expect(host.textContent).not.toContain("s3cret");
  });

  it("copies each value through Rust, in the UI language", async () => {
    await render();
    for (const label of [f.bank, f.branch, f.account, f.holderName, f.password]) await click(en.bank.copy(label));
    expect(copyBankField.mock.calls).toEqual([
      ["b1", "bank", "en"],
      ["b1", "branch", "en"],
      ["b1", "account", "en"],
      ["b1", "holderName", "en"],
      ["b1", "password", "en"],
    ]);
    expect(button(en.bank.copy(f.accountType))).toBeNull();
  });

  it("copies the transfer details from the header", async () => {
    await render();
    const copyDetails = [...host.querySelectorAll<HTMLButtonElement>(".item-head-actions button")].find((b) => b.textContent?.includes(en.bank.copyDetails))!;
    await act(async () => copyDetails.click());
    expect(copyBankField).toHaveBeenCalledWith("b1", "details", "en");
    expect(document.body.textContent).toContain(en.bank.copied(en.bank.details, 30));
  });

  it("lists bank, branch, account, type, then holder, and names the account's ending", async () => {
    await render();
    const labels = [...host.querySelectorAll(".row-label")].map((l) => l.textContent);
    expect(labels).toEqual([f.bank, f.branch, f.account, f.accountType, f.holderName, f.password]);
    expect(host.querySelector(`[aria-label="${en.bank.accountEnding("45-6")}"]`)?.textContent).toBe("•••45-6");
  });
});
