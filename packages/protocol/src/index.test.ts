import { describe, expect, it } from "vitest";
import { envelope, MAX_MATCHES, parseIncoming } from "./index";

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
