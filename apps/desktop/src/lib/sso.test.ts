import { describe, expect, it } from "vitest";
import type { ItemOverview } from "./types";
import { providerLogin, ssoSubtitle } from "./sso";

const base = { itemType: "login", hasPassword: true, hasTotp: false, hasNotes: false, hasPasskey: false, autoSignIn: true, createdAt: 0, updatedAt: 0 } as const;
const item = (id: string, over: Partial<ItemOverview>): ItemOverview => ({ id, title: id, username: null, urls: [], ...base, ...over });

describe("providerLogin", () => {
  const typeform = item("typeform", { urls: [{ url: "https://typeform.com/", matchType: "domain" }], signInWith: { provider: "google", account: "Me@gmail.com" } });
  it("finds the vault's login for the provider with the same account", () => {
    const google = item("google", { username: "me@gmail.com", urls: [{ url: "https://accounts.google.com/", matchType: "domain" }] });
    const other = item("other", { username: "you@gmail.com", urls: [{ url: "https://accounts.google.com/", matchType: "domain" }] });
    expect(providerLogin([typeform, other, google], typeform)?.id).toBe("google");
  });
  it("never matches look-alike hosts or a missing account", () => {
    const evil = item("evil", { username: "me@gmail.com", urls: [{ url: "https://google.com.evil.com/", matchType: "domain" }] });
    expect(providerLogin([typeform, evil], typeform)).toBeNull();
    const noAccount = { ...typeform, signInWith: { provider: "google" as const, account: null } };
    expect(providerLogin([noAccount], noAccount)).toBeNull();
  });
  it("formats the subtitle", () => {
    expect(ssoSubtitle({ provider: "github", account: null })).toBe("GitHub");
    expect(ssoSubtitle({ provider: "google", account: "me@gmail.com" })).toBe("Google · me@gmail.com");
  });
});
