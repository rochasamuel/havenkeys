// Identity field classification (spec 2026-09-29-identity-autofill §5.1).
//
// Separate from the login classifier: that one keeps treating these words as
// reasons *not* to fill a login. Pure reads of attributes compared against
// fixed keyword lists; nothing from the page is evaluated or inserted.
// Bounded like login groups: at most MAX_GROUP_INPUTS elements per group.

import { DOCUMENT_ROLES, MAX_IDENTITY_ROLES, type IdentityRole } from "@havenkeys/protocol";
import { cardKindOf } from "./card-kind";
import { groupRoot, MAX_GROUP_INPUTS, type Env } from "./group";
import { labelText } from "./labels";
import { hasAny, MAX_HINT_CHARS, normalize } from "./text";
import {
  IDENTITY_BIRTH as BIRTH,
  IDENTITY_PLACE as PLACE,
  IDENTITY_DOCUMENT as DOCUMENT,
  IDENTITY_ADDRESS as ADDRESS,
  IDENTITY_NEW as NEW,
  IDENTITY_NEGATIVE as NEGATIVE,
  IDENTITY_SPECIFIC_ADDRESS_ROLES,
  IDENTITY_WORDS_FULL_NAME,
  IDENTITY_WORDS_FIRST_NAME,
  IDENTITY_WORDS_MIDDLE_NAME,
  IDENTITY_WORDS_LAST_NAME,
  IDENTITY_WORDS_CPF,
  IDENTITY_WORDS_RG,
  IDENTITY_WORDS_PASSPORT,
  IDENTITY_WORDS_DRIVERS_LICENSE,
  IDENTITY_WORDS_BIRTH_DATE,
  IDENTITY_WORDS_EMAIL,
  IDENTITY_WORDS_PHONE,
  IDENTITY_WORDS_USERNAME,
  IDENTITY_WORDS_COMPANY,
  IDENTITY_WORDS_POSTAL_CODE,
  IDENTITY_WORDS_NEIGHBORHOOD,
  IDENTITY_WORDS_COMPLEMENT,
  IDENTITY_WORDS_NUMBER,
  IDENTITY_WORDS_STREET,
  IDENTITY_WORDS_CITY,
  IDENTITY_WORDS_STATE,
  IDENTITY_WORDS_COUNTRY,
  IDENTITY_WORDS_FULL_NAME_GENERIC,
  IDENTITY_THRESHOLD as THRESHOLD,
  IDENTITY_WORDS_ON_ATTRS as WORDS_ON_ATTRS,
  IDENTITY_WORDS_ON_TEXT as WORDS_ON_TEXT,
} from "./words.generated";

export type IdentityElement = HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;

export interface IdentityField {
  el: IdentityElement;
  role: IdentityRole;
  confidence: number;
  byAutocomplete: boolean;
}

export interface IdentityGroup {
  root: ParentNode;
  fields: IdentityField[];
}

/** Input types that can hold an identity value. */
const TEXT_TYPES = new Set(["text", "email", "tel", "number", "url", "date", ""]);

/** autocomplete token → role. Prefixes (section-*, shipping, …) are stripped first. */
const AUTOCOMPLETE: Record<string, IdentityRole> = {
  name: "fullName",
  "given-name": "firstName",
  "additional-name": "middleName",
  "family-name": "lastName",
  email: "email",
  tel: "phone",
  "tel-national": "phone",
  bday: "birthDate",
  "bday-day": "birthDay",
  "bday-month": "birthMonth",
  "bday-year": "birthYear",
  organization: "company",
  "street-address": "addressLine1",
  "address-line1": "addressLine1",
  "address-line2": "addressLine2",
  "address-level3": "neighborhood",
  "address-level2": "city",
  "address-level1": "state",
  "postal-code": "postalCode",
  country: "country",
  "country-name": "country",
  username: "username",
};
const AC_PREFIX = /^(section-\S+|shipping|billing|home|work|mobile|fax|pager)$/;

