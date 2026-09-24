// @vitest-environment jsdom
//
// The page-world wrapper against a fake browser WebAuthn implementation and a
// fake bridge (a listener on the request event). jsdom has no WebAuthn, so
// the tests provide PublicKeyCredential and navigator.credentials.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { REQUEST_EVENT, RESPONSE_EVENT } from "./messages";
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
});

// The fake bridge.
window.addEventListener(REQUEST_EVENT, (e) => {
  const req = JSON.parse((e as CustomEvent).detail as string) as Record<string, unknown>;
  requests.push(req);
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

  it("passes a conditional create straight through, like a silent get", async () => {
    const win = fresh();
    install(win);
    await win.navigator.credentials.create({ publicKey: pk, mediation: "conditional" } as CredentialCreationOptions);
    expect(requests).toEqual([]);
    expect(nativeCreate).toHaveBeenCalledTimes(1);
  });

  it("falls back instead of sending a request the bridge would silently drop", async () => {
    const win = fresh();
    install(win);
    const bigChallenge = new Uint8Array(1025);
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
