// Mirrors the serialized shapes of havenkeys-core types. The core is the
// source of truth and re-validates everything sent from here.

export type VaultState = "locked" | "unlocking" | "unlocked" | "locking";

export interface VaultStatus {
  state: VaultState;
  vaultExists: boolean;
  damagedItems: number;
}

export type ItemType = "login" | "secure_note";
export type MatchType = "exact" | "origin" | "domain";

export interface UrlRule {
  url: string;
  matchType: MatchType;
}

/** Item summary. Contains no passwords, TOTP secrets or note bodies. */
export interface ItemOverview {
  id: string;
  itemType: ItemType;
  title: string;
  username: string | null;
  urls: UrlRule[];
  hasPassword: boolean;
  hasTotp: boolean;
  hasNotes: boolean;
  createdAt: number;
  updatedAt: number;
}

/** How an edit treats a secret field. `keep` never requires reading it. */
export type SecretUpdate =
  | { op: "keep" }
  | { op: "set"; value: string }
  | { op: "clear" };

export interface ItemInput {
  itemType: ItemType;
  title: string;
  username?: string | null;
  urls?: UrlRule[];
  password?: SecretUpdate;
  totp?: SecretUpdate;
  notes?: SecretUpdate;
  content?: SecretUpdate;
}

export type SecretField = "password" | "notes" | "content";
export type CopyField = "username" | "password" | "totp";

export interface TotpCode {
  code: string;
  period: number;
  secondsRemaining: number;
}

export interface GeneratorOptions {
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  digits: boolean;
  symbols: boolean;
  avoidAmbiguous: boolean;
}

export interface GeneratedPassword {
  password: string;
  entropyBits: number;
}

export type Theme = "dark" | "light" | "system";

export interface Settings {
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  theme: Theme;
  browserIntegration: boolean;
}

export interface CopyResult {
  clearAfterSeconds: number;
}

export interface ImportReport {
  imported: number;
  logins: number;
  secureNotes: number;
  convertedToNotes: number;
  skippedDuplicates: number;
  skippedArchived: number;
  failed: number;
  attachmentsSkipped: number;
  passwordHistorySkipped: number;
  urlsMovedToNotes: number;
}

export interface ImportResult {
  report: ImportReport;
  fileName: string;
}

export type KeyScheme = "password_only" | "password_and_secret_key";

/** Counts only; never item data. */
export interface SyncReport {
  added: number;
  updated: number;
  deleted: number;
  unreadableDevices: number;
  skippedItems: number;
  headerAdopted: boolean;
  headerRejected: boolean;
}

export interface SyncStatus {
  at: number;
  ok: boolean;
  report: SyncReport | null;
  error: string | null;
}

/** Safe while locked: no secrets. */
export interface DeviceStatus {
  keyScheme: KeyScheme | null;
  /** This device must be given the Secret Key to unlock. */
  needsSecretKey: boolean;
  /** Folder name only. */
  syncFolder: string | null;
  lastSync: SyncStatus | null;
}

export interface EmergencyKit {
  secretKey: string;
  vaultId: string;
  createdAt: number;
  qrSize: number;
  qrModules: boolean[];
}
