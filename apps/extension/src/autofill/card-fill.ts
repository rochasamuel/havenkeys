// Writing card values into a checkout (spec 2026-09-29-card-autofill §5.4),
// and reading a card the user typed (§5.6).
//
// Only the group the user picked from (or, for other frames of the tab,
// their card fields); only empty fields or fields whose value we wrote; each
// field re-checked just before writing. Values go into `value` /
// `selectedIndex` only, never attributes, and nothing is logged. Rust sends
// whole values; the shapes (MM/YY, slices, option text) are made here.

import { luhnOk, type CardRole, type CardValue } from "@havenkeys/protocol";
import type { Env } from "./group";
import { cardRolesOf, isCardFillable, type CardElement, type CardField, type CardGroup } from "./card";
import { typedByUser } from "./fill";
import { normalize } from "./text";

/** What we last wrote into each element: a value equal to it is ours to replace. */
const written = new WeakMap<CardElement, string>();
const inputSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;

/** Month names and abbreviations, English and Portuguese, normalized; index 0 = January. */
const MONTHS: ReadonlyArray<readonly string[]> = [
  ["january", "jan", "janeiro"],
  ["february", "feb", "fevereiro", "fev"],
  ["march", "mar", "marco"],
  ["april", "apr", "abril", "abr"],
  ["may", "maio", "mai"],
  ["june", "jun", "junho"],
  ["july", "jul", "julho"],
  ["august", "aug", "agosto", "ago"],
  ["september", "sep", "sept", "setembro", "set"],
  ["october", "oct", "outubro", "out"],
  ["november", "nov", "novembro"],
  ["december", "dec", "dezembro", "dez"],
];

/** Every way a brand select may name a brand, normalized. */
const BRAND_ALIASES: Record<string, readonly string[]> = {
  visa: ["visa", "vi"],
  mastercard: ["mastercard", "master card", "master", "mc"],
  amex: ["amex", "american express", "ax"],
  elo: ["elo"],
  hipercard: ["hipercard", "hiper", "hc"],
  diners: ["diners", "diners club", "dc"],
  discover: ["discover", "di"],
  jcb: ["jcb"],
  unionpay: ["unionpay", "union pay", "cup"],
  maestro: ["maestro"],
  other: [],
};

const hint = (el: Element) => `${el.getAttribute("placeholder") ?? ""}`.slice(0, 200).toLowerCase();

/**
 * The shape a `pattern` such as `\d{2}\s?/\s?\d{2}` or `[0-9]{4}` asks for:
 * the separator ("" for none) and whether the year has 4 digits. Simple
 * digit patterns only; anything else gives null.
 */
function patternShape(el: HTMLInputElement): { sep: string; long: boolean } | null {
  const raw = (el.getAttribute("pattern") ?? "").slice(0, 100);
  const flat = raw
    .replace(/^\^|\$$/g, "")
    .replace(/\\s\??/g, "")
    .replace(/\\([/.-])/g, "$1")
    .replace(/(?:\\d|\[0-9\])\{(\d)\}/g, (_, n: string) => "d".repeat(Number(n)))
    .replace(/\\d|\[0-9\]/g, "d");
  const m = /^(dd)([/.-]?)(dd|dddd)$/.exec(flat);
  return m ? { sep: m[2] ?? "", long: (m[3] ?? "").length === 4 } : null;
}

/** Month and year in one field: the placeholder's format, else the pattern's, else by maxlength, else MM/YY. */
export function expiryFor(el: HTMLInputElement, month: string, year: string): string {
  const mm = month.padStart(2, "0");
  const yy = year.slice(-2);
  const fits = (s: string) => el.maxLength < 0 || s.length <= el.maxLength;
  const m = /mm(\s*[/.-]\s*|)(yyyy|aaaa|yy|aa)(?![a-z])/.exec(hint(el));
  if (m) {
    const s = `${mm}${m[1] ?? ""}${(m[2] ?? "").length === 4 ? year : yy}`;
    if (fits(s)) return s;
  } else {
    const p = patternShape(el);
    if (p) {
      const s = `${mm}${p.sep}${p.long ? year : yy}`;
      if (fits(s)) return s;
    }
  }
  switch (el.maxLength) {
    case 4:
      return `${mm}${yy}`;
    case 7:
      return `${mm}/${year}`;
    default:
      return `${mm}/${yy}`;
  }
}

function monthText(el: HTMLInputElement, month: string): string {
  return el.maxLength === 2 || /\bmm\b/.test(hint(el)) ? month.padStart(2, "0") : month;
}

function yearText(el: HTMLInputElement, year: string): string {
  return el.maxLength === 2 || /^\s*(yy|aa)\s*$/.test(hint(el)) ? year.slice(-2) : year;
}

