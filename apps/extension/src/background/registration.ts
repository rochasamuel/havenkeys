// Inline suggestions are opt-in. The content script is registered only for
// the host patterns the user granted through the optional host permission
// (options page, or the browser's own site-access controls), and removed
// when they revoke it. Without a grant the extension works from the toolbar
// popup alone, with activeTab.
//
// Passkeys (docs/superpowers/specs/2026-09-23-passkeys-design.md §5.1) share
// this same grant: the page-world wrapper and the isolated bridge are
// registered and removed alongside the inline content script, for exactly
// the hosts the user granted.

export const INLINE_ORIGINS = ["https://*/*", "http://*/*"] as const;
const SCRIPT_ID = "havenkeys-inline";
// Passkeys (docs/superpowers/specs/2026-09-23-passkeys-design.md §5.1): the
// page-world wrapper must run before the site's own scripts, so both halves
// load at document_start, for exactly the hosts the user granted.
const PAGE_SCRIPT_ID = "havenkeys-webauthn-page";
const BRIDGE_SCRIPT_ID = "havenkeys-webauthn-bridge";

/** Which of INLINE_ORIGINS the user has granted. */
export async function grantedOrigins(): Promise<string[]> {
  const out: string[] = [];
  for (const origin of INLINE_ORIGINS) {
    if (await chrome.permissions.contains({ origins: [origin] })) out.push(origin);
  }
  return out;
}

function inlineScript(matches: string[]): chrome.scripting.RegisteredContentScript {
  return { id: SCRIPT_ID, matches, js: ["content.js"], allFrames: true, runAt: "document_idle", persistAcrossSessions: true };
}

function passkeyScripts(matches: string[]): chrome.scripting.RegisteredContentScript[] {
  return [
    {
      id: PAGE_SCRIPT_ID,
      matches,
      js: ["webauthn-page.js"],
      allFrames: true,
      runAt: "document_start",
      world: "MAIN",
      persistAcrossSessions: true,
    },
    { id: BRIDGE_SCRIPT_ID, matches, js: ["webauthn-bridge.js"], allFrames: true, runAt: "document_start", persistAcrossSessions: true },
  ];
}

interface Group {
  ids: string[];
  build: (matches: string[]) => chrome.scripting.RegisteredContentScript[];
  /** Swallow a registration failure (some browsers reject `world: "MAIN"`). */
  optional: boolean;
}

const INLINE_GROUP: Group = { ids: [SCRIPT_ID], build: (matches) => [inlineScript(matches)], optional: false };
const PASSKEY_GROUP: Group = { ids: [PAGE_SCRIPT_ID, BRIDGE_SCRIPT_ID], build: passkeyScripts, optional: true };

function matchesGrant(existing: chrome.scripting.RegisteredContentScript[], group: Group, matches: string[]): boolean {
  return (
    existing.length === (matches.length > 0 ? group.ids.length : 0) &&
    existing.every((s) => (s.matches ?? []).length === matches.length && (s.matches ?? []).every((m) => matches.includes(m)))
  );
}

/**
 * Sync one group's registration to the grant, independently of every other
 * group. A group that already matches is left untouched — in particular, a
 * browser that permanently rejects the passkey group (`optional: true`)
 * must not cause the inline group to be unregistered and re-registered on
 * every call: index.ts calls this at service-worker startup, so that would
 * churn on every MV3 wake.
 */
async function syncGroup(group: Group, matches: string[]): Promise<void> {
  const existing = await chrome.scripting.getRegisteredContentScripts({ ids: group.ids });
  if (matchesGrant(existing, group, matches)) return;
  if (existing.length > 0) await chrome.scripting.unregisterContentScripts({ ids: existing.map((s) => s.id) });
  if (matches.length === 0) return;
  if (!group.optional) {
    await chrome.scripting.registerContentScripts(group.build(matches));
    return;
  }
  try {
    await chrome.scripting.registerContentScripts(group.build(matches));
  } catch {
    // Passkeys are unavailable on this browser; inline autofill still works.
  }
}

/** Make the registered content scripts match the current grant. Idempotent per group. */
export async function syncContentScripts(): Promise<void> {
  const matches = await grantedOrigins();
  await syncGroup(INLINE_GROUP, matches);
  await syncGroup(PASSKEY_GROUP, matches);
}
