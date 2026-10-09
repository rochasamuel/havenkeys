// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const healthReport = vi.fn();
const listItems = vi.fn();
const accountStatus = vi.fn();
const trashItem = vi.fn();
const restoreItem = vi.fn();
const listTrash = vi.fn();
let itemsChangedListener: (() => void) | null = null;

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    healthReport: (...a: unknown[]) => healthReport(...a),
    listItems: (...a: unknown[]) => listItems(...a),
    accountStatus: () => accountStatus(),
    trashItem: (...a: unknown[]) => trashItem(...a),
    restoreItem: (...a: unknown[]) => restoreItem(...a),
    listTrash: () => listTrash(),
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
import { ToastProvider } from "../components/Toast";

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
  accountStatus.mockReset().mockResolvedValue(null);
  trashItem.mockReset().mockResolvedValue(true);
  restoreItem.mockReset();
  listTrash.mockReset().mockResolvedValue([]);
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
        <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly={false} frozen={false} onLock={() => undefined} />,
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
      <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly={false} frozen={false} onLock={() => undefined} />,
    ),
  );
  await settle();
}

/** The nav row whose name span reads `label` (the count is a separate span). */
const navButton = (label: string) =>
  [...host.querySelectorAll<HTMLButtonElement>(".nav button")].find(
    (b) => b.querySelector("span:not(.nav-count)")?.textContent === label,
  );
const listHead = () => host.querySelector(".list-head h2")?.textContent;
const click = (el: Element | undefined | null) => act(() => el!.dispatchEvent(new MouseEvent("click", { bubbles: true })));

const three = () => [overview("a", "Alpha", ["staging"]), overview("b", "Bravo", ["staging", "work"]), overview("c", "Charlie", [])];

describe("VaultScreen tags", () => {
  it("lists the vault's tags with counts in the sidebar", async () => {
    await mountWith(three());
    const headings = [...host.querySelectorAll(".nav .nav-heading")].map((h) => h.textContent);
    expect(headings).toContain("Tags");
    expect(navButton("staging")).toBeTruthy();
    expect(navButton("work")).toBeTruthy();
  });

  it("filters the list by a tag and titles the list with it", async () => {
    await mountWith(three());
    click(navButton("staging"));
    await settle();
    expect(listHead()).toBe("staging");
    const text = host.querySelector(".list")!.textContent!;
    expect(text).toContain("Alpha");
    expect(text).toContain("Bravo");
    expect(text).not.toContain("Charlie");
    expect(navButton("staging")!.getAttribute("aria-current")).toBe("page");
    expect(navButton("All items")!.getAttribute("aria-current")).toBeNull();
  });

  it("filters by a tag whatever its case on each item", async () => {
    // Rust stores one spelling per tag; an item from before that rule may differ in case.
    await mountWith([overview("a", "Alpha", ["Work"]), overview("b", "Bravo", ["work"]), overview("c", "Charlie", [])]);
    expect(navButton("work")).toBeFalsy();
    click(navButton("Work"));
    await settle();
    const text = host.querySelector(".list")!.textContent!;
    expect(text).toContain("Alpha");
    expect(text).toContain("Bravo");
    expect(text).not.toContain("Charlie");
    expect(navButton("Work")!.getAttribute("aria-current")).toBe("page");
  });

  it("has no Tags section when no item has a tag", async () => {
    await mountWith([overview("c", "Charlie", [])]);
    expect([...host.querySelectorAll(".nav .nav-heading")].map((h) => h.textContent)).not.toContain("Tags");
  });

  it("falls back to All items when the selected tag disappears", async () => {
    await mountWith(three());
    click(navButton("work"));
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
    // A tag glyph tells a tag from the vault-health chips just above it.
    expect(pill.querySelector("svg")).not.toBeNull();
    click(pill);
    await settle();
    expect(listHead()).toBe("staging");
    expect(navButton("staging")!.getAttribute("aria-current")).toBe("page");
  });

  it("pins the Account row in All items but not in a tag's list", async () => {
    accountStatus.mockResolvedValue({
      email: "me@example.com",
      serverUrl: "https://vault.example.com",
      accountId: "acc",
      online: true,
      lastSyncedAt: null,
    });
    await mountWith(three());
    expect(host.querySelector(".list")!.textContent).toContain("HavenKeys Account");
    click(navButton("staging"));
    await settle();
    expect(host.querySelector(".list")!.textContent).not.toContain("HavenKeys Account");
  });
});

