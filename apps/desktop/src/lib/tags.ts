import type { ItemOverview } from "./types";

export const MAX_TAGS = 20;
export const MAX_TAG_CHARS = 32;

/**
 * What Rust will store for `raw` (case kept), or null if Rust would refuse it.
 * Rust decides, and may store the spelling another item already uses; this only guides typing.
 */
export function cleanTag(raw: string): string | null {
  const tag = raw.trim().split(/\s+/).join(" ");
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
  return [...tag].length > MAX_TAG_CHARS ? "tooLong" : null;
}

export interface TagCount {
  name: string;
  count: number;
}

/** The key tags compare by: "Work" and "work" are one tag (Rust's `tags::key`). */
export const tagKey = (tag: string): string => tag.toLowerCase();

export const sameTag = (a: string, b: string): boolean => tagKey(a) === tagKey(b);

export const hasTag = (tags: readonly string[], tag: string): boolean => tags.some((t) => sameTag(t, tag));

/** A–Z without case, then by spelling so the order is stable. */
export const compareTags = (a: string, b: string): number =>
  tagKey(a).localeCompare(tagKey(b)) || a.localeCompare(b);

/**
 * Every tag in use with how many items carry it, A–Z without case. Spellings of one tag merge under
 * the one most items carry, a tie going to the smallest, as Rust picks it (`tags::spellings`).
 */
export function tagCounts(items: readonly Pick<ItemOverview, "tags">[]): TagCount[] {
  const byKey = new Map<string, Map<string, number>>();
  for (const item of items) {
    for (const tag of item.tags) {
      const spellings = byKey.get(tagKey(tag)) ?? new Map<string, number>();
      spellings.set(tag, (spellings.get(tag) ?? 0) + 1);
      byKey.set(tagKey(tag), spellings);
    }
  }
  return [...byKey.values()]
    .map((spellings) => {
      let name = "";
      let best = 0;
      let count = 0;
      for (const [spelling, n] of spellings) {
        count += n;
        if (n > best || (n === best && spelling < name)) [name, best] = [spelling, n];
      }
      return { name, count };
    })
    .sort((a, b) => compareTags(a.name, b.name));
}
