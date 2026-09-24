// Messages of the passkey bridge.
//
//   page.ts (MAIN world) ──CustomEvent (JSON string)──► bridge.ts (ISOLATED)
//        ▲                                                   │ runtime msg
//        └──────────── CustomEvent ◄────────── bg_wa_result ─┤
//                                                            ▼
//   passkey.html frame ──runtime msg──► background ──native──► desktop
//
// Everything from the page is hostile: the bridge parses it here, and the
// background takes the page URL from the browser, never from a message.

import {
  CREDENTIAL_ID_BYTES,
  MAX_CHALLENGE_BYTES,
  MAX_CREDENTIAL_LIST,
  MAX_RP_ID_BYTES,
  MAX_USER_HANDLE_BYTES,
  isB64Url,
  isUuid,
  type PasskeyCandidate,
} from "@havenkeys/protocol";
import { TOKEN } from "../messaging/inline";

export const REQUEST_EVENT = "havenkeys-webauthn-request";
export const RESPONSE_EVENT = "havenkeys-webauthn-response";
export const MAX_TIMEOUT_MS = 5 * 60_000;
/**
 * Our cards stay up at least this long, whatever the site asks: a site
 * timeout of 0 or 1000 ms would otherwise close the chooser before anyone
 * could read it.
 */
export const MIN_TIMEOUT_MS = 10_000;
/** How often the bridge asks whether its session is still alive. */
export const PING_INTERVAL_MS = 20_000;
/** Largest CustomEvent detail accepted from the page. */
export const MAX_EVENT_CHARS = 16_384;
const MAX_NAME_CHARS = 512;
const MAX_ALGS = 16;
const REQ_ID = /^[0-9a-f]{32}$/;

export interface CreateOptions {
  rpId: string | null;
  challenge: string;
  userId: string;
  userName: string;
  userDisplayName: string | null;
  /** COSE algorithm identifiers the site accepts. */
  algs: number[];
  /** Only 16-byte IDs; others cannot be ours and are dropped by page.ts. */
  excludeCredentials: string[];
  timeoutMs: number | null;
}

export interface GetOptions {
  rpId: string | null;
  challenge: string;
  allowCredentials: string[];
  conditional: boolean;
  timeoutMs: number | null;
}

export type PageRequest =
  | { kind: "create"; id: string; options: CreateOptions }
  | { kind: "get"; id: string; options: GetOptions }
  | { kind: "cancel"; id: string };

export type ErrorName = "NotAllowedError" | "InvalidStateError" | "SecurityError" | "AbortError";

export interface CreatedCredential {
  type: "create";
  credentialId: string;
  clientDataJson: string;
  attestationObject: string;
  authenticatorData: string;
  publicKey: string;
  publicKeyAlgorithm: number;
}

export interface AssertedCredential {
  type: "get";
  credentialId: string;
  clientDataJson: string;
  authenticatorData: string;
  signature: string;
  userHandle: string;
}

export type Outcome =
  | { outcome: "credential"; credential: CreatedCredential | AssertedCredential }
  | { outcome: "fallback" }
  | { outcome: "error"; name: ErrorName };

export type PageResponse = Outcome & { id: string };

/** The bridge's "request received": sent at once, before any answer. */
export interface PageAck {
  id: string;
  outcome: "ack";
}

/** The site's timeout, clamped to [MIN_TIMEOUT_MS, MAX_TIMEOUT_MS]; the maximum when unset. */
export function clampTimeout(ms: number | null): number {
  return Math.min(Math.max(ms ?? MAX_TIMEOUT_MS, MIN_TIMEOUT_MS), MAX_TIMEOUT_MS);
}

// ---------------------------------------------------------------- bridge ↔ background

export type WaRequest =
  | { type: "wa_create"; options: CreateOptions }
  | { type: "wa_get"; options: GetOptions }
  | { type: "wa_cancel"; token: string }
  /** Is this session still alive? Also keeps the worker awake. */
  | { type: "wa_ping"; token: string };

/** `ui`: which frame the bridge shows; "none" for conditional mediation. */
export type WaReply =
  | { ok: true; token: string; ui: "chooser" | "create" | "none" }
  | { ok: false; outcome: Exclude<Outcome, { outcome: "credential" }> };

export interface BgWaResult {
  type: "bg_wa_result";
  token: string;
  outcome: Outcome;
}

// ---------------------------------------------------------------- passkey frame ↔ background

export type PkRequest =
  | { type: "pk_state"; token: string }
  | { type: "pk_pick"; token: string; itemId: string; credentialId: string }
  | { type: "pk_save"; token: string; itemId: string | null }
  | { type: "pk_fallback"; token: string }
  | { type: "pk_cancel"; token: string }
  /** "Close" on the already-saved card: the site learns InvalidStateError. */
  | { type: "pk_close"; token: string };

export interface PasskeyRow {
  itemId: string;
  credentialId: string;
  title: string;
  userName: string;
}

