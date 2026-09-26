import { describe, expect, it } from "vitest";
import { resolveLocale } from "./locale";

describe("resolveLocale", () => {
  it("picks Brazilian Portuguese for any Portuguese tag", () => {
    expect(resolveLocale(["pt-BR"])).toBe("pt-BR");
    expect(resolveLocale(["pt"])).toBe("pt-BR");
    expect(resolveLocale(["PT-pt"])).toBe("pt-BR");
    expect(resolveLocale(["pt_BR"])).toBe("pt-BR");
  });

  it("falls back to English", () => {
    expect(resolveLocale(["en-US"])).toBe("en");
    expect(resolveLocale(["es"])).toBe("en");
    expect(resolveLocale([])).toBe("en");
  });

  it("takes the first Portuguese tag in the list", () => {
    expect(resolveLocale(["pt-BR", "en"])).toBe("pt-BR");
    expect(resolveLocale(["en-US", "pt-BR"])).toBe("pt-BR");
  });
});
