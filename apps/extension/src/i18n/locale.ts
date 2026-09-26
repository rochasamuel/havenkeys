/*
 * The extension follows the browser's UI language: the first English or
 * Portuguese tag in its list decides (Portuguese gets Brazilian Portuguese),
 * anything else English. Pure so it can be tested without a browser.
 */
export type Locale = "en" | "pt-BR";

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
