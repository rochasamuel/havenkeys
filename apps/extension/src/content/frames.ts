// The suggestion menu and save prompt are extension pages shown in iframes.
//
// An extension-origin iframe keeps usernames, titles and buttons out of
// the page's reach: page script cannot read or restyle what is inside it,
// and it cannot forge clicks in it. The page can still move, hide or cover
// the <iframe> element itself, so:
// * every style is set inline with !important (beats page stylesheets);
// * a MutationObserver closes the frame if the page touches its attributes
//   or removes it;
// * the menu page itself ignores clicks until it has been visible for a
//   moment, and on Chromium only while IntersectionObserver v2 reports it
//   unobscured (see menu.ts);
// * a prompt sliding out (see remove) goes at once if the page touches it;
// * the frame's session token is posted to it after it loads, for the
//   extension's origin only, never put in its URL: the page can read an
//   iframe's src, and with the token could show the same page in a frame
//   of its own that none of the above watches (EX-03).

import { t } from "../i18n";
import { TOKEN_MESSAGE } from "../messaging/inline";

const Z_TOP = "2147483647";

const BASE_STYLE: Record<string, string> = {
  all: "initial",
  position: "fixed",
  "z-index": Z_TOP,
  display: "block",
  visibility: "visible",
  opacity: "1",
  border: "0",
  margin: "0",
  padding: "0",
  background: "transparent",
  "color-scheme": "normal",
  "pointer-events": "auto",
  transform: "none",
  filter: "none",
  "clip-path": "none",
  "box-shadow": "none",
  "border-radius": "10px",
  overflow: "hidden",
};

const EASE_OUT = "cubic-bezier(0.32, 0.72, 0, 1)";
const FRAME_TRANSITION = `height 220ms ${EASE_OUT}, top 220ms ${EASE_OUT}`;
const SLIDE_IN = `transform 280ms ${EASE_OUT}, opacity 200ms ease-out`;
export const SLIDE_OUT_MS = 200;
const SLIDE_OUT = `transform ${SLIDE_OUT_MS}ms cubic-bezier(0.4, 0, 1, 1), opacity ${SLIDE_OUT_MS}ms ease-in`;
/** Just past the right edge of the viewport, where a prompt slides from and back to. */
const OFFSTAGE: Record<string, string> = { transform: "translateX(calc(100% + 24px))", opacity: "0", "pointer-events": "none" };

