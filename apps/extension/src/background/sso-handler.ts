// Background side of "Sign in with": the offer balloon, the run a pick
// starts, and the save prompt after the user signed in with a provider.
//
// Kept free of `chrome.*` so it can be tested with a fake native client.
//
// Trust model (docs/superpowers/specs/2026-09-28-sign-in-with-design.md §7):
// * Frames come from the browser's sender data. A pick names only an item
//   this tab's offer listed, and start_sso re-checks it against the page.
// * The run's provider origins come from start_sso (Rust). A provider page
//   gets the account to click only in the run's tab or a popup it opened.
// * On the provider's top frame the run completes one login: the single
//   provider login with the run's account, filled by `fill_item` (Rust
//   re-checks the page) and pressed only when Rust's autoSubmit allows
//   (docs/superpowers/specs/2026-09-29-sign-in-with-provider-login-design.md).
// * Save prompts hold no secret. Everything is dropped on lock.

import type { Match, Request, RequestType, ResultFor, SsoProvider } from "@havenkeys/protocol";
import { buttonFrameProvider, SSO_PROVIDERS } from "@havenkeys/protocol";
import { t } from "../i18n";
import type { FillPayload, InlineReply } from "../messaging/inline";
import { BridgeError } from "../messaging/native";
import {
  isAccount,
  parsePressReply,
  type BackgroundToSso,
  type SsoContentRequest,
  type SsoFrameRequest,
  type SsoReady,
  type SsoView,
} from "../messaging/sso";
import { displayHost } from "../shared/url";
import type { AutoRun, FrameRef } from "./inline-handler";
import { createSsoState, type PendingSso, type TabRef } from "./sso-state";

type Client = { request<T extends RequestType>(r: Extract<Request, { type: T }>): Promise<ResultFor<T>> };

export interface SsoDeps {
  client: Client;
  /** Send to one frame's content script; resolves to its reply or undefined. */
  sendToFrame(frame: Pick<FrameRef, "tabId" | "frameId" | "documentId">, msg: BackgroundToSso): Promise<unknown>;
  now(): number;
  newToken(): string;
  /** Whether in-page suggestions are on (the options page preference). Absent: on. */
  suggestionsOn?(): Promise<boolean>;
  /** The page's name in the Passkeys Directory, for a new login's title. */
  siteName?(url: string): string | null;
  /** The inline handler's fill-and-maybe-press (starts an automatic sign-in run when `auto` is set). */
  pickFill?(frame: FrameRef, payload: FillPayload, auto: AutoRun | null): Promise<number>;
}

export const FRAME_TTL_MS = 5 * 60_000;

type Session =
  | { kind: "offer"; token: string; frame: FrameRef; rows: Match[]; expires: number }
  | { kind: "notice"; token: string; frame: FrameRef; provider: SsoProvider; expires: number }
  | {
      kind: "save";
      token: string;
      pending: PendingSso;
      action: "add" | "update";
      itemId: string | null;
      title: string | null;
      accounts: string[];
      expires: number;
    };

function fail(e: unknown): { ok: false; message: string } {
  return { ok: false, message: e instanceof BridgeError ? e.message : t.errors.generic };
}

const frameFields = (f: { url: string; topUrl?: string }) => (f.topUrl === undefined ? { url: f.url } : { url: f.url, topUrl: f.topUrl });
const top = (tabId: number) => ({ tabId, frameId: 0 });
const sameAccount = (a: string | null, b: string | null) => a !== null && b !== null && a.trim().toLowerCase() === b.trim().toLowerCase();

