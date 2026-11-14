// @vitest-environment jsdom
//
// The page-world wrapper against a fake browser WebAuthn implementation and a
// fake bridge (a listener on the request event). jsdom has no WebAuthn, so
// the tests provide PublicKeyCredential and navigator.credentials.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_CHALLENGE_BYTES } from "@havenkeys/protocol";
import { MIN_TIMEOUT_MS, REQUEST_EVENT, RESPONSE_EVENT } from "./messages";
import { install } from "./page";

const CRED = "AQEBAQEBAQEBAQEBAQEBAQ";
// Captured before any test can tamper with the page's `CustomEvent`: the
// fake bridge below stands in for the real extension's isolated-world
// content script, which a page can never reach, so it must not be broken by
// the same tampering page.ts is built to survive.
const RealCustomEvent = window.CustomEvent;
const nativeCreate = vi.fn(async (_o?: unknown) => ({ native: "create" }) as unknown as Credential);
const nativeGet = vi.fn(async (_o?: unknown) => ({ native: "get" }) as unknown as Credential);
let requests: Array<Record<string, unknown>> = [];
let answer: (req: Record<string, unknown>) => unknown = () => undefined;
/** False: no bridge in this frame (Firefox unloaded the isolated script). */
let bridgeLoaded = true;

class FakePKC {}
class FakeAttestation {}
class FakeAssertion {}

function fresh(): Window & typeof globalThis {
  // A new window-like object per test so install() captures fresh fakes.
  const win = window as Window & typeof globalThis;
  Object.assign(win, { PublicKeyCredential: FakePKC, AuthenticatorAttestationResponse: FakeAttestation, AuthenticatorAssertionResponse: FakeAssertion });
  Object.defineProperty(win.navigator, "credentials", {
    configurable: true,
    value: { create: nativeCreate, get: nativeGet },
  });
  return win;
}

beforeEach(() => {
  nativeCreate.mockClear();
  nativeGet.mockClear();
  requests = [];
  answer = () => undefined;
  bridgeLoaded = true;
});

afterEach(() => vi.useRealTimers());

// The fake bridge.
window.addEventListener(REQUEST_EVENT, (e) => {
  if (!bridgeLoaded) return;
  const req = JSON.parse((e as CustomEvent).detail as string) as Record<string, unknown>;
  requests.push(req);
  // Like the real bridge: acknowledged synchronously.
  if (req.kind !== "cancel") window.dispatchEvent(new RealCustomEvent(RESPONSE_EVENT, { detail: JSON.stringify({ id: req.id, outcome: "ack" }) }));
  const out = answer(req);
  if (out !== undefined) {
    queueMicrotask(() =>
      window.dispatchEvent(new RealCustomEvent(RESPONSE_EVENT, { detail: JSON.stringify({ id: req.id, ...(out as object) }) })),
    );
  }
});

const pk = { challenge: new Uint8Array(32), rp: { id: "github.com", name: "GitHub" }, user: { id: Uint8Array.of(1), name: "octo", displayName: "Octo" }, pubKeyCredParams: [{ type: "public-key", alg: -7 }] };

