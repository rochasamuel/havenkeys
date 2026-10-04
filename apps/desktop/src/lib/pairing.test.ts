import { describe, expect, it } from "vitest";
import { nextStep, secondsLeft } from "./pairing";

describe("secondsLeft", () => {
  it("counts down to zero and never below", () => {
    const at = Date.parse("2026-10-03T12:02:00Z");
    expect(secondsLeft("2026-10-03T12:02:00Z", at - 90_500)).toBe(91);
    expect(secondsLeft("2026-10-03T12:02:00Z", at + 5_000)).toBe(0);
  });
  it("treats an unreadable time as expired", () => {
    expect(secondsLeft("not a date", Date.now())).toBe(0);
  });
});

describe("nextStep", () => {
  it("keeps polling only while waiting", () => {
    expect(nextStep("waiting")).toBe("poll");
    expect(nextStep("denied")).toBe("denied");
    expect(nextStep("expired")).toBe("expired");
    expect(nextStep("approved")).toBe("done");
  });
});
