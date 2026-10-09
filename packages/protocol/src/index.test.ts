import { describe, expect, it } from "vitest";
import { b64urlLength, envelope, MAX_MATCHES, parseIncoming, providersForOrigin } from "./index";

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const match = { id: ID, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host", provider: null, tags: [] };

describe("parseIncoming", () => {
  it("accepts every valid message shape", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "status", state: "unlocked", vaultExists: true, entitlement: "full" } },
      { v: 1, id: 2, result: { type: "lock" } },
      { v: 1, id: 3, result: { type: "find_matches", matches: [match], entitlement: "full" } },
      { v: 1, id: 4, result: { type: "fill_item", username: "octo", password: "pw", autoSubmit: false } },
      { v: 1, id: 5, result: { type: "fill_item", username: null, password: null, autoSubmit: true } },
      { v: 1, id: 6, result: { type: "get_totp", code: "123456", period: 30, secondsRemaining: 12, autoSubmit: false } },
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
      { v: 1, id: 13, result: { type: "open_item" } },
      { v: 1, id: 14, result: { type: "show_unlock" } },
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
      { v: 1, id: 1, result: { type: "status", state: "open", vaultExists: true, entitlement: "full" } },
      { v: 1, id: 1, result: { type: "find_matches", matches: [{ ...match, password: "pw" }], entitlement: "full" } },
      { v: 1, id: 1, result: { type: "find_matches", matches: [{ ...match, id: "../../etc" }], entitlement: "full" } },
      { v: 1, id: 1, result: { type: "find_matches", matches: Array(MAX_MATCHES + 1).fill(match), entitlement: "full" } },
      { v: 1, id: 1, result: { type: "get_totp", code: "<img>", period: 30, secondsRemaining: 1 } },
      { v: 1, id: 1, error: { code: "pwned", message: "x" } },
      { v: 1, event: { type: "locked", extra: true } },
      { v: 1, event: { type: "other" } },
      { v: 1, id: 1, event: { type: "locked" } },
      { v: 1, id: 1, result: { type: "open_item", itemId: ID } },
      { v: 1, id: 1, result: { type: "show_unlock", extra: 1 } },
      { v: 1, id: 1, result: { type: "show_unlock", password: "pw" } },
    ];
    for (const m of bad) expect(parseIncoming(m), JSON.stringify(m)).toBeNull();
  });

  it("does not trust inherited properties", () => {
    const proto = { type: "lock" };
    const result = Object.create(proto) as object;
    expect(parseIncoming({ v: 1, id: 1, result })).toBeNull();
  });

  it("requires a boolean autoSubmit on fills", () => {
    const ok = parseIncoming({ v: 1, id: 1, result: { type: "fill_item", username: "u", password: "p", autoSubmit: true } });
    expect(ok && "result" in ok && ok.result).toEqual({ type: "fill_item", username: "u", password: "p", autoSubmit: true });
    for (const result of [
      { type: "fill_item", username: "u", password: "p" },
      { type: "fill_item", username: "u", password: "p", autoSubmit: "yes" },
      { type: "get_totp", code: "123456", period: 30, secondsRemaining: 1 },
      { type: "get_totp", code: "123456", period: 30, secondsRemaining: 1, autoSubmit: 1 },
    ]) {
      expect(parseIncoming({ v: 1, id: 1, result })).toBeNull();
    }
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
      { v: 1, id: 3, result: { type: "check_passkey_create", excluded: false, candidates: [{ itemId: ID, title: "t", username: null }], upgrade: { kind: "none" } } },
      { v: 1, id: 4, result: { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "none" } } },
      { v: 1, id: 5, result: { type: "passkey_create", credentialId: CRED, attestationObject: "oA", clientDataJson: "e30", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -7 } },
    ];
    for (const m of ok) expect(parseIncoming(m), JSON.stringify(m)).not.toBeNull();
  });

  it("parses the upgrade decision and passkey_status exactly", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "auto", itemId: ID } } },
      { v: 1, id: 2, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "ask", itemId: ID } } },
      { v: 1, id: 3, result: { type: "passkey_status", hasPasskey: false } },
    ];
    for (const m of ok) expect(parseIncoming(m)).not.toBeNull();
    const bad = [
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [] } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "auto", itemId: ID } } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "auto" } } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "none", itemId: ID } } },
      { v: 1, id: 1, result: { type: "check_passkey_create", excluded: false, candidates: [], upgrade: { kind: "maybe" } } },
      { v: 1, id: 1, result: { type: "passkey_status", hasPasskey: "yes" } },
      { v: 1, id: 1, result: { type: "passkey_status", hasPasskey: true, rpId: "x" } },
    ];
    for (const m of bad) expect(parseIncoming(m)).toBeNull();
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

