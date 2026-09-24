import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import type { CreateOptions, GetOptions } from "../webauthn/messages";
import type { FrameRef } from "./inline-handler";
import { createWebAuthnHandler, NOTICE_TTL_MS } from "./webauthn-handler";

const ITEM = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";
const T1 = "0".repeat(31) + "1";
const getOpts: GetOptions = { rpId: null, challenge: "AQ", allowCredentials: [], conditional: false, timeoutMs: null };
const createOpts: CreateOptions = { rpId: "github.com", challenge: "AQ", userId: "AQ", userName: "octo", userDisplayName: null, algs: [-7], excludeCredentials: [], timeoutMs: null, conditional: false };
const match = { itemId: ITEM, credentialId: CRED, title: "GitHub", userName: "octo" };
const signed = { type: "passkey_get", credentialId: CRED, authenticatorData: "AA", clientDataJson: "e30", signature: "MEU", userHandle: "AQ" };
const created = { type: "passkey_create", credentialId: CRED, attestationObject: "oA", clientDataJson: "e30", authenticatorData: "AA", publicKey: "MA", publicKeyAlgorithm: -7 };

function frame(over: Partial<FrameRef> = {}): FrameRef {
  return { tabId: 1, frameId: 0, url: "https://github.com/login", origin: "https://github.com", ...over };
}

function setup(answer: (r: Request) => unknown) {
  const requests: Request[] = [];
  const sent: unknown[] = [];
  const broadcast: Array<{ tabId: number; msg: unknown }> = [];
  let n = 0;
  let clock = 0;
  const h = createWebAuthnHandler({
    client: { request: (async (r: Request) => (requests.push(r), answer(r))) as never },
    sendToFrame: async (_to, msg) => void sent.push(msg),
    sendToTab: async (tabId, msg) => void broadcast.push({ tabId, msg }),
    now: () => clock,
    newToken: () => (++n).toString(16).padStart(32, "0"),
  });
  return { h, requests, sent, broadcast, advance: (ms: number) => (clock += ms) };
}

