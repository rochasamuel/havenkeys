// Runs under Node in vitest; excluded from tsconfig because it uses Node APIs.
// The extension's provider table must be the core's: same providers, same
// names, same origins (crates/havenkeys-core/src/sso.rs).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { SSO_PROVIDERS } from "./sso";

const RUST = join(__dirname, "../../../crates/havenkeys-core/src/sso.rs");

function arms(src: string, fn: string): Record<string, string> {
  const body = src.slice(src.indexOf(`pub fn ${fn}(self)`));
  const block = body.slice(0, body.indexOf("\n    }\n"));
  const out: Record<string, string> = {};
  for (const m of block.matchAll(/Self::(\w+)\s*=>\s*(&\[[^\]]*\]|"[^"]*")/g)) out[(m[1] as string).toLowerCase()] = m[2] as string;
  return out;
}

describe("provider table", () => {
  it("matches havenkeys-core sso.rs", () => {
    const src = readFileSync(RUST, "utf8");
    const names = arms(src, "name");
    const origins = arms(src, "origins");
    expect(Object.keys(names).sort()).toEqual(Object.keys(SSO_PROVIDERS).sort());
    for (const [p, v] of Object.entries(SSO_PROVIDERS)) {
      expect(`"${v.name}"`).toBe(names[p]);
      expect([...(origins[p] as string).matchAll(/"([^"]+)"/g)].map((m) => m[1])).toEqual([...v.origins]);
    }
  });
});
