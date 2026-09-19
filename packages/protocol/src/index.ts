// HavenKeys bridge protocol, TypeScript side.
//
// Mirrors crates/havenkeys-protocol/src/message.rs. The Rust side is the
// authority: the native host and the desktop both re-validate everything.
// These validators exist so the extension, too, only ever acts on messages
// of exactly the expected shape. See docs/native-messaging.md.

export const PROTOCOL_VERSION = 1;
export const MAX_URL_BYTES = 4096;
export const MAX_MATCHES = 50;

// ------------------------------------------------------------------ requests

export type Request =
  | { type: "status" }
  | { type: "lock" }
  | { type: "find_matches"; url: string }
  | { type: "fill_item"; itemId: string; url: string }
  | { type: "get_totp"; itemId: string; url: string };

export type RequestType = Request["type"];

export interface RequestEnvelope {
  v: typeof PROTOCOL_VERSION;
  id: number;
  request: Request;
}

// ------------------------------------------------------------------ results

export type LockState = "locked" | "unlocking" | "unlocked" | "locking";
export type MatchStrength = "exact_url" | "same_host" | "same_site";

/** A suggestion. Never contains a secret. */
export interface Match {
  id: string;
  title: string;
  username: string | null;
  hasTotp: boolean;
  strength: MatchStrength;
}

export type Result =
  | { type: "status"; state: LockState; vaultExists: boolean }
  | { type: "lock" }
  | { type: "find_matches"; matches: Match[] }
  | { type: "fill_item"; username: string | null; password: string | null }
  | { type: "get_totp"; code: string; period: number; secondsRemaining: number };

/** The result type that answers request type `T`. */
export type ResultFor<T extends RequestType> = Extract<Result, { type: T }>;

export const ERROR_CODES = [
  "locked",
  "busy",
  "no_vault",
  "not_found",
  "denied",
  "invalid_input",
  "decryption",
  "corrupted",
  "malformed",
  "too_large",
  "unsupported_version",
  "rate_limited",
  "integration_disabled",
  "desktop_unavailable",
  "internal",
] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

export interface WireError {
  code: ErrorCode;
  message: string;
}

export type BridgeEvent = { type: "locked" } | { type: "unlocked" } | { type: "disconnected" };

/** A validated message from the native host. */
export type Incoming =
  | { kind: "result"; id: number; result: Result }
  | { kind: "error"; id: number | null; error: WireError }
  | { kind: "event"; event: BridgeEvent };

// ------------------------------------------------------------------ building

export function isValidId(id: number): boolean {
  return Number.isInteger(id) && id >= 0 && id <= 0xffff_ffff;
}

export function envelope(id: number, request: Request): RequestEnvelope {
  if (!isValidId(id)) throw new RangeError("invalid request id");
  return { v: PROTOCOL_VERSION, id, request };
}

// ------------------------------------------------------------------ validation

type Obj = Record<string, unknown>;

function isObj(v: unknown): v is Obj {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** True if `o` has exactly `keys` (no more, no fewer). */
function hasExactKeys(o: Obj, keys: readonly string[]): boolean {
  const own = Object.keys(o);
  return own.length === keys.length && keys.every((k) => Object.prototype.hasOwnProperty.call(o, k));
}

const isStr = (v: unknown): v is string => typeof v === "string";
const isNullableStr = (v: unknown): v is string | null => v === null || typeof v === "string";
const isBool = (v: unknown): v is boolean => typeof v === "boolean";
const isU32 = (v: unknown): v is number => typeof v === "number" && isValidId(v);
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

const LOCK_STATES: readonly LockState[] = ["locked", "unlocking", "unlocked", "locking"];
const STRENGTHS: readonly MatchStrength[] = ["exact_url", "same_host", "same_site"];

function parseMatch(v: unknown): Match | null {
  if (!isObj(v) || !hasExactKeys(v, ["id", "title", "username", "hasTotp", "strength"])) return null;
  const { id, title, username, hasTotp, strength } = v;
  if (!isStr(id) || !UUID.test(id) || !isStr(title) || !isNullableStr(username) || !isBool(hasTotp)) return null;
  if (!STRENGTHS.includes(strength as MatchStrength)) return null;
  return { id, title, username, hasTotp, strength: strength as MatchStrength };
}

function parseResult(v: unknown): Result | null {
  if (!isObj(v)) return null;
  switch (v.type) {
    case "status":
      if (!hasExactKeys(v, ["type", "state", "vaultExists"])) return null;
      if (!LOCK_STATES.includes(v.state as LockState) || !isBool(v.vaultExists)) return null;
      return { type: "status", state: v.state as LockState, vaultExists: v.vaultExists };
    case "lock":
      return hasExactKeys(v, ["type"]) ? { type: "lock" } : null;
    case "find_matches": {
      if (!hasExactKeys(v, ["type", "matches"]) || !Array.isArray(v.matches)) return null;
      if (v.matches.length > MAX_MATCHES) return null;
      const matches: Match[] = [];
      for (const m of v.matches) {
        const parsed = parseMatch(m);
        if (!parsed) return null;
        matches.push(parsed);
      }
      return { type: "find_matches", matches };
    }
    case "fill_item":
      if (!hasExactKeys(v, ["type", "username", "password"])) return null;
      if (!isNullableStr(v.username) || !isNullableStr(v.password)) return null;
      return { type: "fill_item", username: v.username, password: v.password };
    case "get_totp":
      if (!hasExactKeys(v, ["type", "code", "period", "secondsRemaining"])) return null;
      if (!isStr(v.code) || !/^[0-9]{6,8}$/.test(v.code) || !isU32(v.period) || !isU32(v.secondsRemaining)) return null;
      return { type: "get_totp", code: v.code, period: v.period, secondsRemaining: v.secondsRemaining };
    default:
      return null;
  }
}

function parseError(v: unknown): WireError | null {
  if (!isObj(v) || !hasExactKeys(v, ["code", "message"]) || !isStr(v.message)) return null;
  if (!ERROR_CODES.includes(v.code as ErrorCode)) return null;
  return { code: v.code as ErrorCode, message: v.message };
}

function parseEvent(v: unknown): BridgeEvent | null {
  if (!isObj(v) || !hasExactKeys(v, ["type"])) return null;
  switch (v.type) {
    case "locked":
    case "unlocked":
    case "disconnected":
      return { type: v.type };
    default:
      return null;
  }
}

/**
 * Validate a message received from the native host. Returns null for
 * anything that is not exactly one of the protocol's message shapes.
 */
export function parseIncoming(msg: unknown): Incoming | null {
  if (!isObj(msg) || msg.v !== PROTOCOL_VERSION) return null;
  if ("event" in msg) {
    if (!hasExactKeys(msg, ["v", "event"])) return null;
    const event = parseEvent(msg.event);
    return event && { kind: "event", event };
  }
  const id = msg.id;
  if (id !== null && !isU32(id)) return null;
  if ("result" in msg) {
    if (!hasExactKeys(msg, ["v", "id", "result"]) || id === null) return null;
    const result = parseResult(msg.result);
    return result && { kind: "result", id, result };
  }
  if ("error" in msg) {
    if (!hasExactKeys(msg, ["v", "id", "error"])) return null;
    const error = parseError(msg.error);
    return error && { kind: "error", id, error };
  }
  return null;
}