const defaults = (r: Request): unknown => {
  switch (r.type) {
    case "find_passkeys":
      return { type: "find_passkeys", passkeys: [match] };
    case "passkey_get":
      return signed;
    case "check_passkey_create":
      return { type: "check_passkey_create", excluded: false, candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }], upgrade: { kind: "none" } };
    case "passkey_create":
      return created;
    default:
      throw new Error(r.type);
  }
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("sign in", () => {
  it("offers matches, signs only after a pick, and never trusts the page for the URL", async () => {
    const { h, requests, sent } = setup(defaults);
    const reply = await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    expect(reply).toEqual({ ok: true, token: T1, ui: "chooser" });
    expect(requests[0]).toEqual({ type: "find_passkeys", url: "https://github.com/login", rpId: "github.com", allowCredentials: [] });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({
      ok: true,
      value: { state: "chooser", site: "github.com", passkeys: [match] },
    });
    expect(requests.filter((r) => r.type === "passkey_get")).toEqual([]);
    expect(await h.handleFrame(1, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: CRED })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toMatchObject({ type: "passkey_get", itemId: ITEM, credentialId: CRED, url: "https://github.com/login", rpId: "github.com" });
    expect(sent.at(-1)).toMatchObject({ type: "bg_wa_result", token: T1, outcome: { outcome: "credential", credential: { type: "get", credentialId: CRED } } });
  });

  it("refuses picks that were not offered, from other tabs, or with stale tokens", async () => {
    const { h, requests } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    expect((await h.handleFrame(1, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: "AgICAgICAgICAgICAgICAg" })).ok).toBe(false);
    expect((await h.handleFrame(2, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: CRED })).ok).toBe(false);
    expect(requests.some((r) => r.type === "passkey_get")).toBe(false);
  });

  it("falls back silently when nothing matches or the desktop is gone", async () => {
    expect(await setup(() => ({ type: "find_passkeys", passkeys: [] })).h.handleContent(frame(), { type: "wa_get", options: getOpts })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
    const gone = setup(() => {
      throw new BridgeError("desktop_unavailable", "x");
    });
    expect(await gone.h.handleContent(frame(), { type: "wa_get", options: getOpts })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
  });

  it("shows the locked state for a modal request, but not for conditional ones", async () => {
    const locked = () => {
      throw new BridgeError("locked", "x");
    };
    const { h } = setup(locked);
    expect(await h.handleContent(frame(), { type: "wa_get", options: getOpts })).toMatchObject({ ok: true, ui: "chooser" });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "locked", site: "github.com" } });
    expect(await setup(locked).h.handleContent(frame(), { type: "wa_get", options: { ...getOpts, conditional: true } })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
  });

  it("finishes every session when the vault locks", async () => {
    const { h, sent } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    h.reset();
    expect(sent.at(-1)).toEqual({ type: "bg_wa_result", token: T1, outcome: { outcome: "error", name: "NotAllowedError" } });
    expect((await h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(false);
  });

  it("times out", async () => {
    const { h, sent } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_get", options: { ...getOpts, timeoutMs: 60_000 } });
    await vi.advanceTimersByTimeAsync(60_001);
    expect(sent.at(-1)).toMatchObject({ outcome: { outcome: "error", name: "NotAllowedError" } });
  });

  it("gives the user at least 10 seconds, whatever the site's timeout", async () => {
    for (const timeoutMs of [0, 1000]) {
      const { h, sent } = setup(defaults);
      await h.handleContent(frame(), { type: "wa_get", options: { ...getOpts, timeoutMs } });
      await vi.advanceTimersByTimeAsync(9_999);
      expect(sent).toEqual([]);
      expect((await h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(true);
      await vi.advanceTimersByTimeAsync(2);
      expect(sent.at(-1)).toMatchObject({ outcome: { outcome: "error", name: "NotAllowedError" } });
    }
  });

  it("falls back instead of raising SecurityError when the desktop denies the rpId", async () => {
    const denied = () => {
      throw new BridgeError("denied", "x");
    };
    expect(await setup(denied).h.handleContent(frame(), { type: "wa_get", options: getOpts })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
    expect(await setup(denied).h.handleContent(frame(), { type: "wa_create", options: createOpts })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
  });
});

describe("create", () => {
  it("asks first, creates on save", async () => {
    const { h, requests, sent } = setup(defaults);
    expect(await h.handleContent(frame(), { type: "wa_create", options: createOpts })).toEqual({ ok: true, token: T1, ui: "create" });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({
      ok: true,
      value: { state: "create", site: "github.com", userName: "octo", candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }], upgradeItemId: null },
    });
    expect(requests.some((r) => r.type === "passkey_create")).toBe(false);
    await h.handleFrame(1, { type: "pk_save", token: T1, itemId: ITEM });
    expect(requests.at(-1)).toEqual({
      type: "passkey_create",
      url: "https://github.com/login",
      rpId: "github.com",
      challenge: "AQ",
      userHandle: "AQ",
      userName: "octo",
      displayName: null,
      itemId: ITEM,
      conditional: false,
    });
    expect(sent.at(-1)).toMatchObject({ outcome: { outcome: "credential", credential: { type: "create", publicKeyAlgorithm: -7 } } });
  });

  it("refuses an item that was not offered", async () => {
    const { h, requests } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    expect((await h.handleFrame(1, { type: "pk_save", token: T1, itemId: "11111111-2222-4333-8444-555555555555" })).ok).toBe(false);
    expect(requests.some((r) => r.type === "passkey_create")).toBe(false);
  });

  it("falls back without ES256", async () => {
    expect(await setup(defaults).h.handleContent(frame(), { type: "wa_create", options: { ...createOpts, algs: [-257] } })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
  });

  it("reports an excluded credential only after the user closes the card", async () => {
    const excluded = (r: Request) => (r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "none" } } : defaults(r));
    const { h, sent, requests } = setup(excluded);
    expect(await h.handleContent(frame(), { type: "wa_create", options: createOpts })).toEqual({ ok: true, token: T1, ui: "create" });
    expect(sent).toEqual([]);
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "exists", site: "github.com" } });
    expect((await h.handleFrame(1, { type: "pk_save", token: T1, itemId: null })).ok).toBe(false);
    expect(requests.some((r) => r.type === "passkey_create")).toBe(false);
    expect(sent).toEqual([]);
    expect(await h.handleFrame(1, { type: "pk_close", token: T1 })).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toEqual({ type: "bg_wa_result", token: T1, outcome: { outcome: "error", name: "InvalidStateError" } });

    const other = setup(excluded);
    await other.h.handleContent(frame(), { type: "wa_create", options: createOpts });
    await other.h.handleFrame(1, { type: "pk_fallback", token: T1 });
    expect(other.sent.at(-1)).toEqual({ type: "bg_wa_result", token: T1, outcome: { outcome: "fallback" } });
  });

  it("refuses pk_close on any other card", async () => {
    const { h, sent } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    expect((await h.handleFrame(1, { type: "pk_close", token: T1 })).ok).toBe(false);
    expect(sent).toEqual([]);
  });

  it("keeps the card open with the error when the save fails offline", async () => {
    const { h, sent } = setup((r) => {
      if (r.type === "passkey_create") throw new BridgeError("offline", "HavenKeys is offline.");
      return defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    expect(await h.handleFrame(1, { type: "pk_save", token: T1, itemId: null })).toEqual({ ok: false, message: "HavenKeys is offline." });
    expect(sent).toEqual([]);
    await h.handleFrame(1, { type: "pk_fallback", token: T1 });
    expect(sent.at(-1)).toEqual({ type: "bg_wa_result", token: T1, outcome: { outcome: "fallback" } });
  });

  it("drops a late result after the page aborted", async () => {
    let release: () => void = () => {};
    const { h, sent } = setup((r) =>
      r.type === "passkey_create" ? new Promise((res) => (release = () => res(created))) : defaults(r),
    );
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    const saving = h.handleFrame(1, { type: "pk_save", token: T1, itemId: null });
    await h.handleContent(frame(), { type: "wa_cancel", token: T1 });
    release();
    await saving;
    expect(sent).toEqual([]);
  });
});

