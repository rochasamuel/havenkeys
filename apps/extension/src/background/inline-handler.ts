// Background side of in-page autofill: suggestion menus and save prompts.
//
// Kept free of `chrome.*` so it can be tested with a fake native client.
//
// Trust model:
// * `FrameRef`s are built by the caller from the browser's sender data
//   (tab, frame, URL), never from message contents.
// * A menu session binds a random token to the tab and frame the user
//   clicked in, and to that frame's URL. Picks from the menu frame are
//   accepted only from the same tab, only for items the session offered, and
//   the desktop re-checks the item against the URL regardless.
// * Nothing here is persisted. Pending save prompts hold the submitted
//   password in memory only, for at most SAVE_TTL_MS, and are dropped when
//   the vault locks.
// * A sign-in run (signin-run.ts) starts only from a pick whose fill Rust
//   marked autoSubmit, is bound to that tab, frame and origin, and every
//   continuation fill is re-requested from the desktop for the frame's URL.

import type { Match, Request, ResultFor, RequestType } from "@havenkeys/protocol";
import { SSO_PROVIDERS } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import { t } from "../i18n";
import {
  MENU_MAX_ROWS,
  parseFillReply,
  type BackgroundToContent,
  type ContentRequest,
  type FillPayload,
  type InlineReply,
  type InlineRequest,
  type MenuHint,
  type MenuKind,
  type MenuView,
  type NextStep,
  type OpenMenuReply,
  type ReadyReply,
  type SaveView,
} from "../messaging/inline";
import { displayHost } from "../shared/url";
import type { BgWaResize, BgWaResult, PasskeyRow } from "../webauthn/messages";
import type { PasskeySite } from "./passkey-sites";
import { createRuns, nextStep } from "./signin-run";

type Client = {
  request<T extends RequestType>(r: Extract<Request, { type: T }>): Promise<ResultFor<T>>;
};

/** A frame, as the browser reported it. */
export interface FrameRef {
  tabId: number;
  frameId: number;
  /** Chrome only: pins messages to one document, so a navigated frame gets nothing. */
  documentId?: string;
  /** The frame's URL, stripped for sending to the desktop. */
  url: string;
  /** The tab's top-level URL when this is an iframe. */
  topUrl?: string;
  /** The frame's origin; the content script checks it before filling. */
  origin: string;
}

/** A pick whose fill Rust marked autoSubmit: the run it may start. */
export interface AutoRun {
  itemId: string;
  hasTotp: boolean;
}

export interface InlineDeps {
  client: Client;
  /** Send to one frame's content script; resolves to its reply or undefined. */
  sendToFrame(frame: Pick<FrameRef, "tabId" | "frameId" | "documentId">, msg: BackgroundToContent | BgWaResult | BgWaResize): Promise<unknown>;
  now(): number;
  newToken(): string;
  passkeys?: {
    conditionalFor(frame: FrameRef): PasskeyRow[];
    pickConditional(frame: FrameRef, itemId: string, credentialId: string): Promise<InlineReply<null>>;
  };
  /** The page's entry in the Passkeys Directory, if any (Task 7). */
  passkeySite?(url: string): PasskeySite | null;
  /** Opens a new tab, e.g. for the directory's help link. */
  openTab?(url: string): void;
  /** Whether the field menu may open (the options page preference). Absent: on. */
  suggestionsOn?(): Promise<boolean>;
  /** Starts a "Sign in with" run for a login saved with a provider (sso-handler.ts). */
  startSso?(frame: FrameRef, itemId: string): Promise<InlineReply<null>>;
}

export const MENU_TTL_MS = 5 * 60_000;
export const SAVE_TTL_MS = 3 * 60_000;
export const USERNAME_TTL_MS = 5 * 60_000;

interface MenuSession {
  token: string;
  frame: FrameRef;
  kind: MenuKind;
  locked: boolean;
  items: Match[];
  passkeys: PasskeyRow[];
  hint: MenuHint | null;
  help: string | null;
  expires: number;
}

interface PendingSave {
  token: string;
  frame: FrameRef;
  username: string | null;
  /** What the prompt shows: for an update, the saved login's username. */
  shownUsername: string | null;
  /** A new login's suggested name: the known site's, else the host. Null for an update. */
  title: string | null;
  password: string;
  action: "add" | "update";
  itemId: string | null;
  expires: number;
}

