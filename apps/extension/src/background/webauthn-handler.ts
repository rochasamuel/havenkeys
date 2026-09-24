// Background side of passkeys: one session per tab, from the bridge's
// request to the credential, fallback or error it ends with.
//
// Kept free of `chrome.*` so it can be tested with a fake native client.
//
// Trust model (same as inline-handler.ts):
// * `FrameRef`s come from the browser's sender data. The rpId defaults to
//   the frame's host; whatever the page sends, the desktop checks it
//   against the frame URL (`authorize_rp`).
// * A session binds a random token to one tab and frame. The passkey frame
//   is accepted only from that tab, and only for passkeys or logins the
//   session offered; the desktop re-checks everything regardless.
// * Nothing is signed or created without a pick/save from the frame.
// * Nothing is persisted.

import type { PasskeyCandidate, Request, RequestType, ResultFor } from "@havenkeys/protocol";
import type { InlineReply } from "../messaging/inline";
import { BridgeError } from "../messaging/native";
import { displayHost } from "../shared/url";
import {
  MAX_TIMEOUT_MS,
  type CreateOptions,
  type GetOptions,
  type Outcome,
  type PasskeyRow,
  type PkRequest,
  type PkView,
  type WaReply,
  type WaRequest,
} from "../webauthn/messages";
import type { FrameRef, InlineDeps } from "./inline-handler";

export const SESSION_TTL_MS = MAX_TIMEOUT_MS;
export const CONDITIONAL_TTL_MS = 30 * 60_000;
const ES256 = -7;

type Client = {
  request<T extends RequestType>(r: Extract<Request, { type: T }>): Promise<ResultFor<T>>;
};

/**
 * `locked`: the vault was locked when the request came in; the card shows
 * "unlock" until `refreshLocked()` re-runs the lookup after an unlock.
 * `busy`: a sign or save is in flight; a second one is refused.
 */
type Session =
  | { kind: "get"; token: string; frame: FrameRef; rpId: string; options: GetOptions; locked: boolean; busy: boolean; passkeys: PasskeyRow[]; timer: ReturnType<typeof setTimeout> }
  | { kind: "create"; token: string; frame: FrameRef; rpId: string; options: CreateOptions; locked: boolean; busy: boolean; candidates: PasskeyCandidate[]; timer: ReturnType<typeof setTimeout> };

const FALLBACK: WaReply = { ok: false, outcome: { outcome: "fallback" } };
const BUSY = { ok: false as const, message: "Please wait…" };

function fail(e: unknown): { ok: false; message: string } {
  return { ok: false, message: e instanceof BridgeError ? e.message : "Something went wrong." };
}

