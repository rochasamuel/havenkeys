import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import {
  parseBackgroundMessage,
  parseContentRequest,
  parseFillReply,
  parseInlineRequest,
  MENU_MAX_HEIGHT,
  MENU_MIN_HEIGHT,
  SAVE_MAX_HEIGHT,
  SAVE_MIN_HEIGHT,
  type BackgroundToContent,
} from "../messaging/inline";
import { createInlineHandler, MENU_TTL_MS, SAVE_TTL_MS, type FrameRef, type InlineDeps } from "./inline-handler";

const GH = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const OTHER = "11111111-2222-4333-8444-555555555555";
const T1 = "0".repeat(31) + "1";
const GH_SITE = { name: "GitHub", domains: ["github.com"], passwordless: true, mfa: true, help: "https://docs.github.com/passkeys" };

const ghMatch = { id: GH, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host" as const, provider: null };

function frame(over: Partial<FrameRef> = {}): FrameRef {
  return { tabId: 1, frameId: 0, url: "https://github.com/login", origin: "https://github.com", ...over };
}

function setup(
  answer: (r: Request) => unknown = defaultAnswer,
  extra: Partial<InlineDeps> = {},
  pressing: (msg: BackgroundToContent) => string | null = () => null,
) {
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
      return msg.type === "bg_fill" ? { filled: 2, pressing: pressing(msg as BackgroundToContent) } : undefined;
    },
    now: () => clock,
    newToken: () => (++n).toString(16).padStart(32, "0"),
    ...extra,
  });
  return { h, requests, sent, advance: (ms: number) => (clock += ms) };
}

