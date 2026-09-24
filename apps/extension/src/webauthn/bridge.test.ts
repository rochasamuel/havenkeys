// @vitest-environment jsdom
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_TIMEOUT_MS, REQUEST_EVENT, RESPONSE_EVENT } from "./messages";

type Listener = (msg: unknown, sender: { id?: string; tab?: unknown }) => boolean | void;
const sent: unknown[] = [];
let reply: (m: unknown) => unknown = () => undefined;
let onMessage: Listener | null = null;
const responses: Array<Record<string, unknown>> = [];

const ID = "0123456789abcdef0123456789abcdef";
const TOKEN = "a".repeat(32);
const get = { rpId: null, challenge: "AQ", allowCredentials: [], conditional: false, timeoutMs: 1000 };

/** A 32-hex-char id/token distinct from the fixtures above, so tests never share pending state. */
const hex = (n: number): string => n.toString(16).padStart(32, "0");

/** Swappable per test; defaults to the `reply`-driven mock below. */
let sendMessageImpl: (m: unknown) => unknown = () => undefined;
const defaultSendMessageImpl = async (m: unknown) => {
  sent.push(m);
  return reply(m);
};

beforeAll(async () => {
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      id: "ext",
      getURL: (p: string) => `chrome-extension://ext/${p}`,
      sendMessage: (m: unknown) => sendMessageImpl(m),
      onMessage: { addListener: (l: Listener) => (onMessage = l) },
    },
  };
  window.addEventListener(RESPONSE_EVENT, (e) => responses.push(JSON.parse((e as CustomEvent).detail as string)));
  const { startBridge } = await import("./bridge");
  startBridge();
});

beforeEach(() => {
  sent.length = 0;
  responses.length = 0;
  vi.useRealTimers();
  sendMessageImpl = defaultSendMessageImpl;
});

const request = (detail: unknown) => window.dispatchEvent(new CustomEvent(REQUEST_EVENT, { detail: JSON.stringify(detail) }));
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("isolated bridge", () => {
  it("ignores malformed page events", async () => {
    window.dispatchEvent(new CustomEvent(REQUEST_EVENT, { detail: { kind: "get" } }));
    request({ kind: "get", id: ID, options: { ...get, url: "https://evil" } });
    await flush();
    expect(sent).toEqual([]);
  });

  it("falls back when the background cannot be reached", async () => {
    reply = () => {
      throw new Error("no worker");
    };
    request({ kind: "get", id: ID, options: get });
    await flush();
    expect(responses).toEqual([{ id: ID, outcome: "fallback" }]);
  });

  it("shows the chooser and relays the result", async () => {
    reply = () => ({ ok: true, token: TOKEN, ui: "chooser" });
    request({ kind: "get", id: ID, options: get });
    await flush();
    const frame = document.querySelector("iframe");
    expect(frame?.src).toBe(`chrome-extension://ext/passkey.html#${TOKEN}`);
    onMessage?.({ type: "bg_wa_result", token: TOKEN, outcome: { outcome: "error", name: "NotAllowedError" } }, { id: "ext" });
    expect(responses).toEqual([{ id: ID, outcome: "error", name: "NotAllowedError" }]);
    expect(document.querySelector("iframe")).toBeNull();
  });

  it("does not accept results from pages or other extensions", async () => {
    reply = () => ({ ok: true, token: TOKEN, ui: "none" });
    request({ kind: "get", id: ID, options: get });
    await flush();
    onMessage?.({ type: "bg_wa_result", token: TOKEN, outcome: { outcome: "fallback" } }, { id: "other" });
    onMessage?.({ type: "bg_wa_result", token: TOKEN, outcome: { outcome: "fallback" } }, { id: "ext", tab: {} });
    expect(responses).toEqual([]);
    request({ kind: "cancel", id: ID });
    await flush();
    expect(sent.at(-1)).toEqual({ type: "wa_cancel", token: TOKEN });
  });

  it("never leaves the page hanging when the worker dies", async () => {
    vi.useFakeTimers();
    reply = () => ({ ok: true, token: TOKEN, ui: "chooser" });
    request({ kind: "get", id: ID, options: get });
    await vi.advanceTimersByTimeAsync(1000 + 10_000 + 1);
    expect(responses).toEqual([{ id: ID, outcome: "error", name: "NotAllowedError" }]);
  });
});

// The bridge must never leave the page hanging even if the extension context
// is invalidated (update/reload/disable) mid-flight: chrome.runtime.sendMessage
// then throws synchronously instead of returning a rejected promise.
describe("extension context invalidation", () => {
  it("falls back immediately for a new request when sendMessage throws synchronously", async () => {
    sendMessageImpl = () => {
      throw new Error("Extension context invalidated.");
    };
    const id = hex(1);
    request({ kind: "get", id, options: get });
    await flush();
    expect(responses).toEqual([{ id, outcome: "fallback" }]);
  });

  it("still resolves NotAllowedError via the timeout when sendMessage throws synchronously while cancelling", async () => {
    vi.useFakeTimers();
    const id = hex(2);
    const token = hex(3);
    reply = () => ({ ok: true, token, ui: "chooser" });
    request({ kind: "get", id, options: get });
    // Let the reply resolve so entry.token is set (and the frame shown) before pulling the rug out.
    await vi.advanceTimersByTimeAsync(0);
    expect(document.querySelector("iframe")).not.toBeNull();
    sendMessageImpl = () => {
      throw new Error("Extension context invalidated.");
    };
    await vi.advanceTimersByTimeAsync(1000 + 10_000 + 1);
    expect(responses).toEqual([{ id, outcome: "error", name: "NotAllowedError" }]);
  });
});

describe("additional security paths", () => {
  it("cancels and reports NotAllowedError when the page tampers with the chooser frame", async () => {
    const id = hex(4);
    const token = hex(5);
    reply = () => ({ ok: true, token, ui: "chooser" });
    request({ kind: "get", id, options: get });
    await flush();
    const frame = document.querySelector("iframe");
    expect(frame).not.toBeNull();
    frame?.setAttribute("style", "display:none");
    await vi.waitFor(() => {
      expect(responses).toEqual([{ id, outcome: "error", name: "NotAllowedError" }]);
    });
    expect(sent.at(-1)).toEqual({ type: "wa_cancel", token });
    expect(document.querySelector("iframe")).toBeNull();
  });

  it("cancels the late reply with its token when the page cancels before the background answers", async () => {
    const id = hex(6);
    const token = hex(7);
    reply = () => ({ ok: true, token, ui: "chooser" });
    request({ kind: "get", id, options: get });
    request({ kind: "cancel", id });
    await flush();
    expect(sent).toEqual([
      { type: "wa_get", options: get },
      { type: "wa_cancel", token },
    ]);
    expect(responses).toEqual([]);
    expect(document.querySelector("iframe")).toBeNull();
  });

  it("never times out a conditional request", async () => {
    vi.useFakeTimers();
    const id = hex(8);
    const token = hex(9);
    reply = () => ({ ok: true, token, ui: "none" });
    request({ kind: "get", id, options: { ...get, conditional: true } });
    await vi.advanceTimersByTimeAsync(MAX_TIMEOUT_MS + 10_000 + 60_000);
    expect(responses).toEqual([]);
  });
});
