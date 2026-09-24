import { describe, expect, it } from "vitest";
import { b64urlLength, envelope, MAX_MATCHES, parseIncoming } from "./index";

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const match = { id: ID, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host" };

describe("parseIncoming", () => {
  it("accepts every valid message shape", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "status", state: "unlocked", vaultExists: true } },
      { v: 1, id: 2, result: { type: "lock" } },
      { v: 1, id: 3, result: { type: "find_matches", matches: [match] } },
      { v: 1, id: 4, result: { type: "fill_item", username: "octo", password: "pw" } },
      { v: 1, id: 5, result: { type: "fill_item", username: null, password: null } },
      { v: 1, id: 6, result: { type: "get_totp", code: "123456", period: 30, secondsRemaining: 12 } },
      { v: 1, id: 7, error: { code: "denied", message: "x" } },
      { v: 1, id: null, error: { code: "malformed", message: "x" } },
      { v: 1, event: { type: "locked" } },
      { v: 1, event: { type: "unlocked" } },
      { v: 1, event: { type: "disconnected" } },
      { v: 1, id: 8, result: { type: "generate_password", password: "x" } },
      { v: 1, id: 9, result: { type: "check_login", action: "add", itemId: null } },
      { v: 1, id: 10, result: { type: "check_login", action: "unchanged", itemId: null } },
      { v: 1, id: 11, result: { type: "check_login", action: "update", itemId: ID } },
      { v: 1, id: 12, result: { type: "save_login", itemId: ID } },
    ];
    for (const m of ok) expect(parseIncoming(m), JSON.stringify(m)).not.toBeNull();
  });

  it("rejects anything else", () => {
    const bad: unknown[] = [
      null,
      "string",
      [],
      {},
      { v: 2, id: 1, result: { type: "lock" } },
      { v: 1, id: 1 },
      { v: 1, id: 1, result: { type: "lock" }, error: { code: "denied", message: "x" } },
      { v: 1, id: 1, result: { type: "lock" }, extra: 1 },
      { v: 1, id: 1, result: { type: "lock", extra: 1 } },
      { v: 1, id: 1, result: { type: "dump_vault" } },
      { v: 1, id: -1, result: { type: "lock" } },
      { v: 1, id: 1.5, result: { type: "lock" } },
      { v: 1, id: null, result: { type: "lock" } },
      { v: 1, id: 1, result: { type: "status", state: "open", vaultExists: true } },
      { v: 1, id: 1, result: { type: "find_matches", matches: [{ ...match, password: "pw" }] } },
      { v: 1, id: 1, result: { type: "find_matches", matches: [{ ...match, id: "../../etc" }] } },
      { v: 1, id: 1, result: { type: "find_matches", matches: Array(MAX_MATCHES + 1).fill(match) } },
      { v: 1, id: 1, result: { type: "get_totp", code: "<img>", period: 30, secondsRemaining: 1 } },
      { v: 1, id: 1, error: { code: "pwned", message: "x" } },
      { v: 1, event: { type: "locked", extra: true } },
      { v: 1, event: { type: "other" } },
      { v: 1, id: 1, event: { type: "locked" } },
    ];
    for (const m of bad) expect(parseIncoming(m), JSON.stringify(m)).toBeNull();
  });

  it("does not trust inherited properties", () => {
    const proto = { type: "lock" };
    const result = Object.create(proto) as object;
    expect(parseIncoming({ v: 1, id: 1, result })).toBeNull();
  });
});

describe("envelope", () => {
  it("builds a versioned request", () => {
    expect(envelope(3, { type: "status" })).toEqual({ v: 1, id: 3, request: { type: "status" } });
  });
  it("refuses ids the Rust side cannot represent", () => {
    for (const id of [-1, 1.5, 2 ** 32, NaN]) expect(() => envelope(id, { type: "status" })).toThrow();
  });
});

describe("phase 5 results", () => {
  it("rejects inconsistent or loose shapes", () => {
    const bad: unknown[] = [
      { v: 1, id: 1, result: { type: "generate_password", password: "" } },
      { v: 1, id: 1, result: { type: "generate_password", password: 5 } },
      { v: 1, id: 1, result: { type: "generate_password" } },
      { v: 1, id: 1, result: { type: "check_login", action: "update", itemId: null } },
      { v: 1, id: 1, result: { type: "check_login", action: "add", itemId: ID } },
      { v: 1, id: 1, result: { type: "check_login", action: "delete", itemId: null } },
      { v: 1, id: 1, result: { type: "check_login", action: "add" } },
      { v: 1, id: 1, result: { type: "save_login", itemId: "nope" } },
      { v: 1, id: 1, result: { type: "save_login", itemId: ID, extra: 1 } },
    ];
    for (const m of bad) expect(parseIncoming(m), JSON.stringify(m)).toBeNull();
  });
});

const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";

describe("passkey results", () => {
  it("accepts valid shapes", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "find_passkeys", passkeys: [{ itemId: ID, credentialId: CRED, title: "GitHub", userName: "octo" }] } },
      { v: 1, id: 2, result: { type: "passkey_get", credentialId: CRED, authenticatorData: "AA", clientDataJson: "e30", signature: "MEU", userHandle: "AQ" } },
      { v: 1, id: 3, result: { type: "check_passkey_create", excluded: false, candidates: [{ itemId: ID, title: "t", username: null }] } },
      { v: 1, id: 4, result: { type: "check_passkey_create", excluded: true, candidates: [] } },
      { v: 1, id: 5, result: { type: "passkey_create", credentialId: CRED, attestationObject: "oA", clientDataJson: "e30", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -7 } },
    ];
    for (const m of ok) expect(parseIncoming(m), JSON.stringify(m)).not.toBeNull();
  });

  it("rejects loose or dangerous shapes", () => {
    const bad = [
      { v: 1, id: 1, result: { type: "find_passkeys", passkeys: [{ itemId: ID, credentialId: "AQ", title: "t", userName: "u" }] } },
      { v: 1, id: 1, result: { type: "find_passkeys", passkeys: [{ itemId: ID, credentialId: CRED, title: "t", userName: "u", privateKey: "x" }] } },
      { v: 1, id: 1, result: { type: "passkey_get", credentialId: CRED, authenticatorData: "a+b", clientDataJson: "e30", signature: "MEU", userHandle: "AQ" } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: true, candidates: [{ itemId: ID, title: "t", username: null }] } },
      { v: 1, id: 1, result: { type: "passkey_create", credentialId: CRED, attestationObject: "oA", clientDataJson: "e30", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -257 } },
    ];
    for (const m of bad) expect(parseIncoming(m), JSON.stringify(m)).toBeNull();
  });

  it("measures base64url", () => {
    expect(b64urlLength("")).toBe(0);
    expect(b64urlLength("AQ")).toBe(1);
    expect(b64urlLength(CRED)).toBe(16);
    expect(b64urlLength("A")).toBeNull();
    expect(b64urlLength("a+b/")).toBeNull();
    expect(b64urlLength("AQ==")).toBeNull();
    expect(b64urlLength(5)).toBeNull();
  });
});
