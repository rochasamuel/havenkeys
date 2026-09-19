// Content script: runs in each http(s) frame the user granted (inline
// suggestions), or in the top frame of a tab the user filled from the popup.
//
// It has no access to the vault. It classifies fields when the user
// interacts with them, asks the background to open a suggestion menu, fills
// what the background sends after the user picked an item, and reports
// submitted logins so the user can be asked whether to save them.
//
// Rules:
// * Menus open only on trusted user input (click, Tab, ArrowDown), never on
//   page load or programmatic focus.
// * The page is never scanned up front; see autofill/group.ts.
// * Fills are accepted only from the background, only for the menu the
//   user picked from (or a popup fill), and only if this frame is still on
//   the origin the desktop matched.
// * Nothing is logged, stored, or written anywhere but field values.

import { isLoginRole, isNewPasswordRole } from "../autofill/classify";
import { fillLogin, fillNewPassword, fillOtp, markUserEdit } from "../autofill/fill";
import { classifyGroup, defaultEnv, groupFor, groupRoot, isFillable } from "../autofill/group";
import { findLoginGroup, findOtpGroup, readSubmission } from "../autofill/page";
import { hasAny, normalize } from "../autofill/text";
import {
  parseBackgroundMessage,
  TOKEN,
  type BackgroundToContent,
  type ContentRequest,
  type MenuKind,
} from "../messaging/inline";
import { InlineFrame, menuBox, saveBox } from "./frames";

declare global {
  // Set in this content script's isolated world; page script cannot see it.
  var __havenkeysContent: boolean | undefined;
}

/** A fill for a menu must arrive within this long of the pick. */
const PICK_WINDOW_MS = 30_000;
/** Focus within this long of a Tab key press counts as keyboard navigation. */
const TAB_FOCUS_MS = 500;
/** Submissions closer together than this are the same submission. */
const SUBMIT_DEBOUNCE_MS = 1000;
/** Labels of form-less buttons that submit a login. */
const SUBMIT_WORDS = [
  "sign in",
  "log in",
  "login",
  "signin",
  "continue",
  "next",
  "submit",
  "sign up",
  "register",
  "create account",
  "save",
  "update",
  "change password",
  "entrar",
  "acessar",
  "continuar",
  "proximo",
  "avancar",
  "cadastrar",
  "salvar",
];

interface OpenMenu {
  frame: InlineFrame;
  field: HTMLInputElement;
  rows: number;
}

function send(msg: ContentRequest): Promise<unknown> {
  return chrome.runtime.sendMessage(msg).catch(() => undefined);
}

function deepActiveElement(): Element | null {
  let a = document.activeElement;
  while (a?.shadowRoot?.activeElement) a = a.shadowRoot.activeElement;
  return a;
}

function inputFrom(e: Event): HTMLInputElement | null {
  const t = e.composedPath()[0];
  return t instanceof HTMLInputElement ? t : null;
}

function menuKindFor(field: HTMLInputElement): MenuKind | null {
  const { kind } = groupFor(field, defaultEnv());
  if (isLoginRole(kind)) return "login";
  if (isNewPasswordRole(kind)) return "new_password";
  if (kind === "otp") return "otp";
  return null;
}

