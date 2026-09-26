// The HavenKeys icon shown inside a focused login field. Clicking it opens
// or closes the suggestion menu.
//
// It carries nothing from the vault: it only says "HavenKeys can help with
// this field". It lives in a closed shadow root so page stylesheets cannot
// reach its contents, and its host element's styles are set inline with
// !important, like the menu frame (frames.ts). The page can still see the
// host element, hide it or synthesize clicks on it; the content script
// ignores untrusted clicks, and opening the menu grants nothing by itself
// (picks inside the menu frame are guarded separately, see menu/common.ts).

import type { Box } from "./frames";

export const ICON_SIZE = 20;
/** Gap between the icon and the field's right edge. */
const ICON_INSET = 8;
/** How far left the icon moves to get out of the way of the page's own field button. */
export const ICON_STEP = 26;
/** Fields narrower than this get no icon: it would cover what the user types. */
const MIN_FIELD_WIDTH = 90;
const MIN_FIELD_HEIGHT = 18;

const HOST_STYLE: Record<string, string> = {
  all: "initial",
  position: "fixed",
  "z-index": "2147483646",
  display: "block",
  visibility: "visible",
  opacity: "1",
  margin: "0",
  padding: "0",
  border: "0",
  "box-sizing": "border-box",
  "border-radius": "6px",
  background: "#171a19",
  "box-shadow": "0 0 0 1px rgba(201, 164, 92, 0.35)",
  cursor: "pointer",
  "pointer-events": "auto",
  transform: "none",
  filter: "none",
  "clip-path": "none",
};

/**
 * Inside the field, vertically centred, near the right edge; `shift` moves
 * it left in ICON_STEP steps. Null when the field is too small to share.
 */
export function iconBox(field: DOMRect, shift = 0): Box | null {
  if (field.width < MIN_FIELD_WIDTH || field.height < MIN_FIELD_HEIGHT) return null;
  const size = Math.min(ICON_SIZE, field.height - 4);
  return {
    top: field.top + (field.height - size) / 2,
    left: field.right - ICON_INSET - size - shift * ICON_STEP,
    width: size,
    height: size,
  };
}

function mark(): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 1024 1024");
  svg.setAttribute("aria-hidden", "true");
  for (const [k, v] of Object.entries({ display: "block", width: "100%", height: "100%" })) svg.style.setProperty(k, v, "important");
  const shield = document.createElementNS(ns, "path");
  shield.setAttribute("d", "M512 196 L772 300 V500 C772 668 662 792 512 846 C362 792 252 668 252 500 V300 Z");
  shield.setAttribute("fill", "none");
  shield.setAttribute("stroke", "#c9a45c");
  shield.setAttribute("stroke-width", "60");
  shield.setAttribute("stroke-linejoin", "round");
  const head = document.createElementNS(ns, "circle");
  head.setAttribute("cx", "512");
  head.setAttribute("cy", "468");
  head.setAttribute("r", "84");
  head.setAttribute("fill", "#c9a45c");
  const stem = document.createElementNS(ns, "rect");
  for (const [k, v] of Object.entries({ x: "486", y: "520", width: "52", height: "170", rx: "18", fill: "#c9a45c" })) stem.setAttribute(k, v);
  svg.append(shield, head, stem);
  return svg;
}

export class FieldIcon {
  readonly el: HTMLDivElement;

  /** `onClick` runs for trusted clicks only. */
  constructor(box: Box, onClick: () => void) {
    const el = document.createElement("div");
    el.title = "HavenKeys";
    el.setAttribute("role", "button");
    el.setAttribute("aria-label", "HavenKeys: show logins");
    const root = el.attachShadow({ mode: "closed" });
    root.append(mark());
    // Keep focus in the field: the menu belongs to it.
    el.addEventListener("mousedown", (e) => e.preventDefault());
    el.addEventListener("click", (e) => {
      e.preventDefault();
      if (e.isTrusted) onClick();
    });
    this.el = el;
    this.place(box);
    document.documentElement.append(el);
  }

  place(box: Box): void {
    const style = { ...HOST_STYLE, top: `${box.top}px`, left: `${box.left}px`, width: `${box.width}px`, height: `${box.height}px` };
    for (const [k, v] of Object.entries(style)) this.el.style.setProperty(k, v, "important");
  }

  remove(): void {
    this.el.remove();
  }
}
