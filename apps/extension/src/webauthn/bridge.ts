// Isolated-world half of the passkey bridge. Registered for the same frames
// as page.ts, at document_start. It is the only listener for the page
// script's events and treats each one as hostile input: parsed with
// parsePageRequest, then forwarded to the background worker, which takes the
// frame's URL from the browser. It shows the passkey frame the background
// asks for, and it never leaves the page waiting: every request ends in a
// credential, a fallback or an error, even if the worker dies.

import { InlineFrame, passkeyBox } from "../content/frames";
import {
  MAX_TIMEOUT_MS,
  parseBgWaResult,
  parsePageRequest,
  parseWaReply,
  REQUEST_EVENT,
  RESPONSE_EVENT,
  type Outcome,
  type PageRequest,
  type WaRequest,
} from "./messages";

/** Slack on top of the background's own timeout. */
const GRACE_MS = 10_000;

interface Pending {
  token: string | null;
  frame: InlineFrame | null;
  timer: ReturnType<typeof setTimeout> | null;
}

function send(msg: WaRequest): Promise<unknown> {
  return chrome.runtime.sendMessage(msg).catch(() => undefined);
}

function viewport() {
  return { width: document.documentElement.clientWidth || innerWidth };
}

export function startBridge(): void {
  const pending = new Map<string, Pending>(); // by page request id

  function respond(id: string, outcome: Outcome): void {
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    if (p.timer !== null) clearTimeout(p.timer);
    p.frame?.remove();
    window.dispatchEvent(new CustomEvent(RESPONSE_EVENT, { detail: JSON.stringify({ id, ...outcome }) }));
  }

  function cancel(id: string): void {
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    if (p.timer !== null) clearTimeout(p.timer);
    p.frame?.remove();
    if (p.token) void send({ type: "wa_cancel", token: p.token });
  }

  async function begin(req: Exclude<PageRequest, { kind: "cancel" }>): Promise<void> {
    if (pending.has(req.id)) return;
    const entry: Pending = { token: null, frame: null, timer: null };
    // Conditional requests live as long as the page; everything else is bounded.
    if (!(req.kind === "get" && req.options.conditional)) {
      const ms = Math.min(req.options.timeoutMs ?? MAX_TIMEOUT_MS, MAX_TIMEOUT_MS) + GRACE_MS;
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
    entry.token = reply.token;
    if (reply.ui === "none") return;
    const token = reply.token;
    entry.frame = new InlineFrame("passkey.html", token, passkeyBox(viewport()), () => {
      // The page moved, hid or removed our frame: treat it as a refusal.
      if (pending.get(req.id) !== entry) return;
      void send({ type: "wa_cancel", token });
      respond(req.id, { outcome: "error", name: "NotAllowedError" });
    });
  }

  window.addEventListener(REQUEST_EVENT, (e) => {
    const req = parsePageRequest((e as CustomEvent).detail);
    if (!req) return;
    if (req.kind === "cancel") cancel(req.id);
    else void begin(req);
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
