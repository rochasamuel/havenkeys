// Background service worker (Chrome) / event page (Firefox).
//
// The only component that talks to the native host. It accepts runtime
// messages from exactly three kinds of sender, each checked from the
// browser's own sender data:
// * the toolbar popup (an extension page with no tab);
// * the in-page menu, save, passkey and sign-in-with frames (extension
//   pages, embedded in a tab);
// * content scripts (in http(s) frames of a tab), including the passkey
//   bridge, which relays the page's navigator.credentials calls.
// Web pages cannot reach it: `externally_connectable` is empty, and page
// script has no extension APIs.

import { t } from "../i18n";
import { NativeClient, type NativePort } from "../messaging/native";
import { newToken, parseContentRequest, parseIdentityRolesReply, parseInlineRequest, type BackgroundToContent, type FillPayload } from "../messaging/inline";
import type { IdentityRole } from "@havenkeys/protocol";
import { parsePopupRequest } from "../messaging/popup";
import { parseSsoContentRequest, parseSsoFrameRequest, type BackgroundToSso } from "../messaging/sso";
import { NATIVE_HOST_NAME } from "../shared/constants";
import { frameUrlForHost, pageUrlForRequest } from "../shared/url";
import { createInlineHandler, type AutoRun, type FrameRef } from "./inline-handler";
import { findPasskeySite } from "./passkey-sites";
import { createPopupHandler, type ActiveTab } from "./popup-handler";
import { syncContentScripts } from "./registration";
import { getInlineSuggestions } from "../shared/prefs";
import { createSsoHandler } from "./sso-handler";
import type { TabRef } from "./sso-state";
import { createWebAuthnHandler } from "./webauthn-handler";
import { parsePkRequest, parseWaRequest, type BgWaResize, type BgWaResult } from "../webauthn/messages";

const client = new NativeClient(() => chrome.runtime.connectNative(NATIVE_HOST_NAME) as NativePort, {
  onEvent: (event) => {
    // Locked or desktop gone: drop menus, pending saves and their passwords,
    // refuse every waiting passkey request, and end sign-in-with runs.
    if (event.type === "locked" || event.type === "disconnected") {
      inline.reset();
      passkeys.reset();
      sso.reset();
    }
    // Unlocked: passkey cards showing "locked" look their passkeys up again.
    if (event.type === "unlocked") void passkeys.refreshLocked();
  },
});

type Target = Pick<FrameRef, "tabId" | "frameId" | "documentId">;

async function sendToFrame(target: Target, msg: BackgroundToContent | BackgroundToSso | BgWaResult | BgWaResize): Promise<unknown> {
  const options: { frameId: number; documentId?: string } = { frameId: target.frameId };
  if (target.documentId !== undefined) options.documentId = target.documentId;
  try {
    return await chrome.tabs.sendMessage(target.tabId, msg, options);
  } catch {
    return undefined; // No content script there (any more).
  }
}

/** To every frame of a tab (a passkey result whose session was lost, see webauthn-handler.ts; a card scan). */
async function sendToTab(tabId: number, msg: BgWaResult | BackgroundToContent): Promise<unknown> {
  try {
    return await chrome.tabs.sendMessage(tabId, msg);
  } catch {
    return undefined;
  }
}

const passkeys = createWebAuthnHandler({ client, sendToFrame, sendToTab, now: Date.now, newToken });
const sso = createSsoHandler({
  client,
  sendToFrame,
  now: Date.now,
  newToken,
  suggestionsOn: getInlineSuggestions,
  siteName: (url) => findPasskeySite(url)?.name ?? null,
  // The inline handler is created below; it exists by the time a run reaches the provider page.
  pickFill: (frame, payload, auto) => inline.pickFill(frame, null, payload, auto),
});
const inline = createInlineHandler({
  client,
  sendToFrame,
  now: Date.now,
  newToken,
  passkeys,
  passkeySite: (url) => findPasskeySite(url),
  openTab: (url) => void chrome.tabs.create({ url }).catch(() => undefined),
  suggestionsOn: getInlineSuggestions,
  startSso: (frame, itemId) => sso.start(frame, itemId),
  sendToTab: (tabId, msg) => sendToTab(tabId, msg),
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

/** Popup "Sign in": make sure the content script is in the top frame, then start the run. */
async function ssoTab(tabId: number, pageUrl: string, itemId: string) {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
  } catch {
    return { ok: false as const, message: t.errors.pageNotSupported };
  }
  const r = await sso.start({ tabId, frameId: 0, url: pageUrl, origin: new URL(pageUrl).origin }, itemId);
  return r.ok ? { ok: true as const, value: null } : r;
}

/** Popup "Fill identity": which identity fields the top frame can fill now. */
async function scanIdentity(tabId: number): Promise<IdentityRole[]> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
    const reply = await chrome.tabs.sendMessage(tabId, { type: "bg_identity_roles" }, { frameId: 0 });
    return parseIdentityRolesReply(reply);
  } catch {
    return [];
  }
}

