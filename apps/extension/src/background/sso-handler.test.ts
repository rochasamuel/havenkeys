import { describe, expect, it, vi } from "vitest";
import { BridgeError } from "../messaging/native";
import type { SsoContentRequest } from "../messaging/sso";
import { createSsoHandler, FRAME_TTL_MS } from "./sso-handler";
import type { FrameRef } from "./inline-handler";

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const top: FrameRef = { tabId: 1, frameId: 0, url: "https://typeform.com/login", origin: "https://typeform.com" };
const googleFrame = (tabId = 1): FrameRef => ({ tabId, frameId: 0, url: "https://accounts.google.com/o/oauth2/v2", origin: "https://accounts.google.com" });
const match = { id: ID, title: "Typeform", username: "me@gmail.com", hasTotp: false, strength: "same_site", provider: "google" } as const;

const START_SSO = { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true };
const tokenOf = (s: { msg: { type: string } } | undefined) => (s!.msg as unknown as { token: string }).token;

function setup(answers: Record<string, unknown>, opts: { suggestions?: boolean } = {}) {
  let clock = 1_000;
  const requests: { type: string }[] = [];
  const sent: { frame: unknown; msg: { type: string } }[] = [];
  const client = {
    request: vi.fn(async (r: { type: string }) => {
      requests.push(r);
      const a = answers[r.type];
      if (a instanceof Error) throw a;
      return typeof a === "function" ? (a as () => unknown)() : a;
    }),
  };
  let n = 0;
  const h = createSsoHandler({
    client: client as never,
    sendToFrame: async (frame, msg) => {
      sent.push({ frame, msg });
      return msg.type === "bg_sso_press" ? (answers.press ?? { pressed: true }) : undefined;
    },
    now: () => clock,
    newToken: () => (n++).toString(16).padStart(32, "0"),
    suggestionsOn: async () => opts.suggestions ?? true,
  });
  return { h, requests, sent, advance: (ms: number) => (clock += ms) };
}

/** Click Google on typeform.com, visit Google, come back: the save question opens. */
async function returnFromGoogle(h: ReturnType<typeof setup>["h"]) {
  await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
  h.ready(googleFrame(), { tabId: 1 });
  h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 });
}

