// The extension's one stored preference: whether the field icon and the
// menu under login fields are shown. A non-sensitive boolean in
// chrome.storage.local; save prompts, passkeys and popup fills never read
// it. Absent, unreadable or not a boolean means on.

export const INLINE_SUGGESTIONS_KEY = "inlineSuggestions";

const isOn = (v: unknown): boolean => v !== false;

export async function getInlineSuggestions(): Promise<boolean> {
  try {
    const got = await globalThis.chrome?.storage?.local?.get(INLINE_SUGGESTIONS_KEY);
    return isOn(got?.[INLINE_SUGGESTIONS_KEY]);
  } catch {
    return true;
  }
}

export async function setInlineSuggestions(on: boolean): Promise<void> {
  await globalThis.chrome?.storage?.local?.set({ [INLINE_SUGGESTIONS_KEY]: on });
}

export function onInlineSuggestionsChanged(cb: (on: boolean) => void): void {
  globalThis.chrome?.storage?.onChanged?.addListener((changes, area) => {
    if (area === "local" && INLINE_SUGGESTIONS_KEY in changes) cb(isOn(changes[INLINE_SUGGESTIONS_KEY]?.newValue));
  });
}
