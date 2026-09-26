// Content script: runs in each http(s) frame the user granted (inline
// suggestions), or in the top frame of a tab the user filled from the popup.
//
// It has no access to the vault. It classifies fields when the user
// interacts with them, asks the background to open a suggestion menu, fills
// what the background sends after the user picked an item, and reports
// submitted logins so the user can be asked whether to save them.
//
// Rules:
// * Menus open only on trusted user input (click, typing, Tab, ArrowDown,
//   or the field's HavenKeys icon), never on page load or programmatic focus.
//   The icon (content/icon.ts) appears in the focused login field.
// * The page is never scanned up front; see autofill/group.ts.
// * Fills are accepted only from the background, only for the menu the
//   user picked from (or a popup fill), and only if this frame is still on
//   the origin the desktop matched.
// * Automatic sign-in: after a fill the background marked `submit` (Rust's
//   autoSubmit), press the page's button once, then watch for the next step
//   and ask the background to continue. Any trusted key, input or pointer
//   event from the user ends the run.
// * Nothing is logged, stored, or written anywhere but field values.

import { isLoginRole, isNewPasswordRole } from "../autofill/classify";
import { fillLogin, fillNewPassword, fillOtp, markUserEdit, valueSource } from "../autofill/fill";
import { classifyGroup, defaultEnv, fieldsOf, groupFor, groupRoot, isFillable } from "../autofill/group";
import { findLoginGroup, findOtpGroup, readSubmission } from "../autofill/page";
import { findSubmitButton, hasChallenge, pressWhenReady, type PressStep } from "../autofill/submit";
import { hasAny, normalize, SUBMIT_WORDS } from "../autofill/text";
import { watchNext, type WatchKind } from "../autofill/watch";
import {
  parseBackgroundMessage,
  TOKEN,
  type BackgroundToContent,
  type ContentRequest,
  type FillReply,
  type MenuKind,
  type NextStep,
} from "../messaging/inline";
import { InlineFrame, menuBox, saveBox, type Box } from "./frames";
import { FieldIcon, iconBox } from "./icon";

declare global {
  // Set in this content script's isolated world; page script cannot see it.
  var __havenkeysContent: boolean | undefined;
}

