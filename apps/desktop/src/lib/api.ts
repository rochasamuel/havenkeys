// Typed wrappers around the Tauri command allowlist. This module is the only
// place the UI talks to the Rust core.
//
// Rules: never log arguments or results; never persist anything returned
// here (no localStorage/sessionStorage/IndexedDB).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CopyField,
  CopyResult,
  DeviceStatus,
  EmergencyKit,
  SyncReport,
  GeneratedPassword,
  GeneratorOptions,
  ImportResult,
  ItemInput,
  ItemOverview,
  SecretField,
  Settings,
  TotpCode,
  VaultStatus,
} from "./types";

export class ApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/**
 * Normalize whatever the IPC layer rejected with into an ApiError that only
 * carries the core's fixed code/message. Unknown shapes become a generic
 * error so nothing unexpected (possibly containing input) reaches the UI.
 */
export function toApiError(raw: unknown): ApiError {
  if (
    typeof raw === "object" &&
    raw !== null &&
    typeof (raw as { code?: unknown }).code === "string" &&
    typeof (raw as { message?: unknown }).message === "string"
  ) {
    const { code, message } = raw as { code: string; message: string };
    return new ApiError(code, message);
  }
  return new ApiError("internal", "Something went wrong. Try again.");
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (raw) {
    throw toApiError(raw);
  }
}

export const api = {
  status: () => call<VaultStatus>("vault_status"),
  createVault: (password: string) => call<VaultStatus>("create_vault", { password }),
  /** `secretKey` only when this device does not have it yet (from the Emergency Kit). */
  unlock: (password: string, secretKey?: string) =>
    call<VaultStatus>("unlock_vault", { password, secretKey: secretKey?.trim() ? secretKey : null }),
  lock: () => call<void>("lock_vault"),
  changeMasterPassword: (current: string, next: string) =>
    call<void>("change_master_password", { current, new: next }),
  recordActivity: () => call<void>("record_activity"),

  listItems: (query?: string) =>
    call<ItemOverview[]>("list_items", { query: query?.trim() ? query : null }),
  getItem: (id: string) => call<ItemOverview>("get_item", { id }),
  reveal: (id: string, field: SecretField) => call<string>("reveal_secret", { id, field }),
  /** When each previous password was replaced (Unix ms, newest first). */
  passwordHistory: (id: string) => call<number[]>("password_history", { id }),
  revealPreviousPassword: (id: string, index: number) =>
    call<string>("reveal_previous_password", { id, index }),
  totp: (id: string) => call<TotpCode>("get_totp_code", { id }),
  copy: (id: string, field: CopyField) => call<CopyResult>("copy_secret", { id, field }),
  createItem: (input: ItemInput) => call<ItemOverview>("create_item", { input }),
  updateItem: (id: string, input: ItemInput) => call<ItemOverview>("update_item", { id, input }),
  deleteItem: (id: string) => call<void>("delete_item", { id }),

  generate: (options: GeneratorOptions) => call<GeneratedPassword>("generate_password", { options }),
  copyGenerated: (value: string) => call<CopyResult>("copy_generated_password", { value }),

  /** Opens the native file picker in Rust; resolves to null if cancelled. */
  import1pux: () => call<ImportResult | null>("import_1pux"),
  /** Deletes the file chosen in the last import (the UI never sends a path). */
  deleteImportFile: () => call<void>("delete_import_file"),

  deviceStatus: () => call<DeviceStatus>("device_status"),
  emergencyKit: () => call<EmergencyKit>("get_emergency_kit"),
  setupSecretKey: (password: string) => call<void>("setup_secret_key", { password }),
  /** Opens the native folder picker in Rust; resolves to the folder name or null. */
  chooseSyncFolder: () => call<string | null>("choose_sync_folder"),
  syncNow: () => call<SyncReport | null>("sync_now"),
  stopSync: () => call<void>("stop_sync"),
  /** New device: pick the sync folder that holds the vault (native picker). */
  pickJoinFolder: () => call<{ folder: string } | null>("pick_join_folder"),
  joinSyncedVault: (password: string, secretKey: string) =>
    call<VaultStatus>("join_synced_vault", { password, secretKey }),

  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),

  onLocked: (handler: (reason: string) => void): Promise<UnlistenFn> =>
    listen<{ reason: string }>("vault://locked", (e) => handler(e.payload.reason)),
  /** A login was saved from the browser extension. Carries no data. */
  onItemsChanged: (handler: () => void): Promise<UnlistenFn> => listen("vault://items-changed", () => handler()),
};
