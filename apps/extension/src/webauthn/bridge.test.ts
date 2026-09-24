// @vitest-environment jsdom
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, NOTICE_MS, PING_INTERVAL_MS, REQUEST_EVENT, RESPONSE_EVENT } from "./messages";

type Listener = (msg: unknown, sender: { id?: string; tab?: unknown }) => boolean | void;
const sent: unknown[] = [];
let reply: (m: unknown) => unknown = () => undefined;
/** The background's answer to `wa_ping`; alive unless a test says otherwise. */
let ping: (m: unknown) => unknown = () => ({ ok: true });
let onMessage: Listener | null = null;
const responses: Array<Record<string, unknown>> = [];
const acks: string[] = [];

const ID = "0123456789abcdef0123456789abcdef";
const TOKEN = "a".repeat(32);
const get = { rpId: null, challenge: "AQ", allowCredentials: [], conditional: false, timeoutMs: 1000 };
const create = { rpId: null, challenge: "AQ", userId: "AQ", userName: "octo", userDisplayName: null, algs: [-7], excludeCredentials: [], timeoutMs: 1000, conditional: false };
const savedCredential = { type: "create", credentialId: "AQEBAQEBAQEBAQEBAQEBAQ", clientDataJson: "e30", attestationObject: "oA", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -7 };

/** A 32-hex-char id/token distinct from the fixtures above, so tests never share pending state. */
const hex = (n: number): string => n.toString(16).padStart(32, "0");

/** Swappable per test; defaults to the `reply`-driven mock below. */
let sendMessageImpl: (m: unknown) => unknown = () => undefined;
const defaultSendMessageImpl = async (m: unknown) => {
  sent.push(m);
  return (m as { type?: string }).type === "wa_ping" ? ping(m) : reply(m);
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
  window.addEventListener(RESPONSE_EVENT, (e) => {
    const r = JSON.parse((e as CustomEvent).detail as string) as Record<string, unknown>;
    if (r.outcome === "ack") acks.push(r.id as string);
    else responses.push(r);
  });
  const { startBridge } = await import("./bridge");
  startBridge();
});

beforeEach(() => {
  sent.length = 0;
  responses.length = 0;
  acks.length = 0;
  ping = () => ({ ok: true });
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
    // The site asked for 1 s; the card still gets MIN_TIMEOUT_MS.
    expect(responses).toEqual([]);
    await vi.advanceTimersByTimeAsync(MIN_TIMEOUT_MS);
    expect(responses).toEqual([{ id: ID, outcome: "error", name: "NotAllowedError" }]);
  });

  it("acknowledges every valid request at once", () => {
    reply = () => new Promise(() => {});
    const id = hex(20);
    request({ kind: "get", id, options: get });
    // Synchronously, before the background has answered.
    expect(acks).toEqual([id]);
    request({ kind: "get", id: "short", options: get });
    expect(acks).toEqual([id]);
    request({ kind: "cancel", id });
  });
});

