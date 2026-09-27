import { describe, expect, it } from "vitest";

import { EMPTY, KEEP, canScan, scanLabel, toUpdate } from "./secretEdit";

describe("toUpdate", () => {
  it("maps each edit to what Rust expects", () => {
    expect(toUpdate(KEEP)).toEqual({ op: "keep" });
    expect(toUpdate({ mode: "clear" })).toEqual({ op: "clear" });
    expect(toUpdate({ mode: "set", value: "abc" })).toEqual({ op: "set", value: "abc" });
    expect(toUpdate(EMPTY)).toEqual({ op: "clear" });
    expect(toUpdate({ mode: "scanned", token: "t0k", label: "GitHub" })).toEqual({ op: "scanned", value: "t0k" });
  });
});

describe("scanLabel", () => {
  it("joins issuer and account, skipping what is missing", () => {
    expect(scanLabel({ token: "t", issuer: "GitHub", account: "alice" }, "Unnamed")).toBe("GitHub · alice");
    expect(scanLabel({ token: "t", issuer: null, account: "alice" }, "Unnamed")).toBe("alice");
    expect(scanLabel({ token: "t", issuer: " ", account: null }, "Unnamed")).toBe("Unnamed");
  });
});

describe("canScan", () => {
  it("offers the QR button only while the field is empty", () => {
    expect(canScan(EMPTY)).toBe(true);
    expect(canScan({ mode: "set", value: "JBSW" })).toBe(false);
    expect(canScan(KEEP)).toBe(false);
    expect(canScan({ mode: "clear" })).toBe(false);
    expect(canScan({ mode: "scanned", token: "t", label: "x" })).toBe(false);
  });
});
