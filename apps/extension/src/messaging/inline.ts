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

import {
  CREDENTIAL_ID_BYTES,
  isB64Url,
  isCardRole,
  isIdentityRole,
  isPasswordOptions,
  isUuid,
  MAX_CARD_ROLES,
  MAX_CARD_VALUE_BYTES,
  MAX_IDENTITY_ROLES,
  MAX_IDENTITY_VALUE_BYTES,
  MAX_URL_BYTES,
  type CardBrandId,
  type CardRole,
  type CardValue,
  type IdentityRole,
  type IdentityValue,
  type PasswordOptions,
  type SsoProvider,
} from "@havenkeys/protocol";
import type { PasskeyRow } from "../webauthn/messages";

export type MenuKind = "login" | "otp" | "new_password" | "identity" | "card";

/** A field's box in its own frame's viewport (card menus a processor frame asks the top frame to host). */
export interface Anchor {
  top: number;
  left: number;
  width: number;
  height: number;
}

/**
 * A card frame's viewport (innerWidth x innerHeight, whole CSS px). The top
 * frame uses it to tell apart iframes that load the same URL.
 */
export interface Viewport {
  width: number;
  height: number;
}

/** Ancestor frames a subframe may report, at most (deeper: it reports an unknown chain). */
export const MAX_FRAME_DEPTH = 8;

/**
 * Where a subframe sits, as its content script sees it (content/ancestry.ts):
 * its ancestors' origins, nearest first, the top page's last; or, where the
 * browser cannot tell (Firefox, a cross-origin parent), only whether its
 * parent is the top page. Card menus and card scans carry it: Rust compares
 * a frame with the top page only, and the background uses this to drop a
 * card frame nested inside a frame Rust would not accept (an ad's).
 */
export type Ancestry = { ancestors: string[] } | { ancestors: null; directChildOfTop: boolean };

/** A card the user typed into a checkout (autofill/card-fill.ts readCardSubmission). */
export interface SubmittedCardWire {
  number: string;
  /** YYYY-MM. */
  expiry: string;
  verificationNumber: string | null;
  cardholderName: string | null;
}

/** Steps of an automatic sign-in, in order. */
export type RunStep = "username" | "password" | "otp";
/** Steps a run can continue to. */
export type NextStep = "password" | "otp";
const NEXT_STEPS: readonly NextStep[] = ["password", "otp"];
const RUN_STEPS: readonly RunStep[] = ["username", "password", "otp"];

// ---------------------------------------------------------------- content → background

export type ContentRequest =
  /** `explicit`: the user clicked the field's HavenKeys icon, so answer even with no matches. */
  | { type: "cs_open_menu"; kind: MenuKind; explicit?: true; roles?: IdentityRole[]; cardRoles?: CardRole[]; anchor?: Anchor; ancestry?: Ancestry; viewport?: Viewport }
  | { type: "cs_close_menu"; token: string }
  | { type: "cs_submit"; username: string | null; password: string | null; currentPassword?: string }
  | { type: "cs_ready" }
  /** The next step's field appeared in this frame during a sign-in run. */
  | { type: "cs_run_step"; kind: NextStep }
  /** The run should end here: the user took over, a stop condition, or nothing appeared. */
  | { type: "cs_run_stop" }
  /** This frame's card fields, answering bg_card_scan. */
  | { type: "cs_card_fields"; scan: string; roles: CardRole[]; ancestry?: Ancestry }
  /** The user submitted a card they typed (top frame only). */
  | { type: "cs_card_submit"; card: SubmittedCardWire };

export type OpenMenuReply = { ok: true; token: string; rows: number; hosted?: true } | { ok: false };
export type ReadyReply = { saveToken: string | null; watch: NextStep | null };

// ---------------------------------------------------------------- background → content

