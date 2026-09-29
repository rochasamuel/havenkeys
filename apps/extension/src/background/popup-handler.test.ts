import { describe, expect, it } from "vitest";
import type { IdentityRole, Request } from "@havenkeys/protocol";
import type { FillPayload } from "../messaging/inline";
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
  it("accepts the popup requests", () => {
    expect(parsePopupRequest({ type: "popup_fill", itemId: ID })).toEqual({ type: "popup_fill", itemId: ID });
    expect(parsePopupRequest({ type: "popup_fill_totp", itemId: ID })).toEqual({ type: "popup_fill_totp", itemId: ID });
    expect(parsePopupRequest({ type: "popup_state" })).toEqual({ type: "popup_state" });
    expect(parsePopupRequest({ type: "popup_lock" })).toEqual({ type: "popup_lock" });
    expect(parsePopupRequest({ type: "popup_totp", itemId: ID })).toEqual({ type: "popup_totp", itemId: ID });
    expect(parsePopupRequest({ type: "popup_open_item", itemId: ID })).toEqual({ type: "popup_open_item", itemId: ID });
  });
  it("rejects everything else, including a popup-supplied URL", () => {
    for (const m of [
      null,
      [],
      { type: "popup_state", url: "https://evil.com" },
      { type: "popup_totp", itemId: ID, url: "https://github.com" },
      { type: "popup_totp", itemId: "x" },
      { type: "fill_item", itemId: ID, url: "https://github.com" },
      { type: "popup_fill", itemId: ID, url: "https://github.com" },
      { type: "popup_fill", itemId: ID, tabId: 3 },
      { type: "popup_open_item", itemId: ID, url: "https://github.com" },
      { type: "popup_open_item", itemId: "x" },
      { type: "open_item", itemId: ID, url: "https://github.com" },
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
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/login?next=x#y" }));
    const r = await h.handle({ type: "popup_state" });
    expect(r).toEqual({ ok: true, value: { kind: "unlocked", site: "github.com", matches: [], identity: null } });
    expect(c.seen[1]).toEqual({ type: "find_matches", url: "https://github.com/login" });
  });

  it("does not query matches while locked", async () => {
    const c = fakeClient(() => ({ type: "status", state: "locked", vaultExists: true }));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/" }));
    expect(await h.handle({ type: "popup_state" })).toEqual({ ok: true, value: { kind: "locked" } });
    expect(c.seen).toHaveLength(1);
  });

  it("uses the tab URL, not anything from the popup, for TOTP", async () => {
    const c = fakeClient(() => ({ type: "get_totp", code: "123456", period: 30, secondsRemaining: 9, autoSubmit: false }));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/" }));
    const r = await h.handle({ type: "popup_totp", itemId: ID });
    expect(r).toEqual({ ok: true, value: { code: "123456", secondsRemaining: 9 } });
    expect(c.seen[0]).toEqual({ type: "get_totp", itemId: ID, url: "https://github.com/" });
  });

  it("refuses TOTP on non-web pages without contacting the host", async () => {
    const c = fakeClient(() => ({}));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "chrome://newtab/" }));
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
      const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/" }));
      const r = await h.handle({ type: "popup_state" });
      expect(r.ok && r.value).toEqual({ kind });
    }
  });

  it("fills the active tab using the tab's own URL", async () => {
    const c = fakeClient((r) =>
      r.type === "find_matches"
        ? { type: "find_matches", matches: [] }
        : { type: "fill_item", username: "octo", password: "pw", autoSubmit: false },
    );
    const fills: unknown[] = [];
    const h = createPopupHandler(c, async () => ({ id: 7, url: "https://github.com/login?x=1" }), async (...a) => {
      fills.push(a);
      return 2;
    });
    expect(await h.handle({ type: "popup_fill", itemId: ID })).toEqual({ ok: true, value: null });
    expect(c.seen[0]).toEqual({ type: "find_matches", url: "https://github.com/login" });
    expect(c.seen[1]).toEqual({ type: "fill_item", itemId: ID, url: "https://github.com/login" });
    expect(fills).toEqual([[7, "https://github.com/login", { kind: "login", username: "octo", password: "pw" }, null]]);
  });

  it("passes a run to the tab filler when Rust says autoSubmit, with the item's TOTP flag", async () => {
    const c = fakeClient((r) =>
      r.type === "fill_item"
        ? { type: "fill_item", username: "octo", password: "pw", autoSubmit: true }
        : { type: "find_matches", matches: [{ id: ID, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host", provider: null }] },
    );
    const fills: unknown[][] = [];
    const h = createPopupHandler(c, async () => ({ id: 7, url: "https://github.com/login" }), async (...a) => {
      fills.push(a);
      return 2;
    });
    await h.handle({ type: "popup_fill", itemId: ID });
    expect(fills[0]?.[3]).toEqual({ itemId: ID, hasTotp: true });
    expect(c.seen.map((r) => r.type)).toEqual(["find_matches", "fill_item"]);
  });

  function providerLogin(password: string | null) {
    const c = fakeClient((r) =>
      r.type === "fill_item"
        ? { type: "fill_item", username: password === null ? null : "me", password, autoSubmit: false }
        : {
            type: "find_matches",
            matches: [{ id: ID, title: "Typeform", username: "me@gmail.com", hasTotp: false, strength: "same_site", provider: "google" }],
          },
    );
    const fills: unknown[] = [];
    const starts: unknown[] = [];
    const h = createPopupHandler(
      c,
      async () => ({ id: 7, url: "https://typeform.com/login?x=1" }),
      async (...a) => {
        fills.push(a);
        return 2;
      },
      async (...a) => {
        starts.push(a);
        return { ok: true, value: null };
      },
    );
    return { c, h, fills, starts };
  }

  it("starts a sign-in-with run for a login saved with a provider and no password, without filling", async () => {
    const { c, h, fills, starts } = providerLogin(null);
    expect(await h.handle({ type: "popup_fill", itemId: ID })).toEqual({ ok: true, value: null });
    expect(starts).toEqual([[7, "https://typeform.com/login", ID]]);
    expect(fills).toEqual([]);
    expect(c.seen.map((r) => r.type)).toEqual(["find_matches", "fill_item"]);
  });

  it("fills the password of a login saved with a provider and a password", async () => {
    const { h, fills, starts } = providerLogin("pw");
    expect(await h.handle({ type: "popup_fill", itemId: ID })).toEqual({ ok: true, value: null });
    expect(starts).toEqual([]);
    expect(fills).toEqual([[7, "https://typeform.com/login", { kind: "login", username: "me", password: "pw" }, null]]);
  });

  it("passes a code-only run for a popup TOTP fill", async () => {
    const c = fakeClient(() => ({ type: "get_totp", code: "123456", period: 30, secondsRemaining: 9, autoSubmit: true }));
    const fills: unknown[][] = [];
    const h = createPopupHandler(c, async () => ({ id: 7, url: "https://github.com/login" }), async (...a) => {
      fills.push(a);
      return 1;
    });
    await h.handle({ type: "popup_fill_totp", itemId: ID });
    expect(fills[0]?.[3]).toEqual({ itemId: ID, hasTotp: true });
  });

  it("reports pages without a login form, and refuses non-web pages", async () => {
    const c = fakeClient(() => ({ type: "get_totp", code: "123456", period: 30, secondsRemaining: 9, autoSubmit: false }));
    const h = createPopupHandler(c, async () => ({ id: 7, url: "https://github.com/" }), async () => 0);
    expect(await h.handle({ type: "popup_fill_totp", itemId: ID })).toEqual({
      ok: false,
      message: "No login form found on this page.",
    });
    const h2 = createPopupHandler(c, async () => ({ id: 7, url: "about:blank" }), async () => 1);
    expect((await h2.handle({ type: "popup_fill", itemId: ID })).ok).toBe(false);
    expect(c.seen).toHaveLength(1);
  });

  it("asks the desktop to open the login, using the tab's URL", async () => {
    const c = fakeClient(() => ({ type: "open_item" }));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/login?next=x#y" }));
    expect(await h.handle({ type: "popup_open_item", itemId: ID })).toEqual({ ok: true, value: null });
    expect(c.seen).toEqual([{ type: "open_item", itemId: ID, url: "https://github.com/login" }]);
  });

  it("refuses to open on non-web pages without contacting the host", async () => {
    const c = fakeClient(() => ({}));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "chrome://newtab/" }));
    expect((await h.handle({ type: "popup_open_item", itemId: ID })).ok).toBe(false);
    expect(c.seen).toHaveLength(0);
  });

  it("passes the desktop's refusal to the popup", async () => {
    const c = fakeClient(() => {
      throw new BridgeError("denied", "This item is not saved for this website.");
    });
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://evil.com/" }));
    expect(await h.handle({ type: "popup_open_item", itemId: ID })).toEqual({
      ok: false,
      message: "This item is not saved for this website.",
    });
  });
});

