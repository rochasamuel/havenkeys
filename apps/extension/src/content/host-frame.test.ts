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

  it("tells same-path iframes apart by their full URL (split Stripe, Adyen, Braintree)", () => {
    const number = iframe("number", `${STRIPE}?type=number#${"n".repeat(20)}`);
    const expiry = iframe("expiry", `${STRIPE}?type=expiry#${"e".repeat(20)}`);
    const cvc = iframe("cvc", `${STRIPE}?type=cvc#${"c".repeat(20)}`);
    expect(findHostIframe(7, `${STRIPE}?type=expiry#${"e".repeat(20)}`, all(), undefined)).toBe(expiry);
    expect(findHostIframe(7, `${STRIPE}?type=cvc#${"c".repeat(20)}`, all(), undefined)).toBe(cvc);
    expect(findHostIframe(7, `${STRIPE}?type=number#${"n".repeat(20)}`, all(), undefined)).toBe(number);
    // Braintree: the same file, one id per field in the fragment.
    document.body.textContent = "";
    const bt = "https://assets.braintreegateway.com/web/3.97.0/html/hosted-fields-frame.min.html";
    iframe("bt1", `${bt}#aaa`);
    const bt2 = iframe("bt2", `${bt}#bbb`);
    iframe("bt3", `${bt}#ccc`);
    expect(findHostIframe(7, `${bt}#bbb`, all(), undefined)).toBe(bt2);
  });

  it("then tells identical iframes apart by size, within 2 px", () => {
    const sized = (id: string, w: number, h: number) => {
      const f = iframe(id, STRIPE);
      Object.defineProperty(f, "clientWidth", { value: w });
      Object.defineProperty(f, "clientHeight", { value: h });
      return f;
    };
    sized("number", 300, 40);
    const expiry = sized("expiry", 140, 40);
    sized("cvc", 100, 40);
    expect(findHostIframe(7, STRIPE, all(), undefined, { width: 141, height: 38 })).toBe(expiry);
    // Only among iframes with the right URL.
    iframe("ad", "https://ads.example.net/slot");
    expect(findHostIframe(7, STRIPE, all(), undefined, { width: 500, height: 40 })).toBeNull();
  });

  it("returns null when size does not single one out either", () => {
    const sized = (id: string, w: number, h: number) => {
      const f = iframe(id, STRIPE);
      Object.defineProperty(f, "clientWidth", { value: w });
      Object.defineProperty(f, "clientHeight", { value: h });
    };
    sized("a", 140, 40);
    sized("b", 140, 40);
    sized("c", 100, 40);
    expect(findHostIframe(7, STRIPE, all(), undefined, { width: 140, height: 40 })).toBeNull();
    expect(findHostIframe(7, STRIPE, all(), undefined)).toBeNull();
    expect(findHostIframe(7, STRIPE, all(), undefined, null)).toBeNull();
  });

  it("looks at a bounded number of iframes", () => {
    for (let i = 0; i < MAX_HOST_IFRAMES; i++) iframe(`x${i}`, "https://ads.example.net/slot");
    iframe("late", STRIPE);
    expect(findHostIframe(7, STRIPE, all(), undefined)).toBeNull();
  });
});