function defaultAnswer(r: Request): unknown {
  switch (r.type) {
    case "find_matches":
      return { type: "find_matches", matches: r.url.startsWith("https://github.com") ? [ghMatch] : [] };
    case "fill_item":
      return { type: "fill_item", username: "octo", password: "pw", autoSubmit: false };
    case "get_totp":
      return { type: "get_totp", code: "123456", period: 30, secondsRemaining: 10, autoSubmit: false };
    case "generate_password":
      return { type: "generate_password", password: "Gen!" };
    case "generator_options":
      return { type: "generator_options", options: { length: 32, uppercase: true, lowercase: true, digits: true, symbols: false, avoidAmbiguous: true } };
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
    expect(parseContentRequest({ type: "cs_open_menu", kind: "login", explicit: true })).toEqual({
      type: "cs_open_menu",
      kind: "login",
      explicit: true,
    });
    expect(parseContentRequest({ type: "cs_submit", username: null, password: "x" })).not.toBeNull();
    expect(parseContentRequest({ type: "cs_submit", username: null, password: "x", currentPassword: "old" })).toEqual({
      type: "cs_submit",
      username: null,
      password: "x",
      currentPassword: "old",
    });
    for (const bad of [
      { type: "cs_open_menu", kind: "login", url: "https://github.com" },
      { type: "cs_open_menu", kind: "everything" },
      { type: "cs_open_menu", kind: "login", explicit: false },
      { type: "cs_open_menu", kind: "login", explicit: "yes" },
      { type: "cs_submit", username: null, password: null },
      { type: "cs_submit", username: "", password: "x" },
      { type: "cs_submit", username: null, password: "x".repeat(4097) },
      { type: "cs_submit", username: "u".repeat(513), password: "x" },
      { type: "cs_submit", username: null, password: "x", currentPassword: "" },
      { type: "cs_submit", username: null, password: "x", currentPassword: null },
      { type: "cs_submit", username: null, password: "x", currentPassword: "x".repeat(4097) },
      { type: "cs_submit", username: "u", password: null, currentPassword: "old" },
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
    expect(parseInlineRequest({ type: "menu_pick_passkey", token: T1, itemId: GH, credentialId: "AQEBAQEBAQEBAQEBAQEBAQ" })).not.toBeNull();
    expect(parseInlineRequest({ type: "menu_open_help", token: T1 })).toEqual({ type: "menu_open_help", token: T1 });
    expect(parseInlineRequest({ type: "menu_show_unlock", token: T1 })).toEqual({ type: "menu_show_unlock", token: T1 });
    expect(parseInlineRequest({ type: "save_confirm", token: T1, title: "GitHub – work" })).toEqual({
      type: "save_confirm",
      token: T1,
      title: "GitHub – work",
    });
    for (const bad of [
      { type: "save_confirm", token: T1, title: null },
      { type: "save_confirm", token: T1, title: 7 },
      { type: "save_confirm", token: T1, title: "x".repeat(257) },
      { type: "menu_pick", token: T1, itemId: "x" },
      { type: "menu_pick", token: "nope", itemId: GH },
      { type: "menu_state", token: T1, url: "https://evil.com" },
      { type: "save_confirm", token: T1, password: "x" },
      { type: "menu_pick_passkey", token: T1, itemId: GH, credentialId: "AQ" },
      { type: "menu_open_help", token: T1, url: "https://evil.com" },
      { type: "menu_show_unlock", token: T1, itemId: GH },
    ]) {
      expect(parseInlineRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("bounds the height a menu frame may report", () => {
    expect(parseInlineRequest({ type: "menu_resize", token: T1, height: 120 })).toEqual({ type: "menu_resize", token: T1, height: 120 });
    expect(parseBackgroundMessage({ type: "bg_resize_menu", token: T1, height: 120 })).toEqual({ type: "bg_resize_menu", token: T1, height: 120 });
    for (const height of [MENU_MIN_HEIGHT - 1, MENU_MAX_HEIGHT + 1, 100.5, "120", null, Infinity]) {
      expect(parseInlineRequest({ type: "menu_resize", token: T1, height }), String(height)).toBeNull();
      expect(parseBackgroundMessage({ type: "bg_resize_menu", token: T1, height }), String(height)).toBeNull();
    }
    expect(parseInlineRequest({ type: "menu_resize", token: T1, height: 120, width: 900 })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_resize_menu", token: "x", height: 120 })).toBeNull();
  });

  it("bounds the height a save prompt may report", () => {
    expect(parseInlineRequest({ type: "save_resize", token: T1, height: 160 })).toEqual({ type: "save_resize", token: T1, height: 160 });
    expect(parseBackgroundMessage({ type: "bg_resize_save", token: T1, height: 160 })).toEqual({ type: "bg_resize_save", token: T1, height: 160 });
    for (const height of [SAVE_MIN_HEIGHT - 1, SAVE_MAX_HEIGHT + 1, 160.5, "160", null, Infinity]) {
      expect(parseInlineRequest({ type: "save_resize", token: T1, height }), String(height)).toBeNull();
      expect(parseBackgroundMessage({ type: "bg_resize_save", token: T1, height }), String(height)).toBeNull();
    }
    expect(parseInlineRequest({ type: "save_resize", token: T1, height: 160, width: 900 })).toBeNull();
    expect(parseInlineRequest({ type: "save_resize", token: "x", height: 160 })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_resize_save", token: "x", height: 160 })).toBeNull();
  });

  it("content script only accepts well-formed background messages", () => {
    expect(
      parseBackgroundMessage({
        type: "bg_fill",
        origin: "https://a.com",
        token: null,
        fill: { kind: "otp", code: "123456" },
        submit: false,
        totp: false,
      }),
    ).not.toBeNull();
    for (const bad of [
      { type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "otp", code: "12ab56" }, submit: false, totp: false },
      { type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "login", username: "a" }, submit: false, totp: false },
      { type: "bg_fill", origin: "https://a.com", token: "x", fill: { kind: "generated", password: "p" }, submit: false, totp: false },
      { type: "bg_fill", token: null, fill: { kind: "generated", password: "p" }, submit: false, totp: false },
      { type: "bg_eval", code: "alert(1)" },
    ]) {
      expect(parseBackgroundMessage(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("validates run messages", () => {
    expect(parseContentRequest({ type: "cs_run_step", kind: "password" })).toEqual({ type: "cs_run_step", kind: "password" });
    expect(parseContentRequest({ type: "cs_run_step", kind: "otp" })).toEqual({ type: "cs_run_step", kind: "otp" });
    expect(parseContentRequest({ type: "cs_run_stop" })).toEqual({ type: "cs_run_stop" });
    for (const bad of [
      { type: "cs_run_step", kind: "username" },
      { type: "cs_run_step", kind: "password", itemId: GH },
      { type: "cs_run_step" },
      { type: "cs_run_stop", reason: "x" },
    ]) {
      expect(parseContentRequest(bad)).toBeNull();
    }
    const fillMsg = { type: "bg_fill", origin: "https://github.com", token: null, fill: { kind: "otp", code: "123456" }, submit: true, totp: false };
    expect(parseBackgroundMessage(fillMsg)).toEqual(fillMsg);
    expect(parseBackgroundMessage({ ...fillMsg, submit: "yes" })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://github.com", token: null, fill: fillMsg.fill })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_run_end" })).toEqual({ type: "bg_run_end" });
    expect(parseBackgroundMessage({ type: "bg_run_end", token: T1 })).toBeNull();
  });

  it("reads fill replies defensively", () => {
    expect(parseFillReply({ filled: 2, pressing: "password" })).toEqual({ filled: 2, pressing: "password" });
    expect(parseFillReply({ filled: 1, pressing: null })).toEqual({ filled: 1, pressing: null });
    for (const bad of [undefined, null, { filled: "2" }, { filled: 2, pressing: "everything" }, { filled: -1, pressing: null }]) {
      expect(parseFillReply(bad)).toEqual({ filled: 0, pressing: null });
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
      value: {
        state: "ready",
        kind: "login",
        site: "github.com",
        items: [{ id: GH, title: "GitHub", username: "octo", provider: null }],
        passkeys: [],
        hint: null,
        identity: null,
      },
    });
    expect(JSON.stringify(state)).not.toContain("pw");

    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "fill_item", itemId: GH, url: "https://github.com/login" });
    expect(sent.map((s) => s.msg.type)).toEqual(["bg_close_menu", "bg_fill"]);
    expect(sent[1]).toEqual({
      to: { tabId: 1, frameId: 0 },
      msg: {
        type: "bg_fill",
        origin: "https://github.com",
        token: T1,
        fill: { kind: "login", username: "octo", password: "pw" },
        submit: false,
        totp: false,
      },
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

  it("opens an empty menu when the user asked for it from the field icon", async () => {
    const { h } = setup();
    const evil = frame({ url: "https://evil.com/" });
    expect(await h.handleContent(evil, { type: "cs_open_menu", kind: "login", explicit: true })).toEqual({ ok: true, token: T1, rows: 1 });
    const view = await h.handleInline(1, { type: "menu_state", token: T1 });
    expect(view).toMatchObject({ ok: true, value: { state: "ready", items: [], passkeys: [] } });
    // Still nothing to pick.
    expect((await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).ok).toBe(false);
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

  it("a locked menu's Unlock raises the desktop and closes the menu; only for its own tab, once", async () => {
    const { h, requests, sent } = setup((r) => {
      if (r.type === "find_matches") throw new BridgeError("locked", "x");
      return null;
    });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect((await h.handleInline(2, { type: "menu_show_unlock", token: T1 })).ok).toBe(false);
    expect(await h.handleInline(1, { type: "menu_show_unlock", token: T1 })).toEqual({ ok: true, value: null });
    expect(requests.map((r) => r.type)).toEqual(["find_matches", "show_unlock"]);
    expect(sent.map((s) => s.msg.type)).toEqual(["bg_close_menu"]);
    // The menu closed: a second click does nothing.
    expect((await h.handleInline(1, { type: "menu_show_unlock", token: T1 })).ok).toBe(false);
    expect(requests).toHaveLength(2);
  });

  it("an unlocked menu does not send show_unlock", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect((await h.handleInline(1, { type: "menu_show_unlock", token: T1 })).ok).toBe(false);
    expect(requests.some((r) => r.type === "show_unlock")).toBe(false);
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
    expect(requests.map((r) => r.type)).toEqual(["find_matches", "passkey_status"]);
  });

  const googleAnswer =
    (password: string | null) =>
    (r: Request): unknown =>
      r.type === "find_matches"
        ? { type: "find_matches", matches: [{ ...ghMatch, provider: "google" }] }
        : r.type === "fill_item"
          ? { type: "fill_item", username: password === null ? null : "octo", password, autoSubmit: false }
          : defaultAnswer(r);

  it("a login saved with a provider and no password starts a sign-in-with run instead of filling", async () => {
    const startSso = vi.fn(async () => ({ ok: true as const, value: null }));
    const { h, requests, sent } = setup(googleAnswer(null), { startSso });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({
      ok: true,
      value: { items: [{ id: GH, title: "GitHub", username: "octo", provider: "google" }] },
    });
    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).toEqual({ ok: true, value: null });
    expect(startSso).toHaveBeenCalledWith(frame(), GH);
    expect(requests.map((r) => r.type)).toEqual(["find_matches", "passkey_status", "fill_item"]);
    expect(sent.map((s) => s.msg.type)).toEqual(["bg_close_menu"]);
  });

  it("a login saved with a provider and a password fills the password", async () => {
    const startSso = vi.fn(async () => ({ ok: true as const, value: null }));
    const { h, sent } = setup(googleAnswer("pw"), { startSso });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).toEqual({ ok: true, value: null });
    expect(startSso).not.toHaveBeenCalled();
    expect(sent.find((s) => s.msg.type === "bg_fill")?.msg).toMatchObject({ fill: { kind: "login", username: "octo", password: "pw" } });
  });

  it("in a non-top frame, a provider login without a password starts no run (spec §6.2: only the top frame presses)", async () => {
    const startSso = vi.fn(async () => ({ ok: true as const, value: null }));
    const { h, sent } = setup(googleAnswer(null), { startSso });
    await h.handleContent(frame({ frameId: 3 }), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({
      ok: true,
      value: { items: [{ id: GH, title: "GitHub", username: "octo", provider: "google" }] },
    });
    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).toEqual({
      ok: false,
      message: "Couldn’t find the “Sign in with Google” button on this page.",
    });
    expect(startSso).not.toHaveBeenCalled();
    expect(sent.some((s) => s.msg.type === "bg_fill")).toBe(false);
  });

  it("in a non-top frame, a provider login with a password fills as usual", async () => {
    const startSso = vi.fn(async () => ({ ok: true as const, value: null }));
    const { h, sent } = setup(googleAnswer("pw"), { startSso });
    await h.handleContent(frame({ frameId: 3 }), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).toEqual({ ok: true, value: null });
    expect(startSso).not.toHaveBeenCalled();
    expect(sent.find((s) => s.msg.type === "bg_fill")).toMatchObject({ to: { tabId: 1, frameId: 3 }, msg: { fill: { password: "pw" } } });
  });

  it("passes the menu's measured height to the frame that opened it, for a live menu only", async () => {
    const { h, sent, advance } = setup();
    await h.handleContent(frame({ frameId: 3 }), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_resize", token: T1, height: 140 })).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toEqual({ to: { tabId: 1, frameId: 3 }, msg: { type: "bg_resize_menu", token: T1, height: 140 } });
    const before = sent.length;
    expect((await h.handleInline(2, { type: "menu_resize", token: T1, height: 140 })).ok).toBe(false);
    expect((await h.handleInline(1, { type: "menu_resize", token: "f".repeat(32), height: 140 })).ok).toBe(false);
    advance(MENU_TTL_MS + 1);
    expect((await h.handleInline(1, { type: "menu_resize", token: T1, height: 140 })).ok).toBe(false);
    expect(sent.slice(before).map((x) => x.msg.type)).toEqual(["bg_close_menu"]);
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

  it("passes the menu's generator options to the desktop", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "new_password" });
    const options = { length: 40, uppercase: false, lowercase: true, digits: true, symbols: false, avoidAmbiguous: false };
    await h.handleInline(1, { type: "menu_generate", token: T1, options });
    expect(requests.at(-1)).toEqual({ type: "generate_password", options });
  });

  it("without options, leaves the policy to the desktop's generator tab", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "new_password" });
    await h.handleInline(1, { type: "menu_generate", token: T1 });
    expect(requests.at(-1)).toEqual({ type: "generate_password" });
  });

  it("reads the desktop generator's saved policy for a new-password menu only", async () => {
    const { h, requests } = setup();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "new_password" });
    expect(await h.handleInline(1, { type: "menu_generator_options", token: T1 })).toEqual({
      ok: true,
      value: { length: 32, uppercase: true, lowercase: true, digits: true, symbols: false, avoidAmbiguous: true },
    });
    expect(requests.at(-1)).toEqual({ type: "generator_options" });

    const other = setup();
    await other.h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect((await other.h.handleInline(1, { type: "menu_generator_options", token: T1 })).ok).toBe(false);
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
    expect(state).toEqual({ ok: true, value: { action: "add", site: "github.com", username: "octo", title: "github.com" } });
    expect(JSON.stringify(state)).not.toContain("typed");
    expect(await h.handleInline(1, { type: "save_confirm", token: T1 })).toEqual({ ok: true, value: null });
    expect(requests[1]).toEqual({
      type: "save_login",
      url: "https://github.com/login",
      topUrl: "https://github.com/",
      username: "octo",
      password: "typed",
      itemId: null,
      title: "github.com",
    });
    // Gone once used.
    expect((await h.handleInline(1, { type: "save_state", token: T1 })).ok).toBe(false);
  });

  it("a change-password form: the current password only goes to check_login", async () => {
    const { h, requests } = setup((r) =>
      r.type === "check_login" ? { type: "check_login", action: "update", itemId: GH } : defaultAnswer(r),
    );
    await h.handleContent(frame(), { type: "cs_submit", username: null, password: "new-pw", currentPassword: "old-pw" });
    expect(requests[0]).toEqual({
      type: "check_login",
      url: "https://github.com/login",
      username: null,
      password: "new-pw",
      currentPassword: "old-pw",
    });
    // The prompt names the saved login's username, which the form did not have.
    const state = await h.handleInline(1, { type: "save_state", token: T1 });
    expect(state).toEqual({ ok: true, value: { action: "update", site: "github.com", username: "octo", title: null } });
    expect(await h.handleInline(1, { type: "save_confirm", token: T1 })).toEqual({ ok: true, value: null });
    const save = requests.find((r) => r.type === "save_login");
    expect(save).toMatchObject({ username: null, password: "new-pw", itemId: GH });
    expect(save).not.toHaveProperty("title");
    expect(JSON.stringify([state, save])).not.toContain("old-pw");
  });

  it("names a new login after the known site, or the host, and takes the user's title", async () => {
    const known = setup(defaultAnswer, { passkeySite: (url) => (url.startsWith("https://github.com") ? GH_SITE : null) });
    await known.h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect(await known.h.handleInline(1, { type: "save_state", token: T1 })).toMatchObject({ ok: true, value: { title: "GitHub" } });
    await known.h.handleInline(1, { type: "save_confirm", token: T1 });
    expect(known.requests.find((r) => r.type === "save_login")).toMatchObject({ title: "GitHub" });

    for (const [typed, saved] of [["  GitHub – work  ", "GitHub – work"], ["   ", "github.com"]] as const) {
      const { h, requests } = setup();
      await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
      expect(await h.handleInline(1, { type: "save_confirm", token: T1, title: typed })).toEqual({ ok: true, value: null });
      expect(requests.find((r) => r.type === "save_login")).toMatchObject({ title: saved });
    }
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
      watch: null,
    });
    expect(await h.handleContent(frame({ url: "https://github.com/dashboard" }), { type: "cs_ready" })).toEqual({
      saveToken: T1,
      watch: null,
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

  it("passes the save prompt's measured height to the top frame, for a live prompt only", async () => {
    const { h, sent } = setup();
    await h.handleContent(frame({ frameId: 3, topUrl: "https://github.com/" }), { type: "cs_submit", username: "octo", password: "pw" });
    expect(await h.handleInline(1, { type: "save_resize", token: T1, height: 170 })).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toEqual({ to: { tabId: 1, frameId: 0 }, msg: { type: "bg_resize_save", token: T1, height: 170 } });
    const before = sent.length;
    expect((await h.handleInline(2, { type: "save_resize", token: T1, height: 170 })).ok).toBe(false);
    expect((await h.handleInline(1, { type: "save_resize", token: "f".repeat(32), height: 170 })).ok).toBe(false);
    vi.advanceTimersByTime(SAVE_TTL_MS + 1);
    expect((await h.handleInline(1, { type: "save_resize", token: T1, height: 170 })).ok).toBe(false);
    expect(sent.slice(before).map((x) => x.msg.type)).not.toContain("bg_resize_save");
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

describe("passkeys in the field menu", () => {
  const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";
  const row = { itemId: GH, credentialId: CRED, title: "GitHub", userName: "octo" };

  function withPasskeys(rows = [row]) {
    const picks: unknown[] = [];
    const h = createInlineHandler({
      client: { request: (async () => ({ type: "find_matches", matches: [] })) as never },
      sendToFrame: async () => undefined,
      now: () => 1,
      newToken: () => T1,
      passkeys: {
        conditionalFor: () => rows,
        pickConditional: async (...args: unknown[]) => {
          picks.push(args);
          return { ok: true as const, value: null };
        },
      },
    });
    return { h, picks };
  }

  it("opens a menu for a waiting passkey request even with no password logins", async () => {
    const { h } = withPasskeys();
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 1 });
    const view = await h.handleInline(1, { type: "menu_state", token: T1 });
    expect(view).toMatchObject({ ok: true, value: { state: "ready", passkeys: [row], items: [] } });
  });

  it("signs only passkeys the menu offered, for the menu's frame", async () => {
    const { h, picks } = withPasskeys();
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect((await h.handleInline(1, { type: "menu_pick_passkey", token: T1, itemId: OTHER, credentialId: CRED })).ok).toBe(false);
    expect(await h.handleInline(1, { type: "menu_pick_passkey", token: T1, itemId: GH, credentialId: CRED })).toEqual({ ok: true, value: null });
    expect(picks).toEqual([[frame(), GH, CRED]]);
  });

  it("no passkeys without a waiting request", async () => {
    const { h } = withPasskeys([]);
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: false });
  });
});

describe("passkey hints in the login menu", () => {
  const status = (has: boolean) => (r: Request) => (r.type === "passkey_status" ? { type: "passkey_status", hasPasskey: has } : defaultAnswer(r));

  it("leads with a use-your-passkey hint when a passkey exists", async () => {
    const { h, requests } = setup(status(true), { passkeySite: () => GH_SITE });
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 2 });
    expect(requests.find((r) => r.type === "passkey_status")).toEqual({ type: "passkey_status", url: "https://github.com/login" });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ ok: true, value: { hint: { kind: "use_passkey" } } });
  });

  it("offers the directory help link when there is no passkey, and opens it only through the menu", async () => {
    const opened: string[] = [];
    const { h } = setup(status(false), { passkeySite: () => GH_SITE, openTab: (u) => void opened.push(u) });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ ok: true, value: { hint: { kind: "add_passkey", name: "GitHub" } } });
    expect((await h.handleInline(2, { type: "menu_open_help", token: T1 })).ok).toBe(false);
    expect(opened).toEqual([]);
    expect(await h.handleInline(1, { type: "menu_open_help", token: T1 })).toEqual({ ok: true, value: null });
    expect(opened).toEqual(["https://docs.github.com/passkeys"]);
    // The menu closed: a second click does nothing.
    expect((await h.handleInline(1, { type: "menu_open_help", token: T1 })).ok).toBe(false);
  });

  it("shows no hint without a help link, when the status fails, for unknown sites, or in OTP menus", async () => {
    const noHelp = setup(status(false), { passkeySite: () => ({ ...GH_SITE, help: null }) });
    await noHelp.h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(await noHelp.h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ value: { hint: null } });

    const failing = setup(defaultAnswer, { passkeySite: () => null }); // passkey_status throws in defaultAnswer
    expect(await failing.h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 1 });

    const otp = setup(status(true), { passkeySite: () => GH_SITE });
    await otp.h.handleContent(frame(), { type: "cs_open_menu", kind: "otp" });
    expect(otp.requests.some((r) => r.type === "passkey_status")).toBe(false);
  });

  it("never falls back to the directory row when passkey_status itself fails", async () => {
    // Even though the site is a known passkey site with a help link, a failed
    // status check must not be treated as "no passkey" -> no directory hint.
    const { h } = setup(defaultAnswer, { passkeySite: () => GH_SITE }); // passkey_status throws in defaultAnswer
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 1 });
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ ok: true, value: { hint: null } });
  });

  it("does not ask when the site offers passkey autofill (passkey rows already lead)", async () => {
    const row = { itemId: GH, credentialId: "AQEBAQEBAQEBAQEBAQEBAQ", title: "GitHub", userName: "octo" };
    const { h, requests } = setup(status(true), {
      passkeySite: () => GH_SITE,
      passkeys: { conditionalFor: () => [row], pickConditional: async () => ({ ok: true, value: null }) },
    });
    await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" });
    expect(requests.some((r) => r.type === "passkey_status")).toBe(false);
    expect(await h.handleInline(1, { type: "menu_state", token: T1 })).toMatchObject({ value: { hint: null } });
  });
});