function start(): void {
  let menu: OpenMenu | null = null;
  /** The last menu the user could pick from, kept briefly for its fill. */
  let picked: { token: string; field: HTMLInputElement; until: number } | null = null;
  let saveFrame: InlineFrame | null = null;
  /** A group we filled a generated password into and have not seen submitted. */
  let generatedIn: ParentNode | null = null;
  let opening = false;
  let lastTab = 0;
  let lastSubmit = 0;
  let rafPending = false;

  // ------------------------------------------------------------ menu

  function closeMenu(tellBackground: boolean): void {
    if (!menu) return;
    const { frame, field } = menu;
    menu = null;
    frame.remove();
    picked = { token: frame.token, field, until: Date.now() + PICK_WINDOW_MS };
    if (tellBackground) void send({ type: "cs_close_menu", token: frame.token });
  }

  function viewport() {
    return { width: document.documentElement.clientWidth || innerWidth, height: innerHeight };
  }

  function placeMenu(): void {
    if (!menu) return;
    if (!menu.field.isConnected || !isFillable(menu.field, defaultEnv())) {
      closeMenu(true);
      return;
    }
    menu.frame.place(menuBox(menu.field.getBoundingClientRect(), menu.rows, viewport()));
  }

  async function maybeOpen(field: HTMLInputElement): Promise<void> {
    if (opening || menu?.field === field || !isFillable(field, defaultEnv())) return;
    const kind = menuKindFor(field);
    if (!kind) return;
    opening = true;
    const reply = (await send({ type: "cs_open_menu", kind })) as { ok?: unknown; token?: unknown; rows?: unknown } | undefined;
    opening = false;
    if (!reply || reply.ok !== true || typeof reply.token !== "string" || !TOKEN.test(reply.token)) return;
    const rows = typeof reply.rows === "number" && reply.rows >= 1 && reply.rows <= 5 ? reply.rows : 1;
    // The user moved on while we asked.
    if (deepActiveElement() !== field) {
      void send({ type: "cs_close_menu", token: reply.token });
      return;
    }
    closeMenu(true);
    const token = reply.token;
    const frame = new InlineFrame("menu.html", token, menuBox(field.getBoundingClientRect(), rows, viewport()), () => {
      if (menu?.frame.token === token) closeMenu(true);
    });
    menu = { frame, field, rows };
  }

  // ------------------------------------------------------------ save prompt

  function closeSave(): void {
    saveFrame?.remove();
    saveFrame = null;
  }

  function showSave(token: string): void {
    if (window.top !== window) return;
    closeSave();
    saveFrame = new InlineFrame("save.html", token, saveBox(viewport()), () => {
      if (saveFrame?.token === token) closeSave();
    });
  }

  // ------------------------------------------------------------ submissions

  function captureFrom(root: ParentNode, force = false): void {
    const now = Date.now();
    if (!force && now - lastSubmit < SUBMIT_DEBOUNCE_MS) return;
    const sub = readSubmission(classifyGroup(root, defaultEnv()));
    if (!sub) return;
    lastSubmit = now;
    if (sub.password !== null) generatedIn = null;
    void send({ type: "cs_submit", username: sub.username, password: sub.password });
  }

  /** The group a submit button belongs to: its form, or the nearest container with a login field. */
  function rootForButton(button: Element): ParentNode | null {
    const form = button.closest("form");
    if (form) return form;
    let node: Element | null = button.parentElement;
    for (let i = 0; node && i < 6; i++, node = node.parentElement) {
      const field = node.querySelector<HTMLInputElement>('input[type="password"], input[type="email"], input[autocomplete~="username"]');
      if (field) return groupRoot(field);
    }
    return null;
  }

  /**
   * A form's submit button, or (outside forms, as in many SPAs) a button
   * whose label says it submits. "Show password" toggles and the like must
   * not count, or we would read a half-typed password.
   */
  function isSubmitLike(el: Element): boolean {
    if (el instanceof HTMLInputElement) return el.type === "submit" || el.type === "image";
    if (el instanceof HTMLButtonElement && el.form) return el.type === "submit";
    const label = normalize(`${el.textContent ?? ""} ${el.getAttribute("aria-label") ?? ""}`);
    return hasAny(label, SUBMIT_WORDS);
  }

  // ------------------------------------------------------------ fills

  function handleFill(m: Extract<BackgroundToContent, { type: "bg_fill" }>): number {
    // The frame may have navigated since the desktop matched its URL.
    if (m.origin !== location.origin) return 0;
    const env = defaultEnv();
    let group;
    if (m.token !== null) {
      const target = picked && picked.token === m.token && picked.until > Date.now() ? picked : null;
      picked = null;
      if (!target || !target.field.isConnected) return 0;
      group = groupFor(target.field, env).group;
    } else {
      group = m.fill.kind === "otp" ? findOtpGroup(document, env) : findLoginGroup(document, env);
    }
    if (!group) return 0;
    switch (m.fill.kind) {
      case "login":
        return fillLogin(group, m.fill, env);
      case "otp":
        return fillOtp(group, m.fill.code, env);
      case "generated": {
        const n = fillNewPassword(group, m.fill.password, env);
        if (n > 0) generatedIn = group.root;
        return n;
      }
    }
  }

  // ------------------------------------------------------------ listeners

  const opts = { capture: true, passive: true } as const;

  window.addEventListener(
    "pointerdown",
    (e) => {
      if (!e.isTrusted) return;
      const input = inputFrom(e);
      if (!input) {
        closeMenu(true);
        return;
      }
      if (menu && menu.field !== input) closeMenu(true);
      // Let focus settle on the field before asking.
      setTimeout(() => void maybeOpen(input), 0);
    },
    opts,
  );

  window.addEventListener(
    "keydown",
    (e) => {
      if (!e.isTrusted) return;
      const input = inputFrom(e);
      switch (e.key) {
        case "Tab":
          lastTab = Date.now();
          return;
        case "Escape":
          closeMenu(true);
          return;
        case "ArrowDown":
          if (input && menu?.field === input) menu.frame.focus();
          else if (input) void maybeOpen(input);
          return;
        case "Enter":
          if (input && (input.type === "password" || input.form)) captureFrom(input.form ?? groupRoot(input));
          return;
      }
    },
    { capture: true },
  );

  window.addEventListener(
    "focusin",
    (e) => {
      // Focus moving into our own menu (a click or ArrowDown) keeps it open.
      if (menu && e.composedPath()[0] === menu.frame.el) return;
      const input = inputFrom(e);
      if (menu && input !== menu.field) closeMenu(true);
      if (input && Date.now() - lastTab < TAB_FOCUS_MS) void maybeOpen(input);
    },
    opts,
  );

  window.addEventListener(
    "input",
    (e) => {
      const input = inputFrom(e);
      if (e.isTrusted && input) markUserEdit(input);
    },
    opts,
  );

  window.addEventListener(
    "submit",
    (e) => {
      const form = e.composedPath()[0];
      if (form instanceof HTMLFormElement) captureFrom(form);
    },
    opts,
  );

  window.addEventListener(
    "click",
    (e) => {
      if (!e.isTrusted) return;
      const el = (e.composedPath()[0] as Element | undefined)?.closest?.('button, input[type="submit"], input[type="image"], [role="button"]');
      if (!el || !isSubmitLike(el)) return;
      const root = rootForButton(el);
      if (root) captureFrom(root);
    },
    opts,
  );

  const reposition = () => {
    if (!menu || rafPending) return;
    rafPending = true;
    requestAnimationFrame(() => {
      rafPending = false;
      placeMenu();
    });
  };
  window.addEventListener("scroll", reposition, opts);
  window.addEventListener("resize", reposition, opts);
  // SPA route changes and page teardown close the menu.
  window.addEventListener("popstate", () => closeMenu(true), opts);
  window.addEventListener("hashchange", () => closeMenu(true), opts);
  window.addEventListener("pagehide", () => {
    closeMenu(true);
    closeSave();
    // A generated password must not be lost because we missed the submit
    // (e.g. a script-driven signup): offer it as the page goes away.
    // readSubmission only reports it if the field still holds our value.
    if (generatedIn) captureFrom(generatedIn, true);
  });

  // ------------------------------------------------------------ background

  chrome.runtime.onMessage.addListener((raw: unknown, sender, sendResponse) => {
    // Only the background worker (no tab) of this extension.
    if (sender.id !== chrome.runtime.id || sender.tab !== undefined) return false;
    const m = parseBackgroundMessage(raw);
    if (!m) return false;
    switch (m.type) {
      case "bg_fill":
        sendResponse({ filled: handleFill(m) });
        return false;
      case "bg_close_menu":
        if (menu?.frame.token === m.token) closeMenu(false);
        return false;
      case "bg_show_save":
        showSave(m.token);
        return false;
      case "bg_close_save":
        if (saveFrame?.token === m.token) closeSave();
        return false;
    }
  });

  // A save prompt for a login submitted just before this page loaded.
  if (window.top === window) {
    void send({ type: "cs_ready" }).then((r) => {
      const token = (r as { saveToken?: unknown } | undefined)?.saveToken;
      if (typeof token === "string" && TOKEN.test(token)) showSave(token);
    });
  }
}

if (!globalThis.__havenkeysContent) {
  globalThis.__havenkeysContent = true;
  start();
}
