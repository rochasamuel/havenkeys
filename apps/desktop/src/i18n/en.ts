/*
 * Every word the desktop app shows, in English. pt-BR.ts must match this
 * shape exactly (it is typed as Messages), so a string added here and
 * forgotten there fails the typecheck instead of shipping half-translated.
 *
 * Product names (HavenKeys, Secret Key, Emergency Kit, havenkeys-server)
 * stay in English in every locale, as do URLs and keyboard keys. Strings
 * are rendered as React text only; parameterised strings are functions.
 */

/** Every error code the desktop core returns (see errors.ts). */
export type ErrorCode =
  | "locked"
  | "busy"
  | "unlock_failed"
  | "secret_key_required"
  | "decryption"
  | "encryption"
  | "corrupted"
  | "unsupported_version"
  | "vault_exists"
  | "no_vault"
  | "not_found"
  | "denied"
  | "invalid_input"
  | "storage"
  | "kdf"
  | "rng"
  | "offline"
  | "item_changed_elsewhere"
  | "internal"
  | "file"
  | "sign_in_failed"
  | "vault_unreadable"
  | "clipboard"
  | "open_website"
  | "qr_not_found"
  | "qr_not_found_macos"
  | "qr_not_totp"
  | "screen_capture"
  | "scan_expired"
  | "autostart"
  | "unsupported_language"
  | "keychain_unavailable"
  | "password_change_unknown"
  | "password_changed_elsewhere"
  | "signed_out"
  | "rate_limited"
  | "invalid_server_url"
  | "sync_failed"
  | "update_unavailable"
  | "update_failed"
  | "update_settings";

/**
 * The core's errors, by code (havenkeys-core Error::code and the
 * desktop's CmdError codes). The English is Rust's message word for word.
 * `null` shows Rust's own message: it varies (a detail, a folder path),
 * so a fixed text would lose it.
 */
const codes: Record<ErrorCode, string | null> = {
  locked: "The vault is locked.",
  busy: "The vault is busy. Try again in a moment.",
  unlock_failed: "Incorrect master password or damaged vault.",
  secret_key_required: "This vault needs your Secret Key.",
  decryption: "Failed to decrypt vault item.",
  encryption: "Failed to encrypt vault item.",
  corrupted: "The vault file is corrupted.",
  unsupported_version: "This vault was created by an unsupported version.",
  vault_exists: "A vault already exists.",
  no_vault: "No vault exists yet.",
  not_found: "Item not found.",
  denied: "This item is not saved for this website.",
  invalid_input: null,
  storage: "Vault storage error.",
  kdf: "Key derivation failed.",
  rng: "Secure random number generator unavailable.",
  offline: "HavenKeys is offline — the vault is read-only until it reconnects.",
  item_changed_elsewhere: "This item changed on another device.",
  internal: null,
  file: "Could not read or delete the file.",
  sign_in_failed: "Email, master password or Secret Key is incorrect.",
  vault_unreadable: null,
  clipboard: "Could not access the clipboard.",
  open_website: "Could not open the website.",
  qr_not_found: "No QR code found. Make sure it's fully visible on screen.",
  qr_not_found_macos:
    "No QR code found. Make sure it's fully visible on screen. If this is the first scan, allow HavenKeys in System Settings → Privacy & Security → Screen Recording.",
  qr_not_totp: "The QR code isn't a one-time code setup.",
  screen_capture: "Could not capture the screen.",
  scan_expired: "The scanned code expired. Scan it again.",
  autostart: "Could not change whether HavenKeys opens at login.",
  update_unavailable: "There is no update to install.",
  update_failed: "The update could not be installed. Try again later.",
  update_settings: "Could not save the update setting.",
  unsupported_language: "Unsupported language.",
  keychain_unavailable:
    "Your system keychain did not answer. Approve its prompt if one is showing, then try again.",
  password_change_unknown:
    "HavenKeys could not confirm whether the server applied the new master password. If your current password stops working, use the new one.",
  password_changed_elsewhere:
    "Your master password was changed on another device. Lock and unlock with the new password.",
  signed_out: "HavenKeys is signed out of this account. Unlock again to reconnect.",
  rate_limited: "Too many attempts. Try again in a few minutes.",
  invalid_server_url: "That server address cannot be used. It must start with https://.",
  sync_failed: null,
};