describe("sign in with", () => {
  const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
  it("parses start_sso, check_sso, save_sso and Match.provider", () => {
    expect(parseIncoming({ v: 1, id: 1, result: { type: "start_sso", provider: "google", account: null, providerOrigins: ["https://accounts.google.com"], autoChoose: true } }))
      .toEqual({ kind: "result", id: 1, result: { type: "start_sso", provider: "google", account: null, providerOrigins: ["https://accounts.google.com"], autoChoose: true } });
    expect(parseIncoming({ v: 1, id: 1, result: { type: "check_sso", action: "update", itemId: ID } })?.kind).toBe("result");
    expect(parseIncoming({ v: 1, id: 1, result: { type: "save_sso", itemId: ID } })?.kind).toBe("result");
    // `accounts` is optional (an older desktop omits it) and defaults to none.
    expect(parseIncoming({ v: 1, id: 1, result: { type: "check_sso", action: "add", itemId: null } })).toEqual({
      kind: "result",
      id: 1,
      result: { type: "check_sso", action: "add", itemId: null, accounts: [] },
    });
    expect(parseIncoming({ v: 1, id: 1, result: { type: "check_sso", action: "add", itemId: null, accounts: ["me@gmail.com"] } })).toEqual({
      kind: "result",
      id: 1,
      result: { type: "check_sso", action: "add", itemId: null, accounts: ["me@gmail.com"] },
    });
    const m = { id: ID, title: "t", username: null, hasTotp: false, strength: "same_site", provider: "github", tags: [] };
    expect(parseIncoming({ v: 1, id: 1, result: { type: "find_matches", matches: [m], entitlement: "full" } })?.kind).toBe("result");
  });
  it("parses Match.tags within limits and rejects the rest", () => {
    const match = (tags: unknown) => ({
      id: ID, title: "t", username: null,
      hasTotp: false, strength: "same_host", provider: null, tags,
    });
    const res = (m: unknown) => parseIncoming({ v: 1, id: 1, result: { type: "find_matches", matches: [m], entitlement: "full" } });
    expect(res(match(["staging", "work"]))).not.toBeNull();
    expect(res(match([]))).not.toBeNull();
    expect(res(match(Array.from({ length: 21 }, (_, i) => String(i))))).toBeNull();
    expect(res(match(["x".repeat(129)]))).toBeNull();
    expect(res(match([1]))).toBeNull();
    expect(res(match("staging"))).toBeNull();
    const { tags: _drop, ...noTags } = match([]);
    expect(res(noTags)).toBeNull();
  });
  it("rejects wrong shapes", () => {
    const bad = [
      { type: "start_sso", provider: "okta", account: null, providerOrigins: ["x"], autoChoose: true },
      { type: "start_sso", provider: "google", account: null, providerOrigins: [], autoChoose: true },
      { type: "start_sso", provider: "google", account: null, providerOrigins: ["a", "b", "c", "d", "e"], autoChoose: true },
      { type: "check_sso", action: "update", itemId: null },
      { type: "check_sso", action: "add", itemId: null, accounts: Array(11).fill("a@b.co") },
      { type: "check_sso", action: "add", itemId: null, accounts: [""] },
      { type: "check_sso", action: "add", itemId: null, accounts: ["a".repeat(255)] },
      { type: "check_sso", action: "add", itemId: null, accounts: "me@gmail.com" },
      { type: "find_matches", matches: [{ id: ID, title: "t", username: null, hasTotp: false, strength: "same_site" }], entitlement: "full" },
      { type: "status", state: "unlocked", vaultExists: true, entitlement: "trial" },
      { type: "status", state: "unlocked", vaultExists: true, entitlement: null },
      { type: "status", state: "unlocked", vaultExists: true, extra: 1 },
      { type: "find_matches", matches: [], entitlement: "trial" },
      { type: "find_matches", matches: [], entitlement: "full", extra: 1 },
    ];
    for (const result of bad) expect(parseIncoming({ v: 1, id: 1, result })).toBeNull();
  });
  it("knows which providers an origin belongs to", () => {
    expect(providersForOrigin("https://github.com")).toEqual(["github"]);
    expect(providersForOrigin("https://accounts.google.com.evil.com")).toEqual([]);
  });
});

