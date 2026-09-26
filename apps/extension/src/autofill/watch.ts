// Watching for the next step of an automatic sign-in: the password page
// after a username step, or the one-time-code page after the password.
//
// Runs only during a sign-in run, for at most WATCH_TIMEOUT_MS. One
// MutationObserver, debounced to one bounded check per CHECK_DEBOUNCE_MS;
// each check uses the same capped page lookups as the popup fill, so a page
// with thousands of nodes costs the same as a small one (attack 7). A field
// counts only when the same element is found by two consecutive checks.

import { valueSource } from "./fill";
import { fieldsOf, type Env } from "./group";
import { findLoginGroup, findOtpGroup } from "./page";
import { hasChallenge } from "./submit";

export type WatchKind = "password" | "otp";
export type WatchResult = { found: WatchKind; field: HTMLInputElement } | { stop: "timeout" | "came_back" | "challenge" };

export const WATCH_TIMEOUT_MS = 30_000;
export const CHECK_DEBOUNCE_MS = 150;

export type Seen =
  | { kind: WatchKind; el: HTMLInputElement }
  | { kind: "came_back"; el: HTMLInputElement }
  | { kind: "challenge"; el: null };

/**
 * What the page shows now, for a run waiting for `want`. `filled` is the
 * field we filled for the previous step on this page (null on a freshly
 * loaded page). The previous step's field "came back" when it is a
 * different element, or no longer holds our value: the site re-rendered it
 * after rejecting the submission.
 */
export function detectStep(doc: Document, env: Env, want: WatchKind, filled: HTMLInputElement | null): Seen | null {
  if (hasChallenge(doc, env)) return { kind: "challenge", el: null };
  const login = findLoginGroup(doc, env);
  const [pw] = login ? fieldsOf(login, "current-password", "password") : [];
  const [user] = login ? fieldsOf(login, "username") : [];
  const cameBack = (el: HTMLInputElement) => el !== filled || valueSource(el) !== "vault";
  if (want === "password") {
    if (pw) return { kind: "password", el: pw };
    if (user && cameBack(user)) return { kind: "came_back", el: user };
    return null;
  }
  const otp = findOtpGroup(doc, env);
  const [box] = otp ? fieldsOf(otp, "otp") : [];
  if (box) return { kind: "otp", el: box };
  if (pw && cameBack(pw)) return { kind: "came_back", el: pw };
  return null;
}

/** Watch for `want`; calls `onResult` once. Returns a cancel function that suppresses it. */
export function watchNext(o: {
  want: WatchKind;
  filled: HTMLInputElement | null;
  env: () => Env;
  onResult: (r: WatchResult) => void;
  doc?: Document;
  timeoutMs?: number;
}): () => void {
  const doc = o.doc ?? document;
  let last: Seen | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let done = false;

  const observer = new MutationObserver(() => schedule());
  const deadline = setTimeout(() => finish({ stop: "timeout" }), o.timeoutMs ?? WATCH_TIMEOUT_MS);

  function cancel(): void {
    done = true;
    observer.disconnect();
    clearTimeout(deadline);
    if (timer) clearTimeout(timer);
    timer = null;
  }

  function finish(r: WatchResult): void {
    if (done) return;
    cancel();
    o.onResult(r);
  }

  function schedule(): void {
    if (done || timer) return;
    timer = setTimeout(() => {
      timer = null;
      check();
    }, CHECK_DEBOUNCE_MS);
  }

  function check(): void {
    if (done) return;
    const seen = detectStep(doc, o.env(), o.want, o.filled);
    if (seen && last && seen.kind === last.kind && seen.el === last.el) {
      if (seen.kind === "challenge") return finish({ stop: "challenge" });
      if (seen.kind === "came_back") return finish({ stop: "came_back" });
      return finish({ found: seen.kind, field: seen.el });
    }
    last = seen;
    // Confirm on the next check even if nothing else mutates.
    if (seen) schedule();
  }

  observer.observe(doc.documentElement, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["class", "style", "hidden", "disabled", "type"],
  });
  check();
  return cancel;
}
