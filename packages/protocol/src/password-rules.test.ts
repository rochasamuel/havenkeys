// Runs under Node in vitest. The generator constraints of generate_password
// and generator_options (spec 2026-11-14-password-rules-generator).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { isPasswordFieldRules, MAX_FIELD_LENGTH, MAX_PASSWORD_RULES_BYTES, parseIncoming } from "./index";

const OPTIONS = { length: 20, uppercase: true, lowercase: true, digits: true, symbols: true, avoidAmbiguous: false };
const result = (r: unknown) => parseIncoming({ v: 1, id: 1, result: r });

describe("password field rules", () => {
  it("accepts any subset of its keys, in bounds", () => {
    for (const ok of [
      {},
      { rules: "minlength: 8; required: digit;" },
      { minLength: 1 },
      { maxLength: MAX_FIELD_LENGTH },
      { rules: "x".repeat(MAX_PASSWORD_RULES_BYTES), minLength: 8, maxLength: 16 },
      // A page may say min > max; Rust then ignores the bounds.
      { minLength: 20, maxLength: 10 },
    ]) {
      expect(isPasswordFieldRules(ok), JSON.stringify(ok)).toBe(true);
    }
  });

  it("rejects anything else", () => {
    for (const bad of [
      null,
      [],
      "minlength: 8",
      { rules: "" },
      { rules: 7 },
      { rules: "x".repeat(MAX_PASSWORD_RULES_BYTES + 1) },
      // Bytes, not characters: 342 three-byte characters are 1026 bytes.
      { rules: "€".repeat(342) },
      { minLength: 0 },
      { minLength: -1 },
      { maxLength: MAX_FIELD_LENGTH + 1 },
      { maxLength: 2.5 },
      { maxLength: "16" },
      { maxLength: NaN },
      { url: "https://a.com/" },
      { rules: "x", extra: 1 },
    ]) {
      expect(isPasswordFieldRules(bad), JSON.stringify(bad)).toBe(false);
    }
  });

  it("uses the Rust protocol's bounds", () => {
    const src = readFileSync(join(__dirname, "../../../crates/havenkeys-protocol/src/lib.rs"), "utf8");
    expect(src).toContain(`pub const MAX_PASSWORD_RULES_BYTES: usize = ${MAX_PASSWORD_RULES_BYTES};`);
    expect(src).toContain(`pub const MAX_FIELD_LENGTH: u32 = ${MAX_FIELD_LENGTH};`);
  });
});

describe("followed in generator results", () => {
  it("reads a missing key (no constraints, or an older desktop) as null", () => {
    expect(result({ type: "generate_password", password: "x" })).toMatchObject({ result: { followed: null } });
    expect(result({ type: "generator_options", options: OPTIONS })).toMatchObject({ result: { followed: null } });
  });

  it("carries a known source", () => {
    expect(result({ type: "generate_password", password: "x", followed: "field" })).toMatchObject({ result: { followed: "field" } });
    expect(result({ type: "generate_password", password: "x", followed: "site" })).toMatchObject({ result: { followed: "site" } });
    expect(result({ type: "generator_options", options: OPTIONS, followed: "site" })).toMatchObject({ result: { followed: "site" } });
  });

  it("rejects anything else", () => {
    for (const bad of [
      { type: "generate_password", password: "x", followed: null },
      { type: "generate_password", password: "x", followed: "page" },
      { type: "generate_password", password: "x", followed: 1 },
      { type: "generate_password", password: "x", followed: "Field" },
      { type: "generate_password", password: "x", followed: "field", extra: 1 },
      { type: "generate_password", password: "", followed: "field" },
      { type: "generator_options", options: OPTIONS, followed: "nope" },
      { type: "generator_options", followed: "site" },
    ]) {
      expect(result(bad), JSON.stringify(bad)).toBeNull();
    }
  });
});
