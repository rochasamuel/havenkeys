import { describe, expect, it } from "vitest";
import { isSsoProvider, providersForOrigin, SSO_PROVIDER_IDS } from "./sso";

describe("sso providers", () => {
  it("accepts the nine wire names and nothing else", () => {
    expect([...SSO_PROVIDER_IDS].sort()).toEqual(["apple", "discord", "facebook", "github", "gitlab", "google", "linkedin", "microsoft", "x"]);
    for (const p of ["x", "linkedin", "gitlab", "discord", "facebook"]) expect(isSsoProvider(p)).toBe(true);
    for (const p of ["X", "twitter", "okta", "", "__proto__", 1, null]) expect(isSsoProvider(p)).toBe(false);
  });
  it("maps exact origins only", () => {
    expect(providersForOrigin("https://x.com")).toEqual(["x"]);
    expect(providersForOrigin("https://twitter.com")).toEqual([]);
    expect(providersForOrigin("https://www.facebook.com")).toEqual(["facebook"]);
    expect(providersForOrigin("https://facebook.com")).toEqual([]);
    expect(providersForOrigin("https://gitlab.com.evil.com")).toEqual([]);
  });
});
