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
  /** The provider popup tied to this run without an opener tab (`popupFor`). */
  popupTabId?: number;
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

  /** The pending capture this tab belongs to: its own, its opener's, or the
   * one whose provider popup it is (tied by `popupFor`). */
  function pendingOf(tab: TabRef): PendingSso | null {
    const own = pending(tab.tabId) ?? (tab.openerTabId === undefined ? null : pending(tab.openerTabId));
    if (own) return own;
    for (const p of pendings.values()) if (p.popupTabId === tab.tabId && p.expires > now()) return p;
    return null;
  }

  /**
   * A provider page opened without an opener tab (Firefox gives Google's
   * popup none) names the site it signs in to: Google's OAuth popup in its
   * `origin` parameter, any OAuth/OpenID provider in `redirect_uri` (where
   * it sends the result). Tie it to that site's pending click: exactly one
   * pending for that provider and site, else nothing. A site whose
   * redirect_uri is on another origin (its own auth domain) is not tied.
   */
  function popupFor(tab: TabRef, url: string): void {
    if (tab.openerTabId !== undefined || runs.has(tab.tabId)) return;
    let u: URL;
    try {
      u = new URL(url);
    } catch {
      return;
    }
    const site = namedSite(u);
    if (site === null) return;
    const hits = [...pendings.values()].filter((p) => {
      if (p.expires <= now() || !isProviderOrigin(p, u.origin)) return false;
      try {
        return new URL(p.url).origin === site;
      } catch {
        return false;
      }
    });
    if (hits.length === 1 && hits[0] && !pendingOf(tab)) hits[0].popupTabId = tab.tabId;
    runPopupFor(tab, u.origin, site);
  }

  /**
   * The same for a run: Firefox opens Google's popup without an opener tab,
   * so the run's chooser and login steps would never see it. A run
   * whose provider origins include the page's and whose site is the page's
   * `origin` parameter takes the tab; exactly one such run, else nothing.
   */
  function runPopupFor(tab: TabRef, pageOrigin: string, site: string): void {
    const hits = [...runs.values()].filter(
      (r) => r.expires > now() && r.tabId !== tab.tabId && r.siteOrigin === site && r.providerOrigins.includes(pageOrigin),
    );
    if (hits.length === 1 && hits[0]) hits[0].popupTabId = tab.tabId;
  }

  /** The site a provider URL names: `origin`, else `redirect_uri`; an http(s) origin, or null. */
  function namedSite(u: URL): string | null {
    for (const name of ["origin", "redirect_uri"]) {
      const v = u.searchParams.get(name);
      if (!v) continue;
      try {
        const o = new URL(v);
        if (o.protocol === "https:" || o.protocol === "http:") return o.origin;
      } catch {
        // not a URL: try the next one
      }
    }
    return null;
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

  /** The run for this tab, its opener, or the run it is the tied popup of, if the origin is one of its provider origins. */
  function runFor(tab: TabRef, origin: string): SsoRun | null {
    const r = run(tab.tabId) ?? (tab.openerTabId === undefined ? null : run(tab.openerTabId)) ?? tiedRun(tab.tabId);
    return r && r.providerOrigins.includes(origin) ? r : null;
  }

  function tiedRun(tabId: number): SsoRun | null {
    for (const r of runs.values()) if (r.popupTabId === tabId) return run(r.tabId);
    return null;
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
      popupFor(tab, url);
      const p = pendingOf(tab);
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
    /** The tab whose run `tab` belongs to: its own, its opener's, or the one it is the tied popup of. */
    runTabOf(tab: TabRef): number | null {
      if (run(tab.tabId)) return tab.tabId;
      if (tab.openerTabId !== undefined && run(tab.openerTabId)) return tab.openerTabId;
      return tiedRun(tab.tabId)?.tabId ?? null;
    },
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
