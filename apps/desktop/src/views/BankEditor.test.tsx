// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const createItem = vi.fn();
const updateItem = vi.fn();
const revealBank = vi.fn();

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    createItem: (...a: unknown[]) => createItem(...a),
    updateItem: (...a: unknown[]) => updateItem(...a),
    revealBank: (...a: unknown[]) => revealBank(...a),
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
import type { BankView, ItemOverview } from "../lib/types";
import { BankEditor } from "./BankEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const f = en.bank.fields;
const saved: ItemOverview = {
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
const onSaved = vi.fn();

beforeEach(() => {
  createItem.mockReset().mockImplementation((input: unknown) => Promise.resolve({ id: "new", ...(input as object) }));
  updateItem.mockReset().mockImplementation((id: string, input: unknown) => Promise.resolve({ id, ...(input as object) }));
  revealBank.mockReset().mockResolvedValue(view);
  onSaved.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const box = (label: string) => host.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`)!;
function type(input: HTMLInputElement, text: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  act(() => {
    setter.call(input, text);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}
function paste(input: HTMLInputElement, text: string) {
  const e = new Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(e, "clipboardData", { value: { getData: () => text } });
  act(() => {
    input.dispatchEvent(e);
  });
}
async function submit() {
  await act(async () => {
    host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
}
const render = (existing?: ItemOverview) =>
  act(async () => root.render(<BankEditor existing={existing} onCancel={() => undefined} onSaved={onSaved} />));

describe("BankEditor", () => {
  it("splits a pasted 12345-6 into the account and its digit", async () => {
    await render();
    paste(box(f.account), "12345-6");
    expect(box(f.account).value).toBe("12345");
    expect(box(en.bank.digitOf(f.account)).value).toBe("6");
    paste(box(f.branch), "0001-x");
    expect(box(f.branch).value).toBe("0001");
    expect(box(en.bank.digitOf(f.branch)).value).toBe("X");
  });

  it("leaves a paste that does not fit the boxes unsplit", async () => {
    await render();
    paste(box(f.branch), "1234-56");
    expect(box(f.branch).value).toBe("");
    expect(box(en.bank.digitOf(f.branch)).value).toBe("");
    paste(box(f.account), "123456789012345678901-6");
    expect(box(f.account).value).toBe("");
  });

  it("lists the fields in the detail's order", async () => {
    await render();
    const labels = [...host.querySelectorAll(".edit-label")].map((l) => l.textContent);
    const fields = [f.bank, f.branch, f.account, f.accountType, f.holderName, f.password];
    expect(labels.filter((l) => fields.includes(l!))).toEqual(fields);
  });

  it("labels the number box with its row label", async () => {
    await render();
    const label = [...host.querySelectorAll<HTMLLabelElement>("label.edit-label")].find((l) => l.textContent === f.account)!;
    expect(label.control).toBe(box(f.account));
  });

  it("saves a new account as a bank_account with its bank values", async () => {
    await render();
    type(box(f.bank), "Itaú");
    type(box(f.holderName), "Maria");
    type(box(f.branch), "1234");
    type(box(en.bank.digitOf(f.branch)), "5");
    type(box(f.account), "12345");
    type(box(en.bank.digitOf(f.account)), "6");
    await submit();
    expect(createItem).toHaveBeenCalledTimes(1);
    expect(createItem.mock.calls[0]![0]).toMatchObject({
      itemType: "bank_account",
      bank: {
        bank: "Itaú",
        holderName: "Maria",
        accountType: null,
        branch: "1234",
        branchDigit: "5",
        account: { op: "set", value: "12345" },
        accountDigit: { op: "set", value: "6" },
      },
    });
  });

  it("keeps an untouched account, digit and password on edit", async () => {
    await render(saved);
    expect(revealBank).toHaveBeenCalledWith("b1");
    expect(box(f.bank).value).toBe("Itaú");
    expect(box(f.account).value).toBe("");
    type(box(f.holderName), "Maria Silva");
    await submit();
    expect(updateItem).toHaveBeenCalledTimes(1);
    const input = updateItem.mock.calls[0]![1];
    expect(input).toMatchObject({
      itemType: "bank_account",
      password: { op: "keep" },
      notes: { op: "keep" },
      bank: { holderName: "Maria Silva", branch: "1234", branchDigit: "5", account: { op: "keep" }, accountDigit: { op: "keep" } },
    });
  });

  it("a new account number clears a digit left empty", async () => {
    await render(saved);
    type(box(f.account), "999999");
    await submit();
    expect(updateItem.mock.calls[0]![1].bank).toMatchObject({ account: { op: "set", value: "999999" }, accountDigit: { op: "clear" } });
  });
});
