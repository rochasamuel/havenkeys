// Where this frame sits (messaging/inline.ts `Ancestry`), for card menus and
// card scans: the content script attaches `frameAncestry()` to
// cs_open_menu (card kind) and cs_card_fields when it is not null.
//
// * Chromium: `location.ancestorOrigins`, which the browser fills in; page
//   script cannot change what the isolated world reads.
// * Elsewhere (Firefox): walk `parent` up to `top`, reading each origin;
//   a cross-origin parent throws, and then only whether the parent is the
//   top page is reported (the unknown-chain form).
//
// An opaque ("null") or otherwise odd origin, or a chain deeper than
// MAX_FRAME_DEPTH, reports the unknown-chain form too; the background then
// accepts the frame only as a direct child of the top page.

import { isWebOrigin, MAX_FRAME_DEPTH, type Ancestry } from "../messaging/inline";

/** The parts of `window` read here (tests pass fakes). */
export interface AncestryWindow {
  readonly top: AncestryWindow | null;
  readonly parent: AncestryWindow;
  readonly location: { readonly origin: string; readonly ancestorOrigins?: ArrayLike<string> };
}

/** This frame's ancestry; null in the top frame. */
export function frameAncestry(win: AncestryWindow = window as unknown as AncestryWindow): Ancestry | null {
  if (win.top === win) return null;
  const unknown = (): Ancestry => ({ ancestors: null, directChildOfTop: win.parent === win.top });
  const listed = win.location.ancestorOrigins;
  if (listed) {
    const ancestors = Array.from(listed);
    return ancestors.length > 0 && ancestors.length <= MAX_FRAME_DEPTH && ancestors.every(isWebOrigin) ? { ancestors } : unknown();
  }
  const ancestors: string[] = [];
  let w = win;
  try {
    while (w !== w.top) {
      if (ancestors.length >= MAX_FRAME_DEPTH) return { ancestors: null, directChildOfTop: false };
      w = w.parent;
      const origin = w.location.origin; // throws for a cross-origin parent
      if (!isWebOrigin(origin)) return unknown();
      ancestors.push(origin);
    }
  } catch {
    return unknown();
  }
  return ancestors.length > 0 ? { ancestors } : unknown();
}