describe("page wrapper", () => {
  it("passes non-passkey requests straight through", async () => {
    const win = fresh();
    install(win);
    await win.navigator.credentials.get({ password: true } as CredentialRequestOptions);
    await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(8) }, mediation: "silent" });
    await win.navigator.credentials.create({ publicKey: { ...pk, authenticatorSelection: { authenticatorAttachment: "cross-platform" } } } as CredentialCreationOptions);
    await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(8), allowCredentials: [{ type: "public-key", id: new Uint8Array(32) }] } });
    expect(requests).toEqual([]);
    expect(nativeGet).toHaveBeenCalledTimes(3);
    expect(nativeCreate).toHaveBeenCalledTimes(1);
  });

  it("forwards a conditional create (the automatic upgrade), and falls back with the site's own options", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    const options = { publicKey: pk, mediation: "conditional" } as CredentialCreationOptions;
    expect(await win.navigator.credentials.create(options)).toEqual({ native: "create" });
    expect(requests).toHaveLength(1);
    expect(requests[0]?.kind).toBe("create");
    expect((requests[0]?.options as { conditional: boolean }).conditional).toBe(true);
    expect(nativeCreate).toHaveBeenCalledTimes(1);
    expect(nativeCreate.mock.calls[0]?.[0]).toBe(options);
    expect((nativeCreate.mock.calls[0]?.[0] as { mediation?: string }).mediation).toBe("conditional");
  });

  it("sends conditional: false for an ordinary create", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    await win.navigator.credentials.create({ publicKey: pk } as CredentialCreationOptions);
    expect((requests[0]?.options as { conditional: boolean }).conditional).toBe(false);
  });

  it("falls back instead of sending a request the bridge would silently drop", async () => {
    const win = fresh();
    install(win);
    const bigChallenge = new Uint8Array(MAX_CHALLENGE_BYTES + 1);
    const bigUserId = new Uint8Array(65);
    await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(0) } });
    await win.navigator.credentials.get({ publicKey: { challenge: bigChallenge } });
    await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4), rpId: "" } });
    await win.navigator.credentials.create({ publicKey: { ...pk, challenge: new Uint8Array(0) } } as CredentialCreationOptions);
    await win.navigator.credentials.create({ publicKey: { ...pk, challenge: bigChallenge } } as CredentialCreationOptions);
    await win.navigator.credentials.create({ publicKey: { ...pk, user: { ...pk.user, id: new Uint8Array(0) } } } as CredentialCreationOptions);
    await win.navigator.credentials.create({ publicKey: { ...pk, user: { ...pk.user, id: bigUserId } } } as CredentialCreationOptions);
    await win.navigator.credentials.create({ publicKey: { ...pk, rp: { ...pk.rp, id: "" } } } as CredentialCreationOptions);
    expect(requests).toEqual([]);
    expect(nativeGet).toHaveBeenCalledTimes(3);
    expect(nativeCreate).toHaveBeenCalledTimes(5);
  });

  it("sends Google's 1950-byte challenge to HavenKeys", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    const id = new Uint8Array(16).fill(7);
    await win.navigator.credentials.get({
      publicKey: { challenge: new Uint8Array(1950).buffer, rpId: "google.com", allowCredentials: [{ type: "public-key", id: id.buffer }] },
    });
    expect(requests).toHaveLength(1);
    expect((requests[0]?.options as { allowCredentials: string[] }).allowCredentials).toEqual(["BwcHBwcHBwcHBwcHBwcHBw"]);
  });

  it("sends a null timeout instead of a non-finite one the bridge would reject", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4), timeout: Infinity } });
    expect((requests[0]?.options as { timeoutMs: number | null }).timeoutMs).toBeNull();
  });

  it("returns a credential built from the bridge's answer", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "credential", credential: { type: "get", credentialId: CRED, clientDataJson: "e30", authenticatorData: "AA", signature: "MEU", userHandle: "AQ" } });
    const c = (await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) } })) as PublicKeyCredential;
    expect(requests[0]?.kind).toBe("get");
    expect(c instanceof FakePKC).toBe(true);
    expect(c.id).toBe(CRED);
    expect(c.type).toBe("public-key");
    expect(new Uint8Array(c.rawId)).toEqual(new Uint8Array(16).fill(1));
    expect(c.response instanceof FakeAssertion).toBe(true);
    // jsdom's TextEncoder returns a Uint8Array from a different realm than
    // this file's own (a jsdom/vitest quirk, unrelated to page.ts), so the
    // two would never `toEqual`; comparing plain byte arrays sidesteps that.
    expect(Array.from(new Uint8Array((c.response as AuthenticatorAssertionResponse).clientDataJSON))).toEqual(
      Array.from(new TextEncoder().encode("{}")),
    );
    expect(c.getClientExtensionResults()).toEqual({});
    expect((c as unknown as { toJSON(): { id: string } }).toJSON().id).toBe(CRED);
    expect(nativeGet).not.toHaveBeenCalled();
  });

  it("lets the site overwrite the credential's methods, as it can on a real one", async () => {
    // GitHub's webauthn-json ponyfill does `credential.toJSON = ...` in strict
    // mode; a read-only own property made that throw and registration fail.
    const win = fresh();
    install(win);
    answer = () => ({
      outcome: "credential",
      credential: { type: "create", credentialId: CRED, clientDataJson: "e30", attestationObject: "AA", authenticatorData: "AA", publicKey: "AA", publicKeyAlgorithm: -7 },
    });
    const c = (await win.navigator.credentials.create({ publicKey: pk } as CredentialCreationOptions)) as unknown as Record<string, unknown> & {
      response: Record<string, unknown>;
    };
    const replacement = () => "site";
    for (const k of ["toJSON", "getClientExtensionResults"]) {
      c[k] = replacement;
      expect(c[k]).toBe(replacement);
    }
    for (const k of ["getAuthenticatorData", "getPublicKey", "getPublicKeyAlgorithm", "getTransports"]) {
      c.response[k] = replacement;
      expect(c.response[k]).toBe(replacement);
    }
  });

  it("drops foreign credential IDs and falls back on request", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    await win.navigator.credentials.create({ publicKey: { ...pk, excludeCredentials: [{ type: "public-key", id: new Uint8Array(64) }, { type: "public-key", id: new Uint8Array(16).fill(1) }] } } as CredentialCreationOptions);
    const sent = requests[0]?.options as { excludeCredentials: string[]; algs: number[]; rpId: string };
    expect(sent.excludeCredentials).toEqual([CRED]);
    expect(sent.algs).toEqual([-7]);
    expect(sent.rpId).toBe("github.com");
    expect(nativeCreate).toHaveBeenCalledTimes(1);
  });

  it("maps errors to DOMExceptions", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "error", name: "InvalidStateError" });
    await expect(win.navigator.credentials.create({ publicKey: pk } as CredentialCreationOptions)).rejects.toMatchObject({ name: "InvalidStateError" });
  });

  it("aborts with the site's reason and tells the bridge", async () => {
    const win = fresh();
    install(win);
    const ctrl = new AbortController();
    const p = win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, signal: ctrl.signal });
    ctrl.abort("gone");
    await expect(p).rejects.toBe("gone");
    expect(requests.map((r) => r.kind)).toEqual(["get", "cancel"]);
  });

  it("keeps working after the page replaces globals", async () => {
    const win = fresh();
    install(win);
    const saved = { stringify: JSON.stringify, CustomEvent: win.CustomEvent };
    try {
      JSON.stringify = () => "tampered";
      (win as { CustomEvent: unknown }).CustomEvent = function Evil() {};
      answer = () => ({ outcome: "fallback" });
      const p = win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) } });
      JSON.stringify = saved.stringify; // the fake bridge needs it
      await p;
      expect(requests[0]?.kind).toBe("get");
    } finally {
      JSON.stringify = saved.stringify;
      (win as { CustomEvent: unknown }).CustomEvent = saved.CustomEvent;
    }
  });

  it("races conditional requests with the browser's own", async () => {
    const win = fresh();
    let nativeSignal: AbortSignal | undefined;
    nativeGet.mockImplementationOnce((o?: unknown) => {
      nativeSignal = (o as { signal: AbortSignal }).signal;
      return new Promise(() => {}) as Promise<Credential>;
    });
    install(win);
    answer = () => ({ outcome: "credential", credential: { type: "get", credentialId: CRED, clientDataJson: "e30", authenticatorData: "AA", signature: "MEU", userHandle: "AQ" } });
    const c = await win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, mediation: "conditional" });
    expect((requests[0]?.options as { conditional: boolean }).conditional).toBe(true);
    expect((c as PublicKeyCredential).id).toBe(CRED);
    expect(nativeSignal?.aborted).toBe(true);
    await expect((win.PublicKeyCredential as unknown as { isConditionalMediationAvailable(): Promise<boolean> }).isConditionalMediationAvailable()).resolves.toBe(true);
  });

  it("reports a passkey store, which sites check before offering to create one", async () => {
    // Google's "Create a passkey" checks isUserVerifyingPlatformAuthenticatorAvailable
    // and never calls create() when it is false (Linux; some Windows setups).
    class PKC {
      static isUserVerifyingPlatformAuthenticatorAvailable = () => Promise.resolve(false);
      static getClientCapabilities = () =>
        Promise.resolve({ conditionalGet: true, hybridTransport: true, passkeyPlatformAuthenticator: false, userVerifyingPlatformAuthenticator: false });
    }
    const win = fresh();
    Object.assign(win, { PublicKeyCredential: PKC });
    install(win);
    await expect(PKC.isUserVerifyingPlatformAuthenticatorAvailable()).resolves.toBe(true);
    // Only the two platform-authenticator answers and the extensions
    // HavenKeys answers change; the rest is the browser's.
    await expect(PKC.getClientCapabilities()).resolves.toEqual({
      conditionalGet: true,
      hybridTransport: true,
      passkeyPlatformAuthenticator: true,
      userVerifyingPlatformAuthenticator: true,
      "extension:credProps": true,
      "extension:prf": true,
    });
  });

  it("leaves getClientCapabilities absent where the browser has none, and survives it rejecting", async () => {
    class Bare {}
    const win = fresh();
    Object.assign(win, { PublicKeyCredential: Bare });
    install(win);
    expect("getClientCapabilities" in Bare).toBe(false);
    await expect((Bare as unknown as { isUserVerifyingPlatformAuthenticatorAvailable(): Promise<boolean> }).isUserVerifyingPlatformAuthenticatorAvailable()).resolves.toBe(true);

    class Failing {
      static getClientCapabilities = () => Promise.reject(new Error("nope"));
    }
    Object.assign(win, { PublicKeyCredential: Failing });
    install(win);
    await expect(Failing.getClientCapabilities()).resolves.toEqual({
      passkeyPlatformAuthenticator: true,
      userVerifyingPlatformAuthenticator: true,
      "extension:credProps": true,
      "extension:prf": true,
    });
  });

  it("rejects a conditional get immediately when the signal is already aborted", async () => {
    const win = fresh();
    install(win);
    const ctrl = new AbortController();
    ctrl.abort("gone");
    await expect(
      win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, mediation: "conditional", signal: ctrl.signal }),
    ).rejects.toBe("gone");
    expect(requests).toEqual([]);
    expect(nativeGet).not.toHaveBeenCalled();
  });
});