describe("VaultScreen Trash", () => {
  const github = overview("11111111-1111-4111-8111-111111111111", "GitHub", []);

  async function mountSelected() {
    listItems.mockResolvedValue([github]);
    act(() =>
      root.render(
        <ToastProvider>
          <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly={false} frozen={false} onLock={() => undefined} />
        </ToastProvider>,
      ),
    );
    await settle();
    click([...host.querySelectorAll<HTMLElement>(".list button")].find((b) => b.textContent?.includes("GitHub")));
    await settle();
  }
  const button = (label: string) => [...host.querySelectorAll<HTMLButtonElement>("button")].find((b) => b.textContent?.trim() === label);
  const toastText = () => host.ownerDocument.querySelector(".toast")?.textContent ?? "";

  it("Delete moves the item to the Trash with an Undo that restores it", async () => {
    restoreItem.mockResolvedValue(github);
    await mountSelected();
    click(host.querySelector(".item-foot .btn-quiet-danger"));
    await settle();
    // No confirm first: the Trash is the safety net.
    expect(trashItem).toHaveBeenCalledWith(github.id);
    expect(toastText()).toContain("Moved “GitHub” to Trash.");
    click(button("Undo"));
    await settle();
    expect(restoreItem).toHaveBeenCalledWith(github.id);
    expect(host.querySelector(".item-title")?.textContent).toBe("GitHub");
  });

  it("says Deleted when the item could not be trashed", async () => {
    trashItem.mockResolvedValue(false);
    await mountSelected();
    click(host.querySelector(".item-foot .btn-quiet-danger"));
    await settle();
    expect(trashItem).toHaveBeenCalledWith(github.id);
    expect(toastText()).toContain("Deleted “GitHub”.");
    expect(button("Undo")).toBeUndefined();
  });

  it("shows Trash in the sidebar with a count only when not empty", async () => {
    await mountSelected();
    expect(navButton("Trash")).toBeTruthy();
    expect(navButton("Trash")!.querySelector(".nav-count")).toBeNull();

    listTrash.mockResolvedValue([{ ...github, trashedAt: Date.now(), daysLeft: 30 }]);
    listItems.mockResolvedValue([]);
    click(host.querySelector(".item-foot .btn-quiet-danger"));
    await settle();
    expect(navButton("Trash")!.querySelector(".nav-count")?.textContent).toBe("1");
  });
});

describe("VaultScreen plan", () => {
  const account = (extra: Record<string, unknown>) => ({
    email: "me@example.com",
    serverUrl: "https://api.havenkeys.net",
    accountId: "3f1c0000-0000-0000-0000-000000000000",
    online: true,
    lastSyncedAt: null,
    planStatus: null,
    entitlement: "full",
    trialEndsAt: null,
    periodEnd: null,
    ...extra,
  });
  const foot = () => host.querySelector(".sidebar-foot .conn")?.textContent;

  it("counts the trial days left in the sidebar", async () => {
    accountStatus.mockResolvedValue(
      account({ planStatus: "trialing", trialEndsAt: new Date(Date.now() + 4.5 * 86_400_000).toISOString() }),
    );
    await mountWith([]);
    expect(foot()).toBe("Trial: 5 days left");
  });

  it("says read-only and disables New while frozen", async () => {
    accountStatus.mockResolvedValue(account({ planStatus: "past_due", entitlement: "frozen" }));
    listItems.mockResolvedValue([]);
    act(() =>
      root.render(
        <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly frozen onLock={() => undefined} />,
      ),
    );
    await settle();
    expect(foot()).toBe("Read-only (trial ended)");
    const newButton = host.querySelector<HTMLButtonElement>(".list-head button");
    expect(newButton?.disabled).toBe(true);
  });

  it("a trial that ended reads frozen, not zero days left, and is not styled offline", async () => {
    accountStatus.mockResolvedValue(
      account({ planStatus: "trialing", entitlement: "frozen", trialEndsAt: new Date(Date.now() - 86_400_000).toISOString() }),
    );
    listItems.mockResolvedValue([]);
    act(() =>
      root.render(
        <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly frozen offline={false} onLock={() => undefined} />,
      ),
    );
    await settle();
    expect(foot()).toBe("Read-only (trial ended)");
    const conn = host.querySelector(".sidebar-foot .conn")!;
    expect(conn.classList.contains("is-offline")).toBe(false);
    expect(conn.classList.contains("is-frozen")).toBe(true);
  });

  it("offline while frozen keeps the frozen line and the offline styling", async () => {
    accountStatus.mockResolvedValue(account({ planStatus: "past_due", entitlement: "frozen" }));
    listItems.mockResolvedValue([]);
    act(() =>
      root.render(
        <VaultScreen damagedItems={0} damagedSettings={false} unreadableItems={0} readOnly frozen offline onLock={() => undefined} />,
      ),
    );
    await settle();
    expect(foot()).toBe("Read-only (trial ended)");
    expect(host.querySelector(".sidebar-foot .conn")!.classList.contains("is-offline")).toBe(true);
  });

  it("an active trial counts down and is not styled offline", async () => {
    accountStatus.mockResolvedValue(
      account({ planStatus: "trialing", trialEndsAt: new Date(Date.now() + 1.2 * 86_400_000).toISOString() }),
    );
    await mountWith([]);
    expect(foot()).toBe("Trial: 2 days left");
    expect(host.querySelector(".sidebar-foot .conn")!.className).toBe("conn");
  });
});
