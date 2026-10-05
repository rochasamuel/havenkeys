// Sign-in-with state in the background: what the user clicked (to offer
// saving it) and which pick is running. Pure: no chrome.*, no secrets, in
// memory only; cleared when the vault locks.
//
// A run moves press → choose → login → end. On the provider's page it
// chooses the account once and completes one login, only on an origin Rust
// returned with start_sso, only in the run's tab or a popup that tab
// opened, and lasts SSO_RUN_TTL_MS.

import { SSO_PROVIDERS, type SsoProvider } from "@havenkeys/protocol";
import { isAccount } from "../messaging/sso";

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
  /** Learned on the provider's own page (a chooser row, a username step). */
  account: string | null;
  /** A `login_hint` from a URL the tab or its popup loaded after the click.
   * The site chose it: a suggestion only, weaker than `account`. */
  hint: string | null;
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
  phase: "press" | "choose" | "login";
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

  /** The run for this tab or its opener, if the origin is one of its provider origins. */
  function runFor(tab: TabRef, origin: string): SsoRun | null {
    const r = run(tab.tabId) ?? (tab.openerTabId === undefined ? null : run(tab.openerTabId));
    return r && r.providerOrigins.includes(origin) ? r : null;
  }

  return {
    /** `sawProvider`: the click was inside the provider's own button frame
     * (Google's gsi/button), which already is the provider's page. Its popup
     * gets no opener tab in Chrome, so nothing later would tell us. */
    click(tabId: number, url: string, topUrl: string | undefined, provider: SsoProvider, sawProvider = false): void {
      const p: PendingSso = { tabId, url, provider, account: null, hint: null, sawProvider, popupTabId: null, expires: now() + PENDING_TTL_MS };
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
    /** A tab opened by the clicked tab (the site's login popup). Its closing is a
     * return even if no provider page loaded in it: a signed-in, consented
     * provider may approve with redirects alone. */
    opened(tab: TabRef): void {
      if (tab.openerTabId === undefined) return;
      const p = pending(tab.openerTabId);
      if (p) p.popupTabId = tab.tabId;
    },
    /** A URL the clicked tab, or a popup it opened, is loading: note its
     * OAuth/OpenID `login_hint`, if email-shaped. Any origin: sites often
     * pass it through their own IdP first. */
    hint(tab: TabRef, url: string): void {
      const p = pending(tab.tabId) ?? (tab.openerTabId === undefined ? null : pending(tab.openerTabId));
      if (!p) return;
      let value: string | null;
      try {
        value = new URL(url).searchParams.get("login_hint");
      } catch {
        return;
      }
      const hint = value?.trim().toLowerCase();
      if (hint && isAccount(hint)) p.hint = hint;
    },
    /** The account a provider's personalized button frame shows: a suggestion, like a login_hint. */
    buttonHint(tabId: number, provider: SsoProvider, account: string): void {
      const p = pending(tabId);
      const hint = account.trim().toLowerCase();
      if (p && p.provider === provider && isAccount(hint)) p.hint = hint;
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
        if (p.popupTabId === tabId && p.expires > now()) {
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
      const r = runFor(tab, origin);
      if (!r || r.phase !== "choose" || r.account === null) return null;
      r.phase = "login";
      return r.account;
    },
    /** A later provider document while the run waits for its login form. */
    loginReady(tab: TabRef, origin: string): boolean {
      return runFor(tab, origin)?.phase === "login";
    },
    /** cs_sso_login: the run to complete, consumed. */
    loginFor(tab: TabRef, origin: string): SsoRun | null {
      const r = runFor(tab, origin);
      if (!r || r.phase !== "login") return null;
      runs.delete(r.tabId);
      return r;
    },
    /** A top-frame load in the run's tab: leaving for an unrelated origin ends
     * the run, and so does coming back to the site once the run reached
     * `login` (the provider finished). Before that the site may reload first. */
    topLoad(tabId: number, origin: string): void {
      const r = run(tabId);
      if (!r || r.providerOrigins.includes(origin)) return;
      if (origin !== r.siteOrigin || r.phase === "login") runs.delete(tabId);
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
