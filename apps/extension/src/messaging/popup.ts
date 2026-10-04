// Messages between the popup and the background worker.
//
// The popup never talks to the native host and never supplies a URL: the
// background worker reads the active tab's URL itself.

import type { IdentityRole, Match } from "@havenkeys/protocol";
import type { CardRowView } from "./inline";

export type PopupRequest =
  | { type: "popup_state" }
  | { type: "popup_lock" }
  | { type: "popup_totp"; itemId: string }
  /** Fill this login into the active tab's login form. */
  | { type: "popup_fill"; itemId: string }
  /** Fill this login's current one-time code into the active tab. */
  | { type: "popup_fill_totp"; itemId: string }
  /**
   * Fill this saved card into the active tab's card form. `origin` is the
   * page origin the list was built for; the background refuses a tab that
   * has since moved to another origin.
   */
  | { type: "popup_fill_card"; itemId: string; origin: string }
  /** Show this login in the desktop app's editor. */
  | { type: "popup_open_item"; itemId: string }
  /** Bring the desktop app forward on its unlock screen (the password is typed there). */
  | { type: "popup_show_unlock" }
  /**
   * Fill the page's first identity form. `documents`: null = not asked yet.
   * The answer carries the origin the question named; the background
   * refuses it if the tab has since moved to another origin.
   */
  | { type: "popup_fill_identity"; documents: null }
  | { type: "popup_fill_identity"; documents: boolean; origin: string };

export type PopupState =
  | { kind: "host_unavailable" }
  | { kind: "desktop_unavailable" }
  | { kind: "no_vault" }
  | { kind: "locked" }
  | { kind: "disabled" }
  | { kind: "unlocked"; site: string | null; matches: Match[]; identity: { title: string } | null; cards?: undefined; cardsOrigin?: undefined }
  /** With cards, the origin they were listed for (the fill request must name it). */
  | { kind: "unlocked"; site: string | null; matches: Match[]; identity: { title: string } | null; cards: CardRowView[]; cardsOrigin: string }
  | { kind: "error"; message: string };

export interface TotpView {
  code: string;
  secondsRemaining: number;
}

/** Reply to `popup_fill_identity`: null = filled, or ask about these documents first, for this page origin. */
export type IdentityFillReply = { confirm: IdentityRole[]; origin: string } | null;

export type PopupReply<T> = { ok: true; value: T } | { ok: false; message: string };

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** An http(s) origin exactly as URL serializes it. */
function isOrigin(v: unknown): v is string {
  if (typeof v !== "string" || v.length > 512) return false;
  try {
    const u = new URL(v);
    return (u.protocol === "https:" || u.protocol === "http:") && u.origin === v;
  } catch {
    return false;
  }
}

/** Strictly validate a popup request. */
export function parsePopupRequest(msg: unknown): PopupRequest | null {
  if (typeof msg !== "object" || msg === null || Array.isArray(msg)) return null;
  const o = msg as Record<string, unknown>;
  const keys = Object.keys(o);
  switch (o.type) {
    case "popup_state":
    case "popup_lock":
    case "popup_show_unlock":
      return keys.length === 1 ? { type: o.type } : null;
    case "popup_totp":
    case "popup_fill":
    case "popup_fill_totp":
    case "popup_open_item":
      return keys.length === 2 && typeof o.itemId === "string" && UUID.test(o.itemId)
        ? { type: o.type, itemId: o.itemId }
        : null;
    case "popup_fill_card":
      if (typeof o.itemId !== "string" || !UUID.test(o.itemId)) return null;
      return keys.length === 3 && isOrigin(o.origin) ? { type: "popup_fill_card", itemId: o.itemId, origin: o.origin } : null;
    case "popup_fill_identity":
      if (o.documents === null) return keys.length === 2 ? { type: "popup_fill_identity", documents: null } : null;
      return keys.length === 3 && typeof o.documents === "boolean" && isOrigin(o.origin)
        ? { type: "popup_fill_identity", documents: o.documents, origin: o.origin }
        : null;
    default:
      return null;
  }
}
