// Provider marks as DOM nodes (createElementNS only; path data is ours, see
// @havenkeys/ui/provider-icons). Used by the "sign in with" balloon so a row
// or a save prompt can show which provider a saved login belongs to.

import { PROVIDER_ICONS } from "@havenkeys/ui/provider-icons";
import type { SsoProvider } from "@havenkeys/protocol";

const NS = "http://www.w3.org/2000/svg";

export function providerIcon(p: SsoProvider, size = 18): SVGSVGElement {
  const icon = PROVIDER_ICONS[p];
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", icon.viewBox);
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("provider-icon");
  for (const s of icon.shapes) {
    const el = document.createElementNS(NS, s.rect ? "rect" : "path");
    if (s.rect) {
      const [x, y, w, hgt] = s.rect;
      el.setAttribute("x", String(x));
      el.setAttribute("y", String(y));
      el.setAttribute("width", String(w));
      el.setAttribute("height", String(hgt));
    } else el.setAttribute("d", s.d as string);
    el.setAttribute("fill", s.fill);
    svg.append(el);
  }
  return svg;
}
