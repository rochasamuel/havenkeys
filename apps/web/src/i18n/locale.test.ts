import { describe, expect, it } from "vitest";
import { localeFromPath, localizePath, prefersPortuguese, stripLocale, switchLocalePath } from "./locale";

describe("locale paths", () => {
  it("reads the locale from the URL", () => {
    expect(localeFromPath("/")).toBe("en");
    expect(localeFromPath("/security")).toBe("en");
    expect(localeFromPath("/pt-br")).toBe("pt-BR");
    expect(localeFromPath("/pt-br/download")).toBe("pt-BR");
    expect(localeFromPath("/PT-BR/download")).toBe("pt-BR");
    // A path that merely starts with the letters is not the prefix.
    expect(localeFromPath("/pt-brazil")).toBe("en");
  });

  it("strips and adds the prefix", () => {
    expect(stripLocale("/pt-br")).toBe("/");
    expect(stripLocale("/pt-br/security")).toBe("/security");
    expect(stripLocale("/terms")).toBe("/terms");
    expect(localizePath("/", "pt-BR")).toBe("/pt-br");
    expect(localizePath("/security", "pt-BR")).toBe("/pt-br/security");
    expect(localizePath("/security", "en")).toBe("/security");
  });

  it("switches language on the same page and section", () => {
    expect(switchLocalePath("/", "#journey", "pt-BR")).toBe("/pt-br#journey");
    expect(switchLocalePath("/pt-br/download", "", "en")).toBe("/download");
    expect(switchLocalePath("/pt-br", "", "en")).toBe("/");
  });

  it("detects a Portuguese browser", () => {
    expect(prefersPortuguese(["pt-BR", "en"])).toBe(true);
    expect(prefersPortuguese(["pt"])).toBe(true);
    expect(prefersPortuguese(["en-US", "pt-BR"])).toBe(false);
    expect(prefersPortuguese([])).toBe(false);
  });
});
