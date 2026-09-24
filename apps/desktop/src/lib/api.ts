// Typed wrappers around the Tauri command allowlist. This module is the only
// place the UI talks to the Rust core.
//
// Rules: never log arguments or results; never persist anything returned
// here (no localStorage/sessionStorage/IndexedDB).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AccountStatus,
  CopyField,
  CopyResult,
  DeviceEntry,
  DeviceStatus,
  EmergencyKit,
  GeneratedPassword,
  GeneratorOptions,
  ImportResult,
  ItemInput,
  ItemOverview,
  SecretField,
  Settings,
  SyncReport,
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

  /** First run: the invite string from the account's operator. */
  activate: (invite: string, password: string) =>
    call<VaultStatus>("activate_account", { invite: invite.trim(), password }),
  /**
   * A device that has no vault yet joins an existing account. An empty
   * `secretKey` uses the one this computer already holds, if any — which is
   * how a setup interrupted after the server accepted it is finished.
   */
  signIn: (serverUrl: string, email: string, password: string, secretKey: string) =>
    call<VaultStatus>("sign_in", {
      serverUrl: serverUrl.trim(),
      email: email.trim(),
      password,
      secretKey: secretKey.trim() ? secretKey.trim() : null,
    }),
  /** Ends the server session and locks the vault. The vault stays on disk. */
  signOut: () => call<void>("sign_out"),
  accountStatus: () => call<AccountStatus | null>("account_status"),
  listDevices: () => call<DeviceEntry[]>("list_devices"),
  revokeDevice: (id: string) => call<void>("revoke_device", { id }),
  /** Remove this computer from the account; the vault stays on the server. */
  removeDevice: (confirmation: string) => call<void>("remove_device", { confirmation }),
  syncNow: () => call<SyncReport>("sync_now"),
  /** Re-download the whole vault. For a replica suspected to be stale. */
  resync: () => call<SyncReport>("resync_vault"),

  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),

  onLocked: (handler: (reason: string) => void): Promise<UnlistenFn> =>
    listen<{ reason: string }>("vault://locked", (e) => handler(e.payload.reason)),
  /** A login was saved from the browser extension. Carries no data. */
  onItemsChanged: (handler: () => void): Promise<UnlistenFn> => listen("vault://items-changed", () => handler()),
  /** The server session was opened or lost. */
  onConnectivity: (handler: (online: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("vault://connectivity", (e) => handler(e.payload)),
  /** The server refused this computer's sign-in after an unlock. */
  onSignedOut: (handler: () => void): Promise<UnlistenFn> => listen("vault://signed-out", () => handler()),
  /** A pull finished; carries counts only. */
  onSynced: (handler: (report: SyncReport) => void): Promise<UnlistenFn> =>
    listen<SyncReport>("vault://synced", (e) => handler(e.payload)),
  /**
   * This computer was removed from its account; the vault reopened empty.
   * `keychainWarning` is set when the Secret Key may still be in the system
   * keychain and the user must delete it by hand.
   */
  onRemoved: (handler: (removed: { keychainWarning: string | null }) => void): Promise<UnlistenFn> =>
    listen<{ keychainWarning: string | null }>("vault://removed", (e) =>
      handler({ keychainWarning: e.payload?.keychainWarning ?? null }),
    ),
};