export type PkView =
  | { state: "locked"; site: string }
  | { state: "chooser"; site: string; passkeys: PasskeyRow[] }
  | { state: "create"; site: string; userName: string; candidates: PasskeyCandidate[] }
  /** The site's excludeCredentials names a passkey HavenKeys holds. */
  | { state: "exists"; site: string };

// ---------------------------------------------------------------- validation

type Obj = Record<string, unknown>;

function obj(v: unknown): Obj | null {
  return typeof v === "object" && v !== null && !Array.isArray(v) ? (v as Obj) : null;
}

function keysAre(o: Obj, keys: readonly string[]): boolean {
  const own = Object.keys(o);
  return own.length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
}

const isToken = (v: unknown): v is string => typeof v === "string" && TOKEN.test(v);
const isReqId = (v: unknown): v is string => typeof v === "string" && REQ_ID.test(v);
const isCred = (v: unknown): v is string => isB64Url(v, CREDENTIAL_ID_BYTES, CREDENTIAL_ID_BYTES);
const isChallenge = (v: unknown): v is string => isB64Url(v, 1, MAX_CHALLENGE_BYTES);
const isBlob = (v: unknown): v is string => isB64Url(v, 1, 256 * 1024);
const isName = (v: unknown): v is string => typeof v === "string" && v.length <= MAX_NAME_CHARS;
const isRpId = (v: unknown): v is string | null =>
  v === null || (typeof v === "string" && v.length > 0 && v.length <= MAX_RP_ID_BYTES);
const isTimeout = (v: unknown): v is number | null =>
  v === null || (typeof v === "number" && Number.isFinite(v) && v >= 0);
const isCredList = (v: unknown): v is string[] => Array.isArray(v) && v.length <= MAX_CREDENTIAL_LIST && v.every(isCred);
const ERROR_NAMES: readonly ErrorName[] = ["NotAllowedError", "InvalidStateError", "SecurityError", "AbortError"];

function parseCreateOptions(v: unknown): CreateOptions | null {
  const o = obj(v);
  const keys = ["rpId", "challenge", "userId", "userName", "userDisplayName", "algs", "excludeCredentials", "timeoutMs"];
  if (!o || !keysAre(o, keys)) return null;
  const { rpId, challenge, userId, userName, userDisplayName, algs, excludeCredentials, timeoutMs } = o;
  if (!isRpId(rpId) || !isChallenge(challenge) || !isB64Url(userId, 1, MAX_USER_HANDLE_BYTES)) return null;
  if (!isName(userName) || !(userDisplayName === null || isName(userDisplayName))) return null;
  if (!Array.isArray(algs) || algs.length > MAX_ALGS || !algs.every((a) => Number.isInteger(a))) return null;
  if (!isCredList(excludeCredentials) || !isTimeout(timeoutMs)) return null;
  return { rpId, challenge, userId, userName, userDisplayName, algs: algs as number[], excludeCredentials, timeoutMs };
}

function parseGetOptions(v: unknown): GetOptions | null {
  const o = obj(v);
  if (!o || !keysAre(o, ["rpId", "challenge", "allowCredentials", "conditional", "timeoutMs"])) return null;
  const { rpId, challenge, allowCredentials, conditional, timeoutMs } = o;
  if (!isRpId(rpId) || !isChallenge(challenge) || !isCredList(allowCredentials)) return null;
  if (typeof conditional !== "boolean" || !isTimeout(timeoutMs)) return null;
  return { rpId, challenge, allowCredentials, conditional, timeoutMs };
}

function parseJsonString(raw: unknown): unknown {
  if (typeof raw !== "string" || raw.length > MAX_EVENT_CHARS) return null;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return null;
  }
}

/** A request from the page script. Only JSON strings are accepted. */
export function parsePageRequest(raw: unknown): PageRequest | null {
  const o = obj(parseJsonString(raw));
  if (!o || !isReqId(o.id)) return null;
  switch (o.kind) {
    case "create": {
      if (!keysAre(o, ["kind", "id", "options"])) return null;
      const options = parseCreateOptions(o.options);
      return options && { kind: "create", id: o.id, options };
    }
    case "get": {
      if (!keysAre(o, ["kind", "id", "options"])) return null;
      const options = parseGetOptions(o.options);
      return options && { kind: "get", id: o.id, options };
    }
    case "cancel":
      return keysAre(o, ["kind", "id"]) ? { kind: "cancel", id: o.id } : null;
    default:
      return null;
  }
}