describe("conditional", () => {
  it("exposes passkeys to the field menu for the same frame only", async () => {
    const { h, sent } = setup(defaults);
    expect(await h.handleContent(frame(), { type: "wa_get", options: { ...getOpts, conditional: true } })).toEqual({ ok: true, token: T1, ui: "none" });
    expect(h.conditionalFor(frame())).toEqual([match]);
    expect(h.conditionalFor(frame({ frameId: 3 }))).toEqual([]);
    expect(await h.pickConditional(frame(), ITEM, CRED)).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toMatchObject({ type: "bg_wa_result", token: T1 });
  });

  it("keeps a session away from a navigated frame that has no documentId", async () => {
    const { h, requests } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_get", options: { ...getOpts, conditional: true } });
    const navigated = frame({ url: "https://evil.example/login", origin: "https://evil.example" });
    expect(navigated.documentId).toBeUndefined();
    expect(h.conditionalFor(navigated)).toEqual([]);
    expect((await h.pickConditional(navigated, ITEM, CRED)).ok).toBe(false);
    await h.handleContent(navigated, { type: "wa_cancel", token: T1 });
    expect(h.conditionalFor(frame())).toEqual([match]);
    expect(requests.some((r) => r.type === "passkey_get")).toBe(false);
  });
});

describe("unlock while a card is open", () => {
  it("turns a locked chooser into the real one", async () => {
    let locked = true;
    const { h, requests, sent } = setup((r) => {
      if (locked) throw new BridgeError("locked", "x");
      return defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    expect((await h.handleFrame(1, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: CRED })).ok).toBe(false);
    locked = false;
    await h.refreshLocked();
    expect(requests.at(-1)).toEqual({ type: "find_passkeys", url: "https://github.com/login", rpId: "github.com", allowCredentials: [] });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "chooser", site: "github.com", passkeys: [match] } });
    expect(await h.handleFrame(1, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: CRED })).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toMatchObject({ type: "bg_wa_result", token: T1, outcome: { outcome: "credential" } });
  });

  it("turns a locked save card into the real one", async () => {
    let locked = true;
    const { h } = setup((r) => {
      if (locked) throw new BridgeError("locked", "x");
      return defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    locked = false;
    await h.refreshLocked();
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toMatchObject({ ok: true, value: { state: "create" } });
  });

  it("falls back when the unlocked vault has no passkeys for the site", async () => {
    let locked = true;
    const { h, sent } = setup((r) => {
      if (locked) throw new BridgeError("locked", "x");
      return r.type === "find_passkeys" ? { type: "find_passkeys", passkeys: [] } : defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    locked = false;
    await h.refreshLocked();
    expect(sent.at(-1)).toEqual({ type: "bg_wa_result", token: T1, outcome: { outcome: "fallback" } });
    expect((await h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(false);
  });

  it("keeps waiting while still locked, then shows the already-saved card", async () => {
    let mode: "locked" | "excluded" = "locked";
    const { h, sent } = setup((r) => {
      if (mode === "locked") throw new BridgeError("locked", "x");
      return r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "none" } } : defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    await h.refreshLocked();
    expect(sent).toEqual([]);
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "locked", site: "github.com" } });
    mode = "excluded";
    await h.refreshLocked();
    expect(sent).toEqual([]);
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "exists", site: "github.com" } });
  });

  it("looks the passkeys up again when a locked card asks for its state", async () => {
    let locked = true;
    const { h } = setup((r) => {
      if (locked) throw new BridgeError("locked", "x");
      return defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "locked", site: "github.com" } });
    // Unlocked, but the "unlocked" event never arrived (the native port had closed).
    locked = false;
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "chooser", site: "github.com", passkeys: [match] } });
  });

  it("falls back when the lookup after an unlock is denied", async () => {
    let mode: "locked" | "denied" = "locked";
    const { h, sent } = setup(() => {
      throw new BridgeError(mode, "x");
    });
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    mode = "denied";
    await h.refreshLocked();
    expect(sent.at(-1)).toEqual({ type: "bg_wa_result", token: T1, outcome: { outcome: "fallback" } });
  });
});

