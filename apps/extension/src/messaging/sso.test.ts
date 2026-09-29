import { describe, expect, it } from "vitest";
import { isAccount, parsePressReply, parseSsoBackgroundMessage, parseSsoContentRequest, parseSsoFrameRequest, parseSsoReady } from "./sso";

const T = "0123456789abcdef0123456789abcdef";
const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

describe("sso messages", () => {
  it("accepts exact shapes", () => {
    expect(parseSsoContentRequest({ type: "cs_sso_buttons", providers: ["google", "github"] })).toEqual({ type: "cs_sso_buttons", providers: ["google", "github"] });
    expect(parseSsoContentRequest({ type: "cs_sso_click", provider: "apple" })).not.toBeNull();
    expect(parseSsoContentRequest({ type: "cs_sso_account", account: "me@gmail.com" })).not.toBeNull();
    expect(parseSsoContentRequest({ type: "cs_sso_stop" })).not.toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_pick", token: T, itemId: ID })).not.toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_save", token: T, account: "", title: null })).not.toBeNull();
    // A GitHub username or phone number, not email-shaped: the user's own edit, Rust validates on save.
    expect(parseSsoFrameRequest({ type: "sso_save", token: T, account: "samuelrocha", title: null })).toEqual({
      type: "sso_save",
      token: T,
      account: "samuelrocha",
      title: null,
    });
    expect(parseSsoBackgroundMessage({ type: "bg_sso_press", provider: "google", origin: "https://accounts.google.com" })).toEqual({
      type: "bg_sso_press",
      provider: "google",
      origin: "https://accounts.google.com",
    });
    expect(parseSsoReady({ kind: "choose", account: "me@gmail.com" })).toEqual({ kind: "choose", account: "me@gmail.com" });
  });
  it("rejects everything else", () => {
    for (const bad of [
      { type: "cs_sso_buttons", providers: [] },
      { type: "cs_sso_buttons", providers: ["google", "google"] },
      { type: "cs_sso_buttons", providers: ["okta"] },
      { type: "cs_sso_click", provider: "google", extra: 1 },
      { type: "cs_sso_account", account: "not an email" },
      { type: "cs_sso_account", account: `${"a".repeat(250)}@b.co` },
    ]) expect(parseSsoContentRequest(bad)).toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_pick", token: "short", itemId: ID })).toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_save", token: T, account: "x".repeat(255), title: null })).toBeNull();
    // A control character in a user-edited account is rejected even though it is not email-shaped.
    expect(parseSsoFrameRequest({ type: "sso_save", token: T, account: "sam\u0000rocha", title: null })).toBeNull();
    expect(parseSsoFrameRequest({ type: "sso_resize", token: T, height: 9999 })).toBeNull();
    expect(parseSsoBackgroundMessage({ type: "bg_sso_press", provider: "google" })).toBeNull();
    expect(parseSsoBackgroundMessage({ type: "bg_sso_press", provider: "google", origin: 7 })).toBeNull();
    expect(parseSsoReady({ kind: "choose", account: 5 })).toBeNull();
    expect(parsePressReply(undefined)).toEqual({ pressed: false });
    expect(isAccount("me@x.io")).toBe(true);
    expect(isAccount("me@x.io\u0007")).toBe(false);
    expect(isAccount(`me${String.fromCodePoint(0x202e)}@x.io`)).toBe(false);
  });
  it("parses the login phase and cs_sso_login", () => {
    expect(parseSsoReady({ kind: "login" })).toEqual({ kind: "login" });
    expect(parseSsoReady({ kind: "login", account: "a@b.co" })).toBeNull();
    expect(parseSsoContentRequest({ type: "cs_sso_login" })).toEqual({ type: "cs_sso_login" });
    expect(parseSsoContentRequest({ type: "cs_sso_login", itemId: "x" })).toBeNull();
  });
});
