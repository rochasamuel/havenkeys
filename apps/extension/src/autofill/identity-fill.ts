// Writing identity values into a form (spec 2026-09-29-identity-autofill §5.4).
//
// Only the group the user picked from; only empty fields or fields whose
// value we wrote; each field re-checked just before writing. Values go into
// `value` / `selectedIndex` only, never attributes, and nothing is logged.

import type { IdentityRole, IdentityValue } from "@havenkeys/protocol";
import type { Env } from "./group";
import { isIdentityFillable, rolesOf, type IdentityElement, type IdentityGroup } from "./identity";
import { normalize } from "./text";

/** What we last wrote into each element: a value equal to it is ours to replace. */
const written = new WeakMap<IdentityElement, string>();

const setters = {
  input: Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set,
  textarea: Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set,
};

/** The 27 Brazilian states: UF code and name. */
const UF: ReadonlyArray<readonly [string, string]> = [
  ["AC", "Acre"], ["AL", "Alagoas"], ["AP", "Amapa"], ["AM", "Amazonas"], ["BA", "Bahia"],
  ["CE", "Ceara"], ["DF", "Distrito Federal"], ["ES", "Espirito Santo"], ["GO", "Goias"],
  ["MA", "Maranhao"], ["MT", "Mato Grosso"], ["MS", "Mato Grosso do Sul"], ["MG", "Minas Gerais"],
  ["PA", "Para"], ["PB", "Paraiba"], ["PR", "Parana"], ["PE", "Pernambuco"], ["PI", "Piaui"],
  ["RJ", "Rio de Janeiro"], ["RN", "Rio Grande do Norte"], ["RS", "Rio Grande do Sul"],
  ["RO", "Rondonia"], ["RR", "Roraima"], ["SC", "Santa Catarina"], ["SP", "Sao Paulo"],
  ["SE", "Sergipe"], ["TO", "Tocantins"],
];
const BRAZIL = ["br", "bra", "brasil", "brazil"];

/** Everything the value may be written as in an option, normalized. */
function aliases(role: IdentityRole, value: string): string[] {
  const v = normalize(value);
  const out = new Set([v]);
  if (role === "state") {
    for (const [code, name] of UF) {
      const c = normalize(code);
      const n = normalize(name);
      if (v === c || v === n) {
        out.add(c);
        out.add(n);
      }
    }
  }
  if (role === "country" && BRAZIL.includes(v)) for (const b of BRAZIL) out.add(b);
  return [...out];
}

/** The index of the option matching `value` by value or text, or −1. */
export function matchOption(select: HTMLSelectElement, role: IdentityRole, value: string): number {
  const wanted = aliases(role, value);
  const options = Array.from(select.options).slice(0, 500);
  return options.findIndex(
    (o) =>
      !o.disabled &&
      !(o.parentElement instanceof HTMLOptGroupElement && o.parentElement.disabled) &&
      (wanted.includes(normalize(o.value)) || wanted.includes(normalize(o.textContent ?? ""))),
  );
}

/**
 * A birth date (ISO, as Rust sends it) in the format a text field's
 * placeholder shows: dd/mm/aaaa (or yyyy), mm/dd/yyyy, yyyy-mm-dd, with
 * `/`, `-` or `.`. Null when the placeholder shows no format: the field is
 * skipped rather than guessed at.
 */
export function birthDateFor(placeholder: string, iso: string): string | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return null;
  const [, y, mo, d] = m as unknown as [string, string, string, string];
  const p = placeholder.slice(0, 200).toLowerCase();
  let f = /(?:^|[^a-z])dd([/.-])mm\1(?:aaaa|yyyy)(?![a-z])/.exec(p);
  if (f) return `${d}${f[1]}${mo}${f[1]}${y}`;
  f = /(?:^|[^a-z])mm([/.-])dd\1(?:aaaa|yyyy)(?![a-z])/.exec(p);
  if (f) return `${mo}${f[1]}${d}${f[1]}${y}`;
  f = /(?:^|[^a-z])(?:aaaa|yyyy)([/.-])mm\1dd(?![a-z])/.exec(p);
  if (f) return `${y}${f[1]}${mo}${f[1]}${d}`;
  return null;
}

