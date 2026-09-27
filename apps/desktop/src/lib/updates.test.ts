import { describe, expect, it } from "vitest";

import type { UpdatePhase, UpdateStatus } from "./types";
import { bannerFor } from "./updates";

const base = { autoCheck: true, canInstallInPlace: true, currentVersion: "0.9.0" };
const s = (phase: UpdatePhase, extra: Partial<UpdateStatus> = {}): UpdateStatus =>
  ({ ...base, ...phase, ...extra }) as UpdateStatus;
const available = s({ phase: "available", version: "0.9.1", notes: "Fixes." });

describe("bannerFor", () => {
  it("shows nothing before the status arrives, while idle or checking", () => {
    expect(bannerFor(null, null)).toEqual({ kind: "hidden" });
    expect(bannerFor(s({ phase: "idle" }), null)).toEqual({ kind: "hidden" });
    expect(bannerFor(s({ phase: "checking" }), null)).toEqual({ kind: "hidden" });
  });

  it("offers an available update, installing in place when it can", () => {
    expect(bannerFor(available, null)).toEqual({
      kind: "available",
      version: "0.9.1",
      notes: "Fixes.",
      action: "update",
    });
  });

  it("offers a download for .deb/.rpm installs", () => {
    const deb = { ...available, canInstallInPlace: false };
    expect(bannerFor(deb, null)).toMatchObject({ kind: "available", action: "download" });
  });

  it("stays hidden once that version is dismissed, but not for a newer one", () => {
    expect(bannerFor(available, "0.9.1")).toEqual({ kind: "hidden" });
    expect(bannerFor(available, "0.9.0")).toMatchObject({ kind: "available" });
  });

  it("passes notes through as text", () => {
    const markup = "<img src=x onerror=alert(1)>";
    const withMarkup = s({ phase: "available", version: "0.9.1", notes: markup });
    expect(bannerFor(withMarkup, null)).toMatchObject({ notes: markup });
  });

  it("shows download progress, capped at 100, or none when the size is unknown", () => {
    expect(bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 50, total: 200 }), null)).toEqual({
      kind: "downloading",
      version: "0.9.1",
      percent: 25,
    });
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 300, total: 200 }), null),
    ).toMatchObject({ percent: 100 });
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 5, total: null }), null),
    ).toMatchObject({ percent: null });
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 5, total: 0 }), null),
    ).toMatchObject({ percent: null });
  });

  it("shows progress even if the version was dismissed earlier", () => {
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 0, total: 10 }), "0.9.1"),
    ).toMatchObject({ kind: "downloading" });
  });

  it("shows installing and install failures; a failed check stays out of the banner", () => {
    expect(bannerFor(s({ phase: "installing" }), null)).toEqual({ kind: "installing" });
    expect(bannerFor(s({ phase: "failed", during: "install" }), null)).toEqual({ kind: "failed" });
    expect(bannerFor(s({ phase: "failed", during: "check" }), null)).toEqual({ kind: "hidden" });
  });
});
