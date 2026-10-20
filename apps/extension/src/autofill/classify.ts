// Field classification: which inputs in a login group are the username,
// password, new password, confirmation and one-time-code fields.
//
// Pure functions over features extracted from the DOM (see group.ts), so
// every rule is unit-testable without a browser. No single heuristic
// decides: each field gets a score per role from several independent
// signals, and the group as a whole resolves the password roles.
// See docs/autofill.md for the signal table.

import { hasAny } from "./text";
import {
  LOGIN_CARD_WORDS,
  USERNAME_STRONG,
  USERNAME_DOCUMENT,
  USERNAME_WEAK,
  USERNAME_NEGATIVE,
  OTP_STRONG,
  OTP_WEAK,
  OTP_NEGATIVE,
  PW_CONFIRM,
  PW_NEW,
  PW_CURRENT,
  INTENT_SIGNUP,
  INTENT_CHANGE,
  INTENT_LOGIN,
  AC_NOT_LOGIN,
  PROVIDER_BUTTON,
  PROVIDER_NAMES,
  AC_SIGNUP_ONLY,
  SIGNUP_ONLY_WORDS,
  USERNAME_THRESHOLD,
  USERNAME_ALONE_THRESHOLD,
  OTP_THRESHOLD,
  USERNAME_LOGIN_INTENT_BONUS,
  USERNAME_AC_USERNAME,
  USERNAME_AC_EMAIL,
  USERNAME_AC_TEL,
  USERNAME_EMAIL_INPUT,
  USERNAME_WORD_IN_ATTRS,
  USERNAME_WORD_IN_ATTRS_WEAK,
  USERNAME_WORD_IN_TEXT,
  USERNAME_WORD_IN_TEXT_WEAK,
  USERNAME_NEGATIVE_PENALTY,
  USERNAME_LAST_BEFORE_PASSWORD,
  OTP_AC,
  OTP_WORD,
  OTP_WORD_WEAK,
  OTP_LENGTH,
  OTP_NUMERIC,
  OTP_SPLIT_GROUP,
  OTP_MIN_LENGTH,
  OTP_MAX_LENGTH,
} from "./words.generated";

export type FieldClassification =
  | "username"
  | "password"
  | "current-password"
  | "new-password"
  | "confirmation-password"
  | "otp"
  | "unknown";

export interface FieldFeatures {
  /** `input.type`, lowercased. */
  type: string;
  /** `autocomplete` tokens, lowercased. */
  autocomplete: readonly string[];
  /** Normalized `name` and `id`. */
  attrs: string;
  /** Normalized placeholder, aria-label, title and label text. */
  text: string;
  /** `maxLength`, or -1 when unset. */
  maxLength: number;
  inputMode: string;
}

export interface Classified {
  kind: FieldClassification;
  /** 0..1. */
  confidence: number;
}

export type GroupIntent = "login" | "signup" | "change" | "unknown";

/** Input types that can hold a username or one-time code. */
const TEXT_TYPES = new Set(["text", "email", "tel", "number", "search", "url", ""]);

// ---------------------------------------------------------------- keywords
// The lists live in packages/field-words/words.json (shared with Android),
// matched as whole normalized words (see text.ts).

const USERNAME_STRONG_ALL = [...USERNAME_STRONG, ...USERNAME_DOCUMENT];







export function isProviderButton(text: string): boolean {
  return hasAny(text, PROVIDER_BUTTON) && hasAny(text, PROVIDER_NAMES);
}


export function signupDetailByAutocomplete(f: FieldFeatures): boolean {
  return f.autocomplete.some((t) => AC_SIGNUP_ONLY.includes(t));
}

export function signupDetailByWords(f: FieldFeatures): boolean {
  return f.type !== "password" && hasAny(`${f.attrs} ${f.text}`, SIGNUP_ONLY_WORDS);
}

/** Does this (primary) text say sign in? */
export function saysLogin(text: string): boolean {
  return hasAny(text, INTENT_LOGIN);
}

// ---------------------------------------------------------------- scores

export { USERNAME_THRESHOLD };
/** Without a password field in the group, demand more (username-only step). */
export { USERNAME_ALONE_THRESHOLD };
export { OTP_THRESHOLD };
/**
 * A username-only step on a form that says it signs in, for a field whose
 * own wording names a username or document ("CPF" label, generic name).
 */
export { USERNAME_LOGIN_INTENT_BONUS };

function isCardField(f: FieldFeatures): boolean {
  return f.autocomplete.some((t) => t.startsWith("cc-")) || hasAny(f.attrs, LOGIN_CARD_WORDS);
}

export interface UsernameContext {
  /** The group has a password field. */
  hasPassword: boolean;
  /** This is the last text field before the first password field. */
  lastBeforePassword: boolean;
  /** The group's intent is login. */
  loginIntent?: boolean;
}