/** A fill for a menu must arrive within this long of the pick. */
const PICK_WINDOW_MS = 30_000;
/** Focus within this long of a Tab key press counts as keyboard navigation. */
const TAB_FOCUS_MS = 500;
/** How often a shown field icon follows its field (layout shifts, removal). */
const ICON_TRACK_MS = 500;
/** Submissions closer together than this are the same submission. */
const SUBMIT_DEBOUNCE_MS = 1000;

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
  let icon: { view: FieldIcon; field: HTMLInputElement; timer: ReturnType<typeof setInterval> } | null = null;
  /** Fields where the user closed the menu: typing there does not reopen it. */
  const dismissed = new WeakSet<HTMLInputElement>();
  /** Fields with nothing to offer: typing there does not ask again. */
  const empty = new WeakSet<HTMLInputElement>();
  /** A sign-in run is active in this frame (we pressed, or the background said to watch). */
  let runActive = false;
  /** Bumped whenever a run locally starts or ends, so a stale async press or
   * watch (from a run since ended or superseded) can tell it is no longer
   * current and do nothing. */
  let runSeq = 0;
  let stopWatch: (() => void) | null = null;

  // ------------------------------------------------------------ menu

  function closeMenu(tellBackground: boolean): void {
    if (!menu) return;
    const { frame, field } = menu;
    menu = null;
    frame.remove();
    picked = { token: frame.token, field, until: Date.now() + PICK_WINDOW_MS };
    if (tellBackground) void send({ type: "cs_close_menu", token: frame.token });
    if (icon && deepActiveElement() !== icon.field) hideIcon();
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

  /** `explicit`: from the field icon, so show the menu even with nothing to offer. */
  async function maybeOpen(field: HTMLInputElement, explicit = false): Promise<void> {
    if (opening || menu?.field === field || !isFillable(field, defaultEnv())) return;
    const kind = menuKindFor(field);
    if (!kind) return;
    opening = true;
    const req: ContentRequest = explicit ? { type: "cs_open_menu", kind, explicit: true } : { type: "cs_open_menu", kind };
    const reply = (await send(req)) as { ok?: unknown; token?: unknown; rows?: unknown } | undefined;
    opening = false;
    if (!reply || reply.ok !== true || typeof reply.token !== "string" || !TOKEN.test(reply.token)) {
      if (!explicit) empty.add(field);
      return;
    }
    empty.delete(field);
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

  // ------------------------------------------------------------ field icon

  /**
   * Where the icon goes in `field`: the first spot, from the right edge
   * leftwards, where the field itself is on top (not the page's own
   * show-password or clear button). Null when there is no free spot.
   */
  function iconSpot(field: HTMLInputElement): Box | null {
    const rect = field.getBoundingClientRect();
    for (let shift = 0; shift < 3; shift++) {
      const box = iconBox(rect, shift);
      if (!box) return null;
      // No hit testing (jsdom), or a field in a shadow root, whose host is what we would hit.
      if (typeof document.elementsFromPoint !== "function" || field.getRootNode() !== document) return box;
      const hit = document
        .elementsFromPoint(box.left + box.width / 2, box.top + box.height / 2)
        .find((el) => el !== icon?.view.el);
      if (!hit || hit === field) return box;
    }
    return null;
  }

  function hideIcon(): void {
    if (!icon) return;
    clearInterval(icon.timer);
    icon.view.remove();
    icon = null;
  }

  function placeIcon(): void {
    if (!icon) return;
    const box = icon.field.isConnected && isFillable(icon.field, defaultEnv()) ? iconSpot(icon.field) : null;
    if (box) icon.view.place(box);
    else hideIcon();
  }

  function showIcon(field: HTMLInputElement): void {
    if (icon?.field === field) return placeIcon();
    hideIcon();
    if (!isFillable(field, defaultEnv()) || !menuKindFor(field)) return;
    const box = iconSpot(field);
    if (!box) return;
    const view = new FieldIcon(box, () => toggleFromIcon(field));
    icon = { view, field, timer: setInterval(placeIcon, ICON_TRACK_MS) };
  }

  function toggleFromIcon(field: HTMLInputElement): void {
    if (menu?.field === field) {
      dismissed.add(field);
      closeMenu(true);
      return;
    }
    dismissed.delete(field);
    if (deepActiveElement() !== field) field.focus();
    void maybeOpen(field, true);
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

  // ------------------------------------------------------------ automatic sign-in

  function cancelWatch(): void {
    stopWatch?.();
    stopWatch = null;
  }

  /** End the run here; `tell`: let the background know (not for bg_run_end). */
  function endLocalRun(tell: boolean): void {
    cancelWatch();
    if (runActive && tell) void send({ type: "cs_run_stop" });
    runActive = false;
    runSeq++;
  }

  function startWatch(want: WatchKind, filled: HTMLInputElement | null): void {
    cancelWatch();
    runActive = true;
    runSeq++;
    const mine = runSeq;
    stopWatch = watchNext({
      want,
      filled,
      env: defaultEnv,
      onResult: (r) => {
        stopWatch = null;
        // Superseded by a newer run while waiting (watchNext's own cancel
        // already guarantees this for a watch we replaced ourselves, but a
        // stale continuation could still reach here defensively).
        if (mine !== runSeq) return;
        if ("found" in r) void send({ type: "cs_run_step", kind: r.found });
        else endLocalRun(true);
      },
    });
  }

  /** The step a fill just completed, and the field it ended in. */
  function filledStep(
    group: ReturnType<typeof groupFor>["group"],
    kind: "login" | "otp",
  ): { step: PressStep; last: HTMLInputElement } | null {
    if (kind === "otp") {
      const boxes = fieldsOf(group, "otp");
      const last = boxes[boxes.length - 1];
      return last && valueSource(last) === "vault" ? { step: "otp", last } : null;
    }
    const [pw] = fieldsOf(group, "current-password", "password");
    if (pw) return valueSource(pw) === "vault" ? { step: "password", last: pw } : null;
    const [user] = fieldsOf(group, "username");
    return user && valueSource(user) === "vault" ? { step: "username", last: user } : null;
  }

  /** Press after a fill, then watch for the next step. Returns the step being pressed, or null. */
  function pressAfterFill(group: ReturnType<typeof groupFor>["group"], kind: "login" | "otp", totp: boolean): PressStep | null {
    const done = filledStep(group, kind);
    if (!done) return null;
    const env = defaultEnv();
    const button = findSubmitButton(group.root, done.last, done.step, env);
    if (!button || hasChallenge(document, env)) return null;
    const next: NextStep | null = done.step === "username" ? "password" : done.step === "password" && totp ? "otp" : null;
    cancelWatch();
    runActive = true;
    runSeq++;
    const mine = runSeq;
    void pressWhenReady({ button, field: done.last, step: done.step, env, cancelled: () => mine !== runSeq })
      .then((outcome) => {
        // A later event (user takeover, bg_run_end, a new pick) already ended
        // this run or started another one: do not act on this stale press.
        if (mine !== runSeq) return;
        if (outcome === "gave_up") return endLocalRun(true);
        if (next) startWatch(next, done.last);
        else endLocalRun(false);
      })
      .catch(() => {
        // The press threw (e.g. the page broke requestSubmit): stop this run.
        if (mine === runSeq) endLocalRun(true);
      });
    return done.step;
  }

  // ------------------------------------------------------------ fills

  function handleFill(m: Extract<BackgroundToContent, { type: "bg_fill" }>): FillReply {
    const none: FillReply = { filled: 0, pressing: null };
    // The frame may have navigated since the desktop matched its URL.
    if (m.origin !== location.origin) return none;
    const env = defaultEnv();
    let group;
    if (m.token !== null) {
      const target = picked && picked.token === m.token && picked.until > Date.now() ? picked : null;
      picked = null;
      if (!target || !target.field.isConnected) return none;
      group = groupFor(target.field, env).group;
    } else {
      group = m.fill.kind === "otp" ? findOtpGroup(document, env) : findLoginGroup(document, env);
    }
    if (!group) return none;
    switch (m.fill.kind) {
      case "login": {
        const filled = fillLogin(group, m.fill, env);
        return { filled, pressing: filled > 0 && m.submit ? pressAfterFill(group, "login", m.totp) : null };
      }
      case "otp": {
        const filled = fillOtp(group, m.fill.code, env);
        return { filled, pressing: filled > 0 && m.submit ? pressAfterFill(group, "otp", m.totp) : null };
      }
      case "generated": {
        const n = fillNewPassword(group, m.fill.password, env);
        if (n > 0) generatedIn = group.root;
        return { filled: n, pressing: null };
      }
    }
  }

  // ------------------------------------------------------------ listeners

  const opts = { capture: true, passive: true } as const;

  window.addEventListener(
    "pointerdown",
    (e) => {
      if (!e.isTrusted) return;
      if (runActive) endLocalRun(true); // the user took over
      // The icon's own click handler toggles the menu.
      if (icon && e.composedPath()[0] === icon.view.el) return;
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
      if (runActive) endLocalRun(true); // the user took over
      const input = inputFrom(e);
      switch (e.key) {
        case "Tab":
          lastTab = Date.now();
          return;
        case "Escape":
          if (menu) dismissed.add(menu.field);
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
      if (input) showIcon(input);
      else hideIcon();
      if (input && Date.now() - lastTab < TAB_FOCUS_MS) void maybeOpen(input);
    },
    opts,
  );

  window.addEventListener(
    "focusout",
    (e) => {
      if (!icon || inputFrom(e) !== icon.field) return;
      // Wait for focus to land; our menu frame keeps the icon.
      setTimeout(() => {
        if (icon && deepActiveElement() !== icon.field && menu?.field !== icon.field) hideIcon();
      }, 0);
    },
    opts,
  );

  window.addEventListener(
    "input",
    (e) => {
      const input = inputFrom(e);
      if (!e.isTrusted || !input) return;
      if (runActive) endLocalRun(true); // the user took over
      markUserEdit(input);
      // Typing in a login field (e.g. one the page focused on load) opens
      // the menu, unless the user closed it there or there is nothing to offer.
      if (!menu && !dismissed.has(input) && !empty.has(input) && deepActiveElement() === input) void maybeOpen(input);
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
      if (icon && e.composedPath()[0] === icon.view.el) return;
      const el = (e.composedPath()[0] as Element | undefined)?.closest?.('button, input[type="submit"], input[type="image"], [role="button"]');
      if (!el || !isSubmitLike(el)) return;
      const root = rootForButton(el);
      if (root) captureFrom(root);
    },
    opts,
  );

  const reposition = () => {
    if ((!menu && !icon) || rafPending) return;
    rafPending = true;
    requestAnimationFrame(() => {
      rafPending = false;
      placeMenu();
      placeIcon();
    });
  };
  window.addEventListener("scroll", reposition, opts);
  window.addEventListener("resize", reposition, opts);
  // SPA route changes and page teardown close the menu.
  window.addEventListener("popstate", () => closeMenu(true), opts);
  window.addEventListener("hashchange", () => closeMenu(true), opts);
  window.addEventListener("pagehide", () => {
    closeMenu(true);
    hideIcon();
    closeSave();
    // A generated password must not be lost because we missed the submit
    // (e.g. a script-driven signup): offer it as the page goes away.
    // readSubmission only reports it if the field still holds our value.
    if (generatedIn) captureFrom(generatedIn, true);
    // Navigation is expected mid-run; the next page asks with cs_ready.
    endLocalRun(false);
  });

  // ------------------------------------------------------------ background

  chrome.runtime.onMessage.addListener((raw: unknown, sender, sendResponse) => {
    // Only the background worker (no tab) of this extension.
    if (sender.id !== chrome.runtime.id || sender.tab !== undefined) return false;
    const m = parseBackgroundMessage(raw);
    if (!m) return false;
    switch (m.type) {
      case "bg_fill":
        sendResponse(handleFill(m));
        return false;
      case "bg_close_menu":
        // Escape in the menu, or a pick: either way the user is done with it here.
        if (menu?.frame.token === m.token) {
          dismissed.add(menu.field);
          closeMenu(false);
        }
        return false;
      case "bg_show_save":
        showSave(m.token);
        return false;
      case "bg_close_save":
        if (saveFrame?.token === m.token) closeSave();
        return false;
      case "bg_run_end":
        endLocalRun(false);
        return false;
    }
  });

  // A field the page focused before we loaded gets its icon (not a menu).
  const focused = deepActiveElement();
  if (focused instanceof HTMLInputElement) showIcon(focused);

  // A save prompt for a login submitted just before this page loaded (top
  // frame only), and the next step of a sign-in run in progress.
  void send({ type: "cs_ready" }).then((r) => {
    const reply = r as { saveToken?: unknown; watch?: unknown } | undefined;
    if (window.top === window && typeof reply?.saveToken === "string" && TOKEN.test(reply.saveToken)) showSave(reply.saveToken);
    if (reply?.watch === "password" || reply?.watch === "otp") startWatch(reply.watch, null);
  });
}

if (!globalThis.__havenkeysContent) {
  globalThis.__havenkeysContent = true;
  start();
}
