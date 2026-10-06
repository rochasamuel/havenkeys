import { describe, expect, it } from "vitest";
import { EMPTY, KEEP } from "./secretEdit";
import { shouldCollapse, ssoSubtitle } from "./sso";

describe("ssoSubtitle", () => {
  it("formats the subtitle", () => {
    expect(ssoSubtitle({ provider: "github", account: null })).toBe("GitHub");
    expect(ssoSubtitle({ provider: "google", account: "me@gmail.com" })).toBe("Google · me@gmail.com");
  });
});

describe("shouldCollapse", () => {
  it("collapses an untouched, empty password on an item that never had one", () => {
    expect(shouldCollapse("", EMPTY, false)).toBe(true);
  });
  it("never collapses a typed username", () => {
    expect(shouldCollapse("alice", EMPTY, false)).toBe(false);
    expect(shouldCollapse("  alice  ", EMPTY, false)).toBe(false);
  });
  it("never collapses a typed password", () => {
    expect(shouldCollapse("", { mode: "set", value: "hunter2" }, false)).toBe(false);
  });
  it("never collapses an existing password, kept or otherwise", () => {
    expect(shouldCollapse("", KEEP, true)).toBe(false);
    expect(shouldCollapse("", { mode: "clear" }, true)).toBe(false);
    expect(shouldCollapse("", EMPTY, true)).toBe(false);
  });
});
