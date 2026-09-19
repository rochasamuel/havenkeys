import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BridgeEvent } from "@havenkeys/protocol";
import { BridgeError, NativeClient, type NativePort } from "./native";

class FakePort implements NativePort {
  sent: unknown[] = [];
  disconnected = false;
  #msg: ((m: unknown) => void)[] = [];
  #disc: (() => void)[] = [];
  onMessage = { addListener: (cb: (m: unknown) => void) => void this.#msg.push(cb) };
  onDisconnect = { addListener: (cb: () => void) => void this.#disc.push(cb) };
  postMessage(m: unknown) {
    this.sent.push(m);
  }
  disconnect() {
    this.disconnected = true;
  }
  deliver(m: unknown) {
    for (const cb of this.#msg) cb(m);
  }
  drop() {
    for (const cb of this.#disc) cb();
  }
  lastId(): number {
    return (this.sent.at(-1) as { id: number }).id;
  }
}

let port: FakePort;
let connects: number;
let events: BridgeEvent[];

function client(opts: { timeoutMs?: number; idleMs?: number } = {}) {
  return new NativeClient(
    () => {
      connects++;
      port = new FakePort();
      return port;
    },
    { ...opts, onEvent: (e) => events.push(e) },
  );
}

beforeEach(() => {
  connects = 0;
  events = [];
});
afterEach(() => {
  vi.useRealTimers();
});

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

describe("NativeClient", () => {
  it("sends versioned envelopes and resolves by id", async () => {
    const c = client();
    const p = c.request({ type: "status" });
    expect(port.sent[0]).toEqual({ v: 1, id: 1, request: { type: "status" } });
    port.deliver({ v: 1, id: 1, result: { type: "status", state: "unlocked", vaultExists: true } });
    await expect(p).resolves.toEqual({ type: "status", state: "unlocked", vaultExists: true });
  });

  it("maps protocol errors to BridgeError", async () => {
    const c = client();
    const p = c.request({ type: "fill_item", itemId: ID, url: "https://evil.com/" });
    port.deliver({ v: 1, id: port.lastId(), error: { code: "denied", message: "This item is not saved for this website." } });
    await expect(p).rejects.toMatchObject({ code: "denied" });
  });

  it("ignores invalid, unknown-id and mismatched messages", async () => {
    vi.useFakeTimers();
    const c = client({ timeoutMs: 1000 });
    const p = c.request({ type: "find_matches", url: "https://github.com/" });
    const id = port.lastId();
    // Invalid shapes are dropped silently.
    port.deliver({ v: 1, id, result: { type: "find_matches", matches: [{ id: ID, password: "x" }] } });
    port.deliver("garbage");
    // Unknown IDs are dropped.
    port.deliver({ v: 1, id: id + 100, result: { type: "find_matches", matches: [] } });
    // A result of the wrong type fails the request.
    port.deliver({ v: 1, id, result: { type: "fill_item", username: "u", password: "p" } });
    await expect(p).rejects.toMatchObject({ code: "malformed" });
  });

  it("times out", async () => {
    vi.useFakeTimers();
    const c = client({ timeoutMs: 500 });
    const p = c.request({ type: "status" });
    vi.advanceTimersByTime(501);
    await expect(p).rejects.toMatchObject({ code: "timeout" });
  });

  it("fails pending requests when the host disappears, then reconnects", async () => {
    const c = client();
    const p = c.request({ type: "status" });
    port.drop();
    await expect(p).rejects.toMatchObject({ code: "host_unavailable" });
    const p2 = c.request({ type: "lock" });
    expect(connects).toBe(2);
    port.deliver({ v: 1, id: port.lastId(), result: { type: "lock" } });
    await expect(p2).resolves.toEqual({ type: "lock" });
  });

  it("reports connect failure as host_unavailable", async () => {
    const c = new NativeClient(() => {
      throw new Error("Specified native messaging host not found.");
    });
    await expect(c.request({ type: "status" })).rejects.toBeInstanceOf(BridgeError);
  });

  it("forwards events and fails pending on desktop disconnect", async () => {
    const c = client();
    const p = c.request({ type: "status" });
    port.deliver({ v: 1, event: { type: "locked" } });
    port.deliver({ v: 1, event: { type: "disconnected" } });
    await expect(p).rejects.toMatchObject({ code: "desktop_unavailable" });
    expect(events).toEqual([{ type: "locked" }, { type: "disconnected" }]);
  });

  it("closes the port when idle", async () => {
    vi.useFakeTimers();
    const c = client({ idleMs: 1000 });
    const p = c.request({ type: "status" });
    port.deliver({ v: 1, id: port.lastId(), result: { type: "status", state: "locked", vaultExists: true } });
    await p;
    vi.advanceTimersByTime(999);
    expect(port.disconnected).toBe(false);
    vi.advanceTimersByTime(2);
    expect(port.disconnected).toBe(true);
  });

  it("never reuses an id while a request is pending", () => {
    const c = client();
    void c.request({ type: "status" }).catch(() => {});
    void c.request({ type: "status" }).catch(() => {});
    const ids = port.sent.map((m) => (m as { id: number }).id);
    expect(new Set(ids).size).toBe(2);
  });
});
