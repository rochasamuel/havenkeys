import { describe, expect, it } from "vitest";
import { PROVIDER_ICONS } from "./provider-icons";

describe("provider icons", () => {
  it("has one icon per provider, paths only", () => {
    expect(Object.keys(PROVIDER_ICONS).sort()).toEqual(["apple", "github", "google", "microsoft"]);
    for (const icon of Object.values(PROVIDER_ICONS)) {
      expect(icon.viewBox).toMatch(/^0 0 \d+ \d+$/);
      for (const s of icon.shapes) {
        expect(Boolean(s.d) !== Boolean(s.rect)).toBe(true);
        if (s.d) expect(s.d).toMatch(/^[MmLlHhVvCcSsQqTtAaZz0-9.,\s-]+$/);
        expect(s.fill).toMatch(/^(#[0-9A-F]{6}|currentColor)$/);
      }
    }
  });
});
