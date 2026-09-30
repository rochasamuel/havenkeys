// The card kind of a single field (spec 2026-09-29-card-autofill §5.1).
//
// Kept apart from card.ts (groups) so the login and identity classifiers can
// refuse card fields by asking this module, with no import cycle. Pure reads
// of attributes compared against fixed keyword lists; nothing from the page
// is evaluated or inserted.

import type { CardRole } from "@havenkeys/protocol";
import { hasAny, MAX_HINT_CHARS, normalize } from "./text";

export type CardElement = HTMLInputElement | HTMLSelectElement;
/** A role, or month and year together in one field. */
export type CardFieldKind = CardRole | "expiry";

/** Input types that can hold a card value. `password`: some sites hide the CVV. */
export const TEXT_TYPES = new Set(["text", "tel", "number", "password", ""]);

const AUTOCOMPLETE: Record<string, CardFieldKind> = {
  "cc-name": "cardholderName",
  "cc-given-name": "cardholderGivenName",
  "cc-family-name": "cardholderFamilyName",
  "cc-number": "number",
  "cc-csc": "verificationNumber",
  "cc-exp": "expiry",
  "cc-exp-month": "expiryMonth",
  "cc-exp-year": "expiryYear",
  "cc-type": "brand",
};
const AC_PREFIX = /^(section-\S+|shipping|billing)$/;

/** Words per kind, English and Portuguese, normalized. Order matters: first hit wins. */
const WORDS: Array<[CardFieldKind, readonly string[]]> = [
  ["verificationNumber", ["cvv", "cvc", "csc", "cvv2", "cvc2", "cid", "security code", "codigo de seguranca", "cod seguranca", "card code", "card verification"]],
  ["cardholderName", ["name on card", "nome impresso", "nome impresso no cartao", "nome no cartao", "nome do titular", "titular", "cardholder", "card holder", "holder name"]],
  ["number", ["card number", "numero do cartao", "numero cartao", "cardnumber", "ccnumber", "cc number", "credit card", "cartao de credito", "card no"]],
  ["expiryMonth", ["exp month", "expiry month", "expiration month", "mes de validade", "mes validade", "mes de vencimento"]],
  ["expiryYear", ["exp year", "expiry year", "expiration year", "ano de validade", "ano validade", "ano de vencimento"]],
  ["expiry", ["expiry", "expiration", "exp date", "expiry date", "expiration date", "valid thru", "validade", "vencimento", "data de validade", "mm aa", "mm yy"]],
  ["brand", ["card type", "bandeira", "card brand"]],
];

/**
 * Phrases that also name fields of a login or identity form (a 2FA "security
 * code", a customer "cid", a bare "titular"). They claim a field only inside
 * a group that already has a number, CVV-word or expiry field (strong).
 */
const AMBIGUOUS = new Set(["cid", "security code", "codigo de seguranca", "cod seguranca", "card code", "titular", "cardholder", "card holder", "holder name", "nome do titular"]);

/** Expiry words also appear in birth-date placeholders ("DD/MM/AA"). */
const NOT_AN_EXPIRY = ["dd", "dia", "day", "nascimento", "birth", "bday", "birthday"];

/** Weak words: only in a group that already has a number, CVV or expiry field. */
const MONTH_WORDS = ["month", "mes", "mm"];
const YEAR_WORDS = ["year", "ano", "yy", "yyyy", "aa", "aaaa"];
const CODE_WORDS = ["code", "codigo", "cod"];

/** Wording that means "not a payment card" (or not a card field: installments). */
const NEGATIVE = [
  "gift card", "cartao presente", "vale presente", "loyalty", "fidelidade", "coupon", "cupom", "promo", "search", "busca", "pesquisar",
  "parcela", "parcelas", "installment", "installments", "vezes", "quantidade",
];

/** A cardholder-name field that also asks for one of these is something else. */
const NOT_A_NAME = ["cpf", "documento", "nascimento", "birth", "email", "e mail", "telefone", "phone"];

const MONTH_NAMES = ["jan", "feb", "fev", "mar", "apr", "abr", "may", "mai", "jun", "jul", "aug", "ago", "sep", "set", "oct", "out", "nov", "dec", "dez"];

