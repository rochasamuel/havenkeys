import { describe, expect, it } from "vitest";
import { parseBackgroundMessage, parseInlineRequest } from "./inline";

const TOKEN = "a".repeat(32);
const OPTIONS = { length: 32, uppercase: true, lowercase: true, digits: true, symbols: false, avoidAmbiguous: false };

describe("menu_generate options", () => {
  it("accepts no options or a valid policy", () => {
    expect(parseInlineRequest({ type: "menu_generate", token: TOKEN })).toEqual({ type: "menu_generate", token: TOKEN });
    expect(parseInlineRequest({ type: "menu_generate", token: TOKEN, options: OPTIONS })).toEqual({ type: "menu_generate", token: TOKEN, options: OPTIONS });
  });

  it("rejects out-of-range lengths, no classes, wrong types and extra keys", () => {
    for (const options of [
      { ...OPTIONS, length: 7 },
      { ...OPTIONS, length: 129 },
      { ...OPTIONS, length: 20.5 },
      { ...OPTIONS, length: "20" },
      { ...OPTIONS, uppercase: false, lowercase: false, digits: true, symbols: false, digits2: true },
      { length: 20, uppercase: false, lowercase: false, digits: false, symbols: false, avoidAmbiguous: false },
      { ...OPTIONS, symbols: "yes" },
      { ...OPTIONS, avoidAmbiguous: 1 },
      { length: 20, uppercase: true },
      null,
      [],
    ]) {
      expect(parseInlineRequest({ type: "menu_generate", token: TOKEN, options })).toBeNull();
    }
  });
});

describe("menu_generator_options", () => {
  it("takes only the token", () => {
    expect(parseInlineRequest({ type: "menu_generator_options", token: TOKEN })).toEqual({ type: "menu_generator_options", token: TOKEN });
    expect(parseInlineRequest({ type: "menu_generator_options", token: TOKEN, options: OPTIONS })).toBeNull();
  });
});

describe("animated menu resize", () => {
  it("passes animate: true and nothing else", () => {
    expect(parseInlineRequest({ type: "menu_resize", token: TOKEN, height: 200, animate: true })).toEqual({ type: "menu_resize", token: TOKEN, height: 200, animate: true });
    expect(parseInlineRequest({ type: "menu_resize", token: TOKEN, height: 200, animate: false })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_resize_menu", token: TOKEN, height: 200, animate: true })).toEqual({ type: "bg_resize_menu", token: TOKEN, height: 200, animate: true });
    expect(parseBackgroundMessage({ type: "bg_resize_menu", token: TOKEN, height: 200, animate: 1 })).toBeNull();
  });
});
