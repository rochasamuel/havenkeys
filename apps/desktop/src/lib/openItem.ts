// "Edit in HavenKeys" from the browser extension's popup: deciding what an
// open request does to the vault screen's current pane, and telling an
// untouched editor from a changed one.

import type { SecretEdit } from "./secretEdit";
import type { UrlRule } from "./types";

/** The part of the vault screen's pane that decides what an open request does. */
export type PaneRef = { kind: "empty" } | { kind: "view"; id: string } | { kind: "edit"; id: string } | { kind: "new" };

/**
 * What to do when the browser extension asks to edit `id`:
 * `already` — that editor is open; `confirm` — another edit has unsaved
 * changes, ask before discarding them; `open` — switch to `id`'s editor.
 */
export function decideOpen(pane: PaneRef, dirty: boolean, id: string): "open" | "already" | "confirm" {
  if (pane.kind === "edit" && pane.id === id) return "already";
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
}

/** Compared in memory only; never logged or stored. */
export function isDirty(initial: EditorSnapshot, current: EditorSnapshot): boolean {
  return JSON.stringify(initial) !== JSON.stringify(current);
}
