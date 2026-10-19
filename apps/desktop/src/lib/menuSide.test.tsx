// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useMenuSide } from "./hooks";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLDivElement;

function Menu({ anchorTop }: { anchorTop: number }) {
  const menu = useMenuSide(true);
  return (
    <div data-anchor={anchorTop}>
      <div ref={menu.ref} data-side={menu.side} />
    </div>
  );
}

/** A 600px window; the anchor is a 40px row at `anchorTop`; the menu is 300px tall. */
function open(anchorTop: number) {
  act(() => root.render(<Menu anchorTop={anchorTop} />));
  return host.querySelector<HTMLElement>("[data-side]")!.dataset.side;
}

beforeEach(() => {
  vi.spyOn(window, "innerHeight", "get").mockReturnValue(600);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(300);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const top = Number(this.dataset.anchor ?? 0);
    return { top, bottom: top + 40 } as DOMRect;
  });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.restoreAllMocks();
});

it("opens below while it fits there", () => {
  expect(open(100)).toBe("bottom");
});

it("opens above when it would run off the bottom and fits above", () => {
  expect(open(450)).toBe("top");
});

it("fitting neither side, opens below and scrolls its pane to show it", () => {
  const scroll = vi.fn();
  HTMLElement.prototype.scrollIntoView = scroll;
  // 280px below, 280px above, 306px needed.
  expect(open(280)).toBe("bottom");
  expect(scroll).toHaveBeenCalledWith({ block: "nearest" });
});
