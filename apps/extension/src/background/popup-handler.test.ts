import { describe, expect, it } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import { parsePopupRequest } from "../messaging/popup";
import { pageUrlForRequest } from "../shared/url";
import { createPopupHandler } from "./popup-handler";

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

function fakeClient(answer: (r: Request) => unknown) {
  const seen: Request[] = [];
  return {
    seen,
    request: (async (r: Request) => {
      seen.push(r);
      return answer(r);
    }) as never,
  };
}

describe("pageUrlForRequest", () => {
  it("strips credentials, query and fragment", () => {
    expect(pageUrlForRequest("https://user:pw@github.com/login?token=abc#frag")).toBe("https://github.com/login");
  });
  it("rejects non-web pages", () => {
    for (const u of [undefined, "", "chrome://settings", "about:blank", "file:///etc/passwd", "javascript:alert(1)", "data:text/html,x", "not a url"]) {
      expect(pageUrlForRequest(u)).toBeNull();
    }
  });
  it("rejects over-long URLs", () => {
    expect(pageUrlForRequest(`https://a.com/${"a".repeat(5000)}`)).toBeNull();
  });
});

describe("parsePopupRequest", () => {
  it("accepts the three popup requests", () => {
    expect(parsePopupRequest({ type: "popup_state" })).toEqual({ type: "popup_state" });
    expect(parsePopupRequest({ type: "popup_lock" })).toEqual({ type: "popup_lock" });
    expect(parsePopupRequest({ type: "popup_totp", itemId: ID })).toEqual({ type: "popup_totp", itemId: ID });
  });
  it("rejects everything else, including a popup-supplied URL", () => {
    for (const m of [
      null,
      [],
      { type: "popup_state", url: "https://evil.com" },
      { type: "popup_totp", itemId: ID, url: "https://github.com" },
      { type: "popup_totp", itemId: "x" },
      { type: "fill_item", itemId: ID, url: "https://github.com" },
    ]) {
      expect(parsePopupRequest(m)).toBeNull();
    }
  });
});

describe("popup handler", () => {
  it("looks up matches for the active tab, without query or fragment", async () => {
    const c = fakeClient((r) =>
      r.type === "status"
        ? { type: "status", state: "unlocked", vaultExists: true }
        : { type: "find_matches", matches: [] },
    );
    const h = createPopupHandler(c, async () => "https://github.com/login?next=x#y");
    const r = await h.handle({ type: "popup_state" });
    expect(r).toEqual({ ok: true, value: { kind: "unlocked", site: "github.com", matches: [] } });
    expect(c.seen[1]).toEqual({ type: "find_matches", url: "https://github.com/login" });
  });

  it("does not query matches while locked", async () => {
    const c = fakeClient(() => ({ type: "status", state: "locked", vaultExists: true }));
    const h = createPopupHandler(c, async () => "https://github.com/");
    expect(await h.handle({ type: "popup_state" })).toEqual({ ok: true, value: { kind: "locked" } });
    expect(c.seen).toHaveLength(1);
  });

  it("uses the tab URL, not anything from the popup, for TOTP", async () => {
    const c = fakeClient(() => ({ type: "get_totp", code: "123456", period: 30, secondsRemaining: 9 }));
    const h = createPopupHandler(c, async () => "https://github.com/");
    const r = await h.handle({ type: "popup_totp", itemId: ID });
    expect(r).toEqual({ ok: true, value: { code: "123456", secondsRemaining: 9 } });
    expect(c.seen[0]).toEqual({ type: "get_totp", itemId: ID, url: "https://github.com/" });
  });

  it("refuses TOTP on non-web pages without contacting the host", async () => {
    const c = fakeClient(() => ({}));
    const h = createPopupHandler(c, async () => "chrome://newtab/");
    expect((await h.handle({ type: "popup_totp", itemId: ID })).ok).toBe(false);
    expect(c.seen).toHaveLength(0);
  });

  it("maps connection errors to states", async () => {
    for (const [code, kind] of [
      ["host_unavailable", "host_unavailable"],
      ["desktop_unavailable", "desktop_unavailable"],
      ["integration_disabled", "disabled"],
      ["locked", "locked"],
    ] as const) {
      const c = fakeClient(() => {
        throw new BridgeError(code, "x");
      });
      const h = createPopupHandler(c, async () => "https://github.com/");
      const r = await h.handle({ type: "popup_state" });
      expect(r.ok && r.value).toEqual({ kind });
    }
  });
});
