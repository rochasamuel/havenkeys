import { isPreference, type Preference } from "./locale";

/*
 * The language picked in Settings. A UI preference, not a vault setting: the
 * unlock screen needs it before any vault is open, and it is not sensitive.
 * Storage can be unavailable or throw, so every access is guarded and a
 * failure just means "auto".
 */
const KEY = "hk-locale";

export function readPreference(): Preference {
  try {
    const v = window.localStorage.getItem(KEY);
    return isPreference(v) ? v : "auto";
  } catch {
    return "auto";
  }
}

export function writePreference(preference: Preference): void {
  try {
    if (preference === "auto") window.localStorage.removeItem(KEY);
    else window.localStorage.setItem(KEY, preference);
  } catch {
    // Nothing to do: the choice still applies until the app restarts.
  }
}
