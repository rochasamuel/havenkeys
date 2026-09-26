/*
 * The extension follows the browser's UI language: any Portuguese tag gets
 * Brazilian Portuguese, everything else English. Pure so it can be tested
 * without a browser.
 */
export type Locale = "en" | "pt-BR";

/** The first tag that starts with "pt" (any case) wins; otherwise English. */
export function resolveLocale(tags: readonly string[]): Locale {
  for (const tag of tags) {
    if (typeof tag === "string" && tag.toLowerCase().startsWith("pt")) return "pt-BR";
  }
  return "en";
}
