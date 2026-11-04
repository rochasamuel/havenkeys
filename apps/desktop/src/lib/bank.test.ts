import { describe, expect, it } from "vitest";
import { bankSubtitle, joinDigit, splitPasted } from "./bank";

describe("splitPasted", () => {
  it("splits a trailing check digit", () => {
    expect(splitPasted("12345-6")).toEqual({ number: "12345", digit: "6" });
    expect(splitPasted(" 12.345 - x ")).toEqual({ number: "12345", digit: "X" });
  });
  it("leaves a plain number alone", () => {
    expect(splitPasted("1234")).toEqual({ number: "1234", digit: null });
    expect(splitPasted("")).toEqual({ number: "", digit: null });
  });
});

describe("joinDigit", () => {
  it("joins with a dash, or not at all", () => {
    expect(joinDigit("12345", "6")).toBe("12345-6");
    expect(joinDigit("1234", null)).toBe("1234");
    expect(joinDigit("1234", "")).toBe("1234");
  });
});

describe("bankSubtitle", () => {
  it("joins the parts present", () => {
    expect(bankSubtitle({ bank: "Itaú", branch: "1234", accountLast3: "45-6" })).toBe("Branch 1234 · Account •••45-6");
    expect(bankSubtitle({ bank: "Itaú", branch: "1234", accountLast3: null })).toBe("Branch 1234");
  });
  it("falls back to the bank's name, then nothing", () => {
    expect(bankSubtitle({ bank: "Itaú", branch: null, accountLast3: null })).toBe("Itaú");
    expect(bankSubtitle({ bank: null, branch: null, accountLast3: null })).toBe("");
  });
  it("takes the caller's labels", () => {
    expect(bankSubtitle({ bank: null, branch: "1", accountLast3: "45-6" }, { ag: "Ag", account: "Conta" })).toBe("Ag 1 · Conta •••45-6");
  });
});