/** Digits, grouped with spaces when the placeholder shows groups (Amex 4-6-5). */
function numberText(el: HTMLInputElement, digits: string): string {
  const grouped = /\d{4}\s\d|[x•*]{4}\s[x•*]/i.test(hint(el));
  if (!grouped) return digits;
  const amex = digits.length === 15 && /^3[47]/.test(digits);
  const parts = amex ? [digits.slice(0, 4), digits.slice(4, 10), digits.slice(10)] : (digits.match(/.{1,4}/g) ?? [digits]);
  const spaced = parts.join(" ");
  return el.maxLength < 0 || spaced.length <= el.maxLength ? spaced : digits;
}

/** The index of the option matching `value` by value or text, or −1. Never a disabled or placeholder option. */
export function matchCardOption(select: HTMLSelectElement, kind: "expiryMonth" | "expiryYear" | "brand", value: string): number {
  const wanted = new Set<string>();
  if (kind === "expiryMonth") {
    const n = Number(value);
    wanted.add(String(n));
    wanted.add(String(n).padStart(2, "0"));
    for (const name of MONTHS[n - 1] ?? []) wanted.add(name);
  } else if (kind === "expiryYear") {
    wanted.add(value);
    wanted.add(value.slice(-2));
  } else {
    for (const a of BRAND_ALIASES[value] ?? []) wanted.add(normalize(a));
  }
  const matches = (raw: string): boolean => {
    const t = normalize(raw);
    if (t === "") return false;
    if (wanted.has(t)) return true;
    // "03 - Março": every word names the same month.
    return kind === "expiryMonth" && t.split(" ").every((w) => wanted.has(w));
  };
  return Array.from(select.options)
    .slice(0, 500)
    .findIndex(
      (o) => !o.disabled && !(o.parentElement instanceof HTMLOptGroupElement && o.parentElement.disabled) && (matches(o.value) || matches(o.textContent ?? "")),
    );
}

/** What a field gets from the values, in its shape, or undefined. */
function valueFor(f: CardField, byRole: ReadonlyMap<CardRole, string>): string | undefined {
  const el = f.el;
  const month = byRole.get("expiryMonth");
  const year = byRole.get("expiryYear");
  const text = el instanceof HTMLInputElement ? el : null;
  switch (f.kind) {
    case "expiry":
      return text && month && year ? expiryFor(text, month, year) : undefined;
    case "expiryMonth":
      return month && text ? monthText(text, month) : month;
    case "expiryYear":
      return year && text ? yearText(text, year) : year;
    case "number": {
      const digits = byRole.get("number");
      if (!digits || !text) return undefined;
      if (f.shape.kind === "slice") return digits.slice(f.shape.start, f.shape.start + f.shape.length) || undefined;
      return numberText(text, digits);
    }
    default:
      return byRole.get(f.kind);
  }
}

/** Compare-through key: card numbers, codes and expiries by digits, so a site's mask keeps the same key. */
function keyOf(kind: CardField["kind"], v: string): string {
  switch (kind) {
    case "number":
    case "verificationNumber":
    case "expiry":
      return v.replace(/\D/g, "");
    case "expiryMonth": {
      const d = v.replace(/\D/g, "");
      return d === "" ? v : String(Number(d));
    }
    case "expiryYear": {
      const d = v.replace(/\D/g, "");
      return d === "" ? v : d.slice(-2);
    }
    default:
      return v;
  }
}

/** A short non-cryptographic hash (cyrb53): only to recognise our own value again, not to protect it. */
function hash(s: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 2654435761);
    h2 = Math.imul(h2 ^ c, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return `${s.length}:${(4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(36)}`;
}

function fingerprint(el: CardElement, kind: CardField["kind"]): string {
  return hash(el instanceof HTMLSelectElement ? `#${el.selectedIndex}` : keyOf(kind, el.value));
}

/** Does this option name a real month, year or brand (not "Month", "Ano", "Select")? */
function optionMeansSomething(o: HTMLOptionElement, kind: CardField["kind"]): boolean {
  const texts = [o.value, o.textContent ?? ""];
  if (kind === "brand") {
    return texts.some((t) => Object.values(BRAND_ALIASES).some((a) => a.includes(normalize(t))));
  }
  return texts.some((t) => /\d/.test(t) || monthOf(t) !== null);
}

