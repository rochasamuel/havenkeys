// Recognising "Sign in with <provider>" buttons, and the saved account's row
// on a provider's account chooser.
//
// The DOM is untrusted: its strings are only compared against fixed lists,
// and the work per scan is bounded (MAX_SSO_CANDIDATES). A match only lets
// the extension *offer* a sign-in the desktop already matched to this page;
// what a run may do on the provider's page is bounded by origins from Rust.

import { SSO_PROVIDERS, type SsoProvider } from "@havenkeys/protocol";
import type { Env } from "./group";
import { PRESS_MIN_MARGIN } from "./submit";
import { hasAny, hasPhrase, MAX_HINT_CHARS, normalize } from "./text";

export const SSO_CANDIDATES = 'button, a[href], [role="button"], [role="link"], input[type="submit"], input[type="button"]';
export const MAX_SSO_CANDIDATES = 400;
export const SSO_MIN_SCORE = 60;
/** A label longer than this (normalized) is prose, not a button. */
const MAX_LABEL_CHARS = 60;

// `normalize()` splits camelCase, so "GitHub" becomes "git hub"; "Github" or
// "github" do not split. Both spellings are accepted.
const NAMES: Record<SsoProvider, readonly string[]> = {
  google: ["google"],
  microsoft: ["microsoft"],
  github: ["github", "git hub"],
  apple: ["apple"],
  facebook: ["facebook"],
  discord: ["discord"],
  // "x" alone is not a name: it joins this list with the joiner-only rule (Task 2).
  x: ["twitter"],
  linkedin: ["linkedin", "linked in"],
  gitlab: ["gitlab", "git lab"],
};
const JOINERS = ["with", "using", "via", "com", "com a", "com o", "pelo", "pela"];
const NEGATIVE = [
  "drive", "docs", "play", "store", "maps", "calendar", "repository", "repo", "star", "fork", "sponsor",
  "download", "app store", "teams", "office", "outlook", "music", "pay", "wallet", "podcasts", "tv",
];
// Phrases safe to recognise as consent at the *start* of a label, at any
// length: verbs (or verb + object) no one is named after, so a long
// trailing account or app name ("Allow access for <the long account the
// page chose>", "Grant access to <a long application name>") is still
// caught. Bare "allow" is included: an account row never starts with the
// verb "Allow". Bare "grant" is deliberately excluded — it is a common
// given/family name ("Grant Smith"), and no position- or length-based
// string check can tell "Grant Smith" from "Grant access" apart by the
// word "grant" alone; only the longer, unambiguous "grant access"/"grant
// permission(s)" phrases are recognised. "accept"/"aceitar" stay in
// CONSENT_WORDS below (common enough as a bare short button) rather than
// here, since a name could plausibly start with them too.
const CONSENT_PREFIXES = [
  "continue as", "continuar como",
  "authorize", "authorise", "permitir", "autorizar",
  "allow", "allow access",
  "grant access", "grant permission", "grant permissions",
  "accept and continue", "aceitar e continuar",
  "conceder acesso", "permitir acesso",
];
const CONSENT_WORDS = ["accept", "aceitar", "continue", "continuar", "confirmar"];
/** A short bare consent word only counts within this length. */
const MAX_CONSENT_LABEL_CHARS = 30;
const EMAIL = /[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}/i;

/**
 * `el`'s text nodes joined with spaces (unlike `.textContent`, so sibling
 * elements' text, e.g. a name and an email in adjacent `<div>`s, or two
 * `<span>`s split mid-word ("Continuar com" + "o Google"), cannot fuse into
 * one token: "Continuar com" + "o Google" stays "continuar com o google",
 * not "continuar como google").
 */
function textOf(el: Element): string {
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
  const parts: string[] = [];
  for (let n = walker.nextNode(), total = 0; n && total < 400; n = walker.nextNode()) {
    const t = n.nodeValue ?? "";
    parts.push(t);
    total += t.length;
  }
  return parts.join(" ");
}

function label(el: Element): string {
  const attr = (n: string) => (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
  const own = el instanceof HTMLInputElement ? el.value.slice(0, MAX_HINT_CHARS) : textOf(el).slice(0, MAX_HINT_CHARS);
  const img = el.querySelector("img[alt]")?.getAttribute("alt")?.slice(0, 60) ?? "";
  return normalize(`${own} ${attr("aria-label")} ${attr("title")} ${img}`, MAX_HINT_CHARS * 3);
}

/** Is `t` (a normalized label) a consent/permissions button? Never pressed. */
function isConsentLabel(t: string): boolean {
  return CONSENT_PREFIXES.some((p) => t.startsWith(p)) || (t.length <= MAX_CONSENT_LABEL_CHARS && hasAny(t, CONSENT_WORDS));
}

function hrefProvider(el: Element): SsoProvider | null {
  const href = el instanceof HTMLAnchorElement ? el.getAttribute("href") : null;
  if (!href) return null;
  let origin: string;
  try {
    origin = new URL(href, document.baseURI).origin;
  } catch {
    return null;
  }
  for (const [p, v] of Object.entries(SSO_PROVIDERS)) if (v.origins.includes(origin)) return p as SsoProvider;
  return null;
}

function usable(el: HTMLElement, env: Env): boolean {
  return env.isVisible(el) && !el.matches(":disabled") && el.getAttribute("aria-disabled") !== "true";
}

/**
 * The provider this element signs in with, and how sure we are. `context`:
 * the page also has a login field or another provider button, which lets a
 * bare "Google" label count.
 */
export function providerOf(el: Element, context: boolean): { provider: SsoProvider; score: number } | null {
  const text = label(el);
  if (!text || text.length > MAX_LABEL_CHARS || hasAny(text, NEGATIVE)) return null;
  const linked = hrefProvider(el);
  let best: { provider: SsoProvider; score: number } | null = null;
  for (const [p, names] of Object.entries(NAMES) as [SsoProvider, readonly string[]][]) {
    for (const name of names) {
      if (!hasPhrase(text, name)) continue;
      let s = JOINERS.some((j) => hasPhrase(text, `${j} ${name}`)) ? 80 : text === name || text === `${name} account` ? 40 : 0;
      if (s === 40 && context) s += 20;
      if (linked === p) s += 30;
      if (s > 0 && (!best || s > best.score)) best = { provider: p, score: s };
    }
  }
  return best;
}

function candidates(root: ParentNode, env: Env): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(SSO_CANDIDATES))
    .slice(0, MAX_SSO_CANDIDATES)
    .filter((el) => usable(el, env));
}

