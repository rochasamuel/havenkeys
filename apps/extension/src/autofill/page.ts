// Page-level helpers: finding a login form without a focused field (fill
// from the toolbar popup) and reading what the user submitted.

import { valueSource } from "./fill";
import { classifyGroup, fieldsOf, groupRoot, isFillable, type Env, type LoginGroup } from "./group";
import type { FieldClassification } from "./classify";

/** Candidates examined when looking for a login form on the whole page. */
const MAX_PAGE_CANDIDATES = 200;

/**
 * `groupFor`, but reusing one classification per distinct group root across
 * the whole scan. Without this, a page with no small containing element
 * (thousands of unrelated inputs directly under <body>) makes every
 * candidate resolve to the same page-wide root, and re-classifying it from
 * scratch per candidate turns a bounded 200-candidate scan into hundreds of
 * full-document passes (threat-model attack 7).
 */
function groupForCached(
  field: HTMLInputElement,
  env: Env,
  cache: Map<ParentNode, LoginGroup>,
): { group: LoginGroup; kind: FieldClassification } {
  const root = groupRoot(field);
  let group = cache.get(root);
  if (!group) {
    group = classifyGroup(root, env);
    cache.set(root, group);
  }
  const kind = group.fields.find((f) => f.el === field)?.kind ?? "unknown";
  return { group, kind };
}

/**
 * The login group to fill when the user picked a login in the toolbar
 * popup: the first visible password field's group, else the first visible
 * username-only group. Bounded like everything else.
 */
export function findLoginGroup(doc: Document, env: Env): LoginGroup | null {
  const cache = new Map<ParentNode, LoginGroup>();
  const passwords = Array.from(doc.querySelectorAll<HTMLInputElement>('input[type="password"]')).slice(
    0,
    MAX_PAGE_CANDIDATES,
  );
  for (const pw of passwords) {
    if (!isFillable(pw, env)) continue;
    const { group, kind } = groupForCached(pw, env, cache);
    if (kind === "password" || kind === "current-password") return group;
  }
  const texts = Array.from(
    doc.querySelectorAll<HTMLInputElement>('input[type="email"], input[type="text"], input:not([type])'),
  ).slice(0, MAX_PAGE_CANDIDATES);
  for (const el of texts) {
    if (!isFillable(el, env)) continue;
    const { group, kind } = groupForCached(el, env, cache);
    if (kind === "username") return group;
  }
  return null;
}

/** The first visible one-time-code group on the page. */
export function findOtpGroup(doc: Document, env: Env): LoginGroup | null {
  const cache = new Map<ParentNode, LoginGroup>();
  const texts = Array.from(doc.querySelectorAll<HTMLInputElement>("input")).slice(0, MAX_PAGE_CANDIDATES);
  for (const el of texts) {
    if (el.type === "password" || !isFillable(el, env)) continue;
    const { group, kind } = groupForCached(el, env, cache);
    if (kind === "otp") return group;
  }
  return null;
}

export const MAX_USERNAME_CHARS = 512;
export const MAX_PASSWORD_CHARS = 4096;

export interface Submission {
  username: string | null;
  /** Null for a username-only step. */
  password: string | null;
}

/**
 * What the user submitted in `group`, if it is worth offering to save.
 *
 * * A new password wins over the current one (signup, change password),
 *   and is dropped if its confirmation does not match.
 * * Only a password the user typed, or one we generated, counts. One we
 *   filled from the vault is not new, and one a page script put there is
 *   not the user's (see fill.ts).
 * * Over-long values are ignored rather than truncated.
 */
export function readSubmission(group: LoginGroup): Submission | null {
  const [userEl] = fieldsOf(group, "username");
  const username = userEl?.value.trim() || null;
  if (username !== null && username.length > MAX_USERNAME_CHARS) return null;

  const [newEl] = fieldsOf(group, "new-password");
  const confirms = fieldsOf(group, "confirmation-password");
  const [currentEl] = fieldsOf(group, "current-password", "password");

  let pwEl: HTMLInputElement | undefined;
  if (newEl && newEl.value) {
    if (confirms.some((c) => c.value !== newEl.value)) return null;
    pwEl = newEl;
  } else if (currentEl && currentEl.value) {
    pwEl = currentEl;
  }

  if (!pwEl) {
    // A username-only step. Password fields that exist but are empty mean
    // the user did not actually submit a login.
    if (newEl || currentEl || username === null) return null;
    return { username, password: null };
  }
  const source = valueSource(pwEl);
  if (source !== "user" && source !== "generated") return null;
  const password = pwEl.value;
  if (password.length > MAX_PASSWORD_CHARS) return null;
  return { username, password };
}