export type FillPayload =
  | { kind: "login"; username: string | null; password: string | null }
  | { kind: "otp"; code: string }
  | { kind: "generated"; password: string }
  | { kind: "identity"; values: IdentityValue[] }
  | { kind: "card"; values: CardValue[] };

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
  /** Resize the menu frame to the height its page reported (menu_resize). */
  | { type: "bg_resize_menu"; token: string; height: number; animate?: true }
  | { type: "bg_show_save"; token: string }
  /** Resize the save prompt to the height its page reported (save_resize). */
  | { type: "bg_resize_save"; token: string; height: number }
  | { type: "bg_close_save"; token: string }
  | { type: "bg_run_end" }
  /** Popup: which identity roles has this (top) frame's first identity form? */
  | { type: "bg_identity_roles" }
  /** Every frame: report your card fields with cs_card_fields. */
  | { type: "bg_card_scan"; scan: string }
  /**
   * Top frame: show menu `token` over child frame `frameId`, at `anchor`
   * inside it. Reply { ok }. `url` is the child's full URL as the browser
   * reported it (query and fragment kept, credentials removed) and
   * `viewport` the size the child reported, to find its <iframe> where
   * runtime.getFrameId does not exist (Chromium; content/host-frame.ts).
   * Only ever sent to frame 0.
   */
  | { type: "bg_host_menu"; token: string; frameId: number; url: string; anchor: Anchor; rows: number; viewport?: Viewport };

export type FillReply = { filled: number; pressing: RunStep | null };

// ---------------------------------------------------------------- menu / save frames → background

export type InlineRequest =
  | { type: "menu_state"; token: string }
  | { type: "menu_pick"; token: string; itemId: string }
  | { type: "menu_pick_passkey"; token: string; itemId: string; credentialId: string }
  /** The desktop generator's saved policy, for the menu's settings panel. */
  | { type: "menu_generator_options"; token: string }
  /** `options`: the policy set in the menu's settings panel; else the desktop's saved one. */
  | { type: "menu_generate"; token: string; options?: PasswordOptions }
  | { type: "menu_open_help"; token: string }
  /** Fill the identity; `documents`: the user confirmed the document fields. */
  | { type: "menu_pick_identity"; token: string; documents: boolean }
  | { type: "menu_pick_card"; token: string; itemId: string }
  /** Open the identity in the desktop app (the menu's empty-identity row). */
  | { type: "menu_open_identity"; token: string }
  /** Locked: bring the desktop app forward on its unlock screen (the password is typed there). */
  | { type: "menu_show_unlock"; token: string }
  | { type: "menu_close"; token: string }
  /** The menu's content height, so wrapped rows are not clipped. `animate`: the user opened or closed a panel. */
  | { type: "menu_resize"; token: string; height: number; animate?: true }
  | { type: "save_state"; token: string }
  /** `title`: what the user left in the prompt's name field (a new login only). */
  | { type: "save_confirm"; token: string; title?: string }
  | { type: "save_dismiss"; token: string }
  /** The save prompt's content height, so a long message is not clipped. */
  | { type: "save_resize"; token: string; height: number };

export interface MenuItemView {
  id: string;
  title: string;
  username: string | null;
  /** Set for a login saved with "Sign in with <provider>"; picking it starts a run. */
  provider: SsoProvider | null;
  /** The login's tags (vault data; rendered as text only). */
  tags: string[];
}

/**
 * Passkeys first: either HavenKeys already holds a passkey for the page (use
 * the site's own sign-in), or the page is a known passkey site with no
 * saved passkey yet (a link to add one). `name` is third-party text from
 * the Passkeys Directory; the menu renders it with textContent only.
 */
export type MenuHint = { kind: "use_passkey" } | { kind: "add_passkey"; name: string };

/** The menu's identity row. No values: counts and role names only. */
export interface IdentityRowView {
  title: string;
  /** Fields in the form the identity has a value for. */
  fills: number;
  /** Document roles the form asks for and the identity has. */
  documents: IdentityRole[];
  /** false on http pages: documents will not be filled. */
  documentsAllowed: boolean;
  /** The identity has no values at all (or does not exist yet). */
  empty: boolean;
  /**
   * The identity item does not exist on this device yet (not synced, or not
   * created). The row only says to open HavenKeys: `open_identity` would
   * have nothing to open.
   */
  missing: boolean;
}

/** A card row in the menu. No number beyond the last four digits. */
export interface CardRowView {
  id: string;
  title: string;
  brand: CardBrandId | null;
  last4: string | null;
  /** MM/YY. */
  expiry: string | null;
  expired: boolean;
}