export function createSsoHandler(deps: SsoDeps) {
  const state = createSsoState(deps.now);
  const sessions = new Map<number, Session>(); // by tab; the balloon lives in the top frame
  // Bumped by reset(): an ask() whose check_sso was in flight across a lock is dropped.
  let generation = 0;

  function close(tabId: number): void {
    const s = sessions.get(tabId);
    if (!s) return;
    sessions.delete(tabId);
    void deps.sendToFrame(top(tabId), { type: "bg_sso_close", token: s.token });
  }

  function live(tabId: number, token: string): Session | null {
    const s = sessions.get(tabId);
    if (!s || s.token !== token) return null;
    if (s.expires <= deps.now()) {
      close(tabId);
      return null;
    }
    return s;
  }

  /** Replace the tab's balloon session (closing the one it shows, if any). */
  function open(tabId: number, make: (token: string, expires: number) => Session): Session {
    close(tabId);
    const s = make(deps.newToken(), deps.now() + FRAME_TTL_MS);
    sessions.set(tabId, s);
    return s;
  }

  /** Whether the tab shows a save question that has not expired (an expired one is dropped). */
  function asking(tabId: number): boolean {
    const s = sessions.get(tabId);
    if (s?.kind !== "save") return false;
    return live(tabId, s.token) !== null;
  }

  // ------------------------------------------------------------ content script

  async function offer(frame: FrameRef, providers: SsoProvider[]): Promise<{ ok: true; token: string } | { ok: false }> {
    if (frame.frameId !== 0) return { ok: false };
    const gen = generation;
    if (deps.suggestionsOn && !(await deps.suggestionsOn())) return { ok: false };
    // A live save question outranks an offer.
    if (asking(frame.tabId)) return { ok: false };
    let matches: Match[];
    try {
      matches = (await deps.client.request({ type: "find_matches", ...frameFields(frame) })).matches;
    } catch {
      return { ok: false }; // locked, not running, integration off: stay out of the page
    }
    if (gen !== generation) return { ok: false }; // locked meanwhile
    const rows = matches.filter((m) => m.provider !== null && providers.includes(m.provider));
    if (rows.length === 0) return { ok: false };
    // An ask() may have opened a save question while find_matches was in flight.
    if (asking(frame.tabId)) return { ok: false };
    const s = open(frame.tabId, (token, expires) => ({ kind: "offer", token, frame, rows, expires }));
    return { ok: true, token: s.token };
  }

  async function handleContent(frame: FrameRef, tab: TabRef, req: SsoContentRequest): Promise<unknown> {
    switch (req.type) {
      case "cs_sso_buttons":
        return offer(frame, req.providers);
      case "cs_sso_click": {
        // A provider's button frame embedded in a site (Google's gsi/button):
        // the site the user signs in to is the page embedding it.
        const framed = buttonFrameProvider(frame.url);
        if (framed !== null) {
          if (framed === req.provider && frame.frameId !== 0 && frame.topUrl !== undefined) state.click(frame.tabId, frame.topUrl, undefined, req.provider);
        } else if (!SSO_PROVIDERS[req.provider].origins.includes(frame.origin)) {
          // (A click on the provider's own pages is not signing in somewhere with it.)
          state.click(frame.tabId, frame.url, frame.topUrl, req.provider);
        }
        return {};
      }
      case "cs_sso_account":
        state.account(tab, frame.origin, req.account, frame.frameId === 0);
        return {};
      case "cs_sso_stop": {
        // Any trusted input in the run's tab, or in a popup it opened, ends it (spec §6.3).
        const owner = state.run(tab.tabId) ? tab.tabId : tab.openerTabId;
        if (owner !== undefined && state.run(owner)) state.endRun(owner);
        return {};
      }
      case "cs_sso_login":
        await completeLogin(frame, tab);
        return {};
    }
  }

  /** cs_ready: note the provider visit, ask to save on return, and tell a provider page what to choose. */
  function ready(frame: FrameRef, tab: TabRef): SsoReady {
    const isTop = frame.frameId === 0;
    state.visit(tab, frame.origin, isTop);
    if (isTop) {
      // A save question still open follows the tab to its next page: sites
      // often move on (a popup closes, then the dashboard loads) right as it
      // appears, and the old page takes the balloon with it.
      const s = sessions.get(frame.tabId);
      if (s?.kind === "save" && asking(frame.tabId)) void deps.sendToFrame(top(frame.tabId), { type: "bg_sso_show", token: s.token });
      state.topLoad(frame.tabId, frame.origin);
      const back = state.takeReturn(frame.tabId, frame.origin);
      if (back) void ask(back);
    }
    // Only a top frame chooses: the run is used once, and a provider-origin
    // iframe (a sign-in widget on some page) must not spend it.
    if (!isTop) return null;
    const account = state.chooseFor(tab, frame.origin);
    if (account !== null) return { kind: "choose", account };
    return state.loginReady(tab, frame.origin) ? { kind: "login" } : null;
  }

  /** cs_sso_login: the provider's login form is up; fill the provider login with the run's account. */
  async function completeLogin(frame: FrameRef, tab: TabRef): Promise<void> {
    if (frame.frameId !== 0 || !deps.pickFill) return;
    const run = state.loginFor(tab, frame.origin); // consumed: once per run
    if (!run) return;
    const gen = generation;
    try {
      const { matches } = await deps.client.request({ type: "find_matches", ...frameFields(frame) });
      // Exactly one login for this account; none or several: fill nothing.
      const mine = matches.filter((m) => sameAccount(m.username, run.account));
      if (mine.length !== 1 || gen !== generation) return;
      const login = mine[0] as Match;
      const c = await deps.client.request({ type: "fill_item", itemId: login.id, ...frameFields(frame) });
      if (gen !== generation) return; // locked meanwhile
      // autoSubmit is Rust's: the vault setting and the provider login's switch.
      await deps.pickFill(frame, { kind: "login", username: c.username, password: c.password }, c.autoSubmit ? { itemId: login.id, hasTotp: login.hasTotp } : null);
    } catch {
      // Locked, denied, desktop gone: the run is already over; the field menu stays available.
    }
  }

  /** A username-only step submitted on the provider's page names the account. */
  function noteUsername(frame: FrameRef, tab: TabRef, username: string): void {
    const account = username.trim();
    if (isAccount(account)) state.account(tab, frame.origin, account.toLowerCase(), frame.frameId === 0);
  }

  /** A tab opened by another: the site's login popup, if that tab just clicked a provider button. */
  function tabCreated(tab: TabRef): void {
    state.opened(tab);
  }

  /** A tab closed: a provider popup closing is a return, too. */
  function tabRemoved(tabId: number): void {
    const back = state.takeOnTabClosed(tabId);
    if (back) void ask(back);
    state.forgetTab(tabId);
    sessions.delete(tabId);
  }

  /** The account to suggest: the provider page's own, else the site's login_hint. */
  const suggested = (p: PendingSso) => p.account ?? p.hint;

  /** A URL a tab is loading (from the browser): a login_hint for a pending capture. */
  function tabUrl(tab: TabRef, url: string): void {
    state.hint(tab, url);
  }

  /** Back from the provider: ask whether to save, unless the vault already has it. */
  async function ask(p: PendingSso): Promise<void> {
    const gen = generation;
    let check: ResultFor<"check_sso">;
    try {
      check = await deps.client.request({ type: "check_sso", ...frameFields(p), provider: p.provider, account: suggested(p) });
    } catch {
      return; // locked, not running, rate limited: no prompt
    }
    if (gen !== generation || check.action === "unchanged") return; // locked meanwhile, or nothing new
    const title = check.action === "add" ? (deps.siteName?.(p.url) ?? displayHost(p.url) ?? null) : null;
    const action = check.action;
    const accounts = check.accounts;
    const s = open(p.tabId, (token, expires) => ({ kind: "save", token, pending: p, action, itemId: check.itemId, title, accounts, expires }));
    // If the page is still loading, the send finds no listener; the balloon
    // then misses this prompt, which holds no secret and simply expires.
    void deps.sendToFrame(top(p.tabId), { type: "bg_sso_show", token: s.token });
  }

  // ------------------------------------------------------------ the run

  /** A pick (balloon, field menu or popup): Rust checks the item, then the page's button is pressed. */
  async function start(frame: FrameRef, itemId: string): Promise<InlineReply<null>> {
    let s: ResultFor<"start_sso">;
    try {
      s = await deps.client.request({ type: "start_sso", itemId, ...frameFields(frame) });
    } catch (e) {
      return fail(e);
    }
    state.startRun({
      tabId: frame.tabId,
      frameId: frame.frameId,
      siteOrigin: frame.origin,
      provider: s.provider,
      account: s.account,
      providerOrigins: s.providerOrigins,
      autoChoose: s.autoChoose,
    });
    const { pressed } = parsePressReply(await deps.sendToFrame(frame, { type: "bg_sso_press", provider: s.provider, origin: frame.origin }));
    if (pressed) {
      state.pressed(frame.tabId);
      return { ok: true, value: null };
    }
    state.endRun(frame.tabId);
    const provider = s.provider;
    const n = open(frame.tabId, (token, expires) => ({ kind: "notice", token, frame, provider, expires }));
    void deps.sendToFrame(top(frame.tabId), { type: "bg_sso_show", token: n.token });
    return { ok: false, message: t.sso.noButton(SSO_PROVIDERS[provider].name) };
  }

  // ------------------------------------------------------------ the balloon frame

  function view(s: Session): SsoView {
    switch (s.kind) {
      case "offer":
        return {
          mode: "offer",
          site: displayHost(s.frame.url) ?? "",
          rows: s.rows.map((m) => ({ id: m.id, provider: m.provider as SsoProvider, title: m.title, account: m.username })),
        };
      case "notice":
        return { mode: "notice", site: displayHost(s.frame.url) ?? "", provider: s.provider };
      case "save":
        return {
          mode: "save",
          site: displayHost(s.pending.url) ?? "",
          provider: s.pending.provider,
          account: suggested(s.pending),
          accounts: s.accounts,
          title: s.title,
          action: s.action,
        };
    }
  }

  async function handleFrame(tabId: number, req: SsoFrameRequest): Promise<InlineReply<SsoView | null>> {
    switch (req.type) {
      case "sso_state": {
        const s = live(tabId, req.token);
        return s ? { ok: true, value: view(s) } : { ok: false, message: t.errors.promptExpired };
      }
      case "sso_pick": {
        const s = live(tabId, req.token);
        if (!s || s.kind !== "offer") return { ok: false, message: t.errors.promptExpired };
        // Only items this offer listed; start_sso re-checks the page anyway.
        if (!s.rows.some((r) => r.id === req.itemId)) return { ok: false, message: t.errors.unknownItem };
        close(tabId);
        return start(s.frame, req.itemId);
      }
      case "sso_save": {
        const s = live(tabId, req.token);
        if (!s || s.kind !== "save") return { ok: false, message: t.errors.promptExpired };
        const p = s.pending;
        const account = req.account.trim() || null;
        // A cleared name field falls back to the suggestion; the desktop checks the rest.
        const title = s.action === "add" ? req.title?.trim() || s.title : null;
        try {
          await deps.client.request({
            type: "save_sso",
            ...frameFields(p),
            provider: p.provider,
            account,
            itemId: s.itemId,
            ...(title ? { title } : {}),
          });
        } catch (e) {
          return fail(e);
        } finally {
          close(tabId);
        }
        return { ok: true, value: null };
      }
      case "sso_dismiss":
        if (live(tabId, req.token)) close(tabId);
        return { ok: true, value: null };
      case "sso_resize": {
        const s = live(tabId, req.token);
        if (!s) return { ok: false, message: t.errors.promptExpired };
        void deps.sendToFrame(top(tabId), { type: "bg_sso_resize", token: s.token, height: req.height });
        return { ok: true, value: null };
      }
    }
  }

  // ------------------------------------------------------------ lifecycle

  /** Vault locked or desktop gone: forget everything, close every balloon. */
  function reset(): void {
    generation++;
    for (const tabId of [...sessions.keys()]) close(tabId);
    state.clear();
  }

  return { handleContent, handleFrame, ready, noteUsername, start, tabCreated, tabUrl, tabRemoved, reset };
}

export type SsoHandler = ReturnType<typeof createSsoHandler>;
