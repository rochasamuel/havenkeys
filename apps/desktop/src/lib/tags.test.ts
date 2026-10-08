import { describe, expect, it } from "vitest";
import { cleanTag, hasTag, sameTag, tagCounts } from "./tags";
import type { ItemOverview } from "./types";

const item = (tags: string[]) => ({ tags }) as unknown as ItemOverview;

describe("cleanTag", () => {
  it("previews Rust's rule", () => {
    expect(cleanTag("  Dev   Team ")).toBe("Dev Team");
    expect(cleanTag("PRODUÇÃO")).toBe("PRODUÇÃO");
    expect(cleanTag("")).toBeNull();
    expect(cleanTag("a,b")).toBeNull();
    expect(cleanTag("x".repeat(33))).toBeNull();
    expect(cleanTag("é".repeat(32))).toBe("é".repeat(32));
  });
});

describe("tagCounts", () => {
  it("counts each tag across items, A–Z", () => {
    expect(tagCounts([item(["work", "prod"]), item(["work"]), item([])])).toEqual([
      { name: "prod", count: 1 },
      { name: "work", count: 2 },
    ]);
  });
});

describe("tagCounts without case", () => {
  it("merges Work and work, A–Z without case", () => {
    expect(tagCounts([item(["Work"]), item(["work", "api"]), item(["Beta"])])).toEqual([
      { name: "api", count: 1 },
      { name: "Beta", count: 1 },
      { name: "Work", count: 2 },
    ]);
  });

  it("shows the spelling most items carry, a tie going to the smallest, whatever the order", () => {
    const items = [item(["work"]), item(["Work"]), item(["Work", "API"]), item(["api"])];
    const want = [
      { name: "API", count: 2 },
      { name: "Work", count: 3 },
    ];
    expect(tagCounts(items)).toEqual(want);
    expect(tagCounts([...items].reverse())).toEqual(want);
  });
});

describe("sameTag / hasTag", () => {
  it("compares without case, so the tag filter matches every spelling", () => {
    expect(sameTag("Work", "work")).toBe(true);
    expect(sameTag("Work", "worker")).toBe(false);
    expect(hasTag(["api", "Work"], "work")).toBe(true);
    expect(hasTag(["api"], "work")).toBe(false);
  });
});