/** The card save prompt. */
export interface CardSaveView {
  site: string;
  /** The suggested name: the brand's. */
  title: string;
  card: { brand: CardBrandId | null; last4: string; expiry: string };
}

export type IdentityRolesReply = { roles: IdentityRole[] };

export type MenuView =
  | { state: "locked" }
  | { state: "ready"; kind: MenuKind; site: string; items: MenuItemView[]; passkeys: PasskeyRow[]; hint: MenuHint | null; identity: IdentityRowView | null }
  | { state: "cards"; site: string; cards: CardRowView[]; insecure: boolean };

export interface SaveView {
  action: "add" | "update";
  site: string;
  username: string | null;
  /** The suggested name for a new login; null for an update, which keeps its title. */
  title: string | null;
}

/** The core's limit on a login title. */
export const MAX_TITLE_CHARS = 256;

export type InlineReply<T> = { ok: true; value: T } | { ok: false; message: string };

// ---------------------------------------------------------------- validation

/** 128-bit random token, hex. */
export const TOKEN = /^[0-9a-f]{32}$/;

/**
 * The message that hands a menu, save, passkey or sign-in-with frame its
 * session token. Posted by the content script to the frame's window, for
 * the extension's origin only, once the frame has loaded: the token never
 * appears in the frame's URL, where the page could read it and frame the
 * same page itself (security review EX-03).
 */
export const TOKEN_MESSAGE = "hk_token";
export const MAX_USERNAME_CHARS = 512;
export const MAX_PASSWORD_CHARS = 4096;
/** Rows the menu shows before its list scrolls. */
export const MENU_MAX_ROWS = 5;
/** Bounds on the menu height a menu page may report (header, one row, padding … five wrapped rows). */
export const MENU_MIN_HEIGHT = 90;
export const MENU_MAX_HEIGHT = 420;

/** Bounds on the save prompt height its page may report (one-line question … a long wrapped error). */
export const SAVE_MIN_HEIGHT = 100;
export const SAVE_MAX_HEIGHT = 320;

const isSaveHeight = (v: unknown): v is number =>
  typeof v === "number" && Number.isInteger(v) && v >= SAVE_MIN_HEIGHT && v <= SAVE_MAX_HEIGHT;

const isMenuHeight = (v: unknown): v is number =>
  typeof v === "number" && Number.isInteger(v) && v >= MENU_MIN_HEIGHT && v <= MENU_MAX_HEIGHT;

type Obj = Record<string, unknown>;

function obj(msg: unknown): Obj | null {
  return typeof msg === "object" && msg !== null && !Array.isArray(msg) ? (msg as Obj) : null;
}

function keysAre(o: Obj, keys: readonly string[]): boolean {
  const own = Object.keys(o);
  return own.length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
}

const isToken = (v: unknown): v is string => typeof v === "string" && TOKEN.test(v);
const MENU_KINDS: readonly MenuKind[] = ["login", "otp", "new_password", "identity", "card"];

function boundedOrNull(v: unknown, max: number): v is string | null {
  return v === null || (typeof v === "string" && v.length > 0 && v.length <= max);
}

function parseRoleList(v: unknown): IdentityRole[] | null {
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_IDENTITY_ROLES) return null;
  const out: IdentityRole[] = [];
  for (const r of v) {
    if (!isIdentityRole(r) || out.includes(r)) return null;
    out.push(r);
  }
  return out;
}

function parseCardRoleList(v: unknown): CardRole[] | null {
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_CARD_ROLES) return null;
  const out: CardRole[] = [];
  for (const r of v) {
    if (!isCardRole(r) || out.includes(r)) return null;
    out.push(r);
  }
  return out;
}

const within = (x: unknown, lo: number, hi: number): x is number => typeof x === "number" && Number.isFinite(x) && x >= lo && x <= hi;

function parseAnchor(v: unknown): Anchor | null {
  const o = obj(v);
  if (!o || !keysAre(o, ["top", "left", "width", "height"])) return null;
  const { top, left, width, height } = o;
  return within(top, -1e5, 1e5) && within(left, -1e5, 1e5) && within(width, 0, 1e4) && within(height, 0, 1e4) ? { top, left, width, height } : null;
}

