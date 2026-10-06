// Card field groups (spec 2026-09-29-card-autofill §5.1).
//
// Separate from the login and identity classifiers, which refuse any field
// cardKindOf claims: a card field only ever gets the card menu. Kinds of
// single fields live in card-kind.ts so those classifiers can ask without an
// import cycle. Bounded like login groups: at most MAX_GROUP_INPUTS elements
// per group.

import { MAX_CARD_ROLES, type CardRole } from "@havenkeys/protocol";
import { cardKindOf, TEXT_TYPES, type CardElement, type CardFieldKind } from "./card-kind";
import { groupRoot, MAX_GROUP_INPUTS, type Env } from "./group";

export { cardKindOf, type CardElement, type CardFieldKind } from "./card-kind";

export type CardShape = { kind: "whole" } | { kind: "slice"; start: number; length: number };

export interface CardField {
  el: CardElement;
  kind: CardFieldKind;
  shape: CardShape;
  confidence: number;
  byAutocomplete: boolean;
}

export interface CardGroup {
  root: ParentNode;
  fields: CardField[];
}

/** Visible, enabled, editable. The identity's stricter visibility applies: cards are not site-bound either. */
export function isCardFillable(el: CardElement, env: Env): boolean {
  if (el.disabled) return false;
  if (!(env.strictVisible ? env.strictVisible(el) : env.isVisible(el))) return false;
  if (el instanceof HTMLSelectElement) return true;
  return !el.readOnly && TEXT_TYPES.has(el.type.toLowerCase());
}

/** A number typed into 3–5 adjacent boxes of 4–6 characters: one slice each. */
function splitNumbers(fields: CardField[], els: CardElement[]): CardField[] {
  const first = fields.find((f) => f.kind === "number");
  if (!first || !(first.el instanceof HTMLInputElement)) return fields;
  const run: HTMLInputElement[] = [];
  for (let i = els.indexOf(first.el); i >= 0 && i < els.length && run.length < 5; i++) {
    const el = els[i];
    if (!(el instanceof HTMLInputElement) || el.maxLength < 4 || el.maxLength > 6) break;
    const known = fields.find((f) => f.el === el);
    if (known && known.kind !== "number") break;
    run.push(el);
  }
  const total = run.reduce((n, el) => n + el.maxLength, 0);
  if (run.length < 3 || total < 13 || total > 19) return fields;
  const slices = new Map<CardElement, CardField>();
  let start = 0;
  for (const el of run) {
    slices.set(el, { el, kind: "number", shape: { kind: "slice", start, length: el.maxLength }, confidence: first.confidence, byAutocomplete: first.byAutocomplete });
    start += el.maxLength;
  }
  const out: CardField[] = [];
  for (const el of els) {
    const f = slices.get(el) ?? fields.find((x) => x.el === el);
    if (f) out.push(f);
  }
  return out;
}

/** Every card field under `root`, in document order, bounded. */
export function classifyCard(root: ParentNode, env: Env): CardField[] {
  const els = Array.from(root.querySelectorAll<CardElement>("input, select"))
    .slice(0, MAX_GROUP_INPUTS * 4)
    .filter((el) => isCardFillable(el, env))
    .slice(0, MAX_GROUP_INPUTS);
  const first = els.map((el) => ({ el, r: cardKindOf(el, false) }));
  const strong = first.some((x) => x.r !== null && (x.r.kind === "number" || x.r.kind === "verificationNumber" || x.r.kind === "expiry"));
  const fields: CardField[] = [];
  for (const { el, r } of first) {
    const k = r ?? (strong ? cardKindOf(el, true) : null);
    if (k) fields.push({ el, kind: k.kind, shape: { kind: "whole" }, confidence: k.confidence, byAutocomplete: k.byAutocomplete });
  }
  return splitNumbers(fields, els);
}

/** A number field; or a CVV and an expiry; or (a processor's frame) fields named by cc-* autocomplete alone. */
function qualifies(fields: readonly CardField[]): boolean {
  const kinds = new Set(fields.map((f) => f.kind));
  if (kinds.has("number")) return true;
  if (kinds.has("verificationNumber") && (kinds.has("expiry") || kinds.has("expiryMonth"))) return true;
  return fields.length > 0 && fields.every((f) => f.byAutocomplete);
}

/** The card group around a field the user interacted with, if it qualifies. */
export function cardGroupFor(field: HTMLInputElement, env: Env): CardGroup | null {
  const root = groupRoot(field);
  const fields = classifyCard(root, env);
  if (!fields.some((f) => f.el === field) || !qualifies(fields)) return null;
  return { root, fields };
}

/** The first qualifying card group under `root` (a submit's form, the popup's page), bounded. */
export function findCardGroup(root: ParentNode, env: Env): CardGroup | null {
  const seen = new Set<ParentNode>();
  for (const input of Array.from(root.querySelectorAll<HTMLInputElement>("input")).slice(0, MAX_GROUP_INPUTS)) {
    const r = groupRoot(input);
    if (seen.has(r)) continue;
    seen.add(r);
    const fields = classifyCard(r, env);
    if (qualifies(fields)) return { root: r, fields };
  }
  return null;
}

/**
 * What a frame fills when the user picked a card elsewhere in the tab (or
 * from the popup): its first card group, else every card field of the page
 * (a processor frame holding only the expiry box).
 */
export function cardFieldsForFill(doc: Document, env: Env): CardGroup | null {
  const group = findCardGroup(doc, env);
  if (group) return group;
  const fields = classifyCard(doc, env);
  return fields.length > 0 ? { root: doc, fields } : null;
}

/** The roles the fields ask Rust for, unique, in document order. */
export function cardRolesOf(fields: readonly CardField[]): CardRole[] {
  const out: CardRole[] = [];
  const add = (r: CardRole) => {
    if (!out.includes(r)) out.push(r);
  };
  for (const f of fields) {
    if (f.kind === "expiry") {
      add("expiryMonth");
      add("expiryYear");
    } else add(f.kind);
  }
  return out.slice(0, MAX_CARD_ROLES);
}
