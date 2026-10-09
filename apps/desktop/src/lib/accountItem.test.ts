import { describe, expect, it } from "vitest";
import { showAccountItem } from "./accountItem";
import type { AccountStatus } from "./types";

const account: AccountStatus = {
  email: "Me@Example.com",
  serverUrl: "https://vault.example.com:8443",
  accountId: "3f1c0000-0000-0000-0000-000000000000",
  online: true,
  lastSyncedAt: null,
  planStatus: null,
  entitlement: "full",
  trialEndsAt: null,
  periodEnd: null,
};

describe("showAccountItem", () => {
  it("shows in All items with no query", () => {
    expect(showAccountItem(account, "all", "")).toBe(true);
    expect(showAccountItem(account, "all", "   ")).toBe(true);
  });

  it("never shows in the other sections or without an account", () => {
    expect(showAccountItem(account, "login", "")).toBe(false);
    expect(showAccountItem(account, "secure_note", "")).toBe(false);
    expect(showAccountItem(null, "all", "")).toBe(false);
  });

  it("matches havenkeys, the email and the server host, ignoring case", () => {
    for (const q of ["haven", "HavenKeys", "me@example", "EXAMPLE.COM", "vault.example", "account"]) {
      expect(showAccountItem(account, "all", q), q).toBe(true);
    }
  });

  it("matches the translated title", () => {
    expect(showAccountItem(account, "all", "conta", "Conta HavenKeys")).toBe(true);
  });

  it("does not match unrelated text, the port or the scheme", () => {
    for (const q of ["github", "8443", "https", "3f1c"]) {
      expect(showAccountItem(account, "all", q), q).toBe(false);
    }
  });
});
