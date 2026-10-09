// Typed wrappers around the Tauri command allowlist. This module is the only
// place the UI talks to the Rust core.
//
// Rules: never log arguments or results; never persist anything returned
// here (no localStorage/sessionStorage/IndexedDB).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AccountField,
  AccountStatus,
  InvitePreview,
  AddressPart,
  CardCopyField,
  CardNumberCheck,
  CardRevealField,
  CardView,
  CopyField,
  CopyResult,
  DeviceEntry,
  DeviceStatus,
  EmergencyKit,
  ExportFormat,
  HealthCheck,
  HealthReport,
  ExportResult,
  ExportSummary,
  GeneratedPassword,
  GeneratorOptions,
  IdentityCopyField,
  IdentityView,
  ImportResult,
  ImportSource,
  ItemInput,
  ItemOverview,
  PairingCode,
  PairingState,
  PasskeyInfo,
  ProviderLogin,
  ScannedTotp,
  SecretField,
  SectionView,
  Settings,
  SsoAccount,
  SsoProvider,
  SyncReport,
  TotpCode,
  TrashEntry,
  UpdateStatus,
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
  /** `secretKey` only when this device does not have it yet (from the Recovery Sheet). */
  unlock: (password: string, secretKey?: string) =>
    call<VaultStatus>("unlock_vault", { password, secretKey: secretKey?.trim() ? secretKey : null }),
  lock: () => call<void>("lock_vault"),
  changeMasterPassword: (current: string, next: string) =>
    call<void>("change_master_password", { current, new: next }),
  recordActivity: () => call<void>("record_activity"),

  listItems: (query?: string) =>
    call<ItemOverview[]>("list_items", { query: query?.trim() ? query : null }),
  getItem: (id: string) => call<ItemOverview>("get_item", { id }),
  /** The vault's logins for a provider's sign-in page, for the editor's picker. */
  ssoAccounts: (provider: SsoProvider) => call<SsoAccount[]>("sso_accounts", { provider }),
  /** The provider login a "Sign in with" login links to, decided in Rust. */
  providerLogin: (id: string) => call<ProviderLogin>("provider_login", { id }),
  reveal: (id: string, field: SecretField) => call<string>("reveal_secret", { id, field }),
  /** When each previous password was replaced (Unix ms, newest first). */
  passwordHistory: (id: string) => call<number[]>("password_history", { id }),
  listPasskeys: (id: string) => call<PasskeyInfo[]>("list_passkeys", { id }),
  deletePasskey: (id: string, credentialId: string) => call<ItemOverview>("delete_passkey", { id, credentialId }),
  revealPreviousPassword: (id: string, index: number) =>
    call<string>("reveal_previous_password", { id, index }),
  totp: (id: string) => call<TotpCode>("get_totp_code", { id }),
  copy: (id: string, field: CopyField) => call<CopyResult>("copy_secret", { id, field }),
  /** Rust opens it only if `url` is one of the item's saved websites. */
  openWebsite: (id: string, url: string) => call<void>("open_website", { id, url }),
  healthReport: () => call<HealthReport>("health_report"),
  setHealthIgnored: (id: string, checks: HealthCheck[]) => call<void>("set_health_ignored", { id, checks }),
  openHealthHelp: (id: string, check: HealthCheck) => call<void>("open_health_help", { id, check }),
  loginFields: (id: string) => call<SectionView[]>("login_fields", { id }),
  revealLoginField: (id: string, fieldId: string) => call<string>("reveal_login_field", { id, fieldId }),
  loginFieldTotp: (id: string, fieldId: string) => call<TotpCode>("login_field_totp", { id, fieldId }),
  copyLoginField: (id: string, fieldId: string, part?: AddressPart) =>
    call<CopyResult>("copy_login_field", { id, fieldId, part: part ?? null }),
  /** Rust opens only that field's saved http(s) address. */
  openLoginFieldUrl: (id: string, fieldId: string) => call<void>("open_login_field_url", { id, fieldId }),
  /** Rust reads the clipboard image, then the screen; returns tokens and labels, never the secret. */
  scanTotpQr: () => call<ScannedTotp[]>("scan_totp_qr"),
  createItem: (input: ItemInput) => call<ItemOverview>("create_item", { input }),
  updateItem: (id: string, input: ItemInput) => call<ItemOverview>("update_item", { id, input }),
  /** Moves the item to the Trash; `false` when it was deleted for good (its details did not open). */
  trashItem: (id: string) => call<boolean>("trash_item", { id }),
  restoreItem: (id: string) => call<ItemOverview>("restore_item", { id }),
  purgeItem: (id: string) => call<void>("purge_item", { id }),
  emptyTrash: () => call<number>("empty_trash"),
  listTrash: () => call<TrashEntry[]>("list_trash"),

  generate: (options: GeneratorOptions) => call<GeneratedPassword>("generate_password", { options }),
  copyGenerated: (value: string) => call<CopyResult>("copy_generated_password", { value }),

  /** Opens the native file picker in Rust; resolves to null if cancelled. */
  importFile: (source: ImportSource) => call<ImportResult | null>("import_file", { source }),
  /** Deletes the file chosen in the last import (the UI never sends a path). */
  deleteImportFile: () => call<void>("delete_import_file"),
  exportSummary: (format: ExportFormat) => call<ExportSummary>("export_summary", { format }),
  /** Rust opens the save dialog; `null` when it was cancelled. */
  exportFile: (format: ExportFormat, masterPassword: string, backupPassword: string | null) =>
    call<ExportResult | null>("export_file", { format, masterPassword, backupPassword }),
  /** Rust opens the file picker; `null` when it was cancelled. */
  restoreBackup: (backupPassword: string) => call<ImportResult | null>("restore_backup", { backupPassword }),

  deviceStatus: () => call<DeviceStatus>("device_status"),
  emergencyKit: () => call<EmergencyKit>("get_emergency_kit"),
  /** The Account item's Secret Key, on an explicit reveal. */
  revealAccountSecretKey: () => call<string>("reveal_account_secret_key"),
  /** Rust copies the Account item's value and clears the clipboard later. */
  copyAccountField: (field: AccountField) => call<CopyResult>("copy_account_field", { field }),
  /** The account's one Identity; only while unlocked. */
  identityItemId: () => call<string>("identity_item_id"),
  /** Every value of the identity, on opening it. */
  revealIdentity: (id: string) => call<IdentityView>("reveal_identity", { id }),
  /** Rust copies the value and clears the clipboard later. */
  copyIdentityField: (id: string, field: IdentityCopyField) => call<CopyResult>("copy_identity_field", { id, field }),
  /** A card's values without its number or verification number, on opening it. */
  revealCard: (id: string) => call<CardView>("reveal_card", { id }),
  /** The number or the verification number, on an explicit reveal. */
  revealCardField: (id: string, field: CardRevealField) => call<string>("reveal_card_field", { id, field }),
  /** Rust copies the value and clears the clipboard later. */
  copyCardField: (id: string, field: CardCopyField) => call<CopyResult>("copy_card_field", { id, field }),
  /** Brand and check digit of a number being typed; Rust keeps nothing. */
  checkCardNumber: (number: string) => call<CardNumberCheck>("check_card_number", { number }),

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
  previewInvite: (invite: string) => call<InvitePreview>("preview_invite", { invite: invite.trim() }),
  /** Opens the fixed sign-up page; the renderer never supplies a URL. */
  openSignup: () => call<void>("open_signup"),
  openPricing: () => call<void>("open_pricing"),
  pairingStart: (serverUrl: string) => call<PairingCode>("pairing_start", { serverUrl: serverUrl.trim() }),
  pairingPoll: () => call<PairingState>("pairing_poll"),
  pairingCancel: () => call<void>("pairing_cancel"),
  listDevices: () => call<DeviceEntry[]>("list_devices"),
  revokeDevice: (id: string) => call<void>("revoke_device", { id }),
  /** Remove this computer from the account; the vault stays on the server. */
  removeDevice: (confirmation: string) => call<void>("remove_device", { confirmation }),
  /** Delete the account on the server and this computer's copy. Irreversible. */
  deleteAccount: (confirmation: string, masterPassword: string) =>
    call<void>("delete_account", { confirmation, masterPassword }),
  syncNow: () => call<SyncReport>("sync_now"),
  /** Re-download the whole vault. For a replica suspected to be stale. */
  resync: () => call<SyncReport>("resync_vault"),

  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),
  /** The generator tab's policy; the extension's "Generate strong password" uses it too. */
  setGeneratorOptions: (options: GeneratorOptions) => call<GeneratorOptions>("set_generator_options", { options }),
  /** Whether this computer opens HavenKeys at login (an OS setting, not a vault one). */
  launchAtLogin: () => call<boolean>("launch_at_login"),
  setLaunchAtLogin: (enabled: boolean) => call<boolean>("set_launch_at_login", { enabled }),
  /** Relabel the tray menu. Rust accepts only "en" and "pt-BR". */
  setUiLanguage: (lang: "en" | "pt-BR") => call<void>("set_ui_language", { lang }),

  /** In-app updates. Rust talks to GitHub; the UI only sees this status. */
  updateStatus: () => call<UpdateStatus>("update_status"),
  /** "Check now". A failed check shows in the status (`failed`/`check`), not as a rejection. */
  checkForUpdate: () => call<UpdateStatus>("check_for_update"),
  /** Download, verify, lock, install and restart — or, for a .deb/.rpm install, open the release page. */
  installUpdate: () => call<void>("install_update"),
  setUpdateAutoCheck: (enabled: boolean) => call<UpdateStatus>("set_update_auto_check", { enabled }),

  onLocked: (handler: (reason: string) => void): Promise<UnlistenFn> =>
    listen<{ reason: string }>("vault://locked", (e) => handler(e.payload.reason)),
  /** A login was saved from the browser extension. Carries no data. */
  onItemsChanged: (handler: () => void): Promise<UnlistenFn> => listen("vault://items-changed", () => handler()),
  /** The browser extension asked to edit this item (already checked against the page in Rust). */
  onOpenItem: (handler: (id: string) => void): Promise<UnlistenFn> =>
    listen<string>("vault://open-item", (e) => handler(e.payload)),
  /** The server session was opened or lost. */
  onConnectivity: (handler: (online: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("vault://connectivity", (e) => handler(e.payload)),
  /** The server refused this computer's sign-in after an unlock. */
  onSignedOut: (handler: () => void): Promise<UnlistenFn> => listen("vault://signed-out", () => handler()),
  /** The account's plan changed (trial ended, subscribed, lapsed). */
  onPlanChanged: (handler: () => void): Promise<UnlistenFn> => listen("plan_changed", () => handler()),
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
  /** The account was deleted (here or elsewhere); this computer's copy is erased. */
  onAccountDeleted: (handler: (removed: { keychainWarning: string | null }) => void): Promise<UnlistenFn> =>
    listen<{ keychainWarning: string | null }>("vault://account-deleted", (e) =>
      handler({ keychainWarning: e.payload?.keychainWarning ?? null }),
    ),
  onUpdateStatus: (handler: (status: UpdateStatus) => void): Promise<UnlistenFn> =>
    listen<UpdateStatus>("updates://status", (e) => handler(e.payload)),
  /** The tray's "Update available" was chosen: show the offer again. */
  onUpdateShow: (handler: () => void): Promise<UnlistenFn> => listen("updates://show", () => handler()),
};
