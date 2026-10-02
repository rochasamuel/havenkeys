import { describe, expect, it } from "vitest";
import { detectOs } from "./os";

describe("detectOs", () => {
  it("recognises an Android phone", () => {
    expect(detectOs("Mozilla/5.0 (Linux; Android 16; SM-S926B) AppleWebKit/537.36 Chrome/141 Mobile")).toBe("android");
  });
  it("keeps desktop Linux", () => {
    expect(detectOs("Mozilla/5.0 (X11; Linux x86_64) Gecko/20100101 Firefox/143.0")).toBe("linux");
  });
  it("keeps Windows and macOS", () => {
    expect(detectOs("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("windows");
    expect(detectOs("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)")).toBe("macos");
  });
});