/** Words per role, English and Portuguese, normalized (no accents). Order matters: first hit wins. */
const WORDS: Array<[IdentityRole, readonly string[]]> = [
  ["fullName", IDENTITY_WORDS_FULL_NAME],
  ["firstName", IDENTITY_WORDS_FIRST_NAME],
  ["middleName", IDENTITY_WORDS_MIDDLE_NAME],
  ["lastName", IDENTITY_WORDS_LAST_NAME],
  ["cpf", IDENTITY_WORDS_CPF],
  ["rg", IDENTITY_WORDS_RG],
  ["passport", IDENTITY_WORDS_PASSPORT],
  ["driversLicense", IDENTITY_WORDS_DRIVERS_LICENSE],
  ["birthDate", IDENTITY_WORDS_BIRTH_DATE],
  ["email", IDENTITY_WORDS_EMAIL],
  ["phone", IDENTITY_WORDS_PHONE],
  ["username", IDENTITY_WORDS_USERNAME],
  ["company", IDENTITY_WORDS_COMPANY],
  ["postalCode", IDENTITY_WORDS_POSTAL_CODE],
  ["neighborhood", IDENTITY_WORDS_NEIGHBORHOOD],
  ["complement", IDENTITY_WORDS_COMPLEMENT],
  ["number", IDENTITY_WORDS_NUMBER],
  ["street", IDENTITY_WORDS_STREET],
  ["city", IDENTITY_WORDS_CITY],
  ["state", IDENTITY_WORDS_STATE],
  ["country", IDENTITY_WORDS_COUNTRY],
  ["fullName", IDENTITY_WORDS_FULL_NAME_GENERIC],
];



/**
 * A generic address in the name (`enderecoCidade`, `address[city]`) is
 * read as the street; the visible label naming a specific part (Cidade,
 * Estado, País…) wins over it.
 */
function labelledPart(found: IdentityRole, words: readonly string[], text: string): IdentityRole {
  if (found !== "street" || hasAny(text, words)) return found;
  return WORDS.find(([role, w]) => IDENTITY_SPECIFIC_ADDRESS_ROLES.includes(role) && hasAny(text, w))?.[0] ?? found;
}

/** Null when the words make a role match wrong; the role otherwise. */
function compound(role: IdentityRole, all: string): IdentityRole | null {
  // "Cidade/País de nascimento", "Country of birth": a birthplace, not a birth date, city or country.
  if (hasAny(all, BIRTH) && hasAny(all, PLACE)) return null;
  // "Número do documento", "Document number": not a house number.
  if (role === "number" && hasAny(all, DOCUMENT)) return null;
  // "Endereço da empresa", "Company address": the company's address, not a
  // company name; ambiguous as the user's street, so neither.
  if (role === "company" && hasAny(all, ADDRESS)) return null;
  // "New Yahoo email", "Novo e-mail": an address being chosen or changed
  // to, not the user's own.
  if (role === "email" && hasAny(all, NEW)) return null;
  return role;
}


