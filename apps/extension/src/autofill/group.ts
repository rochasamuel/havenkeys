// Login groups: the set of inputs that belong together around the field the
// user interacted with, classified as a whole.
//
// Work is bounded and lazy. Nothing scans the page up front: a group is
// built only for a field the user clicked or focused, and it examines at
// most MAX_GROUP_INPUTS inputs, so a page with thousands of inputs costs the
// same as a small one (threat-model attack 7). The DOM is untrusted: its
// strings are only compared against keyword lists.

import {
  classifyPasswords,
  confidenceOf,
  groupIntent,
  otpScore,
  OTP_THRESHOLD,
  USERNAME_ALONE_THRESHOLD,
  USERNAME_THRESHOLD,
  usernameScore,
  type FieldClassification,
  type FieldFeatures,
  type GroupIntent,
} from "./classify";
import { MAX_HINT_CHARS, normalize } from "./text";

/** Inputs examined per group. */
export const MAX_GROUP_INPUTS = 60;
/** Ancestor levels climbed when a field has no <form>. */
const MAX_CLIMB = 8;
/** A <form> with more inputs than this is a page wrapper (ASP.NET style), not a login form. */
const MAX_FORM_INPUTS = 40;
/** Elements read for the group's intent text. */
const MAX_INTENT_ELEMENTS = 24;

