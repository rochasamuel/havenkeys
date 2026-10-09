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
// * Cards are not site-bound (spec 2026-09-29-card-autofill §2): a pick
//   fills the clicked frame and the tab's other card frames that Rust
//   accepts (find_cards per frame, then one fill_card that Rust re-checks
//   frame by frame). Rust compares each frame with the top page only, so a
//   subframe is also dropped unless every frame between it and the top page
//   is one Rust would accept, by the frame's own ancestry report (no
//   report, no card): a payment processor's frame inside an ad's frame
//   gets nothing. One pick makes at most MAX_CARD_LOOKUPS lookups. Each
//   frame gets only its own values, pinned to the document that reported
//   its fields (documentId, Chromium; Firefox checks the origin only).
//   Typed cards wait for the user's save in memory only, for at most
//   CARD_SAVE_TTL_MS; the number goes to Rust only on Save.

import type { Match, PasswordOptions, Request, ResultFor, RequestType } from "@havenkeys/protocol";
import {
  brandOf,
  CARD_BRAND_NAMES,
  DOCUMENT_ROLES,
  luhnOk,
  MAX_CARD_FRAMES,
  SSO_PROVIDERS,
  type CardBrandId,
  type CardRole,
  type IdentityRole,
} from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import { t } from "../i18n";
import {
  MENU_MAX_ROWS,
  parseFillReply,
  parseHostReply,
  type Anchor,
  type Ancestry,
  type CardRowView,
  type CardSaveView,
  type SubmittedCardWire,
  type BackgroundToContent,
  type ContentRequest,
  type FillPayload,
  type IdentityRowView,
  type InlineReply,
  type InlineRequest,
  type MenuHint,
  type MenuKind,
  type MenuView,
  type NextStep,
  type OpenMenuReply,
  type ReadyReply,
  type SaveView,
  type Viewport,
} from "../messaging/inline";
import { displayHost } from "../shared/url";
import { cardRows, displayExpiry } from "./card-rows";
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
  /**
   * An iframe's full URL (query and fragment kept, credentials removed), as
   * the browser reported it. Only ever sent to the tab's top frame, to find
   * the <iframe> for a hosted card menu; never to the desktop.
   */
  fullUrl?: string;
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
  /** Send to every frame of a tab (the card scan). */
  sendToTab?(tabId: number, msg: BackgroundToContent): Promise<unknown>;
  /** Wait `ms` (the card scan's window). Absent: setTimeout. */
  wait?(ms: number): Promise<void>;
}

export const MENU_TTL_MS = 5 * 60_000;
export const CARD_SCAN_MS = 300;
export const CARD_SAVE_TTL_MS = 120_000;
export const MAX_SCAN_REPORTS = 16;
/** find_cards lookups one card pick may make (frames and their ancestors); then the remaining frames are dropped. */
export const MAX_CARD_LOOKUPS = MAX_CARD_FRAMES + 4;

/** A frame's card fields, as it reported them (cs_card_fields). */
export interface CardReport {
  frame: FrameRef;
  roles: CardRole[];
  /** The frame's own account of its ancestors; required for a subframe. */
  ancestry?: Ancestry;
}

/** A frame to fill: its roles, the menu token for the frame the user clicked in, and its ancestry. */
export interface CardTarget {
  frame: FrameRef;
  token: string | null;
  roles: CardRole[];
  ancestry?: Ancestry;
}

interface CardMenu {
  roles: CardRole[];
  /** What the clicked frame reported with cs_open_menu (a subframe's only). */
  ancestry: Ancestry | null;
  rows: CardRowView[];
  insecure: boolean;
}

