import { describe, expect, it } from "vitest";
import { frameUrlForHost, pageUrlForRequest } from "./url";

describe("frameUrlForHost", () => {
  it("keeps query and fragment, drops credentials", () => {
    const u = "https://assets.braintreegateway.com/web/html/hosted-fields-frame.min.html#abc";
    expect(frameUrlForHost(u)).toBe(u);
    expect(frameUrlForHost("https://checkoutshopper-live.adyen.com/securedFields.html?type=card&d=1")).toBe(
      "https://checkoutshopper-live.adyen.com/securedFields.html?type=card&d=1",
    );
    expect(frameUrlForHost("https://user:pw@pay.example.com/f?x=1#y")).toBe("https://pay.example.com/f?x=1#y");
  });

  it("refuses anything but a bounded http(s) URL", () => {
    expect(frameUrlForHost(undefined)).toBeNull();
    expect(frameUrlForHost("about:blank")).toBeNull();
    expect(frameUrlForHost("javascript:alert(1)")).toBeNull();
    expect(frameUrlForHost("not a url")).toBeNull();
    expect(frameUrlForHost(`https://a.com/#${"x".repeat(5000)}`)).toBeNull();
  });

  it("differs from the desktop's stripped URL only by query and fragment", () => {
    expect(pageUrlForRequest("https://a.com/p?q#f")).toBe("https://a.com/p");
  });
});
