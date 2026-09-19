import { describe, expect, it } from "vitest";
import { groupCode, monogram, primaryHost, strengthLabel } from "./format";
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
    expect(monogram("GitHub")).toBe("GI");
    expect(monogram("Bank of Mars")).toBe("BO");
    expect(monogram("  ")).toBe("?");
  });

  it("groups codes", () => {
    expect(groupCode("381492")).toBe("381 492");
    expect(groupCode("38149275")).toBe("3814 9275");
  });

  it("labels strength", () => {
    expect(strengthLabel(40)).toBe("Weak");
    expect(strengthLabel(150)).toBe("Excellent");
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