interface PendingCardSave {
  token: string;
  frame: FrameRef;
  card: SubmittedCardWire;
  brand: CardBrandId | null;
  title: string;
  expires: number;
}
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
  /** The form's identity roles (from the content script). */
  roles: IdentityRole[];
  identity: IdentityRowView | null;
  card: CardMenu | null;
  /** The top frame showing this menu for a processor frame. */
  host: Pick<FrameRef, "tabId" | "frameId"> | null;
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
  const cardSaves = new Map<number, PendingCardSave>(); // by tab
  const scans = new Map<string, { tabId: number; reports: CardReport[] }>(); // by scan token
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
    if (m.host) void deps.sendToFrame(m.host, { type: "bg_close_menu", token: m.token });
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

  function dropCardSave(tabId: number, notify: boolean): void {
    const s = cardSaves.get(tabId);
    if (!s) return;
    cardSaves.delete(tabId);
    // Strings cannot be wiped in JS; dropping the only reference is the most we can do.
    s.card = { number: "", expiry: "", verificationNumber: null, cardholderName: null };
    if (notify) void deps.sendToFrame(top(s.frame), { type: "bg_close_save", token: s.token });
  }

  function liveCardSave(tabId: number, token: string): PendingCardSave | null {
    const s = cardSaves.get(tabId);
    if (!s || s.token !== token) return null;
    if (s.expires <= deps.now()) {
      dropCardSave(tabId, true);
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

  /**
   * The identity row for a form with `roles`, or null when there is nothing
   * to offer. Throws BridgeError("locked") through for the caller.
   */
  async function identityRow(frame: FrameRef, roles: IdentityRole[]): Promise<IdentityRowView | null> {
    let summary: ResultFor<"find_identity">;
    try {
      summary = await deps.client.request({ type: "find_identity", ...frameFields(frame) });
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked") throw e;
      if (e instanceof BridgeError && e.code === "not_found") {
        return { title: "", fills: 0, documents: [], documentsAllowed: false, empty: true, missing: true };
      }
      return null;
    }
    const has = roles.filter((r) => summary.roles.includes(r));
    return {
      title: summary.title,
      fills: has.length,
      documents: has.filter((r) => DOCUMENT_ROLES.includes(r)),
      documentsAllowed: frame.url.startsWith("https:"),
      empty: summary.roles.length === 0,
      missing: false,
    };
  }

  // ------------------------------------------------------------ cards

  /** find_cards lookups of one menu or pick: cached by URL, at most MAX_CARD_LOOKUPS sent. */
  interface Lookups {
    cache: Map<string, Promise<boolean>>;
    left: number;
  }
  const newLookups = (): Lookups => ({ cache: new Map(), left: MAX_CARD_LOOKUPS });
  const lookupKey = (url: string, topUrl: string | undefined) => `${url} ${topUrl ?? ""}`;

  /** Rust would serve a card to a frame at `url` under `topUrl` (a lookup: no values). False once the budget is spent. */
  function cardUrlAllowed(url: string, topUrl: string | undefined, l: Lookups): Promise<boolean> {
    const key = lookupKey(url, topUrl);
    let p = l.cache.get(key);
    if (!p) {
      if (l.left <= 0) return Promise.resolve(false);
      l.left -= 1;
      p = deps.client
        .request({ type: "find_cards", ...(topUrl === undefined ? { url } : { url, topUrl }) })
        .then((r) => !r.insecure)
        .catch(() => false);
      l.cache.set(key, p);
    }
    return p;
  }

  const originOf = (url: string): string | null => {
    try {
      return new URL(url).origin;
    } catch {
      return null;
    }
  };

  /**
   * Rust compares a frame with the top page only, so a card frame inside
   * another frame (a processor's inside an ad's) would pass there. The
   * frame's own report of its ancestors (content/ancestry.ts) must show
   * that every frame between it and the top page is the top page's origin
   * or one Rust would also accept (same-site or a processor). Where the
   * browser could not tell the chain (Firefox, cross-origin parent), only a
   * frame that is a direct child of the top page and that Rust accepts
   * itself passes. The top frame always passes; no report: no subframe.
   */
  async function ancestryAllowed(frame: FrameRef, ancestry: Ancestry | null | undefined, l: Lookups): Promise<boolean> {
    if (frame.frameId === 0) return true;
    const topUrl = frame.topUrl;
    const topOrigin = topUrl === undefined ? null : originOf(topUrl);
    if (!ancestry || topUrl === undefined || topOrigin === null) return false;
    if (ancestry.ancestors === null) return ancestry.directChildOfTop && (await cardUrlAllowed(frame.url, topUrl, l));
    // Nearest first: the last one is the top page (else the tab moved on).
    if (ancestry.ancestors.at(-1) !== topOrigin) return false;
    for (const origin of ancestry.ancestors) {
      if (origin !== topOrigin && !(await cardUrlAllowed(`${origin}/`, topUrl, l))) return false;
    }
    return true;
  }

  /**
   * Ask the top frame to show a processor frame's menu over it (the frame
   * itself is too small). Only ever sent to frame 0; the child's id and URL
   * come from the browser's sender data (the full URL where the browser gave
   * one: iframes of one processor often differ only in query or fragment).
   * `viewport` is the child's own report, only used to pick among iframes.
   */
  async function hostMenu(reply: OpenMenuReply, frame: FrameRef, anchor: Anchor | null, viewport: Viewport | null): Promise<OpenMenuReply> {
    if (!reply.ok || frame.frameId === 0 || !anchor) return reply;
    const m = menus.get(frame.tabId);
    if (!m || m.token !== reply.token) return reply;
    const r = await deps.sendToFrame(top(frame), {
      type: "bg_host_menu",
      token: reply.token,
      frameId: frame.frameId,
      url: frame.fullUrl ?? frame.url,
      anchor,
      rows: reply.rows,
      ...(viewport ? { viewport } : {}),
    });
    if (!parseHostReply(r) || menus.get(frame.tabId) !== m) return reply;
    m.host = top(frame);
    return { ...reply, hosted: true };
  }

  async function openCardMenu(
    frame: FrameRef,
    roles: CardRole[],
    anchor: Anchor | null,
    ancestry: Ancestry | null,
    viewport: Viewport | null,
  ): Promise<OpenMenuReply> {
    // A subframe that cannot say where it sits gets no menu at all.
    if (frame.frameId !== 0 && (!ancestry || (ancestry.ancestors === null && !ancestry.directChildOfTop))) return { ok: false };
    let list: ResultFor<"find_cards">;
    try {
      list = await deps.client.request({ type: "find_cards", ...frameFields(frame) });
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked") {
        // The unlock row carries no card data; the pick re-checks everything.
        const locked = { roles, ancestry, rows: [], insecure: false };
        return hostMenu(register(frame, "card", true, [], [], { hint: null, help: null }, [], null, locked), frame, anchor, viewport);
      }
      // A frame Rust denies, integration off, app gone: stay out of the page.
      return { ok: false };
    }
    if (frame.frameId !== 0) {
      const l = newLookups();
      l.cache.set(lookupKey(frame.url, frame.topUrl), Promise.resolve(!list.insecure));
      if (!(await ancestryAllowed(frame, ancestry, l))) return { ok: false };
    }
    const card = { roles, ancestry, rows: cardRows(list.cards, deps.now()), insecure: list.insecure };
    return hostMenu(register(frame, "card", false, [], [], { hint: null, help: null }, [], null, card), frame, anchor, viewport);
  }

  /** Every frame of the tab reports its card fields for CARD_SCAN_MS. */
  async function scanCards(tabId: number): Promise<CardReport[]> {
    if (!deps.sendToTab) return [];
    const scan = deps.newToken();
    scans.set(scan, { tabId, reports: [] });
    void deps.sendToTab(tabId, { type: "bg_card_scan", scan }).catch(() => undefined);
    await (deps.wait ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms))))(CARD_SCAN_MS);
    const s = scans.get(scan);
    scans.delete(scan);
    return s?.reports ?? [];
  }

  /**
   * Fill a card into the clicked frame (if any) and the tab's other card
   * frames that are on the same top page and that Rust accepts, each with
   * an acceptable ancestry. At most MAX_CARD_LOOKUPS lookups; once spent,
   * the remaining frames are dropped. One fill_card; each frame gets its
   * own values, pinned to its document.
   */
  async function fillCard(tabId: number, topUrl: string, clicked: CardTarget | null, itemId: string): Promise<number> {
    endRun(tabId);
    const reports = await scanCards(tabId);
    const l = newLookups();
    const targets: CardTarget[] = [];
    // The clicked frame passed Rust's lookup when its menu opened; its ancestry is re-checked.
    if (clicked && clicked.roles.length > 0 && (await ancestryAllowed(clicked.frame, clicked.ancestry, l))) targets.push(clicked);
    for (const r of reports) {
      if (targets.length >= MAX_CARD_FRAMES || l.left <= 0) break;
      if (r.frame.tabId !== tabId || targets.some((x) => x.frame.frameId === r.frame.frameId)) continue;
      if ((r.frame.topUrl ?? r.frame.url) !== topUrl) continue;
      if (!(await ancestryAllowed(r.frame, r.ancestry, l))) continue;
      if (!(await cardUrlAllowed(r.frame.url, r.frame.topUrl, l))) continue;
      targets.push({ frame: r.frame, token: null, roles: r.roles });
    }
    if (targets.length === 0) return 0;
    const res = await deps.client.request({ type: "fill_card", itemId, topUrl, frames: targets.map((x) => ({ url: x.frame.url, roles: x.roles })) });
    // One answer per frame, in order; anything else could hand a frame another's values.
    if (res.frames.length !== targets.length) throw new Error("fill_card answered for a different number of frames");
    let n = 0;
    for (const [i, x] of targets.entries()) {
      const values = res.frames[i]?.values ?? [];
      if (values.length > 0) n += (await sendFill(x.frame, x.token, { kind: "card", values }, false, false)).filled;
    }
    return n;
  }

  /** A card the user typed in the top frame: ask to save it unless it is already saved. */
  async function submitCard(frame: FrameRef, card: SubmittedCardWire): Promise<void> {
    if (frame.frameId !== 0 || !frame.url.startsWith("https:") || !luhnOk(card.number)) return;
    let list: ResultFor<"find_cards">;
    try {
      // A lookup by URL only: the number stays here until the user says Save.
      list = await deps.client.request({ type: "find_cards", url: frame.url });
    } catch {
      return;
    }
    if (list.insecure) return;
    const last4 = card.number.slice(-4);
    if (list.cards.some((c) => c.last4 === last4 && c.expiry === card.expiry)) return;
    dropSave(frame.tabId, true);
    dropCardSave(frame.tabId, true);
    const brand = brandOf(card.number);
    const token = deps.newToken();
    cardSaves.set(frame.tabId, {
      token,
      frame,
      card,
      brand,
      title: brand && brand !== "other" ? CARD_BRAND_NAMES[brand] : t.menu.cardFallback,
      expires: deps.now() + CARD_SAVE_TTL_MS,
    });
    setTimeout(() => {
      if (cardSaves.get(frame.tabId)?.token === token) dropCardSave(frame.tabId, true);
    }, CARD_SAVE_TTL_MS);
    void deps.sendToFrame(top(frame), { type: "bg_show_save", token });
  }

  async function openMenu(
    frame: FrameRef,
    kind: MenuKind,
    explicit: boolean,
    roles: IdentityRole[] = [],
    cardRoles: CardRole[] = [],
    anchor: Anchor | null = null,
    ancestry: Ancestry | null = null,
    viewport: Viewport | null = null,
  ): Promise<OpenMenuReply> {
    if (deps.suggestionsOn && !(await deps.suggestionsOn())) {
      // The user hid the menu under login fields; saving and passkeys go on.
      // A site's passkey autofill (a conditional get() waiting in this frame)
      // still gets its rows, and nothing else: signing in with a passkey
      // must not depend on the preference.
      const waiting = kind === "login" && !explicit ? (deps.passkeys?.conditionalFor(frame) ?? []) : [];
      return waiting.length === 0 ? { ok: false } : register(frame, kind, false, [], waiting, { hint: null, help: null });
    }
    if (kind === "card") return openCardMenu(frame, cardRoles, anchor, ancestry, viewport);
    if (kind === "identity") {
      let row: IdentityRowView | null;
      try {
        row = await identityRow(frame, roles);
      } catch {
        return register(frame, kind, true, [], [], { hint: null, help: null }, roles, null);
      }
      if (!row || (!row.empty && row.fills === 0 && !explicit)) return { ok: false };
      return register(frame, kind, false, [], [], { hint: null, help: null }, roles, row);
    }
    let locked = false;
    let items: Match[] = [];
    try {
      const found = await deps.client.request({ type: "find_matches", ...frameFields(frame) });
      // A frozen account fills nothing. A site's passkey autofill (a
      // conditional get() waiting in this frame) still gets its rows, as
      // with the menu turned off: passkey sign-in stays while frozen.
      if (found.entitlement === "frozen") {
        const waiting = kind === "login" && !explicit ? (deps.passkeys?.conditionalFor(frame) ?? []) : [];
        return waiting.length === 0 ? { ok: false } : register(frame, kind, false, [], waiting, { hint: null, help: null });
      }
      items = found.matches;
    } catch (e) {
      // Locked: show a small "unlock HavenKeys" menu. Anything else (app
      // not running, integration off): stay out of the page.
      if (e instanceof BridgeError && e.code === "locked") locked = true;
      else return { ok: false };
    }
    if (kind === "otp") items = items.filter((m) => m.hasTotp);
    if (kind === "new_password") items = [];
    const passkeys = kind === "login" && !locked ? (deps.passkeys?.conditionalFor(frame) ?? []) : [];
    // A sign-up form's email field also offers the identity, when it has something for the form.
    let identity: IdentityRowView | null = null;
    if (kind === "login" && !locked && roles.length > 0) {
      identity = await identityRow(frame, roles).catch(() => null);
      if (identity && identity.fills === 0) identity = null;
    }
    // Nothing to offer: stay out of the page, unless the user asked for the menu.
    if (!locked && kind !== "new_password" && items.length === 0 && passkeys.length === 0 && !identity && !explicit) return { ok: false };
    const extra =
      kind === "login" && !locked && items.length > 0 && passkeys.length === 0 ? await passkeyHint(frame) : { hint: null, help: null };
    return register(frame, kind, locked, items, passkeys, extra, roles, identity);
  }

  function register(
    frame: FrameRef,
    kind: MenuKind,
    locked: boolean,
    items: Match[],
    passkeys: PasskeyRow[],
    { hint, help }: { hint: MenuHint | null; help: string | null },
    roles: IdentityRole[] = [],
    identity: IdentityRowView | null = null,
    card: CardMenu | null = null,
  ): OpenMenuReply {
    closeMenu(frame.tabId);
    const token = deps.newToken();
    menus.set(frame.tabId, {
      token,
      frame,
      kind,
      locked,
      items,
      passkeys,
      hint,
      help,
      roles,
      identity,
      card,
      host: null,
      expires: deps.now() + MENU_TTL_MS,
    });
    const offered = items.length + passkeys.length + (hint ? 1 : 0) + (identity ? 1 : 0);
    const rows =
      locked || kind === "new_password" || kind === "identity"
        ? 1
        : kind === "card"
          ? Math.max(1, Math.min(card && !card.insecure ? card.rows.length : 1, MENU_MAX_ROWS))
          : Math.max(1, Math.min(offered, MENU_MAX_ROWS));
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
    dropCardSave(frame.tabId, true); // one prompt per tab
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
    // A pending card is offered again after the checkout navigates, like a login.
    const c = cardSaves.get(frame.tabId);
    if (c && c.expires > deps.now()) return { saveToken: c.token, watch };
    dropCardSave(frame.tabId, false);
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
        return openMenu(frame, req.kind, req.explicit === true, req.roles ?? [], req.cardRoles ?? [], req.anchor ?? null, req.ancestry ?? null, req.viewport ?? null);
      case "cs_close_menu": {
        const m = menus.get(frame.tabId);
        if (!m || m.token !== req.token) return {};
        if (m.host && (m.frame.frameId === frame.frameId || m.host.frameId === frame.frameId)) {
          // A hosted menu has two frames showing it: close the other one too.
          closeMenu(frame.tabId);
        } else if (m.frame.frameId === frame.frameId) {
          menus.delete(frame.tabId);
        }
        return {};
      }
      case "cs_card_fields": {
        // The tab and frame are the browser's; one report per frame, a bounded number per scan.
        const s = scans.get(req.scan);
        if (s && s.tabId === frame.tabId && s.reports.length < MAX_SCAN_REPORTS && !s.reports.some((r) => r.frame.frameId === frame.frameId)) {
          s.reports.push({ frame, roles: req.roles, ...(req.ancestry ? { ancestry: req.ancestry } : {}) });
        }
        return {};
      }
      case "cs_card_submit":
        await submitCard(frame, req.card);
        return {};
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

  async function handleInline(tabId: number, req: InlineRequest): Promise<InlineReply<MenuView | SaveView | CardSaveView | PasswordOptions | null>> {
    switch (req.type) {
      case "menu_state": {
        const m = liveMenu(tabId, req.token);
        if (!m) return { ok: false, message: t.errors.menuExpired };
        if (m.locked) return { ok: true, value: { state: "locked" } };
        if (m.kind === "card" && m.card) {
          // The top page's site: that is who is paid.
          const site = displayHost(m.frame.topUrl ?? m.frame.url) ?? "";
          return { ok: true, value: { state: "cards", site, cards: m.card.insecure ? [] : m.card.rows, insecure: m.card.insecure } };
        }
        const site = displayHost(m.frame.url) ?? "";
        const items = m.items.map((i) => ({ id: i.id, title: i.title, username: i.username, provider: i.provider, tags: i.tags }));
        return { ok: true, value: { state: "ready", kind: m.kind, site, items, passkeys: m.passkeys, hint: m.hint, identity: m.identity } };
      }
      case "menu_pick_card": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || m.kind !== "card" || !m.card) return { ok: false, message: t.errors.menuExpired };
        // Only cards this menu offered (never on an http page); Rust re-checks every frame anyway.
        if (m.card.insecure || !m.card.rows.some((c) => c.id === req.itemId)) return { ok: false, message: t.errors.unknownItem };
        closeMenu(tabId);
        try {
          const clicked: CardTarget = { frame: m.frame, token: m.token, roles: m.card.roles, ...(m.card.ancestry ? { ancestry: m.card.ancestry } : {}) };
          const filled = await fillCard(tabId, m.frame.topUrl ?? m.frame.url, clicked, req.itemId);
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.menu.cardNothing };
        } catch (e) {
          if (e instanceof BridgeError && e.code === "not_found") return { ok: false, message: t.menu.cardGone };
          return fail(e);
        }
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
      case "menu_generator_options": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || m.kind !== "new_password") return { ok: false, message: t.errors.menuExpired };
        try {
          const r = await deps.client.request({ type: "generator_options" });
          return { ok: true, value: r.options };
        } catch (e) {
          return fail(e);
        }
      }
      case "menu_generate": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || m.kind !== "new_password") return { ok: false, message: t.errors.menuExpired };
        closeMenu(tabId);
        try {
          const g = await deps.client.request({ type: "generate_password", ...(req.options ? { options: req.options } : {}) });
          await pickFill(m.frame, m.token, { kind: "generated", password: g.password }, null);
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_pick_identity": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !m.identity) return { ok: false, message: t.errors.menuExpired };
        if (m.identity.empty) return { ok: false, message: t.menu.identityEmptyBody };
        const docs = req.documents && m.identity.documentsAllowed;
        const roles = m.roles.filter((r) => docs || !DOCUMENT_ROLES.includes(r));
        if (roles.length === 0) return { ok: false, message: t.menu.identityNothing };
        closeMenu(tabId);
        try {
          const r = await deps.client.request({ type: "fill_identity", ...frameFields(m.frame), roles, documents: docs });
          const filled = await pickFill(m.frame, m.token, { kind: "identity", values: r.values }, null);
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.menu.identityNothing };
        } catch (e) {
          return fail(e);
        }
      }
      case "menu_open_identity": {
        const m = liveMenu(tabId, req.token);
        if (!m || !m.identity) return { ok: false, message: t.errors.menuExpired };
        // Nothing to open yet: the menu row only says to open HavenKeys.
        if (m.identity.missing) return { ok: false, message: t.menu.identityMissingBody };
        closeMenu(tabId);
        try {
          await deps.client.request({ type: "open_identity", ...frameFields(m.frame) });
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_open_help": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || !m.help || !deps.openTab) return { ok: false, message: t.errors.menuExpired };
        closeMenu(tabId);
        deps.openTab(m.help);
        return { ok: true, value: null };
      }
      case "menu_show_unlock": {
        // As the popup's Unlock: no URL or item, the desktop only raises its
        // window. The master password is typed there, never in the page.
        const m = liveMenu(tabId, req.token);
        if (!m || !m.locked) return { ok: false, message: t.errors.menuExpired };
        closeMenu(tabId);
        try {
          await deps.client.request({ type: "show_unlock" });
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: null };
      }
      case "menu_close":
        if (liveMenu(tabId, req.token)) closeMenu(tabId);
        return { ok: true, value: null };
      case "menu_resize": {
        const m = liveMenu(tabId, req.token);
        if (!m) return { ok: false, message: t.errors.menuExpired };
        void deps.sendToFrame(m.host ?? m.frame, { type: "bg_resize_menu", token: m.token, height: req.height, ...(req.animate ? { animate: true as const } : {}) });
        return { ok: true, value: null };
      }
      case "save_state": {
        const c = liveCardSave(tabId, req.token);
        if (c) {
          const card = { brand: c.brand, last4: c.card.number.slice(-4), expiry: displayExpiry(c.card.expiry) ?? "" };
          return { ok: true, value: { site: displayHost(c.frame.url) ?? "", title: c.title, card } };
        }
        const s = liveSave(tabId, req.token);
        if (!s) return { ok: false, message: t.errors.promptExpired };
        return { ok: true, value: { action: s.action, site: displayHost(s.frame.url) ?? "", username: s.shownUsername, title: s.title } };
      }
      case "save_confirm": {
        const c = liveCardSave(tabId, req.token);
        if (c) {
          const title = req.title?.trim() || c.title;
          try {
            await deps.client.request({
              type: "save_card",
              url: c.frame.url,
              title,
              number: c.card.number,
              expiry: c.card.expiry,
              ...(c.card.verificationNumber ? { verificationNumber: c.card.verificationNumber } : {}),
              ...(c.card.cardholderName ? { cardholderName: c.card.cardholderName } : {}),
            });
          } catch (e) {
            return fail(e);
          } finally {
            dropCardSave(tabId, true);
          }
          return { ok: true, value: null };
        }
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
        if (liveCardSave(tabId, req.token)) dropCardSave(tabId, true);
        if (liveSave(tabId, req.token)) dropSave(tabId, true);
        return { ok: true, value: null };
      case "save_resize": {
        const s = liveCardSave(tabId, req.token) ?? liveSave(tabId, req.token);
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
    for (const tabId of [...cardSaves.keys()]) dropCardSave(tabId, true);
    scans.clear();
    recentUsernames.clear();
    for (const r of runs.clear()) void deps.sendToFrame({ tabId: r.tabId, frameId: r.frameId }, { type: "bg_run_end" });
  }

  /** A tab closed. */
  function forgetTab(tabId: number): void {
    menus.delete(tabId);
    dropSave(tabId, false);
    dropCardSave(tabId, false);
    recentUsernames.delete(tabId);
    runs.end(tabId);
  }

  return { handleContent, handleInline, pickFill, fillCard, scanCards, reset, forgetTab };
}

export type InlineHandler = ReturnType<typeof createInlineHandler>;
