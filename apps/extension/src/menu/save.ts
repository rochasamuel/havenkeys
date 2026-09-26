// "Save login?" prompt. Shows the site and username only; the password
// stays in the background until the user confirms, and is dropped if they
// don't.

import { SAVE_MAX_HEIGHT, SAVE_MIN_HEIGHT, type SaveView } from "../messaging/inline";
import { applyDocumentLang, t } from "../i18n";
import { ask, createClickGuard, tokenFromHash } from "./common";

const question = document.getElementById("question") as HTMLElement;
const detail = document.getElementById("detail") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const confirmBtn = document.getElementById("confirm") as HTMLButtonElement;
const dismissBtn = document.getElementById("dismiss") as HTMLButtonElement;
const token = tokenFromHash();
const card = document.querySelector(".card") as HTMLElement;
const guard = createClickGuard(card);

applyDocumentLang();
document.title = t.save.pageTitle;
question.textContent = t.save.loading;
dismissBtn.textContent = t.save.notNow;
confirmBtn.textContent = t.save.save;

function show(view: SaveView): void {
  site.textContent = view.site;
  if (view.action === "update") {
    question.textContent = t.save.updateQuestion;
    confirmBtn.textContent = t.save.update;
  } else {
    question.textContent = t.save.addQuestion;
    confirmBtn.textContent = t.save.save;
  }
  detail.textContent = view.username ?? t.common.noUsername;
  confirmBtn.disabled = false;
}

function fail(message: string): void {
  // The error takes the question's place; the prompt grows to fit it (save_resize).
  detail.textContent = "";
  question.textContent = message;
  question.className = "error";
  confirmBtn.disabled = true;
}

confirmBtn.addEventListener("click", async (e) => {
  if (!token || !e.isTrusted || !guard.armed()) return;
  confirmBtn.disabled = true;
  const r = await ask<null>({ type: "save_confirm", token });
  // On success the background closes this frame.
  if (!r.ok) fail(r.message);
});

dismissBtn.addEventListener("click", (e) => {
  if (!token || !e.isTrusted) return;
  void ask({ type: "save_dismiss", token });
});

// ------------------------------------------------------------ size
// The content script opens our iframe at SAVE_HEIGHT. A longer question or
// error (other languages, long messages) wraps, so we report the height the
// card's content needs and it resizes the frame (save_resize → bg_resize_save).

let reported = 0;

function naturalHeight(): number {
  let total = card.offsetHeight - card.clientHeight; // borders
  for (const el of Array.from(card.children) as HTMLElement[]) total += el.getBoundingClientRect().height;
  return Math.ceil(total);
}

function reportSize(): void {
  if (!token) return;
  const height = Math.min(SAVE_MAX_HEIGHT, Math.max(SAVE_MIN_HEIGHT, naturalHeight()));
  if (height === reported) return;
  reported = height;
  void ask({ type: "save_resize", token, height });
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
  const r = await ask<SaveView>({ type: "save_state", token });
  if (r.ok) show(r.value);
  else fail(r.message);
}

void init();
