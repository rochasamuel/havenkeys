// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PairingState, VaultStatus } from "../lib/types";

const poll = vi.fn<() => Promise<PairingState>>();
const start = vi.fn();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    pairingStart: (...a: unknown[]) => start(...a),
    pairingPoll: () => poll(),
    pairingCancel: () => Promise.resolve(),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { ApiError } from "../lib/api";
import { en } from "../i18n/en";
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
    let answer!: (s: PairingState) => void;
    poll.mockResolvedValue({ state: "waiting" });
    const onSignedIn = vi.fn();
    await act(async () => root.render(panel(onSignedIn)));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(118_000))); // 59 polls
    poll.mockImplementationOnce(() => new Promise<PairingState>((r) => (answer = r)));
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000))); // last poll in flight
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000))); // countdown is past 0
    await act(async () => answer(approved));
    expect(onSignedIn).toHaveBeenCalledTimes(1);
    expect(onSignedIn).toHaveBeenCalledWith(STATUS);
  });

  it("counts 120 s from its own clock, polls once more at 0, then says expired", async () => {
    // The server's clock is a minute ahead: its expiresAt is already past here.
    start.mockImplementation(async () => ({
      qrSize: 1,
      qrModules: [true],
      expiresAt: new Date(Date.now() - 60_000).toISOString(),
    }));
    poll.mockResolvedValue({ state: "waiting" });
    await act(async () => root.render(panel(vi.fn())));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(119_000)));
    expect(host.textContent).toContain(en.welcome.phoneExpiresIn(1));
    expect(host.textContent).not.toContain(en.welcome.phoneExpired);
    const before = poll.mock.calls.length;
    await act(async () => void (await vi.advanceTimersByTimeAsync(1000)));
    expect(poll.mock.calls.length).toBe(before + 1);
    expect(host.textContent).toContain(en.welcome.phoneExpired);
    await act(async () => void (await vi.advanceTimersByTimeAsync(10_000)));
    expect(poll.mock.calls.length).toBe(before + 1);
  });

  it("stops at a failed sign-in and keeps its message, with a new-code button", async () => {
    poll
      .mockRejectedValueOnce(new ApiError("pairing_failed", "failed"))
      .mockRejectedValue(new ApiError("pairing_gone", "gone"));
    await act(async () => root.render(panel(vi.fn())));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000)));
    await act(async () => void (await vi.advanceTimersByTimeAsync(10_000)));
    expect(poll).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain(en.errors.codes.pairing_failed);
    expect(host.textContent).not.toContain(en.errors.codes.pairing_gone);
    expect(host.querySelector("img, svg, [role=img]")).toBeNull();
    const again = [...host.querySelectorAll("button")].find((b) => b.textContent === en.welcome.phoneNewCode);
    expect(again).toBeDefined();
  });

  it("stops when the pairing is gone", async () => {
    poll.mockRejectedValue(new ApiError("pairing_gone", "gone"));
    await act(async () => root.render(panel(vi.fn())));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(10_000)));
    expect(poll).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain(en.errors.codes.pairing_gone);
  });

  it("keeps polling through a network error and clears it on the next answer", async () => {
    poll.mockRejectedValueOnce(new ApiError("offline", "offline")).mockResolvedValue({ state: "waiting" });
    await act(async () => root.render(panel(vi.fn())));
    await reachCode();
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000)));
    expect(host.querySelector(".form-error")).not.toBeNull();
    await act(async () => void (await vi.advanceTimersByTimeAsync(2000)));
    expect(poll).toHaveBeenCalledTimes(2);
    expect(host.querySelector(".form-error")).toBeNull();
    expect(host.textContent).toContain(en.welcome.phoneScan);
  });
});
