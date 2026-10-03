// Field classification: which inputs in a login group are the username,
// password, new password, confirmation and one-time-code fields.
//
// Pure functions over features extracted from the DOM (see group.ts), so
// every rule is unit-testable without a browser. No single heuristic
// decides: each field gets a score per role from several independent
// signals, and the group as a whole resolves the password roles.
// See docs/autofill.md for the signal table.

import { hasAny } from "./text";

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
// Matched as whole normalized words (see text.ts). English plus Portuguese
// and Spanish, the languages of the first users.

const USERNAME_STRONG = [
  "username",
  "user name",
  "userid",
  "user id",
  "login",
  "login id",
  "email",
  "e mail",
  "email address",
  "account",
  "account name",
  "identifier",
  "sign in",
  "usuario",
  "nome de usuario",
  "correo",
];
/**
 * National ID and document numbers that sites use as the login (gov.br's
 * CPF, Spain's DNI, Chile's RUT…). Counted like the strong words above.
 * Ambiguous short tokens (`id`, `ci`, `cc`: card fields) are left out.
 */
const USERNAME_DOCUMENT = [
  // Brazil
  "cpf",
  "cnpj",
  "rg",
  "documento",
  "numero do documento",
  "matricula",
  // Portugal, Spain, Latin America
  "nif",
  "nie",
  "dni",
  "cif",
  "rut",
  "cuit",
  "cuil",
  "curp",
  "rfc",
  "cedula",
  "documento de identidad",
  // Elsewhere
  "document number",
  "passport",
  "passaporte",
  "pasaporte",
  "national id",
  "codice fiscale",
];
const USERNAME_STRONG_ALL = [...USERNAME_STRONG, ...USERNAME_DOCUMENT];
const USERNAME_WEAK = ["user", "mail", "phone", "mobile", "telefone", "celular"];
const USERNAME_NEGATIVE = [
  "search",
  "query",
  "q",
  "coupon",
  "promo",
  "captcha",
  "newsletter",
  "subscribe",
  "zip",
  "postal",
  "city",
  "address",
  "street",
  "first name",
  "last name",
  "full name",
  "company",
  "code",
  "otp",
  "message",
  "comment",
  "amount",
  "quantity",
];

const OTP_STRONG = [
  "otp",
  "totp",
  "2fa",
  "mfa",
  "one time",
  "one time code",
  "one time password",
  "verification code",
  "security code",
  "auth code",
  "authentication code",
  "authenticator",
  "two factor",
  "2 step",
  "two step",
  "codigo de verificacao",
  "codigo de seguranca",
  "codigo de verificacion",
  "token",
];
const OTP_WEAK = ["code", "codigo", "pin", "verify", "verification"];
const OTP_NEGATIVE = [
  "postal code",
  "zip code",
  "promo code",
  "coupon",
  "discount",
  "country code",
  "area code",
  "captcha",
  "gift",
  "referral",
  "invite",
  "cvv",
  "cvc",
];

const PW_CONFIRM = [
  "confirm",
  "confirmation",
  "re enter",
  "reenter",
  "repeat",
  "retype",
  "again",
  "verify",
  "confirmar",
  "confirme",
  "confirmacao",
  "repita",
  "repetir",
];
const PW_NEW = ["new", "create", "choose", "set", "nova", "novo", "nueva", "criar", "crie"];
const PW_CURRENT = ["current", "old", "existing", "atual", "antiga", "actual"];

const INTENT_SIGNUP = [
  "sign up",
  "signup",
  "register",
  "registration",
  "create account",
  "create an account",
  "create your account",
  "join",
  "cadastro",
  "cadastrar",
  "cadastre se",
  "criar conta",
  "crear cuenta",
  "registrarse",
];
const INTENT_CHANGE = [
  "change password",
  "change your password",
  "reset password",
  "reset your password",
  "update password",
  "new password",
  "alterar senha",
  "redefinir senha",
  "nova senha",
  "cambiar contrasena",
];
const INTENT_LOGIN = [
  "sign in",
  "signin",
  "log in",
  "login",
  "log on",
  "entrar",
  "acessar",
  "iniciar sesion",
  "welcome back",
];

/** autocomplete tokens that mark a field as something other than a login. */
const AC_NOT_LOGIN = [
  "name",
  "given-name",
  "family-name",
  "additional-name",
  "nickname",
  "organization",
  "street-address",
  "address-line1",
  "address-line2",
  "address-level1",
  "address-level2",
  "postal-code",
  "country",
  "country-name",
  "bday",
  "bday-day",
  "bday-month",
  "bday-year",
  "sex",
  "url",
  "photo",
];

