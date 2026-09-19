// Inline suggestions are opt-in. The content script is registered only for
// the host patterns the user granted through the optional host permission
// (options page, or the browser's own site-access controls), and removed
// when they revoke it. Without a grant the extension works from the toolbar
// popup alone, with activeTab.

export const INLINE_ORIGINS = ["https://*/*", "http://*/*"] as const;
const SCRIPT_ID = "havenkeys-inline";

/** Which of INLINE_ORIGINS the user has granted. */
export async function grantedOrigins(): Promise<string[]> {
  const out: string[] = [];
  for (const origin of INLINE_ORIGINS) {
    if (await chrome.permissions.contains({ origins: [origin] })) out.push(origin);
  }
  return out;
}

/** Make the registered content script match the current grant. Idempotent. */
export async function syncContentScripts(): Promise<void> {
  const matches = await grantedOrigins();
  const existing = await chrome.scripting.getRegisteredContentScripts({ ids: [SCRIPT_ID] });
  const current = existing[0]?.matches ?? [];
  const same = current.length === matches.length && current.every((m) => matches.includes(m));
  if (same && (existing.length > 0) === (matches.length > 0)) return;
  if (existing.length > 0) await chrome.scripting.unregisterContentScripts({ ids: [SCRIPT_ID] });
  if (matches.length === 0) return;
  await chrome.scripting.registerContentScripts([
    {
      id: SCRIPT_ID,
      matches,
      js: ["content.js"],
      allFrames: true,
      runAt: "document_idle",
      persistAcrossSessions: true,
    },
  ]);
}
