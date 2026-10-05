// "Sign in with" in the page: notice the site's provider buttons (to offer a
// saved sign-in), notice the user's own clicks on them (to offer saving),
// press the button after the user picked, and, on the provider's page during
// a run, click the saved account on the chooser, or "Use another account",
// and hand the login form to the background.
//
// Bounded work: one scan per debounce, at most MAX_SSO_CANDIDATES elements,
// observation for SCAN_WINDOW_MS after load or a URL change. Page strings
// are only compared to fixed lists; nothing is logged or stored.

import { buttonFrameProvider, providersForOrigin } from "@havenkeys/protocol";
import { defaultEnv } from "../autofill/group";
import { findLoginGroup } from "../autofill/page";
import { anotherAccountButton, chooserRow, emailIn, findProviderButtons, isConsentScreen, providerButton, providerOf, SSO_CANDIDATES, SSO_MIN_SCORE } from "../autofill/sso";
import { hasChallenge } from "../autofill/submit";
import { TOKEN } from "../messaging/inline";
import type { BackgroundToSso, SsoContentRequest, SsoPressReply, SsoReady } from "../messaging/sso";
import { InlineFrame, ssoBox } from "./frames";

export const SCAN_DEBOUNCE_MS = 750;
export const SCAN_WINDOW_MS = 60_000;
export const CHOOSE_WAIT_MS = 10_000;
const CHOOSE_DEBOUNCE_MS = 300;
/** An account row on a provider's chooser that is not itself a button or link. */
const ACCOUNT_ROW = "[data-identifier], [data-email]";

