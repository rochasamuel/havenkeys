// Isolated-world half of the passkey bridge. Registered for the same frames
// as page.ts, at document_start. It is the only listener for the page
// script's events and treats each one as hostile input: parsed with
// parsePageRequest, then forwarded to the background worker, which takes the
// frame's URL from the browser. It shows the passkey frame the background
// asks for, and it never leaves the page waiting: every request ends in a
// credential, a fallback or an error, even if the worker dies.
//
// Each valid request is acknowledged at once (`outcome: "ack"`), so the page
// script can tell a missing bridge (Firefox unloads isolated scripts when the
// extension is disabled or updated) from a slow one. While a request has a
// session, the bridge pings it every PING_INTERVAL_MS: that keeps the MV3
// worker from being suspended, and notices when it was anyway (the session
// is then gone). A lost conditional session is asked for again; a lost modal
// one ends in the browser's own implementation.
//
// The site's automatic passkey upgrade (a conditional create) may come back
// already saved (`ui: "saved"`, with the credential): the bridge answers the
// page at once and shows a "passkey saved" notice for NOTICE_MS, which the
// page cannot keep up (touching it removes it). No session is left to ping.

import { InlineFrame, noticeBox, passkeyBox } from "../content/frames";
import {
  clampTimeout,
  parseBgWaResult,
  parsePageRequest,
  parsePingReply,
  parseWaReply,
  NOTICE_MS,
  PING_INTERVAL_MS,
  REQUEST_EVENT,
  RESPONSE_EVENT,
  type Outcome,
  type PageRequest,
  type WaRequest,
} from "./messages";

/** Slack on top of the background's own timeout. */
const GRACE_MS = 10_000;

interface Pending {
  req: Exclude<PageRequest, { kind: "cancel" }>;
  token: string | null;
  frame: InlineFrame | null;
  timer: ReturnType<typeof setTimeout> | null;
  ping: ReturnType<typeof setInterval> | null;
}

/**
 * Never rejects and never throws. After an extension update/reload/disable,
 * `chrome.runtime.sendMessage` throws synchronously ("Extension context
 * invalidated") instead of returning a rejected promise, and a bridge that
 * let that throw escape here would leave the timer callback or the frame's
 * onGone callback before they reach `respond`, hanging the page.
 */
function send(msg: WaRequest): Promise<unknown> {
  try {
    return Promise.resolve(chrome.runtime.sendMessage(msg)).catch(() => undefined);
  } catch {
    return Promise.resolve(undefined);
  }
}

function viewport() {
  return { width: document.documentElement.clientWidth || innerWidth };
}

