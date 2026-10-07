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
