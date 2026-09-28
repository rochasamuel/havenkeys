// Options page: the in-page suggestions preference (the field icon and the
// menu under login fields), and the site access that save prompts and
// passkeys need. Site access is granted at install; if the user withdrew it
// in the browser, Allow asks for it again. The background re-registers the
// content scripts when the grant changes (background/registration.ts).

import { INLINE_ORIGINS, grantedOrigins } from "../background/registration";
import { applyDocumentLang, t } from "../i18n";
import { getInlineSuggestions, onInlineSuggestionsChanged, setInlineSuggestions } from "../shared/prefs";

const status = document.getElementById("status") as HTMLElement;
const toggle = document.getElementById("toggle") as HTMLButtonElement;
const accessStatus = document.getElementById("access-status") as HTMLElement;
const allow = document.getElementById("allow") as HTMLButtonElement;
let on = true;

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
  text("access-title", t.options.accessTitle);
  text("access-label", t.options.accessLabel);
  text("access-note", t.options.accessNote);
  allow.textContent = t.options.allow;
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

function showSuggestions(value: boolean): void {
  on = value;
  status.textContent = on ? t.options.suggestionsOn : t.options.suggestionsOff;
  status.className = on ? "status on" : "status";
  toggle.textContent = on ? t.options.turnOff : t.options.turnOn;
  toggle.className = on ? "btn" : "btn primary";
  toggle.hidden = false;
}

async function refreshAccess(): Promise<void> {
  const granted = await grantedOrigins();
  if (granted.length === 0) {
    accessStatus.textContent = t.options.accessOff;
    accessStatus.className = "status";
  } else {
    accessStatus.textContent = granted.length < INLINE_ORIGINS.length ? t.options.accessHttps : t.options.accessOn;
    accessStatus.className = "status on";
  }
  allow.hidden = granted.length > 0;
}

toggle.addEventListener("click", () => {
  const next = !on;
  void setInlineSuggestions(next).then(() => showSuggestions(next));
});

allow.addEventListener("click", () => {
  // permissions.request must run directly in the click handler.
  void chrome.permissions
    .request({ origins: [...INLINE_ORIGINS] })
    .catch(() => false)
    .then(refreshAccess);
});

fillStatic();
onInlineSuggestionsChanged(showSuggestions);
chrome.permissions.onAdded.addListener(() => void refreshAccess());
chrome.permissions.onRemoved.addListener(() => void refreshAccess());
void getInlineSuggestions().then(showSuggestions);
void refreshAccess();
