// Which <iframe> in the top page holds a child frame (bg_host_menu), so the
// top frame can draw that frame's card menu over it.
//
// `runtime.getFrameId(element)` answers exactly, but only Firefox has it;
// Chromium does not. Without it, a fallback chain:
//   1. the iframes whose resolved src is exactly the child's full URL
//      (query and fragment included: split Stripe Elements, Adyen secured
//      fields and Braintree hosted fields load one file per field and differ
//      only there);
//   2. if none matched, the iframes whose src without query and fragment is
//      the child's;
//   3. one candidate: that one. Several: the one whose client size is the
//      child's viewport (± SIZE_SLACK px), when exactly one is.
// Otherwise null: the child draws its own menu. A wrong match only
// misplaces the menu; the pick still fills the frame the user clicked in.

import { pageUrlForRequest } from "../shared/url";

/** Iframes looked at, at most (a page could add thousands). */
export const MAX_HOST_IFRAMES = 64;
/** How far an iframe's client size may differ from the child's viewport. */
export const SIZE_SLACK = 2;

export interface ViewportSize {
  width: number;
  height: number;
}

/** A URL as `HTMLIFrameElement.src` would write it, or null. */
function normalized(url: string): string | null {
  try {
    return new URL(url).href;
  } catch {
    return null;
  }
}

export function findHostIframe(
  frameId: number,
  url: string,
  iframes: readonly HTMLIFrameElement[],
  getFrameId: ((el: Element) => number) | undefined,
  viewport: ViewportSize | null = null,
): HTMLIFrameElement | null {
  const list = iframes.slice(0, MAX_HOST_IFRAMES);
  if (typeof getFrameId === "function") {
    return (
      list.find((f) => {
        try {
          return getFrameId(f) === frameId;
        } catch {
          return false;
        }
      }) ?? null
    );
  }
  // `src` is the attribute resolved against the page.
  const withSrc = list.filter((f) => f.getAttribute("src") !== null);
  const full = normalized(url);
  let hits = full === null ? [] : withSrc.filter((f) => f.src === full);
  if (hits.length === 0) {
    const want = pageUrlForRequest(url);
    if (!want) return null;
    hits = withSrc.filter((f) => pageUrlForRequest(f.src) === want);
  }
  if (hits.length === 1) return hits[0] as HTMLIFrameElement;
  if (hits.length === 0 || !viewport) return null;
  const sized = hits.filter(
    (f) => Math.abs(f.clientWidth - viewport.width) <= SIZE_SLACK && Math.abs(f.clientHeight - viewport.height) <= SIZE_SLACK,
  );
  return sized.length === 1 ? (sized[0] as HTMLIFrameElement) : null;
}
