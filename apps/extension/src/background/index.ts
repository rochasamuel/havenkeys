// Background service worker (Chrome) / event page (Firefox).
//
// The only component that talks to the native host. It accepts runtime
// messages from exactly three kinds of sender, each checked from the
// browser's own sender data:
// * the toolbar popup (an extension page with no tab);
// * the in-page menu and save frames (extension pages, embedded in a tab);
// * content scripts (in http(s) frames of a tab).
// Web pages cannot reach it: `externally_connectable` is empty, and page
// script has no extension APIs.

import { NativeClient, type NativePort } from "../messaging/native";
import { newToken, parseContentRequest, parseInlineRequest, type BackgroundToContent } from "../messaging/inline";
import { parsePopupRequest } from "../messaging/popup";
import { NATIVE_HOST_NAME } from "../shared/constants";
import { pageUrlForRequest } from "../shared/url";
import { createInlineHandler, type FrameRef } from "./inline-handler";
import { createPopupHandler, type ActiveTab } from "./popup-handler";
import { syncContentScripts } from "./registration";

const client = new NativeClient(() => chrome.runtime.connectNative(NATIVE_HOST_NAME) as NativePort, {
  onEvent: (event) => {
    // Locked or desktop gone: drop menus, pending saves and their passwords.
    if (event.type === "locked" || event.type === "disconnected") inline.reset();
  },
});

type Target = Pick<FrameRef, "tabId" | "frameId" | "documentId">;

async function sendToFrame(target: Target, msg: BackgroundToContent): Promise<unknown> {
  const options: { frameId: number; documentId?: string } = { frameId: target.frameId };
  if (target.documentId !== undefined) options.documentId = target.documentId;
  try {
    return await chrome.tabs.sendMessage(target.tabId, msg, options);
  } catch {
    return undefined; // No content script there (any more).
  }
}

const inline = createInlineHandler({ client, sendToFrame, now: Date.now, newToken });

async function activeTab(): Promise<ActiveTab | undefined> {
  // Readable because the user opened the popup on this tab (activeTab).
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  return tab?.id === undefined ? undefined : { id: tab.id, url: tab.url };
}

/** Popup fill: make sure the content script is in the top frame, then hand it the values. */
async function fillTab(tabId: number, pageUrl: string, payload: Parameters<typeof inline.fill>[2]): Promise<number> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
  } catch {
    return 0;
  }
  const frame: FrameRef = { tabId, frameId: 0, url: pageUrl, origin: new URL(pageUrl).origin };
  return inline.fill(frame, null, payload);
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

/** The tab of a menu or save frame embedded in a page. */
function inlineFrameTab(sender: chrome.runtime.MessageSender): number | null {
  const page = extensionPage(sender);
  if (page !== "/menu.html" && page !== "/save.html") return null;
  return sender.tab?.id ?? null;
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

  const tabId = inlineFrameTab(sender);
  if (tabId !== null) {
    const req = parseInlineRequest(msg);
    if (!req) {
      sendResponse({ ok: false, message: "Invalid request." });
      return false;
    }
    return reply(inline.handleInline(tabId, req));
  }

  const frame = contentFrame(sender);
  if (frame) {
    const req = parseContentRequest(msg);
    if (!req) return false;
    return reply(inline.handleContent(frame, req));
  }
  return false;
});

chrome.tabs.onRemoved.addListener((tabId) => inline.forgetTab(tabId));

// ------------------------------------------------------------ inline suggestions opt-in

const sync = () => void syncContentScripts().catch(() => undefined);
chrome.permissions.onAdded.addListener(sync);
chrome.permissions.onRemoved.addListener(sync);
chrome.runtime.onInstalled.addListener(sync);
chrome.runtime.onStartup.addListener(sync);
sync();
