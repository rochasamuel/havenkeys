// Writing values into page fields.
//
// Values are set through the native `value` setter and announced with
// `input`/`change` events, which is what React, Vue and Angular listen for.
// Secrets only ever go into the `value` property of a visible, enabled
// field in the group the user chose: never into attributes, never into
// hidden fields, never anywhere else in the DOM.

import { OTP_MAX_LENGTH, OTP_MIN_LENGTH } from "./words.generated";
import { fieldsOf, isStrictlyFillable, type Env, type LoginGroup } from "./group";

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

/**
 * The user typed the field's current value, compared through `key`: a site
 * that reformats as the user types (a card mask adding spaces) keeps the
 * same digits. Page-script values never count.
 */
export function typedByUser(el: HTMLInputElement, key: (v: string) => string): boolean {
  const s = sources.get(el);
  return s !== undefined && s.source === "user" && key(s.value) === key(el.value);
}

const nativeValueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;

/**
 * Fill one field. Returns false (and writes nothing) if it is not fillable.
 * A password-type field must also pass the strict visibility check: off the
 * page, nearly transparent or clipped away, it gets nothing (EX-02).
 */
export function setValue(el: HTMLInputElement, value: string, env: Env, fromVault = true): boolean {
  if (!isStrictlyFillable(el, env)) return false;
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

/** Lets the page's framework apply the last event (React and Vue re-render in a microtask). */
const settle = () => new Promise<void>((r) => setTimeout(r, 0));

/** The page's own paste handling, which most split-code widgets have: the code as clipboard text. */
function paste(el: HTMLInputElement, code: string): void {
  if (typeof DataTransfer === "undefined" || typeof ClipboardEvent === "undefined") return;
  const data = new DataTransfer();
  data.setData("text/plain", code);
  el.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true, composed: true }));
}

/**
 * Fill a one-time code: one field, or one character per box of a split
 * field (4-8 boxes; any number of single-character ones). Widgets differ in
 * what they accept, so a split fill checks what the boxes hold and tries,
 * in turn: typing each box after the page settles from the last (widgets
 * that track the focused box or rebuild the code from their state), a
 * paste on the first box, the whole code in the first box. Resolves to the
 * number of fields that hold their part of the code.
 */
export function fillOtp(group: LoginGroup, code: string, env: Env): number | Promise<number> {
  const boxes = fieldsOf(group, "otp");
  const first = boxes[0];
  if (!first) return 0;
  const split =
    (boxes.length > 1 && boxes.every((b) => b.maxLength === 1)) ||
    (boxes.length >= OTP_MIN_LENGTH && boxes.length <= OTP_MAX_LENGTH);
  return split ? fillBoxes(boxes, code, env) : Number(setValue(first, code, env));
}

async function fillBoxes(boxes: HTMLInputElement[], code: string, env: Env): Promise<number> {
  const first = boxes[0] as HTMLInputElement;
  const parts = boxes.slice(0, code.length);
  const holding = () => parts.filter((b, i) => b.isConnected && b.value === code[i]).length;
  const type = async () => {
    for (const [i, b] of parts.entries()) {
      if (!b.isConnected || !isStrictlyFillable(b, env)) return;
      b.focus({ preventScroll: true });
      await settle();
      setValue(b, code[i] ?? "", env);
      await settle();
    }
  };
  const pasteFirst = async () => {
    if (!isStrictlyFillable(first, env)) return;
    first.focus({ preventScroll: true });
    await settle();
    paste(first, code);
  };
  // The whole code only goes into a box without a maxlength (one that could
  // hold it); if the widget leaves it there, typing again tidies the boxes.
  const whole = () => void setValue(first, code, env);
  const unbounded = first.maxLength === -1;
  for (const attempt of unbounded ? [type, pasteFirst, whole, type] : [type, pasteFirst]) {
    if (!first.isConnected) break;
    await attempt();
    await settle();
    if (holding() === parts.length) break;
  }
  return holding();
}
