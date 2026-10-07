// Mirrors the serialized shapes of havenkeys-core types. The core is the
// source of truth and re-validates everything sent from here.

export type VaultState = "locked" | "unlocking" | "unlocked" | "locking";

export interface VaultStatus {
  state: VaultState;
  vaultExists: boolean;
  damagedItems: number;
  /** Items from the server that could not be opened; retried on every sync. */
  unreadableItems: number;
  /** The saved settings did not open, so automatic behaviors are off until settings are saved again. */
  damagedSettings: boolean;
}

export type ItemType = "login" | "secure_note" | "identity" | "card";
export type MatchType = "exact" | "origin" | "domain";

export interface UrlRule {
  url: string;
  matchType: MatchType;
}

export type SsoProvider = "google" | "microsoft" | "github" | "apple" | "facebook" | "discord" | "x" | "linkedin" | "gitlab";

/** How a login signs in on the provider's own page, in place of a password there. */
export interface SignInWith {
  provider: SsoProvider;
  account: string | null;
}

/** A saved login for a provider's own sign-in page (Rust `SsoAccount`). No secrets. */
export interface SsoAccount {
  id: string;
  title: string;
  username: string;
}

/** Which login a "Sign in with" login's provider login is (Rust `ProviderLogin`). */
export type ProviderLogin = { kind: "one"; id: string } | { kind: "none" } | { kind: "several" };

export type CardBrand =
  | "visa"
  | "mastercard"
  | "amex"
  | "elo"
  | "hipercard"
  | "diners"
  | "discover"
  | "jcb"
  | "unionpay"
  | "maestro"
  | "other";

/** "YYYY-MM" */
export type CardExpiry = string;

