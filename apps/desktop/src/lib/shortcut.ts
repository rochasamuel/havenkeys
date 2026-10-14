/*
 * The Quick Access shortcut recorder's pure half: a key press becomes a
 * chord in Rust's form ("Super+Control+Alt+Shift+Key", that order), and a
 * chord is shown with this platform's symbols. Rust (quick_access.rs
 * parse_keys) checks every proposal again; this only spares a round trip
 * for keys it would never take.
 */

/** KeyboardEvent.code → the key's name in a chord. Rust's ALLOWED_KEYS, entry for entry (shortcut.test.ts). */
export const KEY_NAMES: Readonly<Record<string, string>> = {
  KeyA: "A", KeyB: "B", KeyC: "C", KeyD: "D", KeyE: "E", KeyF: "F", KeyG: "G",
  KeyH: "H", KeyI: "I", KeyJ: "J", KeyK: "K", KeyL: "L", KeyM: "M", KeyN: "N",
  KeyO: "O", KeyP: "P", KeyQ: "Q", KeyR: "R", KeyS: "S", KeyT: "T", KeyU: "U",
  KeyV: "V", KeyW: "W", KeyX: "X", KeyY: "Y", KeyZ: "Z",
  Digit0: "0", Digit1: "1", Digit2: "2", Digit3: "3", Digit4: "4",
  Digit5: "5", Digit6: "6", Digit7: "7", Digit8: "8", Digit9: "9",
  Space: "Space",
  F1: "F1", F2: "F2", F3: "F3", F4: "F4", F5: "F5", F6: "F6",
  F7: "F7", F8: "F8", F9: "F9", F10: "F10", F11: "F11", F12: "F12",
  Backquote: "Backquote", Minus: "Minus", Equal: "Equal",
  BracketLeft: "BracketLeft", BracketRight: "BracketRight", Backslash: "Backslash",
  Semicolon: "Semicolon", Quote: "Quote", Comma: "Comma", Period: "Period", Slash: "Slash",
};

/** Rust's message for a key it never takes; shown without asking it. */
export const KEY_REFUSED = "This key can't be used.";

const MODIFIER_CODES = new Set([
  "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight",
  "MetaLeft", "MetaRight", "OSLeft", "OSRight", "CapsLock", "Fn", "FnLock",
]);
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "AltGraph", "Meta", "OS", "CapsLock", "Fn", "FnLock"]);

/** The fields of a KeyboardEvent the recorder reads. */
export interface KeyPress {
  code: string;
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  repeat: boolean;
  isComposing: boolean;
}

export type Recorded =
  /** A modifier alone, a held key repeating, or an input method composing. */
  | { kind: "wait" }
  | { kind: "cancel" }
  /** A key Rust never takes (Tab, Enter, arrows, keypad…). */
  | { kind: "unusable" }
  | { kind: "chord"; keys: string };

export function recordKey(e: KeyPress): Recorded {
  if (e.repeat || e.isComposing) return { kind: "wait" };
  if (e.code === "Escape" || e.key === "Escape") return { kind: "cancel" };
  if (MODIFIER_CODES.has(e.code) || MODIFIER_KEYS.has(e.key)) return { kind: "wait" };
  const key = Object.hasOwn(KEY_NAMES, e.code) ? KEY_NAMES[e.code] : undefined;
  if (key === undefined) return { kind: "unusable" };
  const mods: string[] = [];
  if (e.metaKey) mods.push("Super");
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  return { kind: "chord", keys: [...mods, key].join("+") };
}

export type ShortcutPlatform = "mac" | "windows" | "other";

/** main.tsx marks macOS on <html>; Windows is read from the user agent. */
export function shortcutPlatform(): ShortcutPlatform {
  if (document.documentElement.dataset.platform === "mac") return "mac";
  return /Windows/.test(navigator.userAgent) ? "windows" : "other";
}

const KEY_SYMBOLS: Readonly<Record<string, string>> = {
  Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]",
  Backslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/",
};
const MAC_MODIFIERS: Readonly<Record<string, string>> = { Super: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧" };

export function formatChord(keys: string, platform: ShortcutPlatform): string {
  const parts = keys.split("+");
  const last = parts.pop() ?? "";
  const key = Object.hasOwn(KEY_SYMBOLS, last) ? (KEY_SYMBOLS[last] ?? last) : last;
  if (platform === "mac") return parts.map((m) => MAC_MODIFIERS[m] ?? m).join("") + key;
  const names: Record<string, string> = { Super: platform === "windows" ? "Win" : "Super", Control: "Ctrl", Alt: "Alt", Shift: "Shift" };
  return [...parts.map((m) => names[m] ?? m), key].join("+");
}