function parseViewport(v: unknown): Viewport | null {
  const o = obj(v);
  if (!o || !keysAre(o, ["width", "height"])) return null;
  const { width, height } = o;
  return within(width, 0, 1e5) && Number.isInteger(width) && within(height, 0, 1e5) && Number.isInteger(height) ? { width, height } : null;
}

/** An exact http(s) origin, as `URL.origin` writes it. */
export function isWebOrigin(v: unknown): v is string {
  if (typeof v !== "string" || v.length === 0 || v.length > MAX_URL_BYTES) return false;
  try {
    const u = new URL(v);
    return (u.protocol === "https:" || u.protocol === "http:") && u.origin === v;
  } catch {
    return false;
  }
}

export function parseAncestry(v: unknown): Ancestry | null {
  const o = obj(v);
  if (!o) return null;
  if (keysAre(o, ["ancestors"])) {
    const a = o.ancestors;
    if (!Array.isArray(a) || a.length === 0 || a.length > MAX_FRAME_DEPTH || !a.every(isWebOrigin)) return null;
    return { ancestors: [...(a as string[])] };
  }
  if (keysAre(o, ["ancestors", "directChildOfTop"]) && o.ancestors === null && typeof o.directChildOfTop === "boolean") {
    return { ancestors: null, directChildOfTop: o.directChildOfTop };
  }
  return null;
}

function parseSubmittedCard(v: unknown): SubmittedCardWire | null {
  const o = obj(v);
  if (!o || !keysAre(o, ["number", "expiry", "verificationNumber", "cardholderName"])) return null;
  const { number, expiry, verificationNumber, cardholderName } = o;
  if (typeof number !== "string" || !/^[0-9]{12,19}$/.test(number)) return null;
  if (typeof expiry !== "string" || !/^\d{4}-(0[1-9]|1[0-2])$/.test(expiry)) return null;
  if (verificationNumber !== null && !(typeof verificationNumber === "string" && /^[0-9]{3,8}$/.test(verificationNumber))) return null;
  if (!boundedOrNull(cardholderName, 256)) return null;
  return { number, expiry, verificationNumber, cardholderName };
}

/** The top frame's answer to bg_host_menu. */
export function parseHostReply(v: unknown): boolean {
  const o = obj(v);
  return !!o && keysAre(o, ["ok"]) && o.ok === true;
}

export function parseIdentityRolesReply(v: unknown): IdentityRole[] {
  const o = obj(v);
  return (o && keysAre(o, ["roles"]) && parseRoleList(o.roles)) || [];
}