describe("extensions, algorithms and transports (spec 2026-11-14-passkey-parity)", () => {
  const OUT = "AQ" + "A".repeat(41); // 32 bytes, first byte 0x01
  const createdCred = { type: "create", credentialId: CRED, clientDataJson: "e30", attestationObject: "AA", authenticatorData: "AA", publicKey: "AA", publicKeyAlgorithm: -8 };
  const assertedCred = { type: "get", credentialId: CRED, clientDataJson: "e30", authenticatorData: "AA", signature: "MEU", userHandle: "AQ" };
  const bytes = (b: unknown) => Array.from(new Uint8Array(b as ArrayBuffer));

  it("answers credProps only when asked, and reports hybrid and internal transports", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "credential", credential: createdCred });
    const asked = (await win.navigator.credentials.create({ publicKey: { ...pk, extensions: { credProps: true } } } as CredentialCreationOptions)) as PublicKeyCredential;
    expect(asked.getClientExtensionResults()).toEqual({ credProps: { rk: true } });
    const json = (asked as unknown as { toJSON(): { clientExtensionResults: unknown; response: { transports: string[]; publicKeyAlgorithm: number } } }).toJSON();
    expect(json.clientExtensionResults).toEqual({ credProps: { rk: true } });
    expect(json.response.transports).toEqual(["hybrid", "internal"]);
    expect(json.response.publicKeyAlgorithm).toBe(-8);
    const response = asked.response as unknown as { getTransports(): string[]; getPublicKeyAlgorithm(): number };
    expect(response.getTransports()).toEqual(["hybrid", "internal"]);
    expect(response.getPublicKeyAlgorithm()).toBe(-8);

    const plain = (await win.navigator.credentials.create({ publicKey: pk } as CredentialCreationOptions)) as PublicKeyCredential;
    expect(plain.getClientExtensionResults()).toEqual({});
  });

  it("forwards the site's algorithms in order and falls back with no public-key entry", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    await win.navigator.credentials.create({ publicKey: { ...pk, pubKeyCredParams: [{ type: "public-key", alg: -8 }, { type: "public-key", alg: -7 }] } } as CredentialCreationOptions);
    expect((requests[0]?.options as { algs: number[] }).algs).toEqual([-8, -7]);
    await win.navigator.credentials.create({ publicKey: { ...pk, pubKeyCredParams: [] } } as CredentialCreationOptions);
    expect((requests[1]?.options as { algs: number[] }).algs).toEqual([]);
    requests = [];
    await win.navigator.credentials.create({ publicKey: { ...pk, pubKeyCredParams: [{ type: "other", alg: -7 }] } } as unknown as CredentialCreationOptions);
    expect(requests).toEqual([]);
  });

  it("sends PRF inputs as base64url and returns PRF outputs as ArrayBuffers", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "credential", credential: { ...createdCred, prf: { enabled: true, results: { first: OUT, second: OUT } } } });
    const c = (await win.navigator.credentials.create({
      publicKey: { ...pk, extensions: { prf: { eval: { first: Uint8Array.of(1, 2, 3), second: new Uint8Array(0) } } } },
    } as CredentialCreationOptions)) as PublicKeyCredential;
    expect((requests[0]?.options as { prf: unknown }).prf).toEqual({ eval: { first: "AQID", second: "" } });
    const ext = c.getClientExtensionResults() as { prf: { enabled: boolean; results: { first: ArrayBuffer; second: ArrayBuffer } } };
    expect(ext.prf.enabled).toBe(true);
    expect(bytes(ext.prf.results.first)).toEqual([1, ...new Array(31).fill(0)]);
    expect(bytes(ext.prf.results.second)).toHaveLength(32);
    expect((c as unknown as { toJSON(): { clientExtensionResults: unknown } }).toJSON().clientExtensionResults).toEqual({
      prf: { enabled: true, results: { first: OUT, second: OUT } },
    });

    // prf: {} at creation enables PRF without evaluating.
    answer = () => ({ outcome: "credential", credential: { ...createdCred, prf: { enabled: true } } });
    const e = (await win.navigator.credentials.create({ publicKey: { ...pk, extensions: { prf: {} } } } as CredentialCreationOptions)) as PublicKeyCredential;
    expect((requests[1]?.options as { prf: unknown }).prf).toEqual({ eval: null });
    expect(e.getClientExtensionResults()).toEqual({ prf: { enabled: true } });
  });

  it("picks our evalByCredential entries and drops foreign ones", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "credential", credential: { ...assertedCred, prf: { results: { first: OUT } } } });
    const ours = new Uint8Array(16).fill(1);
    const foreign = new Uint8Array(32).fill(2);
    const c = (await win.navigator.credentials.get({
      publicKey: {
        challenge: new Uint8Array(4),
        allowCredentials: [{ type: "public-key", id: ours }, { type: "public-key", id: foreign }],
        extensions: {
          prf: {
            eval: { first: Uint8Array.of(9) },
            evalByCredential: { [CRED]: { first: Uint8Array.of(1) }, ["Ag".repeat(21) + "A"]: { first: Uint8Array.of(2) } },
          },
        },
      },
    } as unknown as CredentialRequestOptions)) as PublicKeyCredential;
    expect((requests[0]?.options as { prf: unknown }).prf).toEqual({ eval: { first: "CQ" }, byCredential: [{ credentialId: CRED, values: { first: "AQ" } }] });
    const ext = c.getClientExtensionResults() as { prf: { results: { first: ArrayBuffer } } };
    expect(bytes(ext.prf.results.first)[0]).toBe(1);
  });

  it("leaves PRF requests the browser must refuse, or too large ones, to the browser", async () => {
    const win = fresh();
    install(win);
    answer = () => ({ outcome: "fallback" });
    const big = new Uint8Array(1025);
    // evalByCredential at creation; a salt over 1024 bytes; a missing first.
    await win.navigator.credentials.create({ publicKey: { ...pk, extensions: { prf: { evalByCredential: {} } } } } as unknown as CredentialCreationOptions);
    await win.navigator.credentials.create({ publicKey: { ...pk, extensions: { prf: { eval: { first: big } } } } } as unknown as CredentialCreationOptions);
    await win.navigator.credentials.create({ publicKey: { ...pk, extensions: { prf: { eval: {} } } } } as unknown as CredentialCreationOptions);
    // evalByCredential without allowCredentials; a key that is not base64url;
    // one of ours that the site did not allow.
    const get = (prf: unknown, allow: unknown[] = []) =>
      win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4), allowCredentials: allow, extensions: { prf } } } as unknown as CredentialRequestOptions);
    await get({ evalByCredential: { [CRED]: { first: Uint8Array.of(1) } } });
    await get({ evalByCredential: { "not base64!": { first: Uint8Array.of(1) } } }, [{ type: "public-key", id: new Uint8Array(16).fill(1) }]);
    await get({ evalByCredential: { ["Ag".repeat(10) + "Ag"]: { first: Uint8Array.of(1) } } }, [{ type: "public-key", id: new Uint8Array(16).fill(1) }]);
    // Many large salts: more than the bridge accepts in one message.
    const many: Record<string, unknown> = {};
    const allow = [];
    for (let i = 0; i < 20; i++) {
      const id = new Uint8Array(16).fill(i + 1);
      allow.push({ type: "public-key", id });
      let s = "";
      for (const b of id) s += String.fromCharCode(b);
      many[btoa(s).replace(/=+$/, "").replace(/\+/g, "-").replace(/\//g, "_")] = { first: new Uint8Array(1024) };
    }
    await get({ evalByCredential: many }, allow);
    expect(requests).toEqual([]);
    expect(nativeCreate).toHaveBeenCalledTimes(3);
    expect(nativeGet).toHaveBeenCalledTimes(4);
  });
});