describe("automatic sign-in", () => {
  const auto = (r: Request): unknown => {
    const base = defaultAnswer(r) as Record<string, unknown>;
    return r.type === "fill_item" || r.type === "get_totp" ? { ...base, autoSubmit: true } : base;
  };
  const pressWhenSubmit = (msg: BackgroundToContent) =>
    msg.type === "bg_fill" && msg.submit
      ? msg.fill.kind === "otp"
        ? "otp"
        : msg.fill.kind === "login" && msg.fill.username !== null
          ? "username"
          : "password"
      : null;

  async function pick(h: ReturnType<typeof setup>["h"]) {
    const open = (await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })) as { token: string };
    await h.handleInline(1, { type: "menu_pick", token: open.token, itemId: GH });
  }

  it("asks the content script to press only when Rust says autoSubmit", async () => {
    const off = setup();
    await pick(off.h);
    expect(off.sent.find((s) => s.msg.type === "bg_fill")?.msg).toMatchObject({ submit: false, totp: false });

    const on = setup(auto);
    await pick(on.h);
    expect(on.sent.find((s) => s.msg.type === "bg_fill")?.msg).toMatchObject({ submit: true, totp: true });
  });

  it("continues username → password → otp on the same origin, with Rust checks each step", async () => {
    const { h, requests, sent } = setup(auto, {}, pressWhenSubmit);
    await pick(h); // username-only page: content reports pressing "username"
    expect(await h.handleContent(frame({ url: "https://github.com/login/password" }), { type: "cs_ready" })).toEqual({
      saveToken: null,
      watch: "password",
    });
    await h.handleContent(frame({ url: "https://github.com/login/password" }), { type: "cs_run_step", kind: "password" });
    const pwFill = sent.filter((s) => s.msg.type === "bg_fill").at(-1)?.msg;
    expect(pwFill).toMatchObject({ token: null, submit: true, fill: { kind: "login", username: null, password: "pw" } });
    expect(requests.filter((r) => r.type === "fill_item").at(-1)).toMatchObject({ url: "https://github.com/login/password" });

    await h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(requests.at(-1)).toMatchObject({ type: "get_totp", itemId: GH });
    expect(sent.filter((s) => s.msg.type === "bg_fill").at(-1)?.msg).toMatchObject({ fill: { kind: "otp", code: "123456" }, submit: true });
    // The run is over: a repeat is ignored without asking the desktop.
    const before = requests.length;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(requests.length).toBe(before);
  });

  it("ignores steps from another origin, another tab, or with no run", async () => {
    const { h, requests } = setup(auto, {}, pressWhenSubmit);
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" }); // no pick yet
    await pick(h);
    const before = requests.length;
    await h.handleContent(frame({ url: "https://gist.github.com/", origin: "https://gist.github.com" }), { type: "cs_run_step", kind: "password" });
    await h.handleContent(frame({ tabId: 2 }), { type: "cs_run_step", kind: "password" });
    expect(requests.length).toBe(before);
  });

  it("drops a late step after expiry or after another pick in the tab", async () => {
    const { h, requests, advance } = setup(auto, {}, pressWhenSubmit);
    await pick(h);
    advance(2 * 60_000);
    const before = requests.length;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    expect(requests.length).toBe(before);

    // A second pick in the same tab replaces the run, even when that pick
    // itself does not start a new one (the content script did not press).
    let pressNull = false;
    const { h: h2, requests: requests2 } = setup(auto, {}, (msg) => (pressNull ? null : pressWhenSubmit(msg)));
    await pick(h2); // run starts at username
    pressNull = true;
    await pick(h2); // ends the first run; starts none
    const before2 = requests2.length;
    await h2.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    expect(requests2.length).toBe(before2);
  });

  it("ignores a continuation reply once its run is gone (lock or cs_run_stop mid-flight)", async () => {
    // Case 1: the vault locks (h.reset()) while the continuation's fill_item
    // request is outstanding. The reply must not fill the page, and must not
    // end whatever the tab is doing next.
    let hRef: ReturnType<typeof setup>["h"] | undefined;
    let fillCalls = 0;
    const lockMidFlight = (r: Request): unknown => {
      if (r.type === "fill_item") {
        fillCalls++;
        if (fillCalls === 2) hRef?.reset();
      }
      return auto(r);
    };
    const a = setup(lockMidFlight, {}, pressWhenSubmit);
    hRef = a.h;
    await pick(a.h); // fillCalls === 1
    const beforeA = a.sent.filter((s) => s.msg.type === "bg_fill").length;
    await a.h.handleContent(frame({ url: "https://github.com/login/password" }), { type: "cs_run_step", kind: "password" });
    expect(a.sent.filter((s) => s.msg.type === "bg_fill").length).toBe(beforeA);

    // Case 2: cs_run_stop arrives (from the run's own frame) while that same
    // request is outstanding.
    let hRef2: ReturnType<typeof setup>["h"] | undefined;
    let fillCalls2 = 0;
    const stopMidFlight = (r: Request): unknown => {
      if (r.type === "fill_item") {
        fillCalls2++;
        if (fillCalls2 === 2) void hRef2?.handleContent(frame(), { type: "cs_run_stop" });
      }
      return auto(r);
    };
    const b = setup(stopMidFlight, {}, pressWhenSubmit);
    hRef2 = b.h;
    await pick(b.h);
    const beforeB = b.sent.filter((s) => s.msg.type === "bg_fill").length;
    await b.h.handleContent(frame({ url: "https://github.com/login/password" }), { type: "cs_run_step", kind: "password" });
    expect(b.sent.filter((s) => s.msg.type === "bg_fill").length).toBe(beforeB);
  });

  it("stops on cs_run_stop, on lock, and when the content script did not press", async () => {
    const a = setup(auto, {}, pressWhenSubmit);
    await pick(a.h);
    await a.h.handleContent(frame(), { type: "cs_run_stop" });
    expect(await a.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });

    const b = setup(auto, {}, pressWhenSubmit);
    await pick(b.h);
    b.h.reset();
    expect(await b.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });
    expect(b.sent.some((s) => s.msg.type === "bg_run_end")).toBe(true);

    const c = setup(auto); // content never presses
    await pick(c.h);
    expect(await c.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });
  });

  it("does not end the frame's run after a pressed last step (its press is still pending there)", async () => {
    // Final OTP step, pressed by the content script.
    const a = setup(auto, {}, pressWhenSubmit);
    await pick(a.h); // username pressed
    await a.h.handleContent(frame(), { type: "cs_run_step", kind: "password" }); // password pressed
    await a.h.handleContent(frame(), { type: "cs_run_step", kind: "otp" }); // otp pressed: last step
    expect(a.sent.some((s) => s.msg.type === "bg_run_end")).toBe(false);
    const beforeA = a.requests.length;
    await a.h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(a.requests.length).toBe(beforeA);
    expect(await a.h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });

    // Final password step of a login without TOTP, pressed by the content script.
    const noTotp = (r: Request): unknown =>
      r.type === "find_matches" ? { type: "find_matches", matches: [{ ...ghMatch, hasTotp: false }] } : auto(r);
    const b = setup(noTotp, {}, pressWhenSubmit);
    await pick(b.h); // username pressed
    await b.h.handleContent(frame(), { type: "cs_run_step", kind: "password" }); // last step
    expect(b.sent.some((s) => s.msg.type === "bg_run_end")).toBe(false);
    const beforeB = b.requests.length;
    await b.h.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    await b.h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(b.requests.length).toBe(beforeB);
  });

  it("tells the frame the run ended when a continuation was not pressed", async () => {
    let pressNull = false;
    const { h, sent, requests } = setup(auto, {}, (msg) => (pressNull ? null : pressWhenSubmit(msg)));
    await pick(h); // username pressed
    pressNull = true;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    expect(sent.filter((s) => s.msg.type === "bg_run_end")).toEqual([{ to: { tabId: 1, frameId: 0 }, msg: { type: "bg_run_end" } }]);
    const before = requests.length;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "otp" });
    expect(requests.length).toBe(before);
  });

  it("ignores a step from another frame of the same tab and origin", async () => {
    const { h, requests } = setup(auto, {}, pressWhenSubmit);
    await pick(h); // run bound to frame 0
    const before = requests.length;
    await h.handleContent(frame({ frameId: 3 }), { type: "cs_run_step", kind: "password" });
    expect(requests.length).toBe(before);
  });

  it("fills without pressing and ends when autoSubmit turns off mid-run", async () => {
    let on = true;
    const answer = (r: Request): unknown => {
      const base = defaultAnswer(r) as Record<string, unknown>;
      return r.type === "fill_item" || r.type === "get_totp" ? { ...base, autoSubmit: on } : base;
    };
    const { h, sent } = setup(answer, {}, pressWhenSubmit);
    await pick(h);
    on = false;
    await h.handleContent(frame(), { type: "cs_run_step", kind: "password" });
    expect(sent.filter((s) => s.msg.type === "bg_fill").at(-1)?.msg).toMatchObject({ submit: false });
    expect(await h.handleContent(frame(), { type: "cs_ready" })).toEqual({ saveToken: null, watch: null });
  });
});

