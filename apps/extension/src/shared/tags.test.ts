import { describe, expect, it } from "vitest";
import { tagChips } from "./tags";

describe("tagChips", () => {
  it("shows up to two tags and counts the rest", () => {
    expect(tagChips([])).toEqual({ shown: [], more: 0 });
    expect(tagChips(["staging"])).toEqual({ shown: ["staging"], more: 0 });
    expect(tagChips(["prod", "work"])).toEqual({ shown: ["prod", "work"], more: 0 });
    expect(tagChips(["a", "b", "c", "d"])).toEqual({ shown: ["a", "b"], more: 2 });
  });
});
