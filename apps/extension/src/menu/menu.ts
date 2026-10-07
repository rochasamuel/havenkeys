// In-page suggestion menu. Shows titles and usernames only; passwords and
// codes go from the background straight to the content script and never
// pass through this page.

import { MAX_PASSWORD_LENGTH, MIN_PASSWORD_LENGTH, SSO_PROVIDERS, type IdentityRole, type PasswordOptions } from "@havenkeys/protocol";
import { applyDocumentLang, t as msg } from "../i18n";
import { MENU_MAX_HEIGHT, MENU_MAX_ROWS, MENU_MIN_HEIGHT, type CardRowView, type IdentityRowView, type MenuItemView, type MenuView } from "../messaging/inline";
import { tagLine } from "../shared/tags";
import { ask, createClickGuard, h, monogram, receiveToken, userData } from "./common";
import { cardBrandIcon, idCardIcon, providerIcon, switchesIcon, unlockIcon } from "./icons";

const main = document.getElementById("main") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const card = document.querySelector(".card") as HTMLElement;
/** Set once the content script posts it (`receiveToken`). */
let token: string | null = null;
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

/**
 * A login's user line. With tags, the username comes first and the tags
 * follow, very small and faint; the tags are what shrink when the row is full.
 */
function userLine(detail: string, copy: boolean, tags: readonly string[]): HTMLElement {
  const text = tagLine(tags);
  if (!text) return line("user", detail, copy);
  const name = copy ? h("span", { className: "user-name copy", text: detail }) : userData(h("span", { className: "user-name", text: detail }));
  return h("span", { className: "user with-tags" }, name, userData(h("span", { className: "tags", text: `\u00b7 ${text}` })));
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

function row(avatar: string | Node, title: string, detail: string, onPick: () => Promise<void>, copy: Copy = DATA, tags: readonly string[] = []): HTMLButtonElement {
  const b = h(
    "button",
    { className: "row" },
    typeof avatar === "string" ? h("span", { className: "avatar", text: avatar }) : h("span", { className: "avatar avatar-icon" }, avatar),
    h("span", { className: "who" }, line("title", title, copy.title), userLine(detail, copy.detail, tags)),
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
  if (kind === "login" && item.provider) {
    const name = SSO_PROVIDERS[item.provider].name;
    return row(providerIcon(item.provider), item.title, msg.menu.ssoRow(name, item.username), () => pick({ type: "menu_pick", token: t, itemId: item.id }), {
      title: false,
      detail: item.username === null,
    }, kind === "login" ? item.tags : []);
  }
  // A code row names its account too: logins often share a title.
  const detail =
    item.username === null ? (kind === "otp" ? msg.menu.fillCode : msg.common.noUsername) : kind === "otp" ? msg.menu.codeRow(item.username) : item.username;
  const copy = { title: false, detail: item.username === null };
  return row(monogram(item.title), item.title, detail, () => pick({ type: "menu_pick", token: t, itemId: item.id }), copy, kind === "login" ? item.tags : []);
}

function documentList(roles: readonly IdentityRole[]): string {
  return roles.map((r) => msg.menu.documentLabels[r as keyof typeof msg.menu.documentLabels] ?? r).join(msg.menu.and);
}

/**
 * A plain action button for the documents step, under the click guard and
 * the step's own guard (re-armed when the step appears, so a fast second
 * click or Enter on the row cannot land on "Fill CPF too").
 */
function action(text: string, primary: boolean, stepGuard: { armed(): boolean }, onPick: () => Promise<void>): HTMLButtonElement {
  const b = h("button", { className: primary ? "step-btn primary" : "step-btn", text });
  b.type = "button";
  b.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed() || !stepGuard.armed() || b.disabled) return;
    b.disabled = true;
    void onPick().finally(() => (b.disabled = false));
  });
  return b;
}

function documentsStep(t: string, site: string, docs: readonly IdentityRole[]): void {
  const list = documentList(docs);
  const step = h("div", { className: "message step" });
  const stepGuard = createClickGuard(step);
  const withDocs = action(msg.menu.identityFillWithDocs(list), true, stepGuard, () => pick({ type: "menu_pick_identity", token: t, documents: true }));
  const withoutDocs = action(msg.menu.identityFillWithoutDocs, false, stepGuard, () =>
    pick({ type: "menu_pick_identity", token: t, documents: false }),
  );
  step.append(h("strong", { text: msg.menu.identityAlsoAsks(site, list) }), withDocs, withoutDocs);
  main.replaceChildren(step);
  // The row that had focus is gone. Keep keyboard users in the menu, on the
  // safe choice: a second Enter must never confirm the documents.
  withoutDocs.focus();
}

