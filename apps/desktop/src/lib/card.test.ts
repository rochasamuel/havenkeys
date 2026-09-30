import { describe, expect, it } from "vitest";
import { capDigits, cardSecretUpdate, cardSubtitle, digitsOnly, isDerivedTitle, formatExpiry, groupNumber, isExpired, maskedNumber, parseExpiryInput } from "./card";

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

describe("editor payloads", () => {
  it("caps pasted digits after the separators are stripped", () => {
    expect(capDigits("4111 - 1111 - 1111 - 1111", 19)).toBe("4111111111111111");
    expect(capDigits("1".repeat(30), 19)).toHaveLength(19);
  });
  it("keeps an untouched secret", () => {
    expect(cardSecretUpdate("", false)).toEqual({ op: "keep" });
    expect(cardSecretUpdate("   ", false)).toEqual({ op: "keep" });
  });
  it("sets a typed number as digits", () => {
    expect(cardSecretUpdate("5200 8282 8282 8210", false, digitsOnly)).toEqual({ op: "set", value: "5200828282828210" });
  });
  it("sets a typed code trimmed", () => {
    expect(cardSecretUpdate(" 123 ", false)).toEqual({ op: "set", value: "123" });
  });
  it("clears when removed, whatever was typed", () => {
    expect(cardSecretUpdate("", true)).toEqual({ op: "clear" });
    expect(cardSecretUpdate("5200", true, digitsOnly)).toEqual({ op: "clear" });
  });
});

describe("derived titles", () => {
  it("recognises the brand name Rust stored", () => {
    expect(isDerivedTitle("Mastercard", "mastercard")).toBe(true);
    expect(isDerivedTitle("American Express", "amex")).toBe(true);
    expect(isDerivedTitle("Card", "other")).toBe(true);
  });
  it("keeps a title the user chose", () => {
    expect(isDerivedTitle("Nubank", "mastercard")).toBe(false);
    expect(isDerivedTitle("Visa", "mastercard")).toBe(false);
    expect(isDerivedTitle("Mastercard", "other")).toBe(false);
  });
});
