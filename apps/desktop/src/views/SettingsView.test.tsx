// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { QuickAccessStatus } from "../lib/types";

const quickAccessStatus = vi.fn<() => Promise<QuickAccessStatus>>();
const setQuickAccess = vi.fn<(enabled: boolean) => Promise<QuickAccessStatus>>();

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    quickAccessStatus: () => quickAccessStatus(),
    setQuickAccess: (e: boolean) => setQuickAccess(e),
    // The rest of Settings stays hidden: no settings, no launch-at-login.
    getSettings: () => Promise.reject(new Error("not in this test")),
    launchAtLogin: () => Promise.reject(new Error("not in this test")),
    // AccountSection reads `secretKeyStorage` off the answer; no device here.
    deviceStatus: () => Promise.reject(new Error("not in this test")),
  };
  return {
    ...original,
    api: new Proxy(known, {
      get: (target, name: string) =>
        name in target
          ? target[name]
          : name.startsWith("on")
            ? () => Promise.resolve(() => undefined)
            : () => Promise.resolve(null),
    }),
  };
});

import { en } from "../i18n/en";
import { ToastProvider } from "../components/Toast";
import { SettingsView } from "./SettingsView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;
beforeEach(() => {
  document.documentElement.dataset.platform = "";
  quickAccessStatus.mockReset().mockResolvedValue({ enabled: true, inUse: false });
  setQuickAccess.mockReset().mockImplementation(async (enabled) => ({ enabled, inUse: false }));
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function render() {
  await act(async () =>
    root.render(
      <ToastProvider>
        <SettingsView onImported={() => undefined} online frozen={false} />
      </ToastProvider>,
    ),
  );
}
const toggle = () => host.querySelector<HTMLInputElement>(`input[aria-label="${en.settings.quickAccessShortcutLabel}"]`);

describe("Settings → Quick Access", () => {
  it("shows the switch on, with this platform's keys", async () => {
    await render();
    expect(toggle()!.checked).toBe(true);
    expect(host.textContent).toContain(en.settings.quickAccessNote("Ctrl+Shift+Space"));
    expect(host.textContent).not.toContain(en.settings.quickAccessInUse);
  });

  it("names Cmd on macOS", async () => {
    document.documentElement.dataset.platform = "mac";
    await render();
    expect(host.textContent).toContain(en.settings.quickAccessNote("⌘⇧Space"));
  });

  it("says when another app owns the shortcut, and stays on", async () => {
    quickAccessStatus.mockResolvedValue({ enabled: true, inUse: true });
    await render();
    expect(toggle()!.checked).toBe(true);
    expect(host.querySelector("[role=status]")?.textContent).toBe(en.settings.quickAccessInUse);
  });

  it("turns it off and on through Rust", async () => {
    await render();
    await act(async () => toggle()!.click());
    expect(setQuickAccess).toHaveBeenCalledWith(false);
    expect(toggle()!.checked).toBe(false);
    setQuickAccess.mockResolvedValue({ enabled: true, inUse: true });
    await act(async () => toggle()!.click());
    expect(setQuickAccess).toHaveBeenLastCalledWith(true);
    expect(host.textContent).toContain(en.settings.quickAccessInUse);
  });
});
