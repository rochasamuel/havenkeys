import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("i18n context", () => {
  it("runs the tests in English whatever the system locale", async () => {
    const { locale, t, en } = await import("./index");
    expect(locale).toBe("en");
    expect(t).toBe(en);
  });

  it("follows the browser UI language when the i18n API is there", async () => {
    vi.stubGlobal("chrome", { i18n: { getUILanguage: () => "pt-BR" } });
    const { locale, t, ptBR } = await import("./index");
    expect(locale).toBe("pt-BR");
    expect(t).toBe(ptBR);
  });

  it("falls back to navigator when the i18n API throws", async () => {
    vi.stubGlobal("chrome", {
      i18n: {
        getUILanguage: () => {
          throw new Error("no");
        },
      },
    });
    const { locale } = await import("./index");
    expect(locale).toBe("en");
  });
});
