// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const previewInvite = vi.fn<(invite: string) => Promise<{ email: string; serverUrl: string }>>();
const openSignup = vi.fn<() => Promise<void>>();
const estimateMasterPassword =
  vi.fn<(password: string, inputs: string[]) => Promise<{ score: 0 | 1 | 2 | 3 | 4; guessesLog10: number }>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    previewInvite: (i: string) => previewInvite(i),
    openSignup: () => openSignup(),
    estimateMasterPassword: (p: string, i: string[]) => estimateMasterPassword(p, i),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { en } from "../i18n/en";
import { WelcomeScreen } from "./WelcomeScreen";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  vi.useFakeTimers();
  previewInvite.mockReset().mockResolvedValue({ email: "me@example.com", serverUrl: "https://api.havenkeys.net" });
  openSignup.mockReset().mockResolvedValue(undefined);
  estimateMasterPassword.mockReset().mockResolvedValue({ score: 1, guessesLog10: 4.2 });
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.useRealTimers();
});

function typeInvite(value: string) {
  const area = host.querySelector<HTMLTextAreaElement>("textarea")!;
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!;
  setter.call(area, value);
  area.dispatchEvent(new Event("input", { bubbles: true }));
}

function typePassword(value: string) {
  const input = host.querySelector<HTMLInputElement>('input[type="password"]')!;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

async function mount() {
  await act(async () =>
    root.render(<WelcomeScreen onActivated={() => undefined} onSignedIn={() => undefined} onPaired={() => undefined} />),
  );
}

async function debounce() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(350);
  });
}

describe("WelcomeScreen", () => {
  it("opens the sign-up page from Create account", async () => {
    await mount();
    const button = [...host.querySelectorAll("button")].find((b) => b.textContent === en.welcome.createAccount)!;
    await act(async () => button.click());
    expect(openSignup).toHaveBeenCalledTimes(1);
  });

  it("says who a setup code is for once it looks like one", async () => {
    await mount();
    await act(async () => typeInvite("HKINV1-abc"));
    await debounce();
    expect(previewInvite).toHaveBeenCalledWith("HKINV1-abc");
    expect(host.textContent).toContain(en.welcome.creatingFor("me@example.com", "https://api.havenkeys.net"));
  });

  it("asks nothing for other text, and drops the line when the code is refused", async () => {
    await mount();
    await act(async () => typeInvite("hello"));
    await debounce();
    expect(previewInvite).not.toHaveBeenCalled();

    previewInvite.mockRejectedValue(new Error("bad"));
    await act(async () => typeInvite("HKINV1-zzz"));
    await debounce();
    expect(host.textContent).not.toContain("Creating an account for");
  });

  it("gauges the master password in Rust, with the code's email as an easy guess", async () => {
    await mount();
    expect(host.querySelector(".wf-strength")).toBeNull();
    typeInvite("HKINV1-abcdefghijklmnop");
    await debounce();
    typePassword("samuel2024");
    expect(host.querySelector(".wf-strength")).not.toBeNull();
    await debounce();
    expect(estimateMasterPassword).toHaveBeenCalledWith("samuel2024", ["me@example.com"]);
    expect(host.querySelector(".wf-strength")?.className).toContain("strength-weak");
    expect(host.textContent).toContain(en.welcome.strengthNote.weak);
    typePassword("");
    await debounce();
    expect(host.querySelector(".wf-strength")).toBeNull();
  });
});
