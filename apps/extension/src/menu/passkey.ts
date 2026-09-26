// Passkey chooser and "save passkey" card. Shows the site, login titles and
// account names only. Signing and creation happen in the desktop app, after
// a click here; this page never sees a key or a signature.

import type { PasskeyCandidate } from "@havenkeys/protocol";
import { PASSKEY_MAX_HEIGHT, PASSKEY_MIN_HEIGHT, type PasskeyRow, type PkView } from "../webauthn/messages";
import { ask, createClickGuard, h, monogram, tokenFromHash } from "./common";

const question = document.getElementById("question") as HTMLElement;
const detail = document.getElementById("detail") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const main = document.getElementById("main") as HTMLElement;
const cancelBtn = document.getElementById("cancel") as HTMLButtonElement;
const fallbackBtn = document.getElementById("fallback") as HTMLButtonElement;
const confirmBtn = document.getElementById("confirm") as HTMLButtonElement;
const token = tokenFromHash();
const card = document.querySelector(".card") as HTMLElement;
const guard = createClickGuard(card);

function onClick(button: HTMLButtonElement, action: () => Promise<void>): void {
  button.addEventListener("click", (e) => {
    if (!token || !e.isTrusted || !guard.armed() || button.disabled) return;
    button.disabled = true;
    void action().finally(() => (button.disabled = false));
  });
}

const UNREACHABLE = "HavenKeys could not be reached.";

function showError(message: string): void {
  question.textContent = message;
  question.className = "pk-title error";
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

/** "Add to <login>" radios plus "New login"; `preselect` (the login just filled) wins over a username match. */
function choices(candidates: PasskeyCandidate[], userName: string, preselect: string | null): { el: HTMLElement; selected(): string | null } {
  const list = h("div", { className: "choices" });
  list.setAttribute("role", "radiogroup");
  list.setAttribute("aria-label", "Save to");
  const radios: Array<{ input: HTMLInputElement; itemId: string | null }> = [];
  const add = (title: string, sub: string, itemId: string | null, checked: boolean) => {
    const input = h("input");
    input.type = "radio";
    input.name = "target";
    input.checked = checked;
    radios.push({ input, itemId });
    list.append(
      h("label", { className: "choice" }, input, h("span", { className: "who" }, h("span", { className: "title", text: title }), h("span", { className: "user", text: sub }))),
    );
  };
  const preferred = preselect
    ? candidates.find((c) => c.itemId === preselect)
    : candidates.find((c) => (c.username ?? "").toLowerCase() === userName.toLowerCase());
  for (const c of candidates) add(c.title, c.username ?? "No username", c.itemId, c === preferred);
  add("New login", "Create a login for this site", null, preferred === undefined);
  return { el: h("div", {}, h("p", { className: "pk-label", text: "Save to" }), list), selected: () => radios.find((r) => r.input.checked)?.itemId ?? null };
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
    // `ask` normally turns failures into a reply, but a worker that is
    // restarting can answer nothing at all: keep polling then.
    const r = await ask<PkView>({ type: "pk_state", token: t });
    if (r === undefined) pollWhileLocked(t);
    else if (!r?.ok) showError(r?.message ?? UNREACHABLE);
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
      detail.textContent = view.passkeys.length > 1 ? "Choose an account" : "";
      main.replaceChildren(...view.passkeys.map((p) => row(p, t)));
      return;
    case "create": {
      question.textContent = view.upgradeItemId ? "Add a passkey?" : "Save a passkey to HavenKeys?";
      detail.textContent = view.userName ? `Account: ${view.userName}` : "No account name";
      const c = choices(view.candidates, view.userName, view.upgradeItemId);
      main.replaceChildren(c.el);
      // The preselected login may be below the fold. Not scrollIntoView: from
      // inside a frame that can scroll the page too.
      const checked = main.querySelector<HTMLElement>(".choice:has(input:checked)");
      if (checked && checked.offsetTop + checked.offsetHeight > main.clientHeight) main.scrollTop = checked.offsetTop - 8;
      confirmBtn.hidden = false;
      onClick(confirmBtn, async () => {
        const r = await ask<null>({ type: "pk_save", token: t, itemId: c.selected() });
        // On success the background closes this frame.
        if (!r.ok) showError(r.message);
      });
      return;
    }
    case "exists":
      // Nothing to save; "Close" is the only way the site learns that the
      // passkey exists (InvalidStateError), and it takes a click.
      question.textContent = "This account already has a passkey in HavenKeys";
      detail.textContent = "";
      main.replaceChildren();
      cancelBtn.hidden = true;
      confirmBtn.textContent = "Close";
      confirmBtn.hidden = false;
      onClick(confirmBtn, async () => {
        const r = await ask<null>({ type: "pk_close", token: t });
        if (!r?.ok) showError(r?.message ?? UNREACHABLE);
      });
      return;
    case "saved":
      // A notice, not a question: the bridge removes it after NOTICE_MS.
      question.textContent = "Passkey saved to HavenKeys";
      detail.textContent = "Manage it in the HavenKeys app";
      main.replaceChildren();
      cancelBtn.hidden = fallbackBtn.hidden = confirmBtn.hidden = true;
      document.querySelector(".actions")?.setAttribute("hidden", "");
      return;
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

// ------------------------------------------------------------ size
// The bridge sizes our iframe from the height we report. Until then the card
// may be clipped, which also keeps the click guard disarmed.

let reported = 0;

/** The card's natural height: the list up to its own cap, everything else as laid out. */
function naturalHeight(): number {
  let total = card.offsetHeight - card.clientHeight; // borders
  for (const el of Array.from(card.children) as HTMLElement[]) {
    if (el === main) {
      const cap = parseFloat(getComputedStyle(main).maxHeight) || Infinity;
      total += getComputedStyle(main).display === "none" ? 0 : Math.min(main.scrollHeight, cap);
    } else {
      total += el.getBoundingClientRect().height;
    }
  }
  return Math.ceil(total);
}

function reportSize(): void {
  if (!token) return;
  const height = Math.min(PASSKEY_MAX_HEIGHT, Math.max(PASSKEY_MIN_HEIGHT, naturalHeight()));
  if (height === reported) return;
  reported = height;
  void ask({ type: "pk_resize", token, height });
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

if (typeof ResizeObserver === "function") {
  const ro = new ResizeObserver(scheduleSize);
  for (const el of Array.from(card.children)) ro.observe(el);
}
new MutationObserver(scheduleSize).observe(card, { childList: true, subtree: true, attributes: true, characterData: true });
// Web fonts change line heights once loaded.
void document.fonts?.ready.then(scheduleSize);

async function init(): Promise<void> {
  if (!token) return;
  const r = await ask<PkView>({ type: "pk_state", token });
  if (r?.ok) render(token, r.value);
  else showError(r?.message ?? UNREACHABLE);
}

void init();
