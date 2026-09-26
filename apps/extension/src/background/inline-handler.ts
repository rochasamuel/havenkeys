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

import type { Match, Request, ResultFor, RequestType } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import type {
  BackgroundToContent,
  ContentRequest,
  FillPayload,
  FillReply,
  InlineReply,
  InlineRequest,
  MenuHint,
  MenuKind,
  MenuView,
  OpenMenuReply,
  ReadyReply,
  SaveView,
} from "../messaging/inline";
import { displayHost } from "../shared/url";
import type { BgWaResize, BgWaResult, PasskeyRow } from "../webauthn/messages";
import type { PasskeySite } from "./passkey-sites";

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
}

export const MENU_TTL_MS = 5 * 60_000;
export const SAVE_TTL_MS = 3 * 60_000;
export const USERNAME_TTL_MS = 5 * 60_000;
/** Rows shown before the menu scrolls. */
const MAX_ROWS = 5;

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
  password: string;
  action: "add" | "update";
  itemId: string | null;
  expires: number;
}

function fail(e: unknown): { ok: false; message: string } {
  return { ok: false, message: e instanceof BridgeError ? e.message : "Something went wrong." };
}

export function createInlineHandler(deps: InlineDeps) {
  const menus = new Map<number, MenuSession>(); // by tab
  const saves = new Map<number, PendingSave>(); // by tab
  const recentUsernames = new Map<number, { origin: string; username: string; expires: number }>();

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

  async function fill(frame: FrameRef, token: string | null, payload: FillPayload): Promise<number> {
    const reply = (await deps.sendToFrame(frame, { type: "bg_fill", origin: frame.origin, token, fill: payload })) as
      | FillReply
      | undefined;
    return typeof reply?.filled === "number" ? reply.filled : 0;
  }

  // ------------------------------------------------------------ content script

  async function openMenu(frame: FrameRef, kind: MenuKind, explicit: boolean): Promise<OpenMenuReply> {
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
    const { hint, help } =
      kind === "login" && !locked && items.length > 0 && passkeys.length === 0 ? await passkeyHint(frame) : { hint: null, help: null };

    closeMenu(frame.tabId);
    const token = deps.newToken();
    menus.set(frame.tabId, { token, frame, kind, locked, items, passkeys, hint, help, expires: deps.now() + MENU_TTL_MS });
    const offered = items.length + passkeys.length + (hint ? 1 : 0);
    const rows = locked || kind === "new_password" ? 1 : Math.max(1, Math.min(offered, MAX_ROWS));
    return { ok: true, token, rows };
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

  async function submit(frame: FrameRef, username: string | null, password: string | null): Promise<void> {
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
      check = await deps.client.request({ type: "check_login", ...frameFields(frame), username, password });
    } catch {
      return; // Locked, not running, rate limited: no prompt.
    }
    if (check.action === "unchanged") return;

    dropSave(frame.tabId, true);
    const token = deps.newToken();
    saves.set(frame.tabId, {
      token,
      frame,
      username,
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
    if (frame.frameId !== 0) return { saveToken: null };
    const s = saves.get(frame.tabId);
    if (!s || s.expires <= deps.now()) {
      dropSave(frame.tabId, false);
      return { saveToken: null };
    }
    return { saveToken: s.token };
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
        await submit(frame, req.username, req.password);
        return {};
      case "cs_ready":
        return ready(frame);
    }
  }

  // ------------------------------------------------------------ menu and save frames

  async function handleInline(tabId: number, req: InlineRequest): Promise<InlineReply<MenuView | SaveView | null>> {
    switch (req.type) {
      case "menu_state": {
        const m = liveMenu(tabId, req.token);
        if (!m) return { ok: false, message: "This menu has expired." };
        if (m.locked) return { ok: true, value: { state: "locked" } };
        const site = displayHost(m.frame.url) ?? "";
        const items = m.items.map((i) => ({ id: i.id, title: i.title, username: i.username }));
        return { ok: true, value: { state: "ready", kind: m.kind, site, items, passkeys: m.passkeys, hint: m.hint } };
      }
      case "menu_pick": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked) return { ok: false, message: "This menu has expired." };
        // Only items this menu offered; the desktop re-checks the origin anyway.
        if (!m.items.some((i) => i.id === req.itemId)) return { ok: false, message: "Unknown item." };
        closeMenu(tabId);
        try {
          if (m.kind === "otp") {
            const t = await deps.client.request({ type: "get_totp", itemId: req.itemId, ...frameFields(m.frame) });
            await fill(m.frame, m.token, { kind: "otp", code: t.code });
          } else {
            const c = await deps.client.request({ type: "fill_item", itemId: req.itemId, ...frameFields(m.frame) });
            await fill(m.frame, m.token, { kind: "login", username: c.username, password: c.password });
          }
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_pick_passkey": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !deps.passkeys) return { ok: false, message: "This menu has expired." };
        if (!m.passkeys.some((p) => p.itemId === req.itemId && p.credentialId === req.credentialId)) {
          return { ok: false, message: "Unknown passkey." };
        }
        closeMenu(tabId);
        return deps.passkeys.pickConditional(m.frame, req.itemId, req.credentialId);
      }
      case "menu_generate": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || m.kind !== "new_password") return { ok: false, message: "This menu has expired." };
        closeMenu(tabId);
        try {
          const g = await deps.client.request({ type: "generate_password" });
          await fill(m.frame, m.token, { kind: "generated", password: g.password });
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_open_help": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !m.help || !deps.openTab) return { ok: false, message: "This menu has expired." };
        closeMenu(tabId);
        deps.openTab(m.help);
        return { ok: true, value: null };
      }
      case "menu_close":
        if (liveMenu(tabId, req.token)) closeMenu(tabId);
        return { ok: true, value: null };
      case "save_state": {
        const s = liveSave(tabId, req.token);
        if (!s) return { ok: false, message: "This prompt has expired." };
        return { ok: true, value: { action: s.action, site: displayHost(s.frame.url) ?? "", username: s.username } };
      }
      case "save_confirm": {
        const s = liveSave(tabId, req.token);
        if (!s) return { ok: false, message: "This prompt has expired." };
        try {
          await deps.client.request({
            type: "save_login",
            ...frameFields(s.frame),
            username: s.username,
            password: s.password,
            itemId: s.itemId,
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
    }
  }

  // ------------------------------------------------------------ lifecycle

  /** Vault locked or desktop gone: forget everything, close every prompt. */
  function reset(): void {
    for (const tabId of [...menus.keys()]) closeMenu(tabId);
    for (const tabId of [...saves.keys()]) dropSave(tabId, true);
    recentUsernames.clear();
  }

  /** A tab closed. */
  function forgetTab(tabId: number): void {
    menus.delete(tabId);
    dropSave(tabId, false);
    recentUsernames.delete(tabId);
  }

  return { handleContent, handleInline, fill, reset, forgetTab };
}

export type InlineHandler = ReturnType<typeof createInlineHandler>;
