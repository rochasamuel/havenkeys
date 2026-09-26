// Toolbar popup. Shows the lock state and the logins saved for the current
// site. It never shows passwords; a TOTP code appears only when the user
// asks for it and disappears when it expires.
//
// All DOM is built with createElement/textContent: vault data is never
// parsed as HTML.

import type { Match } from "@havenkeys/protocol";
import type { PopupReply, PopupRequest, PopupState, TotpView } from "../messaging/popup";
import { INLINE_ORIGINS, grantedOrigins } from "../background/registration";

const main = document.getElementById("main") as HTMLElement;
const pill = document.getElementById("state") as HTMLElement;
const lockBtn = document.getElementById("lock") as HTMLButtonElement;
const optionsBtn = document.getElementById("options") as HTMLButtonElement;

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
    return { ok: false, message: "The extension could not be reached." };
  }
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
  const status = h("div", { className: "user row-status" });
  const row = h(
    "li",
    { className: "item" },
    h("span", { className: "avatar", text: initial }),
    h(
      "div",
      { className: "who" },
      h("div", { className: "title", text: m.title }),
      h("div", { className: "user", text: m.username ?? "No username" }),
      status,
    ),
  );
  const actions = h("div", { className: "actions" });

  const fill = smallButton("Fill", "Fill this login into the page");
  fill.addEventListener("click", () => void fillFromPopup(fill, { type: "popup_fill", itemId: m.id }, status));
  actions.append(fill);

  if (m.hasTotp) {
    const slot = h("span");
    const show = smallButton("Code", "Show the one-time code");
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
      code.title = "Fill this code into the page";
      code.addEventListener("click", () => void fillFromPopup(code, { type: "popup_fill_totp", itemId: m.id }, status));
      slot.replaceChildren(code);
      // Remove the code when it stops being valid.
      setTimeout(() => slot.replaceChildren(show), r.value.secondsRemaining * 1000);
    });
    slot.append(show);
    actions.append(slot);
  }
  row.append(actions);
  return row;
}

function render(state: PopupState): void {
  lockBtn.hidden = state.kind !== "unlocked";
  switch (state.kind) {
    case "host_unavailable":
      setPill(null);
      main.replaceChildren(
        notice(
          "Not connected",
          "Install or update the HavenKeys app on this computer, then open it once. It connects this browser for you.",
        ),
      );
      return;
    case "desktop_unavailable":
      setPill("Offline");
      main.replaceChildren(notice("HavenKeys is not running", "Open the HavenKeys app on this computer."));
      return;
    case "no_vault":
      setPill(null);
      main.replaceChildren(notice("No vault yet", "Create your vault in the HavenKeys app."));
      return;
    case "locked":
      setPill("Locked", "locked");
      main.replaceChildren(notice("HavenKeys is locked", "Unlock it in the HavenKeys app to use your logins."));
      return;
    case "disabled":
      setPill("Off");
      main.replaceChildren(
        notice("Browser integration is off", "Turn it on in HavenKeys → Settings → Browser extension."),
      );
      return;
    case "error":
      setPill(null);
      main.replaceChildren(notice("Something went wrong", state.message));
      return;
    case "unlocked": {
      setPill("Unlocked", "unlocked");
      const parts: Node[] = [];
      if (state.site) parts.push(h("div", { className: "site", text: state.site }));
      if (!state.site) {
        parts.push(notice("No saved logins here", "This page can't use saved logins."));
      } else if (state.matches.length === 0) {
        parts.push(notice("No saved logins for this site", "Logins saved in HavenKeys for this website appear here."));
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
  const turnOn = smallButton("Turn on", "Show your logins under login fields on websites");
  turnOn.addEventListener("click", () => {
    // permissions.request must run directly in the click handler.
    void chrome.permissions
      .request({ origins: [...INLINE_ORIGINS] })
      .catch(() => false)
      .then((granted) => {
        if (granted) box.replaceChildren(h("strong", { text: "Suggestions are on" }), h("p", { text: "Reload open tabs to use them there." }));
      });
  });
  const box = h(
    "div",
    { className: "notice offer" },
    h("strong", { text: "Suggestions in login fields are off" }),
    h("p", { text: "Turn them on to pick logins right under the field and to save and use passkeys with HavenKeys." }),
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
