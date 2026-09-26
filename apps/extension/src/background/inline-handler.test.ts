import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import {
  parseBackgroundMessage,
  parseContentRequest,
  parseFillReply,
  parseInlineRequest,
  type BackgroundToContent,
} from "../messaging/inline";
import { createInlineHandler, MENU_TTL_MS, SAVE_TTL_MS, type FrameRef, type InlineDeps } from "./inline-handler";

const GH = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const OTHER = "11111111-2222-4333-8444-555555555555";
const T1 = "0".repeat(31) + "1";
const GH_SITE = { name: "GitHub", domains: ["github.com"], passwordless: true, mfa: true, help: "https://docs.github.com/passkeys" };

const ghMatch = { id: GH, title: "GitHub", username: "octo", hasTotp: true, strength: "same_host" as const };

function frame(over: Partial<FrameRef> = {}): FrameRef {
  return { tabId: 1, frameId: 0, url: "https://github.com/login", origin: "https://github.com", ...over };
}

function setup(answer: (r: Request) => unknown = defaultAnswer, extra: Partial<InlineDeps> = {}) {
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
    for (const bad of [
      { type: "cs_open_menu", kind: "login", url: "https://github.com" },
      { type: "cs_open_menu", kind: "everything" },
      { type: "cs_open_menu", kind: "login", explicit: false },
      { type: "cs_open_menu", kind: "login", explicit: "yes" },
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
    expect(parseInlineRequest({ type: "menu_pick_passkey", token: T1, itemId: GH, credentialId: "AQEBAQEBAQEBAQEBAQEBAQ" })).not.toBeNull();
    expect(parseInlineRequest({ type: "menu_open_help", token: T1 })).toEqual({ type: "menu_open_help", token: T1 });
    for (const bad of [
      { type: "menu_pick", token: T1, itemId: "x" },
      { type: "menu_pick", token: "nope", itemId: GH },
      { type: "menu_state", token: T1, url: "https://evil.com" },
      { type: "save_confirm", token: T1, password: "x" },
      { type: "menu_pick_passkey", token: T1, itemId: GH, credentialId: "AQ" },
      { type: "menu_open_help", token: T1, url: "https://evil.com" },
    ]) {
      expect(parseInlineRequest(bad), JSON.stringify(bad)).toBeNull();
    }
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
        items: [{ id: GH, title: "GitHub", username: "octo" }],
        passkeys: [],
        hint: null,
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
