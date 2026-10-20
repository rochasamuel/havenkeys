import { describe, expect, test } from "vitest";
import vectors from "../../../../packages/field-words/fixtures/normalize.json";
import { normalize } from "./text";

// Shared with Android's TextMatchTest and the word-file generator, so the
// three normalizers stay the same rule (spec 2026-10-19-shared-field-words §3.3).
describe("normalize", () => {
  test.each(vectors)("$in → $out", ({ in: raw, out }) => {
    expect(normalize(raw)).toBe(out);
  });
});
