import { describe, expect, it } from "vitest";
import type { HealthReport } from "./types";
import { checksFor, duplicateOthers, groupSize, reusedCount, rowsFor, totalIssues } from "./health";

const report: HealthReport = {
  computedAt: 1,
  counts: { weak: 1, reused: 2, old: 0, passkey: 1, twoFactor: 0, insecure: 0, duplicate: 0 },
  issues: [
    { itemId: "a", checks: ["weak", "reused"], reusedGroup: 0, duplicateGroup: null, help: false },
    { itemId: "b", checks: ["reused", "passkey"], reusedGroup: 0, duplicateGroup: null, help: true },
  ],
  dismissed: [{ itemId: "c", checks: ["old"] }],
};

describe("health helpers", () => {
  it("counts every non-dismissed check", () => expect(totalIssues(report)).toBe(4));
  it("finds an item's checks", () => {
    expect(checksFor(report, "b")).toEqual(["reused", "passkey"]);
    expect(checksFor(report, "zzz")).toEqual([]);
    expect(checksFor(null, "a")).toEqual([]);
  });
  it("sizes a reuse group", () => expect(groupSize(report, "reused", 0)).toBe(2));
  it("filters rows", () => {
    expect(rowsFor(report, "weak").map((r) => r.itemId)).toEqual(["a"]);
    expect(rowsFor(report, "all").map((r) => r.itemId)).toEqual(["a", "b"]);
    expect(rowsFor(report, "dismissed")).toEqual([
      { itemId: "c", checks: ["old"], reusedGroup: null, duplicateGroup: null, help: false, dismissed: true },
    ]);
  });

  // The core numbers groups before dismissals: when a login's group-mates
  // dismissed the check, only that login is visible in its group.
  it("never says a reused password is used in fewer than 2 logins", () => {
    const lone: HealthReport = {
      ...report,
      issues: [
        { itemId: "a", checks: ["reused"], reusedGroup: 3, duplicateGroup: null, help: false },
        { itemId: "d", checks: ["duplicate"], reusedGroup: null, duplicateGroup: 1, help: false },
      ],
    };
    expect(reusedCount(lone, "a")).toBe(2);
    expect(duplicateOthers(lone, "d")).toBe(1);
    expect(reusedCount(report, "b")).toBe(2);
    expect(duplicateOthers(lone, "zzz")).toBe(1);
  });

  it("counts the other duplicates in a group", () => {
    const three: HealthReport = {
      ...report,
      issues: ["x", "y", "z"].map((id) => ({ itemId: id, checks: ["duplicate"], reusedGroup: null, duplicateGroup: 0, help: false })),
    };
    expect(duplicateOthers(three, "x")).toBe(2);
  });
});