describe("one operation at a time", () => {
  it("refuses a second pick while a signature is in flight", async () => {
    let release: () => void = () => {};
    const { h, requests } = setup((r) =>
      r.type === "passkey_get" ? new Promise((res) => (release = () => res(signed))) : defaults(r),
    );
    await h.handleContent(frame(), { type: "wa_get", options: getOpts });
    const first = h.handleFrame(1, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: CRED });
    expect(await h.handleFrame(1, { type: "pk_pick", token: T1, itemId: ITEM, credentialId: CRED })).toEqual({ ok: false, message: "Please wait…" });
    release();
    expect(await first).toEqual({ ok: true, value: null });
    expect(requests.filter((r) => r.type === "passkey_get")).toHaveLength(1);
  });

  it("refuses a second save while one is in flight", async () => {
    let release: () => void = () => {};
    const { h, requests } = setup((r) =>
      r.type === "passkey_create" ? new Promise((res) => (release = () => res(created))) : defaults(r),
    );
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    const first = h.handleFrame(1, { type: "pk_save", token: T1, itemId: null });
    expect(await h.handleFrame(1, { type: "pk_save", token: T1, itemId: ITEM })).toEqual({ ok: false, message: "Please wait…" });
    release();
    await first;
    expect(requests.filter((r) => r.type === "passkey_create")).toHaveLength(1);
  });

  it("allows a retry after a failed save", async () => {
    let offline = true;
    const { h, sent } = setup((r) => {
      if (r.type === "passkey_create" && offline) throw new BridgeError("offline", "HavenKeys is offline.");
      return defaults(r);
    });
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    expect((await h.handleFrame(1, { type: "pk_save", token: T1, itemId: null })).ok).toBe(false);
    offline = false;
    expect(await h.handleFrame(1, { type: "pk_save", token: T1, itemId: null })).toEqual({ ok: true, value: null });
    expect(sent.at(-1)).toMatchObject({ outcome: { outcome: "credential" } });
  });
});

