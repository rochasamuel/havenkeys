import { ApiError, api } from "./api";

/**
 * A write the server refused because another device changed or deleted the
 * item first: sync now, so the stale copy here is replaced or removed rather
 * than waiting for the next periodic sync. The caller still shows its message.
 */
export async function syncAfterConflict(e: unknown): Promise<void> {
  if (!(e instanceof ApiError) || e.code !== "item_changed_elsewhere") return;
  try {
    await api.syncNow();
  } catch {
    // Offline or refused: the periodic sync will catch up.
  }
}
