// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { QuickAccessStatus } from "../lib/types";

const quickAccessStatus = vi.fn<() => Promise<QuickAccessStatus>>();
const setQuickAccess = vi.fn<(enabled: boolean) => Promise<QuickAccessStatus>>();
const setQuickAccessShortcut = vi.fn<(keys: string | null) => Promise<QuickAccessStatus>>();
const status = (over: Partial<QuickAccessStatus> = {}): QuickAccessStatus => ({
  enabled: true,
  inUse: false,
  keys: "Control+Shift+Space",
  isDefault: true,
  candidateInUse: false,
  ...over,
});

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    quickAccessStatus: () => quickAccessStatus(),
    setQuickAccess: (e: boolean) => setQuickAccess(e),
    setQuickAccessShortcut: (k: string | null) => setQuickAccessShortcut(k),
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
import { ApiError } from "../lib/api";
import { ToastProvider } from "../components/Toast";
import { SettingsView } from "./SettingsView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;
beforeEach(() => {
  document.documentElement.dataset.platform = "";
  quickAccessStatus.mockReset().mockResolvedValue(status());
  setQuickAccess.mockReset().mockImplementation(async (enabled) => status({ enabled }));
  setQuickAccessShortcut
    .mockReset()
    .mockImplementation(async (keys) => status({ keys: keys ?? "Control+Shift+Space", isDefault: keys === null }));
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
    quickAccessStatus.mockResolvedValue(status({ keys: "Super+Shift+Space" }));
    await render();
    expect(host.textContent).toContain(en.settings.quickAccessNote("⌘⇧Space"));
  });

  it("says when another app owns the shortcut, and stays on", async () => {
    quickAccessStatus.mockResolvedValue(status({ inUse: true }));
    await render();
    expect(toggle()!.checked).toBe(true);
    expect(host.querySelector(".quick-in-use [role=status]")?.textContent).toBe(en.settings.quickAccessInUse);
  });

  it("turns it off and on through Rust", async () => {
    await render();
    await act(async () => toggle()!.click());
    expect(setQuickAccess).toHaveBeenCalledWith(false);
    expect(toggle()!.checked).toBe(false);
    setQuickAccess.mockResolvedValue(status({ inUse: true }));
    await act(async () => toggle()!.click());
    expect(setQuickAccess).toHaveBeenLastCalledWith(true);
    expect(host.textContent).toContain(en.settings.quickAccessInUse);
  });
});

const recorder = () => host.querySelector<HTMLInputElement>(`input[aria-label="${en.settings.quickAccessRecordLabel}"]`);
const button = (text: string) => Array.from(host.querySelectorAll("button")).find((b) => b.textContent === text);
const alertText = () => host.querySelector("[role=alert]")?.textContent;

async function key(code: string, mods: KeyboardEventInit = {}) {
  await act(async () => {
    recorder()!.dispatchEvent(new KeyboardEvent("keydown", { code, key: code, bubbles: true, cancelable: true, ...mods }));
  });
}

async function startRecording() {
  await render();
  await act(async () => button(en.settings.quickAccessChange)!.click());
}

