// Runs under Node in vitest; excluded from the browser tsconfig because it uses Node APIs.
// Source rules for the extension (docs/development.md): no HTML parsing of
// strings, no logging, no dynamic code, no browser storage. Checked on every
// test run so a slip fails CI instead of relying on review.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const FORBIDDEN: Array<[RegExp, string]> = [
  [/\.innerHTML\b|\.outerHTML\s*=|insertAdjacentHTML|document\.write/, "HTML parsing of strings"],
  [/\bconsole\./, "logging"],
  [/\beval\(|new Function\(|setTimeout\(\s*["'`]/, "dynamic code"],
  [/chrome\.storage|browser\.storage|localStorage|sessionStorage|indexedDB/, "persistent storage"],
  [/setAttribute\(\s*["'](value|data-)/, "values in attributes"],
];

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return sources(p);
    return p.endsWith(".ts") && !p.endsWith(".test.ts") ? [p] : [];
  });
}

describe("extension source hygiene", () => {
  const files = sources(join(__dirname));
  it("finds the sources", () => {
    expect(files.length).toBeGreaterThan(10);
  });
  for (const file of files) {
    it(file.slice(file.indexOf("src")), () => {
      const text = readFileSync(file, "utf8");
      for (const [re, what] of FORBIDDEN) expect(re.test(text), `${what} in ${file}`).toBe(false);
    });
  }
});