describe("suggestions preference", () => {
  const off = { suggestionsOn: async () => false };

  it("opens no menu of any kind when off, even from the field icon, and asks the desktop nothing", async () => {
    const { h, requests } = setup(defaultAnswer, off);
    for (const kind of ["login", "otp", "new_password"] as const) {
      expect(await h.handleContent(frame(), { type: "cs_open_menu", kind })).toEqual({ ok: false });
      expect(await h.handleContent(frame(), { type: "cs_open_menu", kind, explicit: true })).toEqual({ ok: false });
    }
    expect(requests).toEqual([]);
  });

  it("still offers to save a new login and to update a changed password when off", async () => {
    const { h, sent } = setup(defaultAnswer, off);
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "typed" });
    expect(await h.handleInline(1, { type: "save_state", token: T1 })).toMatchObject({ ok: true, value: { action: "add" } });
    expect(sent[0]?.msg).toEqual({ type: "bg_show_save", token: T1 });

    const upd = setup((r) => (r.type === "check_login" ? { type: "check_login", action: "update", itemId: GH } : defaultAnswer(r)), off);
    await upd.h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "changed" });
    expect(await upd.h.handleInline(1, { type: "save_state", token: T1 })).toMatchObject({ ok: true, value: { action: "update" } });
  });

  it("still reshows a pending save prompt on the next page when off", async () => {
    const { h } = setup(defaultAnswer, off);
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect(await h.handleContent(frame(), { type: "cs_ready" })).toMatchObject({ saveToken: T1 });
  });
});

