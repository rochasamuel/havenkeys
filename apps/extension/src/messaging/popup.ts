// Messages between the popup and the background worker.
//
// The popup never talks to the native host and never supplies a URL: the
// background worker reads the active tab's URL itself.

import type { Match } from "@havenkeys/protocol";

export type PopupRequest = { type: "popup_state" } | { type: "popup_lock" } | { type: "popup_totp"; itemId: string };

export type PopupState =
  | { kind: "host_unavailable" }
  | { kind: "desktop_unavailable" }
  | { kind: "no_vault" }
  | { kind: "locked" }
  | { kind: "disabled" }
  | { kind: "unlocked"; site: string | null; matches: Match[] }
  | { kind: "error"; message: string };

export interface TotpView {
  code: string;
  secondsRemaining: number;
}

export type PopupReply<T> = { ok: true; value: T } | { ok: false; message: string };

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** Strictly validate a popup request. */
export function parsePopupRequest(msg: unknown): PopupRequest | null {
  if (typeof msg !== "object" || msg === null || Array.isArray(msg)) return null;
  const o = msg as Record<string, unknown>;
  const keys = Object.keys(o);
  switch (o.type) {
    case "popup_state":
    case "popup_lock":
      return keys.length === 1 ? { type: o.type } : null;
    case "popup_totp":
      return keys.length === 2 && typeof o.itemId === "string" && UUID.test(o.itemId)
        ? { type: "popup_totp", itemId: o.itemId }
        : null;
    default:
      return null;
  }
}
