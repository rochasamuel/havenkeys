/*
 * The messages for this JS context (popup, options, inline frames, content
 * script, background), picked once from the browser's UI language. There is
 * no switcher: the extension speaks whatever the browser speaks.
 *
 * Strings are static and always inserted with textContent; nothing here is
 * built from page or vault data.
 */
import { en, type Messages } from "./en";
import { resolveLocale, type Locale } from "./locale";
import { ptBR } from "./pt-BR";

export type { Locale } from "./locale";
export type { Messages } from "./en";
export { en } from "./en";
export { ptBR } from "./pt-BR";

const MESSAGES: Record<Locale, Messages> = { en, "pt-BR": ptBR };

function browserTags(): string[] {
  try {
    const ui = globalThis.chrome?.i18n?.getUILanguage?.();
    if (typeof ui === "string" && ui) return [ui];
  } catch {
    // No i18n API in this context (tests): fall through to navigator.
  }
  try {
    const nav = globalThis.navigator;
    if (nav?.languages?.length) return [...nav.languages];
    if (nav?.language) return [nav.language];
  } catch {
    // Nothing to go on: English.
  }
  return [];
}

export const locale: Locale = resolveLocale(browserTags());

export const t: Messages = MESSAGES[locale];

/** Marks the page's language so screen readers and hyphenation follow it. */
export function applyDocumentLang(): void {
  document.documentElement.lang = locale;
}