describe("missing or silent bridge", () => {
  const settled = (p: Promise<unknown>) => {
    const state = { done: false, value: undefined as unknown, error: undefined as unknown };
    p.then(
      (v) => Object.assign(state, { done: true, value: v }),
      (e: unknown) => Object.assign(state, { done: true, error: e }),
    );
    return state;
  };

  it("uses the browser's own implementation when no bridge acknowledges", async () => {
    vi.useFakeTimers();
    const win = fresh();
    install(win);
    bridgeLoaded = false;
    const g = settled(win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) } }));
    const c = settled(win.navigator.credentials.create({ publicKey: pk } as CredentialCreationOptions));
    await vi.advanceTimersByTimeAsync(999);
    expect(g.done || c.done).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(g.value).toEqual({ native: "get" });
    expect(c.value).toEqual({ native: "create" });
  });

  it("stops only our leg of a conditional request when no bridge acknowledges", async () => {
    vi.useFakeTimers();
    const win = fresh();
    let release: (c: Credential) => void = () => {};
    nativeGet.mockImplementationOnce(() => new Promise<Credential>((res) => (release = res)));
    install(win);
    bridgeLoaded = false;
    const g = settled(win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, mediation: "conditional" }));
    await vi.advanceTimersByTimeAsync(60_000);
    expect(g.done).toBe(false);
    release({ native: "autofill" } as unknown as Credential);
    await vi.advanceTimersByTimeAsync(0);
    expect(g.value).toEqual({ native: "autofill" });
  });

  it("gives up with NotAllowedError when an acknowledged request is never answered", async () => {
    vi.useFakeTimers();
    const win = fresh();
    install(win);
    const g = settled(win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4), timeout: 60_000 } }));
    await vi.advanceTimersByTimeAsync(60_000 + 15_000 - 1);
    expect(g.done).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(g.error).toMatchObject({ name: "NotAllowedError" });
    expect(requests.map((r) => r.kind)).toEqual(["get", "cancel"]);
    expect(nativeGet).not.toHaveBeenCalled();
  });

  it("never clamps below MIN_TIMEOUT_MS", async () => {
    vi.useFakeTimers();
    const win = fresh();
    install(win);
    const g = settled(win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4), timeout: 0 } }));
    await vi.advanceTimersByTimeAsync(MIN_TIMEOUT_MS + 15_000 - 1);
    expect(g.done).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    expect(g.error).toMatchObject({ name: "NotAllowedError" });
  });

  it("puts no ceiling on an acknowledged conditional request", async () => {
    vi.useFakeTimers();
    const win = fresh();
    nativeGet.mockImplementationOnce(() => new Promise<Credential>(() => {}));
    install(win);
    const g = settled(win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, mediation: "conditional" }));
    await vi.advanceTimersByTimeAsync(30 * 60_000);
    expect(g.done).toBe(false);
  });
});

