/** A suggestion's tags as pills: the first two by name, then how many more. */
export function tagChips(tags: readonly string[]): { shown: string[]; more: number } {
  return { shown: tags.slice(0, 2), more: Math.max(0, tags.length - 2) };
}
