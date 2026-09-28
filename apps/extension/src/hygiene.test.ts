// Runs under Node in vitest; excluded from the browser tsconfig because it uses Node APIs.
// Source rules for the extension (docs/development.md): no HTML parsing of
// strings, no logging, no dynamic code, no browser storage. Checked on every
// test run so a slip fails CI instead of relying on review. The one storage
// exception is shared/prefs.ts, which keeps a single non-sensitive boolean
// (the in-page suggestions preference) in chrome.storage.local.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** Files allowed to break one rule, by the rule's name. */
const EXCEPTIONS: Record<string, string> = { "src/shared/prefs.ts": "persistent storage" };

const FORBIDDEN: Array<[RegExp, string]> = [
  [/\.innerHTML\b|\.outerHTML\s*=|insertAdjacentHTML|document\.write/, "HTML parsing of strings"],
  [/\bconsole\./, "logging"],
  [/\beval\(|new Function\(|setTimeout\(\s*["'`]/, "dynamic code"],
  [/(chrome|browser)\??\.storage|localStorage|sessionStorage|indexedDB/, "persistent storage"],
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
    const name = file.slice(file.indexOf("src")).replaceAll("\\", "/");
    it(name, () => {
      const text = readFileSync(file, "utf8");
      for (const [re, what] of FORBIDDEN) {
        if (EXCEPTIONS[name] === what) continue;
        expect(re.test(text), `${what} in ${file}`).toBe(false);
      }
    });
  }
});
