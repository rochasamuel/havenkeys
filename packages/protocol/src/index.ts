// HavenKeys bridge protocol, TypeScript side.
//
// Mirrors crates/havenkeys-protocol/src/message.rs. The Rust side is the
// authority: the native host and the desktop both re-validate everything.
// These validators exist so the extension, too, only ever acts on messages
// of exactly the expected shape. See docs/native-messaging.md.

export const PROTOCOL_VERSION = 1;
export const MAX_URL_BYTES = 4096;
export const MAX_MATCHES = 50;
/** Byte limits for check_login/save_login (see the Rust protocol crate). */
export const MAX_SECRET_BYTES = 4 * 4096;
export const MAX_USERNAME_BYTES = 4 * 512;

/** Credential IDs HavenKeys creates, and the only length it accepts. */
export const CREDENTIAL_ID_BYTES = 16;
export const MAX_CHALLENGE_BYTES = 1024;
export const MAX_USER_HANDLE_BYTES = 64;
export const MAX_RP_ID_BYTES = 253;
export const MAX_CREDENTIAL_LIST = 64;
/** COSE ES256. */
export const COSE_ES256 = -7;

// ------------------------------------------------------------------ requests

/**
 * `topUrl` is set when `url` is an iframe: the tab's top-level page. The
 * desktop then only serves logins that match both.
 */
export type Request =
  | { type: "status" }
  | { type: "lock" }
  | { type: "find_matches"; url: string; topUrl?: string }
  | { type: "fill_item"; itemId: string; url: string; topUrl?: string }
  | { type: "get_totp"; itemId: string; url: string; topUrl?: string }
  | { type: "generate_password" }
  | { type: "check_login"; url: string; topUrl?: string; username: string | null; password: string }
  | {
      type: "save_login";
      url: string;
      topUrl?: string;
      username: string | null;
      password: string;
      itemId: string | null;
    }
  | { type: "find_passkeys"; url: string; topUrl?: string; rpId: string; allowCredentials: string[] }
  | { type: "passkey_get"; itemId: string; credentialId: string; url: string; topUrl?: string; rpId: string; challenge: string }
  | { type: "check_passkey_create"; url: string; topUrl?: string; rpId: string; userName: string; excludeCredentials: string[]; conditional: boolean }
  | {
      type: "passkey_create";
      url: string;
      topUrl?: string;
      rpId: string;
      challenge: string;
      userHandle: string;
      userName: string;
      displayName: string | null;
      itemId: string | null;
      conditional: boolean;
    }
  | { type: "passkey_status"; url: string; topUrl?: string };

export type RequestType = Request["type"];

export interface RequestEnvelope {
  v: typeof PROTOCOL_VERSION;
  id: number;
  request: Request;
}

// ------------------------------------------------------------------ results

export type LockState = "locked" | "unlocking" | "unlocked" | "locking";
export type MatchStrength = "exact_url" | "same_host" | "same_site";
export type SaveAction = "add" | "update" | "unchanged";
export type UpgradeHint = { kind: "none" } | { kind: "ask"; itemId: string } | { kind: "auto"; itemId: string };

/** A suggestion. Never contains a secret. */
export interface Match {
  id: string;
  title: string;
  username: string | null;
  hasTotp: boolean;
  strength: MatchStrength;
}

/** A passkey offered for a page. Never contains a key. */
export interface PasskeyMatch {
  itemId: string;
  credentialId: string;
  title: string;
  userName: string;
}

/** A login that could hold a new passkey. */
export interface PasskeyCandidate {
  itemId: string;
  title: string;
  username: string | null;
}

export type Result =
  | { type: "status"; state: LockState; vaultExists: boolean }
  | { type: "lock" }
  | { type: "find_matches"; matches: Match[] }
  | { type: "fill_item"; username: string | null; password: string | null }
  | { type: "get_totp"; code: string; period: number; secondsRemaining: number }
  | { type: "generate_password"; password: string }
  | { type: "check_login"; action: SaveAction; itemId: string | null }
  | { type: "save_login"; itemId: string }
  | { type: "find_passkeys"; passkeys: PasskeyMatch[] }
  | { type: "passkey_get"; credentialId: string; authenticatorData: string; clientDataJson: string; signature: string; userHandle: string }
  | { type: "check_passkey_create"; excluded: boolean; candidates: PasskeyCandidate[]; upgrade: UpgradeHint }
  | {
      type: "passkey_create";
      credentialId: string;
      attestationObject: string;
      clientDataJson: string;
      authenticatorData: string;
      publicKey: string;
      publicKeyAlgorithm: number;
    }
  | { type: "passkey_status"; hasPasskey: boolean };

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
  "offline",
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

/** A lowercase hyphenated UUID, the only item ID form the desktop sends. */
export const isUuid = (v: unknown): v is string => typeof v === "string" && UUID.test(v);

const B64URL = /^[A-Za-z0-9_-]*$/;

/** Decoded length of an unpadded base64url string, or null. */
export function b64urlLength(v: unknown): number | null {
  if (typeof v !== "string" || v.length % 4 === 1 || !B64URL.test(v)) return null;
  return Math.floor(v.length / 4) * 3 + ([0, 0, 1, 2][v.length % 4] as number);
}

export const isB64Url = (v: unknown, min: number, max: number): v is string => {
  const n = b64urlLength(v);
  return n !== null && n >= min && n <= max;
};