describe("suggestions off: passkey autofill still shows", () => {
  const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";
  const row = { itemId: GH, credentialId: CRED, title: "GitHub", userName: "octo" };

  function offWithPasskeys(rows = [row]) {
    const requests: Request[] = [];
    const picks: unknown[] = [];
    const h = createInlineHandler({
      client: {
        request: (async (r: Request) => {
          requests.push(r);
          return defaultAnswer(r);
        }) as never,
      },
      sendToFrame: async () => undefined,
      now: () => 1,
      newToken: () => T1,
      suggestionsOn: async () => false,
      passkeys: {
        conditionalFor: () => rows,
        pickConditional: async (...args: unknown[]) => {
          picks.push(args);
          return { ok: true as const, value: null };
        },
      },
    });
    return { h, requests, picks };
  }

  it("opens a login menu with only the waiting passkeys, and no saved passwords", async () => {
    const { h, requests, picks } = offWithPasskeys();
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: true, token: T1, rows: 1 });
    const view = await h.handleInline(1, { type: "menu_state", token: T1 });
    expect(view).toMatchObject({ ok: true, value: { state: "ready", passkeys: [row], items: [], hint: null } });
    expect(requests).toEqual([]); // no password lookup
    expect((await h.handleInline(1, { type: "menu_pick", token: T1, itemId: GH })).ok).toBe(false);
    expect((await h.handleInline(1, { type: "menu_pick_passkey", token: T1, itemId: GH, credentialId: CRED })).ok).toBe(true);
    expect(picks).toHaveLength(1);
  });

  it("opens nothing without a waiting passkey request, from the icon, or in OTP and sign-up fields", async () => {
    const { h: none } = offWithPasskeys([]);
    expect(await none.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: false });
    const { h } = offWithPasskeys();
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login", explicit: true })).toEqual({ ok: false });
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "otp" })).toEqual({ ok: false });
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "new_password" })).toEqual({ ok: false });
  });
});

