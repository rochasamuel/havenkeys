// Helpers shared by the in-page menu, save prompt and passkey card (extension pages
// embedded in web pages).

import type { InlineReply, InlineRequest } from "../messaging/inline";
import { TOKEN } from "../messaging/inline";
import type { PkRequest } from "../webauthn/messages";

/** The session token from our own URL fragment, or null. */
export function tokenFromHash(): string | null {
  const t = location.hash.slice(1);
  return TOKEN.test(t) ? t : null;
}

export async function ask<T>(req: InlineRequest | PkRequest): Promise<InlineReply<T>> {
  try {
    return (await chrome.runtime.sendMessage(req)) as InlineReply<T>;
  } catch {
    return { ok: false, message: "HavenKeys could not be reached." };
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

export function monogram(title: string): string {
  return (title.trim()[0] ?? "?").toUpperCase();
}
