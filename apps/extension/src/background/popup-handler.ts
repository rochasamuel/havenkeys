// Answers popup requests using the native client. Kept free of `chrome.*`
// so it can be tested with a fake client.

import type { PopupReply, PopupRequest, PopupState, TotpView } from "../messaging/popup";
import { BridgeError, type NativeClient } from "../messaging/native";
import { displayHost, pageUrlForRequest } from "../shared/url";

type Client = Pick<NativeClient, "request">;

function stateForError(e: unknown): PopupState {
  if (!(e instanceof BridgeError)) return { kind: "error", message: "Something went wrong." };
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
  return { ok: false, message: e instanceof BridgeError ? e.message : "Something went wrong." };
}

export function createPopupHandler(client: Client, activeTabUrl: () => Promise<string | undefined>) {
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

  async function handle(req: PopupRequest): Promise<PopupReply<PopupState | TotpView>> {
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
        if (!url) return { ok: false, message: "This page can't use saved logins." };
        try {
          const t = await client.request({ type: "get_totp", itemId: req.itemId, url });
          return { ok: true, value: { code: t.code, secondsRemaining: t.secondsRemaining } };
        } catch (e) {
          return fail(e);
        }
      }
    }
  }

  return { handle };
}
