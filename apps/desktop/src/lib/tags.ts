import type { ItemOverview } from "./types";

export const MAX_TAGS = 20;
export const MAX_TAG_CHARS = 32;

/** What Rust will store for `raw`, or null if Rust would refuse it. Rust decides; this only guides typing. */
export function cleanTag(raw: string): string | null {
  const tag = raw.trim().split(/\s+/).join(" ").toLowerCase();
  if (!tag || [...tag].length > MAX_TAG_CHARS) return null;
  // eslint-disable-next-line no-control-regex
  if (/[,\u0000-\u001f\u007f-\u009f]/.test(tag)) return null;
  return tag;
}

/** Why Rust would refuse `raw` as a tag, for the editor to say; null when it would take it or it is blank. */
export function tagProblem(raw: string): "tooLong" | "notAllowed" | null {
  const tag = raw.trim().split(/\s+/).join(" ");
  if (!tag) return null;
  // eslint-disable-next-line no-control-regex
  if (/[,\u0000-\u001f\u007f-\u009f]/.test(tag)) return "notAllowed";
  return [...tag.toLowerCase()].length > MAX_TAG_CHARS ? "tooLong" : null;
}

export interface TagCount {
  name: string;
  count: number;
}

/** Every tag in use with how many items carry it, A–Z. */
export function tagCounts(items: readonly ItemOverview[]): TagCount[] {
  const counts = new Map<string, number>();
  for (const item of items) for (const tag of item.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);
  return [...counts].map(([name, count]) => ({ name, count })).sort((a, b) => a.name.localeCompare(b.name));
}
