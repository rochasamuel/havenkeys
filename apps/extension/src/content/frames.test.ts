import { describe, expect, it } from "vitest";
import { MENU_HEADER, MENU_PADDING, MENU_ROW, SAVE_HEIGHT, SAVE_WIDTH, menuBox, saveBox } from "./frames";

const field = { top: 100, bottom: 130, left: 20, width: 300 } as DOMRect;
const viewport = { width: 1000, height: 800 };

describe("menuBox", () => {
  it("estimates the height from the row count until the menu reports one", () => {
    expect(menuBox(field, 2, viewport).height).toBe(MENU_HEADER + 2 * MENU_ROW + MENU_PADDING);
    expect(menuBox(field, 2, viewport, null).height).toBe(MENU_HEADER + 2 * MENU_ROW + MENU_PADDING);
  });

  it("uses the height the menu page measured, so wrapped rows are not clipped", () => {
    expect(menuBox(field, 2, viewport, 171)).toEqual({ top: 134, left: 20, width: 300, height: 171 });
  });

  it("opens above the field with the measured height when there is no room below", () => {
    const low = { ...field, top: 700, bottom: 730 } as DOMRect;
    expect(menuBox(low, 1, viewport, 200).top).toBe(700 - 200 - 4);
  });
});

describe("saveBox", () => {
  it("starts at the default height, then uses the height the prompt measured", () => {
    expect(saveBox({ width: 1000 })).toEqual({ top: 12, left: 1000 - SAVE_WIDTH - 16, width: SAVE_WIDTH, height: SAVE_HEIGHT });
    expect(saveBox({ width: 1000, height: 800 }, 182).height).toBe(182);
  });

  it("never grows taller than the viewport allows", () => {
    expect(saveBox({ width: 1000, height: 150 }, 200).height).toBe(150 - 24);
  });
});
