import { describe, expect, it, vi } from "vitest";
import { BridgeError } from "../messaging/native";
import type { SsoContentRequest } from "../messaging/sso";
import { createSsoHandler, FRAME_TTL_MS } from "./sso-handler";
import type { FrameRef } from "./inline-handler";

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const top: FrameRef = { tabId: 1, frameId: 0, url: "https://typeform.com/login", origin: "https://typeform.com" };
const googleFrame = (tabId = 1): FrameRef => ({ tabId, frameId: 0, url: "https://accounts.google.com/o/oauth2/v2", origin: "https://accounts.google.com" });
const match = { id: ID, title: "Typeform", username: "me@gmail.com", hasTotp: false, strength: "same_site", provider: "google", tags: [] } as const;

const START_SSO = { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true };
const tokenOf = (s: { msg: { type: string } } | undefined) => (s!.msg as unknown as { token: string }).token;

function setup(answers: Record<string, unknown>, opts: { suggestions?: boolean } = {}) {
  let clock = 1_000;
  const requests: { type: string }[] = [];
  const sent: { frame: unknown; msg: { type: string } }[] = [];
  const fills: { frame: unknown; payload: unknown; auto: unknown }[] = [];
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
    pickFill: async (frame, payload, auto) => {
      fills.push({ frame, payload, auto });
      return 1;
    },
  });
  return { h, requests, sent, fills, advance: (ms: number) => (clock += ms) };
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
    // The next provider document (the password page after the chooser click) waits for the login form.
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "login" });
  });
  it("a popup the run's tab opened may choose", async () => {
    const { h } = setup({
      start_sso: { type: "start_sso", provider: "google", account: "me@gmail.com", providerOrigins: ["https://accounts.google.com"], autoChoose: true },
    });
    expect(await h.start(top, ID)).toEqual({ ok: true, value: null });
    expect(h.ready(googleFrame(9), { tabId: 9, openerTabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
  });
  it("Firefox: Google's popup without an opener chooses once its URL names the site", async () => {
    const { h } = setup({ start_sso: START_SSO });
    expect(await h.start(top, ID)).toEqual({ ok: true, value: null });
    expect(h.ready(googleFrame(9), { tabId: 9 })).toBeNull(); // not tied yet
    h.tabUrl({ tabId: 9 }, `https://accounts.google.com/gsi/select?client_id=x&ux_mode=popup&origin=${encodeURIComponent(top.origin)}`);
    expect(h.ready(googleFrame(9), { tabId: 9 })).toEqual({ kind: "choose", account: "me@gmail.com" });
  });
  it("a provider-origin iframe does not choose or spend the run; the popup's top frame then does", async () => {
    const { h } = setup({ start_sso: START_SSO });
    await h.start(top, ID);
    // Google's own sign-in widget, embedded in the site's page during the run.
    const gsi: FrameRef = { tabId: 1, frameId: 5, url: "https://accounts.google.com/gsi/iframe", origin: "https://accounts.google.com", topUrl: top.url };
    expect(h.ready(gsi, { tabId: 1 })).toBeNull();
    expect(h.ready({ ...gsi, tabId: 9, topUrl: "https://accounts.google.com/o/oauth2/v2" }, { tabId: 9, openerTabId: 1 })).toBeNull();
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
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] }, save_sso: { type: "save_sso", itemId: ID } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_account", account: "me@gmail.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://admin.typeform.com/", origin: "https://admin.typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    const token = (sent.find((s) => s.msg.type === "bg_sso_show")!.msg as unknown as { token: string }).token;
    const view = await h.handleFrame(1, { type: "sso_state", token });
    expect(view).toEqual({ ok: true, value: { mode: "save", site: "typeform.com", provider: "google", account: "me@gmail.com", accounts: [], title: "typeform.com", action: "add" } });
    expect(await h.handleFrame(1, { type: "sso_save", token, account: " me@gmail.com ", title: "Typeform" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_sso", url: "https://typeform.com/login", provider: "google", account: "me@gmail.com", itemId: null, title: "Typeform" });
  });
  it("learns the account from a username step on the provider, and asks when its popup closes", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "update", itemId: ID, accounts: [] }, save_sso: { type: "save_sso", itemId: ID } });
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
      value: { mode: "save", site: "typeform.com", provider: "google", account: "me@gmail.com", accounts: [], title: null, action: "update" },
    });
    // An update never renames; an empty account means none.
    expect(await h.handleFrame(1, { type: "sso_save", token, account: "  ", title: "Ignored" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_sso", url: "https://typeform.com/login", provider: "google", account: null, itemId: ID });
    expect((await h.handleFrame(1, { type: "sso_state", token })).ok).toBe(false); // closed after saving
  });
  it("asks when the site's login popup closes by itself, though no Google page ever loaded in it", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.tabCreated({ tabId: 5, openerTabId: 1 });
    h.tabRemoved(5);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(requests.at(-1)).toEqual({ type: "check_sso", url: "https://typeform.com/login", provider: "google", account: null });
  });
  it("a click in Google's own button iframe saves for the page that embeds it", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    const site = "https://financeiro.hostgator.com.br/";
    const gsi: FrameRef = { tabId: 1, frameId: 7, url: "https://accounts.google.com/gsi/button", origin: "https://accounts.google.com", topUrl: site };
    await h.handleContent(gsi, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    // The personalized button names the account: a suggestion.
    await h.handleContent(gsi, { tabId: 1 }, { type: "cs_sso_account", account: "me@gmail.com" });
    // Chrome gives Google's popup no opener tab: only the site's next page tells us it is back.
    h.tabCreated({ tabId: 5 });
    h.tabRemoved(5);
    h.ready({ tabId: 1, frameId: 0, url: "https://financeiro.hostgator.com.br/index.php", origin: "https://financeiro.hostgator.com.br" }, { tabId: 1 });
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(requests.at(-1)).toEqual({ type: "check_sso", url: site, provider: "google", account: "me@gmail.com" });
  });
  it("account text from a provider iframe that is not the clicked button frame is ignored", async () => {
    const { h, requests } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    const gsi: FrameRef = { tabId: 1, frameId: 7, url: "https://accounts.google.com/gsi/button", origin: "https://accounts.google.com", topUrl: top.url };
    await h.handleContent(gsi, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    await h.handleContent({ ...gsi, url: "https://accounts.google.com/gsi/iframe" }, { tabId: 1 }, { type: "cs_sso_account", account: "other@gmail.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/home", origin: "https://typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(requests.some((r) => r.type === "check_sso")).toBe(true));
    expect(requests.at(-1)).toMatchObject({ type: "check_sso", account: null });
  });
  it("a click on the overlay over Google's button frame asks on the site's next page (Firefox: popup without opener)", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google", embedded: true });
    h.tabCreated({ tabId: 5 });
    h.tabRemoved(5);
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/index.php", origin: "https://typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(requests.at(-1)).toEqual({ type: "check_sso", url: "https://typeform.com/login", provider: "google", account: null });
  });
  it("takes the account picked in Google's popup though the popup has no opener (Firefox), by the origin it names", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google", embedded: true });
    const popup = { tabId: 5 };
    h.tabUrl(popup, "https://accounts.google.com/o/oauth2/v2/auth?client_id=x&origin=https%3A%2F%2Ftypeform.com&display=popup");
    h.ready(googleFrame(5), popup);
    await h.handleContent(googleFrame(5), popup, { type: "cs_sso_account", account: "me@gmail.com" });
    h.tabRemoved(5);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(requests.at(-1)).toEqual({ type: "check_sso", url: "https://typeform.com/login", provider: "google", account: "me@gmail.com" });
  });
  it("does not tie a provider tab naming another site's origin to the pending click", async () => {
    const { h, requests } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google", embedded: true });
    const other = { tabId: 6 };
    h.tabUrl(other, "https://accounts.google.com/o/oauth2/v2/auth?origin=https%3A%2F%2Fevil.example");
    h.tabUrl({ tabId: 7 }, "https://evil.example/?origin=https%3A%2F%2Ftypeform.com");
    await h.handleContent(googleFrame(6), other, { type: "cs_sso_account", account: "other@gmail.com" });
    await h.handleContent({ ...googleFrame(7), url: "https://evil.example/", origin: "https://evil.example" }, { tabId: 7 }, { type: "cs_sso_account", account: "x@gmail.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/home", origin: "https://typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(requests.some((r) => r.type === "check_sso")).toBe(true));
    expect(requests.at(-1)).toMatchObject({ type: "check_sso", account: null });
  });
  it("a plain click still waits for the provider's page before asking", async () => {
    const { h, requests } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/auth/google", origin: "https://typeform.com" }, { tabId: 1 });
    await new Promise((r) => setTimeout(r, 0));
    expect(requests).toEqual([]);
  });
  it("ignores a click reported by any other provider iframe, or for another provider", async () => {
    const { h, requests } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    const gsi: FrameRef = { tabId: 1, frameId: 7, url: "https://accounts.google.com/gsi/button", origin: "https://accounts.google.com", topUrl: top.url };
    await h.handleContent({ ...gsi, url: "https://accounts.google.com/gsi/iframe" }, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    await h.handleContent(gsi, { tabId: 1 }, { type: "cs_sso_click", provider: "github" });
    h.tabCreated({ tabId: 5, openerTabId: 1 });
    h.tabRemoved(5);
    await new Promise((r) => setTimeout(r, 0));
    expect(requests).toEqual([]);
  });
  it("offers the vault's accounts, preselecting the login_hint the site sent", async () => {
    const { h, requests, sent } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: ["me@gmail.com", "work@x.com"] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.tabCreated({ tabId: 5, openerTabId: 1 });
    h.tabUrl({ tabId: 5, openerTabId: 1 }, "https://auth.typeform.com/authorize?login_hint=work%40x.com");
    h.tabRemoved(5);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    expect(requests.at(-1)).toEqual({ type: "check_sso", url: "https://typeform.com/login", provider: "google", account: "work@x.com" });
    const view = await h.handleFrame(1, { type: "sso_state", token: tokenOf(sent.find((s) => s.msg.type === "bg_sso_show")) });
    expect(view).toEqual({
      ok: true,
      value: { mode: "save", site: "typeform.com", provider: "google", account: "work@x.com", accounts: ["me@gmail.com", "work@x.com"], title: "typeform.com", action: "add" },
    });
  });
  it("shows the save question again on the tab's next page (popup closes, then the site moves to its dashboard)", async () => {
    const { h, sent, advance } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    const popup = { tabId: 5, openerTabId: 1 };
    h.ready(googleFrame(5), popup);
    h.tabRemoved(5);
    await vi.waitFor(() => expect(sent.filter((s) => s.msg.type === "bg_sso_show")).toHaveLength(1));
    const token = tokenOf(sent.find((s) => s.msg.type === "bg_sso_show"));
    const dashboard: FrameRef = { tabId: 1, frameId: 0, url: "https://admin.typeform.com/", origin: "https://admin.typeform.com" };
    h.ready({ ...dashboard, frameId: 3 }, { tabId: 1 }); // an iframe does not show it
    h.ready(dashboard, { tabId: 1 });
    const shows = sent.filter((s) => s.msg.type === "bg_sso_show");
    expect(shows).toHaveLength(2);
    expect(shows[1]).toEqual({ frame: { tabId: 1, frameId: 0 }, msg: { type: "bg_sso_show", token } });
    // Answered or expired: not shown again.
    advance(FRAME_TTL_MS + 1);
    h.ready(dashboard, { tabId: 1 });
    expect(sent.filter((s) => s.msg.type === "bg_sso_show")).toHaveLength(2);
  });
  it("does not show an offer again on the next page", async () => {
    const { h, sent } = setup({ find_matches: { type: "find_matches", matches: [match], entitlement: "full" } });
    expect(await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_buttons", providers: ["google"] })).toMatchObject({ ok: true });
    h.ready(top, { tabId: 1 });
    expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(false);
  });
  it("ignores account text from iframes and other origins", async () => {
    const { h, requests } = setup({ check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
    await h.handleContent(top, { tabId: 1 }, { type: "cs_sso_click", provider: "google" });
    h.ready(googleFrame(), { tabId: 1 });
    await h.handleContent({ ...googleFrame(), frameId: 2, topUrl: googleFrame().url }, { tabId: 1 }, { type: "cs_sso_account", account: "iframe@x.com" });
    await h.handleContent({ ...top, frameId: 0 }, { tabId: 1 }, { type: "cs_sso_account", account: "site@x.com" });
    h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 });
    await vi.waitFor(() => expect(requests.some((r) => r.type === "check_sso")).toBe(true));
    expect(requests.at(-1)).toMatchObject({ type: "check_sso", account: null });
  });
  it("says nothing when unchanged", async () => {
    const { h, sent } = setup({ check_sso: { type: "check_sso", action: "unchanged", itemId: null, accounts: [] } });
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
    const { h, sent, advance } = setup({ find_matches: { matches: [match] }, check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] } });
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
      check_sso: { type: "check_sso", action: "add", itemId: null, accounts: [] },
    });
    const offered = h.handleContent(top, { tabId: 1 }, buttons);
    await returnFromGoogle(h);
    await vi.waitFor(() => expect(sent.some((s) => s.msg.type === "bg_sso_show")).toBe(true));
    release();
    expect(await offered).toEqual({ ok: false });
    const token = tokenOf(sent.find((s) => s.msg.type === "bg_sso_show"));
    expect(await h.handleFrame(1, { type: "sso_state", token })).toMatchObject({ ok: true, value: { mode: "save" } });
  });
  it("a lock while find_matches is pending offers nothing", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const { h, requests, sent } = setup({
      find_matches: async () => {
        await gate;
        return { matches: [match] };
      },
    });
    const offered = h.handleContent(top, { tabId: 1 }, buttons);
    await vi.waitFor(() => expect(requests.some((r) => r.type === "find_matches")).toBe(true));
    h.reset();
    release();
    expect(await offered).toEqual({ ok: false });
    expect(sent).toEqual([]);
  });
  it("a lock while check_sso is pending shows nothing", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const { h, requests, sent } = setup({
      check_sso: async () => {
        await gate;
        return { type: "check_sso", action: "add", itemId: null, accounts: [] };
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

describe("completing the provider login", () => {
  const google = (over: object = {}) => ({ id: "9c9e6679-7425-40de-944b-e07fc1f90ae7", title: "Google", username: "Me@Gmail.com", hasTotp: true, strength: "same_host", provider: null, tags: [], ...over });

  async function toLogin(answers: Record<string, unknown>) {
    const env = setup({ start_sso: START_SSO, ...answers });
    await env.h.start(top, ID);
    expect(env.h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
    return env;
  }

  it("fills the one provider login with that account and presses", async () => {
    const { h, fills, requests } = await toLogin({
      find_matches: { matches: [google()] },
      fill_item: { type: "fill_item", username: "Me@Gmail.com", password: "pw", autoSubmit: true },
    });
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "login" }); // the password page after a chooser click
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(requests.map((r) => r.type)).toContain("fill_item");
    expect(requests.find((r) => r.type === "fill_item")).toMatchObject({ itemId: google().id, url: googleFrame().url });
    expect(fills).toEqual([{ frame: googleFrame(), payload: { kind: "login", username: "Me@Gmail.com", password: "pw" }, auto: { itemId: google().id, hasTotp: true } }]);
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(1); // once
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull(); // the run is over
  });
  it("a popup the run's tab opened completes the login", async () => {
    const { h, fills } = await toLogin({
      find_matches: { matches: [google()] },
      fill_item: { type: "fill_item", username: "Me@Gmail.com", password: "pw", autoSubmit: true },
    });
    await h.handleContent(googleFrame(9), { tabId: 9, openerTabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(1);
  });
  it("two_provider_logins_fill_nothing", async () => {
    const { h, fills, requests } = await toLogin({ find_matches: { matches: [google(), google({ id: "8c9e6679-7425-40de-944b-e07fc1f90ae7" })] } });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(requests.some((r) => r.type === "fill_item")).toBe(false);
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull(); // the run ended
  });
  it("other_account_is_never_filled", async () => {
    const { h, fills, requests } = await toLogin({ find_matches: { matches: [google({ username: "you@gmail.com" }), google({ id: "8c9e6679-7425-40de-944b-e07fc1f90ae7", username: null })] } });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(requests.some((r) => r.type === "fill_item")).toBe(false);
  });
  it("provider_switch_off_fills_without_press", async () => {
    const { h, fills } = await toLogin({
      find_matches: { matches: [google()] },
      fill_item: { type: "fill_item", username: "Me@Gmail.com", password: "pw", autoSubmit: false },
    });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(1);
    expect(fills[0]?.auto).toBeNull();
  });
  it("subframe_login_is_ignored", async () => {
    const { h, fills, requests } = await toLogin({ find_matches: { matches: [google()] }, fill_item: { type: "fill_item", username: "u", password: "p", autoSubmit: true } });
    const sub: FrameRef = { ...googleFrame(), frameId: 4, topUrl: "https://typeform.com/" };
    await h.handleContent(sub, { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(requests.some((r) => r.type === "find_matches")).toBe(false);
    expect(h.ready(sub, { tabId: 1 })).toBeNull();
    // The run was not spent: the top frame still completes it.
    expect(h.ready(googleFrame(), { tabId: 1 })).toEqual({ kind: "login" });
  });
  it("another origin or an unrelated tab is ignored", async () => {
    const { h, fills } = await toLogin({ find_matches: { matches: [google()] }, fill_item: { type: "fill_item", username: "u", password: "p", autoSubmit: true } });
    await h.handleContent({ tabId: 1, frameId: 0, url: "https://evil.com/", origin: "https://evil.com" }, { tabId: 1 }, { type: "cs_sso_login" });
    await h.handleContent(googleFrame(2), { tabId: 2 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
  });
  it("a denied fill ends the run quietly", async () => {
    const { h, fills } = await toLogin({ find_matches: { matches: [google()] }, fill_item: new BridgeError("denied", "x") });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
  it("cs_sso_stop from the provider tab ends a run waiting for the login form", async () => {
    const { h, fills } = await toLogin({ find_matches: { matches: [google()] }, fill_item: { type: "fill_item", username: "u", password: "p", autoSubmit: true } });
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_stop" });
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull(); // a later provider document gets nothing
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(fills).toHaveLength(0);
  });
  it("cs_sso_stop from the provider popup ends the opener's run waiting for the login form", async () => {
    const env = setup({ start_sso: START_SSO });
    await env.h.start(top, ID);
    expect(env.h.ready(googleFrame(9), { tabId: 9, openerTabId: 1 })).toEqual({ kind: "choose", account: "me@gmail.com" });
    await env.h.handleContent(googleFrame(9), { tabId: 9, openerTabId: 1 }, { type: "cs_sso_stop" });
    expect(env.h.ready(googleFrame(9), { tabId: 9, openerTabId: 1 })).toBeNull();
    expect(env.h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
  it("returning to the site ends a run waiting for the login form", async () => {
    const { h } = await toLogin({});
    expect(h.ready({ tabId: 1, frameId: 0, url: "https://typeform.com/", origin: "https://typeform.com" }, { tabId: 1 })).toBeNull();
    expect(h.ready(googleFrame(), { tabId: 1 })).toBeNull();
  });
  it("a lock while find_matches is in flight fills nothing", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const env = await toLogin({
      find_matches: async () => {
        await gate;
        return { matches: [google()] };
      },
      fill_item: { type: "fill_item", username: "u", password: "p", autoSubmit: true },
    });
    const done = env.h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    await vi.waitFor(() => expect(env.requests.some((r) => r.type === "find_matches")).toBe(true));
    env.h.reset();
    release();
    await done;
    expect(env.requests.some((r) => r.type === "fill_item")).toBe(false);
    expect(env.fills).toHaveLength(0);
  });
  it("a lock while fill_item is in flight fills nothing", async () => {
    let h!: ReturnType<typeof setup>["h"];
    const env = await toLogin({
      find_matches: { matches: [google()] },
      fill_item: () => {
        h.reset();
        return { type: "fill_item", username: "u", password: "p", autoSubmit: true };
      },
    });
    h = env.h;
    await h.handleContent(googleFrame(), { tabId: 1 }, { type: "cs_sso_login" });
    expect(env.fills).toHaveLength(0);
  });
});
