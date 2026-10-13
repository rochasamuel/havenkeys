import { describe, expect, it } from "vitest";
import { quickKeyAction, type KeyLike } from "./keys";

const press = (key: string, mods: Partial<KeyLike> = {}): KeyLike => ({
  key,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  isComposing: false,
  ...mods,
});

describe("quickKeyAction", () => {
  it("maps the plain keys", () => {
    for (const mac of [true, false]) {
      expect(quickKeyAction(press("Enter"), mac)).toBe("password");
      expect(quickKeyAction(press("Escape"), mac)).toBe("hide");
      expect(quickKeyAction(press("ArrowUp"), mac)).toBe("up");
      expect(quickKeyAction(press("ArrowDown"), mac)).toBe("down");
      expect(quickKeyAction(press("a"), mac)).toBeNull();
    }
  });

  it("uses Cmd on macOS and Ctrl elsewhere", () => {
    expect(quickKeyAction(press("c", { metaKey: true }), true)).toBe("username");
    expect(quickKeyAction(press("t", { metaKey: true }), true)).toBe("totp");
    expect(quickKeyAction(press("o", { metaKey: true }), true)).toBe("open");
    expect(quickKeyAction(press("c", { ctrlKey: true }), false)).toBe("username");
    expect(quickKeyAction(press("T", { ctrlKey: true }), false)).toBe("totp");
    expect(quickKeyAction(press("o", { ctrlKey: true }), false)).toBe("open");
    // The other platform's key does nothing.
    expect(quickKeyAction(press("c", { ctrlKey: true }), true)).toBeNull();
    expect(quickKeyAction(press("c", { metaKey: true }), false)).toBeNull();
  });

  it("ignores extra modifiers and composition", () => {
    expect(quickKeyAction(press("c", { ctrlKey: true, shiftKey: true }), false)).toBeNull();
    expect(quickKeyAction(press("c", { ctrlKey: true, altKey: true }), false)).toBeNull();
    expect(quickKeyAction(press("c", { ctrlKey: true, metaKey: true }), false)).toBeNull();
    expect(quickKeyAction(press("Enter", { altKey: true }), false)).toBeNull();
    expect(quickKeyAction(press("Enter", { shiftKey: true }), false)).toBeNull();
    expect(quickKeyAction(press("Enter", { ctrlKey: true }), false)).toBeNull();
    expect(quickKeyAction(press("Enter", { isComposing: true }), false)).toBeNull();
    expect(quickKeyAction(press("Escape", { isComposing: true }), true)).toBeNull();
  });
});
