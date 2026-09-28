// Messages for "Sign in with <provider>": recognising and pressing a
// provider button, and the popup frame the user picks a saved sign-in from.
//
// Same trust boundary as messaging/inline.ts: content scripts run inside
// untrusted pages, so every field here is bounds-checked before the
// background acts on it; the origin a request is valid for always comes
// from the browser's sender data, never from the message.

import { isSsoProvider, isUuid, MAX_ACCOUNT_CHARS, type SsoProvider } from "@havenkeys/protocol";
import { MAX_TITLE_CHARS, TOKEN } from "./inline";

// ---------------------------------------------------------------- content → background

export type SsoContentRequest =
  /** Top frame: these providers have a visible, qualifying button. */
  | { type: "cs_sso_buttons"; providers: SsoProvider[] }
  /** A trusted click (dispatched by the extension) on a provider button. */
  | { type: "cs_sso_click"; provider: SsoProvider }
  /** A trusted click on the saved account's row in the provider's chooser. */
  | { type: "cs_sso_account"; account: string }
  /** The user took over (typed, clicked elsewhere) after HavenKeys pressed. */
  | { type: "cs_sso_stop" };

export type SsoButtonsReply = { ok: true; token: string } | { ok: false };

/** What a (re)loaded top frame on the provider's site should do. */
export type SsoReady = { kind: "choose"; account: string } | null;

// ---------------------------------------------------------------- background → content

export type BackgroundToSso =
  | { type: "bg_sso_show"; token: string }
  | { type: "bg_sso_close"; token: string }
  | { type: "bg_sso_resize"; token: string; height: number }
  | { type: "bg_sso_press"; provider: SsoProvider };

export type SsoPressReply = { pressed: boolean };

// ---------------------------------------------------------------- sso frame → background

export type SsoFrameRequest =
  | { type: "sso_state"; token: string }
  | { type: "sso_pick"; token: string; itemId: string }
  /** `account`: empty means none (an update with nothing to learn). */
  | { type: "sso_save"; token: string; account: string; title: string | null }
  | { type: "sso_dismiss"; token: string }
  | { type: "sso_resize"; token: string; height: number };

export interface SsoRowView {
  id: string;
  provider: SsoProvider;
  title: string;
  account: string | null;
}

export type SsoView =
  | { mode: "offer"; site: string; rows: SsoRowView[] }
  | { mode: "save"; site: string; provider: SsoProvider; account: string | null; title: string | null; action: "add" | "update" }
  | { mode: "notice"; site: string; provider: SsoProvider };

// ---------------------------------------------------------------- validation

/** Bounds on the sso frame height its page may report. */
export const SSO_MIN_HEIGHT = 90;
export const SSO_MAX_HEIGHT = 360;

type Obj = Record<string, unknown>;

function obj(msg: unknown): Obj | null {
  return typeof msg === "object" && msg !== null && !Array.isArray(msg) ? (msg as Obj) : null;
}

function keysAre(o: Obj, keys: readonly string[]): boolean {
  const own = Object.keys(o);
  return own.length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
}

const isToken = (v: unknown): v is string => typeof v === "string" && TOKEN.test(v);

const isHeight = (v: unknown): v is number =>
  typeof v === "number" && Number.isInteger(v) && v >= SSO_MIN_HEIGHT && v <= SSO_MAX_HEIGHT;

/** Trimmed-shaped, email-like account text: the content script only ever sends this. */
const ACCOUNT = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function isAccount(v: unknown): v is string {
  return typeof v === "string" && v.length >= 3 && v.length <= MAX_ACCOUNT_CHARS && ACCOUNT.test(v);
}

function isProviderList(v: unknown): v is SsoProvider[] {
  return Array.isArray(v) && v.length >= 1 && v.length <= 4 && v.every(isSsoProvider) && new Set(v).size === v.length;
}

/** A sso_save account: empty (none) or a bounded account-shaped string. */
function isSaveAccount(v: unknown): v is string {
  return typeof v === "string" && v.length <= MAX_ACCOUNT_CHARS && (v.length === 0 || isAccount(v));
}

function isTitle(v: unknown): v is string | null {
  return v === null || (typeof v === "string" && v.length <= MAX_TITLE_CHARS);
}

export function parseSsoContentRequest(msg: unknown): SsoContentRequest | null {
  const o = obj(msg);
  if (!o) return null;
  switch (o.type) {
    case "cs_sso_buttons":
      return keysAre(o, ["type", "providers"]) && isProviderList(o.providers)
        ? { type: "cs_sso_buttons", providers: o.providers }
        : null;
    case "cs_sso_click":
      return keysAre(o, ["type", "provider"]) && isSsoProvider(o.provider)
        ? { type: "cs_sso_click", provider: o.provider }
        : null;
    case "cs_sso_account":
      return keysAre(o, ["type", "account"]) && isAccount(o.account)
        ? { type: "cs_sso_account", account: o.account }
        : null;
    case "cs_sso_stop":
      return keysAre(o, ["type"]) ? { type: "cs_sso_stop" } : null;
    default:
      return null;
  }
}

export function parseSsoFrameRequest(msg: unknown): SsoFrameRequest | null {
  const o = obj(msg);
  if (!o || !isToken(o.token)) return null;
  const token = o.token;
  switch (o.type) {
    case "sso_state":
    case "sso_dismiss":
      return keysAre(o, ["type", "token"]) ? { type: o.type, token } : null;
    case "sso_pick":
      return keysAre(o, ["type", "token", "itemId"]) && isUuid(o.itemId)
        ? { type: "sso_pick", token, itemId: o.itemId }
        : null;
    case "sso_save":
      return keysAre(o, ["type", "token", "account", "title"]) && isSaveAccount(o.account) && isTitle(o.title)
        ? { type: "sso_save", token, account: o.account, title: o.title }
        : null;
    case "sso_resize":
      return keysAre(o, ["type", "token", "height"]) && isHeight(o.height)
        ? { type: "sso_resize", token, height: o.height }
        : null;
    default:
      return null;
  }
}

export function parseSsoBackgroundMessage(msg: unknown): BackgroundToSso | null {
  const o = obj(msg);
  if (!o) return null;
  switch (o.type) {
    case "bg_sso_show":
    case "bg_sso_close":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: o.type, token: o.token } : null;
    case "bg_sso_resize":
      return keysAre(o, ["type", "token", "height"]) && isToken(o.token) && isHeight(o.height)
        ? { type: "bg_sso_resize", token: o.token, height: o.height }
        : null;
    case "bg_sso_press":
      return keysAre(o, ["type", "provider"]) && isSsoProvider(o.provider)
        ? { type: "bg_sso_press", provider: o.provider }
        : null;
    default:
      return null;
  }
}

/** What a (re)loaded top frame on the provider's site should do; anything malformed reads as null. */
export function parseSsoReady(v: unknown): SsoReady {
  const o = obj(v);
  if (!o || !keysAre(o, ["kind", "account"]) || o.kind !== "choose" || !isAccount(o.account)) return null;
  return { kind: "choose", account: o.account };
}

/** A content script's reply to bg_sso_press; anything but `{pressed: true}` reads as not pressed. */
export function parsePressReply(v: unknown): SsoPressReply {
  const o = obj(v);
  return o && keysAre(o, ["pressed"]) && o.pressed === true ? { pressed: true } : { pressed: false };
}
