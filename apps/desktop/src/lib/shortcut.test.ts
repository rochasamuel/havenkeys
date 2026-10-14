import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { en } from "../i18n/en";
import { formatChord, KEY_NAMES, KEY_REFUSED, recordKey, type KeyPress } from "./shortcut";

const press = (code: string, mods: Partial<KeyPress> = {}): KeyPress => ({
  code,
  key: code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  repeat: false,
  isComposing: false,
  ...mods,
});

describe("recordKey", () => {
  it("builds the chord in Rust's order from the physical key", () => {
    expect(recordKey(press("KeyK", { ctrlKey: true, shiftKey: true }))).toEqual({ kind: "chord", keys: "Control+Shift+K" });
    expect(recordKey(press("Space", { altKey: true, shiftKey: true }))).toEqual({ kind: "chord", keys: "Alt+Shift+Space" });
    expect(recordKey(press("Digit1", { metaKey: true, ctrlKey: true, altKey: true, shiftKey: true }))).toEqual({
      kind: "chord",
      keys: "Super+Control+Alt+Shift+1",
    });
    expect(recordKey(press("Slash", { metaKey: true }))).toEqual({ kind: "chord", keys: "Super+Slash" });
    // No modifier still goes to Rust, which says what is missing.
    expect(recordKey(press("KeyK"))).toEqual({ kind: "chord", keys: "K" });
  });

  it("uses the physical key, not the character", () => {
    // AZERTY: the key labelled A sits where QWERTY has Q.
    expect(recordKey(press("KeyQ", { key: "a", ctrlKey: true }))).toEqual({ kind: "chord", keys: "Control+Q" });
    // Option on macOS changes the character; the code does not.
    expect(recordKey(press("KeyK", { key: "˚", altKey: true }))).toEqual({ kind: "chord", keys: "Alt+K" });
  });

  it("waits on modifiers alone, repeats and composition", () => {
    for (const code of ["ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight", "OSLeft", "CapsLock"]) {
      expect(recordKey(press(code, { ctrlKey: true })), code).toEqual({ kind: "wait" });
    }
    expect(recordKey(press("AltRight", { key: "AltGraph" }))).toEqual({ kind: "wait" });
    expect(recordKey(press("KeyK", { ctrlKey: true, repeat: true }))).toEqual({ kind: "wait" });
    expect(recordKey(press("KeyK", { ctrlKey: true, isComposing: true }))).toEqual({ kind: "wait" });
  });

  it("cancels on Esc, with or without modifiers", () => {
    expect(recordKey(press("Escape", { key: "Escape" }))).toEqual({ kind: "cancel" });
    expect(recordKey(press("Escape", { key: "Escape", ctrlKey: true }))).toEqual({ kind: "cancel" });
  });

  it("knows keys Rust would refuse", () => {
    for (const code of ["Tab", "Enter", "Backspace", "Delete", "ArrowUp", "F13", "Numpad1", "IntlBackslash", "toString", ""]) {
      expect(recordKey(press(code, { ctrlKey: true })), code).toEqual({ kind: "unusable" });
    }
  });
});

describe("formatChord", () => {
  it("shows macOS symbols", () => {
    expect(formatChord("Super+Shift+Space", "mac")).toBe("⌘⇧Space");
    expect(formatChord("Super+Control+Alt+Shift+K", "mac")).toBe("⌘⌃⌥⇧K");
    expect(formatChord("Alt+Backquote", "mac")).toBe("⌥`");
  });

  it("spells modifiers out elsewhere", () => {
    expect(formatChord("Control+Shift+Space", "other")).toBe("Ctrl+Shift+Space");
    expect(formatChord("Super+Shift+K", "windows")).toBe("Win+Shift+K");
    expect(formatChord("Super+Shift+K", "other")).toBe("Super+Shift+K");
    expect(formatChord("Control+Minus", "windows")).toBe("Ctrl+-");
    expect(formatChord("Alt+Backslash", "other")).toBe("Alt+\\");
  });
});

describe("the key table", () => {
  it("is Rust's table, entry for entry", () => {
    // quick_access.rs ALLOWED_KEYS: `(Code::KeyA, "A")`, where the variant
    // is named as the DOM's KeyboardEvent.code.
    const rust = readFileSync(fileURLToPath(new URL("../../src-tauri/src/quick_access.rs", import.meta.url)), "utf8");
    const table = /const ALLOWED_KEYS: &\[\(Code, &str\)\] = &\[([\s\S]*?)\];/.exec(rust)?.[1];
    expect(table, "ALLOWED_KEYS no longer reads the way this test parses it").toBeTruthy();
    const pairs = Array.from((table ?? "").matchAll(/\(Code::(\w+), "([^"]+)"\)/g), (m) => [m[1], m[2]]);
    expect(pairs.length).toBe(60);
    expect(Object.entries(KEY_NAMES).sort()).toEqual(pairs.sort());
  });

  it("names a refusal Rust also sends", () => {
    expect(en.errors.shortcutRefused[KEY_REFUSED]).toBe(KEY_REFUSED);
  });
});
