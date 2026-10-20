// Text normalization for field heuristics.
//
// Everything here reads page-controlled strings (attributes, labels). They
// are only ever compared against fixed keyword lists; nothing is evaluated,
// inserted into the DOM or sent anywhere.

/** Longest string considered from any one source. Keeps work bounded. */
export const MAX_HINT_CHARS = 200;

/** Single letters joined by dots, as a whole word: `C.P.F.`, `R.G`. */
const DOTTED_LETTERS = /(?<![\p{L}\p{N}])\p{L}(?:\.\p{L})+\.?(?![\p{L}\p{N}])/gu;

/**
 * Lowercase, strip accents, split camelCase and separators into single
 * spaces: `"loginEmail_Address"` → `"login email address"`. Dotted
 * abbreviations join: `"C.P.F."` → `"cpf"`. Android's TextMatch.kt and the
 * word-file generator apply the same rule (packages/field-words/fixtures/normalize.json).
 */
export function normalize(raw: string | null | undefined, maxChars = MAX_HINT_CHARS): string {
  if (!raw) return "";
  return raw
    .slice(0, maxChars)
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(DOTTED_LETTERS, (m) => m.replaceAll(".", ""))
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

/**
 * Does `text` (already normalized) contain `phrase` as whole words?
 * `"user name"` matches `"your user name"` but not `"username"`.
 */
export function hasPhrase(text: string, phrase: string): boolean {
  if (!text) return false;
  return ` ${text} `.includes(` ${phrase} `);
}

export function hasAny(text: string, phrases: readonly string[]): boolean {
  return phrases.some((p) => hasPhrase(text, p));
}

export { PAY_WORDS, SUBMIT_WORDS } from "./words.generated";
