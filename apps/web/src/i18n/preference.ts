import type { Locale } from "./locale";

/*
 * The language a visitor picked with the switcher. Only a convenience: it
 * decides whether a Portuguese browser landing on "/" is sent to /pt-br.
 * Storage can be unavailable (private windows, blocked site data), so every
 * access is guarded and a failure just means no remembered choice.
 */
const KEY = "hk-locale";

export function readPreference(): Locale | null {
  try {
    const v = window.localStorage.getItem(KEY);
    return v === "en" || v === "pt-BR" ? v : null;
  } catch {
    return null;
  }
}

export function writePreference(locale: Locale): void {
  try {
    window.localStorage.setItem(KEY, locale);
  } catch {
    // Nothing to do: the choice still applies to this page.
  }
}
