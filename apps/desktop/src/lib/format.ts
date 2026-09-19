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

/** Two-letter monogram for an item. */
export function monogram(title: string): string {
  const words = title.trim().split(/\s+/).filter(Boolean);
  const letters = words.length >= 2 ? `${words[0]![0]}${words[1]![0]}` : title.trim().slice(0, 2);
  return letters.toUpperCase() || "?";
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