function identityRow(t: string, site: string, v: IdentityRowView): HTMLElement {
  // No identity on this device yet: nothing to open, so no button.
  if (v.missing) return hintNote(msg.menu.identityEmptyTitle, msg.menu.identityMissingBody);
  if (v.empty) {
    return row(idCardIcon(), msg.menu.identityEmptyTitle, msg.menu.identityEmptyBody, () => pick({ type: "menu_open_identity", token: t }), {
      title: true,
      detail: true,
    });
  }
  const askFirst = v.documents.length > 0 && v.documentsAllowed;
  const detail = v.documents.length > 0 && !v.documentsAllowed ? msg.menu.identityNoDocsHttp : msg.menu.identityFills(v.fills);
  return row(
    idCardIcon(),
    v.title || msg.menu.identityFallback,
    detail,
    async () => {
      if (askFirst) documentsStep(t, site, v.documents);
      else await pick({ type: "menu_pick_identity", token: t, documents: false });
    },
    { title: v.title === "", detail: true },
  );
}

function cardRow(t: string, c: CardRowView): HTMLButtonElement {
  const detail = [msg.menu.cardRow(c.last4, c.expiry), c.expired ? msg.menu.cardExpired : ""].filter(Boolean).join(" · ");
  const b = row(cardBrandIcon(c.brand), c.title || msg.menu.cardFallback, detail, () => pick({ type: "menu_pick_card", token: t, itemId: c.id }), {
    title: c.title === "",
    detail: true,
  });
  if (c.expired) b.classList.add("expired");
  return b;
}

type CharClass = "uppercase" | "lowercase" | "digits" | "symbols";

/** Chip text is the same in every language; the full name is the accessible label. */
const CLASSES: ReadonlyArray<{ key: CharClass; text: string; label: () => string }> = [
  { key: "uppercase", text: "A–Z", label: () => msg.menu.generateUppercaseLabel },
  { key: "lowercase", text: "a–z", label: () => msg.menu.generateLowercaseLabel },
  { key: "digits", text: "0–9", label: () => msg.menu.generateDigitsLabel },
  { key: "symbols", text: "!@#", label: () => msg.menu.generateSymbolsLabel },
];

/**
 * The new-password menu: "Generate strong password" with a settings button
 * inside the row. The row generates with the policy saved in the desktop's
 * generator tab (Rust picks it; we send none). The button opens a panel (the
 * frame slides taller) showing that policy; what the user changes there
 * applies to this password only and is not saved. The password itself never
 * passes through this page.
 */
function generateRows(t: string): HTMLElement[] {
  /** The panel's policy while it is open. */
  let chosen: PasswordOptions | null = null;
  const generate = (): Promise<void> => pick({ type: "menu_generate", token: t, ...(chosen ? { options: { ...chosen } } : {}) });

  const genRow = row(sparkle(), msg.menu.generateTitle, msg.menu.generateBody, generate, { title: true, detail: true });
  const toggle = h("button", { className: "icon-btn gen-toggle" }, switchesIcon());
  toggle.type = "button";
  toggle.title = msg.menu.generateSettings;
  toggle.setAttribute("aria-label", msg.menu.generateSettings);
  toggle.setAttribute("aria-expanded", "false");
  toggle.setAttribute("aria-controls", "gen-panel");
  const wrap = h("div", { className: "gen" }, genRow, toggle);

  let panel: HTMLElement | null = null;
  let loading = false;
  const close = (): void => {
    panel?.remove();
    panel = null;
    chosen = null;
    toggle.setAttribute("aria-expanded", "false");
    slide();
  };
  toggle.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed() || loading) return;
    if (panel) {
      close();
      return;
    }
    loading = true;
    void ask<PasswordOptions>({ type: "menu_generator_options", token: t }).then((r) => {
      loading = false;
      if (!wrap.isConnected) return;
      if (r.ok) {
        chosen = { ...r.value };
        panel = settingsPanel(chosen, generate);
      } else {
        panel = message(msg.menu.unavailable, r.message, true);
        panel.id = "gen-panel";
      }
      toggle.setAttribute("aria-expanded", "true");
      slide();
      main.append(panel);
      panel.querySelector<HTMLInputElement>("input[type=range]")?.focus();
    });
  });
  return [wrap];
}

