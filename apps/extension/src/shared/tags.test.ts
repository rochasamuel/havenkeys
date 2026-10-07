import { describe, expect, it } from "vitest";
import { tagLine } from "./tags";

describe("tagLine", () => {
  it("shows at most two tags, then a count", () => {
    expect(tagLine([])).toBe("");
    expect(tagLine(["staging"])).toBe("staging");
    expect(tagLine(["prod", "work"])).toBe("prod, work");
    expect(tagLine(["a", "b", "c", "d"])).toBe("a, b +2");
  });
});
