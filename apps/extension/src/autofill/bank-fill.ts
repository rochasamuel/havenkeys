// Writing a bank account into a form (spec 2026-11-03-bank-account §5.2).
//
// Rust sends the numbers apart; each box gets them in its own shape. Only the
// form around the picked box is written, only boxes that are empty or hold
// what we wrote, each re-checked at write time by fill.ts. Nothing is cut to
// fit, logged or submitted.

import type { BankValues } from "@havenkeys/protocol";
import { classifyBank, type BankRole } from "./bank";
import { hash } from "./card-fill";
import { setValue } from "./fill";
import { classifyGroup, fieldsOf, groupRoot, type Env } from "./group";

export interface BankBox {
  type: string;
  /** The box's maxlength, null when it has none. */
  maxLength: number | null;
  /** The form has a separate box for this number's check digit. */
  hasDigitBox: boolean;
}

/** `value` and its check `digit` in one box: `NNNN-D`, `NNNND`, or the number alone. */
export function shapeBank(value: string, digit: string | null, box: BankBox): string {
  if (digit === null || box.hasDigitBox) return value;
  const room = (n: number) => box.maxLength === null || box.maxLength >= n;
  if (box.type !== "number" && room(value.length + 1 + digit.length)) return `${value}-${digit}`;
  return room(value.length + digit.length) ? `${value}${digit}` : value;
}

/** Our own writes, by fingerprint of the letters and digits: a site's mask keeps the same key. */
const written = new WeakMap<HTMLInputElement, string>();
const keyOf = (v: string) => hash(v.replace(/[^0-9A-Za-z]/g, ""));

function writable(el: HTMLInputElement): boolean {
  if (el.value === "") return true;
  return written.get(el) === keyOf(el.value);
}

/**
 * Fill the bank boxes of the form around `picked`, and that form's password
 * box. Returns how many boxes were written. Never submits.
 */
export function fillBank(root: ParentNode, picked: HTMLElement, values: BankValues, password: string | null, env: Env): number {
  const scope = picked instanceof HTMLInputElement ? groupRoot(picked) : root;
  const boxes = classifyBank(scope, env);
  const has = (role: BankRole) => boxes.some((b) => b.role === role);
  const valueFor = (role: BankRole, el: HTMLInputElement): string | null => {
    const shape = (v: string, d: string | null, digitRole: BankRole) =>
      shapeBank(v, d, { type: el.type, maxLength: el.maxLength < 0 ? null : el.maxLength, hasDigitBox: has(digitRole) });
    switch (role) {
      case "bankBranch":
        return shape(values.branch, values.branchDigit, "bankBranchDigit");
      case "bankAccount":
        return shape(values.account, values.accountDigit, "bankAccountDigit");
      case "bankBranchDigit":
        return values.branchDigit;
      case "bankAccountDigit":
        return values.accountDigit;
    }
  };
  const write = (el: HTMLInputElement, value: string | null): number => {
    // Never cut a value to fit, never replace what the page or the user put there.
    if (!value || (el.maxLength >= 0 && value.length > el.maxLength) || !writable(el)) return 0;
    if (!setValue(el, value, env)) return 0;
    written.set(el, keyOf(el.value));
    return 1;
  };
  let n = 0;
  for (const { el, role } of boxes) n += write(el, valueFor(role, el));
  const [pw] = fieldsOf(classifyGroup(scope, env), "current-password", "password");
  if (pw) n += write(pw, password);
  return n;
}