function attr(el: Element, n: string): string {
  return (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
}

function autocompleteRole(el: IdentityElement): IdentityRole | null {
  const tokens = attr(el, "autocomplete").toLowerCase().split(/\s+/).filter(Boolean).filter((t) => !AC_PREFIX.test(t));
  const last = tokens[tokens.length - 1];
  return last ? (AUTOCOMPLETE[last] ?? null) : null;
}

function kindOf(el: IdentityElement): "input" | "select" | "textarea" {
  return el instanceof HTMLSelectElement ? "select" : el instanceof HTMLTextAreaElement ? "textarea" : "input";
}

/** The field's identity role, or null. */
export function identityRoleOf(el: IdentityElement): { role: IdentityRole; confidence: number; byAutocomplete: boolean } | null {
  const tag = kindOf(el);
  const type = tag === "input" ? (el as HTMLInputElement).type.toLowerCase() : tag;
  if (tag === "input" && !TEXT_TYPES.has(type)) return null;
  const ac = attr(el, "autocomplete").toLowerCase();
  // Password-type fields are excluded by type above. Checkouts put
  // new-password on address fields to keep the browser's autofill away, so
  // only card and one-time-code tokens refuse a text field here.
  if (/\b(cc-|one-time-code)/.test(ac)) return null;
  // Whatever the card classifier claims is a card field, not an identity one.
  if (!(el instanceof HTMLTextAreaElement) && cardKindOf(el, false)) return null;
  const attrs = normalize(`${attr(el, "name")} ${attr(el, "id")}`);
  const text = normalize(`${attr(el, "placeholder")} ${attr(el, "aria-label")} ${attr(el, "title")} ${labelText(el)}`, MAX_HINT_CHARS * 3);
  if (hasAny(`${attrs} ${text}`, NEGATIVE) || attr(el, "role") === "search" || el.closest('[role="search"]')) return null;

  // A date input only takes a birth date; a multi-line field only takes an address.
  const allowed = (role: IdentityRole): boolean =>
    (type !== "date" || role === "birthDate") && (tag !== "textarea" || role === "street" || role === "addressLine1");
  const fromAc = autocompleteRole(el);
  if (fromAc) return allowed(fromAc) ? { role: fromAc, confidence: 1, byAutocomplete: true } : null;
  if (type === "email") return { role: "email", confidence: 0.8, byAutocomplete: false };

  for (const [found, words] of WORDS) {
    const score = hasAny(attrs, words) ? WORDS_ON_ATTRS : hasAny(text, words) ? WORDS_ON_TEXT : 0;
    if (score < THRESHOLD) continue;
    const role = compound(labelledPart(found, words, text), `${attrs} ${text}`);
    if (!role || !allowed(role)) return null;
    // "Número para contato" on a tel box is the phone; a house number names the address.
    if (role === "number" && type === "tel" && !hasAny(`${attrs} ${text}`, ADDRESS)) {
      return { role: "phone", confidence: 0.8, byAutocomplete: false };
    }
    return { role: role === "street" && tag === "textarea" ? "addressLine1" : role, confidence: score / 100, byAutocomplete: false };
  }
  // Sites use type=tel for any digits (birth date, CEP, CPF): the words
  // decide first, and a tel box nothing names is a phone.
  if (type === "tel") return { role: "phone", confidence: 0.8, byAutocomplete: false };
  return null;
}

/** Visible, enabled, editable. */
export function isIdentityFillable(el: IdentityElement, env: Env): boolean {
  if (el.disabled) return false;
  if (!(env.strictVisible ? env.strictVisible(el) : env.isVisible(el))) return false;
  if (el instanceof HTMLSelectElement) return true;
  if (el.readOnly) return false;
  return !(el instanceof HTMLInputElement) || TEXT_TYPES.has(el.type.toLowerCase());
}

function classify(root: ParentNode, env: Env): IdentityField[] {
  const els = Array.from(root.querySelectorAll<IdentityElement>("input, select, textarea"))
    .slice(0, MAX_GROUP_INPUTS * 4)
    .filter((el) => isIdentityFillable(el, env))
    .slice(0, MAX_GROUP_INPUTS);
  const out: IdentityField[] = [];
  for (const el of els) {
    const r = identityRoleOf(el);
    if (r) out.push({ el, ...r });
  }
  return out;
}

/** Two identity fields, or one named by autocomplete that is not an email/username box. */
/**
 * Two identity fields, or one the page names for sure: by autocomplete
 * (not an email or username box: those are a login's first step), or a
 * document number by its words, as a lone CPF box beside a card form.
 */
function qualifies(fields: IdentityField[]): boolean {
  if (fields.length >= 2) return true;
  const [only] = fields;
  if (!only) return false;
  if (isDocumentRole(only.role)) return true;
  return only.byAutocomplete && only.role !== "email" && only.role !== "username";
}

/** The identity group around a field the user interacted with, if it qualifies. */
export function identityGroupFor(field: HTMLInputElement, env: Env): IdentityGroup | null {
  const root = groupRoot(field);
  const fields = classify(root, env);
  if (!fields.some((f) => f.el === field) || !qualifies(fields)) return null;
  return { root, fields };
}

/** The page's first qualifying group (popup fill), bounded. */
export function findIdentityGroup(doc: Document, env: Env): IdentityGroup | null {
  const candidates = Array.from(doc.querySelectorAll<HTMLInputElement>("input")).slice(0, MAX_GROUP_INPUTS);
  const seen = new Set<ParentNode>();
  for (const input of candidates) {
    const root = groupRoot(input);
    if (seen.has(root)) continue;
    seen.add(root);
    const fields = classify(root, env);
    if (qualifies(fields) && fields.length >= 2) return { root, fields };
  }
  return null;
}

/** The group's roles, unique, in document order. */
export function rolesOf(group: IdentityGroup): IdentityRole[] {
  const out: IdentityRole[] = [];
  const add = (r: IdentityRole) => {
    if (!out.includes(r)) out.push(r);
  };
  for (const f of group.fields) {
    add(f.role);
    // A multi-line address box takes street, number and complement on separate lines.
    if (f.role === "addressLine1" && f.el instanceof HTMLTextAreaElement) {
      add("street");
      add("number");
      add("complement");
    }
  }
  return out.slice(0, MAX_IDENTITY_ROLES);
}

export const isDocumentRole = (r: IdentityRole) => DOCUMENT_ROLES.includes(r);
