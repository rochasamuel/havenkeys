// Background service worker (Chrome) / event page (Firefox).
//
// The only component that talks to the native host. It accepts runtime
// messages from exactly three kinds of sender, each checked from the
// browser's own sender data:
// * the toolbar popup (an extension page with no tab);
// * the in-page menu, save and passkey frames (extension pages, embedded in
//   a tab);
// * content scripts (in http(s) frames of a tab), including the passkey
//   bridge, which relays the page's navigator.credentials calls.
// Web pages cannot reach it: `externally_connectable` is empty, and page
// script has no extension APIs.

import { NativeClient, type NativePort } from "../messaging/native";
import { newToken, parseContentRequest, parseInlineRequest, type BackgroundToContent, type FillPayload } from "../messaging/inline";
import { parsePopupRequest } from "../messaging/popup";
import { NATIVE_HOST_NAME } from "../shared/constants";
import { pageUrlForRequest } from "../shared/url";
import { createInlineHandler, type AutoRun, type FrameRef } from "./inline-handler";
import { findPasskeySite } from "./passkey-sites";
import { createPopupHandler, type ActiveTab } from "./popup-handler";
import { syncContentScripts } from "./registration";
import { createWebAuthnHandler } from "./webauthn-handler";
import { parsePkRequest, parseWaRequest, type BgWaResize, type BgWaResult } from "../webauthn/messages";

const client = new NativeClient(() => chrome.runtime.connectNative(NATIVE_HOST_NAME) as NativePort, {
  onEvent: (event) => {
    // Locked or desktop gone: drop menus, pending saves and their passwords,
    // and refuse every waiting passkey request.
    if (event.type === "locked" || event.type === "disconnected") {
      inline.reset();
      passkeys.reset();
    }
    // Unlocked: passkey cards showing "locked" look their passkeys up again.
    if (event.type === "unlocked") void passkeys.refreshLocked();
  },
});

type Target = Pick<FrameRef, "tabId" | "frameId" | "documentId">;

async function sendToFrame(target: Target, msg: BackgroundToContent | BgWaResult | BgWaResize): Promise<unknown> {
  const options: { frameId: number; documentId?: string } = { frameId: target.frameId };
  if (target.documentId !== undefined) options.documentId = target.documentId;
  try {
    return await chrome.tabs.sendMessage(target.tabId, msg, options);
  } catch {
    return undefined; // No content script there (any more).
  }
}

/** To every frame of a tab (a passkey result whose session was lost; see webauthn-handler.ts). */
async function sendToTab(tabId: number, msg: BgWaResult): Promise<unknown> {
  try {
    return await chrome.tabs.sendMessage(tabId, msg);
  } catch {
    return undefined;
  }
}

const passkeys = createWebAuthnHandler({ client, sendToFrame, sendToTab, now: Date.now, newToken });
const inline = createInlineHandler({
  client,
  sendToFrame,
  now: Date.now,
  newToken,
  passkeys,
  passkeySite: (url) => findPasskeySite(url),
  openTab: (url) => void chrome.tabs.create({ url }).catch(() => undefined),
});

async function activeTab(): Promise<ActiveTab | undefined> {
  // Readable because the user opened the popup on this tab (activeTab).
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  return tab?.id === undefined ? undefined : { id: tab.id, url: tab.url };
}

/** Popup fill: make sure the content script is in the top frame, then hand it the values. */
async function fillTab(tabId: number, pageUrl: string, payload: FillPayload, auto: AutoRun | null = null): Promise<number> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
  } catch {
    return 0;
  }
  const frame: FrameRef = { tabId, frameId: 0, url: pageUrl, origin: new URL(pageUrl).origin };
  return inline.pickFill(frame, null, payload, auto);
}

const popup = createPopupHandler(client, activeTab, fillTab);

// ------------------------------------------------------------ senders

const extensionOrigin = new URL(chrome.runtime.getURL("/")).origin;