function fail(e: unknown): { ok: false; message: string } {
  return { ok: false, message: e instanceof BridgeError ? e.message : t.errors.generic };
}

export function createInlineHandler(deps: InlineDeps) {
  const menus = new Map<number, MenuSession>(); // by tab
  const saves = new Map<number, PendingSave>(); // by tab
  const recentUsernames = new Map<number, { origin: string; username: string; expires: number }>();
  const runs = createRuns(deps.now);

  function endRun(tabId: number): void {
    const r = runs.end(tabId);
    if (r) void deps.sendToFrame({ tabId, frameId: r.frameId }, { type: "bg_run_end" });
  }

  const top = (f: FrameRef) => ({ tabId: f.tabId, frameId: 0 });
  const frameFields = (f: FrameRef) => (f.topUrl === undefined ? { url: f.url } : { url: f.url, topUrl: f.topUrl });

  function closeMenu(tabId: number): void {
    const m = menus.get(tabId);
    if (!m) return;
    menus.delete(tabId);
    void deps.sendToFrame(m.frame, { type: "bg_close_menu", token: m.token });
  }

  function dropSave(tabId: number, notify: boolean): void {
    const s = saves.get(tabId);
    if (!s) return;
    saves.delete(tabId);
    // Strings cannot be wiped in JS; dropping the only reference is the most
    // we can do (docs/security-model.md, memory).
    s.password = "";
    if (notify) void deps.sendToFrame(top(s.frame), { type: "bg_close_save", token: s.token });
  }

  function liveMenu(tabId: number, token: string): MenuSession | null {
    const m = menus.get(tabId);
    if (!m || m.token !== token) return null;
    if (m.expires <= deps.now()) {
      closeMenu(tabId);
      return null;
    }
    return m;
  }

  function liveSave(tabId: number, token: string): PendingSave | null {
    const s = saves.get(tabId);
    if (!s || s.token !== token) return null;
    if (s.expires <= deps.now()) {
      dropSave(tabId, true);
      return null;
    }
    return s;
  }

  async function sendFill(frame: FrameRef, token: string | null, payload: FillPayload, submit: boolean, totp: boolean) {
    const reply = await deps.sendToFrame(frame, { type: "bg_fill", origin: frame.origin, token, fill: payload, submit, totp });
    return parseFillReply(reply);
  }

  /**
   * A fill the user picked (menu or popup). Ends any run in the tab; with
   * `auto`, asks the content script to press, and starts a run if it did.
   */
  async function pickFill(frame: FrameRef, token: string | null, payload: FillPayload, auto: AutoRun | null): Promise<number> {
    endRun(frame.tabId);
    const r = await sendFill(frame, token, payload, auto !== null, auto?.hasTotp ?? false);
    if (auto && r.pressing) runs.start(frame, auto.itemId, r.pressing, auto.hasTotp);
    return r.filled;
  }

  // ------------------------------------------------------------ content script

  async function openMenu(frame: FrameRef, kind: MenuKind, explicit: boolean): Promise<OpenMenuReply> {
    if (deps.suggestionsOn && !(await deps.suggestionsOn())) {
      // The user hid the menu under login fields; saving and passkeys go on.
      // A site's passkey autofill (a conditional get() waiting in this frame)
      // still gets its rows, and nothing else: signing in with a passkey
      // must not depend on the preference.
      const waiting = kind === "login" && !explicit ? (deps.passkeys?.conditionalFor(frame) ?? []) : [];
      return waiting.length === 0 ? { ok: false } : register(frame, kind, false, [], waiting, { hint: null, help: null });
    }
    let locked = false;
    let items: Match[] = [];
    try {
      items = (await deps.client.request({ type: "find_matches", ...frameFields(frame) })).matches;
    } catch (e) {
      // Locked: show a small "unlock HavenKeys" menu. Anything else (app
      // not running, integration off): stay out of the page.
      if (e instanceof BridgeError && e.code === "locked") locked = true;
      else return { ok: false };
    }
    if (kind === "otp") items = items.filter((m) => m.hasTotp);
    if (kind === "new_password") items = [];
    const passkeys = kind === "login" && !locked ? (deps.passkeys?.conditionalFor(frame) ?? []) : [];
    // Nothing to offer: stay out of the page, unless the user asked for the menu.
    if (!locked && kind !== "new_password" && items.length === 0 && passkeys.length === 0 && !explicit) return { ok: false };
    const extra =
      kind === "login" && !locked && items.length > 0 && passkeys.length === 0 ? await passkeyHint(frame) : { hint: null, help: null };
    return register(frame, kind, locked, items, passkeys, extra);
  }

  function register(
    frame: FrameRef,
    kind: MenuKind,
    locked: boolean,
    items: Match[],
    passkeys: PasskeyRow[],
    { hint, help }: { hint: MenuHint | null; help: string | null },
  ): OpenMenuReply {
    closeMenu(frame.tabId);
    const token = deps.newToken();
    menus.set(frame.tabId, { token, frame, kind, locked, items, passkeys, hint, help, expires: deps.now() + MENU_TTL_MS });
    const offered = items.length + passkeys.length + (hint ? 1 : 0);
    const rows = locked || kind === "new_password" ? 1 : Math.max(1, Math.min(offered, MENU_MAX_ROWS));
    return { ok: true, token, rows };
  }

  /** A saved login's username, for the update prompt of a form that had none. */
  async function savedUsername(frame: FrameRef, itemId: string): Promise<string | null> {
    try {
      const { matches } = await deps.client.request({ type: "find_matches", ...frameFields(frame) });
      return matches.find((m) => m.id === itemId)?.username ?? null;
    } catch {
      return null;
    }
  }

  /**
   * Passkeys first: a hint to use the site's own passkey sign-in when
   * HavenKeys holds one for the page, otherwise, for a site in the Passkeys
   * Directory, a link to its help article. The answer only shapes our menu,
   * which the page cannot read.
   */
  async function passkeyHint(frame: FrameRef): Promise<{ hint: MenuHint | null; help: string | null }> {
    let status: ResultFor<"passkey_status">;
    try {
      status = await deps.client.request({ type: "passkey_status", ...frameFields(frame) });
    } catch {
      // Locked meanwhile, desktop gone, rate limited: no hint, no directory row either.
      return { hint: null, help: null };
    }
    if (status.hasPasskey) return { hint: { kind: "use_passkey" }, help: null };
    const site = deps.passkeySite?.(frame.url);
    return site?.help ? { hint: { kind: "add_passkey", name: site.name }, help: site.help } : { hint: null, help: null };
  }

  /**
   * `currentPassword` (a change-password form's) only goes into check_login,
   * where it picks the login to update; it is never kept.
   */
  async function submit(frame: FrameRef, username: string | null, password: string | null, currentPassword?: string): Promise<void> {
    const now = deps.now();
    if (password === null) {
      // A username-only step: remember it briefly for the password step.
      if (username !== null) recentUsernames.set(frame.tabId, { origin: frame.origin, username, expires: now + USERNAME_TTL_MS });
      return;
    }
    if (username === null) {
      const recent = recentUsernames.get(frame.tabId);
      if (recent && recent.origin === frame.origin && recent.expires > now) username = recent.username;
    }
    recentUsernames.delete(frame.tabId);

    let check: ResultFor<"check_login">;
    try {
      check = await deps.client.request({
        type: "check_login",
        ...frameFields(frame),
        username,
        password,
        ...(currentPassword === undefined ? {} : { currentPassword }),
      });
    } catch {
      return; // Locked, not running, rate limited: no prompt.
    }
    if (check.action === "unchanged") return;
    const shownUsername = username ?? (check.itemId === null ? null : await savedUsername(frame, check.itemId));

    dropSave(frame.tabId, true);
    const token = deps.newToken();
    saves.set(frame.tabId, {
      token,
      frame,
      username,
      shownUsername,
      title: check.action === "add" ? (deps.passkeySite?.(frame.url)?.name ?? displayHost(frame.url) ?? null) : null,
      password,
      action: check.action,
      itemId: check.itemId,
      expires: now + SAVE_TTL_MS,
    });
    // Never hold the password longer than the prompt lives.
    setTimeout(() => {
      if (saves.get(frame.tabId)?.token === token) dropSave(frame.tabId, true);
    }, SAVE_TTL_MS);
    // The prompt lives in the top frame. If the page is navigating, the new
    // page's content script asks for it with cs_ready.
    void deps.sendToFrame(top(frame), { type: "bg_show_save", token });
  }

  function ready(frame: FrameRef): ReadyReply {
    const watch = runs.watchFor(frame);
    if (frame.frameId !== 0) return { saveToken: null, watch };
    const s = saves.get(frame.tabId);
    if (!s || s.expires <= deps.now()) {
      dropSave(frame.tabId, false);
      return { saveToken: null, watch };
    }
    return { saveToken: s.token, watch };
  }

  /** cs_run_step: the next step's field appeared in the run's frame. */
  async function continueRun(frame: FrameRef, kind: NextStep): Promise<void> {
    const run = runs.accept(frame, kind);
    if (!run) return;
    let payload: FillPayload;
    let auto: boolean;
    try {
      if (kind === "password") {
        const c = await deps.client.request({ type: "fill_item", itemId: run.itemId, ...frameFields(frame) });
        // The vault may have locked, or another pick/stop may have replaced
        // this run, while that request was in flight.
        if (runs.get(frame.tabId) !== run) return;
        if (c.password === null) return endRun(frame.tabId);
        payload = { kind: "login", username: null, password: c.password };
        auto = c.autoSubmit;
      } else {
        const totp = await deps.client.request({ type: "get_totp", itemId: run.itemId, ...frameFields(frame) });
        if (runs.get(frame.tabId) !== run) return;
        payload = { kind: "otp", code: totp.code };
        auto = totp.autoSubmit;
      }
    } catch {
      if (runs.get(frame.tabId) !== run) return;
      return endRun(frame.tabId); // locked, denied, gone: stop quietly
    }
    const r = await sendFill(frame, null, payload, auto, run.hasTotp);
    if (runs.get(frame.tabId) !== run) return; // superseded while the content script replied
    if (!auto || r.pressing !== kind) return endRun(frame.tabId);
    // The last step was pressed: drop the run here without bg_run_end. The
    // frame's press may still be pending (OTP settle wait, a button that
    // enables late), and bg_run_end would cancel it; the content script
    // ends its own run once that press completes.
    if (nextStep(kind, run.hasTotp) === null) runs.end(frame.tabId);
  }

  async function handleContent(frame: FrameRef, req: ContentRequest): Promise<unknown> {
    switch (req.type) {
      case "cs_open_menu":
        return openMenu(frame, req.kind, req.explicit === true);
      case "cs_close_menu": {
        const m = menus.get(frame.tabId);
        if (m && m.token === req.token && m.frame.frameId === frame.frameId) menus.delete(frame.tabId);
        return {};
      }
      case "cs_submit":
        await submit(frame, req.username, req.password, req.currentPassword);
        return {};
      case "cs_ready":
        return ready(frame);
      case "cs_run_step":
        await continueRun(frame, req.kind);
        return {};
      case "cs_run_stop":
        runs.stop(frame);
        return {};
    }
  }

  // ------------------------------------------------------------ menu and save frames

  async function handleInline(tabId: number, req: InlineRequest): Promise<InlineReply<MenuView | SaveView | null>> {
    switch (req.type) {
      case "menu_state": {
        const m = liveMenu(tabId, req.token);
        if (!m) return { ok: false, message: t.errors.menuExpired };
        if (m.locked) return { ok: true, value: { state: "locked" } };
        const site = displayHost(m.frame.url) ?? "";
        const items = m.items.map((i) => ({ id: i.id, title: i.title, username: i.username, provider: i.provider }));
        return { ok: true, value: { state: "ready", kind: m.kind, site, items, passkeys: m.passkeys, hint: m.hint, identity: null } };
      }
      case "menu_pick": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked) return { ok: false, message: t.errors.menuExpired };
        // Only items this menu offered; the desktop re-checks the origin anyway.
        const offered = m.items.find((i) => i.id === req.itemId);
        if (!offered) return { ok: false, message: t.errors.unknownItem };
        closeMenu(tabId);
        try {
          if (m.kind === "otp") {
            const totp = await deps.client.request({ type: "get_totp", itemId: req.itemId, ...frameFields(m.frame) });
            await pickFill(m.frame, m.token, { kind: "otp", code: totp.code }, totp.autoSubmit ? { itemId: req.itemId, hasTotp: true } : null);
          } else {
            const c = await deps.client.request({ type: "fill_item", itemId: req.itemId, ...frameFields(m.frame) });
            // A login saved with "Sign in with" and no password: press the
            // provider's button instead (start_sso re-checks the item for
            // the page). That happens in the top frame only (spec §6.2); a
            // field in an iframe says the button is not here. A saved
            // password is always filled, provider or not.
            if (c.password === null && offered.provider && m.kind === "login") {
              if (m.frame.frameId !== 0) return { ok: false, message: t.sso.noButton(SSO_PROVIDERS[offered.provider].name) };
              if (deps.startSso) return await deps.startSso(m.frame, req.itemId);
            }
            const auto = c.autoSubmit ? { itemId: req.itemId, hasTotp: offered?.hasTotp ?? false } : null;
            await pickFill(m.frame, m.token, { kind: "login", username: c.username, password: c.password }, auto);
          }
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_pick_passkey": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !deps.passkeys) return { ok: false, message: t.errors.menuExpired };
        if (!m.passkeys.some((p) => p.itemId === req.itemId && p.credentialId === req.credentialId)) {
          return { ok: false, message: t.errors.unknownPasskey };
        }
        closeMenu(tabId);
        return deps.passkeys.pickConditional(m.frame, req.itemId, req.credentialId);
      }
      case "menu_generate": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || m.kind !== "new_password") return { ok: false, message: t.errors.menuExpired };
        closeMenu(tabId);
        try {
          const g = await deps.client.request({ type: "generate_password" });
          await pickFill(m.frame, m.token, { kind: "generated", password: g.password }, null);
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_pick_identity":
      case "menu_open_identity":
        return { ok: false, message: t.errors.menuExpired }; // placeholder until identity menus land
      case "menu_open_help": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !m.help || !deps.openTab) return { ok: false, message: t.errors.menuExpired };
        closeMenu(tabId);
        deps.openTab(m.help);
        return { ok: true, value: null };
      }
      case "menu_close":
        if (liveMenu(tabId, req.token)) closeMenu(tabId);
        return { ok: true, value: null };
      case "menu_resize": {
        const m = liveMenu(tabId, req.token);
        if (!m) return { ok: false, message: t.errors.menuExpired };
        void deps.sendToFrame(m.frame, { type: "bg_resize_menu", token: m.token, height: req.height });
        return { ok: true, value: null };
      }
      case "save_state": {
        const s = liveSave(tabId, req.token);
        if (!s) return { ok: false, message: t.errors.promptExpired };
        return { ok: true, value: { action: s.action, site: displayHost(s.frame.url) ?? "", username: s.shownUsername, title: s.title } };
      }
      case "save_confirm": {
        const s = liveSave(tabId, req.token);
        if (!s) return { ok: false, message: t.errors.promptExpired };
        // A cleared name field falls back to the suggestion; the desktop checks the rest.
        const title = s.action === "add" ? req.title?.trim() || s.title : null;
        try {
          await deps.client.request({
            type: "save_login",
            ...frameFields(s.frame),
            username: s.username,
            password: s.password,
            itemId: s.itemId,
            ...(title ? { title } : {}),
          });
        } catch (e) {
          return fail(e);
        } finally {
          dropSave(tabId, true);
        }
        return { ok: true, value: null };
      }
      case "save_dismiss":
        if (liveSave(tabId, req.token)) dropSave(tabId, true);
        return { ok: true, value: null };
      case "save_resize": {
        const s = liveSave(tabId, req.token);
        if (!s) return { ok: false, message: t.errors.promptExpired };
        void deps.sendToFrame(top(s.frame), { type: "bg_resize_save", token: s.token, height: req.height });
        return { ok: true, value: null };
      }
    }
  }

  // ------------------------------------------------------------ lifecycle

  /** Vault locked or desktop gone: forget everything, close every prompt. */
  function reset(): void {
    for (const tabId of [...menus.keys()]) closeMenu(tabId);
    for (const tabId of [...saves.keys()]) dropSave(tabId, true);
    recentUsernames.clear();
    for (const r of runs.clear()) void deps.sendToFrame({ tabId: r.tabId, frameId: r.frameId }, { type: "bg_run_end" });
  }

  /** A tab closed. */
  function forgetTab(tabId: number): void {
    menus.delete(tabId);
    dropSave(tabId, false);
    recentUsernames.delete(tabId);
    runs.end(tabId);
  }

  return { handleContent, handleInline, pickFill, reset, forgetTab };
}

export type InlineHandler = ReturnType<typeof createInlineHandler>;
