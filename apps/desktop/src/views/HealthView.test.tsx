// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HealthCheck, HealthReport, ItemOverview } from "../lib/types";

const setHealthIgnored = vi.fn<(id: string, c: HealthCheck[]) => Promise<void>>();
const openHealthHelp = vi.fn<(id: string, c: HealthCheck) => Promise<void>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    setHealthIgnored: (id: string, c: HealthCheck[]) => setHealthIgnored(id, c),
    openHealthHelp: (id: string, c: HealthCheck) => openHealthHelp(id, c),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { en } from "../i18n/en";
import { HealthView } from "./HealthView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const item = (id: string, title: string): ItemOverview => ({
  id,
  itemType: "login",
  title,
  username: `${title.toLowerCase()}@example.com`,
  urls: [],
  hasPassword: true,
  hasTotp: false,
  hasNotes: false,
  hasPasskey: false,
  autoSignIn: false,
  createdAt: 0,
  updatedAt: 0,
});

const report: HealthReport = {
  computedAt: 1,
  counts: { weak: 1, reused: 2, old: 0, passkey: 1, twoFactor: 0, insecure: 0, duplicate: 0 },
  issues: [
    { itemId: "a", checks: ["weak", "reused"], reusedGroup: 0, duplicateGroup: null },
    { itemId: "b", checks: ["reused", "passkey"], reusedGroup: 0, duplicateGroup: null },
  ],
  dismissed: [{ itemId: "c", checks: ["old"] }],
};
const items = [item("a", "Alpha"), item("b", "Beta"), item("c", "Gamma")];

let host: HTMLElement;
let root: Root;
const onOpen = vi.fn();
const onEdit = vi.fn();
const onChanged = vi.fn();

function render(r: HealthReport | null, loading = false, readOnly = false) {
  act(() =>
    root.render(
      <HealthView items={items} report={r} loading={loading} readOnly={readOnly} onOpen={onOpen} onEdit={onEdit} onChanged={onChanged} />,
    ),
  );
}
// An exact label first: "Dismiss" is also the start of the "Dismissed" filter.
const button = (text: string) => {
  const all = Array.from(host.querySelectorAll("button"));
  return all.find((b) => b.textContent === text) ?? all.find((b) => b.textContent?.includes(text));
};

beforeEach(() => {
  setHealthIgnored.mockReset().mockResolvedValue(undefined);
  openHealthHelp.mockReset().mockResolvedValue(undefined);
  [onOpen, onEdit, onChanged].forEach((f) => f.mockReset());
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("HealthView", () => {
  it("shows a card per check with its count and lists titles from the overviews", () => {
    render(report);
    expect(host.textContent).toContain(en.health.cards.reused.title);
    expect(host.textContent).toContain(en.health.chip.reused(2));
    expect(host.textContent).toContain("Alpha");
    expect(host.textContent).not.toContain("Gamma"); // dismissed, hidden until the filter
  });

  it("filters by a card and shows dismissed checks under their filter", () => {
    render(report);
    act(() => button(en.health.cards.passkey.title)?.click());
    expect(host.textContent).toContain("Beta");
    expect(host.textContent).not.toContain("Alpha");
    act(() => button(en.health.dismissedFilter)?.click());
    expect(host.textContent).toContain("Gamma");
  });

  it("dismisses one check and keeps the others", async () => {
    render(report);
    await act(async () => button(en.health.dismiss)?.click());
    expect(setHealthIgnored).toHaveBeenCalledWith("a", expect.arrayContaining(["weak"]));
    expect(onChanged).toHaveBeenCalled();
  });

  it("sends the login's already-dismissed checks with the new one", async () => {
    render({ ...report, dismissed: [{ itemId: "a", checks: ["old"] }] });
    act(() => button(en.health.cards.weak.title)?.click());
    await act(async () => button(en.health.dismiss)?.click());
    expect(setHealthIgnored).toHaveBeenCalledWith("a", ["old", "weak"]);
  });

  it("undoes one dismissed check and keeps the rest dismissed", async () => {
    render({ ...report, dismissed: [{ itemId: "c", checks: ["old", "weak"] }] });
    act(() => button(en.health.dismissedFilter)?.click());
    await act(async () => button(en.health.undo)?.click());
    expect(setHealthIgnored).toHaveBeenCalledWith("c", ["weak"]);
    expect(onChanged).toHaveBeenCalled();
  });

  it("disables dismissing while offline", () => {
    render(report, false, true);
    expect(button(en.health.dismiss)?.disabled).toBe(true);
  });

  it("opens and edits the login from its row", () => {
    render(report);
    act(() => button(en.health.cards.weak.title)?.click());
    act(() => button(en.health.open)?.click());
    expect(onOpen).toHaveBeenCalledWith("a");
    act(() => button(en.health.changePassword)?.click());
    expect(onEdit).toHaveBeenCalledWith("a");
  });

  it("asks Rust to open the help link by item and check, never by URL", async () => {
    render(report);
    act(() => button(en.health.cards.passkey.title)?.click());
    await act(async () => button(en.health.howToEnable)?.click());
    expect(openHealthHelp).toHaveBeenCalledWith("b", "passkey");
  });

  it("says no issues found, never that the vault is secure", () => {
    render({ ...report, counts: { weak: 0, reused: 0, old: 0, passkey: 0, twoFactor: 0, insecure: 0, duplicate: 0 }, issues: [], dismissed: [] });
    expect(host.textContent).toContain(en.health.empty);
    expect(host.textContent?.toLowerCase()).not.toContain("secure.");
  });

  it("shows the loading state, not a partial report", () => {
    render(null, true);
    expect(host.textContent).toContain(en.health.loading);
  });
});
