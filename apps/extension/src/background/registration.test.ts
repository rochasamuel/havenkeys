import { beforeEach, describe, expect, it } from "vitest";

let granted: string[] = [];
let registered: Array<{ id: string; matches?: string[]; world?: string; runAt?: string }> = [];

/** Swappable per test; defaults to the plain "record what was registered" behaviour below. */
let registerImpl: (s: typeof registered) => Promise<void> = async (s) => {
  registered.push(...s);
};

(globalThis as { chrome?: unknown }).chrome = {
  permissions: { contains: async ({ origins }: { origins: string[] }) => origins.every((o) => granted.includes(o)) },
  scripting: {
    getRegisteredContentScripts: async ({ ids }: { ids: string[] }) => registered.filter((s) => ids.includes(s.id)),
    unregisterContentScripts: async ({ ids }: { ids: string[] }) => {
      registered = registered.filter((s) => !ids.includes(s.id));
    },
    registerContentScripts: (s: typeof registered) => registerImpl(s),
  },
};

const { syncContentScripts } = await import("./registration");

beforeEach(() => {
  granted = [];
  registered = [];
  registerImpl = async (s) => {
    registered.push(...s);
  };
});

describe("content script registration", () => {
  it("registers the passkey scripts with the inline grant, and removes them with it", async () => {
    granted = ["https://*/*"];
    await syncContentScripts();
    const byId = Object.fromEntries(registered.map((s) => [s.id, s]));
    expect(Object.keys(byId).sort()).toEqual(["havenkeys-inline", "havenkeys-webauthn-bridge", "havenkeys-webauthn-page"]);
    expect(byId["havenkeys-webauthn-page"]?.world).toBe("MAIN");
    expect(byId["havenkeys-webauthn-page"]?.runAt).toBe("document_start");
    expect(byId["havenkeys-webauthn-bridge"]?.runAt).toBe("document_start");
    granted = [];
    await syncContentScripts();
    expect(registered).toEqual([]);
  });

  it("adds the passkey scripts to an older registration", async () => {
    granted = ["https://*/*"];
    registered = [{ id: "havenkeys-inline", matches: ["https://*/*"] }];
    await syncContentScripts();
    expect(registered.map((s) => s.id).sort()).toEqual(["havenkeys-inline", "havenkeys-webauthn-bridge", "havenkeys-webauthn-page"]);
  });

  it("keeps the inline script registered even when the passkey scripts fail to register", async () => {
    granted = ["https://*/*"];
    registerImpl = async (s) => {
      if (s.some((entry) => entry.world === "MAIN")) throw new Error("world not supported");
      registered.push(...s);
    };
    await syncContentScripts();
    expect(registered.map((s) => s.id)).toEqual(["havenkeys-inline"]);
  });
});
