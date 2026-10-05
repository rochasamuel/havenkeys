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
/** Past MAX_SCOPE_CLIMB, how far the climb may go on (Google's full sign-in
 * page shares only a `<main>` 14 levels up between field and "Next"), as
 * long as no other field comes into scope (see `scopes`). */
const MAX_FAR_CLIMB = 16;
const TEXT_FIELDS = 'input:not([type]), input[type="text"], input[type="email"], input[type="password"], input[type="tel"], input[type="number"], input[type="search"]';
export const PRESS_MIN_SCORE = 60;
export const PRESS_MIN_MARGIN = 20;
export const ENABLE_WAIT_MS = 1000;
export const ENABLE_POLL_MS = 100;
export const OTP_SETTLE_MS = 500;
/** How long findSubmitButton is retried after a fill while the page settles. */
export const BUTTON_WAIT_MS = 1500;
export const BUTTON_POLL_MS = 150;

const CANDIDATES = 'button, input[type="submit"], input[type="image"], [role="button"]';

const STEP_WORDS: Record<PressStep, readonly string[]> = {
  username: ["continue", "next", "proximo", "continuar", "avancar", "seguinte"],
  // Multi-step sign-ins (Google's "Next", Auth0's "Continue") use the same
  // word for the password step as for the username step.
  password: ["sign in", "log in", "login", "signin", "entrar", "acessar", "iniciar sesion", "next", "continue", "proximo", "continuar", "avancar", "seguinte"],
  otp: ["verify", "confirm", "submit", "verificar", "confirmar", "enviar"],
};

/** Any of these disqualifies a button: it goes somewhere else. */
const NEGATIVE_WORDS = [
  "forgot", "reset", "create account", "sign up", "signup", "register", "cadastrar", "criar conta",
  "cancel", "cancelar", "back", "voltar", "resend", "reenviar", "show", "mostrar", "another", "outra", "esqueceu", "esqueci",
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

/**
 * The group's root, then (outside a form) a few ancestors: SPAs often put
 * the button beside the fields' container. Beyond MAX_SCOPE_CLIMB levels the
 * climb continues only while the scope holds no visible field outside the
 * group's root: one that does may be another form, with its own button.
 */
function scopes(root: ParentNode, env: Env): ParentNode[] {
  const out: ParentNode[] = [root];
  if (root instanceof HTMLFormElement || !(root instanceof Element)) return out;
  let node: Element | null = root.parentElement;
  for (let i = 0; node && node !== document.body && i < MAX_FAR_CLIMB; i++, node = node.parentElement) {
    if (i >= MAX_SCOPE_CLIMB && otherField(node, root, env)) break;
    out.push(node);
  }
  return out;
}

function otherField(scope: Element, root: Element, env: Env): boolean {
  return Array.from(scope.querySelectorAll<HTMLInputElement>(TEXT_FIELDS))
    .slice(0, MAX_BUTTONS)
    .some((f) => !root.contains(f) && env.isVisible(f));
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
  for (const scope of scopes(root, env)) {
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

const realSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * findSubmitButton, retried every BUTTON_POLL_MS for up to BUTTON_WAIT_MS
 * while the filled field stays on the page. A continuation fills the next
 * step as soon as its field shows, often while the page is still swapping
 * views: Google's password view appears before its "Avançar" is rendered,
 * and the email view's own "Avançar" may still be on screen (a tie). Each
 * retry applies the same rules; nothing is pressed here. Null on timeout,
 * a challenge, cancellation, or when the field leaves the page.
 */
export async function waitForSubmitButton(o: {
  root: ParentNode;
  field: HTMLInputElement;
  step: PressStep;
  env: () => Env;
  doc?: Document;
  sleep?: (ms: number) => Promise<void>;
  cancelled?: () => boolean;
}): Promise<HTMLElement | null> {
  const sleep = o.sleep ?? realSleep;
  const doc = o.doc ?? document;
  for (let waited = 0; ; waited += BUTTON_POLL_MS) {
    if (o.cancelled?.() || !o.field.isConnected) return null;
    const env = o.env();
    if (hasChallenge(doc, env)) return null;
    const button = findSubmitButton(o.root, o.field, o.step, env);
    if (button) return button;
    if (waited >= BUTTON_WAIT_MS) return null;
    await sleep(BUTTON_POLL_MS);
  }
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

/** Disabled itself, inside a `<fieldset disabled>` (both match `:disabled`), or `aria-disabled`. */
function isDisabled(b: HTMLElement): boolean {
  return b.matches(":disabled") || b.getAttribute("aria-disabled") === "true";
}

/**
 * Press `button` once, when it is usable:
 * * OTP step: first give the site OTP_SETTLE_MS to submit on its own.
 * * Wait up to ENABLE_WAIT_MS for the button to enable.
 * * Never with a challenge on the page.
 * * `button.click()`, as a user's click: the button's own click handlers
 *   run, then (unless one cancels it) the form submits with the button as
 *   submitter, running validation and submit handlers. `requestSubmit`
 *   would skip the click handlers, where sites like gov.br start their
 *   submit (an invisible hCaptcha).
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
  o.button.click();
  return "pressed";
}
