// Helpers shared by the in-page menu, save prompt and passkey card (extension pages
// embedded in web pages).

import type { InlineReply, InlineRequest } from "../messaging/inline";
import { TOKEN, TOKEN_MESSAGE } from "../messaging/inline";
import type { SsoFrameRequest } from "../messaging/sso";
import type { PkRequest } from "../webauthn/messages";
import { t } from "../i18n";

/** How long a frame waits for its token before showing nothing. */
const TOKEN_WAIT_MS = 5000;

/**
 * The session token the content script posts once this frame has loaded
 * (EX-03), or null. Only a message from the embedding window counts, and
 * only the first well-formed one. The page is that window too, so it could
 * post a token first, but it knows no live one: that only breaks its own
 * menu.
 */
export function receiveToken(): Promise<string | null> {
  return new Promise((resolve) => {
    const done = (token: string | null): void => {
      removeEventListener("message", onMessage);
      clearTimeout(timer);
      resolve(token);
    };
    const onMessage = (e: MessageEvent): void => {
      if (e.source !== window.parent) return;
      const d: unknown = e.data;
      if (typeof d !== "object" || d === null) return;
      const { type, token } = d as { type?: unknown; token?: unknown };
      if (type === TOKEN_MESSAGE && typeof token === "string" && TOKEN.test(token)) done(token);
    };
    addEventListener("message", onMessage);
    const timer = setTimeout(() => done(null), TOKEN_WAIT_MS);
  });
}

export async function ask<T>(req: InlineRequest | PkRequest | SsoFrameRequest): Promise<InlineReply<T>> {
  try {
    return (await chrome.runtime.sendMessage(req)) as InlineReply<T>;
  } catch {
    return { ok: false, message: t.errors.unreachable };
  }
}

export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: { className?: string; text?: string } = {},
  ...children: Node[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (props.className) el.className = props.className;
  if (props.text !== undefined) el.textContent = props.text;
  el.append(...children);
  return el;
}

/** Clicks are ignored until the frame has been visible this long. */
const ARM_DELAY_MS = 400;

/**
 * Clickjacking guard. The embedding page controls the <iframe> element: it
 * could show it under the pointer at the last moment, make it transparent,
 * or cover it with a decoy that lets clicks through. So a click counts only
 * if the frame has been visible for ARM_DELAY_MS, and, where the browser
 * supports IntersectionObserver v2 (Chromium), only while the browser
 * reports it unobscured and fully opaque. Browsers without v2 (Firefox) get
 * the delay only; see docs/autofill.md.
 */
export function createClickGuard(target: Element): { armed(): boolean } {
  const start = performance.now();
  let visibleSince: number | null = null;
  let v2 = false;
  try {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          const isVisible = (e as IntersectionObserverEntry & { isVisible?: boolean }).isVisible;
          if (isVisible !== undefined) v2 = true;
          const visible = isVisible ?? e.isIntersecting;
          visibleSince = visible ? (visibleSince ?? performance.now()) : null;
        }
      },
      { threshold: [1], trackVisibility: true, delay: 100 } as IntersectionObserverInit,
    );
    io.observe(target);
  } catch {
    // No IntersectionObserver: time-based guard only.
  }
  return {
    armed() {
      const now = performance.now();
      if (v2) return visibleSince !== null && now - visibleSince >= ARM_DELAY_MS;
      return now - start >= ARM_DELAY_MS;
    },
  };
}

/**
 * Marks an element that shows user data (a title, username or site) and may
 * cut it with an ellipsis. Our own copy never truncates; the layout check
 * (tools/ui-check) holds everything unmarked to that.
 */
export function userData<T extends HTMLElement>(el: T): T {
  el.dataset.truncate = "";
  return el;
}

export function monogram(title: string): string {
  return (title.trim()[0] ?? "?").toUpperCase();
}