function attr(el: Element, n: string): string {
  return (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
}

function labelText(el: CardElement): string {
  const parts: string[] = [];
  for (const l of Array.from(el.labels ?? []).slice(0, 2)) parts.push((l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  return parts.join(" ");
}

function autocompleteKind(el: CardElement): CardFieldKind | null {
  const tokens = attr(el, "autocomplete").toLowerCase().split(/\s+/).filter(Boolean).filter((t) => !AC_PREFIX.test(t));
  const last = tokens[tokens.length - 1];
  return last ? (AUTOCOMPLETE[last] ?? null) : null;
}

/** The words an option shows; its value when it shows nothing. */
function optionWords(o: HTMLOptionElement): string[] {
  const t = normalize(o.textContent ?? "");
  return (t || normalize(o.value)).split(" ").filter(Boolean);
}

/** Exactly 12 month options plus at most one placeholder. */
function isMonthSelect(s: HTMLSelectElement): boolean {
  const opts = Array.from(s.options).slice(0, 40);
  const months = opts.filter((o) => {
    const w = optionWords(o);
    return (
      w.length > 0 &&
      w.length <= 3 &&
      !w.some((x) => /^\d+[a-z]+$/.test(x)) &&
      w.some((x) => /^(0?[1-9]|1[0-2])$/.test(x) || (x.length >= 3 && MONTH_NAMES.some((m) => x.startsWith(m))))
    );
  });
  return months.length === 12 && opts.length <= 13;
}

/** At least 10 consecutive years (2 or 4 digits) plus at most one placeholder. */
function isYearSelect(s: HTMLSelectElement): boolean {
  const years = new Set<number>();
  let other = 0;
  for (const o of Array.from(s.options).slice(0, 120)) {
    const w = optionWords(o);
    const y = w.length === 1 ? /^(?:20)?(\d{2})$/.exec(w[0] as string) : null;
    // 2-digit years start at 20: "01".."12" is a month list, not a year list.
    if (y && Number(y[1]) >= 20) years.add(2000 + Number(y[1]));
    else other++;
  }
  if (other > 1) return false;
  const sorted = [...years].sort((a, b) => a - b);
  let run = 1;
  let best = sorted.length ? 1 : 0;
  for (let i = 1; i < sorted.length; i++) {
    run = sorted[i] === (sorted[i - 1] as number) + 1 ? run + 1 : 1;
    best = Math.max(best, run);
  }
  return best >= 10;
}

/** The field's card kind, or null. `strong`: the group has a number, CVV or expiry field. */
export function cardKindOf(el: CardElement, strong: boolean): { kind: CardFieldKind; confidence: number; byAutocomplete: boolean } | null {
  const select = el instanceof HTMLSelectElement;
  const type = select ? "select" : (el as HTMLInputElement).type.toLowerCase();
  if (!select && !TEXT_TYPES.has(type)) return null;
  if (attr(el, "role") === "search" || el.closest('[role="search"]')) return null;
  const attrs = normalize(`${attr(el, "name")} ${attr(el, "id")}`);
  const text = normalize(`${attr(el, "placeholder")} ${attr(el, "aria-label")} ${attr(el, "title")} ${labelText(el)}`, MAX_HINT_CHARS * 3);
  const all = `${attrs} ${text}`;
  if (hasAny(all, NEGATIVE)) return null;
  // A one-time code is never a card field.
  if (attr(el, "autocomplete").toLowerCase().split(/\s+/).includes("one-time-code")) return null;

  // A password box only ever holds the CVV; a select holds month, year or brand.
  const allowed = (k: CardFieldKind): boolean =>
    (type !== "password" || k === "verificationNumber") && (!select || k === "expiryMonth" || k === "expiryYear" || k === "brand");

  const fromAc = autocompleteKind(el);
  if (fromAc) return allowed(fromAc) ? { kind: fromAc, confidence: 1, byAutocomplete: true } : null;
  for (const [kind, all_words] of WORDS) {
    const words = strong ? all_words : all_words.filter((w) => !AMBIGUOUS.has(w));
    const confidence = hasAny(attrs, words) ? 0.7 : hasAny(text, words) ? 0.6 : 0;
    if (confidence === 0) continue;
    if (kind === "cardholderName" && hasAny(all, NOT_A_NAME)) return null;
    if (kind === "expiry" && hasAny(all, NOT_AN_EXPIRY)) return null;
    return allowed(kind) ? { kind, confidence, byAutocomplete: false } : null;
  }
  if (!strong) return null;
  if (select) {
    const s = el as HTMLSelectElement;
    if (isMonthSelect(s)) return { kind: "expiryMonth", confidence: 0.5, byAutocomplete: false };
    if (isYearSelect(s)) return { kind: "expiryYear", confidence: 0.5, byAutocomplete: false };
  }
  if (hasAny(all, MONTH_WORDS) && allowed("expiryMonth")) return { kind: "expiryMonth", confidence: 0.5, byAutocomplete: false };
  if (hasAny(all, YEAR_WORDS) && allowed("expiryYear")) return { kind: "expiryYear", confidence: 0.5, byAutocomplete: false };
  const max = select ? -1 : (el as HTMLInputElement).maxLength;
  if (max >= 3 && max <= 4 && hasAny(all, CODE_WORDS)) return { kind: "verificationNumber", confidence: 0.5, byAutocomplete: false };
  return null;
}
