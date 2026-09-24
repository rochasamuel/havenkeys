// Passkey chooser and "save passkey" card. Shows the site, login titles and
// account names only. Signing and creation happen in the desktop app, after
// a click here; this page never sees a key or a signature.

import type { PasskeyCandidate } from "@havenkeys/protocol";
import type { PasskeyRow, PkView } from "../webauthn/messages";
import { ask, createClickGuard, h, monogram, tokenFromHash } from "./common";

const question = document.getElementById("question") as HTMLElement;
const detail = document.getElementById("detail") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const main = document.getElementById("main") as HTMLElement;
const cancelBtn = document.getElementById("cancel") as HTMLButtonElement;
const fallbackBtn = document.getElementById("fallback") as HTMLButtonElement;
const confirmBtn = document.getElementById("confirm") as HTMLButtonElement;
const token = tokenFromHash();
const guard = createClickGuard(document.querySelector(".card") as HTMLElement);

function onClick(button: HTMLButtonElement, action: () => Promise<void>): void {
  button.addEventListener("click", (e) => {
    if (!token || !e.isTrusted || !guard.armed() || button.disabled) return;
    button.disabled = true;
    void action().finally(() => (button.disabled = false));
  });
}

function showError(message: string): void {
  question.textContent = message;
  question.className = "error";
}

function row(p: PasskeyRow, t: string): HTMLButtonElement {
  const b = h(
    "button",
    { className: "row" },
    h("span", { className: "avatar", text: monogram(p.title) }),
    h("span", { className: "who" }, h("span", { className: "title", text: p.title }), h("span", { className: "user", text: p.userName || "Passkey" })),
  );
  b.type = "button";
  onClick(b, async () => {
    const r = await ask<null>({ type: "pk_pick", token: t, itemId: p.itemId, credentialId: p.credentialId });
    if (!r.ok) showError(r.message);
  });
  return b;
}

/** "Add to <login>" radios plus "New login". */
function choices(candidates: PasskeyCandidate[], userName: string): { el: HTMLElement; selected(): string | null } {
  const list = h("div", { className: "choices" });
  const radios: Array<{ input: HTMLInputElement; itemId: string | null }> = [];
  const add = (label: string, itemId: string | null, checked: boolean) => {
    const input = h("input");
    input.type = "radio";
    input.name = "target";
    input.checked = checked;
    radios.push({ input, itemId });
    list.append(h("label", { className: "choice" }, input, h("span", { text: label })));
  };
  const preferred = candidates.find((c) => (c.username ?? "").toLowerCase() === userName.toLowerCase());
  for (const c of candidates) add(`Add to “${c.title}”`, c.itemId, c === preferred);
  add("New login", null, preferred === undefined);
  return { el: list, selected: () => radios.find((r) => r.input.checked)?.itemId ?? null };
}

/** While locked, how often to ask whether the vault has been unlocked. */
const LOCKED_POLL_MS = 1500;
let poll: ReturnType<typeof setTimeout> | null = null;

function stopPolling(): void {
  if (poll !== null) clearTimeout(poll);
  poll = null;
}

/** Re-ask for the state until the background has looked the passkeys up again. */
function pollWhileLocked(t: string): void {
  stopPolling();
  poll = setTimeout(async () => {
    poll = null;
    const r = await ask<PkView>({ type: "pk_state", token: t });
    if (!r.ok) showError(r.message);
    else if (r.value.state === "locked") pollWhileLocked(t);
    else render(t, r.value);
  }, LOCKED_POLL_MS);
}

function render(t: string, view: PkView): void {
  site.textContent = view.site;
  switch (view.state) {
    case "locked":
      question.textContent = "HavenKeys is locked";
      detail.textContent = "Unlock the HavenKeys app — this card will update.";
      pollWhileLocked(t);
      return;
    case "chooser":
      question.textContent = "Sign in with a passkey";
      detail.textContent = "";
      main.replaceChildren(...view.passkeys.map((p) => row(p, t)));
      return;
    case "create": {
      question.textContent = "Save a passkey to HavenKeys?";
      detail.textContent = view.userName || "No account name";
      const c = choices(view.candidates, view.userName);
      main.replaceChildren(c.el);
      confirmBtn.hidden = false;
      onClick(confirmBtn, async () => {
        const r = await ask<null>({ type: "pk_save", token: t, itemId: c.selected() });
        // On success the background closes this frame.
        if (!r.ok) showError(r.message);
      });
      return;
    }
  }
}

onClick(fallbackBtn, async () => {
  if (token) await ask({ type: "pk_fallback", token });
});
cancelBtn.addEventListener("click", (e) => {
  if (!token || !e.isTrusted) return;
  void ask({ type: "pk_cancel", token });
});
addEventListener("pagehide", stopPolling);
document.addEventListener("keydown", (e) => {
  if (token && e.key === "Escape") void ask({ type: "pk_cancel", token });
});

async function init(): Promise<void> {
  if (!token) return;
  const r = await ask<PkView>({ type: "pk_state", token });
  if (r.ok) render(token, r.value);
  else showError(r.message);
}

void init();
