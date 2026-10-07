// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../i18n/en";
import { VaultTagsProvider } from "../lib/vaultTags";
import { TagsEditor } from "./TagsEditor";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const vault = [
  { name: "staging", count: 3 },
  { name: "work", count: 1 },
];

let host: HTMLElement;
let root: Root;
const onChange = vi.fn<(tags: string[]) => void>();

beforeEach(() => {
  onChange.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function show(value: string[]) {
  act(() =>
    root.render(
      <VaultTagsProvider value={vault}>
        <TagsEditor value={value} onChange={onChange} />
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
  it("adds a typed tag on Enter and on comma, normalised", () => {
    show([]);
    type("  Prod ");
    key("Enter");
    expect(onChange).toHaveBeenLastCalledWith(["prod"]);
    show(["prod"]);
    type("eu");
    key(",");
    expect(onChange).toHaveBeenLastCalledWith(["eu", "prod"]);
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
});
