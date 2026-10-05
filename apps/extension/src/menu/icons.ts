// Provider marks and card network logos as DOM nodes (createElementNS only;
// path data is ours, see @havenkeys/ui/provider-icons and card-brand-icons).
// Used by the "sign in with" balloon, the card menu and the card save prompt.

import { CARD_BRAND_ICONS, GENERIC_CARD_ICON } from "@havenkeys/ui/card-brand-icons";
import { PROVIDER_ICONS, type ProviderIcon } from "@havenkeys/ui/provider-icons";
import type { CardBrandId, SsoProvider } from "@havenkeys/protocol";

const NS = "http://www.w3.org/2000/svg";

function drawIcon(icon: ProviderIcon, size: number, cls: string, height = size, tile = false): SVGSVGElement {
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", icon.viewBox);
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(height));
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add(cls);
  if (tile) {
    // A white tile behind the mark, as on the desktop: Elo and Discover are
    // black, and the menu can sit on a dark page.
    const bg = document.createElementNS(NS, "rect");
    bg.setAttribute("width", "780");
    bg.setAttribute("height", "500");
    bg.setAttribute("rx", "60");
    bg.setAttribute("fill", "#ffffff");
    bg.setAttribute("stroke", "#d0d0d0");
    bg.setAttribute("stroke-width", "12");
    svg.append(bg);
  }
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

export function providerIcon(p: SsoProvider, size = 18): SVGSVGElement {
  return drawIcon(PROVIDER_ICONS[p], size, "provider-icon");
}

/** A card network's logo on a white tile; the generic card for "other" and unknown brands. */
export function cardBrandIcon(brand: CardBrandId | null, size = 24): SVGSVGElement {
  const icon = brand && brand !== "other" && brand in CARD_BRAND_ICONS ? CARD_BRAND_ICONS[brand as keyof typeof CARD_BRAND_ICONS] : GENERIC_CARD_ICON;
  return drawIcon(icon, size, "card-brand-icon", Math.round((size * 500) / 780), true);
}

/** The identity row's glyph: an ID card in the 1.6-stroke icon set. */
export function idCardIcon(size = 16): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", "M4.5 6h15a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM9 12a1.8 1.8 0 1 0 0-3.6A1.8 1.8 0 0 0 9 12zM6 15.5c.5-1.3 1.6-2 3-2s2.5.7 3 2M14 10h3.5M14 13.5h3.5");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.6");
  path.setAttribute("stroke-linecap", "round");
  svg.append(path);
  return svg;
}

/**
 * The generator settings glyph: two switches (sliders), drawn to the icon
 * set's rules (24 grid, 1.6 stroke, round caps and joins; see the desktop's
 * components/Icon.tsx), which has no such glyph of its own.
 */
export function switchesIcon(size = 17): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(ns, "path");
  path.setAttribute(
    "d",
    "M4.5 8h9M18.5 8h1M16 10.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM4.5 16h1M10.5 16h9M8 18.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z",
  );
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.6");
  path.setAttribute("stroke-linecap", "round");
  path.setAttribute("stroke-linejoin", "round");
  svg.append(path);
  return svg;
}

/** The locked menu's Unlock button: an open padlock in the 1.6-stroke icon set. */
export function unlockIcon(size = 17): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", "M7 11h10a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2zM8 11V7.5a4 4 0 0 1 7.75-1.4M12 15v2");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.6");
  path.setAttribute("stroke-linecap", "round");
  path.setAttribute("stroke-linejoin", "round");
  svg.append(path);
  return svg;
}
