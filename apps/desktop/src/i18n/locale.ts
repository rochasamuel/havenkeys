/*
 * The desktop app follows the OS language (as the webview reports it) unless
 * the user picked one in Settings. The first English or Portuguese tag in
 * the list decides (Portuguese gets Brazilian Portuguese), anything else
 * English. Pure so it can be tested without a webview.
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

/** A tag's language subtag, lower-cased: "pt" for "PT_br", "en" for "en-GB". */
export function languageOf(tag: string): string {
  return (tag.split(/[-_]/)[0] ?? "").toLowerCase();
}

/**
 * The first tag in a language we have decides: Portuguese (any region) gets
 * Brazilian Portuguese, English gets English, other languages are skipped.
 * With neither, English.
 */
export function resolveLocale(tags: readonly string[]): Locale {
  for (const tag of tags) {
    if (typeof tag !== "string") continue;
    const lang = languageOf(tag);
    if (lang === "pt") return "pt-BR";
    if (lang === "en") return "en";
  }
  return "en";
}

/**
 * The locale to format dates with: the OS's first tag when it is in the UI's
 * language, so an en-GB or pt-PT system keeps its regional format; the UI
 * locale otherwise (and for a tag Intl does not accept).
 */
export function dateLocale(locale: Locale, osTags: readonly string[]): string {
  const first = osTags[0];
  if (typeof first !== "string" || languageOf(first) !== languageOf(locale)) return locale;
  try {
    return Intl.getCanonicalLocales(first.replace(/_/g, "-"))[0] ?? locale;
  } catch {
    return locale;
  }
}

/** The locale to show for a preference, given the OS's language list. */
export function effectiveLocale(preference: Preference, tags: readonly string[]): Locale {
  return preference === "auto" ? resolveLocale(tags) : preference;
}

export function isPreference(value: unknown): value is Preference {
  return value === "auto" || value === "en" || value === "pt-BR";
}
