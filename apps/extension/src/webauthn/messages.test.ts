import { describe, expect, it } from "vitest";
import { bufferSourceBytes, fromB64Url, toB64Url } from "./encoding";
import {
  parseBgWaResult,
  parsePageRequest,
  parsePageResponse,
  parsePkRequest,
  parseWaReply,
  parseWaRequest,
  type CreateOptions,
  type GetOptions,
} from "./messages";

const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";
const ID = "0123456789abcdef0123456789abcdef";
const TOKEN = "f".repeat(32);
const ITEM = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

const create: CreateOptions = {
  rpId: "github.com",
  challenge: "BwcHBwcHBwcHBwcHBwcHBw",
  userId: "AQ",
  userName: "octo",
  userDisplayName: null,
  algs: [-7, -257],
  excludeCredentials: [CRED],
  timeoutMs: 60_000,
};
const get: GetOptions = { rpId: null, challenge: "BwcHBwcHBwcHBwcHBwcHBw", allowCredentials: [], conditional: false, timeoutMs: null };

describe("base64url", () => {
  it("round-trips every length", () => {
    for (let n = 0; n < 40; n++) {
      const bytes = Uint8Array.from({ length: n }, (_, i) => (i * 37 + n) & 0xff);
      expect(fromB64Url(toB64Url(bytes))).toEqual(bytes);
    }
    expect(toB64Url(Uint8Array.of(0xfb, 0xff))).toBe("-_8");
    expect(fromB64Url("A")).toBeNull();
    expect(fromB64Url("a+b/")).toBeNull();
  });
  it("copies buffer sources", () => {
    const buf = Uint8Array.of(1, 2, 3, 4);
    expect(bufferSourceBytes(buf.buffer)).toEqual(buf);
    expect(bufferSourceBytes(new DataView(buf.buffer, 1, 2))).toEqual(Uint8Array.of(2, 3));
    expect(bufferSourceBytes("abc")).toBeNull();
    expect(bufferSourceBytes(null)).toBeNull();
  });
});

describe("page ↔ bridge", () => {
  it("accepts exact requests only", () => {
    expect(parsePageRequest(JSON.stringify({ kind: "create", id: ID, options: create }))).not.toBeNull();
    expect(parsePageRequest(JSON.stringify({ kind: "get", id: ID, options: get }))).not.toBeNull();
    expect(parsePageRequest(JSON.stringify({ kind: "cancel", id: ID }))).not.toBeNull();
    const bad: unknown[] = [
      { kind: "create", id: ID, options: { ...create, excludeCredentials: ["AQ"] } },
      { kind: "create", id: ID, options: { ...create, userId: "" } },
      { kind: "create", id: ID, options: { ...create, userId: "A".repeat(90) } },
      { kind: "create", id: ID, options: { ...create, challenge: "A".repeat(1400) } },
      { kind: "create", id: ID, options: { ...create, userName: "x".repeat(513) } },
      { kind: "create", id: ID, options: { ...create, algs: [1.5] } },
      { kind: "create", id: ID, options: { ...create, extra: 1 } },
      { kind: "get", id: "short", options: get },
      { kind: "get", id: ID, options: { ...get, allowCredentials: Array(65).fill(CRED) } },
      { kind: "get", id: ID, options: { ...get, timeoutMs: -1 } },
      { kind: "dump", id: ID },
    ];
    for (const b of bad) expect(parsePageRequest(JSON.stringify(b)), JSON.stringify(b)).toBeNull();
    expect(parsePageRequest({ kind: "cancel", id: ID })).toBeNull(); // objects are not accepted, only strings
    expect(parsePageRequest("{" + " ".repeat(20_000) + "}")).toBeNull();
    expect(parsePageRequest("not json")).toBeNull();
  });

  it("accepts exact responses only", () => {
    const cred = { type: "get", credentialId: CRED, clientDataJson: "e30", authenticatorData: "AA", signature: "MEU", userHandle: "AQ" };
    expect(parsePageResponse(JSON.stringify({ id: ID, outcome: "credential", credential: cred }))).not.toBeNull();
    expect(parsePageResponse(JSON.stringify({ id: ID, outcome: "fallback" }))).not.toBeNull();
    expect(parsePageResponse(JSON.stringify({ id: ID, outcome: "error", name: "NotAllowedError" }))).not.toBeNull();
    expect(parsePageResponse(JSON.stringify({ id: ID, outcome: "error", name: "TypeError" }))).toBeNull();
    expect(parsePageResponse(JSON.stringify({ id: ID, outcome: "credential", credential: { ...cred, x: 1 } }))).toBeNull();
  });
});

describe("bridge ↔ background ↔ frames", () => {
  it("validates", () => {
    expect(parseWaRequest({ type: "wa_create", options: create })).not.toBeNull();
    expect(parseWaRequest({ type: "wa_get", options: get })).not.toBeNull();
    expect(parseWaRequest({ type: "wa_cancel", token: TOKEN })).not.toBeNull();
    expect(parseWaRequest({ type: "wa_get", options: get, url: "https://github.com" })).toBeNull();
    expect(parseWaReply({ ok: true, token: TOKEN, ui: "chooser" })).not.toBeNull();
    expect(parseWaReply({ ok: false, outcome: { outcome: "fallback" } })).not.toBeNull();
    expect(parseWaReply({ ok: false, outcome: { outcome: "credential" } })).toBeNull();
    expect(parseBgWaResult({ type: "bg_wa_result", token: TOKEN, outcome: { outcome: "fallback" } })).not.toBeNull();
    expect(parsePkRequest({ type: "pk_pick", token: TOKEN, itemId: ITEM, credentialId: CRED })).not.toBeNull();
    expect(parsePkRequest({ type: "pk_save", token: TOKEN, itemId: null })).not.toBeNull();
    expect(parsePkRequest({ type: "pk_pick", token: TOKEN, itemId: "../x", credentialId: CRED })).toBeNull();
    for (const t of ["pk_state", "pk_fallback", "pk_cancel"]) expect(parsePkRequest({ type: t, token: TOKEN })).not.toBeNull();
  });
});
