import { describe, expect, it } from "vitest";
import { CARD_BRAND_ICONS, CARD_BRAND_IDS, GENERIC_CARD_ICON } from "./card-brand-icons";

describe("card brand icons", () => {
  it("has one icon per brand, paths and rects only, opaque hex colours", () => {
    expect([...CARD_BRAND_IDS].sort()).toEqual(
      ["amex", "diners", "discover", "elo", "hipercard", "jcb", "maestro", "mastercard", "unionpay", "visa"],
    );
    expect(Object.keys(CARD_BRAND_ICONS).sort()).toEqual([...CARD_BRAND_IDS].sort());
    for (const icon of [...Object.values(CARD_BRAND_ICONS), GENERIC_CARD_ICON]) {
      expect(icon.viewBox).toBe("0 0 780 500");
      expect(icon.shapes.length).toBeGreaterThan(0);
      for (const s of icon.shapes) {
        expect(Boolean(s.d) !== Boolean(s.rect)).toBe(true);
        // `e`: exponents such as 1e-3 appear in the source path data.
        if (s.d) expect(s.d).toMatch(/^[MmLlHhVvCcSsQqTtAaZz0-9.,\se-]+$/);
        expect(s.fill).toMatch(/^#[0-9A-F]{6}$/);
      }
    }
  });
});
