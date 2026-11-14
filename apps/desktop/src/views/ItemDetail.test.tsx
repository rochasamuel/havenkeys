// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const reveal = vi.fn();
const itemFields = vi.fn();
const listPasskeys = vi.fn();
const openChangePassword = vi.fn();

// Every other command answers null; event subscriptions return an unlisten.
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  const known: Record<string, unknown> = {
    reveal: (...a: unknown[]) => reveal(...a),
    itemFields: (...a: unknown[]) => itemFields(...a),
    listPasskeys: (...a: unknown[]) => listPasskeys(...a),
    openChangePassword: (...a: unknown[]) => openChangePassword(...a),
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

import { ToastProvider } from "../components/Toast";
import type { ItemOverview, SectionView } from "../lib/types";
import { ItemDetail } from "./ItemDetail";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const note: ItemOverview = {
  id: "n1", itemType: "secure_note", title: "Server", username: null, urls: [], hasPassword: false, hasTotp: false,
  hasNotes: false, hasPasskey: false, autoSignIn: false, tags: [], createdAt: 1, updatedAt: 1,
};
const hostField: SectionView[] = [
  { id: "s1", title: null, fields: [{ id: "f1", label: "Host", type: "text", value: "db.example.com" }] },
];

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  reveal.mockReset();
  itemFields.mockReset().mockResolvedValue(hostField);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

async function renderNote() {
  await act(async () =>
    root.render(
      <ToastProvider>
        <ItemDetail
          item={note}
          revision={0}
          readOnly={false}
          onEdit={() => undefined}
          onDelete={() => undefined}
          onOpen={() => undefined}
        />
      </ToastProvider>,
    ),
  );
}

describe("ItemDetail secure note fields", () => {
  it("hides the empty note sheet when the note has only fields", async () => {
    reveal.mockResolvedValue("");
    await renderNote();
    expect(reveal).toHaveBeenCalledWith("n1", "content");
    expect(host.querySelector(".note-sheet")).toBeNull();
    expect(host.textContent).toContain("Host");
  });

  it("shows the note text above its fields", async () => {
    reveal.mockResolvedValue("root box");
    await renderNote();
    const sheet = host.querySelector(".note-sheet")!;
    expect(sheet.textContent).toContain("root box");
    const fieldLabel = [...host.querySelectorAll("*")].find((e) => e.textContent === "Host")!;
    expect(fieldLabel).toBeDefined();
    expect(sheet.compareDocumentPosition(fieldLabel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});

describe("ItemDetail passkeys", () => {
  it("marks a replaced passkey as the older one", async () => {
    const login: ItemOverview = { ...note, id: "l1", itemType: "login", title: "GitHub", username: "octo", hasPasskey: true };
    listPasskeys.mockResolvedValue([
      { credentialId: "new", rpId: "github.com", userName: "octo", displayName: null, createdAt: 2, replaced: false },
      { credentialId: "old", rpId: "github.com", userName: "octo", displayName: null, createdAt: 1, replaced: true },
    ]);
    await act(async () =>
      root.render(
        <ToastProvider>
          <ItemDetail item={login} revision={0} readOnly={false} onEdit={() => undefined} onDelete={() => undefined} onOpen={() => undefined} />
        </ToastProvider>,
      ),
    );
    const rows = [...host.querySelectorAll(".history-row")].map((r) => r.textContent ?? "");
    expect(rows).toHaveLength(2);
    expect(rows[0]).not.toContain("Older passkey");
    expect(rows[1]).toContain("Older passkey");
  });

  it("does not warn about losing access when deleting the older passkey", async () => {
    const login: ItemOverview = { ...note, id: "l1", itemType: "login", title: "GitHub", username: "octo", hasPasskey: true };
    listPasskeys.mockResolvedValue([
      { credentialId: "new", rpId: "github.com", userName: "octo", displayName: null, createdAt: 2, replaced: false },
      { credentialId: "old", rpId: "github.com", userName: "octo", displayName: null, createdAt: 1, replaced: true },
    ]);
    await act(async () =>
      root.render(
        <ToastProvider>
          <ItemDetail item={login} revision={0} readOnly={false} onEdit={() => undefined} onDelete={() => undefined} onOpen={() => undefined} />
        </ToastProvider>,
      ),
    );
    const old = host.querySelectorAll(".history-row")[1]!;
    await act(async () => (old.querySelector("button") as HTMLButtonElement).click());
    expect(old.textContent).toContain("The newer passkey stays.");
    expect(old.textContent).not.toContain("lose access");
  });
});

describe("ItemDetail change on website", () => {
  const site: ItemOverview = {
    ...note,
    id: "l2",
    itemType: "login",
    title: "Shop",
    username: "me",
    hasPassword: true,
    urls: [{ url: "https://shop.example.com/login", matchType: "domain" }],
  };
  const renderLogin = (item: ItemOverview, readOnly = false) =>
    act(async () =>
      root.render(
        <ToastProvider>
          <ItemDetail item={item} revision={0} readOnly={readOnly} onEdit={() => undefined} onDelete={() => undefined} onOpen={() => undefined} />
        </ToastProvider>,
      ),
    );
  const changeButton = () =>
    [...host.querySelectorAll("button")].find((b) => b.textContent?.includes("Change on website")) as HTMLButtonElement | undefined;

  beforeEach(() => openChangePassword.mockReset().mockResolvedValue("curated"));

  it("asks Rust to open the page by item id only, even while read-only", async () => {
    await renderLogin(site, true);
    const b = changeButton()!;
    expect(b.disabled).toBe(false);
    await act(async () => b.click());
    expect(openChangePassword).toHaveBeenCalledWith("l2");
    expect(openChangePassword.mock.calls[0]).toHaveLength(1);
    expect(host.textContent).not.toContain("doesn't point to a change-password page");
  });

  it("says when only the website could be opened", async () => {
    openChangePassword.mockResolvedValue("website");
    await renderLogin(site);
    await act(async () => changeButton()!.click());
    expect(host.textContent).toContain("doesn't point to a change-password page");
  });

  it("is disabled while Rust looks for the page", async () => {
    let finish: (v: string) => void = () => undefined;
    openChangePassword.mockReturnValue(new Promise((r) => (finish = r)));
    await renderLogin(site);
    await act(async () => changeButton()!.click());
    expect(changeButton()!.disabled).toBe(true);
    await act(async () => finish("well_known"));
    expect(changeButton()!.disabled).toBe(false);
  });

  it("is not offered without an http(s) website or on a note", async () => {
    await renderLogin({ ...site, urls: [] });
    expect(changeButton()).toBeUndefined();
    reveal.mockResolvedValue("");
    await renderNote();
    expect(changeButton()).toBeUndefined();
  });
});
