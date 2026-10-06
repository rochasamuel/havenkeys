// @vitest-environment jsdom
import { describe, expect, it, vi } from "vitest";
import { TOKEN_MESSAGE } from "../messaging/inline";
import { InlineFrame, MENU_HEADER, MENU_PADDING, MENU_ROW, SAVE_HEIGHT, SAVE_WIDTH, SSO_HEIGHT, frameToken, menuBox, saveBox, ssoBox } from "./frames";

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

describe("ssoBox", () => {
  it("sits where the save prompt does, at the sign-in-with height", () => {
    expect(ssoBox({ width: 1000 })).toEqual(saveBox({ width: 1000 }, SSO_HEIGHT));
    expect(ssoBox({ width: 1000, height: 800 }, 200)).toEqual(saveBox({ width: 1000, height: 800 }, 200));
  });
});

/** EX-03: the page must not be able to read a frame's token. */
describe("InlineFrame token", () => {
  it("keeps the token out of the URL and posts it to the extension's origin once loaded", () => {
    (globalThis as { chrome?: unknown }).chrome = { runtime: { getURL: (p: string) => `chrome-extension://ext/${p}` } };
    const token = "a".repeat(32);
    const frame = new InlineFrame("menu.html", token, { top: 0, left: 0, width: 10, height: 10 }, () => {});
    expect(frame.el.src).toBe("chrome-extension://ext/menu.html");
    expect(frame.el.outerHTML).not.toContain(token);
    expect(frameToken(frame.el)).toBe(token);

    const post = vi.fn();
    Object.defineProperty(frame.el, "contentWindow", { value: { postMessage: post } });
    frame.el.dispatchEvent(new Event("load"));
    frame.el.dispatchEvent(new Event("load"));
    expect(post).toHaveBeenCalledTimes(1);
    expect(post).toHaveBeenCalledWith({ type: TOKEN_MESSAGE, token }, "chrome-extension://ext/");
    frame.remove();
  });
});
