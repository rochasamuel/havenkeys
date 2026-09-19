// Deterministic fuzz of the extension-side validator (CLAUDE.md §47):
// whatever the native host sends, parseIncoming never throws, and anything
// it accepts is a message the protocol actually defines.

import { describe, expect, it } from "vitest";
import { MAX_MATCHES, parseIncoming } from "./index";

function rng(seed: number) {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) / 0x1_0000_0000;
  };
}

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const SEEDS: unknown[] = [
  { v: 1, id: 1, result: { type: "status", state: "unlocked", vaultExists: true } },
  { v: 1, id: 2, result: { type: "find_matches", matches: [{ id: ID, title: "t", username: null, hasTotp: false, strength: "same_site" }] } },
  { v: 1, id: 3, result: { type: "fill_item", username: "u", password: "p" } },
  { v: 1, id: 4, result: { type: "get_totp", code: "123456", period: 30, secondsRemaining: 3 } },
  { v: 1, id: 5, result: { type: "check_login", action: "update", itemId: ID } },
  { v: 1, id: 6, result: { type: "save_login", itemId: ID } },
  { v: 1, id: 7, result: { type: "generate_password", password: "x" } },
  { v: 1, id: 8, error: { code: "denied", message: "m" } },
  { v: 1, event: { type: "locked" } },
];

const ATOMS: unknown[] = [null, true, false, 0, -1, 1.5, 2 ** 32, "", "x", ID, "__proto__", "constructor", [], {}, NaN];

/** Replace, delete or add one value somewhere in a JSON-like tree. */
function mutate(r: () => number, v: unknown, depth = 0): unknown {
  if (depth > 4 || v === null || typeof v !== "object") return r() < 0.5 ? ATOMS[Math.floor(r() * ATOMS.length)] : v;
  const copy: Record<string, unknown> | unknown[] = Array.isArray(v) ? [...v] : { ...(v as Record<string, unknown>) };
  const keys = Object.keys(copy);
  const roll = r();
  if (roll < 0.2 || keys.length === 0) {
    (copy as Record<string, unknown>)[r() < 0.5 ? "extra" : String(keys.length)] = ATOMS[Math.floor(r() * ATOMS.length)];
  } else {
    const k = keys[Math.floor(r() * keys.length)] as string;
    if (roll < 0.35) delete (copy as Record<string, unknown>)[k];
    else (copy as Record<string, unknown>)[k] = mutate(r, (copy as Record<string, unknown>)[k], depth + 1);
  }
  return copy;
}

describe("parseIncoming fuzz", () => {
  it("never throws and accepts only exact shapes", () => {
    const r = rng(0xc0ffee);
    let accepted = 0;
    for (let i = 0; i < 20_000; i++) {
      const input = mutate(r, SEEDS[Math.floor(r() * SEEDS.length)]);
      let out: ReturnType<typeof parseIncoming> = null;
      expect(() => (out = parseIncoming(input))).not.toThrow();
      if (!out) continue;
      accepted++;
      const msg = out as NonNullable<ReturnType<typeof parseIncoming>>;
      // An accepted message survives a JSON round trip unchanged in meaning.
      expect(parseIncoming(JSON.parse(JSON.stringify(input)))).toEqual(msg);
      if (msg.kind === "result" && msg.result.type === "find_matches") {
        expect(msg.result.matches.length).toBeLessThanOrEqual(MAX_MATCHES);
      }
      if (msg.kind === "result" && msg.result.type === "check_login") {
        expect(msg.result.action === "update").toBe(msg.result.itemId !== null);
      }
    }
    expect(accepted).toBeGreaterThan(0);
  });

  it("handles prototype-less objects and __proto__ keys", () => {
    const hostile = Object.create(null) as Record<string, unknown>;
    hostile.v = 1;
    hostile.event = { type: "locked" };
    expect(parseIncoming(hostile)).toEqual({ kind: "event", event: { type: "locked" } });
    const proto = JSON.parse('{"v":1,"id":1,"result":{"type":"lock","__proto__":{"x":1}}}');
    expect(parseIncoming(proto)).toBeNull();
  });
});
