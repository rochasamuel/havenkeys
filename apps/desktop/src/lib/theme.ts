import type { Theme } from "./types";

/**
 * Apply a UI theme. "system" follows the OS via `prefers-color-scheme` in CSS.
 * The theme is saved with the encrypted settings, so before unlocking the
 * app uses the default (dark).
 */
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
}