describe("identity results", () => {
  it("accepts well-formed identity results", () => {
    const ok = [
      { v: 1, id: 1, result: { type: "find_identity", title: "Samuel", email: null, roles: ["fullName", "cpf"] } },
      { v: 1, id: 2, result: { type: "fill_identity", values: [{ role: "city", value: "Brasília" }] } },
      { v: 1, id: 3, result: { type: "fill_identity", values: [] } },
      { v: 1, id: 4, result: { type: "open_identity" } },
    ];
    for (const m of ok) expect(parseIncoming(m), JSON.stringify(m)).not.toBeNull();
  });

  it("rejects unknown roles, duplicates, empty and oversized values", () => {
    const bad = [
      { v: 1, id: 1, result: { type: "find_identity", title: "x", email: null, roles: ["password"] } },
      { v: 1, id: 1, result: { type: "find_identity", title: "x", email: null, roles: ["city", "city"] } },
      { v: 1, id: 1, result: { type: "fill_identity", values: [{ role: "city", value: "" }] } },
      { v: 1, id: 1, result: { type: "fill_identity", values: [{ role: "city", value: "x".repeat(4097) }] } },
      { v: 1, id: 1, result: { type: "fill_identity", values: [{ role: "city", value: "a", extra: 1 }] } },
      { v: 1, id: 1, result: { type: "open_identity", x: 1 } },
    ];
    for (const m of bad) expect(parseIncoming(m), JSON.stringify(m)).toBeNull();
  });
});

import { brandOf, luhnOk } from "./card";

describe("card helpers", () => {
  it("brandOf follows the table order: Elo and Hipercard before the wide ranges", () => {
    expect(brandOf("4111111111111111")).toBe("visa");
    expect(brandOf("5555555555554444")).toBe("mastercard");
    expect(brandOf("2221000000000009")).toBe("mastercard");
    expect(brandOf("378282246310005")).toBe("amex");
    expect(brandOf("6062825624254001")).toBe("hipercard");
    expect(brandOf("4011780000000000")).toBe("elo");
    expect(brandOf("6362970000457013")).toBe("elo");
    expect(brandOf("9999999999999999")).toBeNull();
    expect(brandOf("41x1")).toBeNull();
  });
  it("luhnOk accepts valid numbers only", () => {
    expect(luhnOk("4111111111111111")).toBe(true);
    expect(luhnOk("4111111111111112")).toBe(false);
    expect(luhnOk("4111")).toBe(false);
  });
});

describe("card results", () => {
  const id = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
  const result = (r: unknown) => parseIncoming({ v: 1, id: 1, result: r });
  it("accepts exact card results", () => {
    expect(result({ type: "find_cards", insecure: false, cards: [{ id, title: "Visa", brand: "visa", last4: "1111", expiry: "2033-11" }] })).not.toBeNull();
    expect(result({ type: "find_cards", insecure: true, cards: [] })).not.toBeNull();
    expect(result({ type: "fill_card", frames: [{ values: [{ role: "number", value: "4111111111111111" }] }, { values: [] }] })).not.toBeNull();
    expect(result({ type: "save_card", itemId: id })).not.toBeNull();
  });
  it("rejects anything else", () => {
    for (const bad of [
      { type: "find_cards", insecure: true, cards: [{ id, title: "x", brand: null, last4: null, expiry: null }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: "nubank", last4: null, expiry: null }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: null, last4: "123", expiry: null }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: null, last4: null, expiry: "11/2033" }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: null, last4: null }] },
      { type: "fill_card", frames: [{ values: [{ role: "number", value: "" }] }] },
      { type: "fill_card", frames: [{ values: [{ role: "number", value: "1" }, { role: "number", value: "2" }] }] },
      { type: "fill_card", frames: [{ values: [{ role: "password", value: "1" }] }] },
      { type: "fill_card", frames: Array.from({ length: 9 }, () => ({ values: [] })) },
      { type: "fill_card", frames: [{ values: [], extra: 1 }] },
      { type: "save_card", itemId: "x" },
    ]) {
      expect(result(bad), JSON.stringify(bad)).toBeNull();
    }
  });
});

describe("entitlement", () => {
  const parse = (result: unknown) => parseIncoming({ v: 1, id: 1, result });
  it("reads a missing entitlement (a Full account, or an older desktop) as full", () => {
    expect(parse({ type: "status", state: "unlocked", vaultExists: true })).toMatchObject({
      result: { type: "status", entitlement: "full" },
    });
    expect(parse({ type: "find_matches", matches: [] })).toMatchObject({
      result: { type: "find_matches", entitlement: "full" },
    });
  });
  it("carries frozen through", () => {
    expect(parse({ type: "status", state: "unlocked", vaultExists: true, entitlement: "frozen" })).toMatchObject({
      result: { entitlement: "frozen" },
    });
    expect(parse({ type: "find_matches", matches: [], entitlement: "frozen" })).toMatchObject({
      result: { entitlement: "frozen" },
    });
  });
});
