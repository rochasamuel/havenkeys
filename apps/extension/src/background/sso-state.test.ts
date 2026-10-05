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
  it("a popup the tab opened after the click counts when it closes, even if no provider page loaded", () => {
    // Google may approve with a redirect alone (signed in, consented, login_hint): no document on its origin.
    const { s } = setup();
    s.click(1, "https://typeform.com/login", undefined, "google");
    s.opened({ tabId: 9, openerTabId: 1 });
    expect(s.takeOnTabClosed(9)?.url).toBe("https://typeform.com/login");
    expect(s.pending(1)).toBeNull();
  });
  it("a popup opened by another tab, or before any click, does not count", () => {
    const { s } = setup();
    s.opened({ tabId: 8, openerTabId: 1 }); // before the click
    s.click(1, "https://typeform.com/login", undefined, "google");
    s.opened({ tabId: 9, openerTabId: 2 }); // someone else's popup
    s.opened({ tabId: 7 }); // no opener
    expect(s.takeOnTabClosed(8)).toBeNull();
    expect(s.takeOnTabClosed(9)).toBeNull();
    expect(s.takeOnTabClosed(7)).toBeNull();
    expect(s.pending(1)).not.toBeNull();
  });
  it("learns a login_hint from URLs the clicked tab or its popup loads after the click", () => {
    const { s } = setup();
    s.hint({ tabId: 1 }, "https://accounts.google.com/o/oauth2/auth?login_hint=early@x.com"); // before the click
    expect(s.pending(1)).toBeNull();
    s.click(1, "https://typeform.com/login", undefined, "google");
    s.hint({ tabId: 1 }, "https://typeform.com/login?next=/");
    expect(s.pending(1)?.hint).toBeNull();
    s.hint({ tabId: 9, openerTabId: 1 }, "https://auth.typeform.com/oauth2/default/v1/authorize?idp=x&login_hint=Me%40Gmail.com");
    expect(s.pending(1)?.hint).toBe("me@gmail.com");
    // Not an email, too long, or someone else's tab: ignored.
    s.hint({ tabId: 1 }, "https://x.com/?login_hint=not-an-email");
    s.hint({ tabId: 1 }, `https://x.com/?login_hint=${"a".repeat(300)}@b.co`);
    s.hint({ tabId: 7, openerTabId: 2 }, "https://x.com/?login_hint=other@x.com");
    s.hint({ tabId: 1 }, "not a url");
    expect(s.pending(1)?.hint).toBe("me@gmail.com");
    // The provider page's own account wins over the hint; both are kept.
    s.visit({ tabId: 1 }, G, true);
    s.account({ tabId: 1 }, G, "chosen@gmail.com", true);
    expect(s.pending(1)).toMatchObject({ account: "chosen@gmail.com", hint: "me@gmail.com" });
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
  it("Firefox: follows Google's popup without an opener when its origin parameter names the run's site", () => {
    const { s } = setup();
    start(s);
    s.hint({ tabId: 7 }, `${G}/gsi/select?client_id=x&ux_mode=popup&origin=${encodeURIComponent("https://typeform.com")}`);
    s.pressed(1);
    expect(s.runTabOf({ tabId: 7 })).toBe(1);
    expect(s.chooseFor({ tabId: 7 }, G)).toBe("me@gmail.com");
    expect(s.loginFor({ tabId: 7 }, G)?.account).toBe("me@gmail.com");
  });
  it("does not tie a popup naming another site, off the provider's origin, or with an opener", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    s.hint({ tabId: 7 }, `${G}/gsi/select?origin=${encodeURIComponent("https://evil.com")}`);
    s.hint({ tabId: 8 }, `https://evil.com/gsi/select?origin=${encodeURIComponent("https://typeform.com")}`);
    s.hint({ tabId: 9, openerTabId: 4 }, `${G}/gsi/select?origin=${encodeURIComponent("https://typeform.com")}`);
    expect(s.chooseFor({ tabId: 7 }, G)).toBeNull();
    expect(s.chooseFor({ tabId: 8 }, G)).toBeNull();
    expect(s.chooseFor({ tabId: 9 }, G)).toBeNull();
    expect(s.runTabOf({ tabId: 7 })).toBeNull();
  });
  it("does not tie a popup when two runs are for the same site", () => {
    const { s } = setup();
    start(s);
    s.startRun({ tabId: 2, frameId: 0, siteOrigin: "https://typeform.com", provider: "google", account: "you@gmail.com", providerOrigins: [G], autoChoose: true });
    s.pressed(1);
    s.pressed(2);
    s.hint({ tabId: 7 }, `${G}/gsi/select?origin=${encodeURIComponent("https://typeform.com")}`);
    expect(s.chooseFor({ tabId: 7 }, G)).toBeNull();
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
  it("a login-phase run ends when the tab's top frame returns to the site", () => {
    const { s } = setup();
    start(s);
    s.pressed(1);
    s.chooseFor({ tabId: 1 }, G);
    s.topLoad(1, G); // another provider document: still waiting for the login form
    expect(s.run(1)?.phase).toBe("login");
    s.topLoad(1, "https://typeform.com"); // the provider finished and sent the tab back
    expect(s.run(1)).toBeNull();
    expect(s.loginReady({ tabId: 1 }, G)).toBe(false);
  });
  it("a press- or choose-phase run survives a site reload (the site may reload before the provider)", () => {
    const { s } = setup();
    start(s);
    s.topLoad(1, "https://typeform.com");
    expect(s.run(1)?.phase).toBe("press");
    s.pressed(1);
    s.topLoad(1, "https://typeform.com");
    expect(s.run(1)?.phase).toBe("choose");
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