describe("worker suspension", () => {
  it("answers pings for live sessions of the same frame only", async () => {
    const { h } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_get", options: { ...getOpts, conditional: true } });
    expect(await h.handleContent(frame(), { type: "wa_ping", token: T1 })).toEqual({ ok: true });
    expect(await h.handleContent(frame({ frameId: 3 }), { type: "wa_ping", token: T1 })).toEqual({ ok: false });
    expect(await h.handleContent(frame(), { type: "wa_ping", token: "f".repeat(32) })).toEqual({ ok: false });
    // A restarted worker has no sessions.
    expect(await setup(defaults).h.handleContent(frame(), { type: "wa_ping", token: T1 })).toEqual({ ok: false });
  });

  it("lets the user close a card whose session was lost", async () => {
    const { h, broadcast, sent } = setup(defaults);
    expect(await h.handleFrame(4, { type: "pk_state", token: T1 })).toEqual({ ok: false, message: "This prompt has expired." });
    expect(await h.handleFrame(4, { type: "pk_cancel", token: T1 })).toEqual({ ok: true, value: null });
    expect(broadcast.at(-1)).toEqual({ tabId: 4, msg: { type: "bg_wa_result", token: T1, outcome: { outcome: "error", name: "NotAllowedError" } } });
    expect(await h.handleFrame(4, { type: "pk_fallback", token: T1 })).toEqual({ ok: true, value: null });
    expect(broadcast.at(-1)).toEqual({ tabId: 4, msg: { type: "bg_wa_result", token: T1, outcome: { outcome: "fallback" } } });
    expect(await h.handleFrame(4, { type: "pk_close", token: T1 })).toEqual({ ok: true, value: null });
    expect(broadcast.at(-1)).toEqual({ tabId: 4, msg: { type: "bg_wa_result", token: T1, outcome: { outcome: "error", name: "NotAllowedError" } } });
    expect(sent).toEqual([]);
  });
});

describe("lookups per tab", () => {
  it("allows one wa_get/wa_create lookup in flight per tab", async () => {
    let release: () => void = () => {};
    let lookups = 0;
    const { h, requests } = setup((r) =>
      r.type === "find_passkeys" && ++lookups === 1
        ? new Promise((res) => (release = () => res({ type: "find_passkeys", passkeys: [match] })))
        : defaults(r),
    );
    const first = h.handleContent(frame(), { type: "wa_get", options: getOpts });
    expect(await h.handleContent(frame(), { type: "wa_get", options: getOpts })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
    expect(await h.handleContent(frame(), { type: "wa_create", options: createOpts })).toEqual({ ok: false, outcome: { outcome: "fallback" } });
    // Another tab is not affected.
    expect(await h.handleContent(frame({ tabId: 2 }), { type: "wa_get", options: getOpts })).toMatchObject({ ok: true });
    release();
    expect(await first).toMatchObject({ ok: true, ui: "chooser" });
    expect(requests.filter((r) => r.type === "find_passkeys")).toHaveLength(2);
    // Once the first lookup is done, the tab may ask again.
    expect(await h.handleContent(frame(), { type: "wa_get", options: getOpts })).toMatchObject({ ok: true });
  });
});

const cond: CreateOptions = { ...createOpts, conditional: true };
const withUpgrade = (upgrade: unknown) => (r: Request) =>
  r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: false, candidates: [{ itemId: ITEM, title: "GitHub", username: "octo" }], upgrade } : defaults(r);
const FB = { ok: false, outcome: { outcome: "fallback" } };

