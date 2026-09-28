// What the update banner shows for a given status. The notes are passed on
// as a string for React to render as text; nothing here builds markup.

import type { UpdateStatus } from "./types";

export type Banner =
  | { kind: "hidden" }
  | { kind: "available"; version: string; notes: string; action: "update" | "download" }
  | { kind: "downloading"; version: string; percent: number | null }
  | { kind: "installing" }
  | { kind: "failed" };

/**
 * `dismissed` is the version the user chose "Later" for in this run of the
 * app. A failed *check* is reported in Settings, where it was asked for.
 */
export function bannerFor(status: UpdateStatus | null, dismissed: string | null): Banner {
  if (!status) return { kind: "hidden" };
  switch (status.phase) {
    case "available":
      if (status.version === dismissed) return { kind: "hidden" };
      return {
        kind: "available",
        version: status.version,
        notes: status.notes,
        action: status.canInstallInPlace ? "update" : "download",
      };
    case "downloading": {
      const percent =
        status.total !== null && status.total > 0
          ? Math.min(100, Math.floor((status.downloaded * 100) / status.total))
          : null;
      return { kind: "downloading", version: status.version, percent };
    }
    case "installing":
      return { kind: "installing" };
    case "failed":
      return status.during === "install" ? { kind: "failed" } : { kind: "hidden" };
    default:
      return { kind: "hidden" };
  }
}
