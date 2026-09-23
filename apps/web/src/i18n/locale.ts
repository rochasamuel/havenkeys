/*
 * Locales live in the URL: English at the root, Brazilian Portuguese under
 * /pt-br. A URL is then enough to share or bookmark a page in either
 * language, with no cookie and no server involved.
 */
export type Locale = "en" | "pt-BR";

export const LOCALES: Record<Locale, { prefix: string; htmlLang: string; label: string; short: string }> = {
  en: { prefix: "", htmlLang: "en", label: "English", short: "EN" },
  "pt-BR": { prefix: "/pt-br", htmlLang: "pt-BR", label: "Português (Brasil)", short: "PT" },
};

const PT_PREFIX = LOCALES["pt-BR"].prefix;

function isPtPath(pathname: string): boolean {
  const lower = pathname.toLowerCase();
  return lower === PT_PREFIX || lower.startsWith(`${PT_PREFIX}/`);
}

export function localeFromPath(pathname: string): Locale {
  return isPtPath(pathname) ? "pt-BR" : "en";
}

/** "/pt-br/security" → "/security", "/pt-br" → "/", "/security" → "/security". */
export function stripLocale(pathname: string): string {
  if (!isPtPath(pathname)) return pathname || "/";
  const rest = pathname.slice(PT_PREFIX.length);
  return rest === "" ? "/" : rest;
}

/** A locale-neutral path ("/security", "/") in the given locale. */
export function localizePath(path: string, locale: Locale): string {
  const prefix = LOCALES[locale].prefix;
  if (!prefix) return path;
  return path === "/" ? prefix : `${prefix}${path}`;
}

/** The same page in another locale, keeping any #section. */
export function switchLocalePath(pathname: string, hash: string, to: Locale): string {
  return localizePath(stripLocale(pathname), to) + hash;
}

/** Whether a browser's language list prefers Portuguese. */
export function prefersPortuguese(languages: readonly string[]): boolean {
  const first = languages[0]?.toLowerCase() ?? "";
  return first === "pt" || first.startsWith("pt-");
}
