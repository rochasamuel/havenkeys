// The inputs a login or bank classifier looks at, filtered one way so the bank
// menu and the login refusal never disagree. Type imports only: group.ts and
// bank.ts both import this module.

import { cardKindOf } from "./card-kind";
import type { Env } from "./group";

/** Inputs examined per group. */
export const MAX_GROUP_INPUTS = 60;

/** Input types that never take part in a login. */
export const IGNORED_TYPES = new Set([
  "hidden",
  "submit",
  "button",
  "reset",
  "image",
  "file",
  "checkbox",
  "radio",
  "range",
  "color",
  "date",
  "datetime-local",
  "month",
  "week",
  "time",
]);

export function isFillable(el: HTMLInputElement, env: Env): boolean {
  return !el.disabled && !el.readOnly && !IGNORED_TYPES.has(el.type) && env.isVisible(el);
}

/** Fillable inputs under `root` that are not card boxes, in document order, capped. */
export function usableInputs(root: ParentNode, env: Env): HTMLInputElement[] {
  return Array.from(root.querySelectorAll("input"))
    .slice(0, MAX_GROUP_INPUTS * 4)
    .filter((el) => isFillable(el, env) && !cardKindOf(el, false))
    .slice(0, MAX_GROUP_INPUTS);
}
