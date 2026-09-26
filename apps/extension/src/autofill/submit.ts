// Pressing a login form's submit button, for automatic sign-in.
//
// Conservative by design: a button is pressed only when one candidate
// clearly wins; otherwise the fields stay filled and the user presses.
// The DOM is untrusted: its strings are only compared against keyword
// lists. Nothing here reads or writes field values.

import type { Env } from "./group";
import { hasAny, MAX_HINT_CHARS, normalize, SUBMIT_WORDS } from "./text";

export type PressStep = "username" | "password" | "otp";

/** Candidate buttons examined per scope. */
const MAX_BUTTONS = 30;
/** Ancestor levels searched above a form-less group for its button. */
const MAX_SCOPE_CLIMB = 3;
export const PRESS_MIN_SCORE = 60;
export const PRESS_MIN_MARGIN = 20;
export const ENABLE_WAIT_MS = 1000;
export const ENABLE_POLL_MS = 100;
export const OTP_SETTLE_MS = 500;

const CANDIDATES = 'button, input[type="submit"], input[type="image"], [role="button"]';

const STEP_WORDS: Record<PressStep, readonly string[]> = {
  username: ["continue", "next", "proximo", "continuar", "avancar", "seguinte"],
  password: ["sign in", "log in", "login", "signin", "entrar", "acessar", "iniciar sesion"],
  otp: ["verify", "confirm", "submit", "verificar", "confirmar", "enviar"],
};

/** Any of these disqualifies a button: it goes somewhere else. */
const NEGATIVE_WORDS = [
  "forgot", "reset", "create account", "sign up", "signup", "register", "cadastrar", "criar conta",
  "cancel", "cancelar", "back", "voltar", "resend", "reenviar", "show", "mostrar", "another", "outra",
  "passkey", "with google", "with apple", "with facebook", "with microsoft", "with github",
  "com google", "com apple", "com facebook", "com microsoft", "com github",
];

