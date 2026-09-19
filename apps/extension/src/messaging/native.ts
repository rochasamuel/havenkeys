// Client for the native messaging host.
//
// Owns the single native port, correlates responses to requests by ID,
// validates every incoming message with the protocol package, and times out
// requests the host never answers. It never logs, and it never keeps a
// secret beyond resolving the promise that asked for it.

import {
  envelope,
  parseIncoming,
  type BridgeEvent,
  type ErrorCode,
  type Request,
  type RequestType,
  type Result,
  type ResultFor,
} from "@havenkeys/protocol";

/** Protocol error codes plus the two the client produces itself. */
export type ClientErrorCode = ErrorCode | "host_unavailable" | "timeout";

export class BridgeError extends Error {
  readonly code: ClientErrorCode;

  constructor(code: ClientErrorCode, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}

const HOST_UNAVAILABLE = "The HavenKeys native messaging host is not installed or failed to start.";
const DESKTOP_UNAVAILABLE = "The HavenKeys app is not running.";
const TIMEOUT = "HavenKeys did not respond.";

/** The subset of `chrome.runtime.Port` the client uses. */
export interface NativePort {
  postMessage(message: unknown): void;
  disconnect(): void;
  onMessage: { addListener(cb: (message: unknown) => void): void };
  onDisconnect: { addListener(cb: () => void): void };
}

export interface NativeClientOptions {
  /** Per-request timeout. */
  timeoutMs?: number;
  /** Close the port after this long without pending requests. */
  idleMs?: number;
  onEvent?: (event: BridgeEvent) => void;
}

interface Pending {
  type: RequestType;
  resolve: (result: Result) => void;
  reject: (error: BridgeError) => void;
  timer: ReturnType<typeof setTimeout>;
}

type RequestOf<T extends RequestType> = Extract<Request, { type: T }>;

export class NativeClient {
  readonly #connect: () => NativePort;
  readonly #timeoutMs: number;
  readonly #idleMs: number;
  readonly #onEvent: (event: BridgeEvent) => void;
  #port: NativePort | null = null;
  #pending = new Map<number, Pending>();
  #nextId = 1;
  #idleTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(connect: () => NativePort, options: NativeClientOptions = {}) {
    this.#connect = connect;
    this.#timeoutMs = options.timeoutMs ?? 10_000;
    this.#idleMs = options.idleMs ?? 60_000;
    this.#onEvent = options.onEvent ?? (() => {});
  }

  request<T extends RequestType>(request: RequestOf<T>): Promise<ResultFor<T>> {
    this.#cancelIdle();
    return new Promise<ResultFor<T>>((resolve, reject) => {
      let port: NativePort;
      try {
        port = this.#ensurePort();
      } catch {
        reject(new BridgeError("host_unavailable", HOST_UNAVAILABLE));
        return;
      }
      const id = this.#takeId();
      const timer = setTimeout(() => this.#settle(id, new BridgeError("timeout", TIMEOUT)), this.#timeoutMs);
      this.#pending.set(id, {
        type: request.type,
        resolve: resolve as (r: Result) => void,
        reject,
        timer,
      });
      try {
        port.postMessage(envelope(id, request));
      } catch {
        this.#settle(id, new BridgeError("host_unavailable", HOST_UNAVAILABLE));
      }
    });
  }

  /** Close the port and fail everything in flight. */
  close(): void {
    this.#cancelIdle();
    const port = this.#port;
    this.#port = null;
    this.#failAll(new BridgeError("host_unavailable", HOST_UNAVAILABLE));
    port?.disconnect();
  }

  #takeId(): number {
    const id = this.#nextId;
    this.#nextId = id >= 0xffff_ffff ? 1 : id + 1;
    return id;
  }

  #ensurePort(): NativePort {
    if (this.#port) return this.#port;
    const port = this.#connect();
    port.onMessage.addListener((msg) => {
      if (this.#port === port) this.#onMessage(msg);
    });
    port.onDisconnect.addListener(() => {
      if (this.#port !== port) return;
      this.#port = null;
      this.#failAll(new BridgeError("host_unavailable", HOST_UNAVAILABLE));
    });
    this.#port = port;
    return port;
  }

  #onMessage(raw: unknown): void {
    const msg = parseIncoming(raw);
    if (!msg) return; // Not a protocol message: ignore it entirely.
    switch (msg.kind) {
      case "event":
        if (msg.event.type === "disconnected") {
          this.#failAll(new BridgeError("desktop_unavailable", DESKTOP_UNAVAILABLE));
        }
        this.#onEvent(msg.event);
        return;
      case "error":
        // An error without an ID cannot be matched to a request; the
        // request will time out instead.
        if (msg.id !== null) this.#settle(msg.id, new BridgeError(msg.error.code, msg.error.message));
        return;
      case "result": {
        const pending = this.#pending.get(msg.id);
        if (!pending) return;
        if (msg.result.type !== pending.type) {
          this.#settle(msg.id, new BridgeError("malformed", "Unexpected response."));
          return;
        }
        this.#settle(msg.id, msg.result);
        return;
      }
    }
  }

  #settle(id: number, outcome: Result | BridgeError): void {
    const pending = this.#pending.get(id);
    if (!pending) return;
    this.#pending.delete(id);
    clearTimeout(pending.timer);
    if (outcome instanceof BridgeError) pending.reject(outcome);
    else pending.resolve(outcome);
    this.#scheduleIdle();
  }

  #failAll(error: BridgeError): void {
    for (const id of [...this.#pending.keys()]) this.#settle(id, error);
  }

  #scheduleIdle(): void {
    if (this.#pending.size > 0 || !this.#port) return;
    this.#cancelIdle();
    this.#idleTimer = setTimeout(() => {
      this.#idleTimer = null;
      if (this.#pending.size === 0) this.close();
    }, this.#idleMs);
  }

  #cancelIdle(): void {
    if (this.#idleTimer !== null) clearTimeout(this.#idleTimer);
    this.#idleTimer = null;
  }
}
