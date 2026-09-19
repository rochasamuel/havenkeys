// Page-level helpers: finding a login form without a focused field (fill
// from the toolbar popup) and reading what the user submitted.

import { valueSource } from "./fill";
import { fieldsOf, groupFor, isFillable, type Env, type LoginGroup } from "./group";

/** Candidates examined when looking for a login form on the whole page. */
const MAX_PAGE_CANDIDATES = 200;

/**
 * The login group to fill when the user picked a login in the toolbar
 * popup: the first visible password field's group, else the first visible
 * username-only group. Bounded like everything else.
 */
export function findLoginGroup(doc: Document, env: Env): LoginGroup | null {
  const passwords = Array.from(doc.querySelectorAll<HTMLInputElement>('input[type="password"]')).slice(
    0,
    MAX_PAGE_CANDIDATES,
  );
  for (const pw of passwords) {
    if (!isFillable(pw, env)) continue;
    const { group, kind } = groupFor(pw, env);
    if (kind === "password" || kind === "current-password") return group;
  }
  const texts = Array.from(
    doc.querySelectorAll<HTMLInputElement>('input[type="email"], input[type="text"], input:not([type])'),
  ).slice(0, MAX_PAGE_CANDIDATES);
  for (const el of texts) {
    if (!isFillable(el, env)) continue;
    const { group, kind } = groupFor(el, env);
    if (kind === "username") return group;
  }
  return null;
}

/** The first visible one-time-code group on the page. */
export function findOtpGroup(doc: Document, env: Env): LoginGroup | null {
  const texts = Array.from(doc.querySelectorAll<HTMLInputElement>("input")).slice(0, MAX_PAGE_CANDIDATES);
  for (const el of texts) {
    if (el.type === "password" || !isFillable(el, env)) continue;
    const { group, kind } = groupFor(el, env);
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