/** Input types that never take part in a login. */
const IGNORED_TYPES = new Set([
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

export interface Env {
  /** Is the element rendered and visible to the user? */
  isVisible(el: HTMLElement): boolean;
  /** Page path, used as intent evidence (`/signup`, `/login`). */
  path: string;
}

export function defaultEnv(): Env {
  return { isVisible: isRendered, path: location.pathname };
}

/** Visible and usable: rendered with a size, not hidden by CSS. */
export function isRendered(el: HTMLElement): boolean {
  if (!el.isConnected) return false;
  const check = (el as HTMLElement & { checkVisibility?: (o: object) => boolean }).checkVisibility;
  if (check && !check.call(el, { opacityProperty: true, visibilityProperty: true, contentVisibilityAuto: true })) {
    return false;
  }
  const r = el.getBoundingClientRect();
  return r.width >= 4 && r.height >= 4;
}

export function isFillable(el: HTMLInputElement, env: Env): boolean {
  return !el.disabled && !el.readOnly && !IGNORED_TYPES.has(el.type) && env.isVisible(el);
}

/** Text of the elements referenced by `aria-labelledby`, bounded. */
function labelledBy(el: HTMLElement): string {
  const ids = (el.getAttribute("aria-labelledby") ?? "").split(/\s+/).slice(0, 4);
  const root = el.getRootNode() as Document | ShadowRoot;
  return ids
    .map((id) => (id && "getElementById" in root ? root.getElementById(id)?.textContent ?? "" : ""))
    .join(" ")
    .slice(0, MAX_HINT_CHARS);
}

function labelText(el: HTMLInputElement): string {
  const parts: string[] = [];
  for (const l of Array.from(el.labels ?? []).slice(0, 2)) parts.push((l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  parts.push(labelledBy(el));
  return parts.join(" ");
}

export function features(el: HTMLInputElement): FieldFeatures {
  const attr = (n: string) => (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
  return {
    type: el.type.toLowerCase(),
    autocomplete: attr("autocomplete").toLowerCase().split(/\s+/).filter(Boolean),
    attrs: normalize(`${attr("name")} ${attr("id")}`),
    text: normalize(`${attr("placeholder")} ${attr("aria-label")} ${attr("title")} ${labelText(el)}`, MAX_HINT_CHARS * 3),
    maxLength: el.maxLength,
    inputMode: attr("inputmode").toLowerCase(),
  };
}

/** Inputs under `root`, in document order, capped. */
function inputsIn(root: ParentNode): HTMLInputElement[] {
  return Array.from(root.querySelectorAll("input")).slice(0, MAX_GROUP_INPUTS * 4);
}

/**
 * The element that holds `field`'s login group: its <form> when that is a
 * plausible login form, otherwise the nearest ancestor that also contains
 * another usable input.
 */
export function groupRoot(field: HTMLInputElement): ParentNode {
  const form = field.form;
  if (form && form.querySelectorAll("input").length <= MAX_FORM_INPUTS) return form;
  let node: HTMLElement = field;
  for (let i = 0; i < MAX_CLIMB; i++) {
    const parent = node.parentElement;
    if (!parent) break;
    const inputs = parent.querySelectorAll("input");
    if (inputs.length > MAX_FORM_INPUTS) break;
    node = parent;
    // Stop at the first container holding another usable input: a username
    // next to a password, a password next to a username, or OTP boxes.
    const textish = Array.from(inputs).filter((i) => !IGNORED_TYPES.has(i.type)).length;
    if (textish >= 2) break;
  }
  return node === field ? (field.getRootNode() as ParentNode) : node;
}

export interface ClassifiedField {
  el: HTMLInputElement;
  kind: FieldClassification;
  confidence: number;
}

export interface LoginGroup {
  root: ParentNode;
  intent: GroupIntent;
  fields: ClassifiedField[];
}

function intentText(root: ParentNode, env: Env): { primary: string; secondary: string } {
  const primary: string[] = [env.path];
  const secondary: string[] = [];
  if (root instanceof HTMLFormElement) {
    primary.push(root.getAttribute("name") ?? "", root.id, root.getAttribute("action") ?? "");
  }
  const els = Array.from(
    root.querySelectorAll('h1, h2, h3, legend, button, input[type="submit"], [role="button"], a'),
  ).slice(0, MAX_INTENT_ELEMENTS);
  for (const el of els) {
    const text =
      el instanceof HTMLInputElement ? el.value : (el.textContent ?? "") + " " + (el.getAttribute("aria-label") ?? "");
    const isPrimary =
      /^H[1-3]$|^LEGEND$/.test(el.tagName) ||
      (el instanceof HTMLButtonElement && el.type === "submit") ||
      (el instanceof HTMLInputElement && el.type === "submit");
    (isPrimary ? primary : secondary).push(text.slice(0, MAX_HINT_CHARS));
  }
  const join = (parts: string[]) => normalize(parts.join(" "), MAX_HINT_CHARS * 4);
  return { primary: join(primary), secondary: join(secondary) };
}

/** 4-8 adjacent single-character inputs: a split one-time-code field. */
function splitOtpRun(inputs: HTMLInputElement[]): Set<HTMLInputElement> {
  const run: HTMLInputElement[] = [];
  let best: HTMLInputElement[] = [];
  for (const i of inputs) {
    if (i.maxLength === 1 && i.type !== "password") {
      run.push(i);
      if (run.length > best.length) best = [...run];
    } else run.length = 0;
  }
  return new Set(best.length >= 4 && best.length <= 8 ? best : []);
}

/** Classify every usable input of the group around `root`. */
export function classifyGroup(root: ParentNode, env: Env): LoginGroup {
  const inputs = inputsIn(root)
    .filter((el) => isFillable(el, env))
    .slice(0, MAX_GROUP_INPUTS);
  const { primary, secondary } = intentText(root, env);
  const intent = groupIntent(primary, secondary);
  const feats = new Map(inputs.map((el) => [el, features(el)]));
  const kinds = new Map<HTMLInputElement, { kind: FieldClassification; confidence: number }>();

  // Passwords (by type), resolved as a set.
  const passwords = inputs.filter((el) => el.type === "password");
  const pwFeats = passwords.map((el) => feats.get(el) as FieldFeatures);
  const split = splitOtpRun(inputs);

  // A password-typed one-time code ("enter the 6-digit code") is an OTP.
  const realPasswords = passwords.filter((_el, i) => otpScore(pwFeats[i] as FieldFeatures, false) < OTP_THRESHOLD + 40);
  classifyPasswords(
    realPasswords.map((el) => feats.get(el) as FieldFeatures),
    intent,
  ).forEach((c, i) => kinds.set(realPasswords[i] as HTMLInputElement, c));

  // One-time codes.
  for (const el of inputs) {
    if (kinds.has(el)) continue;
    const s = otpScore(feats.get(el) as FieldFeatures, split.has(el));
    if (s >= OTP_THRESHOLD) kinds.set(el, { kind: "otp", confidence: confidenceOf(s) });
  }

  // At most one username: the best-scoring text field before the first password.
  const firstPw = realPasswords[0];
  const before = inputs.filter(
    (el) =>
      !kinds.has(el) &&
      el.type !== "password" &&
      (!firstPw || (el.compareDocumentPosition(firstPw) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0),
  );
  const last = before[before.length - 1];
  const threshold = firstPw ? USERNAME_THRESHOLD : USERNAME_ALONE_THRESHOLD;
  let best: { el: HTMLInputElement; score: number } | null = null;
  for (const el of before) {
    const score = usernameScore(feats.get(el) as FieldFeatures, {
      hasPassword: firstPw !== undefined,
      lastBeforePassword: firstPw !== undefined && el === last,
    });
    if (score >= threshold && (!best || score > best.score)) best = { el, score };
  }
  // A signup/change form's username is still worth knowing (for saving), but
  // login fills only happen for groups that have a login role.
  if (best) kinds.set(best.el, { kind: "username", confidence: confidenceOf(best.score) });

  const fields: ClassifiedField[] = inputs.map((el) => ({
    el,
    ...(kinds.get(el) ?? { kind: "unknown" as const, confidence: 0 }),
  }));
  return { root, intent, fields };
}

/** The group around a field the user interacted with, and that field's role. */
export function groupFor(field: HTMLInputElement, env: Env): { group: LoginGroup; kind: FieldClassification } {
  const group = classifyGroup(groupRoot(field), env);
  const kind = group.fields.find((f) => f.el === field)?.kind ?? "unknown";
  return { group, kind };
}

export function fieldsOf(group: LoginGroup, ...kinds: FieldClassification[]): HTMLInputElement[] {
  return group.fields.filter((f) => kinds.includes(f.kind)).map((f) => f.el);
}
