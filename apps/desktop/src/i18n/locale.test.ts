import { afterEach, describe, expect, it, vi } from "vitest";
import { dateLocale, effectiveLocale, resolveLocale } from "./locale";
import { readPreference, writePreference } from "./preference";

describe("resolveLocale", () => {
  it("lets the first tag in English or Portuguese decide", () => {
    expect(resolveLocale(["en-US", "pt-BR"])).toBe("en");
    expect(resolveLocale(["pt-BR", "en"])).toBe("pt-BR");
    expect(resolveLocale(["es", "pt-BR"])).toBe("pt-BR");
    expect(resolveLocale(["fr", "en-GB", "pt"])).toBe("en");
    expect(resolveLocale(["fr"])).toBe("en");
  });

  it("matches the language subtag, not a prefix", () => {
    expect(resolveLocale(["ptx", "pt-BR"])).toBe("pt-BR");
    expect(resolveLocale(["eng", "pt"])).toBe("pt-BR");
  });

  it("picks Brazilian Portuguese for any Portuguese tag", () => {
    expect(resolveLocale(["pt-BR"])).toBe("pt-BR");
    expect(resolveLocale(["pt"])).toBe("pt-BR");
    expect(resolveLocale(["PT-pt"])).toBe("pt-BR");
  });

  it("falls back to English", () => {
    expect(resolveLocale([])).toBe("en");
    expect(resolveLocale(["en-GB", "fr"])).toBe("en");
    expect(resolveLocale(["es-419"])).toBe("en");
  });

  it("lets an explicit preference win over the OS", () => {
    expect(effectiveLocale("auto", ["pt-BR"])).toBe("pt-BR");
    expect(effectiveLocale("auto", ["en-US"])).toBe("en");
    expect(effectiveLocale("en", ["pt-BR"])).toBe("en");
    expect(effectiveLocale("pt-BR", ["en-US"])).toBe("pt-BR");
  });
});

describe("dateLocale", () => {
  it("keeps the OS's regional format when it speaks the UI's language", () => {
    expect(dateLocale("en", ["en-GB", "pt-BR"])).toBe("en-GB");
    expect(dateLocale("pt-BR", ["pt-PT"])).toBe("pt-PT");
    expect(dateLocale("en", ["EN_au"])).toBe("en-AU");
  });

  it("uses the UI locale otherwise", () => {
    // Only the first OS tag counts: the one the OS formats dates with.
    expect(dateLocale("en", ["fr-FR", "en-GB"])).toBe("en");
    expect(dateLocale("pt-BR", ["en-US"])).toBe("pt-BR");
    expect(dateLocale("en", [])).toBe("en");
    expect(dateLocale("en", ["en-!!"])).toBe("en");
  });
});

describe("the language preference", () => {
  afterEach(() => vi.unstubAllGlobals());

  function stubStorage(initial: Record<string, string> = {}) {
    const data = new Map(Object.entries(initial));
    const storage = {
      getItem: (k: string) => data.get(k) ?? null,
      setItem: (k: string, v: string) => void data.set(k, v),
      removeItem: (k: string) => void data.delete(k),
    };
    vi.stubGlobal("window", { localStorage: storage });
    return data;
  }

  it("reads a stored choice", () => {
    stubStorage({ "hk-locale": "pt-BR" });
    expect(readPreference()).toBe("pt-BR");
  });

  it("treats a missing or invalid value as automatic", () => {
    stubStorage();
    expect(readPreference()).toBe("auto");
    stubStorage({ "hk-locale": "fr" });
    expect(readPreference()).toBe("auto");
  });

  it("treats unavailable storage as automatic", () => {
    vi.stubGlobal("window", {
      get localStorage(): Storage {
        throw new Error("blocked");
      },
    });
    expect(readPreference()).toBe("auto");
    expect(() => writePreference("en")).not.toThrow();
  });

  it("round-trips a choice, and forgets it for automatic", () => {
    const data = stubStorage();
    writePreference("pt-BR");
    expect(readPreference()).toBe("pt-BR");
    writePreference("auto");
    expect(data.has("hk-locale")).toBe(false);
    expect(readPreference()).toBe("auto");
  });
});
