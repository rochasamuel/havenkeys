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
//   unobscured (see menu.ts).

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

export interface Box {
  top: number;
  left: number;
  width: number;
  height: number;
}

export class InlineFrame {
  readonly token: string;
  readonly el: HTMLIFrameElement;
  #observer: MutationObserver;
  #closed = false;

  constructor(page: "menu.html" | "save.html" | "passkey.html", token: string, box: Box, onGone: () => void) {
    this.token = token;
    const el = document.createElement("iframe");
    el.src = `${chrome.runtime.getURL(page)}#${token}`;
    el.title = page === "menu.html" ? "HavenKeys suggestions" : page === "passkey.html" ? "HavenKeys passkeys" : "HavenKeys";
    el.setAttribute("referrerpolicy", "no-referrer");
    el.setAttribute("allow", "");
    this.el = el;
    this.#apply(box);
    // Attach to <html>, not <body>: pages transform or replace <body>.
    document.documentElement.append(el);

    this.#observer = new MutationObserver((records) => {
      if (this.#closed) return;
      const touched = records.some((r) => r.type === "attributes" && r.target === el);
      if (touched || !el.isConnected) onGone();
    });
    this.#observer.observe(el, { attributes: true });
    this.#observer.observe(document.documentElement, { childList: true });
  }

  #apply(box: Box): void {
    const style = { ...BASE_STYLE, top: `${box.top}px`, left: `${box.left}px`, width: `${box.width}px`, height: `${box.height}px` };
    for (const [k, v] of Object.entries(style)) this.el.style.setProperty(k, v, "important");
  }

  /** Move the frame. Our own style changes are not tampering. */
  place(box: Box): void {
    this.#apply(box);
    this.#observer.takeRecords();
  }

  focus(): void {
    this.el.focus();
  }

  remove(): void {
    this.#closed = true;
    this.#observer.disconnect();
    this.el.remove();
  }
}

// ---------------------------------------------------------------- layout
// Must agree with menu.css / save.css.

export const MENU_HEADER = 34;
export const MENU_ROW = 46;
/** The list's 4px top and bottom padding, plus the card's 1px borders. */
export const MENU_PADDING = 10;
export const MENU_MIN_WIDTH = 260;
export const MENU_MAX_WIDTH = 360;
export const SAVE_WIDTH = 340;
export const SAVE_HEIGHT = 138;

/** Below the field, or above it when there is no room. */
export function menuBox(field: DOMRect, rows: number, viewport: { width: number; height: number }): Box {
  const width = Math.min(MENU_MAX_WIDTH, Math.max(MENU_MIN_WIDTH, field.width));
  const height = MENU_HEADER + rows * MENU_ROW + MENU_PADDING;
  let top = field.bottom + 4;
  if (top + height > viewport.height && field.top - height - 4 >= 0) top = field.top - height - 4;
  const left = Math.max(4, Math.min(field.left, viewport.width - width - 4));
  return { top, left, width, height };
}

export function saveBox(viewport: { width: number }): Box {
  return { top: 12, left: Math.max(4, viewport.width - SAVE_WIDTH - 16), width: SAVE_WIDTH, height: SAVE_HEIGHT };
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
