// Background service worker (Chrome) / event page (Firefox).
//
// The only component that talks to the native host. Content scripts and
// web pages cannot reach it: it accepts messages only from this extension's
// own popup page, and `externally_connectable` is empty.

import { NativeClient, type NativePort } from "../messaging/native";
import { parsePopupRequest } from "../messaging/popup";
import { NATIVE_HOST_NAME } from "../shared/constants";
import { createPopupHandler } from "./popup-handler";

const client = new NativeClient(() => chrome.runtime.connectNative(NATIVE_HOST_NAME) as NativePort);

async function activeTabUrl(): Promise<string | undefined> {
  // Readable because the user opened the popup on this tab (activeTab).
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  return tab?.url;
}

const popup = createPopupHandler(client, activeTabUrl);

function isFromPopup(sender: chrome.runtime.MessageSender): boolean {
  return (
    sender.id === chrome.runtime.id &&
    sender.tab === undefined &&
    sender.url === chrome.runtime.getURL("popup.html")
  );
}

chrome.runtime.onMessage.addListener((msg: unknown, sender, sendResponse) => {
  if (!isFromPopup(sender)) return false;
  const req = parsePopupRequest(msg);
  if (!req) {
    sendResponse({ ok: false, message: "Invalid request." });
    return false;
  }
  popup.handle(req).then(sendResponse, () => sendResponse({ ok: false, message: "Something went wrong." }));
  return true; // respond asynchronously
});
