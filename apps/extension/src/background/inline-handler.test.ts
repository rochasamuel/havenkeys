import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import {
  parseBackgroundMessage,
  parseContentRequest,
  parseInlineRequest,
  type BackgroundToContent,
} from "../messaging/inline";
import { createInlineHandler, MENU_TTL_MS, SAVE_TTL_MS, type FrameRef } from "./inline-handler";

const GH = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const OTHER = "11111111-2222-4333-8444-555555555555";
const T1 = "0".repeat(31) + "1";

const ghMatch = { id: GH, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host" as const };

function frame(over: Partial<FrameRef> = {}): FrameRef {
  return { tabId: 1, frameId: 0, url: "https://github.com/login", origin: "https://github.com", ...over };
}

function setup(answer: (r: Request) => unknown = defaultAnswer) {
  const requests: Request[] = [];
  const sent: Array<{ to: { tabId: number; frameId: number }; msg: BackgroundToContent }> = [];
  let n = 0;
  let clock = 1_000;
  const h = createInlineHandler({
    client: {
      request: (async (r: Request) => {
        requests.push(r);
        return answer(r);
      }) as never,
    },
    sendToFrame: async (to, msg) => {
      // The inline handler only sends BackgroundToContent; BgWaResult is the passkey handler's.
      sent.push({ to: { tabId: to.tabId, frameId: to.frameId }, msg: msg as BackgroundToContent });
      return msg.type === "bg_fill" ? { filled: 2 } : undefined;
    },
    now: () => clock,
    newToken: () => (++n).toString(16).padStart(32, "0"),
  });
  return { h, requests, sent, advance: (ms: number) => (clock += ms) };
}

function defaultAnswer(r: Request): unknown {
  switch (r.type) {
    case "find_matches":
      return { type: "find_matches", matches: r.url.startsWith("https://github.com") ? [ghMatch] : [] };
    case "fill_item":
      return { type: "fill_item", username: "octo", password: "pw" };
    case "get_totp":
      return { type: "get_totp", code: "123456", period: 30, secondsRemaining: 10 };
    case "generate_password":
      return { type: "generate_password", password: "Gen!" };
    case "check_login":
      return { type: "check_login", action: "add", itemId: null };
    case "save_login":
      return { type: "save_login", itemId: GH };
    default:
      throw new Error(`unexpected ${r.type}`);
  }
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("message validation", () => {
  it("accepts exact content requests only", () => {
    expect(parseContentRequest({ type: "cs_open_menu", kind: "login" })).toEqual({ type: "cs_open_menu", kind: "login" });
    expect(parseContentRequest({ type: "cs_submit", username: null, password: "x" })).not.toBeNull();
    for (const bad of [
      { type: "cs_open_menu", kind: "login", url: "https://github.com" },
      { type: "cs_open_menu", kind: "everything" },
      { type: "cs_submit", username: null, password: null },
      { type: "cs_submit", username: "", password: "x" },
      { type: "cs_submit", username: null, password: "x".repeat(4097) },
      { type: "cs_submit", username: "u".repeat(513), password: "x" },
      { type: "cs_close_menu", token: "short" },
      { type: "fill_item", itemId: GH, url: "https://github.com" },
      null,
      "cs_ready",
    ]) {
      expect(parseContentRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("accepts exact menu/save frame requests only", () => {
    expect(parseInlineRequest({ type: "menu_pick", token: T1, itemId: GH })).not.toBeNull();
    for (const bad of [
      { type: "menu_pick", token: T1, itemId: "x" },
      { type: "menu_pick", token: "nope", itemId: GH },
      { type: "menu_state", token: T1, url: "https://evil.com" },
      { type: "save_confirm", token: T1, password: "x" },
    ]) {
      expect(parseInlineRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("content script only accepts well-formed background messages", () => {
    expect(
      parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "otp", code: "123456" } }),
    ).not.toBeNull();
    for (const bad of [
      { type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "otp", code: "12ab56" } },
      { type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "login", username: "a" } },
      { type: "bg_fill", origin: "https://a.com", token: "x", fill: { kind: "generated", password: "p" } },
      { type: "bg_fill", token: null, fill: { kind: "generated", password: "p" } },
      { type: "bg_eval", code: "alert(1)" },
    ]) {
      expect(parseBackgroundMessage(bad), JSON.stringify(bad)).toBeNull();
    }
  });
});

describe("suggestion menus", () => {
  it("opens with no secrets, then fills the frame the user clicked in", async () => {
    const { h, requests, sent } = setup();
    const r = await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(r).toEqual({ ok: true, token: T1, rows: 1 });
    const state = await h.handleInline(1, { type: "menu_state", token: T1 });
    expect(state).toEqual({
      ok: true,
      value: { state: "ready", kind: "login", site: "github.com", items: [{ id: GH, title: "GitHub", username: "octo" }] },
    });
    expect(JSON.stringify(state)).not.toContain("pw");

    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "fill_item", itemId: GH, url: "https://github.com/login" });
    expect(sent.map((s) => s.msg.type)).toEqual(["bg_close_menu", "bg_fill"]);
    expect(sent[1]).toEqual({
      to: { tabId: 1, frameId: 0 },
      msg: { type: "bg_fill", origin: "https://github.com", token: T1, fill: { kind: "login", username: "octo", password: "pw" } },
    });
    // Single use.
    expect((await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).ok).toBe(false);
  });

  it("sends the top page URL for iframes so the desktop can check both", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame({ frameId: 5, topUrl: "https://evil.com/" }), { type: "cs_open_menu", kind: "login" });
    expect(requests[0]).toEqual({ type: "find_matches", url: "https://github.com/login", topUrl: "https://evil.com/" });
  });

  it("stays out of the page when nothing matches or the app is unavailable", async () => {
    const { h } = setup();
    expect(await h.handleContent(frame({ url: "https://evil.com/" }), { type: "cs_open_menu", kind: "login" })).toEqual({
      ok: false,
    });
    for (const code of ["desktop_unavailable", "integration_disabled", "host_unavailable"] as const) {
      const { h: h2 } = setup(() => {
        throw new BridgeError(code, "x");
      });
      expect(await h2.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: false });
    }
  });

  it("shows a locked menu that cannot fill", async () => {
    const { h, requests } = setup(() => {
      throw new BridgeError("locked", "x");
    });
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 1 });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toEqual({ ok: true, value: { state: "locked" } });
    expect((await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).ok).toBe(false);
    expect(requests).toHaveLength(1);
  });

  it("A2: a menu frame cannot pick items the menu did not offer, or act for another tab", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: OTHER })).toEqual({
      ok: false,
      message: "Unknown item.",
    });
    expect((await h.handleInline(2, { type: "menu_pick", token: T1, itemId: GH })).ok).toBe(false);
    expect((await h.handleInline(1, { type: "menu_pick", token: "f".repeat(32), itemId: GH })).ok).toBe(false);
    expect(requests.map((r) => r.type)).toEqual(["find_matches"]);
  });

  it("menus expire", async () => {
    const { h, advance } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    advance(MENU_TTL_MS + 1);
    expect((await h.handleInline(1, { type: "menu_state", token: T1 })).ok).toBe(false);
  });

  it("OTP menus list only TOTP logins and fill only the code", async () => {
    const { h, requests, sent } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "otp" });
    await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH });
    expect(requests.at(-1)?.type).toBe("get_totp");
    expect(sent.at(-1)?.msg).toMatchObject({ type: "bg_fill", fill: { kind: "otp", code: "123456" } });
  });

  it("new-password menus generate in the desktop and offer nothing else", async () => {
    const { h, sent } = setup();
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "new_password" })).toMatchObject({ ok: true });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ value: { kind: "new_password", items: [] } });
    expect((await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).ok).toBe(false);
    await h.handleInline(1, { type: "menu_generate", token: T1 });
    expect(sent.at(-1)?.msg).toMatchObject({ fill: { kind: "generated", password: "Gen!" } });
  });

  it("locking drops menus", async () => {
    const { h } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    h.reset();
    expect((await h.handleInline(1, { type: "menu_state", token: T1 })).ok).toBe(false);
  });
});

