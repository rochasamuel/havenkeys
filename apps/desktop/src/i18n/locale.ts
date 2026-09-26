/*
 * The desktop app follows the OS language (as the webview reports it) unless
 * the user picked one in Settings. Any Portuguese tag gets Brazilian
 * Portuguese, everything else English. Pure so it can be tested without a
 * webview.
 */
export type Locale = "en" | "pt-BR";

/** What the user chose in Settings; "auto" follows the OS. */
export type Preference = "auto" | Locale;

export const PREFERENCES: readonly Preference[] = ["auto", "en", "pt-BR"];

/** Each language in its own words, so it can be found from either one. */
export const LANGUAGE_NAMES: Record<Locale, string> = {
  en: "English",
  "pt-BR": "Português (Brasil)",
};

/** The first tag that starts with "pt" (any case) wins; otherwise English. */
export function resolveLocale(tags: readonly string[]): Locale {
  for (const tag of tags) {
    if (typeof tag === "string" && tag.toLowerCase().startsWith("pt")) return "pt-BR";
  }
  return "en";
}

/** The locale to show for a preference, given the OS's language list. */
export function effectiveLocale(preference: Preference, tags: readonly string[]): Locale {
  return preference === "auto" ? resolveLocale(tags) : preference;
}

export function isPreference(value: unknown): value is Preference {
  return value === "auto" || value === "en" || value === "pt-BR";
}