/** Edits `o` in place: the length and classes the next generate sends. */
function settingsPanel(o: PasswordOptions, generate: () => Promise<void>): HTMLElement {
  const panel = h("div", { className: "gen-panel" });
  panel.id = "gen-panel";
  panel.setAttribute("role", "group");
  panel.setAttribute("aria-label", msg.menu.generateSettings);

  const value = h("output", { className: "gen-value", text: String(o.length) });
  const range = h("input", { className: "gen-range" });
  range.type = "range";
  range.min = String(MIN_PASSWORD_LENGTH);
  range.max = String(MAX_PASSWORD_LENGTH);
  range.step = "1";
  range.value = String(o.length);
  range.id = "gen-length";
  range.setAttribute("aria-label", msg.menu.generateLength);
  const paint = (): void => {
    const pct = ((o.length - MIN_PASSWORD_LENGTH) / (MAX_PASSWORD_LENGTH - MIN_PASSWORD_LENGTH)) * 100;
    range.style.setProperty("--fill", `${pct}%`);
  };
  paint();
  range.addEventListener("input", () => {
    o.length = Number(range.value);
    value.textContent = range.value;
    paint();
  });
  const lengthLabel = h("label", { className: "gen-label", text: msg.menu.generateLength });
  lengthLabel.htmlFor = range.id;

  const boxes: HTMLInputElement[] = [];
  const chips = CLASSES.map(({ key, text, label }) => {
    const box = h("input");
    box.type = "checkbox";
    box.checked = o[key];
    box.setAttribute("aria-label", label());
    box.addEventListener("change", () => {
      // At least one class: the last one on cannot be turned off.
      if (!box.checked && !boxes.some((b) => b.checked)) {
        box.checked = true;
        return;
      }
      o[key] = box.checked;
    });
    boxes.push(box);
    const chip = h("label", { className: "chip" }, box, h("span", { text }));
    chip.title = label();
    return chip;
  });

  const go = h("button", { className: "btn primary gen-go", text: msg.menu.generateFill });
  go.type = "button";
  // Re-armed when the panel appears: a click meant for the toggle cannot land here.
  const panelGuard = createClickGuard(panel);
  go.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed() || !panelGuard.armed() || go.disabled) return;
    go.disabled = true;
    void generate().finally(() => (go.disabled = false));
  });

  panel.append(
    h("div", { className: "gen-line" }, lengthLabel, value),
    range,
    h("div", { className: "gen-line" }, h("span", { className: "gen-label", text: msg.menu.generateCharacters })),
    h("div", { className: "chips" }, ...chips),
    go,
  );
  return panel;
}

/**
 * Locked: the message with an Unlock button that brings the desktop app
 * forward on its unlock screen. The master password is typed there, never
 * here; the background closes this frame so the window can be seen.
 */
function lockedMessage(t: string): HTMLElement {
  const unlock = h("button", { className: "icon-btn unlock" }, unlockIcon());
  unlock.type = "button";
  unlock.title = msg.menu.unlock;
  unlock.setAttribute("aria-label", msg.menu.unlock);
  const box = h("div", { className: "message locked" }, h("div", { className: "message-text" }, h("strong", { text: msg.menu.lockedTitle }), h("span", { text: msg.menu.lockedBody })), unlock);
  unlock.addEventListener("click", (e) => {
    if (!e.isTrusted || !guard.armed() || unlock.disabled) return;
    unlock.disabled = true;
    void ask<null>({ type: "menu_show_unlock", token: t }).then((r) => {
      unlock.disabled = false;
      if (!r.ok) main.replaceChildren(message(msg.menu.unavailable, r.message, true));
    });
  });
  return box;
}

/** A row that only informs: no button, not in the arrow-key order. */
function hintNote(title: string, detail: string): HTMLElement {
  return h(
    "div",
    { className: "row hint" },
    h("span", { className: "who" }, line("title", title, true), line("user", detail, true)),
  );
}

/** Characters of the host shown at most, the leading "…" included. */
const DEST_HOST_MAX = 32;

/**
 * The host as shown under "Fill on": whole when short, else its END (the
 * registrable domain, the part a look-alike cannot copy) after a leading
 * "…", cut at a label boundary when one falls inside the kept tail.
 * `shop.com.evil.xyz` and `shop.com` must never read the same.
 */
function hostTail(host: string, max = DEST_HOST_MAX): string {
  if (host.length <= max) return host;
  let tail = host.slice(host.length - (max - 1));
  // Snap to a label only while that keeps most of the tail ("…xyz" says little).
  const dot = tail.indexOf(".");
  if (dot >= 0 && tail.length - dot - 1 >= max / 2) tail = tail.slice(dot + 1);
  return `…${tail}`;
}

