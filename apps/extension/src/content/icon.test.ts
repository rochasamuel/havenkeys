import { describe, expect, it } from "vitest";
import { ICON_SIZE, ICON_STEP, iconBox } from "./icon";

const rect = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height }) as DOMRect;

describe("iconBox", () => {
  it("sits inside the field, vertically centred, near the right edge", () => {
    const box = iconBox(rect(40, 60, 400, 40));
    expect(box).toEqual({ top: 70, left: 440 - 8 - ICON_SIZE, width: ICON_SIZE, height: ICON_SIZE });
  });

  it("moves left in steps to clear the page's own field button", () => {
    const a = iconBox(rect(40, 60, 400, 40), 0);
    const b = iconBox(rect(40, 60, 400, 40), 1);
    expect(a!.left - b!.left).toBe(ICON_STEP);
  });

  it("shrinks for short fields and skips fields too small to share", () => {
    expect(iconBox(rect(0, 0, 300, 20))?.width).toBe(16);
    expect(iconBox(rect(0, 0, 60, 40))).toBeNull();
    expect(iconBox(rect(0, 0, 300, 12))).toBeNull();
  });
});