describe("conditional aborts", () => {
  it("leaves the site's promise to the browser when our leg is aborted without the site asking", async () => {
    const win = fresh();
    let release: (c: Credential) => void = () => {};
    nativeGet.mockImplementationOnce(() => new Promise<Credential>((res) => (release = res)));
    install(win);
    answer = () => ({ outcome: "error", name: "AbortError" }); // e.g. pagehide, or a newer request replaced ours
    const p = win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, mediation: "conditional" });
    await new Promise((r) => setTimeout(r, 0));
    release({ native: "autofill" } as unknown as Credential);
    await expect(p).resolves.toEqual({ native: "autofill" });
  });

  it("rejects and stops both legs when the site aborts", async () => {
    const win = fresh();
    let nativeSignal: AbortSignal | undefined;
    nativeGet.mockImplementationOnce((o?: unknown) => {
      nativeSignal = (o as { signal: AbortSignal }).signal;
      return new Promise<Credential>(() => {});
    });
    install(win);
    const ctrl = new AbortController();
    const p = win.navigator.credentials.get({ publicKey: { challenge: new Uint8Array(4) }, mediation: "conditional", signal: ctrl.signal });
    ctrl.abort("gone");
    await expect(p).rejects.toBe("gone");
    expect(nativeSignal?.aborted).toBe(true);
    expect(requests.map((r) => r.kind)).toEqual(["get", "cancel"]);
  });
});
