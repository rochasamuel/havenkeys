// Answers popup requests using the native client. Kept free of `chrome.*`
// so it can be tested with a fake client.

import { DOCUMENT_ROLES, type IdentityRole } from "@havenkeys/protocol";
import type { FillPayload } from "../messaging/inline";
import type { CardsView, IdentityFillReply, PopupReply, PopupRequest, PopupState, TotpView } from "../messaging/popup";
import { BridgeError, type NativeClient } from "../messaging/native";
import { displayHost, pageUrlForRequest } from "../shared/url";
import { t } from "../i18n";
import type { AutoRun } from "./inline-handler";
import { cardRows } from "./card-rows";

/** Card scan and fill for the popup (multi-frame; Rust checks every frame). */
export interface CardAccess {
  scan(tabId: number): Promise<boolean>;
  fill(tabId: number, topUrl: string, itemId: string): Promise<number>;
}

const PRICING_URL = "https://havenkeys.net/pricing";

type Client = Pick<NativeClient, "request">;

/** The tab the popup was opened on (readable thanks to activeTab). */
export interface ActiveTab {
  id: number;
  url: string | undefined;
}

/**
 * Start a "Sign in with" run in the tab's top frame (injecting the content
 * script if needed): the desktop checks the item, the page's provider
 * button is pressed.
 */
export type SsoStarter = (tabId: number, pageUrl: string, itemId: string) => Promise<PopupReply<null>>;

/**
 * Inject the content script into the tab's top frame and ask which identity
 * fields it can fill now.
 */
export type IdentityScanner = (tabId: number) => Promise<IdentityRole[]>;

/**
 * Put a fill into the tab's top frame (injecting the content script if
 * needed). Resolves to the number of fields filled.
 */
export type TabFiller = (tabId: number, pageUrl: string, payload: FillPayload, auto?: AutoRun | null) => Promise<number>;

function stateForError(e: unknown): PopupState {
  if (!(e instanceof BridgeError)) return { kind: "error", message: t.errors.generic };
  switch (e.code) {
    case "host_unavailable":
      return { kind: "host_unavailable" };
    case "desktop_unavailable":
      return { kind: "desktop_unavailable" };
    case "locked":
      return { kind: "locked" };
    case "no_vault":
      return { kind: "no_vault" };
    case "frozen":
      return { kind: "frozen", site: null, matches: [] };
    case "integration_disabled":
      return { kind: "disabled" };
    default:
      // Protocol error messages are fixed strings chosen by the Rust side.
      return { kind: "error", message: e.message };
  }
}

function fail(e: unknown): { ok: false; message: string } {
  return { ok: false, message: e instanceof BridgeError ? e.message : t.errors.generic };
}