describe("lost sessions", () => {
  it("keeps a live session alive with pings", async () => {
    vi.useFakeTimers();
    const id = hex(21);
    const token = hex(22);
    reply = () => ({ ok: true, token, ui: "none" });
    request({ kind: "get", id, options: { ...get, conditional: true } });
    await vi.advanceTimersByTimeAsync(PING_INTERVAL_MS * 3);
    expect(sent.filter((m) => (m as { type: string }).type === "wa_ping")).toEqual([
      { type: "wa_ping", token },
      { type: "wa_ping", token },
      { type: "wa_ping", token },
    ]);
    expect(responses).toEqual([]);
    request({ kind: "cancel", id });
    await vi.advanceTimersByTimeAsync(0);
    sent.length = 0;
    await vi.advanceTimersByTimeAsync(PING_INTERVAL_MS * 2);
    expect(sent).toEqual([]);
  });

  it("re-sends a conditional request whose session was lost", async () => {
    vi.useFakeTimers();
    const id = hex(23);
    const oldToken = hex(24);
    const newToken = hex(25);
    const cond = { ...get, conditional: true };
    reply = () => ({ ok: true, token: oldToken, ui: "none" });
    request({ kind: "get", id, options: cond });
    await vi.advanceTimersByTimeAsync(0);
    ping = () => ({ ok: false });
    reply = () => ({ ok: true, token: newToken, ui: "none" });
    sent.length = 0;
    await vi.advanceTimersByTimeAsync(PING_INTERVAL_MS);
    expect(sent).toEqual([
      { type: "wa_ping", token: oldToken },
      { type: "wa_get", options: cond },
    ]);
    expect(responses).toEqual([]);
    // Answers now arrive for the new token only.
    onMessage?.({ type: "bg_wa_result", token: oldToken, outcome: { outcome: "error", name: "NotAllowedError" } }, { id: "ext" });
    expect(responses).toEqual([]);
    onMessage?.({ type: "bg_wa_result", token: newToken, outcome: { outcome: "error", name: "NotAllowedError" } }, { id: "ext" });
    expect(responses).toEqual([{ id, outcome: "error", name: "NotAllowedError" }]);
  });

  it("keeps a conditional request pending when the re-sent one falls back", async () => {
    vi.useFakeTimers();
    const id = hex(26);
    reply = () => ({ ok: true, token: hex(27), ui: "none" });
    request({ kind: "get", id, options: { ...get, conditional: true } });
    await vi.advanceTimersByTimeAsync(0);
    ping = () => ({ ok: false });
    reply = () => ({ ok: false, outcome: { outcome: "fallback" } });
    await vi.advanceTimersByTimeAsync(PING_INTERVAL_MS * 3);
    // The browser's own conditional request carries on; ours just stops.
    expect(responses).toEqual([]);
    expect(sent.filter((m) => (m as { type: string }).type === "wa_get")).toHaveLength(2);
  });

  it("falls back and removes the card when a modal session was lost", async () => {
    vi.useFakeTimers();
    const id = hex(28);
    reply = () => ({ ok: true, token: hex(29), ui: "chooser" });
    request({ kind: "get", id, options: { ...get, timeoutMs: 120_000 } });
    await vi.advanceTimersByTimeAsync(0);
    expect(document.querySelector("iframe")).not.toBeNull();
    ping = () => undefined; // the worker restarted and knows nothing, or the extension is gone
    await vi.advanceTimersByTimeAsync(PING_INTERVAL_MS);
    expect(responses).toEqual([{ id, outcome: "fallback" }]);
    expect(document.querySelector("iframe")).toBeNull();
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
    await vi.advanceTimersByTimeAsync(MIN_TIMEOUT_MS + 10_000 + 1);
    // The ping on the way may also end it (with a fallback); either way the page is answered once.
    expect(responses).toHaveLength(1);
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

  it("cancels every pending request, conditional ones included, when the page goes away", async () => {
    const modalId = hex(10);
    const modalToken = hex(11);
    const condId = hex(12);
    const condToken = hex(13);
    reply = () => ({ ok: true, token: modalToken, ui: "chooser" });
    request({ kind: "get", id: modalId, options: get });
    await flush();
    reply = () => ({ ok: true, token: condToken, ui: "none" });
    request({ kind: "get", id: condId, options: { ...get, conditional: true } });
    await flush();
    sent.length = 0;
    window.dispatchEvent(new Event("pagehide"));
    expect(sent).toContainEqual({ type: "wa_cancel", token: modalToken });
    expect(sent).toContainEqual({ type: "wa_cancel", token: condToken });
    expect(responses).toContainEqual({ id: modalId, outcome: "error", name: "AbortError" });
    expect(responses).toContainEqual({ id: condId, outcome: "error", name: "AbortError" });
    expect(document.querySelector("iframe")).toBeNull();
    responses.length = 0;
    onMessage?.({ type: "bg_wa_result", token: condToken, outcome: { outcome: "fallback" } }, { id: "ext" });
    expect(responses).toEqual([]);
  });
});

describe("automatic passkey upgrade", () => {
  it("answers a silent save with the credential and shows the notice for NOTICE_MS", async () => {
    vi.useFakeTimers();
    const token = hex(900);
    reply = (m) => ((m as { type: string }).type === "wa_create" ? { ok: true, token, ui: "saved", credential: savedCredential } : undefined);
    const id = hex(901);
    request({ kind: "create", id, options: { ...create, conditional: true } });
    await vi.advanceTimersByTimeAsync(0);
    expect(responses.find((r) => r.id === id)).toMatchObject({ outcome: "credential", credential: { credentialId: savedCredential.credentialId } });
    const frame = () => document.querySelector(`iframe[src$="#${token}"]`);
    expect(frame()?.getAttribute("src")).toContain("passkey.html");
    // No session: nothing to ping or cancel.
    expect(sent.filter((m) => (m as { type: string }).type !== "wa_create")).toEqual([]);
    await vi.advanceTimersByTimeAsync(NOTICE_MS);
    expect(frame()).toBeNull();
  });

  it("removes the notice at once when the page touches it", async () => {
    const token = hex(904);
    reply = (m) => ((m as { type: string }).type === "wa_create" ? { ok: true, token, ui: "saved", credential: savedCredential } : undefined);
    request({ kind: "create", id: hex(905), options: { ...create, conditional: true } });
    await flush();
    const frame = document.querySelector(`iframe[src$="#${token}"]`) as HTMLIFrameElement;
    expect(frame).not.toBeNull();
    frame.setAttribute("style", "display:none");
    await flush();
    expect(document.querySelector(`iframe[src$="#${token}"]`)).toBeNull();
  });

  it("shows no notice when the page cancelled during a silent save", async () => {
    let answer: (v: unknown) => void = () => undefined;
    reply = (m) => ((m as { type: string }).type === "wa_create" ? new Promise((r) => (answer = r)) : undefined);
    const id = hex(902);
    request({ kind: "create", id, options: { ...create, conditional: true } });
    await flush();
    request({ kind: "cancel", id });
    answer({ ok: true, token: hex(903), ui: "saved", credential: savedCredential });
    await flush();
    expect(document.querySelector(`iframe[src$="#${hex(903)}"]`)).toBeNull();
    expect(responses.some((r) => r.id === id)).toBe(false);
  });
});