export function createSsoContent(deps: {
  send(msg: SsoContentRequest): Promise<unknown>;
  viewport(): { width: number; height: number };
  suggestions(): boolean;
  isTop: boolean;
}) {
  let frame: InlineFrame | null = null;
  /** The user (or the page) closed the balloon here: no offer again until the URL changes.
   * Keyed on origin + pathname, not the full href: a query or hash change alone must not
   * bring the offer back. */
  let dismissedAt: string | null = null;
  let lastSent = "";
  let href = location.href;
  let scanUntil = 0;
  let scanTimer: ReturnType<typeof setTimeout> | null = null;
  let observer: MutationObserver | null = null;
  /** watchPage() is in effect (suggestions on); teardown() ends it. */
  let watching = false;
  let listening = false;
  /** We pressed a provider button for a run: the user's next input ends it. */
  let pressed = false;
  /** A provider document of a run (a non-null SsoReady): until the hand-off, the
   * user's input, giving up or a consent screen ends the background's run. */
  let providerRun = false;
  let chooseStop: (() => void) | null = null;

  function closeFrame(): void {
    frame?.remove();
    frame = null;
  }

  /** Origin + pathname, ignoring query and hash (see dismissedAt). */
  const pageKey = () => location.origin + location.pathname;

  function show(token: string): void {
    if (!deps.isTop) return;
    closeFrame();
    frame = new InlineFrame("sso.html", token, ssoBox(deps.viewport()), () => {
      if (frame?.token === token) {
        closeFrame();
        dismissedAt = pageKey(); // the page removed it: do not offer again here
      }
    });
  }

  const offerable = () => watching && deps.suggestions() && !frame && dismissedAt !== pageKey();

  async function scan(): Promise<void> {
    scanTimer = null;
    if (!offerable()) return;
    const providers = [...findProviderButtons(document, defaultEnv()).keys()].sort();
    const key = providers.join(",");
    if (key === "" || key === lastSent) return;
    lastSent = key;
    const reply = (await deps.send({ type: "cs_sso_buttons", providers })) as { ok?: unknown; token?: unknown } | undefined;
    // Suggestions turned off, the balloon was closed, or the page moved on while we asked.
    if (!offerable()) return;
    if (reply?.ok === true && typeof reply.token === "string" && TOKEN.test(reply.token)) show(reply.token);
  }

  function stopObserving(): void {
    observer?.disconnect();
    observer = null;
  }

  function schedule(): void {
    if (location.href !== href) {
      href = location.href;
      lastSent = "";
      scanUntil = Date.now() + SCAN_WINDOW_MS;
    }
    if (Date.now() > scanUntil) {
      stopObserving();
      return;
    }
    if (scanTimer === null) scanTimer = setTimeout(() => void scan(), SCAN_DEBOUNCE_MS);
  }

  function observe(): void {
    scanUntil = Date.now() + SCAN_WINDOW_MS;
    schedule();
    if (observer) return;
    observer = new MutationObserver(schedule);
    observer.observe(document.documentElement, { childList: true, subtree: true });
  }

  /** A route change: look again for a while (the observer may have stopped). */
  function onRoute(): void {
    if (!watching) return;
    if (!observer) observe();
    else schedule();
  }

  function watchPage(): void {
    if (!deps.isTop || watching) return;
    watching = true;
    observe();
    if (!listening) {
      listening = true;
      window.addEventListener("popstate", onRoute);
      window.addEventListener("hashchange", onRoute);
    }
  }

  function cancelChoose(): void {
    chooseStop?.();
    chooseStop = null;
  }

  /** Stop the wait and end the background's run (once per document). */
  function endRun(): void {
    cancelChoose();
    if (pressed || providerRun) {
      pressed = false;
      providerRun = false;
      void deps.send({ type: "cs_sso_stop" });
    }
  }

  /**
   * On the provider's page during a run: choose the saved account (or "Use
   * another account"), then hand the login form to the background, which
   * fills the provider login and signs in (spec 2026-09-29 §3). One click of
   * each kind, one hand-off, within CHOOSE_WAIT_MS; any user input, giving
   * up or a consent screen ends the run (cs_sso_stop).
   * `account` is set on a `choose` document, null on a `login` document.
   */
  function providerStep(account: string | null): void {
    cancelChoose();
    let timer: ReturnType<typeof setTimeout> | null = null;
    let chose = account === null;
    /** Consecutive attempts that saw "Use another account" but not the saved row. */
    let seenAnother = 0;
    const attempt = () => {
      timer = null;
      const env = defaultEnv();
      // A permissions screen: never pressed, and the run is over.
      if (isConsentScreen(document, env)) return endRun();
      if (!chose && account !== null) {
        const row = chooserRow(document, account, env);
        if (row) {
          chose = true;
          row.click();
          return; // the provider moves on: a new document, or a password step here
        }
        // A chooser may render "Use another account" before its account
        // rows: click it only once two consecutive attempts saw it without
        // the saved row (the row wins if it appears meanwhile).
        const other = anotherAccountButton(document, env);
        seenAnother = other ? seenAnother + 1 : 0;
        if (other && seenAnother >= 2) {
          chose = true;
          other.click();
          return;
        }
        if (other) {
          if (timer === null) timer = setTimeout(attempt, CHOOSE_DEBOUNCE_MS);
          return;
        }
      }
      // Neither the row nor "Use another account" but a login form (the
      // provider signed out entirely) falls through to the hand-off too.
      if (findLoginGroup(document, env)) {
        cancelChoose();
        providerRun = false; // the background consumes the run on this request
        void deps.send({ type: "cs_sso_login" });
      }
    };
    const mo = new MutationObserver(() => {
      if (timer === null) timer = setTimeout(attempt, CHOOSE_DEBOUNCE_MS);
    });
    mo.observe(document.documentElement, { childList: true, subtree: true });
    const giveUp = setTimeout(endRun, CHOOSE_WAIT_MS);
    chooseStop = () => {
      mo.disconnect();
      clearTimeout(giveUp);
      if (timer !== null) clearTimeout(timer);
    };
    attempt();
  }

  return {
    watchPage,
    onTrustedClick(target: Element): void {
      const button = target.closest(SSO_CANDIDATES);
      // Inside a provider's embedded button frame, the frame says which
      // provider, whatever the label. Elsewhere the user chose this element,
      // so a bare "Google" counts (context = true).
      const framed = deps.isTop ? null : buttonFrameProvider(location.href);
      const hit = !button ? null : framed ? { provider: framed, score: SSO_MIN_SCORE } : providerOf(button, true);
      if (hit && hit.score >= SSO_MIN_SCORE) void deps.send({ type: "cs_sso_click", provider: hit.provider });
      if (providersForOrigin(location.origin).length > 0) {
        // Only a row-like element: a click on the page background must not
        // report whatever address happens to be in the page's text.
        const row = button ?? target.closest(ACCOUNT_ROW);
        const email = row ? emailIn(row) : null;
        if (email) void deps.send({ type: "cs_sso_account", account: email });
      }
    },
    onTrustedInput(): void {
      endRun();
    },
    handleBackground(m: BackgroundToSso): SsoPressReply | undefined {
      switch (m.type) {
        case "bg_sso_show":
          show(m.token);
          return undefined;
        case "bg_sso_close":
          if (frame?.token === m.token) {
            closeFrame();
            dismissedAt = pageKey();
          }
          return undefined;
        case "bg_sso_resize":
          if (frame?.token === m.token) frame.place(ssoBox(deps.viewport(), m.height));
          return undefined;
        case "bg_sso_press": {
          if (!deps.isTop || m.origin !== location.origin) return { pressed: false };
          const env = defaultEnv();
          const button = providerButton(document, m.provider, env);
          if (!button || hasChallenge(document, env)) return { pressed: false };
          pressed = true;
          button.click();
          return { pressed: true };
        }
      }
    },
    onReady(sso: SsoReady): void {
      // Only a top frame acts (the background answers subframes with null too).
      if (!deps.isTop || !sso || providersForOrigin(location.origin).length === 0) return;
      providerRun = true;
      providerStep(sso.kind === "choose" ? sso.account : null);
    },
    teardown(): void {
      watching = false;
      // A page restored from the back/forward cache re-arms with watchPage():
      // it must send its buttons again, even if they are the same.
      lastSent = "";
      closeFrame();
      cancelChoose();
      stopObserving();
      if (scanTimer !== null) clearTimeout(scanTimer);
      scanTimer = null;
    },
  };
}
