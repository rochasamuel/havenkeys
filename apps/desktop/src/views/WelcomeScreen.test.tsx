// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const previewInvite = vi.fn<(invite: string) => Promise<{ email: string; serverUrl: string }>>();
const openSignup = vi.fn<() => Promise<void>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    previewInvite: (i: string) => previewInvite(i),
    openSignup: () => openSignup(),
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
});
