// Runs under Node in vitest; excluded from tsconfig because it uses Node APIs.
// The extension's role list must be the Rust protocol's (message.rs IdentityRole).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { IDENTITY_ROLES } from "./identity";

const RUST = join(__dirname, "../../../crates/havenkeys-protocol/src/message.rs");

describe("identity roles", () => {
  it("match havenkeys-protocol IdentityRole, in order", () => {
    const src = readFileSync(RUST, "utf8");
    const body = src.slice(src.indexOf("pub enum IdentityRole {"));
    const block = body.slice(0, body.indexOf("}"));
    const variants = [...block.matchAll(/^\s+([A-Z]\w*),$/gm)].map((m) => m[1] as string);
    const camel = variants.map((v) => v[0]!.toLowerCase() + v.slice(1));
    expect(camel).toEqual([...IDENTITY_ROLES]);
  });
});
