// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SignInWith, SsoAccount, SsoProvider } from "../lib/types";

const ssoAccounts = vi.fn<(p: SsoProvider) => Promise<SsoAccount[]>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: { ssoAccounts: (p: SsoProvider) => ssoAccounts(p) },
}));

import { en } from "../i18n/en";
import { SsoPicker } from "./SsoPicker";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const g1: SsoAccount = { id: "g1", title: "google.com", username: "samuelsilv.rocha@gmail.com" };
let host: HTMLElement;
let root: Root;
let value: SignInWith | null;
const onChange = vi.fn((v: SignInWith | null) => {
  value = v;
});

beforeEach(() => {
  value = null;
  onChange.mockClear();
  ssoAccounts.mockReset().mockImplementation(async (p) => (p === "google" ? [g1] : []));
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const trigger = () => host.querySelector<HTMLButtonElement>("button.sso-trigger")!;
const options = () => [...host.querySelectorAll<HTMLElement>('[role="option"]')];
const search = () => host.querySelector<HTMLInputElement>('input[type="search"]')!;
function type(v: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(search(), v);
  search().dispatchEvent(new Event("input", { bubbles: true }));
}
const key = (k: string) => search().dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true }));

describe("SsoPicker", () => {
  it("does not cache a failed load: the next open retries the failed providers", async () => {
    ssoAccounts.mockReset().mockRejectedValue(new Error("locked"));
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    expect(options().some((o) => o.textContent?.includes("samuelsilv.rocha@gmail.com"))).toBe(false);
    expect(options().length).toBeGreaterThan(0);
    await act(async () => trigger().click());
    ssoAccounts.mockReset().mockImplementation(async (p) => (p === "google" ? [g1] : []));
    await act(async () => trigger().click());
    expect(ssoAccounts).toHaveBeenCalledTimes(9);
    expect(options().some((o) => o.textContent?.includes("samuelsilv.rocha@gmail.com"))).toBe(true);
  });
  it("lists providers with their saved logins and copies the picked login's username", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    expect(trigger().textContent).toContain(en.editor.providerNone);
    await act(async () => trigger().click());
    expect(ssoAccounts).toHaveBeenCalledTimes(9);
    const row = options().find((o) => o.textContent?.includes("samuelsilv.rocha@gmail.com"))!;
    await act(async () => row.click());
    expect(onChange).toHaveBeenLastCalledWith({ provider: "google", account: "samuelsilv.rocha@gmail.com" });
    expect(host.querySelector('[role="listbox"]')).toBeNull();
  });
  it("shows the stored value of an existing login and loads accounts only once", async () => {
    value = { provider: "github", account: "rochasamuel" };
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    expect(trigger().textContent).toContain("GitHub");
    expect(trigger().textContent).toContain("rochasamuel");
    await act(async () => trigger().click());
    await act(async () => key("Escape"));
    await act(async () => trigger().click());
    expect(ssoAccounts).toHaveBeenCalledTimes(9);
    expect(onChange).not.toHaveBeenCalled();
  });
  it("filters, moves with arrows, picks with Enter", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    await act(async () => type("goo"));
    expect(options().length).toBe(2);
    await act(async () => key("ArrowDown"));
    await act(async () => key("Enter"));
    expect(onChange).toHaveBeenLastCalledWith({ provider: "google", account: "samuelsilv.rocha@gmail.com" });
  });
  it("says No matches; Enter does nothing; Escape keeps the value", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    await act(async () => type("zzz"));
    expect(options().length).toBe(0);
    expect(host.textContent).toContain(en.editor.noMatches);
    await act(async () => key("Enter"));
    await act(async () => key("Escape"));
    expect(onChange).not.toHaveBeenCalled();
    expect(host.querySelector('[role="listbox"]')).toBeNull();
  });
  it("still lists the providers when the vault is locked", async () => {
    ssoAccounts.mockRejectedValue(new Error("locked"));
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    expect(options().map((o) => o.textContent)).toEqual(
      expect.arrayContaining(["Apple", "Discord", "Facebook", "GitHub", "GitLab", "Google", "LinkedIn", "Microsoft", "X"]),
    );
  });
  it("is a combobox; focus returns to the trigger after pick and Escape", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    expect(search().getAttribute("role")).toBe("combobox");
    await act(async () => key("Escape"));
    expect(document.activeElement).toBe(trigger());
    await act(async () => trigger().click());
    await act(async () => options()[0]!.click());
    expect(document.activeElement).toBe(trigger());
  });
  it("ArrowDown with no rows then Enter does nothing", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    await act(async () => type("zzz"));
    await act(async () => key("ArrowDown"));
    await act(async () => key("Enter"));
    expect(onChange).not.toHaveBeenCalled();
  });
  it("reopening while the first load is pending does not load again", async () => {
    let release: (a: SsoAccount[]) => void = () => {};
    ssoAccounts.mockReset().mockImplementation(() => new Promise<SsoAccount[]>((r) => (release = r)));
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    await act(async () => key("Escape"));
    await act(async () => trigger().click());
    expect(ssoAccounts.mock.calls.length).toBeLessThanOrEqual(9);
    await act(async () => release([]));
  });
});
