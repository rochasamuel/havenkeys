// Messages for in-page autofill.
//
//   content script ──runtime msg──► background ◄──runtime msg── menu / save frame
//          ▲                            │
//          └──────── tabs msg ──────────┘
//
// Content scripts run inside web pages, so everything they send is treated
// as untrusted: the background validates the shape here and takes the page
// URL from the browser's sender data, never from the message. The menu and
// save frames are extension pages embedded in the page; they know only an
// unguessable session token, and the background checks that the frame
// asking is in the same tab as the session.

import { isUuid } from "@havenkeys/protocol";

export type MenuKind = "login" | "otp" | "new_password";

// ---------------------------------------------------------------- content → background

export type ContentRequest =
  | { type: "cs_open_menu"; kind: MenuKind }
  | { type: "cs_close_menu"; token: string }
  | { type: "cs_submit"; username: string | null; password: string | null }
  | { type: "cs_ready" };

export type OpenMenuReply = { ok: true; token: string; rows: number } | { ok: false };
export type ReadyReply = { saveToken: string | null };

// ---------------------------------------------------------------- background → content

export type FillPayload =
  | { kind: "login"; username: string | null; password: string | null }
  | { kind: "otp"; code: string }
  | { kind: "generated"; password: string };

export type BackgroundToContent =
  /**
   * `origin` is the page origin the desktop matched; the content script
   * refuses to fill if its own origin differs (the frame navigated). `token`
   * names the menu the user picked from; null for a fill from the toolbar
   * popup, which targets the page's login form.
   */
  | { type: "bg_fill"; origin: string; token: string | null; fill: FillPayload }
  | { type: "bg_close_menu"; token: string }
  | { type: "bg_show_save"; token: string }
  | { type: "bg_close_save"; token: string };

export type FillReply = { filled: number };

// ---------------------------------------------------------------- menu / save frames → background

export type InlineRequest =
  | { type: "menu_state"; token: string }
  | { type: "menu_pick"; token: string; itemId: string }
  | { type: "menu_generate"; token: string }
  | { type: "menu_close"; token: string }
  | { type: "save_state"; token: string }
  | { type: "save_confirm"; token: string }
  | { type: "save_dismiss"; token: string };

export interface MenuItemView {
  id: string;
  title: string;
  username: string | null;
}

export type MenuView =
  | { state: "locked" }
  | { state: "ready"; kind: MenuKind; site: string; items: MenuItemView[] };

export interface SaveView {
  action: "add" | "update";
  site: string;
  username: string | null;
}

export type InlineReply<T> = { ok: true; value: T } | { ok: false; message: string };

// ---------------------------------------------------------------- validation

/** 128-bit random token, hex. */
export const TOKEN = /^[0-9a-f]{32}$/;
export const MAX_USERNAME_CHARS = 512;
export const MAX_PASSWORD_CHARS = 4096;

type Obj = Record<string, unknown>;

function obj(msg: unknown): Obj | null {
  return typeof msg === "object" && msg !== null && !Array.isArray(msg) ? (msg as Obj) : null;
}

function keysAre(o: Obj, keys: readonly string[]): boolean {
  const own = Object.keys(o);
  return own.length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
}

const isToken = (v: unknown): v is string => typeof v === "string" && TOKEN.test(v);
const MENU_KINDS: readonly MenuKind[] = ["login", "otp", "new_password"];

function boundedOrNull(v: unknown, max: number): v is string | null {
  return v === null || (typeof v === "string" && v.length > 0 && v.length <= max);
}

export function parseContentRequest(msg: unknown): ContentRequest | null {
  const o = obj(msg);
  if (!o) return null;
  switch (o.type) {
    case "cs_open_menu":
      return keysAre(o, ["type", "kind"]) && MENU_KINDS.includes(o.kind as MenuKind)
        ? { type: "cs_open_menu", kind: o.kind as MenuKind }
        : null;
    case "cs_close_menu":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: "cs_close_menu", token: o.token } : null;
    case "cs_submit": {
      if (!keysAre(o, ["type", "username", "password"])) return null;
      const { username, password } = o;
      if (!boundedOrNull(username, MAX_USERNAME_CHARS) || !boundedOrNull(password, MAX_PASSWORD_CHARS)) return null;
      if (username === null && password === null) return null;
      return { type: "cs_submit", username, password };
    }
    case "cs_ready":
      return keysAre(o, ["type"]) ? { type: "cs_ready" } : null;
    default:
      return null;
  }
}

export function parseInlineRequest(msg: unknown): InlineRequest | null {
  const o = obj(msg);
  if (!o || !isToken(o.token)) return null;
  const token = o.token;
  switch (o.type) {
    case "menu_pick":
      return keysAre(o, ["type", "token", "itemId"]) && isUuid(o.itemId)
        ? { type: "menu_pick", token, itemId: o.itemId }
        : null;
    case "menu_state":
    case "menu_generate":
    case "menu_close":
    case "save_state":
    case "save_confirm":
    case "save_dismiss":
      return keysAre(o, ["type", "token"]) ? { type: o.type, token } : null;
    default:
      return null;
  }
}

function parseFill(v: unknown): FillPayload | null {
  const o = obj(v);
  if (!o) return null;
  const nullableStr = (x: unknown): x is string | null => x === null || typeof x === "string";
  switch (o.kind) {
    case "login":
      return keysAre(o, ["kind", "username", "password"]) && nullableStr(o.username) && nullableStr(o.password)
        ? { kind: "login", username: o.username, password: o.password }
        : null;
    case "otp":
      return keysAre(o, ["kind", "code"]) && typeof o.code === "string" && /^[0-9]{6,8}$/.test(o.code)
        ? { kind: "otp", code: o.code }
        : null;
    case "generated":
      return keysAre(o, ["kind", "password"]) && typeof o.password === "string" && o.password.length > 0
        ? { kind: "generated", password: o.password }
        : null;
    default:
      return null;
  }
}

/** Validate a message the content script received (it must come from the background). */
export function parseBackgroundMessage(msg: unknown): BackgroundToContent | null {
  const o = obj(msg);
  if (!o) return null;
  switch (o.type) {
    case "bg_fill": {
      if (!keysAre(o, ["type", "origin", "token", "fill"]) || typeof o.origin !== "string") return null;
      if (o.token !== null && !isToken(o.token)) return null;
      const fill = parseFill(o.fill);
      return fill && { type: "bg_fill", origin: o.origin, token: o.token, fill };
    }
    case "bg_close_menu":
    case "bg_show_save":
    case "bg_close_save":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: o.type, token: o.token } : null;
    default:
      return null;
  }
}

/** A fresh 128-bit token from the CSPRNG. */
export function newToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}
