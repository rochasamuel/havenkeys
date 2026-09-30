import { describe, expect, it } from "vitest";
import { cardSubtitle, digitsOnly, formatExpiry, groupNumber, isExpired, maskedNumber, parseExpiryInput } from "./card";

describe("numbers", () => {
  it("keeps digits only", () => {
    expect(digitsOnly(" 5200 8282-8282 8210 ")).toBe("5200828282828210");
  });
  it("groups by brand", () => {
    expect(groupNumber("5200828282828210", "mastercard")).toBe("5200 8282 8282 8210");
    expect(groupNumber("378282246310005", "amex")).toBe("3782 822463 10005");
    expect(groupNumber("36227206271667", "diners")).toBe("3622 720627 1667");
    expect(groupNumber("3056930009020004", "diners")).toBe("3056 9300 0902 0004");
    expect(groupNumber("52008", "other")).toBe("5200 8");
  });
  it("masks with the last four", () => {
    expect(maskedNumber("7609")).toBe("•••• •••• •••• 7609");
    expect(maskedNumber(null)).toBe("••••");
  });
});

describe("expiry", () => {
  it("formats and parses", () => {
    expect(formatExpiry("2033-11")).toBe("11/2033");
    expect(parseExpiryInput("11/2033")).toEqual({ ok: true, value: "2033-11" });
    expect(parseExpiryInput(" 3 / 33 ")).toEqual({ ok: true, value: "2033-03" });
    expect(parseExpiryInput("")).toEqual({ ok: true, value: null });
    for (const bad of ["13/2030", "00/30", "11-2033", "11/203", "abc"]) {
      expect(parseExpiryInput(bad).ok, bad).toBe(false);
    }
  });
  it("is expired after its month", () => {
    const sept2026 = new Date(2026, 8, 29);
    expect(isExpired("2026-09", sept2026)).toBe(false);
    expect(isExpired("2026-08", sept2026)).toBe(true);
    expect(isExpired("2025-12", sept2026)).toBe(true);
    expect(isExpired("2033-11", sept2026)).toBe(false);
  });
});

describe("cardSubtitle", () => {
  it("joins what exists", () => {
    expect(cardSubtitle({ brand: "visa", last4: "7609", expiry: "2033-11" })).toBe("•••• 7609 · 11/2033");
    expect(cardSubtitle({ brand: "visa", last4: null, expiry: "2033-11" })).toBe("11/2033");
    expect(cardSubtitle({ brand: "other", last4: null, expiry: null })).toBeNull();
    expect(cardSubtitle(undefined)).toBeNull();
  });
});
