// Writing values into page fields.
//
// Values are set through the native `value` setter and announced with
// `input`/`change` events, which is what React, Vue and Angular listen for.
// Secrets only ever go into the `value` property of a visible, enabled
// field in the group the user chose: never into attributes, never into
// hidden fields, never anywhere else in the DOM.

import { fieldsOf, isFillable, type Env, type LoginGroup } from "./group";

/**
 * Where each field's current value came from, as far as we know:
 * * "vault": we filled it from the vault (not a new credential);
 * * "generated": we filled a generated password (must be offered for saving);
 * * "user": the user typed or pasted into it (trusted input events only).
 * Fields a page script filled have no entry. Only "user" and "generated"
 * passwords are ever reported for saving, so a page cannot plant values,
 * forge a submit and learn from the save prompt whether it guessed a saved
 * password.
 */
export type ValueSource = "vault" | "generated" | "user";
// The value is kept alongside so a later change by page script (which fires
// no trusted event) is detected. It is the value already in the page's own
// field, so this holds nothing the page does not.
const sources = new WeakMap<HTMLInputElement, { source: ValueSource; value: string }>();

/** Where the field's current value came from; undefined if page script changed it since. */
export function valueSource(el: HTMLInputElement): ValueSource | undefined {
  const s = sources.get(el);
  return s && s.value === el.value ? s.source : undefined;
}

/** A trusted user edit makes the field the user's. */
export function markUserEdit(el: HTMLInputElement): void {
  sources.set(el, { source: "user", value: el.value });
}

const nativeValueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;

/** Fill one field. Returns false (and writes nothing) if it is not fillable. */
export function setValue(el: HTMLInputElement, value: string, env: Env, fromVault = true): boolean {
  if (!isFillable(el, env)) return false;
  el.focus({ preventScroll: true });
  if (nativeValueSetter) nativeValueSetter.call(el, value);
  else el.value = value;
  el.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
  sources.set(el, { source: fromVault ? "vault" : "generated", value });
  return true;
}

export interface LoginValues {
  username: string | null;
  password: string | null;
}

/**
 * Fill a login into the group: the username field and the (current)
 * password field, whichever exist. Returns how many fields were filled.
 */
export function fillLogin(group: LoginGroup, values: LoginValues, env: Env): number {
  let n = 0;
  const [user] = fieldsOf(group, "username");
  const [pw] = fieldsOf(group, "current-password", "password");
  if (user && values.username) n += Number(setValue(user, values.username, env));
  if (pw && values.password) n += Number(setValue(pw, values.password, env));
  return n;
}

/** Fill a generated password into the new-password and confirmation fields. */
export function fillNewPassword(group: LoginGroup, password: string, env: Env): number {
  let n = 0;
  for (const el of fieldsOf(group, "new-password", "confirmation-password")) n += Number(setValue(el, password, env, false));
  return n;
}

/** Fill a one-time code: one field, or one digit per box of a split field. */
export function fillOtp(group: LoginGroup, code: string, env: Env): number {
  const boxes = fieldsOf(group, "otp");
  const first = boxes[0];
  if (!first) return 0;
  if (boxes.length > 1 && boxes.every((b) => b.maxLength === 1)) {
    let n = 0;
    boxes.slice(0, code.length).forEach((b, i) => (n += Number(setValue(b, code[i] ?? "", env))));
    return n;
  }
  return Number(setValue(first, code, env));
}
