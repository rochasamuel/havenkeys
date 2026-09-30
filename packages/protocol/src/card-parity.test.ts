// Runs under Node in vitest; excluded from tsconfig because it uses Node APIs.
// The extension's card roles and brands must be the Rust protocol's
// (message.rs), and its IIN table the core's (card.rs), in order.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { CARD_BRANDS, CARD_ROLES, IIN_RANGES } from "./card";

const MESSAGE = join(__dirname, "../../../crates/havenkeys-protocol/src/message.rs");
const CARD = join(__dirname, "../../../crates/havenkeys-core/src/card.rs");

function variants(src: string, head: string): string[] {
  const body = src.slice(src.indexOf(head));
  const block = body.slice(0, body.indexOf("}"));
  return [...block.matchAll(/^\s+([A-Z]\w*),$/gm)].map((m) => m[1] as string);
}

describe("card protocol parity", () => {
  const src = readFileSync(MESSAGE, "utf8");
  it("roles match havenkeys-protocol CardRole, in order", () => {
    expect(variants(src, "pub enum CardRole {").map((v) => v[0]!.toLowerCase() + v.slice(1))).toEqual([...CARD_ROLES]);
  });
  it("brands match havenkeys-protocol CardBrandId, in order", () => {
    expect(variants(src, "pub enum CardBrandId {").map((v) => v.toLowerCase())).toEqual([...CARD_BRANDS]);
  });
  it("the IIN table is havenkeys-core's, row for row", () => {
    const rust = readFileSync(CARD, "utf8");
    const rows = [...rust.matchAll(/IinRange \{ brand: CardBrand::(\w+), lo: "(\d+)", hi: "(\d+)" \}/g)].map((m) => ({
      brand: (m[1] as string).toLowerCase(),
      lo: m[2],
      hi: m[3],
    }));
    expect(rows.length).toBeGreaterThan(10);
    expect(IIN_RANGES.map((r) => ({ ...r }))).toEqual(rows);
  });
});
