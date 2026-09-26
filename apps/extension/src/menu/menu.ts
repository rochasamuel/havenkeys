// In-page suggestion menu. Shows titles and usernames only; passwords and
// codes go from the background straight to the content script and never
// pass through this page.

import { applyDocumentLang, t as msg } from "../i18n";
import { MENU_MAX_HEIGHT, MENU_MAX_ROWS, MENU_MIN_HEIGHT, type MenuItemView, type MenuView } from "../messaging/inline";
import { ask, createClickGuard, h, monogram, tokenFromHash, userData } from "./common";

const main = document.getElementById("main") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const card = document.querySelector(".card") as HTMLElement;
const token = tokenFromHash();
const guard = createClickGuard(card);

applyDocumentLang();
document.title = msg.menu.pageTitle;

function message(title: string, detail: string, error = false): HTMLElement {
  return h("div", { className: "message" }, h("strong", { text: title, ...(error ? { className: "error" } : {}) }), h("span", { text: detail }));
}

/** A row's text line: user data truncates with an ellipsis, our own copy wraps. */
function line(className: "title" | "user", text: string, copy: boolean): HTMLElement {
  return copy ? h("span", { className: `${className} copy`, text }) : userData(h("span", { className, text }));
}

/** The generate row's glyph: a sparkle drawn in the app's 1.6-stroke icon set. */
function sparkle(): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "16");
  svg.setAttribute("height", "16");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", "M12 3.5v4M12 16.5v4M3.5 12h4M16.5 12h4M6 6l2.6 2.6M15.4 15.4 18 18M6 18l2.6-2.6M15.4 8.6 18 6");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.6");
  path.setAttribute("stroke-linecap", "round");
  svg.append(path);
  return svg;
}

/** `copy`: which lines are UI copy (wrap) rather than user data (truncate). */
type Copy = { title: boolean; detail: boolean };
const DATA: Copy = { title: false, detail: false };

function row(avatar: string | Node, title: string, detail: string, onPick: () => Promise<void>, copy: Copy = DATA): HTMLButtonElement {
  const b = h(
    "button",
    { className: "row" },
    typeof avatar === "string" ? h("span", { className: "avatar", text: avatar }) : h("span", { className: "avatar avatar-icon" }, avatar),
    h("span", { className: "who" }, line("title", title, copy.title), line("user", detail, copy.detail)),
  );
  b.type = "button";
  b.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed() || b.disabled) return;
    b.disabled = true;
    void onPick().finally(() => (b.disabled = false));
  });
  return b;
}

async function pick(req: Parameters<typeof ask>[0]): Promise<void> {
  const r = await ask<null>(req);
  // On success the background closes this frame.
  if (!r.ok) main.replaceChildren(message(msg.menu.couldNotFill, r.message, true));
}

function itemRow(t: string, item: MenuItemView, kind: "login" | "otp"): HTMLButtonElement {
  const detail = kind === "otp" ? msg.menu.fillCode : (item.username ?? msg.common.noUsername);
  const copy = { title: false, detail: kind === "otp" || item.username === null };
  return row(monogram(item.title), item.title, detail, () => pick({ type: "menu_pick", token: t, itemId: item.id }), copy);
}

/** A row that only informs: no button, not in the arrow-key order. */
function hintNote(title: string, detail: string): HTMLElement {
  return h(
    "div",
    { className: "row hint" },
    h("span", { className: "who" }, line("title", title, true), line("user", detail, true)),
  );
}

