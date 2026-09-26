// Options page: turn in-page suggestions on or off by granting or removing
// the optional host permission. The background registers the content
// script when the permission changes (background/registration.ts).

import { INLINE_ORIGINS, grantedOrigins } from "../background/registration";
import { applyDocumentLang, t } from "../i18n";

const status = document.getElementById("status") as HTMLElement;
const toggle = document.getElementById("toggle") as HTMLButtonElement;
let on = false;

function text(id: string, value: string): void {
  (document.getElementById(id) as HTMLElement).textContent = value;
}

/** Static copy, filled from the message table (textContent only). */
function fillStatic(): void {
  applyDocumentLang();
  document.title = t.options.pageTitle;
  text("subtitle", t.options.subtitle);
  text("suggestions-title", t.options.suggestionsTitle);
  text("toggle-label", t.options.toggleLabel);
  text("without-title", t.options.withoutTitle);
  const notes = document.getElementById("notes") as HTMLElement;
  notes.replaceChildren(
    ...t.options.notes.map((note) => {
      const p = document.createElement("p");
      p.className = "note";
      p.textContent = note;
      return p;
    }),
  );
  const fill = document.createElement("em");
  fill.textContent = t.popup.fill;
  (document.getElementById("without") as HTMLElement).replaceChildren(t.options.withoutBefore, fill, t.options.withoutAfter);
}

async function refresh(): Promise<void> {
  const granted = await grantedOrigins();
  on = granted.length > 0;
  if (!on) {
    status.textContent = t.options.statusOff;
    status.className = "status";
  } else if (granted.length < INLINE_ORIGINS.length) {
    status.textContent = t.options.statusHttps;
    status.className = "status on";
  } else {
    status.textContent = t.options.statusOn;
    status.className = "status on";
  }
  toggle.textContent = on ? t.options.turnOff : t.options.turnOn;
  toggle.className = on ? "btn" : "btn primary";
  toggle.hidden = false;
}

toggle.addEventListener("click", () => {
  // permissions.request must run directly in the click handler.
  const change = on
    ? chrome.permissions.remove({ origins: [...INLINE_ORIGINS] })
    : chrome.permissions.request({ origins: [...INLINE_ORIGINS] });
  void change.catch(() => false).then(refresh);
});

fillStatic();
chrome.permissions.onAdded.addListener(() => void refresh());
chrome.permissions.onRemoved.addListener(() => void refresh());
void refresh();
