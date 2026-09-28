// @vitest-environment jsdom
// Runs under Node in vitest; excluded from the browser tsconfig because it uses Node APIs.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";

let granted: string[] = [];
let stored: Record<string, unknown> = {};
const permCalls: string[] = [];

function install(): void {
  const html = readFileSync(resolve(__dirname, "options.html"), "utf8");
  document.body.innerHTML = html.slice(html.indexOf("<main>"), html.indexOf("</main>") + 7);
  vi.stubGlobal("chrome", {
    permissions: {
      contains: async ({ origins }: { origins: string[] }) => origins.every((o) => granted.includes(o)),
      request: async () => {
        permCalls.push("request");
        granted = ["https://*/*", "http://*/*"];
        return true;
      },
      remove: async () => {
        permCalls.push("remove");
        return true;
      },
      onAdded: { addListener: () => undefined },
      onRemoved: { addListener: () => undefined },
    },
    storage: {
      local: {
        get: async (k: string) => (k in stored ? { [k]: stored[k] } : {}),
        set: async (o: Record<string, unknown>) => Object.assign(stored, o),
      },
      onChanged: { addListener: () => undefined },
    },
  });
}

const flush = () => new Promise((r) => setTimeout(r, 0));
const $ = (id: string) => document.getElementById(id) as HTMLButtonElement;

beforeEach(() => {
  vi.resetModules();
  granted = ["https://*/*", "http://*/*"];
  stored = {};
  permCalls.length = 0;
});

describe("options page", () => {
  it("the suggestions switch writes the preference and never touches permissions", async () => {
    install();
    await import("./options");
    await flush();
    expect($("toggle").textContent).toBe("Turn off");
    $("toggle").click();
    await flush();
    expect(stored).toEqual({ inlineSuggestions: false });
    expect($("toggle").textContent).toBe("Turn on");
    expect(permCalls).toEqual([]);
    expect($("allow").hidden).toBe(true);
    expect($("access-status").textContent).toMatch(/^On\./);
  });

  it("offers Allow only when site access was withdrawn", async () => {
    granted = [];
    install();
    await import("./options");
    await flush();
    expect($("allow").hidden).toBe(false);
    expect($("access-status").textContent).toMatch(/^Off\./);
    $("allow").click();
    await flush();
    expect(permCalls).toEqual(["request"]);
    expect($("allow").hidden).toBe(true);
  });
});