function render(t: string, view: MenuView): void {
  if (view.state === "locked") {
    main.replaceChildren(message(msg.menu.lockedTitle, msg.menu.lockedBody));
    return;
  }
  site.textContent = view.site;
  if (view.kind === "new_password") {
    main.replaceChildren(
      row(sparkle(), msg.menu.generateTitle, msg.menu.generateBody, () => pick({ type: "menu_generate", token: t }), { title: true, detail: true }),
    );
    return;
  }
  const kind = view.kind;
  const passkeyRows = view.passkeys.map((p) =>
    row(monogram(p.title), p.title, msg.menu.passkeyRow(p.userName || msg.menu.passkeyAccountFallback), () =>
      pick({ type: "menu_pick_passkey", token: t, itemId: p.itemId, credentialId: p.credentialId }),
    ),
  );
  const hint = view.hint;
  const lead = hint?.kind === "use_passkey" ? [hintNote(msg.menu.usePasskeyTitle(view.site), msg.menu.usePasskeyBody)] : [];
  const tail =
    hint?.kind === "add_passkey"
      ? [row(sparkle(), msg.menu.addPasskeyTitle(hint.name), msg.menu.addPasskeyBody, () => pick({ type: "menu_open_help", token: t }), { title: true, detail: true })]
      : [];
  const rows = [...lead, ...passkeyRows, ...view.items.map((i) => itemRow(t, i, kind)), ...tail];
  // Opened from the field's icon with nothing saved for this site.
  if (rows.length === 0) {
    main.replaceChildren(
      kind === "otp"
        ? message(msg.menu.noCodesTitle, msg.menu.noCodesBody)
        : message(msg.menu.noLoginsTitle, msg.menu.noLoginsBody),
    );
    return;
  }
  main.replaceChildren(...rows);
}

/** Arrow keys move between rows; Escape closes. */
document.addEventListener("keydown", (e) => {
  if (!token) return;
  if (e.key === "Escape") {
    void ask({ type: "menu_close", token });
    return;
  }
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const rows = Array.from(main.querySelectorAll<HTMLButtonElement>("button.row"));
  if (rows.length === 0) return;
  const i = rows.indexOf(document.activeElement as HTMLButtonElement);
  const next = e.key === "ArrowDown" ? (i + 1) % rows.length : (i - 1 + rows.length) % rows.length;
  rows[next]?.focus();
  e.preventDefault();
});

// Focused from the page with ArrowDown: start on the first row.
window.addEventListener("focus", () => {
  if (!main.contains(document.activeElement)) main.querySelector<HTMLButtonElement>("button.row")?.focus();
});

// ------------------------------------------------------------ size
// The content script first sizes our iframe from the row count (46px rows).
// Rows whose copy wraps (longer languages, narrow fields) are taller, so we
// report the height the first MENU_MAX_ROWS rows need and it resizes the
// frame (menu_resize → bg_resize_menu). Past that many rows the list scrolls.

let reported = 0;

function naturalHeight(): number {
  const head = card.firstElementChild as HTMLElement | null;
  const style = getComputedStyle(main);
  let total = card.offsetHeight - card.clientHeight; // borders
  total += head?.getBoundingClientRect().height ?? 0;
  total += (parseFloat(style.paddingTop) || 0) + (parseFloat(style.paddingBottom) || 0);
  const rows = Array.from(main.children).slice(0, MENU_MAX_ROWS) as HTMLElement[];
  for (const el of rows) total += el.getBoundingClientRect().height;
  return Math.ceil(total);
}

function reportSize(): void {
  if (!token || main.childElementCount === 0) return;
  const height = Math.min(MENU_MAX_HEIGHT, Math.max(MENU_MIN_HEIGHT, naturalHeight()));
  if (height === reported) return;
  reported = height;
  void ask({ type: "menu_resize", token, height });
}

let sizePending = false;
function scheduleSize(): void {
  if (sizePending) return;
  sizePending = true;
  requestAnimationFrame(() => {
    sizePending = false;
    reportSize();
  });
}

const rowObserver = typeof ResizeObserver === "function" ? new ResizeObserver(scheduleSize) : null;
new MutationObserver(() => {
  rowObserver?.disconnect();
  for (const el of Array.from(main.children)) rowObserver?.observe(el);
  scheduleSize();
}).observe(main, { childList: true });
// Web fonts change line heights once loaded.
void document.fonts?.ready.then(scheduleSize);

async function init(): Promise<void> {
  if (!token) return;
  const r = await ask<MenuView>({ type: "menu_state", token });
  if (!r.ok) {
    main.replaceChildren(message(msg.menu.unavailable, r.message, true));
    return;
  }
  render(token, r.value);
}

void init();