export function parseContentRequest(msg: unknown): ContentRequest | null {
  const o = obj(msg);
  if (!o) return null;
  switch (o.type) {
    case "cs_open_menu": {
      if (!MENU_KINDS.includes(o.kind as MenuKind)) return null;
      const kind = o.kind as MenuKind;
      const keys = Object.keys(o).filter((k) => k !== "type" && k !== "kind");
      if (keys.some((k) => k !== "explicit" && k !== "roles" && k !== "cardRoles" && k !== "anchor" && k !== "ancestry" && k !== "viewport")) return null;
      if ("explicit" in o && o.explicit !== true) return null;
      let roles: IdentityRole[] | undefined;
      if ("roles" in o) {
        if (kind !== "identity" && kind !== "login") return null;
        const parsed = parseRoleList(o.roles);
        if (!parsed) return null;
        roles = parsed;
      } else if (kind === "identity") return null;
      let cardRoles: CardRole[] | undefined;
      let anchor: Anchor | undefined;
      let ancestry: Ancestry | undefined;
      let viewport: Viewport | undefined;
      if (kind === "card") {
        const parsed = parseCardRoleList(o.cardRoles);
        if (!parsed) return null;
        cardRoles = parsed;
        if ("anchor" in o) {
          const a = parseAnchor(o.anchor);
          if (!a) return null;
          anchor = a;
        }
        if ("ancestry" in o) {
          const a = parseAncestry(o.ancestry);
          if (!a) return null;
          ancestry = a;
        }
        if ("viewport" in o) {
          const v = parseViewport(o.viewport);
          if (!v) return null;
          viewport = v;
        }
      } else if ("cardRoles" in o || "anchor" in o || "ancestry" in o || "viewport" in o) return null;
      return {
        type: "cs_open_menu",
        kind,
        ...(o.explicit === true ? { explicit: true as const } : {}),
        ...(roles ? { roles } : {}),
        ...(cardRoles ? { cardRoles } : {}),
        ...(anchor ? { anchor } : {}),
        ...(ancestry ? { ancestry } : {}),
        ...(viewport ? { viewport } : {}),
      };
    }
    case "cs_close_menu":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: "cs_close_menu", token: o.token } : null;
    case "cs_submit": {
      const withCurrent = keysAre(o, ["type", "username", "password", "currentPassword"]);
      if (!withCurrent && !keysAre(o, ["type", "username", "password"])) return null;
      const { username, password, currentPassword } = o;
      if (!boundedOrNull(username, MAX_USERNAME_CHARS) || !boundedOrNull(password, MAX_PASSWORD_CHARS)) return null;
      if (username === null && password === null) return null;
      if (!withCurrent) return { type: "cs_submit", username, password };
      // Only beside a new password, and never null.
      if (password === null || currentPassword === null || !boundedOrNull(currentPassword, MAX_PASSWORD_CHARS)) return null;
      return { type: "cs_submit", username, password, currentPassword };
    }
    case "cs_ready":
      return keysAre(o, ["type"]) ? { type: "cs_ready" } : null;
    case "cs_run_step":
      return keysAre(o, ["type", "kind"]) && NEXT_STEPS.includes(o.kind as NextStep)
        ? { type: "cs_run_step", kind: o.kind as NextStep }
        : null;
    case "cs_run_stop":
      return keysAre(o, ["type"]) ? { type: "cs_run_stop" } : null;
    case "cs_card_fields": {
      const withAncestry = keysAre(o, ["type", "scan", "roles", "ancestry"]);
      if ((!withAncestry && !keysAre(o, ["type", "scan", "roles"])) || !isToken(o.scan)) return null;
      const roles = parseCardRoleList(o.roles);
      if (!roles) return null;
      if (!withAncestry) return { type: "cs_card_fields", scan: o.scan, roles };
      const ancestry = parseAncestry(o.ancestry);
      return ancestry && { type: "cs_card_fields", scan: o.scan, roles, ancestry };
    }
    case "cs_card_submit": {
      if (!keysAre(o, ["type", "card"])) return null;
      const card = parseSubmittedCard(o.card);
      return card && { type: "cs_card_submit", card };
    }
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
    case "menu_pick_identity":
      return keysAre(o, ["type", "token", "documents"]) && typeof o.documents === "boolean"
        ? { type: "menu_pick_identity", token, documents: o.documents }
        : null;
    case "menu_pick_card":
      return keysAre(o, ["type", "token", "itemId"]) && isUuid(o.itemId) ? { type: "menu_pick_card", token, itemId: o.itemId } : null;
    case "menu_open_identity":
      return keysAre(o, ["type", "token"]) ? { type: "menu_open_identity", token } : null;
    case "menu_resize":
      if (keysAre(o, ["type", "token", "height", "animate"])) {
        return isMenuHeight(o.height) && o.animate === true ? { type: "menu_resize", token, height: o.height, animate: true } : null;
      }
      return keysAre(o, ["type", "token", "height"]) && isMenuHeight(o.height) ? { type: "menu_resize", token, height: o.height } : null;
    case "menu_generate":
      if (keysAre(o, ["type", "token", "options"])) {
        if (!isPasswordOptions(o.options)) return null;
        const { length, uppercase, lowercase, digits, symbols, avoidAmbiguous } = o.options;
        return { type: "menu_generate", token, options: { length, uppercase, lowercase, digits, symbols, avoidAmbiguous } };
      }
      return keysAre(o, ["type", "token"]) ? { type: "menu_generate", token } : null;
    case "save_resize":
      return keysAre(o, ["type", "token", "height"]) && isSaveHeight(o.height) ? { type: "save_resize", token, height: o.height } : null;
    case "menu_state":
    case "menu_generator_options":
    case "menu_open_help":
    case "menu_show_unlock":
    case "menu_close":
    case "save_state":
    case "save_dismiss":
      return keysAre(o, ["type", "token"]) ? { type: o.type, token } : null;
    case "save_confirm":
      if (keysAre(o, ["type", "token", "title"])) {
        return typeof o.title === "string" && o.title.length <= MAX_TITLE_CHARS ? { type: "save_confirm", token, title: o.title } : null;
      }
      return keysAre(o, ["type", "token"]) ? { type: "save_confirm", token } : null;
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
    case "identity": {
      if (!keysAre(o, ["kind", "values"]) || !Array.isArray(o.values) || o.values.length > MAX_IDENTITY_ROLES) return null;
      const values: IdentityValue[] = [];
      for (const x of o.values) {
        const v = obj(x);
        if (!v || !keysAre(v, ["role", "value"]) || !isIdentityRole(v.role) || typeof v.value !== "string") return null;
        if (v.value.length === 0 || v.value.length > MAX_IDENTITY_VALUE_BYTES || values.some((y) => y.role === v.role)) return null;
        values.push({ role: v.role, value: v.value });
      }
      return { kind: "identity", values };
    }
    case "card": {
      if (!keysAre(o, ["kind", "values"]) || !Array.isArray(o.values) || o.values.length > MAX_CARD_ROLES) return null;
      const values: CardValue[] = [];
      for (const x of o.values) {
        const v = obj(x);
        if (!v || !keysAre(v, ["role", "value"]) || !isCardRole(v.role) || typeof v.value !== "string") return null;
        const role: CardRole = v.role;
        if (v.value.length === 0 || v.value.length > MAX_CARD_VALUE_BYTES || values.some((y) => y.role === role)) return null;
        values.push({ role, value: v.value });
      }
      return { kind: "card", values };
    }
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
      if ((fill?.kind === "identity" || fill?.kind === "card") && (o.submit || o.totp)) return null;
      return fill && { type: "bg_fill", origin: o.origin, token: o.token, fill, submit: o.submit, totp: o.totp };
    }
    case "bg_resize_menu":
      if (keysAre(o, ["type", "token", "height", "animate"])) {
        return isToken(o.token) && isMenuHeight(o.height) && o.animate === true
          ? { type: "bg_resize_menu", token: o.token, height: o.height, animate: true }
          : null;
      }
      return keysAre(o, ["type", "token", "height"]) && isToken(o.token) && isMenuHeight(o.height)
        ? { type: "bg_resize_menu", token: o.token, height: o.height }
        : null;
    case "bg_resize_save":
      return keysAre(o, ["type", "token", "height"]) && isToken(o.token) && isSaveHeight(o.height)
        ? { type: "bg_resize_save", token: o.token, height: o.height }
        : null;
    case "bg_close_menu":
    case "bg_show_save":
    case "bg_close_save":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: o.type, token: o.token } : null;
    case "bg_run_end":
      return keysAre(o, ["type"]) ? { type: "bg_run_end" } : null;
    case "bg_identity_roles":
      return keysAre(o, ["type"]) ? { type: "bg_identity_roles" } : null;
    case "bg_card_scan":
      return keysAre(o, ["type", "scan"]) && isToken(o.scan) ? { type: "bg_card_scan", scan: o.scan } : null;
    case "bg_host_menu": {
      const withViewport = keysAre(o, ["type", "token", "frameId", "url", "anchor", "rows", "viewport"]);
      if ((!withViewport && !keysAre(o, ["type", "token", "frameId", "url", "anchor", "rows"])) || !isToken(o.token)) return null;
      const anchor = parseAnchor(o.anchor);
      const { frameId, url, rows } = o;
      if (!anchor || typeof frameId !== "number" || !Number.isInteger(frameId) || frameId < 1) return null;
      if (typeof url !== "string" || url.length === 0 || url.length > MAX_URL_BYTES) return null;
      if (typeof rows !== "number" || !Number.isInteger(rows) || rows < 1 || rows > MENU_MAX_ROWS) return null;
      if (!withViewport) return { type: "bg_host_menu", token: o.token, frameId, url, anchor, rows };
      const viewport = parseViewport(o.viewport);
      return viewport && { type: "bg_host_menu", token: o.token, frameId, url, anchor, rows, viewport };
    }
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
