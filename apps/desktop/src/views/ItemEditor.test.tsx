// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const createItem = vi.fn();
const updateItem = vi.fn();

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    createItem: (...a: unknown[]) => createItem(...a),
    updateItem: (...a: unknown[]) => updateItem(...a),
    revealIdentity: () => Promise.resolve({ fields: { firstName: "Sam", custom: [] } }),
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
import type { ItemOverview } from "../lib/types";
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