function mine(el: IdentityElement): boolean {
  const current = el instanceof HTMLSelectElement ? String(el.selectedIndex) : el.value;
  return written.get(el) === current;
}

function isEmpty(el: IdentityElement): boolean {
  if (el instanceof HTMLSelectElement) return el.selectedIndex <= 0 || el.value === "";
  return el.value === "";
}

function announce(el: IdentityElement): void {
  el.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

/** A phone that fits the field's maxlength: as saved, then without +55. */
function phoneFor(el: HTMLInputElement, value: string): string | null {
  const max = el.maxLength;
  if (max < 0 || value.length <= max) return value;
  const national = value.replace(/^\+55\s*/, "");
  return national.length <= max ? national : null;
}

function write(el: IdentityElement, role: IdentityRole, value: string): boolean {
  if (el instanceof HTMLSelectElement) {
    const i = matchOption(el, role, value);
    if (i < 0) return false;
    el.selectedIndex = i;
    written.set(el, String(i));
    announce(el);
    return true;
  }
  let v = value;
  if (el instanceof HTMLInputElement) {
    if (el.type === "date") {
      if (!/^\d{4}-\d{2}-\d{2}$/.test(v)) return false;
    } else if (role === "birthDate") {
      const formatted = birthDateFor(el.getAttribute("placeholder") ?? "", v);
      if (formatted === null) return false;
      v = formatted;
    }
    if (role === "phone") {
      const fit = phoneFor(el, v);
      if (fit === null) return false;
      v = fit;
    } else if (el.maxLength >= 0 && v.length > el.maxLength) return false;
  }
  el.focus({ preventScroll: true });
  const setter = el instanceof HTMLTextAreaElement ? setters.textarea : setters.input;
  if (setter) setter.call(el, v);
  else el.value = v;
  written.set(el, v);
  announce(el);
  return true;
}

function fillableNow(el: IdentityElement, env: Env): boolean {
  return el.isConnected && isIdentityFillable(el, env) && (isEmpty(el) || mine(el));
}

/** A birth date text field without a format in its placeholder is never written: do not ask for it. */
function takesValue(f: { el: IdentityElement; role: IdentityRole }): boolean {
  if (f.role !== "birthDate" || !(f.el instanceof HTMLInputElement) || f.el.type === "date") return true;
  return birthDateFor(f.el.getAttribute("placeholder") ?? "", "2000-01-01") !== null;
}

/** The roles worth asking for: those of fields we could write right now. */
export function rolesToFill(group: IdentityGroup, env: Env): IdentityRole[] {
  return rolesOf({ root: group.root, fields: group.fields.filter((f) => takesValue(f) && fillableNow(f.el, env)) });
}

const ADDRESS_LINES: readonly IdentityRole[] = ["street", "number", "complement"];

/** What a field gets: a multi-line address box takes street, number and complement on separate lines. */
function valueFor(f: { el: IdentityElement; role: IdentityRole }, byRole: ReadonlyMap<IdentityRole, string>): string | undefined {
  if (f.role === "addressLine1" && f.el instanceof HTMLTextAreaElement) {
    const lines = ADDRESS_LINES.map((r) => byRole.get(r)).filter((v): v is string => !!v);
    if (lines.length > 0) return lines.join("\n");
  }
  return byRole.get(f.role);
}

/** Fill the group's fields from `values`. Returns how many were written. */
export function fillIdentity(group: IdentityGroup, values: readonly IdentityValue[], env: Env): number {
  const byRole = new Map(values.map((v) => [v.role, v.value] as const));
  let n = 0;
  for (const f of group.fields) {
    const value = valueFor(f, byRole);
    if (value === undefined || !fillableNow(f.el, env)) continue;
    if (write(f.el, f.role, value)) n++;
  }
  return n;
}