function parseCredential(v: unknown): CreatedCredential | AssertedCredential | null {
  const o = obj(v);
  if (!o) return null;
  if (o.type === "create") {
    const keys = ["type", "credentialId", "clientDataJson", "attestationObject", "authenticatorData", "publicKey", "publicKeyAlgorithm"];
    if (!keysAre(o, keys)) return null;
    const { credentialId, clientDataJson, attestationObject, authenticatorData, publicKey, publicKeyAlgorithm } = o;
    if (!isCred(credentialId) || !isBlob(clientDataJson) || !isBlob(attestationObject) || !isBlob(authenticatorData) || !isBlob(publicKey)) return null;
    if (publicKeyAlgorithm !== -7) return null;
    return { type: "create", credentialId, clientDataJson, attestationObject, authenticatorData, publicKey, publicKeyAlgorithm };
  }
  if (o.type === "get") {
    if (!keysAre(o, ["type", "credentialId", "clientDataJson", "authenticatorData", "signature", "userHandle"])) return null;
    const { credentialId, clientDataJson, authenticatorData, signature, userHandle } = o;
    if (!isCred(credentialId) || !isBlob(clientDataJson) || !isBlob(authenticatorData) || !isBlob(signature) || !isBlob(userHandle)) return null;
    return { type: "get", credentialId, clientDataJson, authenticatorData, signature, userHandle };
  }
  return null;
}

export function parseOutcome(v: unknown): Outcome | null {
  const o = obj(v);
  if (!o) return null;
  switch (o.outcome) {
    case "credential": {
      if (!keysAre(o, ["outcome", "credential"])) return null;
      const credential = parseCredential(o.credential);
      return credential && { outcome: "credential", credential };
    }
    case "fallback":
      return keysAre(o, ["outcome"]) ? { outcome: "fallback" } : null;
    case "error":
      return keysAre(o, ["outcome", "name"]) && ERROR_NAMES.includes(o.name as ErrorName)
        ? { outcome: "error", name: o.name as ErrorName }
        : null;
    default:
      return null;
  }
}

/** A response from the bridge, as the page script reads it. */
export function parsePageResponse(raw: unknown): PageResponse | PageAck | null {
  const o = obj(parseJsonString(raw));
  if (!o || !isReqId(o.id)) return null;
  const id: string = o.id;
  const rest: Obj = { ...o };
  delete rest.id;
  if (rest.outcome === "ack") return keysAre(rest, ["outcome"]) ? { id, outcome: "ack" } : null;
  const outcome = parseOutcome(rest);
  return outcome && { id, ...outcome };
}

export function parseWaRequest(msg: unknown): WaRequest | null {
  const o = obj(msg);
  if (!o) return null;
  switch (o.type) {
    case "wa_create": {
      if (!keysAre(o, ["type", "options"])) return null;
      const options = parseCreateOptions(o.options);
      return options && { type: "wa_create", options };
    }
    case "wa_get": {
      if (!keysAre(o, ["type", "options"])) return null;
      const options = parseGetOptions(o.options);
      return options && { type: "wa_get", options };
    }
    case "wa_cancel":
    case "wa_ping":
      return keysAre(o, ["type", "token"]) && isToken(o.token) ? { type: o.type, token: o.token } : null;
    default:
      return null;
  }
}

export function parseWaReply(msg: unknown): WaReply | null {
  const o = obj(msg);
  if (!o) return null;
  if (o.ok === true) {
    if (!keysAre(o, ["ok", "token", "ui"]) || !isToken(o.token)) return null;
    if (o.ui !== "chooser" && o.ui !== "create" && o.ui !== "none") return null;
    return { ok: true, token: o.token, ui: o.ui };
  }
  if (o.ok === false && keysAre(o, ["ok", "outcome"])) {
    const outcome = parseOutcome(o.outcome);
    if (!outcome || outcome.outcome === "credential") return null;
    return { ok: false, outcome };
  }
  return null;
}

/** True only for exactly `{ ok: true }`; anything else means the session is gone. */
export function parsePingReply(msg: unknown): boolean {
  const o = obj(msg);
  return !!o && o.ok === true && keysAre(o, ["ok"]);
}

export function parseBgWaResult(msg: unknown): BgWaResult | null {
  const o = obj(msg);
  if (!o || o.type !== "bg_wa_result" || !keysAre(o, ["type", "token", "outcome"]) || !isToken(o.token)) return null;
  const outcome = parseOutcome(o.outcome);
  return outcome && { type: "bg_wa_result", token: o.token, outcome };
}

export function parsePkRequest(msg: unknown): PkRequest | null {
  const o = obj(msg);
  if (!o || !isToken(o.token)) return null;
  const token = o.token;
  switch (o.type) {
    case "pk_pick":
      return keysAre(o, ["type", "token", "itemId", "credentialId"]) && isUuid(o.itemId) && isCred(o.credentialId)
        ? { type: "pk_pick", token, itemId: o.itemId, credentialId: o.credentialId }
        : null;
    case "pk_save":
      return keysAre(o, ["type", "token", "itemId"]) && (o.itemId === null || isUuid(o.itemId))
        ? { type: "pk_save", token, itemId: o.itemId }
        : null;
    case "pk_state":
    case "pk_fallback":
    case "pk_cancel":
    case "pk_close":
      return keysAre(o, ["type", "token"]) ? { type: o.type, token } : null;
    default:
      return null;
  }
}
