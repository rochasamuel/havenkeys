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
