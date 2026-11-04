import { describe, expect, it } from "vitest";
import { parseBackgroundMessage, parseContentRequest } from "./inline";

const TOKEN = "a".repeat(32);
const values = { branch: "1234", branchDigit: "5", account: "12345", accountDigit: "6" };

describe("bank messages", () => {
  it("cs_open_menu accepts the bank kind, and a password box only on the login menu", () => {
    expect(parseContentRequest({ type: "cs_open_menu", kind: "bank" })).toEqual({ type: "cs_open_menu", kind: "bank" });
    expect(parseContentRequest({ type: "cs_open_menu", kind: "bank", explicit: true })).toEqual({ type: "cs_open_menu", kind: "bank", explicit: true });
    expect(parseContentRequest({ type: "cs_open_menu", kind: "login", passwordBox: true })).toEqual({ type: "cs_open_menu", kind: "login", passwordBox: true });
    for (const bad of [
      { type: "cs_open_menu", kind: "banks" },
      { type: "cs_open_menu", kind: "bankAccount" },
      { type: "cs_open_menu", kind: "bank", roles: ["cpf"] },
      { type: "cs_open_menu", kind: "bank", cardRoles: ["number"] },
      { type: "cs_open_menu", kind: "bank", branch: "1234" },
      { type: "cs_open_menu", kind: "bank", passwordBox: true },
      { type: "cs_open_menu", kind: "login", passwordBox: false },
      { type: "cs_open_menu", kind: "otp", passwordBox: true },
    ]) {
      expect(parseContentRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("bank fills from the background are strictly shaped and never submit", () => {
    const fill = { type: "bg_fill", origin: "https://banco.com.br", token: TOKEN, fill: { kind: "bank", values, password: "pw" }, submit: false, totp: false };
    expect(parseBackgroundMessage(fill)).toEqual(fill);
    const noPassword = { ...fill, fill: { kind: "bank", values: { ...values, branchDigit: null, accountDigit: "X" }, password: null } };
    expect(parseBackgroundMessage(noPassword)).toEqual(noPassword);
    // Either number may be absent (null), with its digit.
    for (const v of [
      { ...values, branch: null, branchDigit: null },
      { ...values, account: null, accountDigit: null },
    ]) {
      const one = { ...fill, fill: { kind: "bank", values: v, password: "pw" } };
      expect(parseBackgroundMessage(one), JSON.stringify(v)).toEqual(one);
    }
    for (const bad of [
      { ...fill, submit: true },
      { ...fill, totp: true },
      { ...fill, fill: { kind: "bank", values } },
      { ...fill, fill: { kind: "bank", values, password: "pw", username: "x" } },
      { ...fill, fill: { kind: "bank", values: { ...values, extra: "1" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, branch: "12a4" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, branch: "123456789" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, account: "1".repeat(21) }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, branchDigit: "55" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, accountDigit: "x" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, accountDigit: "123" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { branch: null, branchDigit: null, account: null, accountDigit: null }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, branch: null }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, account: null }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, branch: null, branchDigit: null, account: "1".repeat(21) }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values: { ...values, account: null, accountDigit: null, branch: "123456789" }, password: "pw" } },
      { ...fill, fill: { kind: "bank", values, password: "x".repeat(4097) } },
      { ...fill, fill: { kind: "bank", values, password: 7 } },
      { ...fill, fill: { kind: "banks", values, password: "pw" } },
    ]) {
      expect(parseBackgroundMessage(bad), JSON.stringify(bad)).toBeNull();
    }
  });
});
