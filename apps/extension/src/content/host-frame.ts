// Which <iframe> in the top page holds a child frame (bg_host_menu), so the
// top frame can draw that frame's card menu over it.
//
// `runtime.getFrameId(element)` answers exactly, but only Firefox has it;
// Chromium does not. Without it, the iframe whose src (without query and
// fragment) is the child's URL is used when exactly one matches. Otherwise
// null: the child draws its own menu. A wrong match only misplaces the
// menu; the pick still fills the frame the user clicked in.

import { pageUrlForRequest } from "../shared/url";

/** Iframes looked at, at most (a page could add thousands). */
export const MAX_HOST_IFRAMES = 64;

export function findHostIframe(
  frameId: number,
  url: string,
  iframes: readonly HTMLIFrameElement[],
  getFrameId: ((el: Element) => number) | undefined,
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
  const want = pageUrlForRequest(url);
  if (!want) return null;
  // `src` is the attribute resolved against the page.
  const hits = list.filter((f) => f.getAttribute("src") !== null && pageUrlForRequest(f.src) === want);
  return hits.length === 1 ? (hits[0] as HTMLIFrameElement) : null;
}
