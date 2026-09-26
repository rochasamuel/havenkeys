import { describe, expect, it } from "vitest";
import { formatDate, groupCode, monogram, primaryHost, strengthLevel } from "./format";
import { toApiError } from "./api";
import type { ItemOverview } from "./types";

const item = (urls: string[]): ItemOverview => ({
  id: "1",
  itemType: "login",
  title: "GitHub",
  username: null,
  urls: urls.map((url) => ({ url, matchType: "domain" })),
  hasPassword: true,
  hasTotp: false,
  hasNotes: false,
  hasPasskey: false,
  autoSignIn: true,
  createdAt: 0,
  updatedAt: 0,
});

describe("format", () => {
  it("extracts the primary host", () => {
    expect(primaryHost(item(["https://www.github.com/login"]))).toBe("github.com");
    expect(primaryHost(item([]))).toBeNull();
    expect(primaryHost(item(["not a url"]))).toBeNull();
  });

  it("builds monograms", () => {
    expect(monogram("GitHub")).toBe("G");
    expect(monogram("bank of Mars")).toBe("B");
    // Punctuation is skipped, so "(work)" never becomes a monogram.
    expect(monogram("(work) Fernway")).toBe("W");
    expect(monogram("1Password")).toBe("1");
    expect(monogram("  ")).toBe("?");
  });

  it("groups codes", () => {
    expect(groupCode("381492")).toBe("381 492");
    expect(groupCode("38149275")).toBe("3814 9275");
  });

  it("labels strength", () => {
    expect(strengthLevel(40)).toBe("weak");
    expect(strengthLevel(60)).toBe("fair");
    expect(strengthLevel(100)).toBe("strong");
    expect(strengthLevel(150)).toBe("excellent");
  });

  it("formats dates in the UI's language", () => {
    const at = Date.UTC(2026, 8, 26, 12, 0);
    expect(formatDate(at, "en")).toContain("2026");
    expect(formatDate(at, "pt-BR")).toContain("de set. de 2026");
    // A regional tag keeps its own order: day before month in en-GB.
    expect(formatDate(at, "en-GB")).toMatch(/^26 Sept? 2026/);
    expect(formatDate(at, "en-US")).toMatch(/^Sep 26, 2026/);
  });
});

describe("toApiError", () => {
  it("keeps only code and message from core errors", () => {
    const e = toApiError({ code: "locked", message: "The vault is locked.", extra: "x" });
    expect(e.code).toBe("locked");
    expect(e.message).toBe("The vault is locked.");
  });

  it("never passes through unknown error text", () => {
    const e = toApiError("invalid args `password` = hunter2");
    expect(e.code).toBe("internal");
    expect(e.message).not.toContain("hunter2");
  });
});
