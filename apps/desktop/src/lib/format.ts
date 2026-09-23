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

export function formatDate(ms: number): string {
  return new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** Rough strength label for a generator entropy estimate. */
export function strengthLabel(bits: number): "Weak" | "Fair" | "Strong" | "Excellent" {
  if (bits < 50) return "Weak";
  if (bits < 75) return "Fair";
  if (bits < 110) return "Strong";
  return "Excellent";
}
