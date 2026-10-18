/*
 * Whether the sidebar's Tags group is collapsed. A UI preference, not a vault
 * setting, and not sensitive (no tag names). Storage can be unavailable or
 * throw, so a failure just means "expanded".
 */
const KEY = "hk-tags-collapsed";

export function readTagsCollapsed(): boolean {
  try {
    return window.localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export function writeTagsCollapsed(collapsed: boolean): void {
  try {
    if (collapsed) window.localStorage.setItem(KEY, "1");
    else window.localStorage.removeItem(KEY);
  } catch {
    // Nothing to do: the choice still applies until the app restarts.
  }
}
