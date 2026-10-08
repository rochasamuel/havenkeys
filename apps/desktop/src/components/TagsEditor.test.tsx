// @vitest-environment jsdom
import { act, createRef, type Ref } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../i18n/en";
import { VaultTagsProvider } from "../lib/vaultTags";
import { TagsEditor, type TagsEditorHandle } from "./TagsEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// api 1, staging 3, work 1.
let vault = [
  { id: "x1", tags: ["api", "staging", "work"] },
  { id: "x2", tags: ["staging"] },
  { id: "x3", tags: ["staging"] },
];

let host: HTMLElement;
let root: Root;
const onChange = vi.fn<(tags: string[]) => void>();

beforeEach(() => {
  onChange.mockReset();
  vault = [
    { id: "x1", tags: ["api", "staging", "work"] },
    { id: "x2", tags: ["staging"] },
    { id: "x3", tags: ["staging"] },
  ];
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function show(value: string[], handle?: Ref<TagsEditorHandle>, itemId?: string) {
  act(() =>
    root.render(
      <VaultTagsProvider value={vault}>
        <TagsEditor value={value} onChange={onChange} handle={handle} itemId={itemId} />
      </VaultTagsProvider>,
    ),
  );
}
const input = () => host.querySelector<HTMLInputElement>("input")!;
function type(text: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  act(() => {
    setter.call(input(), text);
    input().dispatchEvent(new Event("input", { bubbles: true }));
  });
}
function key(k: string) {
  act(() => void input().dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true })));
}
const options = () => [...host.querySelectorAll('[role="option"]')].map((o) => o.textContent);

