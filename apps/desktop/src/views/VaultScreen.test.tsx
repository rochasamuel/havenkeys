// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const healthReport = vi.fn();
const listItems = vi.fn();
let itemsChangedListener: (() => void) | null = null;

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    healthReport: (...a: unknown[]) => healthReport(...a),
    listItems: (...a: unknown[]) => listItems(...a),
    onItemsChanged: (cb: () => void) => {
      itemsChangedListener = cb;
      return Promise.resolve(() => undefined);
    },
  };
  return {
    ...original,
    api: new Proxy(known, {
      get: (target, name: string) =>
        name in target
          ? target[name]
          : name.startsWith("on")
            ? () => Promise.resolve(() => undefined)
            : () => Promise.resolve(null),
    }),
  };
});

import { VaultScreen } from "./VaultScreen";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const emptyReport = {
  computedAt: 1,
  counts: { weak: 0, reused: 0, old: 0, passkey: 0, twoFactor: 0, insecure: 0, duplicate: 0 },
  issues: [],
  dismissed: [],
};

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  vi.useFakeTimers();
  healthReport.mockReset().mockResolvedValue(emptyReport);
  listItems.mockReset().mockResolvedValue([]);
  itemsChangedListener = null;
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.useRealTimers();
});

async function settle() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
}

function type(value: string) {
  const input = host.querySelector<HTMLInputElement>(".search input")!;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  act(() => {
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

describe("VaultScreen health report", () => {
  it("is not recomputed on search keystrokes, only when the items change", async () => {
    act(() =>
      root.render(
        <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly={false} onLock={() => undefined} />,
      ),
    );
    await settle();
    const afterMount = healthReport.mock.calls.length;
    expect(afterMount).toBeGreaterThan(0);

    type("g");
    await settle();
    type("gi");
    await settle();
    expect(listItems.mock.calls.length).toBeGreaterThan(1);
    expect(healthReport.mock.calls.length).toBe(afterMount);

    await act(async () => itemsChangedListener?.());
    await settle();
    expect(healthReport.mock.calls.length).toBe(afterMount + 1);
  });
});

function overview(id: string, title: string, tags: string[]) {
  return {
    id,
    itemType: "login",
    title,
    username: null,
    urls: [],
    hasPassword: false,
    hasTotp: false,
    hasNotes: false,
    hasPasskey: false,
    autoSignIn: false,
    tags,
    createdAt: 1,
    updatedAt: 1,
  };
}

async function mountWith(items: ReturnType<typeof overview>[]) {
  listItems.mockResolvedValue(items);
  act(() =>
    root.render(
      <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly={false} onLock={() => undefined} />,
    ),
  );
  await settle();
}

const navButton = (label: string) =>
  [...host.querySelectorAll<HTMLButtonElement>(".nav button")].find((b) => b.textContent?.trim() === label);
const listHead = () => host.querySelector(".list-head h2")?.textContent;
const click = (el: Element | undefined | null) => act(() => el!.dispatchEvent(new MouseEvent("click", { bubbles: true })));

const three = () => [overview("a", "Alpha", ["staging"]), overview("b", "Bravo", ["staging", "work"]), overview("c", "Charlie", [])];

describe("VaultScreen tags", () => {
  it("lists the vault's tags with counts in the sidebar", async () => {
    await mountWith(three());
    const headings = [...host.querySelectorAll(".nav .nav-heading")].map((h) => h.textContent);
    expect(headings).toContain("Tags");
    expect(navButton("staging2")).toBeTruthy();
    expect(navButton("work1")).toBeTruthy();
  });

  it("filters the list by a tag and titles the list with it", async () => {
    await mountWith(three());
    click(navButton("staging2"));
    await settle();
    expect(listHead()).toBe("staging");
    const text = host.querySelector(".list")!.textContent!;
    expect(text).toContain("Alpha");
    expect(text).toContain("Bravo");
    expect(text).not.toContain("Charlie");
    expect(navButton("staging2")!.getAttribute("aria-current")).toBe("page");
    expect(navButton("All items3")!.getAttribute("aria-current")).toBeNull();
  });

  it("has no Tags section when no item has a tag", async () => {
    await mountWith([overview("c", "Charlie", [])]);
    expect([...host.querySelectorAll(".nav .nav-heading")].map((h) => h.textContent)).not.toContain("Tags");
  });

  it("falls back to All items when the selected tag disappears", async () => {
    await mountWith(three());
    click(navButton("work1"));
    await settle();
    expect(listHead()).toBe("work");
    listItems.mockResolvedValue([overview("a", "Alpha", ["staging"]), overview("b", "Bravo", ["staging"]), overview("c", "Charlie", [])]);
    await act(async () => itemsChangedListener?.());
    await settle();
    expect(listHead()).toBe("All items");
    const text = host.querySelector(".list")!.textContent!;
    expect(text).toContain("Charlie");
  });

  it("opens a tag from the detail pane", async () => {
    await mountWith(three());
    click([...host.querySelectorAll<HTMLElement>(".list button")].find((b) => b.textContent?.includes("Bravo")));
    await settle();
    const pill = host.querySelector<HTMLButtonElement>(".detail-tags button.tag-chip")!;
    expect(pill.textContent).toBe("staging");
    click(pill);
    await settle();
    expect(listHead()).toBe("staging");
    expect(navButton("staging2")!.getAttribute("aria-current")).toBe("page");
  });
});