/**
 * "Invalid input: <detail>." rewritten per Rust detail text. English needs
 * none: Rust's message is already English.
 */
const noDetails: Record<string, string> = {};

export const en = {
  common: {
    cancel: "Cancel",
    save: "Save",
    saving: "Saving…",
    edit: "Edit",
    delete: "Delete",
    keep: "Keep",
    continue: "Continue",
    dismiss: "Dismiss",
    undo: "Undo",
    remove: "Remove",
    login: "Login",
    secureNote: "Secure note",
    masterPassword: "Master password",
    secretKey: "Secret Key",
    email: "Email",
    server: "Server",
    username: "Username",
    password: "Password",
    hiddenPassword: "Hidden password",
    showPassword: "Show password",
    hidePassword: "Hide password",
    decrypting: "Decrypting…",
    loading: "Loading…",
    couldNotCopy: "Could not copy.",
    couldNotReveal: "Could not reveal.",
    offlineReadOnly: "Offline — the vault is read-only until it reconnects.",
  },

  app: {
    fatalTitle: "HavenKeys cannot open this vault",
    fatalFallback: "HavenKeys could not reach its vault storage. Restart the app.",
    kitTitle: "Save your Emergency Kit",
    kitBody:
      "This is the only copy of your Secret Key. Without it — and your master password — nobody can open this vault, including us.",
    /** Rust's KEYCHAIN_NOT_CLEARED (account.rs), word for word. */
    keychainNotCleared:
      "This computer was removed, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry “app.havenkeys” yourself.",
    signedOut:
      "The server did not accept this computer's sign-in. If your master password was changed on another device, lock and unlock with the new one.",
  },

  welcome: {
    title: "Welcome to HavenKeys",
    subInvite: "Set up this computer with your invite.",
    subSignIn: "Add this computer to an account you already have.",
    howToSetUp: "How to set up",
    tabInvite: "I have an invite",
    tabSignIn: "I already have an account",
    invite: "Invite",
    inviteHint: "One line, from whoever runs your HavenKeys server. It works once and expires after seven days.",
    passwordHint: "At least 10 characters. Nobody can reset it for you — not the server, not us.",
    repeatPassword: "Repeat master password",
    tooShort: "Use at least 10 characters.",
    mismatch: "The two passwords don’t match.",
    create: "Create my vault",
    creating: "Setting up…",
    createFailed: "Could not set up this vault.",
    createNote: "Your master password and Secret Key never leave this computer. The server stores only encrypted data.",
    serverHint: "From your Emergency Kit.",
    emailPlaceholder: "you@example.com",
    secretKeyHint: "The long code on your Emergency Kit. Leave it empty if this computer already has it.",
    signIn: "Sign in",
    signingIn: "Signing in…",
    signInFailed: "Could not sign in.",
    signInNote:
      "Signing in downloads your vault and decrypts it here. The server never sees your password or your Secret Key.",
  },

  unlock: {
    /** "HavenKeys is <em>locked</em>." */
    titleBefore: "HavenKeys is ",
    titleLocked: "locked",
    titleAfter: ".",
    reasons: {
      idle: "Locked after a period of inactivity.",
      suspend: "Locked because the computer went to sleep.",
      screen_lock: "Locked because the screen was locked.",
      user: "Locked.",
      extension: "Locked from the browser extension.",
    } as Record<string, string>,
    enterPassword: "Enter your master password to unlock.",
    enterPasswordAndKey:
      "Enter your master password and the Secret Key from your Emergency Kit. This computer remembers the Secret Key after you unlock.",
    unlock: "Unlock",
    secretKeyPlaceholder: "Secret Key  H1-XXXX-XXXX-…",
    useKitInstead: "Enter the Secret Key from my Emergency Kit instead",
    failed: "Could not open the vault.",
    hint: "Your vault is decrypted on this computer only.",
  },

  vault: {
    allItems: "All items",
    logins: "Logins",
    secureNotes: "Secure notes",
    search: "Search",
    searchLabel: "Search vault",
    sectionsLabel: "Vault sections",
    vaultHeading: "Vault",
    toolsHeading: "Tools",
    generator: "Password generator",
    settings: "Settings",
    offline: "Offline, read-only",
    connected: "Connected",
    lockTitle: (shortcut: string) => `Lock now (${shortcut})`,
    unlocked: "Unlocked",
    lockNow: "Lock now",
    damaged: (n: number) =>
      n === 1 ? "1 item could not be decrypted and is hidden." : `${n} items could not be decrypted and are hidden.`,
    unreadable: (n: number) =>
      n === 1 ? "1 item couldn't be read from the server." : `${n} items couldn't be read from the server.`,
    redownload: "Re-download",
    redownloadFailed: "Could not re-download the vault.",
    saved: "Saved.",
    deleted: (title: string) => `Deleted “${title}”.`,
    deleteFailed: "Could not delete the item.",
    details: "Item details",
    emptyTitle: "Your vault is empty",
    emptyBody: "Add a login or a note, or bring everything over from 1Password.",
    nothingSelected: "Nothing selected",
    chooseItem: "Choose an item to see its details.",
    addLogin: "Add a login",
    addNote: "Add a secure note",
    import1Password: "Import from 1Password",
    discardChanges: (title: string) => `Discard your changes to “${title}”?`,
    discardNewItem: "Discard the new item?",
    keepEditing: "Keep editing",
    discard: "Discard",
  },

  list: {
    items: "Items",
    results: "Results",
    newItem: "New item",
    noMatches: "No matches",
    nothingYet: "Nothing here yet",
    searchHint: "Search looks at titles, usernames and websites.",
    emptyHint: "New items you add appear here.",
    hasTotp: "Has one-time codes",
    hasPasskeys: "Has passkeys",
  },

  detail: {
    /** By copied field; each language agrees the word with its own noun. */
    copied: {
      username: (seconds: number) => `Username copied. The clipboard clears in ${seconds}\u00a0s.`,
      password: (seconds: number) => `Password copied. The clipboard clears in ${seconds}\u00a0s.`,
      totp: (seconds: number) => `One-time code copied. The clipboard clears in ${seconds}\u00a0s.`,
    },
    oneTimeCode: "One-time code",
    copyOneTimeCode: "Copy one-time code",
    codeFailed: "Could not generate a code.",
    secondsRemaining: (n: number) => `${n} seconds remaining`,
    replaced: (date: string) => `Replaced ${date}`,
    showPrevious: "Show previous password",
    hidePrevious: "Hide previous password",
    passwordHistory: "Password history",
    noPrevious: "No previous passwords",
    previousPasswords: "Previous passwords",
    historyFailed: "Could not load the history.",
    passkeys: "Passkeys",
    passkeysFailed: "Could not load passkeys.",
    passkeyDeleted: "Passkey deleted.",
    passkeyDeleteFailed: "Could not delete the passkey.",
    noAccountName: "No account name",
    passkeySaved: (date: string) => `Saved ${date}`,
    loseAccess: (site: string) => `You may lose access to ${site}.`,
    deletePasskey: "Delete passkey",
    noteFailed: "Could not open the note.",
    copyUsername: "Copy username",
    copyPassword: "Copy password",
    website: "Website",
    openWebsite: "Open in browser",
    openWebsiteFailed: "Could not open the website.",
    matchDomain: "Whole site",
    matchOrigin: "Exact site",
    matchExact: "Exact page",
    notes: "Notes",
    showNotes: "Show notes",
    hideNotes: "Hide notes",
    hidden: "Hidden",
    dates: (created: string, changed: string) => `Created ${created} · Changed ${changed}`,
    confirmDelete: (title: string) => `Delete “${title}” permanently?`,
    passkeyWarning: " Its passkeys go with it, and you may lose access to those sites.",
  },

  editor: {
    newLogin: "New login",
    newNote: "New secure note",
    editLogin: "Edit login",
    editNote: "Edit secure note",
    title: "Title",
    titlePlaceholderLogin: "e.g. GitHub",
    titlePlaceholderNote: "e.g. Wi-Fi at home",
    usernamePlaceholder: "Username or email",
    change: "Change",
    passwordRemoved: "The password will be removed.",
    generate: "Generate",
    generateFailed: "Could not generate a password.",
    websites: "Websites",
    websiteN: (n: number) => `Website ${n}`,
    matchHow: (n: number) => `How website ${n} is matched`,
    removeWebsiteN: (n: number) => `Remove website ${n}`,
    removeWebsite: "Remove website",
    addWebsite: "Add website",
    matchDomain: "Whole site, any subdomain",
    matchOrigin: "This exact site",
    matchExact: "This exact page",
    oneTimeCodes: "One-time codes",
    setUp: "Set up.",
    replace: "Replace",
    codesRemoved: "One-time codes will be removed.",
    totpPlaceholder: "Setup key or otpauth:// link",
    totpLabel: "One-time code setup key",
    scanQr: "Scan QR code (clipboard or screen)",
    scanning: "Looking for a QR code…",
    scanned: (label: string) => `${label} (scanned)`,
    scanPick: "Several QR codes found. Choose one:",
    scanUnnamed: "Unnamed",
    scanFailed: "Could not scan for a QR code.",
    browser: "Browser",
    autoSignIn: "Sign in automatically on this site",
    offInSettings: "Turned off in Settings.",
    notes: "Notes",
    note: "Note",
    notesPlaceholder: "Anything else worth keeping with this login",
    loadTextFailed: "Could not load the existing text.",
    saveFailed: "Could not save the item.",
  },

  generator: {
    title: "Password generator",
    lede: "Made on this computer from the operating system’s secure random source. Nothing is saved until you use it.",
    strength: {
      weak: "Weak",
      fair: "Fair",
      strong: "Strong",
      excellent: "Excellent",
    },
    aboutBits: (n: number) => `about ${n} bits`,
    regenerate: "Regenerate",
    copy: "Copy",
    copied: (seconds: number) => `Password copied. The clipboard clears in ${seconds}\u00a0s.`,
    failed: "Could not generate a password.",
    length: "Length",
    characters: "Characters",
    uppercase: "Uppercase",
    lowercase: "Lowercase",
    digits: "Numbers",
    symbols: "Symbols",
    avoidAmbiguous: "Avoid look-alikes",
  },

  kit: {
    qrLabel: "Secret Key QR code",
    loadFailed: "Could not load the Emergency Kit.",
    preparing: "Preparing your Emergency Kit…",
    sheetLabel: "Emergency Kit",
    heading: "HavenKeys Emergency Kit",
    created: (date: string) => `Created ${date}`,
    /** "…you need <strong>both</strong> your master password…" */
    ledeBefore: "To open your vault on a new computer or phone you need ",
    ledeBoth: "both",
    ledeAfter:
      " your master password and this Secret Key, plus the email and server below. Print this page or save it somewhere safe and offline. Anyone who has the Secret Key and your master password can open your vault.",
    blankLine: "Blank line to write your master password, if you choose",
    accountId: "Account ID",
    qrCaption: "Scan to set up another device: the account, the address, the server and the Secret Key.",
    foot: "HavenKeys cannot recover your master password or Secret Key. If you lose this kit, you can view it again on any device where your vault is unlocked (Settings → Account).",
    print: "Print or save as PDF",
    savedCheck: "I have saved my Emergency Kit",
  },

  settings: {
    title: "Settings",
    saved: "Settings saved.",
    loadFailed: "Could not load settings.",
    saveFailed: "Could not save settings.",
    appearance: "Appearance",
    language: "Language",
    languageAuto: "Automatic",
    theme: "Theme",
    themes: { dark: "Dark", light: "Light", system: "System" },
    security: "Security",
    autoLock: "Lock automatically",
    autoLockAfter: (minutes: number) => (minutes === 60 ? "After 1 hour" : `After ${minutes} minutes`),
    never: "Never",
    clipboardClear: "Clear copied items",
    afterSeconds: (seconds: number) => `After ${seconds} seconds`,
    securityNote:
      "The vault also locks when the computer sleeps and when you quit HavenKeys, and — on Windows and Linux — when the screen locks. Closing the window keeps HavenKeys in the tray, where the timeout above keeps running.",
    startup: "Startup",
    launchAtLogin: "Open HavenKeys when you log in",
    launchAtLoginLabel: "Open HavenKeys when you log in to this computer",
    startupNote:
      "HavenKeys starts locked, in the tray, so the browser extension can reach it. This applies to this computer only.",
    browserExtension: "Browser extension",
    browserIntegration: "Suggest and fill logins in the browser",
    browserIntegrationLabel: "Allow the HavenKeys browser extension to suggest and fill logins",
    autoPasskey: "Add passkeys automatically after I sign in",
    autoSignIn: "Sign in automatically after filling",
    extensionNote:
      "The extension only receives a login when you choose it on a website that login is saved for, and only while HavenKeys is unlocked. It never receives your master password. Off by default: while it is on, other programs running under your account can make the same requests as the extension.",
    passkeyNote:
      "When a website offers to add a passkey right after HavenKeys fills your password there, HavenKeys saves it to that login. When off, HavenKeys asks first.",
    autoSignInNote:
      "After you choose a login, HavenKeys presses the sign-in button and continues through email, password and two-factor steps on the same site. Sign-ins that span several pages need in-page suggestions. Each login can turn this off.",
    about: "About",
    aboutText: (version: string) =>
      `HavenKeys ${version}. Your vault is encrypted with AES-256-GCM under a key derived from your master password (with Argon2id) and your Secret Key. It stays on this computer unless you turn on sync. This software has not undergone an independent security audit.`,
  },

  changePassword: {
    title: "Master password",
    current: "Current",
    new: "New",
    confirm: "Confirm new",
    note: "Your items are not re-encrypted; only the key that protects them changes. Other computers signed in to this account will be signed out.",
    mismatch: "The new passwords don’t match.",
    submit: "Change master password",
    submitting: "Changing…",
    done: "Master password changed.",
    failed: "Could not change the master password.",
  },

  account: {
    title: "Account",
    never: "never",
    unknown: "unknown",
    justNow: "just now",
    minutesAgo: (n: number) => `${n} min ago`,
    hoursAgo: (n: number) => `${n} h ago`,
    daysAgo: (n: number) => `${n} days ago`,
    devicesFailed: "Could not read the device list.",
    thisSignedOut: "This computer was signed out.",
    deviceSignedOut: (name: string) => `${name} was signed out.`,
    revokeFailed: "Could not revoke that device.",
    devicesOffline: "Your devices are listed when this computer is connected to the server.",
    thisComputer: "This computer",
    lastSeen: (when: string) => `Last seen ${when}`,
    confirmSignOutThis: "Sign this computer out?",
    confirmSignOutOther: "Sign it out and end its session?",
    revoke: "Revoke",
    upToDate: "Already up to date.",
    synced: (n: number) => (n === 1 ? "Synced: 1 item updated." : `Synced: ${n} items updated.`),
    skipped: (n: number) =>
      n === 1
        ? "1 item from the server could not be read and was left alone."
        : `${n} items from the server could not be read and were left alone.`,
    syncFailed: "Could not sync.",
    redownloaded: (n: number) =>
      n === 1 ? "Re-downloaded the vault: 1 item." : `Re-downloaded the vault: ${n} items.`,
    stillSkipped: (n: number) =>
      n === 1 ? "1 item still could not be read." : `${n} items still could not be read.`,
    redownloadFailed: "Could not re-download the vault.",
    signOutFailed: "Could not sign out.",
    removeFailed: "Could not remove this device.",
    status: "Status",
    connected: "Connected",
    offline: "Offline — read-only",
    lastSync: "Last sync",
    notLinked: "This vault is not linked to an account.",
    syncNow: "Sync now",
    syncing: "Syncing…",
    redownloadAll: "Re-download everything",
    signOut: "Sign out and lock",
    devices: "Devices",
    kitTitle: "Emergency Kit",
    keyInFile: "Your Secret Key is stored in a file on this computer because no system keychain is available.",
    kitTeaser:
      "Your Secret Key, the account and the server — everything another computer needs, besides your master password.",
    showKit: "Show Emergency Kit",
    removeTitle: "Remove this device",
    removeNote:
      "Takes this computer off the account and returns HavenKeys to its first-run screen, where you can sign in to any account or server. Your vault stays on the server. A copy of the encrypted file is kept as vault.sqlite3.removed-… in the app's data folder; opening it later needs your master password and the Secret Key from your Emergency Kit.",
    typeToConfirm: (email: string) => `Type ${email} to confirm`,
    remove: "Remove this device",
  },

  import: {
    title: "Import from 1Password",
    note: "In 1Password, choose File › Export and the 1PUX format, then pick that file here. The export contains all of your passwords unencrypted, so delete it once the import is done.",
    choose: "Choose .1pux file…",
    importing: "Importing…",
    failed: "Import failed.",
    fileDeleted: "Export file deleted.",
    deleteFailed: "Could not delete the file.",
    /** "<strong>Imported 3 items</strong> from file: 2 logins and 1 secure note." */
    imported: (n: number) => (n === 1 ? "Imported 1 item" : `Imported ${n} items`),
    summary: (fileName: string, logins: number, notes: number) =>
      ` from ${fileName}: ${logins === 1 ? "1 login" : `${logins} logins`} and ${notes === 1 ? "1 secure note" : `${notes} secure notes`}.`,
    convertedToNotes: (n: number) =>
      `${n} ${n === 1 ? "item" : "items"} of other kinds (cards, identities, keys…) became secure notes with all their fields.`,
    skippedDuplicates: (n: number) => `${n} ${n === 1 ? "item was" : "items were"} already in your vault and skipped.`,
    skippedArchived: (n: number) => `${n} ${n === 1 ? "archived item was" : "archived items were"} left out.`,
    attachmentsSkipped: (n: number) =>
      `${n} ${n === 1 ? "file attachment was" : "file attachments were"} left out (not supported yet).`,
    passwordHistorySkipped: (n: number) =>
      `${n} ${n === 1 ? "old password" : "old passwords"} from password history ${n === 1 ? "was" : "were"} left out.`,
    urlsMovedToNotes: (n: number) =>
      `${n} ${n === 1 ? "website entry wasn’t" : "website entries weren’t"} a web address and ${n === 1 ? "was" : "were"} kept in the item’s notes.`,
    failedItems: (n: number) =>
      `${n} ${n === 1 ? "item" : "items"} couldn’t be imported (a field was over the size limits).`,
    wasDeleted: "The export file was deleted.",
    confirmDelete: (fileName: string) => `Delete ${fileName}?`,
    deleteFile: "Delete file",
    keepFile: "Keep it",
    deleteExport: "Delete the export file",
  },

  errors: {
    /** Anything that is not an error from the core. */
    generic: "Something went wrong. Try again.",
    codes,
    /**
     * Full sentences for "Invalid input: <detail>.", keyed by Rust's detail
     * text. Only details a person can cause from the UI; any other detail
     * shows Rust's message.
     */
    invalidInput: noDetails,
  },
};

export type Messages = typeof en;