/** What the overview keeps of a card: never the number, only its last 4 digits. */
export interface CardSummary {
  brand: CardBrand;
  last4: string | null;
  expiry: CardExpiry | null;
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
  tags: string[];
  signInWith?: SignInWith;
  card?: CardSummary;
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
  | { op: "clear" }
  /** A token from `scan_totp_qr`; TOTP only. */
  | { op: "scanned"; value: string };

export interface ItemInput {
  /** A login's custom fields, whole layout in order. Absent: kept as they are. */
  sections?: SectionInput[];
  itemType: ItemType;
  title: string;
  /** Absent: kept as they are. */
  tags?: string[];
  username?: string | null;
  urls?: UrlRule[];
  password?: SecretUpdate;
  totp?: SecretUpdate;
  notes?: SecretUpdate;
  content?: SecretUpdate;
  autoSignIn?: boolean;
  signInWith?: SignInWith | null;
  /** An identity's values, sent in full on every save. */
  identity?: IdentityFields;
  /** A card's values. */
  card?: CardInput;
}

/** A custom field's kind (spec 2026-09-30-login-custom-fields). */
export type FieldType = "text" | "url" | "email" | "phone" | "date" | "address" | "password" | "otp";
export type AddressPart =
  | "street"
  | "number"
  | "complement"
  | "neighborhood"
  | "city"
  | "state"
  | "postalCode"
  | "country";
export type AddressValue = Partial<Record<AddressPart, string | null>>;

/** A custom field as the core shows it: no Password value, no OTP secret. */
export type FieldView = { id: string; label: string } & (
  | { type: "text" | "url" | "email" | "phone" | "date"; value: string }
  | { type: "address"; parts: AddressValue; formatted: string }
  | { type: "password"; hasValue: boolean }
  | { type: "otp"; hasOtp: boolean }
);
export interface SectionView {
  id: string;
  title: string | null;
  fields: FieldView[];
}

export type FieldValueInput =
  | { type: "text" | "url" | "email" | "phone" | "date"; value: string }
  | { type: "address"; value: AddressValue }
  | { type: "password" | "otp"; value: SecretUpdate };
export interface FieldInput {
  /** Absent: a new field. */
  id?: string;
  label: string;
  value: FieldValueInput;
}
export interface SectionInput {
  id?: string;
  title: string | null;
  fields: FieldInput[];
}

/** Something else the user wants to remember on their identity. */
export interface CustomField {
  label: string;
  value: string;
  /** Masked until revealed, like a password. */
  hidden: boolean;
}

/** The account's one Identity (spec 2026-09-29-identity-item). Every value optional. */
export interface IdentityFields {
  firstName?: string | null;
  middleName?: string | null;
  lastName?: string | null;
  gender?: string | null;
  /** YYYY-MM-DD */
  birthDate?: string | null;
  occupation?: string | null;
  company?: string | null;
  jobTitle?: string | null;
  cpf?: string | null;
  rg?: string | null;
  passport?: string | null;
  driversLicense?: string | null;
  email?: string | null;
  mobilePhone?: string | null;
  homePhone?: string | null;
  workPhone?: string | null;
  street?: string | null;
  number?: string | null;
  complement?: string | null;
  neighborhood?: string | null;
  city?: string | null;
  state?: string | null;
  postalCode?: string | null;
  country?: string | null;
  username?: string | null;
  website?: string | null;
  custom?: CustomField[];
  notes?: string | null;
}

/** An opened identity: its values and the address block Rust formatted. */
export interface IdentityView {
  fields: IdentityFields;
  address: string | null;
}

/** A typed identity value; `address` is the formatted block, `custom:<n>` a custom field. */
export type IdentityCopyField = Exclude<keyof IdentityFields, "custom"> | "address" | `custom:${number}`;

/** A card as the editor sends it; number and code are keep/set/clear. */
export interface CardInput {
  cardholderName?: string | null;
  /** null: detect from the number. */
  brand?: CardBrand | null;
  number?: SecretUpdate;
  verificationNumber?: SecretUpdate;
  expiry?: CardExpiry | null;
  notes?: string | null;
}

/** An opened card, without its number and verification number. */
export interface CardView {
  cardholderName: string | null;
  /** The user's override only; null means detect the brand from the number. */
  brand: CardBrand | null;
  expiry: CardExpiry | null;
  notes: string | null;
  hasNumber: boolean;
  hasVerificationNumber: boolean;
}

export type CardRevealField = "number" | "verificationNumber";
export type CardCopyField = "cardholderName" | "number" | "verificationNumber" | "expiry";

export interface CardNumberCheck {
  brand: CardBrand | null;
  checkDigitOk: boolean;
}

export type SecretField = "password" | "notes" | "content";
export type CopyField = "username" | "password" | "totp";

/** A value of the HavenKeys Account item (spec 2026-09-29-account-item). */
export type AccountField = "email" | "server" | "account_id" | "secret_key";

export interface TotpCode {
  code: string;
  period: number;
  secondsRemaining: number;
}

/** One code a QR scan found. The secret stays in Rust under `token`. */
export interface ScannedTotp {
  token: string;
  issuer: string | null;
  account: string | null;
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
  /** Saved from the generator tab (setGeneratorOptions); updateSettings keeps it. */
  generator: GeneratorOptions;
}

export interface CopyResult {
  clearAfterSeconds: number;
}

export interface ImportReport {
  imported: number;
  logins: number;
  secureNotes: number;
  cards: number;
  convertedToNotes: number;
  skippedDuplicates: number;
  skippedArchived: number;
  failed: number;
  attachmentsSkipped: number;
  passwordHistorySkipped: number;
  urlsMovedToNotes: number;
  fieldsToNotes: number;
  ssoUpgraded: number;
  passkeysSkipped: number;
  skippedExisting: number;
  identities: number;
}

/** Where an export came from; Rust refuses a file that isn't that source's. */
export type ImportSource =
  | "onePassword"
  | "bitwardenJson"
  | "bitwardenCsv"
  | "chrome"
  | "firefox"
  | "keePassXc"
  | "lastPass";

export type ExportFormat = "backup" | "bitwardenJson" | "csv";

export interface ExportSummary {
  logins: number;
  secureNotes: number;
  cards: number;
  identities: number;
  passkeysLeftOut: number;
  passwordHistoryLeftOut: number;
  customFieldsLeftOut: number;
  itemsLeftOut: number;
  unreadable: number;
}

export interface ExportResult {
  fileName: string;
  summary: ExportSummary;
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
  approvedBy?: string | null;
}

export interface PairingCode {
  qrSize: number;
  qrModules: boolean[];
  expiresAt: string;
}

export type PairingState =
  | { state: "waiting" | "denied" | "expired" }
  | { state: "approved"; status: VaultStatus };

/** Counts from a pull. Never item data. */
export interface SyncReport {
  added: number;
  updated: number;
  deleted: number;
  /** Items the server served that did not open under this vault's key. */
  skippedItems: number;
  headerAdopted: boolean;
}

/** Where an update stands (Rust `updates::Phase`). Carries no secrets. */
export type UpdatePhase =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "available"; version: string; notes: string }
  | { phase: "downloading"; version: string; downloaded: number; total: number | null }
  | { phase: "installing" }
  | { phase: "failed"; during: "check" | "install" };

export type UpdateStatus = UpdatePhase & {
  autoCheck: boolean;
  /** False for a .deb/.rpm install: the button opens the release page instead. */
  canInstallInPlace: boolean;
  currentVersion: string;
};

export type HealthCheck = "weak" | "reused" | "old" | "passkey" | "two_factor" | "insecure" | "duplicate";

export interface HealthCounts {
  weak: number;
  reused: number;
  old: number;
  passkey: number;
  twoFactor: number;
  insecure: number;
  duplicate: number;
}

/** IDs and check kinds only: titles and usernames come from the item overviews. */
export interface HealthIssue {
  itemId: string;
  checks: HealthCheck[];
  reusedGroup: number | null;
  duplicateGroup: number | null;
  /** The site's passkey or two-factor setup guide exists ("How to enable"). */
  help: boolean;
}

export interface HealthDismissed {
  itemId: string;
  checks: HealthCheck[];
}

export interface HealthReport {
  computedAt: number;
  counts: HealthCounts;
  issues: HealthIssue[];
  dismissed: HealthDismissed[];
}
