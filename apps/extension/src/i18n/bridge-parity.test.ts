// Runs under Node in vitest; excluded from the browser tsconfig because it uses Node APIs.
// The extension shows desktop errors from its own table by code
// (en.ts `errors.bridge`). The English there must stay the text the Rust
// side sends, so switching from the wire text changed nothing for English.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { en } from "./en";

const RUST = join(__dirname, "../../../../crates/havenkeys-protocol/src/message.rs");

function snake(name: string): string {
  return name.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();
}

describe("desktop error messages", () => {
  it("match ErrorCode::message in havenkeys-protocol", () => {
    const src = readFileSync(RUST, "utf8");
    const body = src.slice(src.indexOf("pub fn message(self)"));
    const rust: Record<string, string> = {};
    for (const m of body.matchAll(/ErrorCode::(\w+)\s*=>\s*\{?\s*"([^"]*)"/g)) rust[snake(m[1] as string)] = m[2] as string;
    expect(Object.keys(rust).length).toBeGreaterThan(10);
    expect(en.errors.bridge).toEqual(rust);
  });
});
