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

/**
 * A date and time in the UI's language. `locale` is the context's
 * `dateLocale`: the UI locale, or the OS's regional variant of it.
 */
export function formatDate(ms: number, locale: string): string {
  return new Date(ms).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" });
}

/** A file size in binary units (the 5 MB attachment limit is 5 MiB): 820 B, 240 KB, 1.5 MB. */
export function formatSize(bytes: number, locale: string): string {
  const n = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  if (bytes < 1024) return `${n.format(bytes)} B`;
  if (bytes < 1024 * 1024) return `${n.format(Math.round(bytes / 1024))} KB`;
  return `${n.format(bytes / (1024 * 1024))} MB`;
}

export type Strength ="weak" | "fair" | "strong" | "excellent";

/** Rough strength level for a generator entropy estimate. */
export function strengthLevel(bits: number): Strength {
  if (bits < 50) return "weak";
  if (bits < 75) return "fair";
  if (bits < 110) return "strong";
  return "excellent";
}

/** A `YYYY-MM-DD` day in the UI locale; read as UTC so no zone moves it. */
export function formatDay(iso: string, locale: string): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return iso;
  const d = new Date(`${iso}T00:00:00Z`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(locale, { dateStyle: "medium", timeZone: "UTC" });
}
