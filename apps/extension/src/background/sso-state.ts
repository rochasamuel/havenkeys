// Sign-in-with state in the background: what the user clicked (to offer
// saving it) and which pick is running. Pure: no chrome.*, no secrets, in
// memory only; cleared when the vault locks.
//
// A run moves press → choose → end, performs at most one action on the
// provider's page, only on an origin Rust returned with start_sso, only in
// the run's tab or a popup that tab opened, and lasts SSO_RUN_TTL_MS.

import { SSO_PROVIDERS, type SsoProvider } from "@havenkeys/protocol";

export const PENDING_TTL_MS = 5 * 60_000;
export const SSO_RUN_TTL_MS = 2 * 60_000;

export interface TabRef {
  tabId: number;
  openerTabId?: number;
}

export interface PendingSso {
  tabId: number;
  url: string;
  topUrl?: string;
  provider: SsoProvider;
  account: string | null;
  sawProvider: boolean;
  popupTabId: number | null;
  expires: number;
}

export interface SsoRun {
  tabId: number;
  frameId: number;
  siteOrigin: string;
  provider: SsoProvider;
  account: string | null;
  providerOrigins: string[];
  autoChoose: boolean;
  phase: "press" | "choose";
  expires: number;
}

export function createSsoState(now: () => number) {
  const pendings = new Map<number, PendingSso>(); // by the tab clicked in
  const runs = new Map<number, SsoRun>(); // by tab

  function pending(tabId: number): PendingSso | null {
    const p = pendings.get(tabId);
    if (!p) return null;
    if (p.expires <= now()) {
      pendings.delete(tabId);
      return null;
    }
    return p;
  }

  /** The pending capture this tab belongs to: its own, or its opener's. */
  function pendingOf(tab: TabRef): PendingSso | null {
    return pending(tab.tabId) ?? (tab.openerTabId === undefined ? null : pending(tab.openerTabId));
  }

  const isProviderOrigin = (p: PendingSso, origin: string) => SSO_PROVIDERS[p.provider].origins.includes(origin);

  function run(tabId: number): SsoRun | null {
    const r = runs.get(tabId);
    if (!r) return null;
    if (r.expires <= now()) {
      runs.delete(tabId);
      return null;
    }
    return r;
  }

  return {
    click(tabId: number, url: string, topUrl: string | undefined, provider: SsoProvider): void {
      const p: PendingSso = { tabId, url, provider, account: null, sawProvider: false, popupTabId: null, expires: now() + PENDING_TTL_MS };
      if (topUrl !== undefined) p.topUrl = topUrl;
      pendings.set(tabId, p);
    },
    // `top`: this is the tab's (or popup's) own top frame, not an iframe
    // embedded in the site's page (e.g. a Google GSI iframe on the site
    // itself, which never leaves the site and must not spuriously mark the
    // provider as visited).
    visit(tab: TabRef, origin: string, top: boolean): void {
      if (!top) return;
      const p = pendingOf(tab);
      if (!p || !isProviderOrigin(p, origin)) return;
      p.sawProvider = true;
      if (tab.tabId !== p.tabId) p.popupTabId = tab.tabId;
    },
    account(tab: TabRef, origin: string, account: string, top: boolean): boolean {
      if (!top) return false;
      const p = pendingOf(tab);
      if (!p || !isProviderOrigin(p, origin)) return false;
      p.account = account;
      p.sawProvider = true;
      if (tab.tabId !== p.tabId) p.popupTabId = tab.tabId;
      return true;
    },
    takeReturn(tabId: number, origin: string): PendingSso | null {
      const p = pending(tabId);
      if (!p || !p.sawProvider || isProviderOrigin(p, origin)) return null;
      pendings.delete(tabId);
      return p;
    },
    takeOnTabClosed(tabId: number): PendingSso | null {
      for (const p of pendings.values()) {
        if (p.popupTabId === tabId && p.sawProvider && p.expires > now()) {
          pendings.delete(p.tabId);
          return p;
        }
      }
      return null;
    },
    pending,
    dropPending(tabId: number): void {
      pendings.delete(tabId);
    },
    startRun(r: Omit<SsoRun, "phase" | "expires">): SsoRun {
      const full: SsoRun = { ...r, providerOrigins: [...r.providerOrigins], phase: "press", expires: now() + SSO_RUN_TTL_MS };
      runs.set(r.tabId, full);
      return full;
    },
    run,
    pressed(tabId: number): SsoRun | null {
      const r = run(tabId);
      if (!r || r.phase !== "press") return null;
      if (!r.autoChoose || r.account === null) {
        runs.delete(tabId);
        return null;
      }
      r.phase = "choose";
      return r;
    },
    chooseFor(tab: TabRef, origin: string): string | null {
      const own = run(tab.tabId);
      const r = own ?? (tab.openerTabId === undefined ? null : run(tab.openerTabId));
      if (!r || r.phase !== "choose" || r.account === null || !r.providerOrigins.includes(origin)) return null;
      runs.delete(r.tabId);
      return r.account;
    },
    topLoad(tabId: number, origin: string): void {
      const r = run(tabId);
      if (r && origin !== r.siteOrigin && !r.providerOrigins.includes(origin)) runs.delete(tabId);
    },
    endRun(tabId: number): void {
      runs.delete(tabId);
    },
    forgetTab(tabId: number): void {
      pendings.delete(tabId);
      runs.delete(tabId);
    },
    clear(): void {
      pendings.clear();
      runs.clear();
    },
  };
}

export type SsoState = ReturnType<typeof createSsoState>;