const isCredentialId = (v: unknown): v is string => isB64Url(v, CREDENTIAL_ID_BYTES, CREDENTIAL_ID_BYTES);
/** Any non-empty base64url blob the host returns (bounded by the frame size anyway). */
const isBlob = (v: unknown): v is string => isB64Url(v, 1, 256 * 1024);

function parsePasskeyMatch(v: unknown): PasskeyMatch | null {
  if (!isObj(v) || !hasExactKeys(v, ["itemId", "credentialId", "title", "userName"])) return null;
  const { itemId, credentialId, title, userName } = v;
  if (!isUuid(itemId) || !isCredentialId(credentialId) || !isStr(title) || !isStr(userName)) return null;
  return { itemId, credentialId, title, userName };
}

function parseCandidate(v: unknown): PasskeyCandidate | null {
  if (!isObj(v) || !hasExactKeys(v, ["itemId", "title", "username"])) return null;
  const { itemId, title, username } = v;
  if (!isUuid(itemId) || !isStr(title) || !isNullableStr(username)) return null;
  return { itemId, title, username };
}

function parseUpgrade(v: unknown): UpgradeHint | null {
  if (!isObj(v)) return null;
  if (v.kind === "none") return hasExactKeys(v, ["kind"]) ? { kind: "none" } : null;
  if (v.kind === "ask" || v.kind === "auto") {
    return hasExactKeys(v, ["kind", "itemId"]) && isUuid(v.itemId) ? { kind: v.kind, itemId: v.itemId } : null;
  }
  return null;
}

function parseList<T>(v: unknown, one: (x: unknown) => T | null): T[] | null {
  if (!Array.isArray(v) || v.length > MAX_MATCHES) return null;
  const out: T[] = [];
  for (const x of v) {
    const p = one(x);
    if (!p) return null;
    out.push(p);
  }
  return out;
}

const LOCK_STATES: readonly LockState[] = ["locked", "unlocking", "unlocked", "locking"];
const STRENGTHS: readonly MatchStrength[] = ["exact_url", "same_host", "same_site"];
const SAVE_ACTIONS: readonly SaveAction[] = ["add", "update", "unchanged"];

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
    case "generate_password":
      if (!hasExactKeys(v, ["type", "password"]) || !isStr(v.password) || v.password.length === 0) return null;
      return { type: "generate_password", password: v.password };
    case "check_login": {
      if (!hasExactKeys(v, ["type", "action", "itemId"])) return null;
      const { action, itemId } = v;
      if (!SAVE_ACTIONS.includes(action as SaveAction)) return null;
      if (itemId !== null && !(isStr(itemId) && UUID.test(itemId))) return null;
      // An update always names its item; nothing else does.
      if ((action === "update") !== (itemId !== null)) return null;
      return { type: "check_login", action: action as SaveAction, itemId };
    }
    case "save_login":
      if (!hasExactKeys(v, ["type", "itemId"]) || !isStr(v.itemId) || !UUID.test(v.itemId)) return null;
      return { type: "save_login", itemId: v.itemId };
    case "find_passkeys": {
      if (!hasExactKeys(v, ["type", "passkeys"])) return null;
      const passkeys = parseList(v.passkeys, parsePasskeyMatch);
      return passkeys && { type: "find_passkeys", passkeys };
    }
    case "passkey_get": {
      if (!hasExactKeys(v, ["type", "credentialId", "authenticatorData", "clientDataJson", "signature", "userHandle"])) return null;
      const { credentialId, authenticatorData, clientDataJson, signature, userHandle } = v;
      if (!isCredentialId(credentialId) || !isBlob(authenticatorData) || !isBlob(clientDataJson) || !isBlob(signature) || !isBlob(userHandle)) return null;
      return { type: "passkey_get", credentialId, authenticatorData, clientDataJson, signature, userHandle };
    }
    case "check_passkey_create": {
      if (!hasExactKeys(v, ["type", "excluded", "candidates", "upgrade"]) || !isBool(v.excluded)) return null;
      const candidates = parseList(v.candidates, parseCandidate);
      const upgrade = parseUpgrade(v.upgrade);
      if (!candidates || !upgrade || (v.excluded && (candidates.length > 0 || upgrade.kind !== "none"))) return null;
      return { type: "check_passkey_create", excluded: v.excluded, candidates, upgrade };
    }
    case "passkey_create": {
      const keys = ["type", "credentialId", "attestationObject", "clientDataJson", "authenticatorData", "publicKey", "publicKeyAlgorithm"];
      if (!hasExactKeys(v, keys)) return null;
      const { credentialId, attestationObject, clientDataJson, authenticatorData, publicKey, publicKeyAlgorithm } = v;
      if (!isCredentialId(credentialId) || !isBlob(attestationObject) || !isBlob(clientDataJson) || !isBlob(authenticatorData) || !isBlob(publicKey)) return null;
      if (publicKeyAlgorithm !== COSE_ES256) return null;
      return { type: "passkey_create", credentialId, attestationObject, clientDataJson, authenticatorData, publicKey, publicKeyAlgorithm };
    }
    case "passkey_status":
      if (!hasExactKeys(v, ["type", "hasPasskey"]) || !isBool(v.hasPasskey)) return null;
      return { type: "passkey_status", hasPasskey: v.hasPasskey };
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
