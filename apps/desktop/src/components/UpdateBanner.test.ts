// Release notes come from a downloaded file. They must only ever reach the
// page as React text, which escapes them.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const source = readFileSync(fileURLToPath(new URL("./UpdateBanner.tsx", import.meta.url)), "utf8");

describe("UpdateBanner", () => {
  it("the banner never renders HTML", () => {
    expect(source).not.toMatch(/dangerouslySetInnerHTML|innerHTML|outerHTML|insertAdjacentHTML/);
  });
});
