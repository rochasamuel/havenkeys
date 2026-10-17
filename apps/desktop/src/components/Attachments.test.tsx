// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AttachmentMeta, ItemOverview } from "../lib/types";

let locked: ((reason: string) => void) | null = null;
const addAttachments = vi.fn<(id: string) => Promise<ItemOverview | null>>();
const saveAttachment = vi.fn<(id: string, a: string) => Promise<boolean>>();
const previewAttachment = vi.fn<(id: string, a: string) => Promise<ArrayBuffer>>();
const removeAttachment = vi.fn<(id: string, a: string) => Promise<ItemOverview>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    addAttachments: (id: string) => addAttachments(id),
    saveAttachment: (id: string, a: string) => saveAttachment(id, a),
    previewAttachment: (id: string, a: string) => previewAttachment(id, a),
    removeAttachment: (id: string, a: string) => removeAttachment(id, a),
    onLocked: (h: (reason: string) => void) => ((locked = h), Promise.resolve(() => void (locked = null))),
  },
}));

import { en } from "../i18n/en";
import { Attachments } from "./Attachments";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const photo: AttachmentMeta = { id: "a1", name: "receipt.png", mime: "image/png", size: 245_760, createdAt: 1 };
const pdf: AttachmentMeta = { id: "a2", name: "receipt.pdf", mime: "application/pdf", size: 1_572_864, createdAt: 2 };
const item: ItemOverview = {
  id: "i1", itemType: "login", title: "Shop", username: null, urls: [], hasPassword: false, hasTotp: false,
  hasNotes: false, hasPasskey: false, autoSignIn: false, tags: [], attachments: [photo, pdf], createdAt: 1, updatedAt: 1,
};

const createObjectURL = vi.fn((_: Blob) => "blob:preview-1");
const revokeObjectURL = vi.fn((_: string) => undefined);

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  for (const f of [addAttachments, saveAttachment, previewAttachment, removeAttachment]) f.mockReset();
  saveAttachment.mockResolvedValue(true);
  previewAttachment.mockResolvedValue(new Uint8Array([0x89, 0x50]).buffer);
  removeAttachment.mockResolvedValue(item);
  createObjectURL.mockClear();
  revokeObjectURL.mockClear();
  Object.assign(URL, { createObjectURL, revokeObjectURL });
  sessionStorage.clear();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function button(label: string): HTMLButtonElement | undefined {
  return [...document.querySelectorAll("button")].find((b) => b.getAttribute("aria-label") === label || b.textContent === label);
}

async function render(readOnly = false) {
  await act(async () => root.render(<Attachments item={item} readOnly={readOnly} />));
}

describe("Attachments", () => {
  it("lists_attachments_without_fetching_bytes", async () => {
    await render();
    expect(host.textContent).toContain("receipt.png");
    expect(host.textContent).toContain("receipt.pdf");
    expect(host.textContent).toContain("240 KB");
    expect(host.textContent).toContain("1.5 MB");
    expect(button(en.attachments.add)).toBeDefined();
    expect(previewAttachment).not.toHaveBeenCalled();
    expect(saveAttachment).not.toHaveBeenCalled();
  });

  it("hides Add file when read-only", async () => {
    await render(true);
    expect(button(en.attachments.add)).toBeUndefined();
    expect(button(en.attachments.remove("receipt.pdf"))?.disabled).toBe(true);
  });

  it("preview_only_for_images", async () => {
    await render();
    expect(button(en.attachments.preview("receipt.png"))).toBeDefined();
    expect(button(en.attachments.preview("receipt.pdf"))).toBeUndefined();
  });

  it("preview_is_revoked_on_lock", async () => {
    await render();
    await act(async () => button(en.attachments.preview("receipt.png"))!.click());
    expect(previewAttachment).toHaveBeenCalledWith("i1", "a1");
    const img = document.querySelector<HTMLImageElement>('[role="dialog"] img');
    expect(img?.getAttribute("src")).toBe("blob:preview-1");
    expect(createObjectURL.mock.calls[0]?.[0].type).toBe("image/png");
    await act(async () => locked?.("manual"));
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:preview-1");
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it("revokes the preview on close and on unmount", async () => {
    await render();
    await act(async () => button(en.attachments.preview("receipt.png"))!.click());
    await act(async () => button(en.attachments.closePreview)!.click());
    expect(revokeObjectURL).toHaveBeenCalledTimes(1);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    await act(async () => button(en.attachments.preview("receipt.png"))!.click());
    act(() => root.unmount());
    expect(revokeObjectURL).toHaveBeenCalledTimes(2);
    root = createRoot(host);
  });

  it("remove_asks_first", async () => {
    await render();
    await act(async () => button(en.attachments.remove("receipt.pdf"))!.click());
    expect(host.textContent).toContain(
      "Remove receipt.pdf? It will be deleted from HavenKeys on all your devices.",
    );
    expect(removeAttachment).not.toHaveBeenCalled();
    await act(async () => button(en.common.remove)!.click());
    expect(removeAttachment).toHaveBeenCalledWith("i1", "a2");
  });

  it("save_warns_once_per_session", async () => {
    await render();
    await act(async () => button(en.attachments.save("receipt.pdf"))!.click());
    expect(host.textContent).toContain("A saved copy is not protected by HavenKeys.");
    expect(saveAttachment).not.toHaveBeenCalled();
    await act(async () => button(en.attachments.saveAnyway)!.click());
    expect(saveAttachment).toHaveBeenCalledWith("i1", "a2");
    await act(async () => button(en.attachments.save("receipt.png"))!.click());
    expect(saveAttachment).toHaveBeenLastCalledWith("i1", "a1");
    expect(host.textContent).not.toContain("A saved copy is not protected by HavenKeys.");
  });
});