function extensionPage(sender: chrome.runtime.MessageSender): string | null {
  if (sender.id !== chrome.runtime.id || !sender.url) return null;
  try {
    const u = new URL(sender.url);
    return u.origin === extensionOrigin ? u.pathname : null;
  } catch {
    return null;
  }
}

function isFromPopup(sender: chrome.runtime.MessageSender): boolean {
  return extensionPage(sender) === "/popup.html" && sender.tab === undefined;
}

/** The tab and page of a menu, save or passkey frame embedded in a page. */
function inlineFrame(sender: chrome.runtime.MessageSender): { tabId: number; page: string } | null {
  const page = extensionPage(sender);
  if (page !== "/menu.html" && page !== "/save.html" && page !== "/passkey.html") return null;
  const tabId = sender.tab?.id;
  return tabId === undefined ? null : { tabId, page };
}

/**
 * The frame a content script runs in, from the browser's sender data. Null
 * for anything that is not an http(s) frame of a tab, or (for an iframe)
 * when the top page's URL is not readable: iframes are only served with the
 * top page's URL, so the desktop can check both.
 */
function contentFrame(sender: chrome.runtime.MessageSender): FrameRef | null {
  if (sender.id !== chrome.runtime.id || !sender.tab?.id || sender.frameId === undefined) return null;
  const url = pageUrlForRequest(sender.url);
  if (!url) return null;
  const origin = new URL(url).origin;
  // Chrome reports the frame's real origin; a sandboxed frame's is "null".
  const reported = (sender as { origin?: string }).origin;
  if (reported !== undefined && reported !== origin) return null;
  const frame: FrameRef = { tabId: sender.tab.id, frameId: sender.frameId, url, origin };
  const documentId = (sender as { documentId?: string }).documentId;
  if (documentId !== undefined) frame.documentId = documentId;
  if (sender.frameId !== 0) {
    const topUrl = pageUrlForRequest(sender.tab.url);
    if (!topUrl) return null;
    frame.topUrl = topUrl;
  }
  return frame;
}

const GENERIC_ERROR = { ok: false, message: "Something went wrong." };

chrome.runtime.onMessage.addListener((msg: unknown, sender, sendResponse) => {
  const reply = (p: Promise<unknown>) => {
    p.then(sendResponse, () => sendResponse(GENERIC_ERROR));
    return true; // respond asynchronously
  };

  if (isFromPopup(sender)) {
    const req = parsePopupRequest(msg);
    if (!req) {
      sendResponse({ ok: false, message: "Invalid request." });
      return false;
    }
    return reply(popup.handle(req));
  }

  const embedded = inlineFrame(sender);
  if (embedded) {
    if (embedded.page === "/passkey.html") {
      const req = parsePkRequest(msg);
      if (!req) {
        sendResponse({ ok: false, message: "Invalid request." });
        return false;
      }
      return reply(passkeys.handleFrame(embedded.tabId, req));
    }
    const req = parseInlineRequest(msg);
    if (!req) {
      sendResponse({ ok: false, message: "Invalid request." });
      return false;
    }
    return reply(inline.handleInline(embedded.tabId, req));
  }

  const frame = contentFrame(sender);
  if (frame) {
    const wa = parseWaRequest(msg);
    if (wa) return reply(passkeys.handleContent(frame, wa));
    const req = parseContentRequest(msg);
    if (!req) return false;
    return reply(inline.handleContent(frame, req));
  }
  return false;
});

chrome.tabs.onRemoved.addListener((tabId) => {
  inline.forgetTab(tabId);
  passkeys.forgetTab(tabId);
});

// ------------------------------------------------------------ inline suggestions opt-in

const sync = () => void syncContentScripts().catch(() => undefined);
chrome.permissions.onAdded.addListener(sync);
chrome.permissions.onRemoved.addListener(sync);
chrome.runtime.onInstalled.addListener(sync);
chrome.runtime.onStartup.addListener(sync);
sync();