export function createWebAuthnHandler(deps: InlineDeps & { client: Client }) {
  const sessions = new Map<number, Session>(); // by tab

  const frameFields = (f: FrameRef) => (f.topUrl === undefined ? { url: f.url } : { url: f.url, topUrl: f.topUrl });
  const rpIdFor = (f: FrameRef, rpId: string | null) => rpId ?? new URL(f.url).hostname;
  // Firefox senders carry no documentId, so a navigated frame would look the
  // same; the origin check keeps one site's session away from the next site.
  const sameFrame = (a: FrameRef, b: FrameRef) =>
    a.tabId === b.tabId && a.frameId === b.frameId && a.documentId === b.documentId && a.origin === b.origin;

  /** End a session and tell its bridge. A missing session is a no-op. */
  function finish(tabId: number, token: string, outcome: Outcome): void {
    const s = sessions.get(tabId);
    if (!s || s.token !== token) return;
    sessions.delete(tabId);
    clearTimeout(s.timer);
    void deps.sendToFrame(s.frame, { type: "bg_wa_result", token, outcome });
  }

  function drop(tabId: number): void {
    const s = sessions.get(tabId);
    if (!s) return;
    sessions.delete(tabId);
    clearTimeout(s.timer);
  }

  function open(tabId: number, make: (token: string, timer: ReturnType<typeof setTimeout>) => Session, ttl: number): string {
    const old = sessions.get(tabId);
    if (old) finish(tabId, old.token, { outcome: "error", name: "AbortError" });
    const token = deps.newToken();
    const timer = setTimeout(() => finish(tabId, token, { outcome: "error", name: "NotAllowedError" }), ttl);
    sessions.set(tabId, make(token, timer));
    return token;
  }

  function live(tabId: number, token: string): Session | null {
    const s = sessions.get(tabId);
    return s && s.token === token ? s : null;
  }

  async function beginGet(frame: FrameRef, options: GetOptions): Promise<WaReply> {
    const rpId = rpIdFor(frame, options.rpId);
    let passkeys: PasskeyRow[] = [];
    let locked = false;
    try {
      passkeys = (await deps.client.request({ type: "find_passkeys", ...frameFields(frame), rpId, allowCredentials: options.allowCredentials })).passkeys;
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked" && !options.conditional) locked = true;
      else if (e instanceof BridgeError && e.code === "denied") return { ok: false, outcome: { outcome: "error", name: "SecurityError" } };
      else return FALLBACK;
    }
    if (!locked && passkeys.length === 0) return FALLBACK;
    const ttl = options.conditional ? CONDITIONAL_TTL_MS : Math.min(options.timeoutMs ?? SESSION_TTL_MS, SESSION_TTL_MS);
    const token = open(frame.tabId, (token, timer) => ({ kind: "get", token, frame, rpId, options, locked, busy: false, passkeys, timer }), ttl);
    return { ok: true, token, ui: options.conditional ? "none" : "chooser" };
  }

  async function beginCreate(frame: FrameRef, options: CreateOptions): Promise<WaReply> {
    if (!options.algs.includes(ES256)) return FALLBACK;
    const rpId = rpIdFor(frame, options.rpId);
    let candidates: PasskeyCandidate[] = [];
    let locked = false;
    try {
      const check = await deps.client.request({
        type: "check_passkey_create",
        ...frameFields(frame),
        rpId,
        userName: options.userName,
        excludeCredentials: options.excludeCredentials,
      });
      if (check.excluded) return { ok: false, outcome: { outcome: "error", name: "InvalidStateError" } };
      candidates = check.candidates;
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked") locked = true;
      else if (e instanceof BridgeError && e.code === "denied") return { ok: false, outcome: { outcome: "error", name: "SecurityError" } };
      else return FALLBACK;
    }
    const ttl = Math.min(options.timeoutMs ?? SESSION_TTL_MS, SESSION_TTL_MS);
    const token = open(frame.tabId, (token, timer) => ({ kind: "create", token, frame, rpId, options, locked, busy: false, candidates, timer }), ttl);
    return { ok: true, token, ui: "create" };
  }

  async function sign(s: Extract<Session, { kind: "get" }>, itemId: string, credentialId: string): Promise<InlineReply<null>> {
    if (s.locked || !s.passkeys.some((p) => p.itemId === itemId && p.credentialId === credentialId)) {
      return { ok: false, message: "Unknown passkey." };
    }
    if (s.busy) return BUSY;
    s.busy = true;
    try {
      const r = await deps.client.request({
        type: "passkey_get",
        itemId,
        credentialId,
        ...frameFields(s.frame),
        rpId: s.rpId,
        challenge: s.options.challenge,
      });
      finish(s.frame.tabId, s.token, {
        outcome: "credential",
        credential: {
          type: "get",
          credentialId: r.credentialId,
          clientDataJson: r.clientDataJson,
          authenticatorData: r.authenticatorData,
          signature: r.signature,
          userHandle: r.userHandle,
        },
      });
      return { ok: true, value: null };
    } catch (e) {
      return fail(e);
    } finally {
      s.busy = false;
    }
  }

  async function save(s: Extract<Session, { kind: "create" }>, itemId: string | null): Promise<InlineReply<null>> {
    if (s.locked || (itemId !== null && !s.candidates.some((c) => c.itemId === itemId))) {
      return { ok: false, message: "Unknown login." };
    }
    if (s.busy) return BUSY;
    s.busy = true;
    try {
      const r = await deps.client.request({
        type: "passkey_create",
        ...frameFields(s.frame),
        rpId: s.rpId,
        challenge: s.options.challenge,
        userHandle: s.options.userId,
        userName: s.options.userName,
        displayName: s.options.userDisplayName,
        itemId,
      });
      // The page may have aborted meanwhile; finish() is then a no-op and
      // the passkey stays in the vault, visible in the desktop app.
      finish(s.frame.tabId, s.token, {
        outcome: "credential",
        credential: {
          type: "create",
          credentialId: r.credentialId,
          clientDataJson: r.clientDataJson,
          attestationObject: r.attestationObject,
          authenticatorData: r.authenticatorData,
          publicKey: r.publicKey,
          publicKeyAlgorithm: r.publicKeyAlgorithm,
        },
      });
      return { ok: true, value: null };
    } catch (e) {
      return fail(e);
    } finally {
      s.busy = false;
    }
  }

  async function handleContent(frame: FrameRef, req: WaRequest): Promise<WaReply | Record<string, never>> {
    switch (req.type) {
      case "wa_get":
        return beginGet(frame, req.options);
      case "wa_create":
        return beginCreate(frame, req.options);
      case "wa_cancel": {
        const s = sessions.get(frame.tabId);
        if (s && s.token === req.token && sameFrame(s.frame, frame)) drop(frame.tabId);
        return {};
      }
    }
  }

  async function handleFrame(tabId: number, req: PkRequest): Promise<InlineReply<PkView | null>> {
    const s = live(tabId, req.token);
    if (!s) return { ok: false, message: "This prompt has expired." };
    const site = displayHost(s.frame.url) ?? "";
    switch (req.type) {
      case "pk_state":
        if (s.locked) return { ok: true, value: { state: "locked", site } };
        return s.kind === "get"
          ? { ok: true, value: { state: "chooser", site, passkeys: s.passkeys } }
          : { ok: true, value: { state: "create", site, userName: s.options.userName, candidates: s.candidates } };
      case "pk_pick":
        return s.kind === "get" ? sign(s, req.itemId, req.credentialId) : { ok: false, message: "Unknown passkey." };
      case "pk_save":
        return s.kind === "create" ? save(s, req.itemId) : { ok: false, message: "Unknown login." };
      case "pk_fallback":
        finish(tabId, s.token, { outcome: "fallback" });
        return { ok: true, value: null };
      case "pk_cancel":
        finish(tabId, s.token, { outcome: "error", name: "NotAllowedError" });
        return { ok: true, value: null };
    }
  }

  /** Passkeys a conditional request in this very frame is waiting on. */
  function conditionalFor(frame: FrameRef): PasskeyRow[] {
    const s = sessions.get(frame.tabId);
    if (!s || s.kind !== "get" || !s.options.conditional || !sameFrame(s.frame, frame)) return [];
    return s.passkeys;
  }

  async function pickConditional(frame: FrameRef, itemId: string, credentialId: string): Promise<InlineReply<null>> {
    const s = sessions.get(frame.tabId);
    if (!s || s.kind !== "get" || !s.options.conditional || !sameFrame(s.frame, frame)) {
      return { ok: false, message: "This menu has expired." };
    }
    return sign(s, itemId, credentialId);
  }

  /**
   * The vault was unlocked: sessions that opened while it was locked look
   * their passkeys (or save targets) up again, with the same frame, rpId and
   * options they were opened with. Whatever the lookup ends in besides a
   * usable list finishes the session like the initial request would have.
   */
  async function refreshLocked(): Promise<void> {
    await Promise.all([...sessions.values()].filter((s) => s.locked).map(refresh));
  }

  async function refresh(s: Session): Promise<void> {
    const tabId = s.frame.tabId;
    const current = () => sessions.get(tabId) === s;
    const failed = (e: unknown) => {
      if (!current()) return;
      // Locked again before the lookup ran: keep waiting for the next unlock.
      if (e instanceof BridgeError && e.code === "locked") return;
      if (e instanceof BridgeError && e.code === "denied") finish(tabId, s.token, { outcome: "error", name: "SecurityError" });
      else finish(tabId, s.token, { outcome: "fallback" });
    };
    if (s.kind === "get") {
      try {
        const r = await deps.client.request({ type: "find_passkeys", ...frameFields(s.frame), rpId: s.rpId, allowCredentials: s.options.allowCredentials });
        if (!current()) return;
        if (r.passkeys.length === 0) return finish(tabId, s.token, { outcome: "fallback" });
        s.passkeys = r.passkeys;
        s.locked = false;
      } catch (e) {
        failed(e);
      }
      return;
    }
    try {
      const r = await deps.client.request({
        type: "check_passkey_create",
        ...frameFields(s.frame),
        rpId: s.rpId,
        userName: s.options.userName,
        excludeCredentials: s.options.excludeCredentials,
      });
      if (!current()) return;
      if (r.excluded) return finish(tabId, s.token, { outcome: "error", name: "InvalidStateError" });
      s.candidates = r.candidates;
      s.locked = false;
    } catch (e) {
      failed(e);
    }
  }

  /** Vault locked or desktop gone: every waiting page gets a refusal. */
  function reset(): void {
    for (const [tabId, s] of [...sessions]) {
      if (s.kind === "get" && s.options.conditional) drop(tabId);
      else finish(tabId, s.token, { outcome: "error", name: "NotAllowedError" });
    }
  }

  function forgetTab(tabId: number): void {
    drop(tabId);
  }

  return { handleContent, handleFrame, conditionalFor, pickConditional, refreshLocked, reset, forgetTab };
}

export type WebAuthnHandler = ReturnType<typeof createWebAuthnHandler>;
