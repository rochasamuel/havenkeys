// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportResult, ImportSource } from "../lib/types";

const restoreBackup = vi.fn<(p: string) => Promise<ImportResult | null>>();
const importFile = vi.fn<(source: ImportSource) => Promise<ImportResult | null>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    importFile: (source: ImportSource) => importFile(source),
    restoreBackup: (p: string) => restoreBackup(p),
    deleteImportFile: () => Promise.resolve(),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { en } from "../i18n/en";
import { ImportSection } from "./ImportSection";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  importFile.mockReset();
  importFile.mockResolvedValue(null);
  restoreBackup.mockReset();
  restoreBackup.mockResolvedValue(null);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function chooseButton(): HTMLButtonElement {
  return [...host.querySelectorAll("button")].find((b) => b.textContent?.startsWith("Choose"))!;
}

describe("ImportSection", () => {
  it("lists every source and starts on 1Password", async () => {
    await act(async () => root.render(<ImportSection onImported={() => undefined} />));
    const radios = [...host.querySelectorAll<HTMLInputElement>('input[type="radio"]')];
    expect(radios.map((r) => r.value)).toEqual([
      "onePassword",
      "bitwardenJson",
      "bitwardenCsv",
      "chrome",
      "firefox",
      "keePassXc",
      "lastPass",
      "havenKeysBackup",
    ]);
    expect(radios[0]?.checked).toBe(true);
    expect(chooseButton().textContent).toBe(en.import.choose("1pux"));
    expect(host.textContent).toContain(en.import.howTo.onePassword);
  });

  it("imports from the chosen source, with its instructions and file type", async () => {
    await act(async () => root.render(<ImportSection onImported={() => undefined} />));
    const firefox = host.querySelector<HTMLInputElement>('input[value="firefox"]')!;
    await act(async () => firefox.click());
    expect(host.textContent).toContain(en.import.howTo.firefox);
    expect(chooseButton().textContent).toBe(en.import.choose("csv"));
    await act(async () => chooseButton().click());
    expect(importFile).toHaveBeenCalledWith("firefox");
  });

  it("reports what was left out", async () => {
    importFile.mockResolvedValue({
      fileName: "bitwarden.json",
      report: {
        imported: 2,
        logins: 1,
        secureNotes: 1,
        cards: 0,
        convertedToNotes: 0,
        skippedDuplicates: 0,
        skippedArchived: 0,
        failed: 0,
        attachmentsSkipped: 0,
        passwordHistorySkipped: 0,
        urlsMovedToNotes: 0,
        fieldsToNotes: 0,
        ssoUpgraded: 0,
        passkeysSkipped: 3,
        skippedExisting: 0,
        identities: 0,
      },
    });
    const onImported = vi.fn();
    await act(async () => root.render(<ImportSection onImported={onImported} />));
    await act(async () => host.querySelector<HTMLInputElement>('input[value="bitwardenJson"]')!.click());
    await act(async () => chooseButton().click());
    expect(importFile).toHaveBeenCalledWith("bitwardenJson");
    expect(onImported).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain(en.import.passkeysSkipped(3));
  });

  it("restores a HavenKeys backup with its password, and offers no file deletion", async () => {
    restoreBackup.mockResolvedValue({
      fileName: "havenkeys-export-2026-10-05.hkbackup",
      report: {
        imported: 3, logins: 2, secureNotes: 0, cards: 0, identities: 1, convertedToNotes: 0,
        skippedDuplicates: 0, skippedExisting: 3, skippedArchived: 0, failed: 2,
        attachmentsSkipped: 0, passwordHistorySkipped: 0, passkeysSkipped: 0,
        urlsMovedToNotes: 0, fieldsToNotes: 0, ssoUpgraded: 0,
      },
    });
    await act(async () => root.render(<ImportSection onImported={() => undefined} />));
    await act(async () => host.querySelector<HTMLInputElement>('input[value="havenKeysBackup"]')!.click());
    expect(chooseButton().textContent).toBe(en.import.choose("hkbackup"));
    expect(chooseButton().disabled).toBe(true);
    const pw = host.querySelector<HTMLInputElement>('input[name="restore-password"]')!;
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    await act(async () => {
      setter.call(pw, "backup passphrase");
      pw.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () => chooseButton().click());
    expect(restoreBackup).toHaveBeenCalledWith("backup passphrase");
    expect(importFile).not.toHaveBeenCalled();
    expect(host.textContent).toContain(en.import.skippedExisting(3));
    expect(host.textContent).toContain(en.import.summary("havenkeys-export-2026-10-05.hkbackup", 2, 0, 0, 1));
    expect(host.textContent).toContain("1 identity");
    expect(host.textContent).toContain(en.import.restoreFailedItems(2));
    expect(host.textContent).not.toContain(en.import.failedItems(2));
    expect(host.textContent).not.toContain(en.import.deleteExport);
    expect(pw.value).toBe("");
  });

  it("clears the backup password when another source is chosen", async () => {
    await act(async () => root.render(<ImportSection onImported={() => undefined} />));
    await act(async () => host.querySelector<HTMLInputElement>('input[value="havenKeysBackup"]')!.click());
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    await act(async () => {
      const pw = host.querySelector<HTMLInputElement>('input[name="restore-password"]')!;
      setter.call(pw, "backup passphrase");
      pw.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () => host.querySelector<HTMLInputElement>('input[value="chrome"]')!.click());
    expect(host.querySelector('input[name="restore-password"]')).toBeNull();
    await act(async () => host.querySelector<HTMLInputElement>('input[value="havenKeysBackup"]')!.click());
    expect(host.querySelector<HTMLInputElement>('input[name="restore-password"]')!.value).toBe("");
  });
});