describe("Settings → Quick Access shortcut", () => {
  it("shows the chord, and Reset only when it is not the default", async () => {
    await render();
    expect(host.querySelector(".shortcut-keys")?.textContent).toBe("Ctrl+Shift+Space");
    expect(button(en.settings.quickAccessReset)).toBeUndefined();
    act(() => root.unmount());
    root = createRoot(host);
    quickAccessStatus.mockResolvedValue(status({ keys: "Alt+Shift+K", isDefault: false }));
    await render();
    expect(host.querySelector(".shortcut-keys")?.textContent).toBe("Alt+Shift+K");
    await act(async () => button(en.settings.quickAccessReset)!.click());
    expect(setQuickAccessShortcut).toHaveBeenCalledWith(null);
    expect(host.querySelector(".shortcut-keys")?.textContent).toBe("Ctrl+Shift+Space");
  });

  it("shows the macOS symbols", async () => {
    document.documentElement.dataset.platform = "mac";
    quickAccessStatus.mockResolvedValue(status({ keys: "Super+Shift+Space" }));
    await render();
    expect(host.querySelector(".shortcut-keys")?.textContent).toBe("⌘⇧Space");
  });

  it("hides the row while the switch is off", async () => {
    quickAccessStatus.mockResolvedValue(status({ enabled: false }));
    await render();
    expect(button(en.settings.quickAccessChange)).toBeUndefined();
  });

  it("starts recording and cancels with Esc", async () => {
    await startRecording();
    expect(document.activeElement).toBe(recorder());
    expect(recorder()!.placeholder).toBe(en.settings.quickAccessRecording);
    await key("Escape", { key: "Escape" });
    expect(recorder()).toBeNull();
    expect(setQuickAccessShortcut).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(button(en.settings.quickAccessChange));
  });

  it("keeps waiting on a modifier alone", async () => {
    await startRecording();
    await key("ControlLeft", { key: "Control", ctrlKey: true });
    await key("ShiftLeft", { key: "Shift", ctrlKey: true, shiftKey: true });
    expect(setQuickAccessShortcut).not.toHaveBeenCalled();
    expect(recorder()).not.toBeNull();
  });

  it("sends a chord once, and shows it when accepted", async () => {
    let answer: (s: QuickAccessStatus) => void = () => undefined;
    setQuickAccessShortcut.mockImplementation(() => new Promise((r) => (answer = r)));
    await startRecording();
    await key("KeyK", { altKey: true, shiftKey: true });
    await key("KeyK", { altKey: true, shiftKey: true, repeat: true });
    await key("KeyJ", { altKey: true });
    expect(setQuickAccessShortcut).toHaveBeenCalledTimes(1);
    expect(setQuickAccessShortcut).toHaveBeenCalledWith("Alt+Shift+K");
    await act(async () => answer(status({ keys: "Alt+Shift+K", isDefault: false })));
    expect(recorder()).toBeNull();
    expect(host.querySelector(".shortcut-keys")?.textContent).toBe("Alt+Shift+K");
    expect(button(en.settings.quickAccessReset)).toBeDefined();
  });

  it("says why Rust refused, and keeps recording", async () => {
    setQuickAccessShortcut.mockRejectedValue(new ApiError("quick_access_shortcut_invalid", "Reserved by the system."));
    await startRecording();
    await key("KeyC", { ctrlKey: true });
    expect(setQuickAccessShortcut).toHaveBeenCalledWith("Control+C");
    expect(alertText()).toBe("Reserved by the system.");
    expect(recorder()).not.toBeNull();
  });

  it("says when another app owns the new chord, and keeps recording and the old one", async () => {
    setQuickAccessShortcut.mockResolvedValue(status({ candidateInUse: true }));
    await startRecording();
    await key("KeyK", { altKey: true });
    expect(alertText()).toBe(en.settings.quickAccessCandidateInUse);
    expect(recorder()).not.toBeNull();
    await key("Escape", { key: "Escape" });
    expect(host.querySelector(".shortcut-keys")?.textContent).toBe("Ctrl+Shift+Space");
  });

  it("refuses an unusable key without asking Rust", async () => {
    await startRecording();
    await key("Tab", { key: "Tab", ctrlKey: true });
    expect(setQuickAccessShortcut).not.toHaveBeenCalled();
    expect(alertText()).toBe(en.errors.shortcutRefused["This key can't be used."]);
    expect(recorder()).not.toBeNull();
  });

  it("keeps the keys away from the app's own shortcuts", async () => {
    const seen = vi.fn();
    window.addEventListener("keydown", seen);
    try {
      await startRecording();
      await key("KeyL", { key: "l", ctrlKey: true });
      expect(seen).not.toHaveBeenCalled();
      expect(setQuickAccessShortcut).toHaveBeenCalledWith("Control+L");
    } finally {
      window.removeEventListener("keydown", seen);
    }
  });

  it("offers another shortcut from the in-use warning", async () => {
    quickAccessStatus.mockResolvedValue(status({ inUse: true }));
    await render();
    await act(async () => button(en.settings.quickAccessChooseAnother)!.click());
    expect(document.activeElement).toBe(recorder());
  });

  it("closes the recorder when a refusal arrives after focus left it", async () => {
    let fail: (e: unknown) => void = () => undefined;
    setQuickAccessShortcut.mockImplementation(() => new Promise((_, reject) => (fail = reject)));
    const elsewhere = document.createElement("button");
    document.body.append(elsewhere);
    try {
      await startRecording();
      await key("KeyC", { ctrlKey: true });
      await act(async () => elsewhere.focus());
      expect(recorder()).not.toBeNull();
      await act(async () => fail(new ApiError("quick_access_shortcut_invalid", "Reserved by the system.")));
      expect(recorder()).toBeNull();
      expect(alertText()).toBe("Reserved by the system.");
      expect(document.activeElement).toBe(elsewhere);
    } finally {
      elsewhere.remove();
    }
  });

  it("closes the recorder when a taken answer arrives after focus left it", async () => {
    let answer: (s: QuickAccessStatus) => void = () => undefined;
    setQuickAccessShortcut.mockImplementation(() => new Promise((r) => (answer = r)));
    const elsewhere = document.createElement("button");
    document.body.append(elsewhere);
    try {
      await startRecording();
      await key("KeyK", { altKey: true });
      await act(async () => elsewhere.focus());
      await act(async () => answer(status({ candidateInUse: true })));
      expect(recorder()).toBeNull();
      expect(alertText()).toBe(en.settings.quickAccessCandidateInUse);
    } finally {
      elsewhere.remove();
    }
  });

  it("disables the other controls while a request is out", async () => {
    let answer: (s: QuickAccessStatus) => void = () => undefined;
    quickAccessStatus.mockResolvedValue(status({ keys: "Alt+Shift+K", isDefault: false, inUse: true }));
    setQuickAccessShortcut.mockImplementation(() => new Promise((r) => (answer = r)));
    await render();
    await act(async () => button(en.settings.quickAccessReset)!.click());
    expect(button(en.settings.quickAccessChange)!.disabled).toBe(true);
    expect(button(en.settings.quickAccessChooseAnother)!.disabled).toBe(true);
    expect(button(en.settings.quickAccessReset)!.disabled).toBe(true);
    expect(toggle()!.disabled).toBe(true);
    await act(async () => answer(status()));
    expect(button(en.settings.quickAccessChange)!.disabled).toBe(false);
    expect(toggle()!.disabled).toBe(false);
  });

  it("says Saved after a reset, but not when the chord is still taken", async () => {
    const toastText = () => host.querySelector(".toast-region")?.textContent ?? "";
    quickAccessStatus.mockResolvedValue(status({ keys: "Alt+Shift+K", isDefault: false }));
    setQuickAccessShortcut.mockResolvedValue(status({ inUse: true }));
    await render();
    await act(async () => button(en.settings.quickAccessReset)!.click());
    expect(toastText()).not.toContain(en.settings.saved);
    expect(host.querySelector(".quick-in-use [role=status]")?.textContent).toBe(en.settings.quickAccessInUse);
    act(() => root.unmount());
    root = createRoot(host);
    setQuickAccessShortcut.mockResolvedValue(status());
    await render();
    await act(async () => button(en.settings.quickAccessReset)!.click());
    expect(toastText()).toContain(en.settings.saved);
  });
});