describe("offer", () => {
  it("offers only this site's items for the providers on the page", async () => {
    const other = { ...match, id: "8c9e6679-7425-40de-944b-e07fc1f90ae7", provider: "github" };
    const plain = { ...match, id: "9c9e6679-7425-40de-944b-e07fc1f90ae7", provider: null };
    const { h } = setup({ find_matches: { matches: [match, other, plain] } });
    const r = (await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })) as { ok: boolean; token: string };
    expect(r.ok).toBe(true);
    const view = await h.handleFrame(1, { type: "sso_state", token: r.token });
    expect(view).toEqual({ ok: true, value: { mode: "offer", site: "typeform.com", rows: [{ id: ID, provider: "google", title: "Typeform", account: "me@gmail.com" }] } });
  });
  it("stays out when suggestions are off, in iframes, or with nothing to offer", async () => {
    expect(await setup({ find_matches: { matches: [match] } }, { suggestions: false }).h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
    expect(await setup({ find_matches: { matches: [match] } }).h.handleContent({ ...top, frameId: 3, topUrl: top.url }, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
    expect(await setup({ find_matches: { matches: [] } }).h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
    expect(await setup({ find_matches: new BridgeError("locked", "x") }).h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toEqual({ ok: false });
  });
  it("a pick starts a run: start_sso, then press, then choose on the provider", async () => {
    const { h, requests, sent } = setup({
      find_matches: { matches: [match] },
      start_sso: { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    const r = (await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })) as { token: string };
    expect(await h.handleFrame(1, { type: "sso_pick", token: r.token, itemId: ID })).toEqual({ ok: true, value: null });
    expect(requests.map((q) => q.type)).toEqual(["find_matches", "start_sso"]);
    expect(sent.map((s) => s.msg.type)).toContain("bg_sso_press");
    // The press names the page's own origin, so a stale press after the frame navigated is refused there.
    expect(sent.find((s) => s.msg.type === "bg_sso_press")?.msg).toMatchObject({ origin: top.origin });
    // Not on another origin (an iframe here: a top-frame load elsewhere would end the run),
    // and not in an unrelated tab.
    expect(h.ready({ tabId: 1, frameId: 4, url: "https://evil.com/", origin: "https://evil.com", topUrl: top.url }, { tabId: 1 })).toBeNull();
    expect(h.ready(googleFrame(2), { tabId: 2 })).toBeNull();
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull(); // once
  });
  it("a popup the run's tab opened may choose", async () => {
    const { h } = setup({
      start_sso: { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    expect(await h.start(top, ID)).toEqual({ ok: true, value: null });
    expect(h.ready(googleFrame(9), { tabId: 9, openerTabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
  });
  it("a top-frame load on an unrelated origin ends the run", async () => {
    const { h } = setup({
      start_sso: { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    await h.start(top, ID);
    expect(h.ready({ tabId: 1, frameId: 0, url: "https://evil.com/", origin: "https://evil.com" }, { tabId: 1 })).toBeNull();
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
  it("rejects picks the offer did not make", async () => {
    const { h } = setup({ find_matches: { matches: [match] } });
    const r = (await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })) as { token: string };
    const res = await h.handleFrame(1, { type: "sso_pick", token: r.token, itemId: "8c9e6679-7425-40de-944b-e07fc1f90ae7" });
    expect(res.ok).toBe(false);
    expect((await h.handleFrame(2, { type: "sso_pick", token: r.token, itemId: ID })).ok).toBe(false); // another tab
  });
  it("a denied start_sso presses nothing", async () => {
    const { h, sent } = setup({ start_sso: new BridgeError("denied", "This item is not saved for this website.") });
    expect(await h.start(top, ID)).toEqual({ ok: false, message: "This item is not saved for this website." });
    expect(sent).toEqual([]);
  });
  it("shows a notice when the button is gone", async () => {
    const { h, sent } = setup({
      press: { pressed: false },
      start_sso: { type: "start_sso", provider: "google", account: null, providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    const r = await h.start(top, ID);
    expect(r).toEqual({ ok: false, message: "Couldn’t find the “Sign in with Google” button on this page." });
    const show = sent.find((s) => s.msg.type === "bg_sso_show")!;
    expect(show.frame).toEqual({ tabId: 1, frameId: 0 });
    const token = (show.msg as unknown as { token: string }).token;
    expect(await h.handleFrame(1, { type: "sso_state", token })).toEqual({ ok: true, value: { mode: "notice", site: "typeform.com", provider: "google" } });
  });
});

describe("save", () => {
  it("asks on return, with the learned account, and saves for the clicked site", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null }, save_sso: { type: "save_sso", itemId: ID } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_account", account: "me@gmail.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://admin.typeform.com/", origin: "https://admin.typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    const token = (sent.find((s) => s.msg.type === "bg_sso_show")!.msg as unknown as { token: string }).token;
    const view = await h.handleFrame(1, { type: "sso_state", token });
    expect(view).toEqual({ ok: true, value: { mode: "save", site: "typeform.com", provider: "google", account: "me@gmail.com", title: "typeform.com", action: "add" } });
    expect(await h.handleFrame(1, { type: "sso_save", token, account: " me@gmail.com ", title: "Typeform" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_sso", url: "https://typeform.com/login", provider: "google", account: "me@gmail.com", itemId: null, title: "Typeform" });
  });
  it("learns the account from a username step on the provider, and asks when its popup closes", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "update", itemId: ID }, save_sso: { type: "save_sso", itemId: ID } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    const popup = { tabId: 5, openerTabId: 1 };
    h.ready(googleFrame(5), popup);
    h.noteUsername(googleFrame(5), popup, "  Me@Gmail.com ");
    h.tabRemoved(5);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(requests.at(-1)).toEqual({ type: "check_sso", url: "https://typeform.com/login", provider: "google", account: "me@gmail.com" });
    const token = (sent.find((s) => s.msg.type === "bg_sso_show")!.msg as unknown as { token: string }).token;
    expect(await h.handleFrame(1, { type: "sso_state", token })).toEqual({
      ok: true,
      value: { mode: "save", site: "typeform.com", provider: "google", account: "me@gmail.com", title: null, action: "update" },
    });
    // An update never renames; an empty account means none.
    expect(await h.handleFrame(1, { type: "sso_save", token, account: "  ", title: "Ignored" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_sso", url: "https://typeform.com/login", provider: "google", account: null, itemId: ID });
    expect((await h.handleFrame(1, { type: "sso_state", token })).ok).toBe(false); // closed after saving
  });
  it("ignores account text from iframes and other origins", async () => {
    const { h, requests } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    await h.handleContent({ ...googleFrame(), frameId: 2, topUrl: googleFrame().url }, { tabId: 1 }, { type: "cs_sso_account", account: "iframe@x.com" });
    await h.handleContent({ ...top, frameId: 0 }, { tabId: 1 }, { type: "cs_sso_account", account: "site@x.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(requests.some((r) => r.type === "check_sso")).toBe(true));
    expect(requests.at(-1)).toMatchObject({ type: "check_sso", account: null });
  });
  it("says nothing when unchanged", async () => {
    const { h, sent } = setup({ check_sso: { type: "check_sso", action: "unchanged", itemId: null } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 });
    await new Promise((r) => setTimeout(r, 0));
    expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(false);
  });
  it("reset forgets everything", async () => {
    const { h, requests } = setup({});
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.reset();
    h.ready(googleFrame(), { tabId: 1 });
    expect(h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 })).toBeNull();
    await new Promise((r) => setTimeout(r, 0));
    expect(requests).toEqual([]); // no check_sso: nothing was pending
  });
});

describe("sessions", () => {
  const buttons: SsoContentRequest = { type: "cs_sso_buttons", providers: ["google"] };

  it("dismiss closes the balloon", async () => {
    const { h, sent } = setup({ find_matches: { matches: [match] } });
    const r = (await h.handleContent(top, { tabId: 1 }, buttons)) as { token: string };
    expect(await h.handleFrame(1, { type: "sso_dismiss", token: r.token })).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toEqual({ frame: { tabId: 1, frameId: 0 }, msg: { type: "bg_sso_close", token: r.token } });
    expect((await h.handleFrame(1, { type: "sso_state", token: r.token })).ok).toBe(false);
  });
  it("an offer expires", async () => {
    const { h, advance } = setup({ find_matches: { matches: [match] } });
    const r = (await h.handleContent(top, { tabId: 1 }, buttons)) as { token: string };
    advance(FRAME_TTL_MS);
    expect(await h.handleFrame(1, { type: "sso_state", token: r.token })).toEqual({ ok: false, message: "This prompt has expired." });
  });
  it("a live save question outranks an offer; an expired one does not block it", async () => {
    const { h, sent, advance } = setup({ find_matches: { matches: [match] }, check_sso: { type: "check_sso", action: "add", itemId: null } });
    await returnFromGoogle(h);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(await h.handleContent(top, { tabId: 1 }, buttons)).toEqual({ ok: false });
    advance(FRAME_TTL_MS + 1);
    expect(await h.handleContent(top, { tabId: 1 }, buttons)).toMatchObject({ ok: true });
  });
  it("an offer does not replace a save question opened while find_matches was in flight", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const { h, sent } = setup({
      find_matches: async () => {
        await gate;
        return { matches: [match] };
      },
      check_sso: { type: "check_sso", action: "add", itemId: null },
    });
    const offered = h.handleContent(top, { tabId: 1 }, buttons);
    await returnFromGoogle(h);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    release();
    expect(await offered).toEqual({ ok: false });
    const token = tokenOf(sent.find((s) => s.msg.type === "bg_sso_show"));
    expect(await h.handleFrame(1, { type: "sso_state", token })).toMatchObject({ ok: true, value: { mode: "save" } });
  });
  it("a lock while check_sso is pending shows nothing", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const { h, requests, sent } = setup({
      check_sso: async () => {
        await gate;
        return { type: "check_sso", action: "add", itemId: null };
      },
    });
    await returnFromGoogle(h);
    await vi.waitFor(() => expect(requests.some((r) => r.type === "check_sso")).toBe(true));
    h.reset();
    release();
    await new Promise((r) => setTimeout(r, 0));
    expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(false);
  });
});

describe("stop", () => {
  it("input in any frame of the run's tab ends the run", async () => {
    const { h } = setup({ start_sso: START_SSO });
    await h.start(top, ID);
    await h.handleContent({ ...top, frameId: 7, topUrl: top.url }, { tabId: 1 }, { type: "cs_sso_stop" });
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
  it("input in a popup the run's tab opened ends the run", async () => {
    const { h } = setup({ start_sso: START_SSO });
    await h.start(top, ID);
    await h.handleContent(googleFrame(9), { tabId: 9, openerTabId: 1 }, { type: "cs_sso_stop" });
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
  it("input in an unrelated tab does not", async () => {
    const { h } = setup({ start_sso: START_SSO });
    await h.start(top, ID);
    await h.handleContent(googleFrame(9), { tabId: 9 }, { type: "cs_sso_stop" });
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
  });
});