describe("save prompts", () => {
  it("asks the desktop, prompts in the top frame, saves on confirm", async () => {
    const { h, requests, sent } = setup();
    await h.handleContent(frame({ frameId: 3, topUrl: "https://github.com/" }), {
      type: "cs_submit",
      username: "octo",
      password: "typed",
    });
    expect(requests[0]).toEqual({
      type: "check_login",
      url: "https://github.com/login",
      topUrl: "https://github.com/",
      username: "octo",
      password: "typed",
    });
    expect(sent[0]).toEqual({ to: { tabId: 1, frameId: 0 }, msg: { type: "bg_show_save", token: T1 } });
    const state = await h.handleInline(1, { type: "save_state", token: T1 });
    expect(state).toEqual({ ok: true, value: { action: "add", site: "github.com", username: "octo" } });
    expect(JSON.stringify(state)).not.toContain("typed");
    expect(await h.handleInline(1, { type: "save_confirm", token: T1 })).toEqual({ ok: true, value: null });
    expect(requests[1]).toEqual({
      type: "save_login",
      url: "https://github.com/login",
      topUrl: "https://github.com/",
      username: "octo",
      password: "typed",
      itemId: null,
    });
    // Gone once used.
    expect((await h.handleInline(1, { type: "save_state", token: T1 })).ok).toBe(false);
  });

  it("does not prompt for unchanged logins", async () => {
    const { h, sent } = setup((r) =>
      r.type === "check_login" ? { type: "check_login", action: "unchanged", itemId: null } : defaultAnswer(r),
    );
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect(sent).toEqual([]);
  });

  it("joins a username step with the following password step on the same origin", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: null });
    await h.handleContent(frame(), { type: "cs_submit", username: null, password: "pw2" });
    expect(requests[0]).toMatchObject({ type: "check_login", username: "octo", password: "pw2" });
    // Not across origins.
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: null });
    await h.handleContent(frame({ origin: "https://evil.com", url: "https://evil.com/" }), {
      type: "cs_submit",
      username: null,
      password: "pw3",
    });
    expect(requests[1]).toMatchObject({ username: null, password: "pw3" });
  });

  it("the new page gets the prompt after navigation; subframes don't", async () => {
    const { h } = setup();
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect(await h.handleContent(frame({ frameId: 2, topUrl: "https://github.com/" }), { type: "cs_ready" })).toEqual({
      saveToken: null,
    });
    expect(await h.handleContent(frame({ url: "https://github.com/dashboard" }), { type: "cs_ready" })).toEqual({
      saveToken: T1,
    });
  });

  it("drops the pending password on dismiss, expiry and lock", async () => {
    for (const end of ["dismiss", "expire", "lock"] as const) {
      const { h, requests } = setup();
      await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
      if (end === "dismiss") await h.handleInline(1, { type: "save_dismiss", token: T1 });
      if (end === "expire") vi.advanceTimersByTime(SAVE_TTL_MS + 1);
      if (end === "lock") h.reset();
      expect((await h.handleInline(1, { type: "save_confirm", token: T1 })).ok, end).toBe(false);
      expect(requests.filter((r) => r.type === "save_login"), end).toEqual([]);
    }
  });

  it("a save prompt in another tab cannot confirm this tab's save", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect((await h.handleInline(2, { type: "save_confirm", token: T1 })).ok).toBe(false);
    expect(requests.filter((r) => r.type === "save_login")).toEqual([]);
  });

  it("no prompt when the desktop refuses (locked, rate limited)", async () => {
    const { h, sent } = setup(() => {
      throw new BridgeError("locked", "x");
    });
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect(sent).toEqual([]);
  });
});
