// Mirrors the serialized shapes of havenkeys-core types. The core is the
// source of truth and re-validates everything sent from here.

export type VaultState = "locked" | "unlocking" | "unlocked" | "locking";

export interface VaultStatus {
  state: VaultState;
  vaultExists: boolean;
  damagedItems: number;
  /** Items from the server that could not be opened; retried on every sync. */
  unreadableItems: number;
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
  hasPasskey: boolean;
  autoSignIn: boolean;
  createdAt: number;
  updatedAt: number;
}

/** A passkey saved on a login. Public details only. */
export interface PasskeyInfo {
  credentialId: string;
  rpId: string;
  userName: string;
  displayName: string | null;
  createdAt: number;
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
  autoSignIn?: boolean;
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
  autoPasskeyUpgrade: boolean;
  autoSignIn: boolean;
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

export type KeyScheme = "password_only" | "password_and_secret_key" | "account_bound";

/** Safe while locked: no secrets. */
export interface DeviceStatus {
  keyScheme: KeyScheme | null;
  /** This device must be given the Secret Key to unlock. */
  needsSecretKey: boolean;
  /** Whether this device currently has a server session. Independent of the lock state. */
  online: boolean;
  /** Where the Secret Key is kept: the OS keychain, `device.json` (no keychain answered), or nowhere yet. */
  secretKeyStorage: "keychain" | "file" | "none";
}

export interface EmergencyKit {
  secretKey: string;
  vaultId: string;
  accountId: string;
  email: string;
  serverUrl: string;
  createdAt: number;
  qrSize: number;
  qrModules: boolean[];
}

/** The account this vault belongs to. No secrets; readable while locked. */
export interface AccountStatus {
  email: string;
  serverUrl: string;
  accountId: string;
  online: boolean;
  /** Unix ms of the last successful pull, or null if none yet. */
  lastSyncedAt: number | null;
}

/** A device signed in to the account. Timestamps are RFC 3339 from the server. */
export interface DeviceEntry {
  id: string;
  name: string;
  createdAt: string;
  lastSeenAt: string | null;
  current: boolean;
}

/** Counts from a pull. Never item data. */
export interface SyncReport {
  added: number;
  updated: number;
  deleted: number;
  /** Items the server served that did not open under this vault's key. */
  skippedItems: number;
  headerAdopted: boolean;
}
