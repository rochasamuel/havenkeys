// Runs in the page's own JavaScript world (MAIN), at document_start, only on
// sites the user granted. It holds no secrets and decides nothing: it turns
// a site's WebAuthn call into plain data for the isolated bridge, and the
// answer back into a PublicKeyCredential. Every path the user did not choose
// ends in the browser's own implementation, so the site behaves as if
// HavenKeys were not installed.
//
// Page script shares this world and can call, replace or observe this
// wrapper. That grants it nothing: it could call WebAuthn itself, and every
// decision is made by the desktop against the URL the browser reports.

import { CREDENTIAL_ID_BYTES } from "@havenkeys/protocol";
import { bufferSourceBytes, toArrayBuffer, toB64Url } from "./encoding";
import {
  parsePageResponse,
  REQUEST_EVENT,
  RESPONSE_EVENT,
  type AssertedCredential,
  type CreatedCredential,
  type CreateOptions,
  type ErrorName,
  type GetOptions,
  type Outcome,
  type PageRequest,
} from "./messages";

const MESSAGES: Record<ErrorName, string> = {
  NotAllowedError: "The operation either timed out or was not allowed.",
  InvalidStateError: "The authenticator already holds a credential for this account.",
  SecurityError: "The relying party ID is not valid for this page.",
  AbortError: "The operation was aborted.",
};

type Win = Window & typeof globalThis;
type Native = (options?: never) => Promise<Credential | null>;

