// Finding a picked field again after the page re-rendered it.
//
// Frameworks (VTEX's React inputs on americanas.com.br) may swap the
// `<input>` the user opened the menu on for a fresh copy while the menu is
// open: same attributes, new element. The menu's fill then finds its field
// gone. The replacement is accepted only when it is unambiguous: exactly one
// fillable input with the same identifying attributes that was not already
// in the page when the menu was picked from. Attributes are only compared.

import { isFillable, type Env } from "../autofill/group";

/** Inputs examined when looking for twins or a replacement. */
const MAX_INPUTS = 200;
const KEY_ATTRS = ["name", "id", "autocomplete", "placeholder", "aria-label"] as const;

/** The attributes that identify an input across re-renders; null when it has none to go by. */
export function twinKey(el: HTMLInputElement): string | null {
  const parts = KEY_ATTRS.map((a) => (el.getAttribute(a) ?? "").slice(0, 200));
  if (parts.every((p) => p === "")) return null;
  return JSON.stringify([el.type, ...parts]);
}

function inputsOf(root: Node): HTMLInputElement[] {
  if (!(root instanceof Document || root instanceof ShadowRoot)) return [];
  return Array.from(root.querySelectorAll("input")).slice(0, MAX_INPUTS);
}

/** What a later replacement for `field` is looked for against: its root and the inputs it already shares a key with. */
export interface Twins {
  root: Node;
  key: string | null;
  present: WeakSet<HTMLInputElement>;
}

export function twinsOf(field: HTMLInputElement): Twins {
  const root = field.getRootNode();
  const key = twinKey(field);
  const present = new WeakSet<HTMLInputElement>();
  if (key !== null) for (const i of inputsOf(root)) if (twinKey(i) === key) present.add(i);
  return { root, key, present };
}

/** The one input that replaced `field` since `twins` was taken, or null. */
export function replacementFor(twins: Twins, env: Env): HTMLInputElement | null {
  if (twins.key === null || !twins.root.isConnected) return null;
  const fresh = inputsOf(twins.root).filter((i) => !twins.present.has(i) && twinKey(i) === twins.key && isFillable(i, env));
  return fresh.length === 1 ? (fresh[0] ?? null) : null;
}
