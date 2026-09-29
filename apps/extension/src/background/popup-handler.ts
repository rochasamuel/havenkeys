// Answers popup requests using the native client. Kept free of `chrome.*`
// so it can be tested with a fake client.

import type { FillPayload } from "../messaging/inline";
import type { PopupReply, PopupRequest, PopupState, TotpView } from "../messaging/popup";
import { BridgeError, type NativeClient } from "../messaging/native";
import { displayHost, pageUrlForRequest } from "../shared/url";
import { t } from "../i18n";
import type { AutoRun } from "./inline-handler";

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
      if (!url) return { kind: "unlocked", site: null, matches: [] };
      const found = await client.request({ type: "find_matches", url });
      return { kind: "unlocked", site: displayHost(url), matches: found.matches };
    } catch (e) {
      return stateForError(e);
    }
  }

  async function handle(req: PopupRequest): Promise<PopupReply<PopupState | TotpView | null>> {
    switch (req.type) {
      case "popup_state":
        return { ok: true, value: await state() };
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