export function install(win: Win): void {
  const container = win.navigator.credentials as CredentialsContainer | undefined;
  const PKC = win.PublicKeyCredential as (typeof PublicKeyCredential) | undefined;
  if (!container || typeof PKC !== "function") return;

  // Captured before any page script runs.
  const origCreate = container.create.bind(container) as Native;
  const origGet = container.get.bind(container) as Native;
  const stringify = JSON.stringify;
  const dispatch = win.dispatchEvent.bind(win);
  const listen = win.addEventListener.bind(win);
  const unlisten = win.removeEventListener.bind(win);
  const Custom = win.CustomEvent;
  const Abort = win.AbortController;
  const DOMErr = win.DOMException;
  const random = win.crypto.getRandomValues.bind(win.crypto);
  const define = Object.defineProperties;
  const create = Object.create;
  const AttProto = typeof win.AuthenticatorAttestationResponse === "function" ? win.AuthenticatorAttestationResponse.prototype : Object.prototype;
  const AssProto = typeof win.AuthenticatorAssertionResponse === "function" ? win.AuthenticatorAssertionResponse.prototype : Object.prototype;

  const newId = (): string => Array.from(random(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
  const send = (req: PageRequest): void => void dispatch(new Custom(REQUEST_EVENT, { detail: stringify(req) }));

  /** Our 16-byte IDs from a descriptor list; null when unreadable. */
  function ownIds(list: unknown): string[] | null {
    if (list === undefined) return [];
    if (!Array.isArray(list)) return null;
    const out: string[] = [];
    for (const d of list as Array<{ id?: unknown }>) {
      const bytes = bufferSourceBytes(d?.id);
      if (bytes && bytes.length === CREDENTIAL_ID_BYTES) out.push(toB64Url(bytes));
    }
    return out.slice(0, 64);
  }

  function createOptions(pk: PublicKeyCredentialCreationOptions): CreateOptions | null {
    try {
      const challenge = bufferSourceBytes(pk.challenge);
      const userId = bufferSourceBytes(pk.user?.id);
      if (!challenge || !userId) return null;
      if (pk.authenticatorSelection?.authenticatorAttachment === "cross-platform") return null;
      const exclude = ownIds(pk.excludeCredentials);
      if (!exclude) return null;
      const algs = (pk.pubKeyCredParams ?? [])
        .filter((p) => p?.type === "public-key" && Number.isInteger(p.alg))
        .map((p) => p.alg)
        .slice(0, 16);
      const display = pk.user.displayName;
      return {
        rpId: typeof pk.rp?.id === "string" ? pk.rp.id : null,
        challenge: toB64Url(challenge),
        userId: toB64Url(userId),
        userName: typeof pk.user.name === "string" ? pk.user.name.slice(0, 512) : "",
        userDisplayName: typeof display === "string" && display ? display.slice(0, 512) : null,
        algs,
        excludeCredentials: exclude,
        timeoutMs: typeof pk.timeout === "number" && pk.timeout >= 0 ? pk.timeout : null,
      };
    } catch {
      return null;
    }
  }

  function getOptions(pk: PublicKeyCredentialRequestOptions, conditional: boolean): GetOptions | null {
    try {
      const challenge = bufferSourceBytes(pk.challenge);
      if (!challenge) return null;
      const allow = ownIds(pk.allowCredentials);
      if (!allow) return null;
      // The site named credentials, none of them ours.
      if ((pk.allowCredentials?.length ?? 0) > 0 && allow.length === 0) return null;
      return {
        rpId: typeof pk.rpId === "string" ? pk.rpId : null,
        challenge: toB64Url(challenge),
        allowCredentials: allow,
        conditional,
        timeoutMs: typeof pk.timeout === "number" && pk.timeout >= 0 ? pk.timeout : null,
      };
    } catch {
      return null;
    }
  }

  /** Send one request; resolve with the bridge's outcome or an abort. */
  function ask(req: PageRequest, signal: AbortSignal | undefined): Promise<Outcome> {
    return new Promise((resolve) => {
      if (signal?.aborted) {
        resolve({ outcome: "error", name: "AbortError" });
        return;
      }
      const done = () => {
        unlisten(RESPONSE_EVENT, onResponse);
        signal?.removeEventListener("abort", onAbort);
      };
      const onResponse = (e: Event) => {
        const r = parsePageResponse((e as CustomEvent).detail);
        if (!r || r.id !== req.id) return;
        done();
        resolve(r);
      };
      const onAbort = () => {
        done();
        send({ kind: "cancel", id: req.id });
        resolve({ outcome: "error", name: "AbortError" });
      };
      listen(RESPONSE_EVENT, onResponse);
      signal?.addEventListener("abort", onAbort);
      send(req);
    });
  }

  function rejection(name: ErrorName, signal: AbortSignal | undefined): unknown {
    if (name === "AbortError" && signal?.reason !== undefined) return signal.reason;
    return new DOMErr(MESSAGES[name], name);
  }

  function credential(c: CreatedCredential | AssertedCredential): Credential {
    const response =
      c.type === "create"
        ? define(create(AttProto), {
            clientDataJSON: { value: toArrayBuffer(c.clientDataJson), enumerable: true },
            attestationObject: { value: toArrayBuffer(c.attestationObject), enumerable: true },
            getAuthenticatorData: { value: () => toArrayBuffer(c.authenticatorData) },
            getPublicKey: { value: () => toArrayBuffer(c.publicKey) },
            getPublicKeyAlgorithm: { value: () => c.publicKeyAlgorithm },
            getTransports: { value: () => ["internal"] },
          })
        : define(create(AssProto), {
            clientDataJSON: { value: toArrayBuffer(c.clientDataJson), enumerable: true },
            authenticatorData: { value: toArrayBuffer(c.authenticatorData), enumerable: true },
            signature: { value: toArrayBuffer(c.signature), enumerable: true },
            userHandle: { value: toArrayBuffer(c.userHandle), enumerable: true },
          });
    const responseJson =
      c.type === "create"
        ? {
            clientDataJSON: c.clientDataJson,
            attestationObject: c.attestationObject,
            authenticatorData: c.authenticatorData,
            publicKey: c.publicKey,
            publicKeyAlgorithm: c.publicKeyAlgorithm,
            transports: ["internal"],
          }
        : { clientDataJSON: c.clientDataJson, authenticatorData: c.authenticatorData, signature: c.signature, userHandle: c.userHandle };
    return define(create((PKC as typeof PublicKeyCredential).prototype), {
      id: { value: c.credentialId, enumerable: true },
      rawId: { value: toArrayBuffer(c.credentialId), enumerable: true },
      type: { value: "public-key", enumerable: true },
      authenticatorAttachment: { value: "platform", enumerable: true },
      response: { value: response, enumerable: true },
      getClientExtensionResults: { value: () => ({}) },
      toJSON: {
        value: () => ({
          id: c.credentialId,
          rawId: c.credentialId,
          type: "public-key",
          authenticatorAttachment: "platform",
          clientExtensionResults: {},
          response: responseJson,
        }),
      },
    }) as Credential;
  }

  function settle(o: Outcome, fallback: () => Promise<Credential | null>, signal: AbortSignal | undefined): Promise<Credential | null> {
    switch (o.outcome) {
      case "credential":
        return Promise.resolve(credential(o.credential));
      case "fallback":
        return fallback();
      case "error":
        return Promise.reject(rejection(o.name, signal));
    }
  }

  /** Passkey autofill: ours and the browser's run side by side; the first the user picks wins. */
  function conditional(req: PageRequest, options: CredentialRequestOptions, signal: AbortSignal | undefined): Promise<Credential | null> {
    const ctrl = new Abort();
    const onSiteAbort = () => ctrl.abort(signal?.reason);
    signal?.addEventListener("abort", onSiteAbort);
    const nativeOptions: CredentialRequestOptions = { ...options, signal: ctrl.signal };
    const native = origGet(nativeOptions as never);
    const ours = ask(req, signal);
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (f: () => void) => {
        if (settled) return;
        settled = true;
        signal?.removeEventListener("abort", onSiteAbort);
        f();
      };
      void ours.then((o) => {
        if (o.outcome === "credential") {
          finish(() => {
            ctrl.abort();
            resolve(credential(o.credential));
          });
        } else if (o.outcome === "error" && o.name === "AbortError") {
          finish(() => reject(rejection("AbortError", signal)));
        }
        // Fallback or a refusal: the browser's own autofill keeps running.
      });
      native.then(
        (c) =>
          finish(() => {
            send({ kind: "cancel", id: req.id });
            resolve(c);
          }),
        (e: unknown) => {
          // Our own abort after we won is not the site's business.
          if (ctrl.signal.aborted && !signal?.aborted) return;
          finish(() => {
            send({ kind: "cancel", id: req.id });
            reject(e);
          });
        },
      );
    });
  }

  function wrappedCreate(options?: CredentialCreationOptions): Promise<Credential | null> {
    const fallback = () => origCreate(options as never);
    const pk = options?.publicKey;
    const opts = pk ? createOptions(pk) : null;
    if (!opts) return fallback();
    const signal = options?.signal ?? undefined;
    return ask({ kind: "create", id: newId(), options: opts }, signal).then((o) => settle(o, fallback, signal));
  }

  function wrappedGet(options?: CredentialRequestOptions): Promise<Credential | null> {
    const fallback = () => origGet(options as never);
    const pk = options?.publicKey;
    if (!pk || options?.mediation === "silent") return fallback();
    const isConditional = options.mediation === "conditional";
    const opts = getOptions(pk, isConditional);
    if (!opts) return fallback();
    const signal = options.signal ?? undefined;
    const req: PageRequest = { kind: "get", id: newId(), options: opts };
    if (isConditional) return conditional(req, options, signal);
    return ask(req, signal).then((o) => settle(o, fallback, signal));
  }

  Object.defineProperty(container, "create", { value: wrappedCreate, writable: true, configurable: true });
  Object.defineProperty(container, "get", { value: wrappedGet, writable: true, configurable: true });
  Object.defineProperty(PKC, "isConditionalMediationAvailable", {
    value: () => Promise.resolve(true),
    writable: true,
    configurable: true,
  });
}
