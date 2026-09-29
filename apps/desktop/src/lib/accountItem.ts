// The HavenKeys Account item: pinned first in All items, built from the
// account record when shown and never stored (spec 2026-09-29-account-item).

import type { AccountStatus } from "./types";
import type { Section } from "../views/VaultScreen";

function hostOf(url: string): string {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return "";
  }
}

/** Whether the pinned row belongs in this section for this search. `title` is the shown (translated) title. */
export function showAccountItem(
  account: AccountStatus | null,
  section: Section,
  query: string,
  title = "HavenKeys Account",
): boolean {
  if (!account || section !== "all") return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return ["havenkeys account", title.toLowerCase(), account.email.toLowerCase(), hostOf(account.serverUrl)].some((text) => text.includes(q));
}
