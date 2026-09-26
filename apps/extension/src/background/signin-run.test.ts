import { describe, expect, it } from "vitest";
import { createRuns, nextStep, RUN_TTL_MS, type RunFrame } from "./signin-run";

const ITEM = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const f = (over: Partial<RunFrame> = {}): RunFrame => ({ tabId: 1, frameId: 0, origin: "https://www.amazon.com", ...over });

function setup() {
  let clock = 1_000;
  return { runs: createRuns(() => clock), advance: (ms: number) => (clock += ms) };
}

describe("nextStep", () => {
  it("goes username → password → otp (only with TOTP) → end", () => {
    expect(nextStep("username", false)).toBe("password");
    expect(nextStep("password", true)).toBe("otp");
    expect(nextStep("password", false)).toBeNull();
    expect(nextStep("otp", true)).toBeNull();
  });
});

describe("runs", () => {
  it("walks an Amazon-style flow", () => {
    const { runs } = setup();
    expect(runs.start(f(), ITEM, "username", true)).not.toBeNull();
    expect(runs.watchFor(f())).toBe("password"); // the password page loaded
    expect(runs.accept(f(), "password")?.itemId).toBe(ITEM);
    expect(runs.watchFor(f())).toBe("otp");
    expect(runs.accept(f(), "otp")?.step).toBe("otp");
  });

  it("keeps no run when the pressed step was the last", () => {
    const { runs } = setup();
    expect(runs.start(f(), ITEM, "password", false)).toBeNull();
    expect(runs.start(f(), ITEM, "otp", true)).toBeNull();
    expect(runs.get(1)).toBeNull();
  });

  it("ignores out-of-order, repeated, and foreign steps", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", true);
    expect(runs.accept(f(), "otp")).toBeNull(); // skips password
    expect(runs.accept(f({ tabId: 2 }), "password")).toBeNull(); // another tab
    expect(runs.accept(f({ frameId: 3 }), "password")).toBeNull(); // another frame
    expect(runs.accept(f({ origin: "https://amazon.com.evil.example" }), "password")).toBeNull();
    expect(runs.accept(f({ origin: "https://smile.amazon.com" }), "password")).toBeNull(); // same site, other origin
    expect(runs.accept(f(), "password")).not.toBeNull();
    expect(runs.accept(f(), "password")).toBeNull(); // once
  });

  it("expires after two minutes", () => {
    const { runs, advance } = setup();
    runs.start(f(), ITEM, "username", false);
    advance(RUN_TTL_MS);
    expect(runs.watchFor(f())).toBeNull();
    expect(runs.accept(f(), "password")).toBeNull();
  });

  it("ends when the run's frame loads another origin", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", false);
    expect(runs.watchFor(f({ origin: "https://evil.example" }))).toBeNull();
    expect(runs.get(1)).toBeNull();
  });

  it("does not let other frames watch or stop the run", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", false);
    expect(runs.watchFor(f({ frameId: 7, origin: "https://ads.example" }))).toBeNull();
    expect(runs.stop(f({ frameId: 7 }))).toBeNull();
    expect(runs.get(1)).not.toBeNull();
    expect(runs.stop(f())).not.toBeNull();
    expect(runs.get(1)).toBeNull();
  });

  it("a new start replaces the run; clear drops all", () => {
    const { runs } = setup();
    runs.start(f(), ITEM, "username", false);
    runs.start(f(), "11111111-2222-4333-8444-555555555555", "username", false);
    expect(runs.get(1)?.itemId).toBe("11111111-2222-4333-8444-555555555555");
    runs.start(f({ tabId: 2 }), ITEM, "username", false);
    expect(runs.clear()).toHaveLength(2);
    expect(runs.get(1)).toBeNull();
  });
});