/**
 * The card menu's header: "Fill on" beside the brand, the host on its own
 * line below. The CSS also clips that line from the start (rtl box, the
 * host isolated as ltr), so its end stays visible at any width.
 */
function showDestination(host: string): void {
  site.textContent = msg.menu.cardFillOn;
  const head = card.firstElementChild as HTMLElement | null;
  if (!head) return;
  head.classList.add("dest");
  const text = h("bdi", { text: hostTail(host) });
  text.dir = "ltr";
  const line = userData(h("span", { className: "dest-host" }, text));
  line.title = host;
  head.querySelector(".dest-host")?.remove();
  head.append(line);
}

function render(t: string, view: MenuView): void {
  if (view.state === "locked") {
    main.replaceChildren(lockedMessage(t));
    return;
  }
  if (view.state === "cards") {
    // The page the card goes to, named once in the header (spec §5.2).
    showDestination(view.site);
    if (view.insecure) main.replaceChildren(message(msg.menu.cardsInsecureTitle, msg.menu.cardsInsecureBody));
    else if (view.cards.length === 0) main.replaceChildren(hintNote(msg.menu.noCardsTitle, msg.menu.noCardsBody));
    else main.replaceChildren(...view.cards.map((c) => cardRow(t, c)));
    return;
  }
  site.textContent = view.site;
  // Card menus are always the "cards" state; a "ready" view never carries them.
  if (view.kind === "card") {
    main.replaceChildren(message(msg.menu.unavailable, msg.menu.cardNothing));
    return;
  }
  if (view.kind === "new_password") {
    main.replaceChildren(...generateRows(t));
    return;
  }
  if (view.kind === "identity") {
    main.replaceChildren(view.identity ? identityRow(t, view.site, view.identity) : message(msg.menu.unavailable, msg.menu.identityNothing));
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
  const identity = view.identity ? [identityRow(t, view.site, view.identity)] : [];
  const rows = [...lead, ...passkeyRows, ...view.items.map((i) => itemRow(t, i, kind)), ...identity, ...tail];
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
  // The length slider and the class boxes keep their own arrow keys.
  if (document.activeElement instanceof HTMLInputElement) return;
  const rows = Array.from(main.querySelectorAll<HTMLButtonElement>("button.row, button.step-btn, button.gen-toggle, button.unlock"));
  if (rows.length === 0) return;
  const i = rows.indexOf(document.activeElement as HTMLButtonElement);
  const next = e.key === "ArrowDown" ? (i + 1) % rows.length : (i - 1 + rows.length) % rows.length;
  rows[next]?.focus();
  e.preventDefault();
});

// Focused from the page with ArrowDown: start on the first row. A click also
// focuses the frame (pointerdown comes first); then focus must stay where the
// click lands, or moving it to the first row scrolls a long list and the
// click ends on another row, picking nothing.
let pointerFocus = false;
window.addEventListener(
  "pointerdown",
  () => {
    pointerFocus = true;
    setTimeout(() => (pointerFocus = false), 0);
  },
  { capture: true },
);
window.addEventListener("focus", () => {
  if (pointerFocus) return;
  if (!main.contains(document.activeElement)) main.querySelector<HTMLButtonElement>("button.row, button.unlock")?.focus();
});

// ------------------------------------------------------------ size
// The content script first sizes our iframe from the row count (46px rows).
// Rows whose copy wraps (longer languages, narrow fields) are taller, so we
// report the height the first MENU_MAX_ROWS rows need and it resizes the
// frame (menu_resize → bg_resize_menu). Past that many rows the list scrolls.

let reported = 0;
/** The next report follows a panel opening or closing: the frame slides. */
let animateNext = false;
let slideTimer: ReturnType<typeof setTimeout> | undefined;

/** Open or close a panel: slide the frame, no scrollbar while it moves. */
function slide(): void {
  animateNext = true;
  main.classList.add("sliding");
  clearTimeout(slideTimer);
  slideTimer = setTimeout(() => main.classList.remove("sliding"), 260);
}

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
  const animate = animateNext;
  animateNext = false;
  if (!token || main.childElementCount === 0) return;
  const height = Math.min(MENU_MAX_HEIGHT, Math.max(MENU_MIN_HEIGHT, naturalHeight()));
  if (height === reported) return;
  reported = height;
  void ask({ type: "menu_resize", token, height, ...(animate ? { animate: true as const } : {}) });
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
  token = await receiveToken();
  if (!token) return;
  const r = await ask<MenuView>({ type: "menu_state", token });
  if (!r.ok) {
    main.replaceChildren(message(msg.menu.unavailable, r.message, true));
    return;
  }
  render(token, r.value);
}

void init();
