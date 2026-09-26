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
// * Nothing is signed or created without a pick/save from the frame, except
//   the site's automatic upgrade, which the desktop allows only after a
//   HavenKeys password fill of that login on that site within the last 5
//   minutes (`upgrade: auto`).
// * Whether HavenKeys holds a passkey the site excluded is shown to the user,
//   and told to the site only after the user closes that card.
// * Nothing is persisted.
//
// Sessions live in the worker's memory. An MV3 worker can be suspended and
// restarted, losing them; the bridge pings its session (`wa_ping`) to keep
// the worker awake and to notice a loss, and a card whose session is gone
// can still be closed (see `handleFrame`).

import type { PasskeyCandidate, Request, RequestType, ResultFor } from "@havenkeys/protocol";
import type { InlineReply } from "../messaging/inline";
import { BridgeError } from "../messaging/native";
import { displayHost } from "../shared/url";
import {
  clampTimeout,
  MAX_TIMEOUT_MS,
  NOTICE_MS,
  type BgWaResult,
  type CreatedCredential,
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
/** How long the "passkey saved" notice can still ask for its site. */
export const NOTICE_TTL_MS = NOTICE_MS + 10_000;
const ES256 = -7;

type Client = {
  request<T extends RequestType>(r: Extract<Request, { type: T }>): Promise<ResultFor<T>>;
};

/**
 * `locked`: the vault was locked when the request came in; the card shows
 * "unlock" until `refresh()` re-runs the lookup after an unlock.
 * `refreshing`: that lookup, while it runs, so it runs once at a time.
 * `busy`: a sign or save is in flight; a second one is refused.
 * `exists`: the site's excludeCredentials names a passkey we hold; the card
 * says so and nothing can be saved.
 * `upgradeItemId`: the login the desktop proposes for the site's automatic
 * upgrade ("Add a passkey?"); null for an ordinary create.
 */
type Common = {
  token: string;
  frame: FrameRef;
  rpId: string;
  locked: boolean;
  refreshing: Promise<void> | null;
  busy: boolean;
  timer: ReturnType<typeof setTimeout>;
};
type Session =
  | (Common & { kind: "get"; options: GetOptions; passkeys: PasskeyRow[] })
  | (Common & { kind: "create"; options: CreateOptions; candidates: PasskeyCandidate[]; exists: boolean; upgradeItemId: string | null });

const FALLBACK: WaReply = { ok: false, outcome: { outcome: "fallback" } };
const BUSY = { ok: false as const, message: "Please wait…" };

function fail(e: unknown): { ok: false; message: string } {
  return { ok: false, message: e instanceof BridgeError ? e.message : "Something went wrong." };
}

function created(r: ResultFor<"passkey_create">): CreatedCredential {
  return {
    type: "create",
    credentialId: r.credentialId,
    clientDataJson: r.clientDataJson,
    attestationObject: r.attestationObject,
    authenticatorData: r.authenticatorData,
    publicKey: r.publicKey,
    publicKeyAlgorithm: r.publicKeyAlgorithm,
  };
}

export interface WebAuthnDeps extends InlineDeps {
  client: Client;
  /** Send to every frame of a tab; only the bridge holding the token acts on it. */
  sendToTab(tabId: number, msg: BgWaResult): Promise<unknown>;
}

export function createWebAuthnHandler(deps: WebAuthnDeps) {
  const sessions = new Map<number, Session>(); // by tab
  // Tabs with a wa_get/wa_create lookup in flight: a page firing requests in
  // a loop must not drain the desktop's shared lookup budget.
  const looking = new Set<number>();
  // Tabs showing the "passkey saved" notice: the notice frame asks for its
  // site by token, after the silent save finished and no session is left.
  const notices = new Map<number, { token: string; site: string; timer: ReturnType<typeof setTimeout> }>();

  function dropNotice(tabId: number): void {
    const n = notices.get(tabId);
    if (!n) return;
    notices.delete(tabId);
    clearTimeout(n.timer);
  }

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
      // "denied" too: the browser applies the same rpId rule, and paths it
      // allows that we do not (related origins, permitted cross-site frames)
      // keep working.
      if (e instanceof BridgeError && e.code === "locked" && !options.conditional) locked = true;
      else return FALLBACK;
    }
    if (!locked && passkeys.length === 0) return FALLBACK;
    const ttl = options.conditional ? CONDITIONAL_TTL_MS : clampTimeout(options.timeoutMs);
    const token = open(frame.tabId, (token, timer) => ({ kind: "get", token, frame, rpId, options, locked, refreshing: null, busy: false, passkeys, timer }), ttl);
    return { ok: true, token, ui: options.conditional ? "none" : "chooser" };
  }

  /**
   * The site's automatic upgrade. The desktop decides (`upgrade`), from a
   * HavenKeys password fill on this site in the last few minutes; anything
   * but a usable decision falls back to the browser, silently.
   */
  async function beginUpgrade(frame: FrameRef, options: CreateOptions, rpId: string): Promise<WaReply> {
    let check: ResultFor<"check_passkey_create">;
    try {
      check = await deps.client.request({
        type: "check_passkey_create",
        ...frameFields(frame),
        rpId,
        userName: options.userName,
        excludeCredentials: options.excludeCredentials,
        conditional: true,
      });
    } catch {
      return FALLBACK;
    }
    const upgrade = check.upgrade;
    if (check.excluded || upgrade.kind === "none") return FALLBACK;
    if (upgrade.kind === "ask") {
      const candidates = check.candidates;
      const token = open(
        frame.tabId,
        (token, timer) => ({ kind: "create", token, frame, rpId, options, locked: false, refreshing: null, busy: false, candidates, exists: false, upgradeItemId: upgrade.itemId, timer }),
        clampTimeout(options.timeoutMs),
      );
      return { ok: true, token, ui: "create" };
    }
    let r: ResultFor<"passkey_create">;
    try {
      r = await deps.client.request({
        type: "passkey_create",
        ...frameFields(frame),
        rpId,
        challenge: options.challenge,
        userHandle: options.userId,
        userName: options.userName,
        displayName: options.userDisplayName,
        itemId: upgrade.itemId,
        conditional: true,
      });
    } catch {
      return FALLBACK;
    }
    dropNotice(frame.tabId);
    const token = deps.newToken();
    const timer = setTimeout(() => {
      if (notices.get(frame.tabId)?.token === token) notices.delete(frame.tabId);
    }, NOTICE_TTL_MS);
    notices.set(frame.tabId, { token, site: displayHost(frame.url) ?? "", timer });
    return { ok: true, token, ui: "saved", credential: created(r) };
  }

  async function beginCreate(frame: FrameRef, options: CreateOptions): Promise<WaReply> {
    if (!options.algs.includes(ES256)) return FALLBACK;
    const rpId = rpIdFor(frame, options.rpId);
    if (options.conditional) return beginUpgrade(frame, options, rpId);
    let candidates: PasskeyCandidate[] = [];
    let locked = false;
    let exists = false;
    try {
      const check = await deps.client.request({
        type: "check_passkey_create",
        ...frameFields(frame),
        rpId,
        userName: options.userName,
        excludeCredentials: options.excludeCredentials,
        conditional: false,
      });
      // Not answered yet: an immediate InvalidStateError would tell any
      // page, without a click, which accounts HavenKeys holds.
      exists = check.excluded;
      candidates = check.candidates;
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked") locked = true;
      else return FALLBACK;
    }
    const ttl = clampTimeout(options.timeoutMs);
    const token = open(frame.tabId, (token, timer) => ({ kind: "create", token, frame, rpId, options, locked, refreshing: null, busy: false, candidates, exists, upgradeItemId: null, timer }), ttl);
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
    if (s.locked || s.exists || (itemId !== null && !s.candidates.some((c) => c.itemId === itemId))) {
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
        // A click on the card, even for the site's automatic upgrade.
        conditional: false,
      });
      // The page may have aborted meanwhile; finish() is then a no-op and
      // the passkey stays in the vault, visible in the desktop app.
      finish(s.frame.tabId, s.token, { outcome: "credential", credential: created(r) });
      return { ok: true, value: null };
    } catch (e) {
      return fail(e);
    } finally {
      s.busy = false;
    }
  }

  /** One lookup per tab at a time; a second request meanwhile falls back. */
  async function lookup(tabId: number, begin: () => Promise<WaReply>): Promise<WaReply> {
    if (looking.has(tabId)) return FALLBACK;
    looking.add(tabId);
    try {
      return await begin();
    } finally {
      looking.delete(tabId);
    }
  }

  async function handleContent(frame: FrameRef, req: WaRequest): Promise<WaReply | { ok: boolean } | Record<string, never>> {
    switch (req.type) {
      case "wa_get":
        return lookup(frame.tabId, () => beginGet(frame, req.options));
      case "wa_create":
        return lookup(frame.tabId, () => beginCreate(frame, req.options));
      case "wa_cancel": {
        const s = sessions.get(frame.tabId);
        if (s && s.token === req.token && sameFrame(s.frame, frame)) drop(frame.tabId);
        return {};
      }
      case "wa_ping": {
        const s = sessions.get(frame.tabId);
        return { ok: !!s && s.token === req.token && sameFrame(s.frame, frame) };
      }
    }
  }

  function view(s: Session): PkView {
    const site = displayHost(s.frame.url) ?? "";
    if (s.locked) return { state: "locked", site };
    if (s.kind === "get") return { state: "chooser", site, passkeys: s.passkeys };
    if (s.exists) return { state: "exists", site };
    return { state: "create", site, userName: s.options.userName, candidates: s.candidates, upgradeItemId: s.upgradeItemId };
  }

  async function handleFrame(tabId: number, req: PkRequest): Promise<InlineReply<PkView | null>> {
    const s = live(tabId, req.token);
    if (!s) {
      // The "passkey saved" notice has a fixed size and no session to relay through.
      if (req.type === "pk_resize") return { ok: true, value: null };
      const n = notices.get(tabId);
      if (req.type === "pk_state" && n && n.token === req.token) return { ok: true, value: { state: "saved", site: n.site } };
      // The session is gone (worker restarted, or already finished) but the
      // card is still up. Closing it must still work: the answer goes to
      // every frame of the card's tab, and only the bridge that holds this
      // token (known to it and to the card) acts on it. A finished request
      // is no longer pending there, so this is a no-op for it.
      if (req.type === "pk_cancel" || req.type === "pk_close" || req.type === "pk_fallback") {
        const outcome: Outcome = req.type === "pk_fallback" ? { outcome: "fallback" } : { outcome: "error", name: "NotAllowedError" };
        void deps.sendToTab(tabId, { type: "bg_wa_result", token: req.token, outcome });
        return { ok: true, value: null };
      }
      return { ok: false, message: "This prompt has expired." };
    }
    switch (req.type) {
      case "pk_state":
        // The "unlocked" event may never come (the native port closes when
        // idle), so the card's own polling re-runs the lookup.
        if (s.locked) {
          await refresh(s);
          if (!live(tabId, req.token)) return { ok: false, message: "This prompt has expired." };
        }
        return { ok: true, value: view(s) };
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
      case "pk_resize":
        // Only to the frame whose bridge shows this session's card.
        void deps.sendToFrame(s.frame, { type: "bg_wa_resize", token: s.token, height: req.height });
        return { ok: true, value: null };
      case "pk_close":
        if (s.kind !== "create" || !s.exists || s.locked) return { ok: false, message: "Unknown request." };
        finish(tabId, s.token, { outcome: "error", name: "InvalidStateError" });
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

  /** Re-run a locked session's lookup; one at a time per session. */
  function refresh(s: Session): Promise<void> {
    s.refreshing ??= lookAgain(s).finally(() => (s.refreshing = null));
    return s.refreshing;
  }

  async function lookAgain(s: Session): Promise<void> {
    const tabId = s.frame.tabId;
    const current = () => sessions.get(tabId) === s;
    const failed = (e: unknown) => {
      if (!current()) return;
      // Locked again before the lookup ran: keep waiting for the next unlock.
      if (e instanceof BridgeError && e.code === "locked") return;
      finish(tabId, s.token, { outcome: "fallback" });
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
        conditional: false,
      });
      if (!current()) return;
      s.exists = r.excluded;
      s.candidates = r.candidates;
      s.locked = false;
    } catch (e) {
      failed(e);
    }
  }

  /**
   * Vault locked or desktop gone: every waiting page gets a refusal, except
   * the "Add a passkey?" card, which hands the site's upgrade back to the
   * browser (spec §7).
   */
  function reset(): void {
    for (const [tabId, s] of [...sessions]) {
      if (s.kind === "get" && s.options.conditional) drop(tabId);
      else if (s.kind === "create" && s.upgradeItemId !== null) finish(tabId, s.token, { outcome: "fallback" });
      else finish(tabId, s.token, { outcome: "error", name: "NotAllowedError" });
    }
  }

  function forgetTab(tabId: number): void {
    drop(tabId);
    dropNotice(tabId);
  }

  return { handleContent, handleFrame, conditionalFor, pickConditional, refreshLocked, reset, forgetTab };
}

export type WebAuthnHandler = ReturnType<typeof createWebAuthnHandler>;
