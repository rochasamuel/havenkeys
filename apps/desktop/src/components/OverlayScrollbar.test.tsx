// @vitest-environment jsdom
import { act, useRef } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { OverlayScrollbar } from "./OverlayScrollbar";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLDivElement;

function Area() {
  const ref = useRef<HTMLDivElement>(null);
  return (
    <div data-testid="parent">
      <div ref={ref} data-testid="area" />
      <OverlayScrollbar target={ref} />
    </div>
  );
}

const q = (id: string) => host.querySelector<HTMLElement>(`[data-testid="${id}"]`);
const track = () => host.querySelector<HTMLElement>(".overlay-track");
const thumb = () => host.querySelector<HTMLElement>(".overlay-thumb")!;

/** 400 of 1600 visible: the thumb is a quarter of the track. */
function sized(scrollHeight = 1600, clientHeight = 400) {
  const el = q("area")!;
  Object.defineProperty(el, "scrollHeight", { configurable: true, value: scrollHeight });
  Object.defineProperty(el, "clientHeight", { configurable: true, value: clientHeight });
  Object.defineProperty(el, "offsetTop", { configurable: true, value: 100 });
  el.scrollBy = vi.fn();
  return el;
}

const pointer = (type: string, init: PointerEventInit) =>
  new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, ...init });

beforeEach(() => {
  vi.useFakeTimers();
  // jsdom has neither PointerEvent nor pointer capture.
  globalThis.PointerEvent ??= MouseEvent as unknown as typeof PointerEvent;
  HTMLElement.prototype.setPointerCapture = () => undefined;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  act(() => root.render(<Area />));
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.useRealTimers();
});

it("shows over the area while its parent is hovered, sized and placed by the scroll", () => {
  const el = sized();
  el.scrollTop = 600;
  act(() => void q("parent")!.dispatchEvent(new Event("pointerenter")));
  expect(track()!.dataset.visible).toBe("true");
  expect(track()!.style.top).toBe("100px");
  expect(track()!.style.height).toBe("400px");
  expect(thumb().style.height).toBe("100px");
  expect(thumb().style.transform).toBe("translateY(150px)");
  act(() => void q("parent")!.dispatchEvent(new Event("pointerleave")));
  expect(track()!.dataset.visible).toBe("false");
});

it("shows while scrolling, then fades", () => {
  sized();
  act(() => void q("area")!.dispatchEvent(new Event("scroll")));
  expect(track()!.dataset.visible).toBe("true");
  act(() => void vi.advanceTimersByTime(1000));
  expect(track()!.dataset.visible).toBe("false");
});

it("drags the thumb to scroll, and a click on the track pages", () => {
  const el = sized();
  act(() => void q("parent")!.dispatchEvent(new Event("pointerenter")));
  act(() => void thumb().dispatchEvent(pointer("pointerdown", { clientY: 10 })));
  // The thumb has 300px to travel for 1200px of scroll: 4px a pixel.
  act(() => void thumb().dispatchEvent(pointer("pointermove", { clientY: 60 })));
  expect(el.scrollTop).toBe(200);
  act(() => void thumb().dispatchEvent(pointer("pointerup", { clientY: 60 })));
  act(() => void track()!.dispatchEvent(pointer("pointerdown", { clientY: 390 })));
  expect(el.scrollBy).toHaveBeenCalledWith({ top: 360 });
});

it("draws nothing when everything fits", () => {
  sized(400, 400);
  act(() => void q("parent")!.dispatchEvent(new Event("pointerenter")));
  expect(track()).toBeNull();
});
