// In-page suggestion menu. Shows titles and usernames only; passwords and
// codes go from the background straight to the content script and never
// pass through this page.

import type { MenuItemView, MenuView } from "../messaging/inline";
import { ask, createClickGuard, h, monogram, tokenFromHash } from "./common";

const main = document.getElementById("main") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const card = document.querySelector(".card") as HTMLElement;
const token = tokenFromHash();
const guard = createClickGuard(card);

function message(title: string, detail: string, error = false): HTMLElement {
  return h("div", { className: "message" }, h("strong", { text: title, ...(error ? { className: "error" } : {}) }), h("span", { text: detail }));
}

function row(avatar: string, title: string, detail: string, onPick: () => Promise<void>): HTMLButtonElement {
  const b = h(
    "button",
    { className: "row" },
    h("span", { className: "avatar", text: avatar }),
    h("span", { className: "who" }, h("span", { className: "title", text: title }), h("span", { className: "user", text: detail })),
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
  if (!r.ok) main.replaceChildren(message("Could not fill", r.message, true));
}

function itemRow(t: string, item: MenuItemView, kind: "login" | "otp"): HTMLButtonElement {
  const detail = kind === "otp" ? "Fill one-time code" : (item.username ?? "No username");
  return row(monogram(item.title), item.title, detail, () => pick({ type: "menu_pick", token: t, itemId: item.id }));
}

function render(t: string, view: MenuView): void {
  if (view.state === "locked") {
    main.replaceChildren(message("HavenKeys is locked", "Unlock the HavenKeys app to fill."));
    return;
  }
  site.textContent = view.site;
  if (view.kind === "new_password") {
    main.replaceChildren(
      row("✱", "Generate strong password", "Fills the new password fields", () => pick({ type: "menu_generate", token: t })),
    );
    return;
  }
  const kind = view.kind;
  main.replaceChildren(...view.items.map((i) => itemRow(t, i, kind)));
}

/** Arrow keys move between rows; Escape closes. */
document.addEventListener("keydown", (e) => {
  if (!token) return;
  if (e.key === "Escape") {
    void ask({ type: "menu_close", token });
    return;
  }
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const rows = Array.from(main.querySelectorAll<HTMLButtonElement>(".row"));
  if (rows.length === 0) return;
  const i = rows.indexOf(document.activeElement as HTMLButtonElement);
  const next = e.key === "ArrowDown" ? (i + 1) % rows.length : (i - 1 + rows.length) % rows.length;
  rows[next]?.focus();
  e.preventDefault();
});

// Focused from the page with ArrowDown: start on the first row.
window.addEventListener("focus", () => {
  if (!main.contains(document.activeElement)) main.querySelector<HTMLButtonElement>(".row")?.focus();
});

async function init(): Promise<void> {
  if (!token) return;
  const r = await ask<MenuView>({ type: "menu_state", token });
  if (!r.ok) {
    main.replaceChildren(message("Unavailable", r.message, true));
    return;
  }
  render(token, r.value);
}

void init();