function motionAllowed(): boolean {
  return typeof matchMedia === "function" && !matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export interface Box {
  top: number;
  left: number;
  width: number;
  height: number;
}

/**
 * Each frame's token, kept in the content script's own world: the page can
 * neither read this map nor the frame's URL for it.
 */
const frameTokens = new WeakMap<Element, string>();

/** The token `el` was given, if it is one of our frames. */
export function frameToken(el: Element): string | undefined {
  return frameTokens.get(el);
}

export class InlineFrame {
  readonly token: string;
  readonly el: HTMLIFrameElement;
  #observer: MutationObserver;
  #closed = false;
  /** The page moved, hid or removed the frame: it goes at once, never animated. */
  #tampered = false;
  /** The prompts at the top right slide in from the right and back out; the menu does not. */
  readonly #slides: boolean;
  #box: Box;
  #offstage = false;
  #slide = "";

  constructor(page: "menu.html" | "save.html" | "passkey.html" | "sso.html", token: string, box: Box, onGone: () => void) {
    this.token = token;
    const el = document.createElement("iframe");
    el.src = chrome.runtime.getURL(page);
    frameTokens.set(el, token);
    // The extension's base URL as the target: the browser delivers the
    // message only if the frame still shows one of our pages. (Not
    // `new URL(…).origin`, which the URL standard makes "null" for an
    // extension scheme.)
    const target = chrome.runtime.getURL("");
    el.addEventListener("load", () => el.contentWindow?.postMessage({ type: TOKEN_MESSAGE, token }, target), { once: true });
    el.title =
      page === "menu.html"
        ? t.menu.pageTitle
        : page === "passkey.html"
          ? t.passkey.pageTitle
          : page === "sso.html"
            ? t.sso.pageTitle
            : "HavenKeys";
    el.setAttribute("referrerpolicy", "no-referrer");
    el.setAttribute("allow", "");
    this.el = el;
    this.#box = box;
    this.#slides = page !== "menu.html" && motionAllowed();
    if (this.#slides) {
      this.#offstage = true;
      this.#slide = SLIDE_IN;
    }
    this.#apply(box);
    // Attach to <html>, not <body>: pages transform or replace <body>.
    document.documentElement.append(el);
    if (this.#offstage) {
      el.getBoundingClientRect(); // commit the offstage style so the slide runs
      this.#offstage = false;
      this.#apply(box);
    }

    this.#observer = new MutationObserver((records) => {
      const touched = records.some((r) => r.type === "attributes" && r.target === el);
      if (!touched && el.isConnected) return;
      if (this.#closed) return el.remove(); // touched while sliding out
      this.#tampered = true;
      onGone();
    });
    this.#observer.observe(el, { attributes: true });
    this.#observer.observe(document.documentElement, { childList: true });
  }

  /** `animate`: slide top and height to the new box (a menu panel opening); else jump, so scrolling never lags. */
  #apply(box: Box, animate = false): void {
    this.#box = box;
    const transition = [animate && motionAllowed() ? FRAME_TRANSITION : "", this.#slide].filter(Boolean).join(", ");
    const style = {
      ...BASE_STYLE,
      ...(this.#offstage ? OFFSTAGE : {}),
      top: `${box.top}px`,
      left: `${box.left}px`,
      width: `${box.width}px`,
      height: `${box.height}px`,
      transition: transition || "none",
    };
    for (const [k, v] of Object.entries(style)) this.el.style.setProperty(k, v, "important");
  }

  /** Move the frame. Our own style changes are not tampering. */
  place(box: Box, animate = false): void {
    this.#apply(box, animate);
    this.#observer.takeRecords();
  }

  focus(): void {
    this.el.focus();
  }

  /** Slides a prompt out to the right first, unless the page touched it. */
  remove(): void {
    if (this.#closed) return;
    this.#closed = true;
    if (!this.#slides || this.#tampered || !this.el.isConnected) {
      this.#observer.disconnect();
      this.el.remove();
      return;
    }
    this.#offstage = true;
    this.#slide = SLIDE_OUT;
    this.#apply(this.#box);
    this.#observer.takeRecords();
    setTimeout(() => {
      this.#observer.disconnect();
      this.el.remove();
    }, SLIDE_OUT_MS);
  }
}

// ---------------------------------------------------------------- layout
// Must agree with inline.css. The menu starts at MENU_ROW per row; rows whose
// copy wraps are taller, and the menu page then reports its real height
// (menu_resize), which replaces the estimate.

export const MENU_HEADER = 34;
export const MENU_ROW = 46;
/** The list's 4px top and bottom padding, plus the card's 1px borders. */
export const MENU_PADDING = 10;
export const MENU_MIN_WIDTH = 260;
export const MENU_MAX_WIDTH = 360;
export const SAVE_WIDTH = 340;
/** Until the prompt reports its content height (save_resize). */
export const SAVE_HEIGHT = 138;

/** Below the field, or above it when there is no room. `reported`: the height the menu page measured. */
export function menuBox(field: DOMRect, rows: number, viewport: { width: number; height: number }, reported: number | null = null): Box {
  const width = Math.min(MENU_MAX_WIDTH, Math.max(MENU_MIN_WIDTH, field.width));
  const height = reported ?? MENU_HEADER + rows * MENU_ROW + MENU_PADDING;
  let top = field.bottom + 4;
  if (top + height > viewport.height && field.top - height - 4 >= 0) top = field.top - height - 4;
  const left = Math.max(4, Math.min(field.left, viewport.width - width - 4));
  return { top, left, width, height };
}

/** Top right of the viewport. `height`: what the prompt measured (save_resize); never taller than the viewport allows. */
export function saveBox(viewport: { width: number; height?: number }, height = SAVE_HEIGHT): Box {
  const room = viewport.height !== undefined && viewport.height > 0 ? Math.max(96, viewport.height - 24) : height;
  return { top: 12, left: Math.max(4, viewport.width - SAVE_WIDTH - 16), width: SAVE_WIDTH, height: Math.min(height, room) };
}

export const SSO_HEIGHT = 150;

/** Top right, like the save prompt. */
export function ssoBox(viewport: { width: number; height?: number }, height = SSO_HEIGHT): Box {
  return saveBox(viewport, height);
}

export const PASSKEY_WIDTH = 380;
/** Until the card reports its content height (pk_resize). */
export const PASSKEY_HEIGHT = 180;

/** Top right of the viewport, like the save prompt; never taller than the viewport allows. */
export function passkeyBox(viewport: { width: number; height?: number }, height = PASSKEY_HEIGHT): Box {
  const room = viewport.height !== undefined && viewport.height > 0 ? Math.max(96, viewport.height - 24) : height;
  return { top: 12, left: Math.max(4, viewport.width - PASSKEY_WIDTH - 16), width: PASSKEY_WIDTH, height: Math.min(height, room) };
}

export const NOTICE_HEIGHT = 112;

/** The "passkey saved" notice: where the passkey card would be, shorter. */
export function noticeBox(viewport: { width: number }): Box {
  return { ...passkeyBox(viewport), height: NOTICE_HEIGHT };
}