/** Popup cards: the top frame gets the content script (the others have it where site access is granted). */
async function injectTop(tabId: number): Promise<boolean> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
    return true;
  } catch {
    return false;
  }
}

const popup = createPopupHandler(client, activeTab, fillTab, ssoTab, scanIdentity, {
  scan: async (tabId) => (await injectTop(tabId)) && (await inline.scanCards(tabId)).length > 0,
  fill: async (tabId, topUrl, itemId) => ((await injectTop(tabId)) ? inline.fillCard(tabId, topUrl, null, itemId) : 0),
});

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

/** The tab and page of a menu, save, passkey or sign-in-with frame embedded in a page. */
function inlineFrame(sender: chrome.runtime.MessageSender): { tabId: number; page: string } | null {
  const page = extensionPage(sender);
  if (page !== "/menu.html" && page !== "/save.html" && page !== "/passkey.html" && page !== "/sso.html") return null;
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
    const fullUrl = frameUrlForHost(sender.url);
    if (fullUrl) frame.fullUrl = fullUrl;
  }
  return frame;
}

const GENERIC_ERROR = { ok: false, message: t.errors.generic };

chrome.runtime.onMessage.addListener((msg: unknown, sender, sendResponse) => {
  const reply = (p: Promise<unknown>) => {
    p.then(sendResponse, () => sendResponse(GENERIC_ERROR));
    return true; // respond asynchronously
  };

  if (isFromPopup(sender)) {
    const req = parsePopupRequest(msg);
    if (!req) {
      sendResponse({ ok: false, message: t.errors.invalidRequest });
      return false;
    }
    return reply(popup.handle(req));
  }

  const embedded = inlineFrame(sender);
  if (embedded) {
    if (embedded.page === "/passkey.html") {
      const req = parsePkRequest(msg);
      if (!req) {
        sendResponse({ ok: false, message: t.errors.invalidRequest });
        return false;
      }
      return reply(passkeys.handleFrame(embedded.tabId, req));
    }
    if (embedded.page === "/sso.html") {
      const req = parseSsoFrameRequest(msg);
      if (!req) {
        sendResponse({ ok: false, message: t.errors.invalidRequest });
        return false;
      }
      return reply(sso.handleFrame(embedded.tabId, req));
    }
    const req = parseInlineRequest(msg);
    if (!req) {
      sendResponse({ ok: false, message: t.errors.invalidRequest });
      return false;
    }
    return reply(inline.handleInline(embedded.tabId, req));
  }

  const frame = contentFrame(sender);
  if (frame) {
    const wa = parseWaRequest(msg);
    if (wa) return reply(passkeys.handleContent(frame, wa));
    // A popup's opener, from the browser: a provider popup the site opened
    // belongs to the site's tab.
    const opener = sender.tab?.openerTabId;
    const tab: TabRef = opener === undefined ? { tabId: frame.tabId } : { tabId: frame.tabId, openerTabId: opener };
    const ssoReq = parseSsoContentRequest(msg);
    if (ssoReq) return reply(sso.handleContent(frame, tab, ssoReq));
    const req = parseContentRequest(msg);
    if (!req) return false;
    if (req.type === "cs_ready") {
      return reply(inline.handleContent(frame, req).then((r) => ({ ...(r as object), sso: sso.ready(frame, tab) })));
    }
    // A username-only step on a provider's page names the account to save.
    if (req.type === "cs_submit" && req.password === null && req.username !== null) sso.noteUsername(frame, tab, req.username);
    return reply(inline.handleContent(frame, req));
  }
  return false;
});

// A site's login popup, and the URLs it or the clicked tab load (an OAuth
// `login_hint` to suggest in the save prompt). URLs are visible to us only
// for sites we have host access to; nothing is stored beyond the hint.
const tabRef = (tab: chrome.tabs.Tab) =>
  tab.id === undefined ? null : tab.openerTabId === undefined ? { tabId: tab.id } : { tabId: tab.id, openerTabId: tab.openerTabId };
chrome.tabs.onCreated.addListener((tab) => {
  const ref = tabRef(tab);
  if (!ref) return;
  if (ref.openerTabId !== undefined) sso.tabCreated(ref);
  const url = tab.pendingUrl ?? tab.url;
  if (url) sso.tabUrl(ref, url);
});
chrome.tabs.onUpdated.addListener((_tabId, change, tab) => {
  const ref = tabRef(tab);
  if (ref && change.url) sso.tabUrl(ref, change.url);
});

chrome.tabs.onRemoved.addListener((tabId) => {
  inline.forgetTab(tabId);
  passkeys.forgetTab(tabId);
  sso.tabRemoved(tabId);
});

// ------------------------------------------------------------ inline suggestions opt-in

const sync = () => void syncContentScripts().catch(() => undefined);
chrome.permissions.onAdded.addListener(sync);
chrome.permissions.onRemoved.addListener(sync);
chrome.runtime.onInstalled.addListener(sync);
chrome.runtime.onStartup.addListener(sync);
sync();
