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

import { CREDENTIAL_ID_BYTES, isB64Url, isUuid } from "@havenkeys/protocol";
import type { PasskeyRow } from "../webauthn/messages";

export type MenuKind = "login" | "otp" | "new_password";

/** Steps of an automatic sign-in, in order. */
export type RunStep = "username" | "password" | "otp";
/** Steps a run can continue to. */
export type NextStep = "password" | "otp";
const NEXT_STEPS: readonly NextStep[] = ["password", "otp"];
const RUN_STEPS: readonly RunStep[] = ["username", "password", "otp"];

// ---------------------------------------------------------------- content → background

export type ContentRequest =
  /** `explicit`: the user clicked the field's HavenKeys icon, so answer even with no matches. */
  | { type: "cs_open_menu"; kind: MenuKind; explicit?: true }
  | { type: "cs_close_menu"; token: string }
  | { type: "cs_submit"; username: string | null; password: string | null }
  | { type: "cs_ready" }
  /** The next step's field appeared in this frame during a sign-in run. */
  | { type: "cs_run_step"; kind: NextStep }
  /** The run should end here: the user took over, a stop condition, or nothing appeared. */
  | { type: "cs_run_stop" };

export type OpenMenuReply = { ok: true; token: string; rows: number } | { ok: false };
export type ReadyReply = { saveToken: string | null; watch: NextStep | null };

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
   * popup, which targets the page's login form. `submit`: press the page's
   * button after filling (Rust's autoSubmit). `totp`: after a password step,
   * watch for a one-time-code step.
   */
  | { type: "bg_fill"; origin: string; token: string | null; fill: FillPayload; submit: boolean; totp: boolean }
  | { type: "bg_close_menu"; token: string }
  | { type: "bg_show_save"; token: string }
  | { type: "bg_close_save"; token: string }
  | { type: "bg_run_end" };

export type FillReply = { filled: number; pressing: RunStep | null };

// ---------------------------------------------------------------- menu / save frames → background

export type InlineRequest =
  | { type: "menu_state"; token: string }
  | { type: "menu_pick"; token: string; itemId: string }
  | { type: "menu_pick_passkey"; token: string; itemId: string; credentialId: string }
  | { type: "menu_generate"; token: string }
  | { type: "menu_open_help"; token: string }
  | { type: "menu_close"; token: string }
  | { type: "save_state"; token: string }
  | { type: "save_confirm"; token: string }
  | { type: "save_dismiss"; token: string };

export interface MenuItemView {
  id: string;
  title: string;
  username: string | null;
}

/**
 * Passkeys first: either HavenKeys already holds a passkey for the page (use
 * the site's own sign-in), or the page is a known passkey site with no
 * saved passkey yet (a link to add one). `name` is third-party text from
 * the Passkeys Directory; the menu renders it with textContent only.
 */
export type MenuHint = { kind: "use_passkey" } | { kind: "add_passkey"; name: string };

export type MenuView =
  | { state: "locked" }
  | { state: "ready"; kind: MenuKind; site: string; items: MenuItemView[]; passkeys: PasskeyRow[]; hint: MenuHint | null };

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
      if (!MENU_KINDS.includes(o.kind as MenuKind)) return null;
      if (keysAre(o, ["type", "kind"])) return { type: "cs_open_menu", kind: o.kind as MenuKind };
      return keysAre(o, ["type", "kind", "explicit"]) && o.explicit === true
        ? { type: "cs_open_menu", kind: o.kind as MenuKind, explicit: true }
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
    case "cs_run_step":
      return keysAre(o, ["type", "kind"]) && NEXT_STEPS.includes(o.kind as NextStep)
        ? { type: "cs_run_step", kind: o.kind as NextStep }
        : null;
    case "cs_run_stop":
      return keysAre(o, ["type"]) ? { type: "cs_run_stop" } : null;
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
    case "menu_pick_passkey":
      return keysAre(o, ["type", "token", "itemId", "credentialId"]) &&
        isUuid(o.itemId) &&
        isB64Url(o.credentialId, CREDENTIAL_ID_BYTES, CREDENTIAL_ID_BYTES)
        ? { type: "menu_pick_passkey", token, itemId: o.itemId, credentialId: o.credentialId }
        : null;
    case "menu_state":
    case "menu_generate":
    case "menu_open_help":
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
      if (!keysAre(o, ["type", "origin", "token", "fill", "submit", "totp"]) || typeof o.origin !== "string") return null;
      if (o.token !== null && !isToken(o.token)) return null;
      if (typeof o.submit !== "boolean" || typeof o.totp !== "boolean") return null;
      const fill = parseFill(o.fill);
      return fill && { type: "bg_fill", origin: o.origin, token: o.token, fill, submit: o.submit, totp: o.totp };
    }
    case "bg_close_menu":
    case "bg_show_save":
    case "bg_close_save":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: o.type, token: o.token } : null;
    case "bg_run_end":
      return keysAre(o, ["type"]) ? { type: "bg_run_end" } : null;
    default:
      return null;
  }
}

/** A content script's reply to bg_fill; anything malformed reads as "nothing filled". */
export function parseFillReply(v: unknown): FillReply {
  const o = obj(v);
  if (!o || typeof o.filled !== "number" || !Number.isInteger(o.filled) || o.filled < 0) return { filled: 0, pressing: null };
  if (o.pressing !== null && !RUN_STEPS.includes(o.pressing as RunStep)) return { filled: 0, pressing: null };
  return { filled: o.filled, pressing: o.pressing as RunStep | null };
}

/** A fresh 128-bit token from the CSPRNG. */
export function newToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}
