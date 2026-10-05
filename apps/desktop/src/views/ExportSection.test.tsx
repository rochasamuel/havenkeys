// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ExportFormat, ExportResult, ExportSummary } from "../lib/types";

const summary: ExportSummary = {
  logins: 3,
  secureNotes: 1,
  cards: 0,
  identities: 0,
  passkeysLeftOut: 2,
  passwordHistoryLeftOut: 0,
  customFieldsLeftOut: 0,
  itemsLeftOut: 1,
  unreadable: 0,
};
const exportSummary = vi.fn<(f: ExportFormat) => Promise<ExportSummary>>();
const exportFile = vi.fn<(f: ExportFormat, m: string, b: string | null) => Promise<ExportResult | null>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    exportSummary: (f: ExportFormat) => exportSummary(f),
    exportFile: (f: ExportFormat, m: string, b: string | null) => exportFile(f, m, b),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { en } from "../i18n/en";
import { ExportSection } from "./ExportSection";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  exportSummary.mockReset().mockResolvedValue(summary);
  exportFile.mockReset().mockResolvedValue(null);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

function type(selector: string, value: string) {
  const input = host.querySelector<HTMLInputElement>(selector)!;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function exportButton(): HTMLButtonElement {
  return [...host.querySelectorAll("button")].find((b) => b.textContent === en.export.export)!;
}

describe("ExportSection", () => {
  it("starts on the encrypted backup and needs matching backup passwords", async () => {
    await act(async () => root.render(<ExportSection />));
    expect(host.querySelector<HTMLInputElement>('input[value="backup"]')?.checked).toBe(true);
    expect(exportSummary).toHaveBeenCalledWith("backup");
    await act(async () => type('input[name="master-password"]', "master pw"));
    await act(async () => type('input[name="backup-password"]', "backup passphrase"));
    expect(exportButton().disabled).toBe(true);
    await act(async () => type('input[name="backup-password-again"]', "backup passphrase"));
    expect(exportButton().disabled).toBe(false);
    await act(async () => exportButton().click());
    expect(exportFile).toHaveBeenCalledWith("backup", "master pw", "backup passphrase");
  });

  it("gates plaintext exports behind the warning and shows what is left out", async () => {
    await act(async () => root.render(<ExportSection />));
    await act(async () => host.querySelector<HTMLInputElement>('input[value="csv"]')!.click());
    expect(exportSummary).toHaveBeenLastCalledWith("csv");
    expect(host.textContent).toContain(en.export.plaintextWarning);
    expect(host.textContent).toContain(en.export.passkeysLeftOut(2));
    await act(async () => type('input[name="master-password"]', "master pw"));
    expect(exportButton().disabled).toBe(true);
    await act(async () => host.querySelector<HTMLInputElement>('input[name="understand"]')!.click());
    expect(exportButton().disabled).toBe(false);
    await act(async () => exportButton().click());
    expect(exportFile).toHaveBeenCalledWith("csv", "master pw", null);
  });

  it("clears every password after an attempt", async () => {
    exportFile.mockRejectedValue(new Error("nope"));
    await act(async () => root.render(<ExportSection />));
    await act(async () => type('input[name="master-password"]', "master pw"));
    await act(async () => type('input[name="backup-password"]', "backup passphrase"));
    await act(async () => type('input[name="backup-password-again"]', "backup passphrase"));
    await act(async () => exportButton().click());
    for (const name of ["master-password", "backup-password", "backup-password-again"]) {
      expect(host.querySelector<HTMLInputElement>(`input[name="${name}"]`)!.value).toBe("");
    }
  });

  it("keeps Export disabled until the summary has loaded", async () => {
    let resolve: (s: ExportSummary) => void = () => undefined;
    exportSummary.mockReturnValue(new Promise((r) => (resolve = r)));
    await act(async () => root.render(<ExportSection />));
    await act(async () => type('input[name="master-password"]', "master pw"));
    await act(async () => type('input[name="backup-password"]', "backup passphrase"));
    await act(async () => type('input[name="backup-password-again"]', "backup passphrase"));
    expect(exportButton().disabled).toBe(true);
    await act(async () => resolve(summary));
    expect(exportButton().disabled).toBe(false);
  });

  it("says so when the summary can't be loaded, and stays disabled", async () => {
    exportSummary.mockRejectedValue(new Error("locked"));
    await act(async () => root.render(<ExportSection />));
    await act(async () => type('input[name="master-password"]', "master pw"));
    await act(async () => type('input[name="backup-password"]', "backup passphrase"));
    await act(async () => type('input[name="backup-password-again"]', "backup passphrase"));
    expect(host.textContent).toContain(en.export.summaryFailed);
    expect(exportButton().disabled).toBe(true);
  });
});
