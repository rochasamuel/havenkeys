// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ItemOverview, ProviderLogin } from "../lib/types";

const providerLogin = vi.fn<(id: string) => Promise<ProviderLogin>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: { providerLogin: (id: string) => providerLogin(id) },
}));

import { en } from "../i18n/en";
import { SsoRow } from "./SsoRow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const vercel: ItemOverview = {
  id: "vercel", itemType: "login", title: "Vercel", username: null, urls: [], hasPassword: false, hasTotp: false,
  hasNotes: false, hasPasskey: false, autoSignIn: true, createdAt: 0, updatedAt: 0,
  signInWith: { provider: "google", account: "me@gmail.com" },
};

let host: HTMLElement;
let root: Root;
beforeEach(() => {
  providerLogin.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("SsoRow", () => {
  it("opens the one provider login", async () => {
    providerLogin.mockResolvedValue({ kind: "one", id: "google" });
    const onOpen = vi.fn();
    await act(async () => root.render(<SsoRow item={vercel} onOpen={onOpen} />));
    expect(providerLogin).toHaveBeenCalledWith("vercel");
    const open = host.querySelector<HTMLButtonElement>(`button[aria-label="${en.detail.openProviderLogin("Google")}"]`)!;
    await act(async () => open.click());
    expect(onOpen).toHaveBeenCalledWith("google");
    expect(host.textContent).not.toContain(en.detail.noProviderLogin("Google"));
  });
  it("says when no saved login matches", async () => {
    providerLogin.mockResolvedValue({ kind: "none" });
    await act(async () => root.render(<SsoRow item={vercel} onOpen={() => {}} />));
    expect(host.textContent).toContain(en.detail.noProviderLogin("Google"));
    expect(host.querySelector("button")).toBeNull();
  });
  it("says when several saved logins match", async () => {
    providerLogin.mockResolvedValue({ kind: "several" });
    await act(async () => root.render(<SsoRow item={vercel} onOpen={() => {}} />));
    expect(host.textContent).toContain(en.detail.severalProviderLogins("Google"));
  });
  it("shows no note for a provider without an account, or on an error", async () => {
    providerLogin.mockResolvedValue({ kind: "none" });
    const bare = { ...vercel, signInWith: { provider: "github" as const, account: null } };
    await act(async () => root.render(<SsoRow item={bare} onOpen={() => {}} />));
    expect(host.textContent).toContain("GitHub");
    expect(host.textContent).not.toContain(en.detail.noProviderLogin("GitHub"));
    providerLogin.mockRejectedValue(new Error("locked"));
    await act(async () => root.render(<SsoRow item={{ ...vercel, id: "other" }} onOpen={() => {}} />));
    expect(host.textContent).not.toContain(en.detail.noProviderLogin("Google"));
  });
});
