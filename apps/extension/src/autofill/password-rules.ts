// What a new-password field says about the password it wants
// (spec 2026-11-14-password-rules-generator): Apple's `passwordrules`
// attribute and the standard `minlength`/`maxlength`. All of it is page
// content, so it is bounded here, again by the background's parser, and in
// Rust; it only ever shapes the password generated for this same field.

import { isPasswordFieldRules, MAX_FIELD_LENGTH, MAX_PASSWORD_RULES_BYTES, type PasswordFieldRules } from "@havenkeys/protocol";

const utf8 = new TextEncoder();

/** A whole number in 1..MAX_FIELD_LENGTH, else nothing (absent, invalid, or saying nothing we could use). */
function fieldLength(n: number): number | undefined {
  return Number.isInteger(n) && n >= 1 && n <= MAX_FIELD_LENGTH ? n : undefined;
}

/**
 * The field's rules, or null when it declares none. A `passwordrules` value
 * over the protocol's bound is dropped whole (cut short, it could say
 * something else); Rust parses what is left.
 */
export function readFieldRules(field: HTMLInputElement): PasswordFieldRules | null {
  const out: PasswordFieldRules = {};
  const raw = field.getAttribute("passwordrules")?.trim();
  if (raw && utf8.encode(raw).length <= MAX_PASSWORD_RULES_BYTES) out.rules = raw;
  // The IDL values: -1 when absent or not a valid non-negative integer.
  const min = fieldLength(field.minLength);
  const max = fieldLength(field.maxLength);
  if (min !== undefined) out.minLength = min;
  if (max !== undefined) out.maxLength = max;
  return Object.keys(out).length > 0 && isPasswordFieldRules(out) ? out : null;
}
