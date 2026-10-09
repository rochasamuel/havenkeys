import { describe, expect, it } from "vitest";
import { RAILWAY_TEMPLATE_URL } from "./links";

describe("links", () => {
  it("has no Railway template until the owner publishes one", () => {
    expect(RAILWAY_TEMPLATE_URL).toBe("");
  });
});
