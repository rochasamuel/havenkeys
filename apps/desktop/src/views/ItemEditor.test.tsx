// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const createItem = vi.fn();
const updateItem = vi.fn();
const itemFields = vi.fn();
const pickFiles = vi.fn();
const discardPendingFiles = vi.fn();

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    createItem: (...a: unknown[]) => createItem(...a),
    updateItem: (...a: unknown[]) => updateItem(...a),
    revealIdentity: () => Promise.resolve({ fields: { firstName: "Sam", custom: [] } }),
    itemFields: (...a: unknown[]) => itemFields(...a),
    reveal: () => Promise.resolve("root box"),
    revealCard: () => Promise.resolve({ hasNumber: false, hasVerificationNumber: false }),
    pickFiles: (...a: unknown[]) => pickFiles(...a),
    discardPendingFiles: (...a: unknown[]) => discardPendingFiles(...a),
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
import type { AttachmentMeta, ItemOverview, SectionView } from "../lib/types";
import { CardEditor } from "./CardEditor";
import { IdentityEditor } from "./IdentityEditor";
import { ItemEditor } from "./ItemEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;
const onSaved = vi.fn();

beforeEach(() => {
  createItem.mockReset().mockImplementation((input: unknown) => Promise.resolve({ id: "new", ...(input as object) }));
  updateItem.mockReset().mockImplementation((id: string, input: unknown) => Promise.resolve({ id, ...(input as object) }));
  onSaved.mockReset();
  itemFields.mockReset().mockResolvedValue([]);
  pickFiles.mockReset().mockResolvedValue([{ token: "t1", name: "notes.txt", size: 2048, mime: "text/plain" }]);
  discardPendingFiles.mockReset().mockResolvedValue(undefined);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function type(input: HTMLInputElement, text: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  act(() => {
    setter.call(input, text);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}
const tagInput = () => host.querySelector<HTMLInputElement>(".tags-input")!;
const titleInput = () => host.querySelector<HTMLInputElement>("input")!;
async function submit() {
  await act(async () => {
    host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
}

describe("ItemEditor tags on save", () => {
  // Final review (tags): Save with a refused tag in the field saved the item without it.
  it("does not save while the tag field holds a tag Rust would refuse", async () => {
    await act(async () => root.render(<ItemEditor itemType="secure_note" onCancel={() => undefined} onSaved={onSaved} />));
    type(titleInput(), "Wi-Fi");
    type(tagInput(), "x".repeat(33));
    await submit();
    expect(createItem).not.toHaveBeenCalled();
    expect(onSaved).not.toHaveBeenCalled();
    expect(host.querySelector(".tags-error")?.textContent).toBe(en.editor.tagTooLong);
    expect(tagInput().value).toBe("x".repeat(33));
  });

  it("saves a tag still being typed with the item", async () => {
    await act(async () => root.render(<ItemEditor itemType="secure_note" onCancel={() => undefined} onSaved={onSaved} />));
    type(titleInput(), "Wi-Fi");
    type(tagInput(), " Home ");
    await submit();
    expect(createItem).toHaveBeenCalledTimes(1);
    expect(createItem.mock.calls[0]![0]).toMatchObject({ title: "Wi-Fi", tags: ["Home"] });
  });

  it("holds a card's save the same way", async () => {
    await act(async () => root.render(<CardEditor onCancel={() => undefined} onSaved={onSaved} />));
    type(tagInput(), "a\u0007b");
    await submit();
    expect(createItem).not.toHaveBeenCalled();
    expect(host.querySelector(".tags-error")?.textContent).toBe(en.editor.tagNotAllowed);
    type(tagInput(), "travel");
    await submit();
    expect(createItem).toHaveBeenCalledTimes(1);
    expect(createItem.mock.calls[0]![0]).toMatchObject({ itemType: "card", tags: ["travel"] });
  });

  it("holds the identity's save the same way", async () => {
    const identity: ItemOverview = {
      id: "me", itemType: "identity", title: "Sam", username: null, urls: [], hasPassword: false, hasTotp: false,
      hasNotes: false, hasPasskey: false, autoSignIn: false, tags: [], createdAt: 1, updatedAt: 1,
    };
    await act(async () => root.render(<IdentityEditor existing={identity} onCancel={() => undefined} onSaved={onSaved} />));
    type(tagInput(), "x".repeat(40));
    await submit();
    expect(updateItem).not.toHaveBeenCalled();
    expect(host.querySelector(".tags-error")?.textContent).toBe(en.editor.tagTooLong);
    type(tagInput(), "family");
    await submit();
    expect(updateItem).toHaveBeenCalledTimes(1);
    expect(updateItem.mock.calls[0]![1]).toMatchObject({ itemType: "identity", tags: ["family"] });
  });
});

describe("ItemEditor secure note fields", () => {
  it("saves a new note with an empty field list", async () => {
    await act(async () => root.render(<ItemEditor itemType="secure_note" onCancel={() => undefined} onSaved={onSaved} />));
    type(titleInput(), "Server");
    await submit();
    expect(createItem).toHaveBeenCalledWith(expect.objectContaining({ itemType: "secure_note", sections: [] }), undefined);
  });

  it("loads an existing note's fields and sends them back on save", async () => {
    const views: SectionView[] = [
      { id: "s1", title: "Access", fields: [{ id: "f1", label: "Root", type: "password", hasValue: true }] },
    ];
    itemFields.mockResolvedValue(views);
    const existing: ItemOverview = {
      id: "n1", itemType: "secure_note", title: "Server", username: null, urls: [], hasPassword: false, hasTotp: false,
      hasNotes: false, hasPasskey: false, autoSignIn: false, tags: [], createdAt: 0, updatedAt: 0,
    };
    await act(async () =>
      root.render(<ItemEditor itemType="secure_note" existing={existing} onCancel={() => undefined} onSaved={onSaved} />),
    );
    expect(itemFields).toHaveBeenCalledWith("n1");
    await submit();
    expect(updateItem).toHaveBeenCalledWith(
      "n1",
      expect.objectContaining({
        sections: [
          {
            id: "s1",
            title: "Access",
            fields: [expect.objectContaining({ id: "f1", label: "Root", value: { type: "password", value: { op: "keep" } } })],
          },
        ],
      }),
      undefined,
    );
  });
});

describe("Editor files", () => {
  const photo: AttachmentMeta = { id: "a1", name: "receipt.png", mime: "image/png", size: 245_760, createdAt: 1 };
  const login: ItemOverview = {
    id: "l1", itemType: "login", title: "Shop", username: null, urls: [], hasPassword: false, hasTotp: false,
    hasNotes: false, hasPasskey: false, autoSignIn: false, tags: [], attachments: [photo], createdAt: 0, updatedAt: 0,
  };
  const onCancel = vi.fn();
  const button = (label: string) =>
    [...host.querySelectorAll("button")].find((b) => b.getAttribute("aria-label") === label || b.textContent?.trim() === label);
  const names = () => [...host.querySelectorAll(".attachment-name")].map((n) => n.textContent);
  const click = (label: string) => act(async () => button(label)!.click());

  async function renderLogin() {
    await act(async () =>
      root.render(<ItemEditor itemType="login" existing={login} onCancel={onCancel} onSaved={onSaved} />),
    );
  }
  async function attach() {
    await click(en.fields.addField);
    await click(en.attachments.attach);
  }

  it("attach_adds_a_pending_row", async () => {
    await renderLogin();
    expect(names()).toEqual(["receipt.png"]);
    await attach();
    expect(pickFiles).toHaveBeenCalledWith(1);
    expect(names()).toEqual(["receipt.png", "notes.txt"]);
    expect(host.textContent).toContain("2 KB");
  });

  it("remove_pending_discards_it", async () => {
    await renderLogin();
    await attach();
    await click(en.attachments.remove("notes.txt"));
    expect(discardPendingFiles).toHaveBeenCalledWith(["t1"]);
    expect(names()).toEqual(["receipt.png"]);
  });

  it("remove_existing_marks_it_until_save", async () => {
    await renderLogin();
    await click(en.attachments.remove("receipt.png"));
    expect(names()).toEqual([]);
    expect(discardPendingFiles).not.toHaveBeenCalled();
    expect(updateItem).not.toHaveBeenCalled();
    await submit();
    expect(updateItem.mock.calls[0]![2]).toEqual({ added: [], removed: ["a1"] });
  });

  it("save_passes_file_edits", async () => {
    await renderLogin();
    await attach();
    await click(en.attachments.remove("receipt.png"));
    await submit();
    expect(updateItem.mock.calls[0]![2]).toEqual({ added: ["t1"], removed: ["a1"] });
    act(() => root.unmount());
    root = createRoot(host);
    expect(discardPendingFiles).not.toHaveBeenCalled();
  });

  it("save_without_file_changes_sends_none", async () => {
    await renderLogin();
    await submit();
    expect(updateItem).toHaveBeenCalledTimes(1);
    expect(updateItem.mock.calls[0]![2]).toBeUndefined();
  });

  it("cancel_discards_pending_files", async () => {
    await renderLogin();
    await attach();
    await click(en.common.cancel);
    expect(onCancel).toHaveBeenCalled();
    expect(discardPendingFiles).toHaveBeenCalledWith(["t1"]);
  });

  it("unmount_discards_pending_files", async () => {
    await renderLogin();
    await attach();
    act(() => root.unmount());
    root = createRoot(host);
    expect(discardPendingFiles).toHaveBeenCalledWith(["t1"]);
  });

  it("failed_save_drops_pending_rows", async () => {
    updateItem.mockRejectedValue({ code: "offline", message: "offline" });
    await renderLogin();
    await attach();
    await click(en.attachments.remove("receipt.png"));
    await submit();
    expect(updateItem.mock.calls[0]![2]).toEqual({ added: ["t1"], removed: ["a1"] });
    // Rust took the picked files with the failed save: the rows go, and the user is told.
    expect(names()).toEqual([]);
    expect(host.querySelector(".form-error")?.textContent).toContain(en.attachments.notSaved);
    // The removal is still marked for the next save; nothing is left to discard.
    updateItem.mockImplementation((id: string, input: unknown) => Promise.resolve({ id, ...(input as object) }));
    await submit();
    expect(updateItem.mock.calls[1]![2]).toEqual({ added: [], removed: ["a1"] });
    act(() => root.unmount());
    root = createRoot(host);
    expect(discardPendingFiles).not.toHaveBeenCalled();
  });

  it("attaches to a new card from its own row", async () => {
    await act(async () => root.render(<CardEditor onCancel={onCancel} onSaved={onSaved} />));
    await click(en.attachments.attach);
    expect(pickFiles).toHaveBeenCalledWith(0);
    await submit();
    expect(createItem.mock.calls[0]![1]).toEqual({ added: ["t1"], removed: [] });
  });
});
