import type { ItemOverview } from "./types";

/** Host of the first website, for list subtitles and avatars. */
export function primaryHost(item: ItemOverview): string | null {
  const first = item.urls[0];
  if (!first) return null;
  try {
    return new URL(first.url).hostname.replace(/^www\./, "");
  } catch {
    return null;
  }
}

/** One-letter monogram for an item: the first letter or digit of its title. */
export function monogram(title: string): string {
  const first = title.trim().match(/[\p{L}\p{N}]/u);
  return first ? first[0].toUpperCase() : "?";
}

/** "381 492" / "3814 9275" — split codes in half for readability. */
export function groupCode(code: string): string {
  const half = Math.ceil(code.length / 2);
  return `${code.slice(0, half)} ${code.slice(half)}`;
}

/** A date and time in the UI's language. */
export function formatDate(ms: number, locale: string): string {
  return new Date(ms).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" });
}

export type Strength = "weak" | "fair" | "strong" | "excellent";

/** Rough strength level for a generator entropy estimate. */
export function strengthLevel(bits: number): Strength {
  if (bits < 50) return "weak";
  if (bits < 75) return "fair";
  if (bits < 110) return "strong";
  return "excellent";
}