const unlocked = { type: "status", state: "unlocked", vaultExists: true };

function handlerWith(answers: Record<string, unknown>, url = "https://shop.com/") {
  const requests: Request[] = [];
  const client = {
    request: (async (r: Request) => {
      requests.push(r);
      const a = answers[r.type];
      if (a === undefined) throw new Error("unexpected request");
      return a;
    }) as never,
  };
  return { h: createPopupHandler(client, async () => ({ id: 1, url })), requests };
}

function identitySetup(pageRoles: IdentityRole[], summary: { roles: IdentityRole[] }, url = "https://shop.com/") {
  const requests: Request[] = [];
  const filled: FillPayload[] = [];
  const client = {
    request: (async (r: Request) => {
      requests.push(r);
      if (r.type === "find_identity") return { type: "find_identity", title: "Samuel", email: null, ...summary };
      if (r.type === "fill_identity") return { type: "fill_identity", values: [{ role: "fullName", value: "Samuel" }] };
      throw new Error("unexpected request");
    }) as never,
  };
  const h = createPopupHandler(
    client,
    async () => ({ id: 1, url }),
    async (_tab, _url, payload) => {
      filled.push(payload);
      return 1;
    },
    undefined,
    async () => pageRoles,
  );
  return { h, requests, filled };
}

