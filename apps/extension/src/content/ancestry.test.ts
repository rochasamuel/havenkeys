import { describe, expect, it } from "vitest";
import { MAX_FRAME_DEPTH } from "../messaging/inline";
import { frameAncestry, type AncestryWindow } from "./ancestry";

/** A chain of fake windows, top first; `readable[i]` false makes window i's origin throw (cross-origin). */
function chain(origins: string[], opts: { ancestorOrigins?: boolean; readable?: boolean[] } = {}): AncestryWindow[] {
  const wins: AncestryWindow[] = [];
  for (const [i, origin] of origins.entries()) {
    const readable = opts.readable?.[i] ?? true;
    const location = {
      get origin(): string {
        if (!readable) throw new DOMException("Blocked a frame", "SecurityError");
        return origin;
      },
      ...(opts.ancestorOrigins ? { ancestorOrigins: origins.slice(0, i).reverse() } : {}),
    };
    const w = { location } as unknown as AncestryWindow;
    wins.push(w);
  }
  for (const [i, w] of wins.entries()) {
    (w as { top: AncestryWindow }).top = wins[0]!;
    (w as { parent: AncestryWindow }).parent = wins[Math.max(0, i - 1)]!;
  }
  return wins;
}

describe("frameAncestry", () => {
  it("reports nothing in the top frame", () => {
    expect(frameAncestry(chain(["https://shop.com"])[0]!)).toBeNull();
  });

  it("uses location.ancestorOrigins where the browser has it (Chromium), nearest first", () => {
    const w = chain(["https://shop.com", "https://ads.example.net", "https://js.stripe.com"], { ancestorOrigins: true, readable: [false, false, true] });
    expect(frameAncestry(w[2]!)).toEqual({ ancestors: ["https://ads.example.net", "https://shop.com"] });
  });

  it("turns an opaque or odd ancestorOrigins entry into the unknown-chain form", () => {
    const w = chain(["https://shop.com", "null", "https://js.stripe.com"], { ancestorOrigins: true });
    expect(frameAncestry(w[2]!)).toEqual({ ancestors: null, directChildOfTop: false });
  });

  it("walks readable parents where ancestorOrigins is missing (Firefox)", () => {
    const w = chain(["https://shop.com", "https://shop.com", "https://shop.com"]);
    expect(frameAncestry(w[2]!)).toEqual({ ancestors: ["https://shop.com", "https://shop.com"] });
  });

  it("reports an unknown chain when a parent is cross-origin (Firefox), saying whether the parent is the top page", () => {
    const direct = chain(["https://shop.com", "https://js.stripe.com"], { readable: [false, true] });
    expect(frameAncestry(direct[1]!)).toEqual({ ancestors: null, directChildOfTop: true });
    const nested = chain(["https://shop.com", "https://ads.example.net", "https://js.stripe.com"], { readable: [false, false, true] });
    expect(frameAncestry(nested[2]!)).toEqual({ ancestors: null, directChildOfTop: false });
  });

  it("reports an unknown, not-direct chain when the frame is nested too deep", () => {
    const origins = Array.from({ length: MAX_FRAME_DEPTH + 2 }, () => "https://shop.com");
    const w = chain(origins);
    expect(frameAncestry(w.at(-1)!)).toEqual({ ancestors: null, directChildOfTop: false });
    const wc = chain(origins, { ancestorOrigins: true });
    expect(frameAncestry(wc.at(-1)!)).toEqual({ ancestors: null, directChildOfTop: false });
  });
});