describe("automatic passkey upgrade", () => {
  it("saves silently on auto and returns the credential with a notice token", async () => {
    const { h, requests, sent } = setup(withUpgrade({ kind: "auto", itemId: ITEM }));
    const reply = await h.handleContent(frame(), { type: "wa_create", options: cond });
    expect(reply).toMatchObject({ ok: true, token: T1, ui: "saved", credential: { type: "create", credentialId: CRED } });
    expect(requests[0]).toMatchObject({ type: "check_passkey_create", conditional: true });
    expect(requests[1]).toMatchObject({ type: "passkey_create", itemId: ITEM, conditional: true, url: "https://github.com/login" });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toEqual({ ok: true, value: { state: "saved", site: "github.com" } });
    expect((await h.handleFrame(2, { type: "pk_state", token: T1 })).ok).toBe(false);
    // No session: nothing else is sent, and the notice token opens nothing else.
    expect(sent).toEqual([]);
    expect((await h.handleFrame(1, { type: "pk_save", token: T1, itemId: ITEM })).ok).toBe(false);
    expect(requests.filter((r) => r.type === "passkey_create")).toHaveLength(1);
  });

  it("forgets the notice after its TTL and when the tab goes away", async () => {
    const { h } = setup(withUpgrade({ kind: "auto", itemId: ITEM }));
    await h.handleContent(frame(), { type: "wa_create", options: cond });
    await vi.advanceTimersByTimeAsync(NOTICE_TTL_MS + 1);
    expect((await h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(false);
    const other = setup(withUpgrade({ kind: "auto", itemId: ITEM }));
    await other.h.handleContent(frame(), { type: "wa_create", options: cond });
    other.h.forgetTab(1);
    expect((await other.h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(false);
  });

  it("shows the Add a passkey card on ask, and the click saves non-conditionally", async () => {
    const { h, requests } = setup(withUpgrade({ kind: "ask", itemId: ITEM }));
    expect(await h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual({ ok: true, token: T1, ui: "create" });
    expect(await h.handleFrame(1, { type: "pk_state", token: T1 })).toMatchObject({ ok: true, value: { state: "create", upgradeItemId: ITEM } });
    expect(requests.some((r) => r.type === "passkey_create")).toBe(false);
    await h.handleFrame(1, { type: "pk_save", token: T1, itemId: ITEM });
    expect(requests.at(-1)).toMatchObject({ type: "passkey_create", itemId: ITEM, conditional: false });
  });

  it("falls back on none, excluded, locked, offline and failed saves", async () => {
    expect(await setup(withUpgrade({ kind: "none" })).h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    const excluded = setup((r) => (r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "none" } } : defaults(r)));
    expect(await excluded.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    const excludedAuto = setup((r) => (r.type === "check_passkey_create" ? { type: "check_passkey_create", excluded: true, candidates: [], upgrade: { kind: "auto", itemId: ITEM } } : defaults(r)));
    expect(await excludedAuto.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    expect(excludedAuto.requests.some((r) => r.type === "passkey_create")).toBe(false);
    const locked = setup(() => {
      throw new BridgeError("locked", "x");
    });
    expect(await locked.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    expect((await locked.h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(false);
    const offline = setup((r) => {
      if (r.type === "passkey_create") throw new BridgeError("offline", "x");
      return withUpgrade({ kind: "auto", itemId: ITEM })(r);
    });
    expect(await offline.h.handleContent(frame(), { type: "wa_create", options: cond })).toEqual(FB);
    expect((await offline.h.handleFrame(1, { type: "pk_state", token: T1 })).ok).toBe(false);
  });

  it("sends conditional: false for ordinary creates", async () => {
    const { h, requests } = setup(defaults);
    await h.handleContent(frame(), { type: "wa_create", options: createOpts });
    expect(requests[0]).toMatchObject({ type: "check_passkey_create", conditional: false });
    await h.handleFrame(1, { type: "pk_save", token: T1, itemId: ITEM });
    expect(requests.at(-1)).toMatchObject({ type: "passkey_create", conditional: false });
  });
});