describe("popup identity fill", () => {
  it("offers the identity when it has values", async () => {
    const { h } = handlerWith({ status: unlocked, find_matches: { type: "find_matches", matches: [] }, find_identity: { type: "find_identity", title: "Samuel", email: null, roles: ["fullName"] } });
    const r = await h.handle({ type: "popup_state" });
    expect(r).toMatchObject({ ok: true, value: { kind: "unlocked", identity: { title: "Samuel" } } });
  });

  it("asks about documents first, then fills with the answer", async () => {
    const { h, requests, filled } = identitySetup(["fullName", "cpf"], { roles: ["fullName", "cpf"] });
    expect(await h.handle({ type: "popup_fill_identity", documents: null })).toEqual({ ok: true, value: { confirm: ["cpf"] } });
    expect(requests.some((x) => x.type === "fill_identity")).toBe(false);
    expect(await h.handle({ type: "popup_fill_identity", documents: false })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toMatchObject({ type: "fill_identity", roles: ["fullName"], documents: false });
    expect(filled.at(-1)).toMatchObject({ kind: "identity" });
  });

  it("says so when the page has no identity form", async () => {
    const { h } = identitySetup([], { roles: ["fullName"] });
    expect(await h.handle({ type: "popup_fill_identity", documents: null })).toMatchObject({ ok: false });
  });

  it("never asks for or sends documents on http", async () => {
    const { h, requests } = identitySetup(["fullName", "cpf"], { roles: ["fullName", "cpf"] }, "http://shop.com/");
    expect(await h.handle({ type: "popup_fill_identity", documents: true })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toMatchObject({ roles: ["fullName"], documents: false });
  });

  it("parses the request strictly", () => {
    expect(parsePopupRequest({ type: "popup_fill_identity", documents: null })).toEqual({ type: "popup_fill_identity", documents: null });
    expect(parsePopupRequest({ type: "popup_fill_identity", documents: true })).toEqual({ type: "popup_fill_identity", documents: true });
    expect(parsePopupRequest({ type: "popup_fill_identity", documents: "yes" })).toBeNull();
    expect(parsePopupRequest({ type: "popup_fill_identity" })).toBeNull();
    expect(parsePopupRequest({ type: "popup_fill_identity", documents: true, url: "x" })).toBeNull();
  });
});
