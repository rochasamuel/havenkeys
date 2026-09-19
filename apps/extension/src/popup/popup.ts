// Toolbar popup. Shows the lock state and the logins saved for the current
// site. It never shows passwords; a TOTP code appears only when the user
// asks for it and disappears when it expires.
//
// All DOM is built with createElement/textContent: vault data is never
// parsed as HTML.

import type { Match } from "@havenkeys/protocol";
import type { PopupReply, PopupRequest, PopupState, TotpView } from "../messaging/popup";

const main = document.getElementById("main") as HTMLElement;
const pill = document.getElementById("state") as HTMLElement;
const lockBtn = document.getElementById("lock") as HTMLButtonElement;

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

function matchRow(m: Match): HTMLElement {
  const initial = (m.title.trim()[0] ?? "?").toUpperCase();
  const row = h(
    "li",
    { className: "item" },
    h("span", { className: "avatar", text: initial }),
    h(
      "div",
      { className: "who" },
      h("div", { className: "title", text: m.title }),
      h("div", { className: "user", text: m.username ?? "No username" }),
    ),
  );
  if (m.hasTotp) {
    const slot = h("span");
    const btn = h("button", { className: "btn small", text: "Show code" });
    btn.type = "button";
    btn.addEventListener("click", async () => {
      btn.disabled = true;
      const r = await send<TotpView>({ type: "popup_totp", itemId: m.id });
      if (!r.ok) {
        slot.replaceChildren(h("span", { className: "error", text: "Unavailable" }));
        slot.title = r.message;
        return;
      }
      slot.replaceChildren(h("span", { className: "code", text: formatCode(r.value.code) }));
      // Remove the code when it stops being valid.
      setTimeout(() => slot.replaceChildren(btn), r.value.secondsRemaining * 1000);
      btn.disabled = false;
    });
    slot.append(btn);
    row.append(slot);
  }
  return row;
}

function render(state: PopupState): void {
  lockBtn.hidden = state.kind !== "unlocked";
  switch (state.kind) {
    case "host_unavailable":
      setPill(null);
      main.replaceChildren(
        notice("Not connected", "The HavenKeys native host is not installed. See docs/native-messaging.md."),
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
      return;
    }
  }
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

void refresh();
