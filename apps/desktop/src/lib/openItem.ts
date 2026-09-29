// "Edit in HavenKeys" from the browser extension's popup: deciding what an
// open request does to the vault screen's current pane, and telling an
// untouched editor from a changed one.

import type { SecretEdit } from "./secretEdit";
import type { SignInWith, UrlRule } from "./types";

/** The part of the vault screen's pane that decides what an open request does. */
export type PaneRef = { kind: "empty" } | { kind: "view"; id: string } | { kind: "edit"; id: string } | { kind: "new" };

/**
 * What to do when the browser extension asks to edit `id`:
 * `already` — that editor is open and visible; `reveal` — that editor is
 * open but hidden behind a tool section (Settings/Generator), switch back
 * to it without touching the editor; `confirm` — another edit has unsaved
 * changes, ask before discarding them; `open` — switch to `id`'s editor.
 * `editorShown` is false when the vault screen is on a section (Settings,
 * Generator) that hides the item pane even though it still has state.
 */
export function decideOpen(
  pane: PaneRef,
  dirty: boolean,
  id: string,
  editorShown = true,
): "open" | "already" | "reveal" | "confirm" {
  if (pane.kind === "edit" && pane.id === id) return editorShown ? "already" : "reveal";
  if ((pane.kind === "edit" || pane.kind === "new") && dirty) return "confirm";
  return "open";
}

/** Everything the editor would save, for telling an untouched editor from a changed one. */
export interface EditorSnapshot {
  title: string;
  username: string;
  urls: UrlRule[];
  password: SecretEdit;
  totp: SecretEdit;
  notes: SecretEdit;
  autoSignIn: boolean;
  signInWith: SignInWith | null;
}

/** Compared in memory only; never logged or stored. */
export function isDirty(initial: EditorSnapshot, current: EditorSnapshot): boolean {
  return JSON.stringify(initial) !== JSON.stringify(current);
}
