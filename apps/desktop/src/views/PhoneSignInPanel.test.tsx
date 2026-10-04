// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PairingState, VaultStatus } from "../lib/types";

const poll = vi.fn<() => Promise<PairingState>>();
const start = vi.fn();
vi.mock("../lib/api", () => ({
  api: {
    pairingStart: (...a: unknown[]) => start(...a),
    pairingPoll: () => poll(),
    pairingCancel: () => Promise.resolve(),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { PhoneSignInPanel } from "./PhoneSignInPanel";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const STATUS = { state: "unlocked" } as unknown as VaultStatus;
const approved: PairingState = { state: "approved", status: STATUS };

let host: HTMLElement;
let root: Root;

function panel(onSignedIn: (s: VaultStatus) => void) {
  return <PhoneSignInPanel onSignedIn={onSignedIn} onUseKit={() => undefined} />;
}

/** Type a server and submit, so the panel reaches its "code" phase. */
async function reachCode() {
  const input = host.querySelector("input")!;
  await act(async () => {
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    set.call(input, "https://vault.example.com");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await act(async () => {
    host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  poll.mockReset();
  start.mockReset();
  start.mockImplementation(async () => ({
    qrSize: 1,
    qrModules: [true],
    expiresAt: new Date(Date.now() + 60_000).toISOString(),
  }));
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.useRealTimers();
});

describe("PhoneSignInPanel polling", () => {
  it("hands an approval to onSignedIn once", async () => {
    poll.mockResolvedValueOnce({ state: "waiting" }).mockResolvedValueOnce(approved);
    const onSignedIn = vi.fn();
    await act(async () => root.render(panel(onSignedIn)));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000)));
    expect(onSignedIn).not.toHaveBeenCalled();
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000)));
    expect(onSignedIn).toHaveBeenCalledTimes(1);
    expect(onSignedIn).toHaveBeenCalledWith(STATUS);
    await act(async () => void (await vi.advanceTimersByTimeAsync(10_000)));
    expect(poll).toHaveBeenCalledTimes(2);
  });

  it("keeps its timer when the parent re-renders with a new callback", async () => {
    poll.mockResolvedValue(approved);
    const first = vi.fn();
    const latest = vi.fn();
    await act(async () => root.render(panel(first)));
    await reachCode();
    // Re-render faster than the poll interval, each time with a new function.
    for (let i = 0; i < 3; i++) {
      await act(async () => void (await vi.advanceTimersByTimeAsync(500)));
      await act(async () => root.render(panel(vi.fn())));
    }
    await act(async () => root.render(panel(latest)));
    await act(async () => void (await vi.advanceTimersByTimeAsync(500)));
    expect(poll).toHaveBeenCalledTimes(1);
    expect(latest).toHaveBeenCalledTimes(1);
    expect(first).not.toHaveBeenCalled();
  });

  it("still signs in when the approval arrives after the code expired", async () => {
    start.mockImplementation(async () => ({
      qrSize: 1,
      qrModules: [true],
      expiresAt: new Date(Date.now() + 3000).toISOString(),
    }));
    let answer!: (s: PairingState) => void;
    poll.mockImplementationOnce(() => new Promise<PairingState>((r) => (answer = r)));
    const onSignedIn = vi.fn();
    await act(async () => root.render(panel(onSignedIn)));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000))); // poll in flight
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000))); // countdown hits 0
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
    await act(async () => answer(approved));
    expect(onSignedIn).toHaveBeenCalledTimes(1);
    expect(onSignedIn).toHaveBeenCalledWith(STATUS);
  });
});