function hasLoginField(root: ParentNode): boolean {
  return root.querySelector('input[type="password"], input[type="email"], input[autocomplete~="username"]') !== null;
}

function scored(root: ParentNode, env: Env): { el: HTMLElement; provider: SsoProvider; score: number }[] {
  const els = candidates(root, env);
  const first = els.map((el) => ({ el, hit: providerOf(el, false) }));
  const providers = new Set(first.flatMap((x) => (x.hit ? [x.hit.provider] : [])));
  const context = providers.size >= 2 || hasLoginField(root);
  const out: { el: HTMLElement; provider: SsoProvider; score: number }[] = [];
  for (const { el } of first) {
    const hit = providerOf(el, context);
    if (hit && hit.score >= SSO_MIN_SCORE) out.push({ el, ...hit });
  }
  return out;
}

/** Each provider with a qualifying button on the page, and its best button. */
export function findProviderButtons(root: ParentNode, env: Env): Map<SsoProvider, HTMLElement> {
  const out = new Map<SsoProvider, { el: HTMLElement; score: number }>();
  for (const c of scored(root, env)) {
    const cur = out.get(c.provider);
    if (!cur || c.score > cur.score) out.set(c.provider, c);
  }
  return new Map([...out].map(([p, c]) => [p, c.el]));
}

/**
 * The button to press for `provider`: the clear winner among that
 * provider's own candidates only (its best score beats its runner-up by at
 * least PRESS_MIN_MARGIN, mirroring `submit.ts`), or null when there is no
 * candidate or the best is ambiguous (a tie, or too close to call).
 */
export function providerButton(root: ParentNode, provider: SsoProvider, env: Env): HTMLElement | null {
  const ranked = scored(root, env)
    .filter((c) => c.provider === provider)
    .sort((a, b) => b.score - a.score);
  const [best, second] = ranked;
  if (!best) return null;
  if (second && best.score - second.score < PRESS_MIN_MARGIN) return null;
  return best.el;
}

/** The email address in an element's text, lowercased, or null. */
export function emailIn(el: Element): string | null {
  const attr = (el.getAttribute("data-identifier") ?? el.getAttribute("data-email") ?? "").slice(0, 400);
  const m = EMAIL.exec(textOf(el).slice(0, 400)) ?? EMAIL.exec(attr);
  return m ? m[0].toLowerCase() : null;
}

/**
 * The chooser row for `account`: exactly one clickable, visible element
 * whose text holds it and whose own label is not itself a consent screen
 * (a "Continue as <account>" tile matches the account text too, but must
 * never be treated as the row to click — safety does not depend on the
 * caller checking `isConsentScreen` first).
 */
export function chooserRow(root: ParentNode, account: string, env: Env): HTMLElement | null {
  const want = account.trim().toLowerCase();
  const hits = candidates(root, env).filter((el) => emailIn(el) === want && !isConsentLabel(label(el)));
  // A row nested in another row (link inside list item) counts once: keep the outermost.
  const outer = hits.filter((el) => !hits.some((o) => o !== el && o.contains(el)));
  return outer.length === 1 ? (outer[0] as HTMLElement) : null;
}

/** A permissions or confirmation screen: HavenKeys never presses these. */
export function isConsentScreen(root: ParentNode, env: Env): boolean {
  return candidates(root, env).some((el) => isConsentLabel(label(el)));
}

/** "Use another account" on a provider's account chooser, in the languages HavenKeys ships. */
const ANOTHER_ACCOUNT = [
  "use another account", "usar outra conta",
  "sign in with a different account", "usar uma conta diferente",
  "use a different account", "entrar com outra conta",
];

/**
 * The chooser's "Use another account" control: exactly one visible,
 * enabled candidate whose whole label is one of the phrases, on a page
 * that is not a consent screen. Null otherwise.
 */
export function anotherAccountButton(root: ParentNode, env: Env): HTMLElement | null {
  if (isConsentScreen(root, env)) return null;
  const hits = candidates(root, env).filter((el) => ANOTHER_ACCOUNT.includes(label(el)));
  const outer = hits.filter((el) => !hits.some((o) => o !== el && o.contains(el)));
  return outer.length === 1 ? (outer[0] as HTMLElement) : null;
}
