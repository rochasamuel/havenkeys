// @vitest-environment jsdom
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { REQUEST_EVENT, RESPONSE_EVENT } from "./messages";

type Listener = (msg: unknown, sender: { id?: string; tab?: unknown }) => boolean | void;
const sent: unknown[] = [];
let reply: (m: unknown) => unknown = () => undefined;
let onMessage: Listener | null = null;
const responses: Array<Record<string, unknown>> = [];

const ID = "0123456789abcdef0123456789abcdef";
const TOKEN = "a".repeat(32);
const get = { rpId: null, challenge: "AQ", allowCredentials: [], conditional: false, timeoutMs: 1000 };

beforeAll(async () => {
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      id: "ext",
      getURL: (p: string) => `chrome-extension://ext/${p}`,
      sendMessage: async (m: unknown) => {
        sent.push(m);
        return reply(m);
      },
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
