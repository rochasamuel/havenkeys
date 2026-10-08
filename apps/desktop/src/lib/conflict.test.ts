import { beforeEach, describe, expect, it, vi } from "vitest";

const syncNow = vi.fn();
vi.mock("./api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./api")>()),
  api: { syncNow: () => syncNow() },
}));

import { ApiError } from "./api";
import { syncAfterConflict } from "./conflict";

beforeEach(() => {
  syncNow.mockReset().mockResolvedValue({});
});

describe("syncAfterConflict", () => {
  it("syncs when another device changed the item first", async () => {
    await syncAfterConflict(new ApiError("item_changed_elsewhere", "x"));
    expect(syncNow).toHaveBeenCalledTimes(1);
  });

  it("does nothing for any other failure", async () => {
    await syncAfterConflict(new ApiError("offline", "x"));
    await syncAfterConflict(new Error("boom"));
    expect(syncNow).not.toHaveBeenCalled();
  });

  it("swallows a failed sync", async () => {
    syncNow.mockRejectedValue(new ApiError("offline", "x"));
    await expect(syncAfterConflict(new ApiError("item_changed_elsewhere", "x"))).resolves.toBeUndefined();
  });
});
