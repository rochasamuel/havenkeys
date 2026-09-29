// Identity field classification (spec 2026-09-29-identity-autofill §5.1).
//
// Separate from the login classifier: that one keeps treating these words as
// reasons *not* to fill a login. Pure reads of attributes compared against
// fixed keyword lists; nothing from the page is evaluated or inserted.
// Bounded like login groups: at most MAX_GROUP_INPUTS elements per group.

import { DOCUMENT_ROLES, MAX_IDENTITY_ROLES, type IdentityRole } from "@havenkeys/protocol";
import { groupRoot, MAX_GROUP_INPUTS, type Env } from "./group";
import { hasAny, MAX_HINT_CHARS, normalize } from "./text";

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
const THRESHOLD = 60;

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
  ["fullName", ["nome completo", "full name", "your name", "seu nome"]],
  ["firstName", ["first name", "given name", "primeiro nome", "firstname", "fname"]],
  ["middleName", ["middle name", "nome do meio"]],
  ["lastName", ["last name", "surname", "family name", "sobrenome", "lastname", "lname"]],
  ["cpf", ["cpf"]],
  ["rg", ["rg", "carteira de identidade", "registro geral"]],
  ["passport", ["passport", "passaporte"]],
  ["driversLicense", ["cnh", "driver license", "drivers license", "driving licence", "carteira de motorista"]],
  ["birthDate", ["data de nascimento", "nascimento", "date of birth", "birth date", "birthday", "birthdate", "dob"]],
  ["email", ["email", "e mail"]],
  ["phone", ["celular", "telefone", "phone", "mobile", "tel", "whatsapp"]],
  ["username", ["username", "user name", "nome de usuario"]],
  ["company", ["empresa", "company", "organization", "organizacao", "nome fantasia", "razao social"]],
  ["postalCode", ["cep", "zip", "zip code", "zipcode", "postal", "postal code", "postcode"]],
  ["neighborhood", ["bairro", "neighborhood", "district"]],
  ["complement", ["complemento", "apt", "apartment", "suite"]],
  ["number", ["numero", "number", "house number", "num"]],
  ["street", ["logradouro", "rua", "endereco", "street", "address", "address line 1"]],
  ["city", ["cidade", "city", "municipio", "town"]],
  ["state", ["estado", "uf", "state", "province"]],
  ["country", ["pais", "country"]],
  ["fullName", ["nome", "name"]],
];

/** Wording that means "not the person's data" even when a role word matches. */
const NEGATIVE = ["search", "busca", "pesquisar", "coupon", "cupom", "promo", "cc", "card", "cartao", "cvv", "cvc", "captcha", "quantity", "quantidade",
  "order", "pedido", "account", "conta", "tracking", "rastreio", "invoice", "nota fiscal", "price", "preco", "mae", "mother", "titular", "holder", "unit"];

const WORDS_ON_ATTRS = 70;
const WORDS_ON_TEXT = 60;

function attr(el: Element, n: string): string {
  return (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
}

function labelText(el: IdentityElement): string {
  const parts: string[] = [];
  for (const l of Array.from(el.labels ?? []).slice(0, 2)) parts.push((l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  return parts.join(" ");
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
  if (/\b(cc-|one-time-code|current-password|new-password)/.test(ac)) return null;
  const attrs = normalize(`${attr(el, "name")} ${attr(el, "id")}`);
  const text = normalize(`${attr(el, "placeholder")} ${attr(el, "aria-label")} ${attr(el, "title")} ${labelText(el)}`, MAX_HINT_CHARS * 3);
  if (hasAny(`${attrs} ${text}`, NEGATIVE) || attr(el, "role") === "search" || el.closest('[role="search"]')) return null;

  // A date input only takes a birth date; a multi-line field only takes an address.
  const allowed = (role: IdentityRole): boolean =>
    (type !== "date" || role === "birthDate") && (tag !== "textarea" || role === "street" || role === "addressLine1");
  const fromAc = autocompleteRole(el);
  if (fromAc) return allowed(fromAc) ? { role: fromAc, confidence: 1, byAutocomplete: true } : null;
  if (type === "email") return { role: "email", confidence: 0.8, byAutocomplete: false };
  if (type === "tel") return { role: "phone", confidence: 0.8, byAutocomplete: false };

  for (const [role, words] of WORDS) {
    const score = hasAny(attrs, words) ? WORDS_ON_ATTRS : hasAny(text, words) ? WORDS_ON_TEXT : 0;
    if (score < THRESHOLD) continue;
    if (!allowed(role)) return null;
    return { role: role === "street" && tag === "textarea" ? "addressLine1" : role, confidence: score / 100, byAutocomplete: false };
  }
  return null;
}

/** Visible, enabled, editable. */
export function isIdentityFillable(el: IdentityElement, env: Env): boolean {
  if (el.disabled) return false;
  if (!(env.identityVisible ? env.identityVisible(el) : env.isVisible(el))) return false;
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
function qualifies(fields: IdentityField[]): boolean {
  if (fields.length >= 2) return true;
  const [only] = fields;
  return only !== undefined && only.byAutocomplete && only.role !== "email" && only.role !== "username";
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
