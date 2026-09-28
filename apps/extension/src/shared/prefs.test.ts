import { afterEach, describe, expect, it, vi } from "vitest";
import { getInlineSuggestions, onInlineSuggestionsChanged, setInlineSuggestions } from "./prefs";

type Changed = (changes: Record<string, { newValue?: unknown }>, area: string) => void;

function fakeStorage(initial: Record<string, unknown> = {}) {
  const data = { ...initial };
  let changed: Changed | null = null;
  vi.stubGlobal("chrome", {
    storage: {
      local: {
        get: async (k: string) => (k in data ? { [k]: data[k] } : {}),
        set: async (o: Record<string, unknown>) => Object.assign(data, o),
      },
      onChanged: { addListener: (l: Changed) => (changed = l) },
    },
  });
  return { data, fire: (c: Record<string, { newValue?: unknown }>, area = "local") => changed?.(c, area) };
}

afterEach(() => vi.unstubAllGlobals());

describe("inline suggestions preference", () => {
  it("is on when absent, off only when stored false", async () => {
    fakeStorage();
    expect(await getInlineSuggestions()).toBe(true);
    fakeStorage({ inlineSuggestions: false });
    expect(await getInlineSuggestions()).toBe(false);
    fakeStorage({ inlineSuggestions: "no" });
    expect(await getInlineSuggestions()).toBe(true);
  });

  it("is on when storage is missing or fails", async () => {
    vi.stubGlobal("chrome", {});
    expect(await getInlineSuggestions()).toBe(true);
    vi.stubGlobal("chrome", {
      storage: {
        local: {
          get: async () => {
            throw new Error("x");
          },
        },
      },
    });
    expect(await getInlineSuggestions()).toBe(true);
  });

  it("writes only the boolean", async () => {
    const { data } = fakeStorage();
    await setInlineSuggestions(false);
    expect(data).toEqual({ inlineSuggestions: false });
  });

  it("reports changes to the local area only", () => {
    const { fire } = fakeStorage();
    const seen: boolean[] = [];
    onInlineSuggestionsChanged((on) => seen.push(on));
    fire({ inlineSuggestions: { newValue: false } });
    fire({ inlineSuggestions: { newValue: true } });
    fire({ inlineSuggestions: {} }); // removed → on
    fire({ inlineSuggestions: { newValue: false } }, "sync");
    fire({ other: { newValue: false } });
    expect(seen).toEqual([false, true, true]);
  });
});
