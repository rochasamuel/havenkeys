import { describe, expect, it } from "vitest";
import { cleanTag, tagCounts } from "./tags";
import type { ItemOverview } from "./types";

const item = (tags: string[]) => ({ tags }) as unknown as ItemOverview;

describe("cleanTag", () => {
  it("previews Rust's rule", () => {
    expect(cleanTag("  Dev   Team ")).toBe("dev team");
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