describe("identity menu", () => {
  const summary = { type: "find_identity", title: "Samuel Rocha", email: "me@x.com", roles: ["fullName", "postalCode", "cpf"] };
  const answerWith = (over: Record<string, unknown>) => (r: Request): unknown => {
    const v = over[r.type];
    if (v instanceof Error) throw v;
    return v !== undefined ? v : defaultAnswer(r);
  };
  const idFrame = (url: string) => frame({ url, origin: new URL(url).origin });
  const TAB = 1;

  it("offers the identity with a count and the documents the form asks for", async () => {
    const { h, requests } = setup(answerWith({ find_identity: summary }));
    const open = await h.handleContent(idFrame("https://shop.com/checkout"), {
      type: "cs_open_menu",
      kind: "identity",
      roles: ["fullName", "cpf", "city"],
    });
    expect(open).toMatchObject({ ok: true, rows: 1 });
    const token = (open as { token: string }).token;
    const view = await h.handleInline(TAB, { type: "menu_state", token });
    expect(view).toMatchObject({
      ok: true,
      value: { state: "ready", kind: "identity", identity: { title: "Samuel Rocha", fills: 2, documents: ["cpf"], documentsAllowed: true, empty: false } },
    });
    expect(requests.map((r) => r.type)).toEqual(["find_identity"]);
  });

  it("fills without documents, then with them after confirmation", async () => {
    const { h, requests, sent } = setup(
      answerWith({ find_identity: summary, fill_identity: { type: "fill_identity", values: [{ role: "fullName", value: "Samuel Rocha" }] } }),
    );
    const open = (await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })) as { token: string };
    await h.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: false });
    expect(requests.at(-1)).toMatchObject({ type: "fill_identity", roles: ["fullName"], documents: false });
    expect(sent.at(-1)?.msg).toMatchObject({ type: "bg_fill", fill: { kind: "identity" }, submit: false, totp: false });

    const again = (await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })) as { token: string };
    await h.handleInline(TAB, { type: "menu_pick_identity", token: again.token, documents: true });
    expect(requests.at(-1)).toMatchObject({ type: "fill_identity", roles: ["fullName", "cpf"], documents: true });
  });

  it("never asks for documents on an http page", async () => {
    const { h, requests } = setup(answerWith({ find_identity: summary, fill_identity: { type: "fill_identity", values: [] } }));
    const open = (await h.handleContent(idFrame("http://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })) as { token: string };
    const view = await h.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { identity: { documentsAllowed: false } } });
    await h.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: true });
    expect(requests.at(-1)).toMatchObject({ roles: ["fullName"] });
  });

  it("shows the missing identity row when there is none, and never sends open_identity for it", async () => {
    const { h, requests } = setup(answerWith({ find_identity: new BridgeError("not_found", "x"), open_identity: { type: "open_identity" } }));
    const open = (await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "city"] })) as { token: string };
    const view = await h.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { identity: { empty: true, missing: true } } });
    expect(await h.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: false })).toMatchObject({ ok: false });
    expect(await h.handleInline(TAB, { type: "menu_open_identity", token: open.token })).toEqual({
      ok: false,
      message: "Open the HavenKeys app to add your details.",
    });
    expect(requests.some((r) => r.type === "open_identity")).toBe(false);
  });

  it("opens an identity that exists but has no values", async () => {
    const { h, requests } = setup(answerWith({ find_identity: { ...summary, roles: [] }, open_identity: { type: "open_identity" } }));
    const open = (await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName", "city"] })) as { token: string };
    const view = await h.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { identity: { empty: true, missing: false } } });
    expect(await h.handleInline(TAB, { type: "menu_open_identity", token: open.token })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)?.type).toBe("open_identity");
  });

  it("adds the identity row under a sign-up form's logins", async () => {
    const { h } = setup(answerWith({ find_matches: { type: "find_matches", matches: [] }, find_identity: summary }));
    const open = (await h.handleContent(idFrame("https://shop.com/signup"), { type: "cs_open_menu", kind: "login", roles: ["email", "fullName"] })) as { ok: boolean; token: string };
    expect(open.ok).toBe(true);
    const view = await h.handleInline(TAB, { type: "menu_state", token: open.token });
    expect(view).toMatchObject({ value: { kind: "login", items: [], identity: { fills: 1 } } });
  });

  it("stays out of the page when the identity has nothing for the form", async () => {
    const { h } = setup(answerWith({ find_identity: { ...summary, roles: ["cpf"] } }));
    const open = await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["city", "state"] });
    expect(open).toEqual({ ok: false });
  });

  it("offers no identity row when browser integration is off", async () => {
    const { h } = setup(answerWith({ find_identity: new BridgeError("integration_disabled", "x") }));
    const open = await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName"] });
    expect(open).toEqual({ ok: false });
  });
  it("shows a locked row when the vault is locked, and refuses to fill from it", async () => {
    const { h, requests } = setup(answerWith({ find_identity: new BridgeError("locked", "x") }));
    const open = (await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName"] })) as { ok: boolean; token: string };
    expect(open.ok).toBe(true);
    expect(await h.handleInline(TAB, { type: "menu_state", token: open.token })).toEqual({ ok: true, value: { state: "locked" } });
    expect(await h.handleInline(TAB, { type: "menu_pick_identity", token: open.token, documents: false })).toMatchObject({ ok: false });
    expect(requests.map((r) => r.type)).toEqual(["find_identity"]);
  });

  it("stays out of the page when the desktop denies the request", async () => {
    const { h } = setup(answerWith({ find_identity: new BridgeError("denied", "x") }));
    expect(await h.handleContent(idFrame("https://shop.com/"), { type: "cs_open_menu", kind: "identity", roles: ["fullName"] })).toEqual({ ok: false });
  });

  it("adds no identity row under a sign-up form when there is no identity", async () => {
    const { h } = setup(answerWith({ find_matches: { type: "find_matches", matches: [] }, find_identity: new BridgeError("not_found", "x") }));
    const open = await h.handleContent(idFrame("https://shop.com/signup"), { type: "cs_open_menu", kind: "login", roles: ["email", "fullName"] });
    expect(open).toEqual({ ok: false });
  });
});