export function createPopupHandler(
  client: Client,
  activeTab: () => Promise<ActiveTab | undefined>,
  fillTab: TabFiller = async () => 0,
  startSso?: SsoStarter,
  scanIdentity?: IdentityScanner,
  cards?: CardAccess,
) {
  const activeTabUrl = async () => (await activeTab())?.url;

  async function fillFromPopup(itemId: string, totp: boolean): Promise<PopupReply<null>> {
    // Tab and URL are read here, never taken from the popup; the desktop
    // checks the item is saved for the URL, and the content script checks
    // the page is still on that origin before writing anything.
    const tab = await activeTab();
    const url = pageUrlForRequest(tab?.url);
    if (!tab || !url) return { ok: false, message: t.errors.pageNotSupported };
    try {
      let filled: number;
      if (totp) {
        const otp = await client.request({ type: "get_totp", itemId, url });
        filled = await fillTab(tab.id, url, { kind: "otp", code: otp.code }, otp.autoSubmit ? { itemId, hasTotp: true } : null);
      } else {
        // One lookup, no secrets: whether the login signs in with a
        // provider, and whether a run needs its OTP step.
        const { matches } = await client.request({ type: "find_matches", url });
        const m = matches.find((x) => x.id === itemId);
        const c = await client.request({ type: "fill_item", itemId, url });
        // A saved password is filled even when the login also signs in with
        // a provider; only a login without one presses the provider's button.
        if (c.password === null && m?.provider && startSso) return startSso(tab.id, url, itemId);
        const auto = c.autoSubmit ? { itemId, hasTotp: m?.hasTotp ?? false } : null;
        filled = await fillTab(tab.id, url, { kind: "login", username: c.username, password: c.password }, auto);
      }
      return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.errors.noLoginForm };
    } catch (e) {
      return fail(e);
    }
  }

  async function state(): Promise<PopupState> {
    try {
      const status = await client.request({ type: "status" });
      if (!status.vaultExists) return { kind: "no_vault" };
      if (status.state !== "unlocked") return { kind: "locked" };
      const url = pageUrlForRequest(await activeTabUrl());
      if (status.entitlement === "frozen") {
        // Read-only: the site's logins (metadata) for their codes; no identity lookup.
        if (!url) return { kind: "frozen", site: null, matches: [] };
        const found = await client.request({ type: "find_matches", url });
        return { kind: "frozen", site: displayHost(url), matches: found.matches };
      }
      if (!url) return { kind: "unlocked", site: null, matches: [], identity: null };
      // Both at once: the popup waits for the slower of the two, not their sum.
      const [found, identity] = await Promise.all([
        client.request({ type: "find_matches", url }),
        client
          .request({ type: "find_identity", url })
          .then((s) => (s.roles.length > 0 ? { title: s.title } : null))
          .catch(() => null),
      ]);
      return { kind: "unlocked", site: displayHost(url), matches: found.matches, identity };
    } catch (e) {
      return stateForError(e);
    }
  }

  /**
   * Cards for the active tab, on https pages whose frames have card fields.
   * Apart from `state()` so the scan's wait never holds back the logins.
   */
  async function cardsView(): Promise<CardsView> {
    const tab = await activeTab();
    const url = pageUrlForRequest(tab?.url);
    if (!cards || !tab || !url?.startsWith("https:")) return null;
    try {
      const found = await client.request({ type: "find_cards", url });
      if (found.cards.length === 0 || !(await cards.scan(tab.id))) return null;
      return { cards: cardRows(found.cards, Date.now()), origin: new URL(url).origin };
    } catch {
      return null;
    }
  }

  async function handle(req: PopupRequest): Promise<PopupReply<PopupState | CardsView | TotpView | IdentityFillReply>> {
    switch (req.type) {
      case "popup_state":
        return { ok: true, value: await state() };
      case "popup_cards":
        return { ok: true, value: await cardsView() };
      case "popup_lock":
        try {
          await client.request({ type: "lock" });
        } catch (e) {
          return fail(e);
        }
        return { ok: true, value: await state() };
      case "popup_totp": {
        // The URL is re-read here, never taken from the popup, and the
        // desktop checks the item is saved for it.
        const url = pageUrlForRequest(await activeTabUrl());
        if (!url) return { ok: false, message: t.errors.pageNotSupported };
        try {
          const totp = await client.request({ type: "get_totp", itemId: req.itemId, url });
          return { ok: true, value: { code: totp.code, secondsRemaining: totp.secondsRemaining } };
        } catch (e) {
          return fail(e);
        }
      }
      case "popup_fill":
        return fillFromPopup(req.itemId, false);
      case "popup_fill_totp":
        return fillFromPopup(req.itemId, true);
      case "popup_fill_identity": {
        const tab = await activeTab();
        const url = pageUrlForRequest(tab?.url);
        if (!tab || !url || !scanIdentity) return { ok: false, message: t.errors.pageNotSupported };
        try {
          const origin = new URL(url).origin;
          // The documents answer is for the site the question named.
          if (req.documents !== null && req.origin !== origin) return { ok: false, message: t.errors.identityPageChanged };
          const pageRoles = await scanIdentity(tab.id);
          if (pageRoles.length === 0) return { ok: false, message: t.errors.noIdentityForm };
          const summary = await client.request({ type: "find_identity", url });
          const has = pageRoles.filter((r) => summary.roles.includes(r));
          // Documents only on https, whatever the popup says.
          const docs = url.startsWith("https:") ? has.filter((r) => DOCUMENT_ROLES.includes(r)) : [];
          if (req.documents === null && docs.length > 0) return { ok: true, value: { confirm: docs, origin } };
          const withDocs = req.documents === true && docs.length > 0;
          const roles = has.filter((r) => withDocs || !DOCUMENT_ROLES.includes(r));
          if (roles.length === 0) return { ok: false, message: t.menu.identityNothing };
          const r = await client.request({ type: "fill_identity", url, roles, documents: withDocs });
          const filled = await fillTab(tab.id, url, { kind: "identity", values: r.values });
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.menu.identityNothing };
        } catch (e) {
          return fail(e);
        }
      }
      case "popup_fill_card": {
        // Tab and URL are read here, never taken from the popup; Rust checks every frame.
        const tab = await activeTab();
        const url = pageUrlForRequest(tab?.url);
        if (!tab || !url || !cards) return { ok: false, message: t.errors.pageNotSupported };
        // The list was built for one origin; a tab that moved gets nothing.
        if (req.origin !== new URL(url).origin) return { ok: false, message: t.errors.cardPageChanged };
        try {
          const offered = await client.request({ type: "find_cards", url });
          if (!offered.cards.some((c) => c.id === req.itemId)) return { ok: false, message: t.errors.unknownItem };
          const filled = await cards.fill(tab.id, url, req.itemId);
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.errors.noCardForm };
        } catch (e) {
          return fail(e);
        }
      }
      case "popup_open_pricing":
        await chrome.tabs.create({ url: PRICING_URL });
        return { ok: true, value: null };
      case "popup_show_unlock":
        // No tab, URL or item: the desktop only raises its window. The
        // master password is typed there, never in the browser.
        try {
          await client.request({ type: "show_unlock" });
          return { ok: true, value: null };
        } catch (e) {
          return fail(e);
        }
      case "popup_open_item": {
        // Same rule as fill: the URL is the tab's, and the desktop opens
        // only a login saved for it.
        const url = pageUrlForRequest(await activeTabUrl());
        if (!url) return { ok: false, message: t.errors.pageNotSupported };
        try {
          await client.request({ type: "open_item", itemId: req.itemId, url });
          return { ok: true, value: null };
        } catch (e) {
          return fail(e);
        }
      }
    }
  }

  return { handle };
}
