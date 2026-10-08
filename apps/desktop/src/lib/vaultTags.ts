import { createContext, useContext, useMemo } from "react";
import type { ItemOverview } from "./types";
import { tagCounts, type TagCount } from "./tags";

/** What the editor needs of each vault item: which item it is, and its tags. */
export type TaggedItem = Pick<ItemOverview, "id" | "tags">;

const VaultTags = createContext<readonly TaggedItem[]>([]);

/** The vault's items' tags, for the editor's suggestions. */
export const VaultTagsProvider = VaultTags.Provider;

/**
 * The vault's tags with counts and spellings, from every item but `exceptId`
 * (the one being edited), as Rust leaves the item itself out when it picks
 * a tag's spelling: the only item with "work" may become "Work".
 */
export function useVaultTags(exceptId?: string): TagCount[] {
  const items = useContext(VaultTags);
  return useMemo(() => tagCounts(exceptId ? items.filter((i) => i.id !== exceptId) : items), [items, exceptId]);
}
