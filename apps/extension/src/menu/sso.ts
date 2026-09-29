// "Sign in with <provider>" balloon: offering a saved login for the
// provider's own sign-in button, confirming the account to save after a
// successful sign-in, and the "couldn't find the button" notice. Passwords
// and TOTP never appear here; the background presses the provider's button
// and drives the sign-in itself (see docs/autofill.md).

import { MAX_ACCOUNT_CHARS, SSO_PROVIDERS, type SsoProvider } from "@havenkeys/protocol";
import { MAX_TITLE_CHARS } from "../messaging/inline";
import { SSO_MAX_HEIGHT, SSO_MIN_HEIGHT, type SsoFrameRequest, type SsoRowView, type SsoView } from "../messaging/sso";
import { applyDocumentLang, t } from "../i18n";
import { ask, createClickGuard, h, tokenFromHash, userData } from "./common";
import { providerIcon } from "./icons";

const heading = document.getElementById("heading") as HTMLElement;
const main = document.getElementById("main") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const closeBtn = document.getElementById("close") as HTMLButtonElement;
const token = tokenFromHash();
const card = document.querySelector(".card") as HTMLElement;
const guard = createClickGuard(card);

applyDocumentLang();
document.title = t.sso.pageTitle;
closeBtn.setAttribute("aria-label", t.sso.close);

/** The save/update message: `title` is only sent for a brand-new login. */
export function saveRequest(token: string, account: string, title: string | null): SsoFrameRequest {
  return { type: "sso_save", token, account, title };
}

function fail(message: string): void {
  const p = h("p", { className: "error", text: message });
  p.id = "heading";
  main.replaceChildren(p);
}

function offerRow(row: SsoRowView, token: string): HTMLButtonElement {
  const detailText = t.sso.row(SSO_PROVIDERS[row.provider].name, row.account);
  const detail = row.account === null ? h("span", { className: "user copy", text: detailText }) : userData(h("span", { className: "user", text: detailText }));
  const b = h(
    "button",
    { className: "row" },
    providerIcon(row.provider),
    h("span", { className: "who" }, userData(h("span", { className: "title", text: row.title })), detail),
  );
  b.type = "button";
  b.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed()) return;
    void ask<null>({ type: "sso_pick", token, itemId: row.id }).then((r) => {
      if (!r.ok) fail(r.message);
    });
  });
  return b;
}

function renderOffer(view: Extract<SsoView, { mode: "offer" }>, token: string): void {
  heading.textContent = t.sso.offerTitle;
  const list = h("ul", { className: "list" }, ...view.rows.map((row) => offerRow(row, token)));
  main.replaceChildren(heading, list);
}

function providerLine(provider: SsoProvider): HTMLElement {
  return h("div", { className: "provider" }, providerIcon(provider), h("span", { text: t.sso.signInWith(SSO_PROVIDERS[provider].name) }));
}

function field(label: string, input: HTMLInputElement): HTMLLabelElement {
  return h("label", { className: "field" }, h("span", { text: label }), input);
}

function renderSave(view: Extract<SsoView, { mode: "save" }>, token: string): void {
  heading.textContent = view.action === "add" ? t.sso.saveQuestion : t.sso.updateQuestion;

  const accountInput = document.createElement("input");
  accountInput.type = "text";
  accountInput.maxLength = MAX_ACCOUNT_CHARS;
  accountInput.value = view.account ?? "";
  accountInput.placeholder = t.sso.accountPlaceholder;
  accountInput.autocomplete = "off";
  accountInput.spellcheck = false;

  let titleInput: HTMLInputElement | null = null;
  const fields = [field(t.sso.accountLabel, accountInput)];
  if (view.action === "add") {
    titleInput = document.createElement("input");
    titleInput.type = "text";
    titleInput.maxLength = MAX_TITLE_CHARS;
    titleInput.value = view.title ?? "";
    fields.push(field(t.sso.titleLabel, titleInput));
  }

  const dismissBtn = h("button", { className: "btn", text: t.sso.notNow });
  dismissBtn.type = "button";
  dismissBtn.addEventListener("click", (e) => {
    if (!e.isTrusted) return;
    void ask({ type: "sso_dismiss", token });
  });

  const confirmBtn = h("button", { className: "btn primary", text: view.action === "add" ? t.sso.save : t.sso.update });
  confirmBtn.type = "button";
  const finalTitleInput = titleInput;
  async function confirm(e: Event): Promise<void> {
    if (!e.isTrusted || !guard.armed() || confirmBtn.disabled) return;
    confirmBtn.disabled = true;
    const r = await ask<null>(saveRequest(token, accountInput.value, finalTitleInput ? finalTitleInput.value : null));
    // On success the background closes this frame. On failure the error
    // takes the heading's place and confirm stays disabled (save.ts's
    // fail()), but Not now/Save stay in place: the user can still dismiss.
    if (!r.ok) {
      heading.textContent = r.message;
      heading.className = "error";
    }
  }
  confirmBtn.addEventListener("click", (e) => void confirm(e));

  const actions = h("div", { className: "actions" }, dismissBtn, confirmBtn);
  main.replaceChildren(heading, providerLine(view.provider), ...fields, actions);
}

function renderNotice(view: Extract<SsoView, { mode: "notice" }>, token: string): void {
  heading.textContent = t.sso.noButton(SSO_PROVIDERS[view.provider].name);
  const closeButton = h("button", { className: "btn primary", text: t.sso.close });
  closeButton.type = "button";
  closeButton.addEventListener("click", (e) => {
    if (!e.isTrusted) return;
    void ask({ type: "sso_dismiss", token });
  });
  main.replaceChildren(heading, h("div", { className: "actions" }, closeButton));
}

export function render(view: SsoView, token: string): void {
  site.textContent = view.site;
  switch (view.mode) {
    case "offer":
      renderOffer(view, token);
      break;
    case "save":
      renderSave(view, token);
      break;
    case "notice":
      renderNotice(view, token);
      break;
  }
}

closeBtn.addEventListener("click", (e) => {
  if (!token || !e.isTrusted) return;
  void ask({ type: "sso_dismiss", token });
});

// ------------------------------------------------------------ size
// The content script opens our iframe at a fixed size. A longer question,
// list or error (other languages, several rows) wraps or grows, so we
// report the height the card's content needs and it resizes the frame
// (sso_resize → bg_sso_resize).

let reported = 0;

function naturalHeight(): number {
  let total = card.offsetHeight - card.clientHeight; // borders
  for (const el of Array.from(card.children) as HTMLElement[]) total += el.getBoundingClientRect().height;
  return Math.ceil(total);
}

function reportSize(): void {
  if (!token) return;
  const height = Math.min(SSO_MAX_HEIGHT, Math.max(SSO_MIN_HEIGHT, naturalHeight()));
  if (height === reported) return;
  reported = height;
  void ask({ type: "sso_resize", token, height });
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
  const r = await ask<SsoView>({ type: "sso_state", token });
  if (r.ok) render(r.value, token);
  else fail(r.message);
}

void init();