function label(el: Element): string {
  const attr = (n: string) => (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
  const own = el instanceof HTMLInputElement ? el.value.slice(0, MAX_HINT_CHARS) : (el.textContent ?? "").slice(0, MAX_HINT_CHARS);
  return normalize(`${own} ${attr("aria-label")} ${attr("title")}`, MAX_HINT_CHARS * 3);
}

function isSubmitter(el: Element): el is HTMLButtonElement | HTMLInputElement {
  return (
    (el instanceof HTMLButtonElement && el.type === "submit") ||
    (el instanceof HTMLInputElement && (el.type === "submit" || el.type === "image"))
  );
}

function score(el: HTMLElement, field: HTMLInputElement, step: PressStep): number {
  const text = label(el);
  if (hasAny(text, NEGATIVE_WORDS)) return -1;
  let s = 0;
  if (field.form && isSubmitter(el) && el.form === field.form) s += 60;
  if (hasAny(text, STEP_WORDS[step])) s += 50;
  if (hasAny(text, SUBMIT_WORDS)) s += 20;
  if (field.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING) s += 10;
  return s;
}

/** The group's root, then (outside a form) a few ancestors: SPAs often put the button beside the fields' container. */
function scopes(root: ParentNode): ParentNode[] {
  const out: ParentNode[] = [root];
  if (root instanceof HTMLFormElement || !(root instanceof Element)) return out;
  let node: Element | null = root.parentElement;
  for (let i = 0; node && node !== document.body && i < MAX_SCOPE_CLIMB; i++, node = node.parentElement) out.push(node);
  return out;
}

/**
 * The button that submits `field`'s step, or null when none clearly wins.
 * Disabled buttons are candidates: many sites enable theirs only once the
 * input validates, and pressWhenReady waits for that.
 *
 * A scope with no candidate reaching PRESS_MIN_SCORE is skipped and the
 * climb continues (e.g. a "Show password" toggle alone in the field's own
 * container); a scope whose best candidate qualifies but does not clear
 * PRESS_MIN_MARGIN over the runner-up stops the search with null, since
 * that ambiguity would not be resolved by climbing further.
 */
export function findSubmitButton(root: ParentNode, field: HTMLInputElement, step: PressStep, env: Env): HTMLElement | null {
  for (const scope of scopes(root)) {
    const buttons = Array.from(scope.querySelectorAll<HTMLElement>(CANDIDATES))
      .slice(0, MAX_BUTTONS)
      .filter((b) => env.isVisible(b));
    if (buttons.length === 0) continue;
    const ranked = buttons.map((b) => ({ b, s: score(b, field, step) })).sort((x, y) => y.s - x.s);
    const [best, second] = ranked;
    if (!best || best.s < PRESS_MIN_SCORE) continue;
    if (second && best.s - second.s < PRESS_MIN_MARGIN) return null;
    return best.b;
  }
  return null;
}

function isChallengeFrame(src: string, base: string): boolean {
  let u: URL;
  try {
    u = new URL(src, base);
  } catch {
    return false;
  }
  if (u.searchParams.get("size") === "invisible") return false;
  const h = u.hostname;
  return (
    ((h === "www.google.com" || h === "www.recaptcha.net") && u.pathname.startsWith("/recaptcha/")) ||
    h === "hcaptcha.com" ||
    h.endsWith(".hcaptcha.com") ||
    h === "challenges.cloudflare.com"
  );
}

/**
 * A visible CAPTCHA or bot challenge. Invisible reCAPTCHA badges do not
 * count, including Google's documented button-bound invisible/v3 pattern
 * (`<button class="g-recaptcha" data-sitekey=... data-callback=...>`): the
 * class marks the sign-in button itself as the reCAPTCHA anchor, it is not
 * a separate widget blocking the page, so such elements are excluded here.
 */
export function hasChallenge(doc: Document, env: Env): boolean {
  for (const f of Array.from(doc.querySelectorAll("iframe")).slice(0, 50)) {
    if (env.isVisible(f) && isChallengeFrame(f.getAttribute("src") ?? "", doc.baseURI)) return true;
  }
  return Array.from(doc.querySelectorAll<HTMLElement>(".g-recaptcha, .h-captcha, .cf-turnstile"))
    .slice(0, 10)
    .some((el) => el.getAttribute("data-size") !== "invisible" && !el.matches(CANDIDATES) && env.isVisible(el));
}

function isDisabled(b: HTMLElement): boolean {
  return (b as HTMLButtonElement).disabled === true || b.getAttribute("aria-disabled") === "true";
}

const realSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Press `button` once, when it is usable:
 * * OTP step: first give the site OTP_SETTLE_MS to submit on its own.
 * * Wait up to ENABLE_WAIT_MS for the button to enable.
 * * Never with a challenge on the page.
 * * `form.requestSubmit(button)` for a form's submit button, so the site's
 *   validation and submit handlers run; `click()` otherwise.
 *
 * `cancelled`, when given, is checked after every wait and again immediately
 * before pressing; once it reports true the run has ended (user takeover,
 * `bg_run_end`, or a newer run superseding this one) and nothing is pressed.
 */
export async function pressWhenReady(o: {
  button: HTMLElement;
  field: HTMLInputElement;
  step: PressStep;
  env: Env;
  doc?: Document;
  sleep?: (ms: number) => Promise<void>;
  cancelled?: () => boolean;
}): Promise<"pressed" | "site_submitted" | "gave_up"> {
  const sleep = o.sleep ?? realSleep;
  const doc = o.doc ?? document;
  if (o.step === "otp") {
    await sleep(OTP_SETTLE_MS);
    if (o.cancelled?.()) return "gave_up";
    if (!o.field.isConnected || !o.env.isVisible(o.field)) return "site_submitted";
  }
  for (let waited = 0; isDisabled(o.button); waited += ENABLE_POLL_MS) {
    if (waited >= ENABLE_WAIT_MS) return "gave_up";
    await sleep(ENABLE_POLL_MS);
    if (o.cancelled?.()) return "gave_up";
  }
  if (o.cancelled?.()) return "gave_up";
  if (!o.button.isConnected || !o.env.isVisible(o.button) || hasChallenge(doc, o.env)) return "gave_up";
  const form = isSubmitter(o.button) ? o.button.form : null;
  if (form && typeof form.requestSubmit === "function") form.requestSubmit(o.button);
  else o.button.click();
  return "pressed";
}
