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
