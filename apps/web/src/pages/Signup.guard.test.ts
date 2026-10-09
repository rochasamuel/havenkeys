import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// The invite lives in React state and nowhere else. These files must never
// reach for storage, the URL, the title or navigation.
const FILES = ["./Signup.tsx", "../lib/signup.ts", "../lib/api.ts"];
const FORBIDDEN = ["localStorage", "sessionStorage", "indexedDB", "history.", "navigate(", "document.title", "searchParams"];

describe("the invite never reaches storage or the URL", () => {
  for (const file of FILES) {
    it(`${file} touches none of the persistence or URL APIs`, () => {
      const source = readFileSync(fileURLToPath(new URL(file, import.meta.url)), "utf8");
      for (const word of FORBIDDEN) expect(source, `${file} contains ${word}`).not.toContain(word);
    });
  }
});
