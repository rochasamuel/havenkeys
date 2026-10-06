import { describe, expect, it } from "vitest";
import { EMPTY, KEEP } from "./secretEdit";
import { applyRow, pickerRows, shouldCollapse, ssoSubtitle, type PickerRow } from "./sso";
import type { SsoAccount } from "./types";

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

describe("pickerRows", () => {
  const g1: SsoAccount = { id: "g1", title: "google.com", username: "samuelsilv.rocha@gmail.com" };
  const gh: SsoAccount = { id: "gh", title: "Github", username: "rochasamuel" };
  const accounts = { google: [g1], github: [gh] };
  const label = (r: PickerRow) => (r.kind === "none" ? "none" : r.kind === "provider" ? r.provider : `${r.provider}:${r.account.id}`);

  it("lists every provider in order, its saved logins under it, then None", () => {
    expect(pickerRows(accounts, "").map(label)).toEqual([
      "apple", "discord", "facebook", "github", "github:gh", "gitlab", "google", "google:g1", "linkedin", "microsoft", "x", "none",
    ]);
  });
  it("filters by provider name, title and username, case-insensitively", () => {
    expect(pickerRows(accounts, "GOO").map(label)).toEqual(["google", "google:g1"]);
    expect(pickerRows(accounts, "rochasam").map(label)).toEqual(["github", "github:gh"]);
    expect(pickerRows(accounts, "rocha").map(label)).toEqual(["github", "github:gh", "google", "google:g1"]);
    expect(pickerRows(accounts, "zzz")).toEqual([]);
  });
});

describe("applyRow", () => {
  const gh: SsoAccount = { id: "gh", title: "Github", username: "rochasamuel" };
  it("copies a saved login's username", () => {
    expect(applyRow({ kind: "account", provider: "github", account: gh }, null)).toEqual({ provider: "github", account: "rochasamuel" });
  });
  it("keeps the account when the provider stays, clears it when it changes", () => {
    const cur = { provider: "google" as const, account: "me@gmail.com" };
    expect(applyRow({ kind: "provider", provider: "google" }, cur)).toEqual(cur);
    expect(applyRow({ kind: "provider", provider: "x" }, cur)).toEqual({ provider: "x", account: null });
  });
  it("None clears", () => {
    expect(applyRow({ kind: "none" }, { provider: "google", account: "a" })).toBeNull();
  });
});