export function startBridge(): void {
  const pending = new Map<string, Pending>(); // by page request id

  function dispatch(detail: object): void {
    window.dispatchEvent(new CustomEvent(RESPONSE_EVENT, { detail: JSON.stringify(detail) }));
  }

  /** Forget a request here: timers, pings, frame. */
  function end(id: string): Pending | null {
    const p = pending.get(id);
    if (!p) return null;
    pending.delete(id);
    if (p.timer !== null) clearTimeout(p.timer);
    stopPing(p);
    p.frame?.remove();
    return p;
  }

  function respond(id: string, outcome: Outcome): void {
    if (end(id)) dispatch({ id, ...outcome });
  }

  function cancel(id: string): void {
    const p = end(id);
    if (p?.token) void send({ type: "wa_cancel", token: p.token });
  }

  function startPing(id: string, entry: Pending): void {
    stopPing(entry);
    entry.ping = setInterval(() => void check(id, entry), PING_INTERVAL_MS);
  }

  function stopPing(entry: Pending): void {
    if (entry.ping !== null) clearInterval(entry.ping);
    entry.ping = null;
  }

  /** Is the session still there? If not, ask again (conditional) or let the browser take over (modal). */
  async function check(id: string, entry: Pending): Promise<void> {
    const token = entry.token;
    if (!token) return;
    const alive = parsePingReply(await send({ type: "wa_ping", token }));
    if (alive || pending.get(id) !== entry || entry.token !== token) return;
    const req = entry.req;
    if (!(req.kind === "get" && req.options.conditional)) return respond(id, { outcome: "fallback" });
    entry.token = null;
    stopPing(entry);
    const reply = parseWaReply(await send({ type: "wa_get", options: req.options }));
    if (pending.get(id) !== entry) {
      if (reply?.ok) void send({ type: "wa_cancel", token: reply.token });
      return;
    }
    // Nothing for us any more (or locked): the browser's own leg carries on.
    if (!reply?.ok) return;
    entry.token = reply.token;
    startPing(id, entry);
  }

  async function begin(req: Exclude<PageRequest, { kind: "cancel" }>): Promise<void> {
    if (pending.has(req.id)) return;
    const entry: Pending = { req, token: null, frame: null, timer: null, ping: null };
    // Conditional requests live as long as the page; everything else is bounded.
    if (!(req.kind === "get" && req.options.conditional)) {
      const ms = clampTimeout(req.options.timeoutMs) + GRACE_MS;
      entry.timer = setTimeout(() => {
        if (entry.token) void send({ type: "wa_cancel", token: entry.token });
        respond(req.id, { outcome: "error", name: "NotAllowedError" });
      }, ms);
    }
    pending.set(req.id, entry);
    const msg: WaRequest = req.kind === "create" ? { type: "wa_create", options: req.options } : { type: "wa_get", options: req.options };
    const reply = parseWaReply(await send(msg));
    if (pending.get(req.id) !== entry) {
      // Cancelled by the page while we asked.
      if (reply?.ok) void send({ type: "wa_cancel", token: reply.token });
      return;
    }
    if (!reply) return respond(req.id, { outcome: "fallback" });
    if (!reply.ok) return respond(req.id, reply.outcome);
    if (reply.ui === "saved") {
      respond(req.id, { outcome: "credential", credential: reply.credential });
      showNotice(reply.token);
      return;
    }
    entry.token = reply.token;
    startPing(req.id, entry);
    if (reply.ui === "none") return;
    const token = reply.token;
    entry.frame = new InlineFrame("passkey.html", token, passkeyBox(viewport()), () => {
      // The page moved, hid or removed our frame: treat it as a refusal.
      if (pending.get(req.id) !== entry) return;
      void send({ type: "wa_cancel", token });
      respond(req.id, { outcome: "error", name: "NotAllowedError" });
    });
  }

  /** "Passkey saved", for NOTICE_MS; gone at once if the page touches it. */
  function showNotice(token: string): void {
    const frame: InlineFrame = new InlineFrame("passkey.html", token, noticeBox(viewport()), () => frame.remove());
    setTimeout(() => frame.remove(), NOTICE_MS);
  }

  window.addEventListener(REQUEST_EVENT, (e) => {
    const req = parsePageRequest((e as CustomEvent).detail);
    if (!req) return;
    if (req.kind === "cancel") return cancel(req.id);
    // Synchronous: the page script knows at once that a bridge is here.
    dispatch({ id: req.id, outcome: "ack" });
    void begin(req);
  });

  // Leaving the page (or entering the back/forward cache): end every request
  // here and in the background, so no session outlives its document. Firefox
  // has no documentId to tell a navigated frame apart. A page can fire a fake
  // "pagehide" itself, but that only cancels its own requests, which it can
  // do anyway.
  window.addEventListener("pagehide", () => {
    for (const [id, p] of [...pending]) {
      if (p.token) void send({ type: "wa_cancel", token: p.token });
      respond(id, { outcome: "error", name: "AbortError" });
    }
  });

  chrome.runtime.onMessage.addListener((raw: unknown, sender) => {
    // Only this extension's background worker (no tab).
    if (sender.id !== chrome.runtime.id || sender.tab !== undefined) return false;
    const m = parseBgWaResult(raw);
    if (!m) return false;
    for (const [id, p] of pending) {
      if (p.token === m.token) {
        respond(id, m.outcome);
        break;
      }
    }
    return false;
  });
}
