/** A suggestion's tags as one short, quiet line: two names, then "+N". */
export function tagLine(tags: readonly string[]): string {
  if (tags.length <= 2) return tags.join(", ");
  return `${tags.slice(0, 2).join(", ")} +${tags.length - 2}`;
}
