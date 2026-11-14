// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { MAX_PASSWORD_RULES_BYTES } from "@havenkeys/protocol";
import { parseContentRequest } from "../messaging/inline";
import { readFieldRules } from "./password-rules";

function field(attrs: Record<string, string>): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "password";
  for (const [k, v] of Object.entries(attrs)) input.setAttribute(k, v);
  return input;
}

describe("readFieldRules", () => {
  it("is null for a field that asks nothing", () => {
    expect(readFieldRules(field({}))).toBeNull();
    expect(readFieldRules(field({ passwordrules: "   " }))).toBeNull();
  });

  it("reads passwordrules, minlength and maxlength", () => {
    const rules = "minlength: 8; maxlength: 16; required: lower, upper; required: digit; allowed: [-().&@?'#,/\"+]; max-consecutive: 2;";
    expect(readFieldRules(field({ passwordrules: ` ${rules} `, minlength: "8", maxlength: "16" }))).toEqual({ rules, minLength: 8, maxLength: 16 });
    expect(readFieldRules(field({ maxlength: "12" }))).toEqual({ maxLength: 12 });
    expect(readFieldRules(field({ minlength: "10" }))).toEqual({ minLength: 10 });
  });

  it("drops what a page could use to send something out of bounds", () => {
    // Over the protocol's bound: dropped whole, never cut.
    expect(readFieldRules(field({ passwordrules: "x".repeat(MAX_PASSWORD_RULES_BYTES + 1) }))).toBeNull();
    expect(readFieldRules(field({ passwordrules: "é".repeat(MAX_PASSWORD_RULES_BYTES / 2 + 1) }))).toBeNull();
    // Lengths: invalid, zero, negative or huge say nothing.
    for (const v of ["", "abc", "0", "-5", "1025", "99999999999"]) {
      expect(readFieldRules(field({ minlength: v, maxlength: v })), v).toBeNull();
    }
    expect(readFieldRules(field({ maxlength: "1024" }))).toEqual({ maxLength: 1024 });
    // The browser's own reading of the attribute, which is also what it enforces.
    expect(readFieldRules(field({ maxlength: "12.5" }))).toEqual({ maxLength: 12 });
  });

  it("produces what the background's parser accepts", () => {
    const passwordRules = readFieldRules(field({ passwordrules: "required: digit;", maxlength: "6" }))!;
    expect(parseContentRequest({ type: "cs_open_menu", kind: "new_password", passwordRules })).toEqual({
      type: "cs_open_menu",
      kind: "new_password",
      passwordRules,
    });
  });
});
