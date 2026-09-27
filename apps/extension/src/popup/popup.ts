// Toolbar popup. Shows the lock state and the logins saved for the current
// site. It never shows passwords; a TOTP code appears only when the user
// asks for it and disappears when it expires.
//
// All DOM is built with createElement/textContent: vault data is never
// parsed as HTML.

import type { Match } from "@havenkeys/protocol";
import type { PopupReply, PopupRequest, PopupState, TotpView } from "../messaging/popup";
import { INLINE_ORIGINS, grantedOrigins } from "../background/registration";
import { applyDocumentLang, t } from "../i18n";

const main = document.getElementById("main") as HTMLElement;
const pill = document.getElementById("state") as HTMLElement;
const lockBtn = document.getElementById("lock") as HTMLButtonElement;
const optionsBtn = document.getElementById("options") as HTMLButtonElement;
const lockLabel = document.getElementById("lock-label") as HTMLElement;

applyDocumentLang();
optionsBtn.title = t.popup.settings;
optionsBtn.setAttribute("aria-label", t.popup.settings);
lockLabel.textContent = t.popup.lock;

function h<K extends keyof HTMLElementTagNameMap>(
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

async function send<T>(req: PopupRequest): Promise<PopupReply<T>> {
  try {
    return (await chrome.runtime.sendMessage(req)) as PopupReply<T>;
  } catch {
    return { ok: false, message: t.popup.unreachable };
  }
}

/** User data (titles, usernames, sites) may be cut with an ellipsis; our own copy never is. */
function truncates<T extends HTMLElement>(el: T): T {
  el.dataset.truncate = "";
  return el;
}

function notice(title: string, body: string): HTMLElement {
  return h("div", { className: "notice" }, h("strong", { text: title }), h("p", { text: body }));
}

function setPill(text: string | null, cls = ""): void {
  pill.hidden = text === null;
  pill.textContent = text ?? "";
  pill.className = `pill ${cls}`;
}

function formatCode(code: string): string {
  const mid = Math.ceil(code.length / 2);
  return `${code.slice(0, mid)} ${code.slice(mid)}`;
}

function smallButton(text: string, title: string): HTMLButtonElement {
  const b = h("button", { className: "btn small", text });
  b.type = "button";
  b.title = title;
  return b;
}

const SVG = "http://www.w3.org/2000/svg";

/** A square icon button; the icon is built with createElementNS, never parsed from markup. */
function iconButton(pathData: string, label: string): HTMLButtonElement {
  const b = h("button", { className: "icon-btn row-icon" });
  b.type = "button";
  b.title = label;
  b.setAttribute("aria-label", label);
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", pathData);
  svg.append(path);
  b.append(svg);
  return b;
}

const PENCIL = "M4.5 19.5h4l10-10a2.1 2.1 0 0 0-3-3l-10 10v3zM14 8l3 3";

/** Fill into the page; the popup closes on success. */
async function fillFromPopup(btn: HTMLButtonElement, req: PopupRequest, status: HTMLElement): Promise<void> {
  btn.disabled = true;
  const r = await send<null>(req);
  btn.disabled = false;
  if (r.ok) {
    window.close();
    return;
  }
  status.replaceChildren(h("span", { className: "error", text: r.message }));
}

function matchRow(m: Match): HTMLElement {
  const initial = (m.title.trim()[0] ?? "?").toUpperCase();
  const status = h("div", { className: "row-status" });
  const row = h(
    "li",
    { className: "item" },
    h("span", { className: "avatar", text: initial }),
    h(
      "div",
      { className: "who" },
      truncates(h("div", { className: "title", text: m.title })),
      m.username !== null ? truncates(h("div", { className: "user", text: m.username })) : h("div", { className: "user copy", text: t.common.noUsername }),
    ),
  );
  const actions = h("div", { className: "actions" });

  const fill = smallButton(t.popup.fill, t.popup.fillTitle);
  fill.addEventListener("click", () => void fillFromPopup(fill, { type: "popup_fill", itemId: m.id }, status));
  actions.append(fill);

  if (m.hasTotp) {
    const slot = h("span");
    const show = smallButton(t.popup.code, t.popup.codeTitle);
    show.addEventListener("click", async () => {
      show.disabled = true;
      const r = await send<TotpView>({ type: "popup_totp", itemId: m.id });
      show.disabled = false;
      if (!r.ok) {
        status.replaceChildren(h("span", { className: "error", text: r.message }));
        return;
      }
      const code = h("button", { className: "code", text: formatCode(r.value.code) });
      code.type = "button";
      code.title = t.popup.fillCodeTitle;
      code.addEventListener("click", () => void fillFromPopup(code, { type: "popup_fill_totp", itemId: m.id }, status));
      slot.replaceChildren(code);
      // Remove the code when it stops being valid.
      setTimeout(() => slot.replaceChildren(show), r.value.secondsRemaining * 1000);
    });
    slot.append(show);
    actions.append(slot);
  }
  // Last in the row: open this login in the desktop app. Closes the popup
  // on success, like a fill, since focus moves to the app.
  const edit = iconButton(PENCIL, t.popup.edit);
  edit.addEventListener("click", () => void fillFromPopup(edit, { type: "popup_open_item", itemId: m.id }, status));
  actions.append(edit);
  // The status line (a fill or code error) goes under the whole row, so a
  // long message gets the popup's full width instead of the text column's.
  row.append(actions, status);
  return row;
}

function render(state: PopupState): void {
  lockBtn.hidden = state.kind !== "unlocked";
  switch (state.kind) {
    case "host_unavailable":
      setPill(null);
      main.replaceChildren(
        notice(t.popup.hostUnavailable.title, t.popup.hostUnavailable.body),
      );
      return;
    case "desktop_unavailable":
      setPill(t.popup.pill.offline);
      main.replaceChildren(notice(t.popup.desktopUnavailable.title, t.popup.desktopUnavailable.body));
      return;
    case "no_vault":
      setPill(null);
      main.replaceChildren(notice(t.popup.noVault.title, t.popup.noVault.body));
      return;
    case "locked":
      setPill(t.popup.pill.locked, "locked");
      main.replaceChildren(notice(t.popup.locked.title, t.popup.locked.body));
      return;
    case "disabled":
      setPill(t.popup.pill.off);
      main.replaceChildren(notice(t.popup.disabled.title, t.popup.disabled.body));
      return;
    case "error":
      setPill(null);
      main.replaceChildren(notice(t.popup.error.title, state.message));
      return;
    case "unlocked": {
      setPill(t.popup.pill.unlocked, "unlocked");
      const parts: Node[] = [];
      if (state.site) parts.push(truncates(h("div", { className: "site", text: state.site })));
      if (!state.site) {
        parts.push(notice(t.popup.noPage.title, t.popup.noPage.body));
      } else if (state.matches.length === 0) {
        parts.push(notice(t.popup.noMatches.title, t.popup.noMatches.body));
      } else {
        parts.push(h("ul", { className: "list" }, ...state.matches.map(matchRow)));
      }
      main.replaceChildren(...parts);
      void offerSuggestions();
      return;
    }
  }
}

/**
 * In-page suggestions are opt-in (background/registration.ts). Until they
 * are on, say so here: otherwise logins only ever show up in this popup.
 */
async function offerSuggestions(): Promise<void> {
  if ((await grantedOrigins()).length > 0) return;
  const turnOn = smallButton(t.popup.offer.turnOn, t.popup.offer.turnOnTitle);
  turnOn.addEventListener("click", () => {
    // permissions.request must run directly in the click handler.
    void chrome.permissions
      .request({ origins: [...INLINE_ORIGINS] })
      .catch(() => false)
      .then((granted) => {
        if (granted) box.replaceChildren(h("strong", { text: t.popup.offer.doneTitle }), h("p", { text: t.popup.offer.doneBody }));
      });
  });
  const box = h(
    "div",
    { className: "notice offer" },
    h("strong", { text: t.popup.offer.title }),
    h("p", { text: t.popup.offer.body }),
    turnOn,
  );
  main.append(box);
}

async function refresh(): Promise<void> {
  const r = await send<PopupState>({ type: "popup_state" });
  render(r.ok ? r.value : { kind: "error", message: r.message });
}

lockBtn.addEventListener("click", async () => {
  lockBtn.disabled = true;
  const r = await send<PopupState>({ type: "popup_lock" });
  lockBtn.disabled = false;
  render(r.ok ? r.value : { kind: "error", message: r.message });
});

optionsBtn.addEventListener("click", () => void chrome.runtime.openOptionsPage());

void refresh();
