// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TrashEntry } from "../lib/types";

const listTrash = vi.fn<() => Promise<TrashEntry[]>>();
const restoreItem = vi.fn();
const purgeItem = vi.fn();
const emptyTrash = vi.fn();
const syncNow = vi.fn();
// Only the Trash calls and a sync exist: any secret fetch would throw.
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    listTrash: () => listTrash(),
    restoreItem: (id: string) => restoreItem(id),
    purgeItem: (id: string) => purgeItem(id),
    emptyTrash: () => emptyTrash(),
    trashItem: () => Promise.reject(new Error("not used here")),
    syncNow: () => syncNow(),
  },
}));

import { ApiError } from "../lib/api";
import { ToastProvider } from "../components/Toast";
import { TrashView } from "./TrashView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const entry: TrashEntry = {
  id: "11111111-1111-4111-8111-111111111111",
  itemType: "login",
  title: "GitHub",
  username: "me@example.com",
  urls: [{ url: "https://github.com", matchType: "domain" }],
  hasPassword: true,
  hasTotp: false,
  hasNotes: false,
  hasPasskey: true,
  autoSignIn: true,
  tags: [],
  createdAt: 1,
  updatedAt: 1,
  trashedAt: Date.now() - 2 * 86_400_000,
  daysLeft: 28,
};

let host: HTMLElement;
let root: Root;
const onChanged = vi.fn();

async function render(readOnly = false) {
  await act(async () =>
    root.render(
      <ToastProvider>
        <TrashView readOnly={readOnly} revision={0} onChanged={onChanged} />
      </ToastProvider>,
    ),
  );
}
const button = (label: string) => [...host.querySelectorAll<HTMLButtonElement>("button")].find((b) => b.textContent?.trim() === label);
const click = async (el: Element | undefined | null) => {
  expect(el).toBeTruthy();
  await act(async () => void el!.dispatchEvent(new MouseEvent("click", { bubbles: true })));
};
const select = () => click([...host.querySelectorAll(".list-item")].find((b) => b.textContent?.includes("GitHub")));

beforeEach(() => {
  listTrash.mockReset().mockResolvedValue([entry]);
  restoreItem.mockReset().mockResolvedValue({ ...entry, trashedAt: undefined });
  purgeItem.mockReset().mockResolvedValue(undefined);
  emptyTrash.mockReset().mockResolvedValue(1);
  syncNow.mockReset().mockResolvedValue({});
  onChanged.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("TrashView", () => {
  it("lists items with their days left", async () => {
    await render();
    const text = host.querySelector(".list")!.textContent!;
    expect(text).toContain("GitHub");
    expect(text).toContain("Deleted 2 days ago");
    expect(text).toContain("Removed in 28 days");
  });

  it("says so when the Trash is empty", async () => {
    listTrash.mockResolvedValue([]);
    await render();
    expect(host.querySelector(".list")!.textContent).toContain("Trash is empty.");
    expect(button("Empty Trash")!.disabled).toBe(true);
  });

  it("restores the selected item", async () => {
    await render();
    await select();
    await click(button("Restore"));
    expect(restoreItem).toHaveBeenCalledWith(entry.id);
    expect(onChanged).toHaveBeenCalled();
    expect(document.querySelector(".toast")?.textContent).toContain("Restored “GitHub”.");
  });

  it("confirms before deleting for good, with the passkey warning", async () => {
    await render();
    await select();
    await click(button("Delete permanently"));
    const confirm = host.querySelector(".confirm")!;
    expect(confirm.textContent).toContain("Delete “GitHub” permanently? This cannot be undone.");
    expect(confirm.textContent).toContain("Its passkeys go too.");
    expect(purgeItem).not.toHaveBeenCalled();
    await click([...confirm.querySelectorAll("button")].find((b) => b.textContent === "Delete"));
    expect(purgeItem).toHaveBeenCalledWith(entry.id);
    expect(onChanged).toHaveBeenCalled();
  });

  it("confirms before emptying", async () => {
    await render();
    await click(button("Empty Trash"));
    const confirm = host.querySelector(".confirm")!;
    expect(confirm.textContent).toContain("Delete 1 item permanently? This cannot be undone.");
    expect(confirm.textContent).toContain("Its passkeys go too.");
    expect(emptyTrash).not.toHaveBeenCalled();
    await click([...confirm.querySelectorAll("button")].find((b) => b.textContent === "Delete"));
    expect(emptyTrash).toHaveBeenCalled();
    expect(onChanged).toHaveBeenCalled();
  });

  it("syncs when another device already deleted the item, keeping the message", async () => {
    restoreItem.mockRejectedValue(new ApiError("item_changed_elsewhere", "Item changed elsewhere."));
    await render();
    await select();
    listTrash.mockResolvedValue([]);
    await click(button("Restore"));
    expect(syncNow).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".toast")?.textContent).toContain("This item changed on another device.");
    expect(onChanged).toHaveBeenCalled();
    expect(host.querySelector(".list")!.textContent).toContain("Trash is empty.");
  });

  it("does not sync for any other failure", async () => {
    purgeItem.mockRejectedValue(new ApiError("offline", "Offline."));
    await render();
    await select();
    await click(button("Delete permanently"));
    await click([...host.querySelectorAll(".confirm button")].find((b) => b.textContent === "Delete"));
    expect(syncNow).not.toHaveBeenCalled();
  });

  it("reloads the counts after an Empty Trash that failed part way", async () => {
    emptyTrash.mockRejectedValue(new ApiError("item_changed_elsewhere", "Item changed elsewhere."));
    await render();
    await click(button("Empty Trash"));
    await click([...host.querySelectorAll(".confirm button")].find((b) => b.textContent === "Delete"));
    expect(syncNow).toHaveBeenCalledTimes(1);
    expect(onChanged).toHaveBeenCalled();
  });

  it("disables every action offline", async () => {
    await render(true);
    await select();
    expect(button("Restore")!.disabled).toBe(true);
    expect(button("Delete permanently")!.disabled).toBe(true);
    expect(button("Empty Trash")!.disabled).toBe(true);
  });

  it("shows no secret: no reveal or copy controls", async () => {
    await render();
    await select();
    const detail = host.querySelector(".detail")!;
    expect(detail.textContent).toContain("me@example.com");
    expect(detail.textContent).toContain("Restore this item to see or use its passwords and codes.");
    const labels = [...detail.querySelectorAll("button")].map((b) => `${b.textContent} ${b.getAttribute("aria-label") ?? ""}`);
    expect(labels.some((l) => /show|copy|reveal/i.test(l))).toBe(false);
    expect(detail.querySelector("a")).toBeNull();
  });
});