/**
 * An identity provider's button ("Sign in with Google", "Continuar com a
 * Apple"). Signup and sign-in pages both have them, so their words are not
 * intent evidence.
 */
const PROVIDER_BUTTON = [
  "sign in with", "signin with", "log in with", "login with", "sign up with", "signup with", "continue with",
  "entrar com", "acessar com", "continuar com", "cadastre se com", "cadastrar com", "inscreva se com",
  "iniciar sesion con", "registrate con", "registrarse con", "continuar con",
];

/** Named providers: "Sign in with a passkey" or "with email" is still a sign-in. */
const PROVIDER_NAMES = [
  "google", "apple", "microsoft", "facebook", "meta", "github", "gitlab", "twitter", "x", "linkedin",
  "amazon", "yahoo", "discord", "slack", "gov br", "paypal", "steam", "twitch",
];

export function isProviderButton(text: string): boolean {
  return hasAny(text, PROVIDER_BUTTON) && hasAny(text, PROVIDER_NAMES);
}

/**
 * Details a sign-in form never asks for: your name or birthday. A group with
 * a password and such a field is a signup. By autocomplete token this
 * outweighs any wording (Yahoo's signup says only "Sign in with Google" and
 * "Sign in instead"). By label it is weaker evidence (a library catalog's
 * sign-in asks for a last name and a PIN), so `classifyGroup` lets it decide
 * only when no heading, submit button or form name says sign in.
 */
const AC_SIGNUP_ONLY = ["name", "given-name", "family-name", "additional-name", "bday", "bday-day", "bday-month", "bday-year", "sex"];
/** Unambiguous label words only: "name"/"nome"/"nombre" alone also label usernames. */
const SIGNUP_ONLY_WORDS = [
  "first name", "firstname", "given name", "middle name", "last name", "lastname", "family name", "surname",
  "date of birth", "birth date", "birthdate", "birthday",
  "primeiro nome", "sobrenome", "data de nascimento",
  "apellido", "apellidos", "fecha de nacimiento",
];

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

export const USERNAME_THRESHOLD = 50;
/** Without a password field in the group, demand more (username-only step). */
export const USERNAME_ALONE_THRESHOLD = 70;
export const OTP_THRESHOLD = 60;
/**
 * A username-only step on a form that says it signs in, for a field whose
 * own wording names a username or document ("CPF" label, generic name).
 */
export const USERNAME_LOGIN_INTENT_BONUS = 30;

function isCardField(f: FieldFeatures): boolean {
  return f.autocomplete.some((t) => t.startsWith("cc-")) || hasAny(f.attrs, ["cc", "card", "cvv", "cvc"]);
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
  if (ac.includes("username")) s += 100;
  if (ac.includes("email")) s += 90;
  if (ac.includes("tel")) s += 30;
  if (f.type === "email") s += 80;
  let worded = true;
  if (hasAny(f.attrs, USERNAME_STRONG_ALL)) s += 50;
  else if (hasAny(f.attrs, USERNAME_WEAK)) s += 25;
  else worded = false;
  if (hasAny(f.text, USERNAME_STRONG_ALL)) {
    s += 40;
    worded = true;
  } else if (hasAny(f.text, USERNAME_WEAK)) {
    s += 20;
    worded = true;
  }
  if (hasAny(f.attrs, USERNAME_NEGATIVE) || hasAny(f.text, USERNAME_NEGATIVE)) s -= 80;
  if (ctx.lastBeforePassword) s += 40;
  if (!ctx.hasPassword && ctx.loginIntent && worded) s += USERNAME_LOGIN_INTENT_BONUS;
  return s;
}

export function otpScore(f: FieldFeatures, inSplitGroup: boolean): number {
  if (!TEXT_TYPES.has(f.type) && f.type !== "password") return 0;
  if (isCardField(f)) return 0;
  const words = `${f.attrs} ${f.text}`;
  if (hasAny(words, OTP_NEGATIVE)) return 0;
  let s = 0;
  if (f.autocomplete.includes("one-time-code")) s += 120;
  if (hasAny(words, OTP_STRONG)) s += 70;
  else if (hasAny(words, OTP_WEAK)) s += 30;
  if (f.maxLength >= 4 && f.maxLength <= 8) s += 15;
  if (f.inputMode === "numeric" || f.type === "tel" || f.type === "number") s += 10;
  if (inSplitGroup) s += 65;
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
