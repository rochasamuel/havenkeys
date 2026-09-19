// Options page: turn in-page suggestions on or off by granting or removing
// the optional host permission. The background registers the content
// script when the permission changes (background/registration.ts).

import { INLINE_ORIGINS, grantedOrigins } from "../background/registration";

const status = document.getElementById("status") as HTMLElement;
const toggle = document.getElementById("toggle") as HTMLButtonElement;
let on = false;

async function refresh(): Promise<void> {
  const granted = await grantedOrigins();
  on = granted.length > 0;
  if (!on) {
    status.textContent = "Off. HavenKeys works from the toolbar button only.";
    status.className = "status";
  } else if (granted.length < INLINE_ORIGINS.length) {
    status.textContent = "On for secure (https) websites.";
    status.className = "status on";
  } else {
    status.textContent = "On. Reload open tabs to see suggestions there.";
    status.className = "status on";
  }
  toggle.textContent = on ? "Turn off" : "Turn on";
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

chrome.permissions.onAdded.addListener(() => void refresh());
chrome.permissions.onRemoved.addListener(() => void refresh());
void refresh();