describe("TagsEditor", () => {
  it("adds a typed tag on Enter and on comma, normalised, case kept", () => {
    show([]);
    type("  Prod ");
    key("Enter");
    expect(onChange).toHaveBeenLastCalledWith(["Prod"]);
    show(["Prod"]);
    type("eu");
    key(",");
    expect(onChange).toHaveBeenLastCalledWith(["eu", "Prod"]);
  });

  it("opens the vault's other tags on focus, most used first", () => {
    show(["work"]);
    act(() => input().focus());
    expect(options()).toEqual(["staging3", "api1"]);
    expect(input().getAttribute("aria-expanded")).toBe("true");
  });

  it("shows no list on focus when the vault has no other tags", () => {
    show(["api", "staging", "work"]);
    act(() => input().focus());
    expect(options()).toEqual([]);
  });

  it("keeps the list open after a suggestion is clicked, for the next one", () => {
    show([]);
    act(() => input().focus());
    const first = host.querySelector<HTMLElement>('[role="option"]')!;
    act(() => void first.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true })));
    expect(onChange).toHaveBeenLastCalledWith(["staging"]);
    show(["staging"]);
    expect(options()).toEqual(["api1", "work1"]);
  });

  it("leaves the edited item out of the vault's tags, so the only item with one may change its case", () => {
    vault = [{ id: "me", tags: ["work"] }, ...vault.slice(1)];
    show([], undefined, "me"); // "work" removed from this item
    type("Work");
    expect(options()).toEqual([en.editor.createTag("Work")]);
    key("Enter");
    expect(onChange).toHaveBeenLastCalledWith(["Work"]);
  });

  it("counts the vault's tags on focus without the edited item's own", () => {
    vault = [{ id: "me", tags: ["staging"] }, ...vault];
    show([], undefined, "me");
    act(() => input().focus());
    expect(options()).toEqual(["staging3", "api1", "work1"]);
  });

  it("clears typed text and closes the list on one Escape; the next is the editor's", () => {
    show([]);
    act(() => input().focus());
    type("st");
    const outer = vi.fn();
    document.body.addEventListener("keydown", outer);
    key("Escape");
    expect(input().value).toBe("");
    expect(options()).toEqual([]);
    expect(outer).not.toHaveBeenCalled();
    key("Escape");
    expect(outer).toHaveBeenCalledTimes(1);
    document.body.removeEventListener("keydown", outer);
  });

  it("closes the open list on Escape with an empty field, and on blur", () => {
    show([]);
    act(() => input().focus());
    const escape = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
    const outer = vi.fn();
    document.body.addEventListener("keydown", outer);
    act(() => void input().dispatchEvent(escape));
    expect(options()).toEqual([]);
    expect(outer).not.toHaveBeenCalled();
    // A second Escape, with the list closed, is the editor's.
    key("Escape");
    expect(outer).toHaveBeenCalled();
    document.body.removeEventListener("keydown", outer);
    key("ArrowDown");
    expect(options().length).toBe(3);
    act(() => input().blur());
    expect(options()).toEqual([]);
  });

  it("adds the vault's spelling when the typed tag differs only in case", () => {
    show([]);
    type("WORK");
    expect(options()).toEqual(["work1"]);
    key("Enter");
    expect(onChange).toHaveBeenLastCalledWith(["work"]);
  });

  it("suggests existing tags while typing and picks one with the arrow keys", () => {
    show([]);
    type("st");
    expect(options()[0]).toBe("staging3");
    expect(input().getAttribute("aria-expanded")).toBe("true");
    key("ArrowDown");
    key("Enter");
    expect(onChange).toHaveBeenLastCalledWith(["staging"]);
  });

  it("offers to create a new tag when nothing matches exactly", () => {
    show([]);
    type("stag");
    expect(options().at(-1)).toBe(en.editor.createTag("stag"));
    type("staging");
    expect(options()).toEqual(["staging3"]);
  });

  it("removes the last tag on Backspace in an empty input, and a tag by its ×", () => {
    show(["a", "b"]);
    key("Backspace");
    expect(onChange).toHaveBeenLastCalledWith(["a"]);
    act(() => host.querySelector<HTMLButtonElement>(`button[aria-label="${en.editor.removeTag("a")}"]`)!.click());
    expect(onChange).toHaveBeenLastCalledWith(["b"]);
  });

  it("does not add a duplicate or an invalid tag", () => {
    show(["work"]);
    type("Work");
    key("Enter");
    expect(onChange).not.toHaveBeenCalled();
    type("a,b");
    expect(onChange).toHaveBeenLastCalledWith(["a", "work"]);
    expect(input().value).toBe("b");
    onChange.mockClear();
    type("x".repeat(33));
    key("Enter");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("hides the input at 20 tags and says so", () => {
    show(Array.from({ length: 20 }, (_, i) => `t${String(i).padStart(2, "0")}`));
    expect(host.querySelector("input")).toBeNull();
    expect(host.textContent).toContain(en.editor.tagLimit);
  });

  it("never shows suggestions already on the item", () => {
    show(["staging"]);
    type("sta");
    expect(options()).toEqual([en.editor.createTag("sta")]);
  });

  it("adds the typed text on Enter when suggestions show but no arrow was pressed", () => {
    show([]);
    type("st");
    key("Enter");
    expect(onChange).toHaveBeenLastCalledWith(["st"]);
  });

  it("commits typed text on blur", () => {
    show([]);
    type("later");
    act(() => void input().dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
    expect(onChange).toHaveBeenLastCalledWith(["later"]);
  });

  it("returns to no pick on ArrowUp from the first option", () => {
    show([]);
    type("st");
    key("ArrowDown");
    expect(input().getAttribute("aria-activedescendant")).not.toBeNull();
    key("ArrowUp");
    expect(input().getAttribute("aria-activedescendant")).toBeNull();
  });

  it("focuses the input when the row's empty area is clicked", () => {
    show(["a"]);
    act(() => void host.querySelector<HTMLElement>(".tags-edit")!.click());
    expect(document.activeElement).toBe(input());
  });

  it("keeps a refused tag's text and says why, until the text changes", () => {
    show([]);
    type("x".repeat(33));
    key("Enter");
    expect(onChange).not.toHaveBeenCalled();
    expect(input().value).toBe("x".repeat(33));
    expect(host.querySelector('[role="alert"]')?.textContent).toBe(en.editor.tagTooLong);
    expect(input().getAttribute("aria-invalid")).toBe("true");
    type("x".repeat(32));
    expect(host.querySelector('[role="alert"]')).toBeNull();
  });

  it("says why on blur too, and names a character it cannot take", () => {
    show([]);
    type("tab\u0007");
    act(() => void input().dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
    expect(onChange).not.toHaveBeenCalled();
    expect(host.querySelector('[role="alert"]')?.textContent).toBe(en.editor.tagNotAllowed);
  });

  it("clears a duplicate without complaint", () => {
    show(["work"]);
    type("Work");
    key("Enter");
    expect(onChange).not.toHaveBeenCalled();
    expect(input().value).toBe("");
    expect(host.querySelector('[role="alert"]')).toBeNull();
  });

  // Final review (tags): a refused part of a pasted list was dropped without a word.
  it("keeps a refused part of a comma list in the field and says why", () => {
    show([]);
    const long = "x".repeat(33);
    type(`prod,${long},db`);
    expect(onChange).toHaveBeenLastCalledWith(["prod"]);
    expect(input().value).toBe(`${long},db`);
    expect(host.querySelector('[role="alert"]')?.textContent).toBe(en.editor.tagTooLong);
  });

  // Final review (tags): Save with a refused tag in the field saved the item without it.
  it("commit() adds the typed text, or refuses with the field's error", () => {
    const handle = createRef<TagsEditorHandle>();
    show(["work"], handle);
    expect(handle.current!.commit()).toEqual(["work"]);
    type("x".repeat(33));
    let result: string[] | null = [];
    act(() => void (result = handle.current!.commit()));
    expect(result).toBeNull();
    expect(input().value).toBe("x".repeat(33));
    expect(host.querySelector('[role="alert"]')?.textContent).toBe(en.editor.tagTooLong);
    expect(document.activeElement).toBe(input());
    type(" Prod ");
    act(() => void (result = handle.current!.commit()));
    expect(result).toEqual(["Prod", "work"]);
    expect(onChange).toHaveBeenLastCalledWith(["Prod", "work"]);
  });
});