export function usernameScore(f: FieldFeatures, ctx: UsernameContext): number {
  if (!TEXT_TYPES.has(f.type) || f.type === "search" || isCardField(f)) return 0;
  const ac = f.autocomplete;
  // `new-password` is not ruled out: sites put it on every field (gov.br's
  // CPF box, for one) to keep browsers' own autofill away.
  if (ac.includes("one-time-code") || ac.includes("current-password")) return 0;
  if (ac.some((t) => AC_NOT_LOGIN.includes(t))) return 0;

  let s = 0;
  if (ac.includes("username")) s += USERNAME_AC_USERNAME;
  if (ac.includes("email")) s += USERNAME_AC_EMAIL;
  if (ac.includes("tel")) s += USERNAME_AC_TEL;
  if (f.type === "email") s += USERNAME_EMAIL_INPUT;
  let worded = true;
  if (hasAny(f.attrs, USERNAME_STRONG_ALL)) s += USERNAME_WORD_IN_ATTRS;
  else if (hasAny(f.attrs, USERNAME_WEAK)) s += USERNAME_WORD_IN_ATTRS_WEAK;
  else worded = false;
  if (hasAny(f.text, USERNAME_STRONG_ALL)) {
    s += USERNAME_WORD_IN_TEXT;
    worded = true;
  } else if (hasAny(f.text, USERNAME_WEAK)) {
    s += USERNAME_WORD_IN_TEXT_WEAK;
    worded = true;
  }
  if (hasAny(f.attrs, USERNAME_NEGATIVE) || hasAny(f.text, USERNAME_NEGATIVE)) s -= USERNAME_NEGATIVE_PENALTY;
  if (ctx.lastBeforePassword) s += USERNAME_LAST_BEFORE_PASSWORD;
  if (!ctx.hasPassword && ctx.loginIntent && worded) s += USERNAME_LOGIN_INTENT_BONUS;
  return s;
}

export function otpScore(f: FieldFeatures, inSplitGroup: boolean): number {
  if (!TEXT_TYPES.has(f.type) && f.type !== "password") return 0;
  if (isCardField(f)) return 0;
  const words = `${f.attrs} ${f.text}`;
  if (hasAny(words, OTP_NEGATIVE)) return 0;
  let s = 0;
  if (f.autocomplete.includes("one-time-code")) s += OTP_AC;
  if (hasAny(words, OTP_STRONG)) s += OTP_WORD;
  else if (hasAny(words, OTP_WEAK)) s += OTP_WORD_WEAK;
  if (f.maxLength >= OTP_MIN_LENGTH && f.maxLength <= OTP_MAX_LENGTH) s += OTP_LENGTH;
  if (f.inputMode === "numeric" || f.type === "tel" || f.type === "number") s += OTP_NUMERIC;
  if (inSplitGroup) s += OTP_SPLIT_GROUP;
  return s;
}

/**
 * What the form is for. `primary` is the strongest evidence (submit button
 * text, headings, the form's own name and the page path); `secondary` is the
 * rest of the group's text, which often contains the opposite action ("No
 * account? Sign up").
 */
export function groupIntent(primary: string, secondary = ""): GroupIntent {
  const hits = (phrases: readonly string[]) =>
    phrases.filter((p) => hasAny(primary, [p])).length * 3 + phrases.filter((p) => hasAny(secondary, [p])).length;
  const scores: Array<[GroupIntent, number]> = [
    ["change", hits(INTENT_CHANGE) * 2],
    ["signup", hits(INTENT_SIGNUP)],
    ["login", hits(INTENT_LOGIN)],
  ];
  scores.sort((a, b) => b[1] - a[1]);
  const [best, second] = scores;
  if (!best || best[1] === 0 || (second && second[1] === best[1])) return "unknown";
  return best[0];
}

/**
 * Resolve the roles of a group's password fields (in document order).
 * Explicit autocomplete wins, then field wording, then position and the
 * group's intent.
 */
export function classifyPasswords(fields: readonly FieldFeatures[], intent: GroupIntent): Classified[] {
  const out: Classified[] = fields.map((f) => {
    const words = `${f.attrs} ${f.text}`;
    const ac = f.autocomplete;
    if (ac.includes("current-password")) return { kind: "current-password", confidence: 1 };
    if (ac.includes("new-password")) {
      if (hasAny(words, PW_CONFIRM)) return { kind: "confirmation-password", confidence: 0.9 };
      // Sites use `new-password` to switch off browser autofill; a field
      // that says it wants the current password ("senha atual") is one.
      if (hasAny(words, PW_CURRENT)) return { kind: "current-password", confidence: 0.8 };
      return { kind: "new-password", confidence: 0.95 };
    }
    if (hasAny(words, PW_CONFIRM)) return { kind: "confirmation-password", confidence: 0.8 };
    if (hasAny(words, PW_CURRENT)) return { kind: "current-password", confidence: 0.8 };
    if (hasAny(words, PW_NEW)) return { kind: "new-password", confidence: 0.8 };
    return { kind: "unknown", confidence: 0 };
  });

  // A second "new password" right after a new password is its confirmation.
  for (let i = 1; i < out.length; i++) {
    if (out[i]?.kind === "new-password" && out[i - 1]?.kind === "new-password") {
      out[i] = { kind: "confirmation-password", confidence: 0.7 };
    }
  }

  // Fill in what the wording did not decide, by position.
  const n = fields.length;
  out.forEach((c, i) => {
    if (c.kind !== "unknown") return;
    let kind: FieldClassification;
    if (n === 1) {
      kind = intent === "signup" || intent === "change" ? "new-password" : "password";
    } else if (n === 2) {
      if (intent === "login") kind = "password";
      else if (intent === "change" && !out.some((o) => o.kind === "confirmation-password")) {
        kind = i === 0 ? "current-password" : "new-password";
      } else kind = i === 0 ? "new-password" : "confirmation-password";
    } else {
      kind = i === 0 ? "current-password" : i === 1 ? "new-password" : "confirmation-password";
    }
    out[i] = { kind, confidence: 0.6 };
  });
  return out;
}

/** Confidence from a raw score. */
export function confidenceOf(score: number): number {
  return Math.max(0, Math.min(1, score / 120));
}

/** Roles that mean "fill an existing login here". */
export function isLoginRole(k: FieldClassification): boolean {
  return k === "username" || k === "password" || k === "current-password";
}

/** Roles that mean "the user is choosing a password here". */
export function isNewPasswordRole(k: FieldClassification): boolean {
  return k === "new-password" || k === "confirmation-password";
}
