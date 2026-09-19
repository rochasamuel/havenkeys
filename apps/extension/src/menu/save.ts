// "Save login?" prompt. Shows the site and username only; the password
// stays in the background until the user confirms, and is dropped if they
// don't.

import type { SaveView } from "../messaging/inline";
import { ask, createClickGuard, tokenFromHash } from "./common";

const question = document.getElementById("question") as HTMLElement;
const detail = document.getElementById("detail") as HTMLElement;
const site = document.getElementById("site") as HTMLElement;
const confirmBtn = document.getElementById("confirm") as HTMLButtonElement;
const dismissBtn = document.getElementById("dismiss") as HTMLButtonElement;
const token = tokenFromHash();
const guard = createClickGuard(document.querySelector(".card") as HTMLElement);

function show(view: SaveView): void {
  site.textContent = view.site;
  if (view.action === "update") {
    question.textContent = "Update the saved password?";
    confirmBtn.textContent = "Update";
  } else {
    question.textContent = "Save this login to HavenKeys?";
    confirmBtn.textContent = "Save";
  }
  detail.textContent = view.username ?? "No username";
  confirmBtn.disabled = false;
}

function fail(message: string): void {
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
