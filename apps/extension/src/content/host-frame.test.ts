// @vitest-environment jsdom
// @vitest-environment-options {"url": "https://shop.com/checkout"}
import { beforeEach, describe, expect, it } from "vitest";
import { findHostIframe, MAX_HOST_IFRAMES } from "./host-frame";

const STRIPE = "https://js.stripe.com/v3/elements-inner-card.html";

beforeEach(() => {
  document.body.textContent = "";
});

function iframe(id: string, src?: string): HTMLIFrameElement {
  const f = document.createElement("iframe");
  f.id = id;
  if (src !== undefined) f.setAttribute("src", src);
  document.body.append(f);
  return f;
}

const all = () => Array.from(document.querySelectorAll("iframe"));

describe("findHostIframe", () => {
  it("uses runtime.getFrameId when the browser has it (Firefox)", () => {
    iframe("a", STRIPE);
    const b = iframe("b", STRIPE);
    const getFrameId = (el: Element) => (el.id === "b" ? 7 : 3);
    expect(findHostIframe(7, STRIPE, all(), getFrameId)).toBe(b);
    // getFrameId is authoritative: no match means the frame is not a direct child.
    expect(findHostIframe(9, STRIPE, all(), getFrameId)).toBeNull();
    // A throwing getFrameId counts as no match.
    expect(findHostIframe(7, STRIPE, all(), () => { throw new Error("x"); })).toBeNull();
  });

  it("otherwise matches the iframe's src without query or fragment, when exactly one matches (Chromium)", () => {
    iframe("ad", "https://ads.example.net/slot");
    const s = iframe("stripe", `${STRIPE}?v=3#${"k".repeat(40)}`);
    expect(findHostIframe(7, STRIPE, all(), undefined)).toBe(s);
    // A relative src resolves against the page.
    const own = iframe("own", "/pay/frame?x=1");
    expect(findHostIframe(8, "https://shop.com/pay/frame", all(), undefined)).toBe(own);
  });

  it("gives up when no iframe or more than one has that src", () => {
    iframe("a", `${STRIPE}#1`);
    iframe("b", `${STRIPE}#2`);
    expect(findHostIframe(7, STRIPE, all(), undefined)).toBeNull();
    expect(findHostIframe(7, "https://js.stripe.com/other.html", all(), undefined)).toBeNull();
    document.body.textContent = "";
    iframe("blank");
    expect(findHostIframe(7, "about:blank", all(), undefined)).toBeNull();
  });

  it("looks at a bounded number of iframes", () => {
    for (let i = 0; i < MAX_HOST_IFRAMES; i++) iframe(`x${i}`, "https://ads.example.net/slot");
    iframe("late", STRIPE);
    expect(findHostIframe(7, STRIPE, all(), undefined)).toBeNull();
  });
});
