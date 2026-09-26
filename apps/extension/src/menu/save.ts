// "Save login?" prompt. Shows the site and username only; the password
// stays in the background until the user confirms, and is dropped if they
// don't.

import type { SaveView } from "../messaging/inline";
import { applyDocumentLang, t } from "../i18n";
import { ask, createClickGuard, tokenFromHash } from "./common";

const question = document.getElementById("question") as HTMLElement;
const detail = document.getElementById("detail") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const confirmBtn = document.getElementById("confirm") as HTMLButtonElement;
const dismissBtn = document.getElementById("dismiss") as HTMLButtonElement;
const token = tokenFromHash();
const guard = createClickGuard(document.querySelector(".card") as HTMLElement);

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
  // The error takes the username's line: the prompt has a fixed height
  // (content/frames.ts SAVE_HEIGHT) and room for a two-line message only.
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

async function init(): Promise<void> {
  if (!token) return;
  const r = await ask<SaveView>({ type: "save_state", token });
  if (r.ok) show(r.value);
  else fail(r.message);
}

void init();
