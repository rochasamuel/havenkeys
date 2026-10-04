import { describe, expect, it } from "vitest";
import { isTerminalPollError, nextStep, secondsLeft } from "./pairing";

describe("secondsLeft", () => {
  it("counts down to zero and never below", () => {
    const deadline = 1_000_000;
    expect(secondsLeft(deadline, deadline - 90_500)).toBe(91);
    expect(secondsLeft(deadline, deadline + 5_000)).toBe(0);
  });
});

describe("isTerminalPollError", () => {
  it("ends the pairing only for a failed or gone pairing", () => {
    expect(isTerminalPollError("pairing_failed")).toBe(true);
    expect(isTerminalPollError("pairing_gone")).toBe(true);
    expect(isTerminalPollError("offline")).toBe(false);
    expect(isTerminalPollError(undefined)).toBe(false);
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
