import { describe, expect, it } from "vitest";

import { decideOpen, isDirty, type EditorSnapshot } from "./openItem";

const A = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const B = "16fd2706-8baf-433b-82eb-8c7fada847da";

describe("decideOpen", () => {
  it("opens when nothing is being edited", () => {
    expect(decideOpen({ kind: "empty" }, false, A)).toBe("open");
    expect(decideOpen({ kind: "view", id: B }, false, A)).toBe("open");
    expect(decideOpen({ kind: "view", id: A }, false, A)).toBe("open");
  });
  it("does nothing when that item's editor is already open", () => {
    expect(decideOpen({ kind: "edit", id: A }, false, A)).toBe("already");
    expect(decideOpen({ kind: "edit", id: A }, true, A)).toBe("already");
  });
  it("asks before discarding another edit's changes", () => {
    expect(decideOpen({ kind: "edit", id: B }, true, A)).toBe("confirm");
    expect(decideOpen({ kind: "new" }, true, A)).toBe("confirm");
  });
  it("switches straight away when the other edit is untouched", () => {
    expect(decideOpen({ kind: "edit", id: B }, false, A)).toBe("open");
    expect(decideOpen({ kind: "new" }, false, A)).toBe("open");
  });
  it("reveals the section when that item's editor is open but hidden behind a tool section", () => {
    expect(decideOpen({ kind: "edit", id: A }, false, A, false)).toBe("reveal");
    expect(decideOpen({ kind: "edit", id: A }, true, A, false)).toBe("reveal");
  });
  it("still reports already when that item's editor is open and shown", () => {
    expect(decideOpen({ kind: "edit", id: A }, false, A, true)).toBe("already");
  });
});

const base: EditorSnapshot = {
  title: "GitHub",
  username: "alice",
  urls: [{ url: "https://github.com", matchType: "domain" }],
  password: { mode: "keep" },
  totp: { mode: "keep" },
  notes: { mode: "keep" },
  autoSignIn: true,
  signInWith: null,
};

describe("isDirty", () => {
  it("is false for the snapshot it started from", () => {
    expect(isDirty(base, { ...base, urls: [{ ...base.urls[0]! }] })).toBe(false);
  });
  it("is true for any change", () => {
    expect(isDirty(base, { ...base, title: "GitHub 2" })).toBe(true);
    expect(isDirty(base, { ...base, username: "bob" })).toBe(true);
    expect(isDirty(base, { ...base, urls: [] })).toBe(true);
    expect(isDirty(base, { ...base, password: { mode: "set", value: "x" } })).toBe(true);
    expect(isDirty(base, { ...base, totp: { mode: "clear" } })).toBe(true);
    expect(isDirty(base, { ...base, notes: { mode: "set", value: "n" } })).toBe(true);
    expect(isDirty(base, { ...base, autoSignIn: false })).toBe(true);
  });
});