function announce(el: CardElement): void {
  el.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

function write(f: CardField, value: string): boolean {
  const el = f.el;
  if (el instanceof HTMLSelectElement) {
    if (f.kind !== "expiryMonth" && f.kind !== "expiryYear" && f.kind !== "brand") return false;
    const i = matchCardOption(el, f.kind, value);
    if (i < 0) return false;
    el.selectedIndex = i;
    announce(el);
    remember(f);
    return true;
  }
  // Never cut a value to fit: a wrong card number or code is worse than an empty field.
  if (el.maxLength >= 0 && value.length > el.maxLength) return false;
  el.focus({ preventScroll: true });
  if (inputSetter) inputSetter.call(el, value);
  else el.value = value;
  announce(el);
  // After announcing: a site's mask may reformat the value in its input handler.
  remember(f);
  return true;
}

/** Keeps only a fingerprint, so the plaintext is never held longer than `el.value` holds it. */
function remember(f: CardField): void {
  written.set(f.el, fingerprint(f.el, f.kind));
}

function mine(f: CardField): boolean {
  const known = written.get(f.el);
  if (known === undefined) return false;
  if (known === fingerprint(f.el, f.kind)) return true;
  written.delete(f.el);
  return false;
}

/** Nothing there, a placeholder option, or a value the site itself pre-selected. */
function isEmpty(f: CardField): boolean {
  const el = f.el;
  if (el instanceof HTMLSelectElement) {
    const o = el.options[el.selectedIndex];
    if (!o || el.value === "" || o.disabled || o.defaultSelected) return true;
    return el.selectedIndex === 0 && !optionMeansSomething(o, f.kind);
  }
  return el.value === "";
}

function fillableNow(f: CardField, env: Env): boolean {
  const el = f.el;
  if (!el.isConnected || !isCardFillable(el, env)) return false;
  if (isEmpty(f)) {
    written.delete(el);
    return true;
  }
  return mine(f);
}

/** The roles worth asking for: those of fields we could write right now. */
export function cardRolesToFill(group: CardGroup, env: Env): CardRole[] {
  return cardRolesOf(group.fields.filter((f) => fillableNow(f, env)));
}

/** Fill the group's fields from `values`. Returns how many were written. */
export function fillCard(group: CardGroup, values: readonly CardValue[], env: Env): number {
  const byRole = new Map(values.map((v) => [v.role, v.value] as const));
  let n = 0;
  for (const f of group.fields) {
    const value = valueFor(f, byRole);
    if (value === undefined || !fillableNow(f, env)) continue;
    if (write(f, value)) n++;
  }
  return n;
}

// ------------------------------------------------------------ a typed card

export interface SubmittedCard {
  number: string;
  /** YYYY-MM. */
  expiry: string;
  verificationNumber: string | null;
  cardholderName: string | null;
}

const digitsOf = (v: string) => v.replace(/[\s.-]/g, "");

function monthOf(raw: string): number | null {
  const t = normalize(raw);
  if (/^\d{1,2}$/.test(t)) return Number(t);
  const i = MONTHS.findIndex((names) => t.split(" ").some((w) => names.includes(w)));
  return i >= 0 ? i + 1 : null;
}

function yearOf(raw: string): number | null {
  const t = raw.trim();
  if (/^\d{4}$/.test(t)) return Number(t);
  if (/^\d{2}$/.test(t)) return 2000 + Number(t);
  return null;
}

function selectedText(el: CardElement, kind: CardField["kind"]): string {
  if (el instanceof HTMLSelectElement) {
    const o = el.options[el.selectedIndex];
    if (!o || o.disabled) return "";
    const text = o.textContent ?? "";
    // A month named in words wins over a 0-based value ("3" for April).
    if (kind === "expiryMonth" && /[a-z]/i.test(text) && monthOf(text) !== null) return text;
    return o.value || text;
  }
  return el.value;
}

function readExpiry(group: CardGroup): string | null {
  const one = group.fields.find((f) => f.kind === "expiry");
  let month: number | null = null;
  let year: number | null = null;
  if (one) {
    const m = /^\s*(\d{1,2})\s*[/.-]?\s*(\d{2}|\d{4})\s*$/.exec(one.el.value);
    if (m) {
      month = Number(m[1]);
      year = yearOf(m[2] ?? "");
    }
  } else {
    const m = group.fields.find((f) => f.kind === "expiryMonth");
    const y = group.fields.find((f) => f.kind === "expiryYear");
    month = m ? monthOf(selectedText(m.el, "expiryMonth")) : null;
    year = y ? yearOf(selectedText(y.el, "expiryYear")) : null;
  }
  if (month === null || year === null || month < 1 || month > 12 || year < 2000 || year > 2099) return null;
  return `${year}-${String(month).padStart(2, "0")}`;
}

/**
 * A card the user typed in `group`, worth offering to save. Only a number
 * the user typed counts (never one we filled or a page script wrote),
 * compared by digits so a site's mask does not hide it.
 */
export function readCardSubmission(group: CardGroup): SubmittedCard | null {
  const numberFields = group.fields.filter((f): f is CardField & { el: HTMLInputElement } => f.kind === "number" && f.el instanceof HTMLInputElement);
  if (numberFields.length === 0 || !numberFields.every((f) => typedByUser(f.el, digitsOf))) return null;
  const number = numberFields.map((f) => digitsOf(f.el.value)).join("");
  if (!/^[0-9]{12,19}$/.test(number) || !luhnOk(number)) return null;
  const expiry = readExpiry(group);
  if (!expiry) return null;
  const cvv = group.fields.find((f) => f.kind === "verificationNumber")?.el.value.trim() ?? "";
  const value = (kind: CardField["kind"]) => group.fields.find((f) => f.kind === kind)?.el.value.trim() ?? "";
  const name = value("cardholderName") || `${value("cardholderGivenName")} ${value("cardholderFamilyName")}`.trim();
  return {
    number,
    expiry,
    verificationNumber: /^[0-9]{3,8}$/.test(cvv) ? cvv : null,
    cardholderName: name.length > 0 && name.length <= 256 ? name : null,
  };
}
