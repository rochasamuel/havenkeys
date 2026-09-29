import { describe, expect, it } from "vitest";
import { createSsoState, PENDING_TTL_MS, SSO_RUN_TTL_MS } from "./sso-state";

function setup() {
  let t = 1_000;
  const s = createSsoState(() => t);
  return { s, advance: (ms: number) => (t += ms) };
}
const G = "https://accounts.google.com";

describe("pending captures", () => {
  it("prompts when the tab comes back after the provider", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/login", undefined, "google");
    s.visit({ tabId: 1 }, G, true);
    expect(s.account({ tabId: 1 }, G, "me@gmail.com", true)).toBe(true);
    const p = s.takeReturn(1, "https://admin.typeform.com");
    expect(p?.account).toBe("me@gmail.com");
    expect(s.pending(1)).toBeNull();
  });
  it("ignores a provider iframe embedded in the site page (not a top frame)", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/", undefined, "google");
    s.visit({ tabId: 1 }, G, false); // a GSI iframe on the site, not a navigation
    expect(s.account({ tabId: 1 }, G, "me@gmail.com", false)).toBe(false);
    expect(s.pending(1)?.sawProvider).toBe(false);
    expect(s.takeReturn(1, "https://typeform.com")).toBeNull();
  });
  it("return_before_provider_is_ignored", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/login", undefined, "google");
    expect(s.takeReturn(1, "https://typeform.com")).toBeNull();
    expect(s.pending(1)).not.toBeNull();
  });
  it("learns the account only on the provider's own origins, also in a popup", () => {
    const { s } = setup();
    s.click(1, "https://typeform.com/", undefined, "google");
    expect(s.account({ tabId: 1 }, "https://evil.com", "x@y.z", true)).toBe(false);
    expect(s.account({ tabId: 9, openerTabId: 1 }, "https://login.live.com", "x@y.z", true)).toBe(false);
    s.visit({ tabId: 9, openerTabId: 1 }, G, true);
    expect(s.account({ tabId: 9, openerTabId: 1 }, G, "me@gmail.com", true)).toBe(true);
    expect(s.takeOnTabClosed(9)?.account).toBe("me@gmail.com");
  });
  it("expires", () => {
    const { s, advance } = setup();
    s.click(1, "https://typeform.com/", undefined, "google");
    s.visit({ tabId: 1 }, G, true);
    advance(PENDING_TTL_MS + 1);
    expect(s.takeReturn(1, "https://typeform.com")).toBeNull();
  });
});

describe("runs", () => {
  const start = (s: ReturnType<typeof createSsoState>, over: Partial<{ account: string | null; autoChoose: boolean }> = {}) =>
    s.startRun({ tabId: 1, frameId: 0, siteOrigin: "https://typeform.com", provider: "google", account: "me@gmail.com", providerOrigins: [G], autoChoose: true, ...over });

  it("choose_only_on_provider_origins", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    expect(s.chooseFor({ tabId: 1 }, "https://evil.com")).toBeNull();
    expect(s.chooseFor({ tabId: 1 }, "https://accounts.google.com.evil.com")).toBeNull();
    expect(s.chooseFor({ tabId: 1 }, G)).toBe("me@gmail.com");
    expect(s.run(1)?.phase).toBe("login"); // choose is used once; the run waits for the login form
  });
  it("follows a popup opened by the run's tab, nothing else", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    expect(s.chooseFor({ tabId: 7 }, G)).toBeNull();
    expect(s.chooseFor({ tabId: 7, openerTabId: 1 }, G)).toBe("me@gmail.com");
  });
  it("ends after pressing without account or auto choose", () => {
    const { s } = setup();
    start(s, { account: null });
    expect(s.pressed(1)).toBeNull();
    start(s, { autoChoose: false });
    expect(s.pressed(1)).toBeNull();
  });
  it("ends on leaving, on expiry, and cannot choose before pressing", () => {
    const { s, advance } = setup();
    start(s);
    expect(s.chooseFor({ tabId: 1 }, G)).toBeNull(); // still "press"
    s.pressed(1);
    s.topLoad(1, "https://typeform.com"); // site origin: fine
    expect(s.run(1)).not.toBeNull();
    s.topLoad(1, "https://elsewhere.com");
    expect(s.run(1)).toBeNull();
    start(s);
    s.pressed(1);
    advance(SSO_RUN_TTL_MS + 1);
    expect(s.chooseFor({ tabId: 1 }, G)).toBeNull();
  });
  it("choose moves to login; login_only_once_top_run_tab", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    expect(s.chooseFor({ tabId: 1 }, G)).toBe("me@gmail.com");
    expect(s.run(1)?.phase).toBe("login");
    expect(s.chooseFor({ tabId: 1 }, G)).toBeNull(); // choose happens once
    expect(s.loginReady({ tabId: 1 }, G)).toBe(true);
    expect(s.loginReady({ tabId: 1 }, "https://evil.com")).toBe(false);
    expect(s.loginFor({ tabId: 7 }, G)).toBeNull(); // unrelated tab
    expect(s.loginFor({ tabId: 1 }, "https://accounts.google.com.evil.com")).toBeNull();
    expect(s.loginFor({ tabId: 9, openerTabId: 1 }, G)?.account).toBe("me@gmail.com"); // popup
    expect(s.loginFor({ tabId: 1 }, G)).toBeNull(); // consumed
    expect(s.run(1)).toBeNull();
  });
  it("login expires with the run", () => {
    const { s, advance } = setup();
    start(s);
    s.pressed(1);
    s.chooseFor({ tabId: 1 }, G);
    advance(SSO_RUN_TTL_MS + 1);
    expect(s.loginFor({ tabId: 1 }, G)).toBeNull();
  });
});
