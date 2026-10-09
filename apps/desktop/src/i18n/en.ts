/*
 * Every word the desktop app shows, in English. pt-BR.ts must match this
 * shape exactly (it is typed as Messages), so a string added here and
 * forgotten there fails the typecheck instead of shipping half-translated.
 *
 * Product names (HavenKeys, Secret Key, Recovery Sheet, havenkeys-server)
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
  | "bundle_refused"
  | "internal"
  | "file"
  | "sign_in_failed"
  | "pairing_gone"
  | "pairing_other_server"
  | "pairing_failed"
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
  | "invalid_kit"
  | "signed_out"
  | "account_deleted"
  | "rate_limited"
  | "account_frozen"
  | "account_frozen_new_device"
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
  bundle_refused: "Enter your master password to unlock.",
  internal: null,
  file: "Could not read or delete the file.",
  sign_in_failed: "Email, master password or Secret Key is incorrect.",
  pairing_gone: "This code has expired. Ask the new device for a new one.",
  pairing_other_server: "This code is for another server.",
  pairing_failed: "The sign-in could not be completed. Ask for a new code.",
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
  invalid_kit: "That is not a HavenKeys Recovery Sheet code.",
  signed_out: "HavenKeys is signed out of this account. Unlock again to reconnect.",
  account_deleted: "This account was deleted.",
  rate_limited: "Too many attempts. Try again in a few minutes.",
  account_frozen: "This account is frozen: the trial ended or payment lapsed. The vault is read-only.",
  account_frozen_new_device:
    "This account is read-only (the trial ended or payment lapsed) and cannot add a new device. Sign in from a device that already has it, or subscribe.",
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
    kitTitle: "Save your Recovery Sheet",
    kitBody:
      "This is the only copy of your Secret Key. Without it — and your master password — nobody can open this vault, including us.",
    /** Rust's KEYCHAIN_NOT_CLEARED (account.rs), word for word. */
    keychainNotCleared:
      "This computer was removed, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry “app.havenkeys” yourself.",
    signedOut:
      "The server did not accept this computer's sign-in. If your master password was changed on another device, lock and unlock with the new one.",
    /** After signing in with the phone: which account the phone approved this computer into. */
    pairedAs: (email: string) => `Signed in as ${email}. If that is not your account, remove this computer in Settings → Account.`,
  },

  welcome: {
    title: "Welcome to HavenKeys",
    createAccount: "Create account",
    createAccountNote: "Opens havenkeys.net in your browser. You come back here with a setup code.",
    creatingFor: (email: string, server: string) => `Creating an account for ${email} on ${server}.`,
    subInvite: "Set up this computer with the code from your email.",
    subSignIn: "Add this computer to an account you already have.",
    howToSetUp: "How to set up",
    tabInvite: "I have a setup code",
    tabSignIn: "I have an account",
    invite: "Setup code",
    inviteHint: "One line, from your sign-up email or from whoever runs your server. It works once.",
    passwordHint: "At least 10 characters. Nobody can reset it for you — not the server, not us.",
    repeatPassword: "Repeat master password",
    tooShort: "Use at least 10 characters.",
    mismatch: "The two passwords don’t match.",
    create: "Create my vault",
    creating: "Setting up…",
    createFailed: "Could not set up this vault.",
    createNote: "Your master password and Secret Key never leave this computer. The server stores only encrypted data.",
    serverHint: "From your Recovery Sheet.",
    tabPhone: "Use my phone",
    subPhone: "Approve this computer from HavenKeys on your phone.",
    phoneServer: "Your server",
    phoneShowCode: "Show code",
    phoneStarting: "Preparing…",
    phoneScan: "On your phone, open HavenKeys → Settings → Sign in a new device, and scan this code.",
    phoneQrLabel: "Sign-in code for your phone",
    phoneExpiresIn: (s: number) => `Expires in ${s}s`,
    phoneExpired: "This code has expired.",
    phoneNewCode: "New code",
    phoneDenied: "The sign-in was denied on your phone.",
    phoneUseKit: "Use the Recovery Sheet instead",
    emailPlaceholder: "you@example.com",
    secretKeyHint: "The long code on your Recovery Sheet. Leave it empty if this computer already has it.",
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
      "Enter your master password and the Secret Key from your Recovery Sheet. This computer remembers the Secret Key after you unlock.",
    unlock: "Unlock",
    secretKeyPlaceholder: "Secret Key  H1-XXXX-XXXX-…",
    useKitInstead: "Enter the Secret Key from my Recovery Sheet instead",
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
    tagsHeading: "Tags",
    toolsHeading: "Tools",
    health: "Vault health",
    generator: "Password generator",
    settings: "Settings",
    offline: "Offline, read-only",
    connected: "Connected",
    trialDays: (n: number) => (n === 1 ? "Trial: 1 day left" : `Trial: ${n} days left`),
    readOnlyFrozen: "Read-only (trial ended)",
    frozenTitle: "Your trial has ended.",
    frozenBody: "The vault is read-only. You can still open, copy, export and sign in with passkeys.",
    subscribe: "Subscribe",
    frozenImport: "Importing needs an active plan.",
    frozenPassword: "Changing the master password needs an active plan.",
    lockTitle: (shortcut: string) => `Lock now (${shortcut})`,
    unlocked: "Unlocked",
    lockNow: "Lock now",
    damaged: (n: number) =>
      n === 1 ? "1 item could not be decrypted and is hidden." : `${n} items could not be decrypted and are hidden.`,
    damagedSettings:
      "Your settings could not be read, so automatic sign-in, passkey upgrade and the browser extension are off. Review them in Settings.",
    unreadable: (n: number) =>
      n === 1 ? "1 item couldn't be read from the server." : `${n} items couldn't be read from the server.`,
    redownload: "Re-download",
    redownloadFailed: "Could not re-download the vault.",
    saved: "Saved.",
    movedToTrash: (title: string) => `Moved “${title}” to Trash.`,
    deleted: (title: string) => `Deleted “${title}”.`,
    restored: (title: string) => `Restored “${title}”.`,
    trash: "Trash",
    deleteFailed: "Could not delete the item.",
    details: "Item details",
    emptyTitle: "Your vault is empty",
    emptyBody: "Add a login or a note, or bring everything over from another password manager.",
    nothingSelected: "Nothing selected",
    chooseItem: "Choose an item to see its details.",
    addLogin: "Add a login",
    addNote: "Add a secure note",
    importOther: "Import from another app",
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
    tags: "Tags",
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
    signInWith: "Sign in with",
    openProviderLogin: (provider: string) => `Open the ${provider} login`,
    noProviderLogin: (provider: string) => `No saved ${provider} login for this account`,
    severalProviderLogins: (provider: string) => `Several saved ${provider} logins match this account`,
  },

  editor: {
    newLogin: "New login",
    newNote: "New secure note",
    editLogin: "Edit login",
    editNote: "Edit secure note",
    title: "Title",
    titlePlaceholderLogin: "e.g. GitHub",
    titlePlaceholderNote: "e.g. Wi-Fi at home",
    signInWith: "Sign in with",
    providerNone: "Nothing (password only)",
    providerPick: "How you sign in",
    providerSearch: "Search providers and logins",
    noMatches: "No matches",
    account: "Account",
    accountPlaceholder: "Email used there (optional)",
    alsoPassword: "Also has a password",
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
    tags: "Tags",
    addTag: "Add tag",
    removeTag: (tag: string) => `Remove tag ${tag}`,
    createTag: (tag: string) => `Create “${tag}”`,
    tagLimit: "20 tags is the limit.",
    tagSuggestions: "Tag suggestions",
    tagTooLong: "Tag is too long. 32 characters is the limit.",
    tagNotAllowed: "Tag contains a character that is not allowed.",
  },

  fields: {
    types: {
      text: "Text",
      url: "URL",
      email: "Email",
      address: "Address",
      date: "Date",
      otp: "One-time password",
      password: "Password",
      phone: "Phone",
    },
    addField: "Add another field",
    addSection: "Add section",
    sectionTitle: "Section title",
    untitled: "Untitled section",
    label: "Label",
    removeField: (label: string) => `Remove ${label}`,
    removeSection: "Remove section",
    confirmRemoveSection: (n: number) =>
      n === 1 ? "Remove this section and its field?" : `Remove this section and its ${n} fields?`,
    move: (label: string) => `Move ${label} (Alt+Up / Alt+Down)`,
    moveSectionUp: "Move section up",
    moveSectionDown: "Move section down",
    copy: (label: string) => `Copy ${label}`,
    copied: (label: string, seconds: number) => `${label} copied. The clipboard clears in ${seconds}\u00a0s.`,
    show: (label: string) => `Show ${label}`,
    hide: (label: string) => `Hide ${label}`,
    loadFailed: "Could not load this login's other fields.",
    limit: "This login has as many fields as it can hold.",
    set: "Saved.",
    removed: "Will be removed.",
    emptySection: "No fields. Drag one here.",
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
    saveFailed: "Could not save the generator settings.",
    length: "Length",
    characters: "Characters",
    uppercase: "Uppercase",
    lowercase: "Lowercase",
    digits: "Numbers",
    symbols: "Symbols",
    avoidAmbiguous: "Avoid look-alikes",
  },

  trash: {
    title: "Trash",
    explain: "Items here are removed for good after 30 days.",
    empty: "Trash is empty.",
    deletedAgo: (days: number) => (days === 0 ? "Deleted today" : days === 1 ? "Deleted yesterday" : `Deleted ${days} days ago`),
    removedIn: (days: number) => (days <= 0 ? "Removed at next sync" : days === 1 ? "Removed in 1 day" : `Removed in ${days} days`),
    restore: "Restore",
    deleteForever: "Delete permanently",
    confirmDelete: (title: string) => `Delete “${title}” permanently? This cannot be undone.`,
    emptyTrash: "Empty Trash",
    confirmEmpty: (n: number) => (n === 1 ? "Delete 1 item permanently? This cannot be undone." : `Delete ${n} items permanently? This cannot be undone.`),
    passkeyWarning: "Its passkeys go too. Sites that use them will need another way to sign in.",
    restoreFailed: "Could not restore the item.",
    emptyFailed: "Could not empty the Trash.",
    loadFailed: "Could not load the Trash.",
    restoreFirst: "Restore this item to see or use its passwords and codes.",
  },

  health: {
    title: "Vault health",
    intro: "Checked on this device with the vault unlocked. Nothing is sent anywhere.",
    loading: "Checking your logins…",
    empty: "No issues found.",
    emptyFiltered: "Nothing here.",
    all: "All issues",
    dismissedFilter: "Dismissed",
    showItems: "Show items",
    open: "Open",
    changePassword: "Change password",
    howToEnable: "How to enable",
    noHelp: "This site doesn't publish a setup guide.",
    dismiss: "Dismiss",
    undo: "Undo",
    dismissFailed: "Couldn't save that change.",
    loadFailed: "Couldn't check the vault.",
    cards: {
      reused: { title: "Reused passwords", body: "The same password on several logins. If one site leaks it, the others are exposed." },
      weak: { title: "Weak passwords", body: "Easy to guess. Generate a strong password instead." },
      insecure: { title: "Unsecured websites", body: "Saved with an http:// address. Change it to https:// if the site supports it." },
      duplicate: { title: "Duplicates", body: "Logins with the same name, username and websites. Delete the extra ones." },
      passkey: { title: "Passkeys available", body: "These sites accept passkeys, which can't be phished or reused." },
      two_factor: { title: "Two-factor authentication", body: "These sites offer one-time codes you haven't set up yet." },
      old: { title: "Old passwords", body: "Not changed in over a year." },
    },
    chip: {
      weak: "Weak password",
      reused: (n: number) => `Used in ${n} logins`,
      old: "Not changed in over a year",
      passkey: "Supports passkeys",
      two_factor: "Supports two-factor codes",
      insecure: "Uses http",
      duplicate: (others: number) => (others === 1 ? "Duplicate of 1 other login" : `Duplicate of ${others} other logins`),
    },
  },

  accountItem: {
    title: "HavenKeys Account",
    kind: "Your HavenKeys account",
    accountId: "Account ID",
    secretKeyHeading: "Secret Key",
    showSecretKey: "Show Secret Key",
    hideSecretKey: "Hide Secret Key",
    hiddenSecretKey: "Hidden Secret Key",
    copy: {
      email: "Copy email",
      server: "Copy server",
      account_id: "Copy account ID",
      secret_key: "Copy Secret Key",
    },
    /** By copied field; each language agrees the word with its own noun. */
    copied: {
      email: (seconds: number) => `Email copied. The clipboard clears in ${seconds}\u00a0s.`,
      server: (seconds: number) => `Server copied. The clipboard clears in ${seconds}\u00a0s.`,
      account_id: (seconds: number) => `Account ID copied. The clipboard clears in ${seconds}\u00a0s.`,
      secret_key: (seconds: number) => `Secret Key copied. The clipboard clears in ${seconds}\u00a0s.`,
    },
    notOnThisComputer: "The Secret Key is not on this computer.",
    note: "Use these with your master password to sign in to HavenKeys on another computer.",
    showKit: "Show Recovery Sheet",
    readOnly: "Built from your account. It cannot be edited or deleted.",
  },

  identity: {
    title: "Identity",
    nav: "Identity",
    kind: "Identity",
    edit: "Edit identity",
    sections: {
      identification: "Identification",
      documents: "Documents",
      contact: "Contact",
      address: "Address",
      internet: "Internet",
      custom: "Other",
      notes: "Notes",
    },
    fields: {
      firstName: "First name",
      middleName: "Middle name",
      lastName: "Last name",
      gender: "Gender",
      birthDate: "Date of birth",
      occupation: "Occupation",
      company: "Company",
      jobTitle: "Job title",
      cpf: "CPF",
      rg: "RG",
      passport: "Passport",
      driversLicense: "Driver's license",
      email: "Email",
      mobilePhone: "Mobile phone",
      homePhone: "Home phone",
      workPhone: "Work phone",
      street: "Street",
      number: "Number",
      complement: "Complement",
      neighborhood: "Neighborhood",
      city: "City",
      state: "State",
      postalCode: "Postal code",
      country: "Country",
      username: "Username",
      website: "Website",
    },
    fullAddress: "Full address",
    copyAddress: "Copy address",
    copy: (label: string) => `Copy ${label}`,
    show: (label: string) => `Show ${label}`,
    hide: (label: string) => `Hide ${label}`,
    hidden: (label: string) => `Hidden ${label}`,
    copied: (label: string, seconds: number) => `${label} copied. The clipboard clears in ${seconds}\u00a0s.`,
    emptyTitle: "Your identity is empty",
    emptyBody: "Add your name, documents, address and anything else you want at hand when filling in forms.",
    fillIn: "Fill in your identity",
    loadFailed: "Could not open your identity.",
    notCreated: "Connect to your server to create your identity.",
    custom: {
      add: "Add field",
      label: "Label",
      value: "Value",
      hidden: "Hidden",
      labelN: (n: number) => `Label of field ${n}`,
      valueN: (n: number) => `Value of field ${n}`,
      hiddenN: (n: number) => `Hide field ${n}`,
      remove: (n: number) => `Remove field ${n}`,
    },
    notesPlaceholder: "Anything else about you",
    readOnly: "Your identity cannot be deleted. Clear any value you no longer want to keep.",
  },

  card: {
    nav: "Cards",
    kind: "Card",
    edit: "Edit card",
    newTitle: "New card",
    fields: {
      title: "Title",
      cardholderName: "Cardholder name",
      brand: "Type",
      number: "Number",
      verificationNumber: "Verification number",
      expiry: "Expiry date",
      notes: "Notes",
    },
    detectBrand: "Detect from number",
    otherBrand: "Other",
    expired: "Expired",
    checkDigitWarning: "This number fails the card check digit. Check it for typos.",
    keepNumber: "Leave empty to keep the saved number",
    keepVerificationNumber: "Empty keeps the code",
    numberRemoved: "The number will be removed.",
    verificationNumberRemoved: "The code will be removed.",
    expiryPlaceholder: "MM/YYYY",
    expiryInvalid: "Enter the expiry date as MM/YYYY.",
    copy: (label: string) => `Copy ${label}`,
    show: (label: string) => `Show ${label}`,
    hide: (label: string) => `Hide ${label}`,
    hidden: (label: string) => `Hidden ${label}`,
    copied: (label: string, seconds: number) => `${label} copied. The clipboard clears in ${seconds} s.`,
    loadFailed: "Could not open this card.",
    revealFailed: "Could not show this value.",
  },
  kit: {
    qrLabel: "Secret Key QR code",
    loadFailed: "Could not load the Recovery Sheet.",
    preparing: "Preparing your Recovery Sheet…",
    sheetLabel: "Recovery Sheet",
    heading: "Recovery Sheet",
    created: (date: string) => `Created ${date}`,
    /** "…you need <strong>both</strong> your master password…" */
    ledeBefore: "To open your vault on a new computer or phone you need ",
    ledeBoth: "both",
    ledeAfter:
      " your master password and this Secret Key, plus the email and server below. Print this page or save it somewhere safe and offline. Anyone who has the Secret Key and your master password can open your vault.",
    blankLine: "Blank line to write your master password, if you choose",
    accountId: "Account ID",
    qrCaption: "Scan to set up another device: the account, the address, the server and the Secret Key.",
    foot: "HavenKeys cannot recover your master password or Secret Key. If you lose this sheet, you can view it again on any device where your vault is unlocked (Settings → Account).",
    print: "Print or save as PDF",
    savedCheck: "I have saved my Recovery Sheet",
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
    data: "Your data",
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
    approvedBy: (name: string) => `Approved by ${name}`,
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
    kitTitle: "Recovery Sheet",
    keyInFile: "Your Secret Key is stored in a file on this computer because no system keychain is available.",
    kitTeaser:
      "Your Secret Key, the account and the server — everything another computer needs, besides your master password.",
    showKit: "Show Recovery Sheet",
    removeTitle: "Remove this device",
    removeNote:
      "Takes this computer off the account and returns HavenKeys to its first-run screen, where you can sign in to any account or server. Your vault stays on the server. A copy of the encrypted file is kept as vault.sqlite3.removed-… in the app's data folder; opening it later needs your master password and the Secret Key from your Recovery Sheet.",
    typeToConfirm: (email: string) => `Type ${email} to confirm`,
    remove: "Remove this device",
  },

  import: {
    title: "Import",
    note: "Bring your logins over from another password manager or browser. Exports contain all of your passwords unencrypted, so delete the file once the import is done.",
    overview: "From 1Password, Bitwarden, LastPass, KeePassXC, a browser or a HavenKeys backup.",
    sourceLabel: "Import from",
    sources: {
      onePassword: "1Password",
      bitwardenJson: "Bitwarden",
      bitwardenCsv: "Bitwarden",
      chrome: "Chrome, Edge or Brave",
      firefox: "Firefox",
      keePassXc: "KeePassXC",
      lastPass: "LastPass",
      havenKeysBackup: "HavenKeys backup",
    },
    howTo: {
      onePassword: "In 1Password, choose File › Export and the 1PUX format.",
      bitwardenJson:
        "In Bitwarden, choose Tools › Export vault and the .json format. Encrypted exports can’t be read; choose the unencrypted one.",
      bitwardenCsv:
        "In Bitwarden, choose Tools › Export vault and the .csv format. The CSV leaves out cards and identities; the .json export keeps them.",
      chrome:
        "In Chrome, open Password Manager › Settings › Export passwords. Edge and Brave have the same option under Passwords.",
      firefox: "In Firefox, open Passwords (about:logins), then the ⋯ menu › Export passwords.",
      keePassXc: "In KeePassXC, choose Database › Export › CSV File.",
      lastPass: "In LastPass, open Advanced Options › Export › LastPass CSV File.",
      havenKeysBackup: "Choose a backup made from Export. Items already in your vault are left as they are.",
    },
    backupPassword: "Backup password",
    skippedExisting: (n: number) => `${n} ${n === 1 ? "item was" : "items were"} already in your vault and left as ${n === 1 ? "it was" : "they were"}.`,
    choose: (extension: string) => `Choose .${extension} file…`,
    importing: "Importing…",
    failed: "Import failed.",
    fileDeleted: "Export file deleted.",
    deleteFailed: "Could not delete the file.",
    /** "<strong>Imported 3 items</strong> from file: 2 logins and 1 secure note." */
    imported: (n: number) => (n === 1 ? "Imported 1 item" : `Imported ${n} items`),
    summary: (fileName: string, logins: number, notes: number, cards: number, identities = 0) => {
      const parts = [logins === 1 ? "1 login" : `${logins} logins`, notes === 1 ? "1 secure note" : `${notes} secure notes`];
      if (cards > 0) parts.push(cards === 1 ? "1 card" : `${cards} cards`);
      if (identities > 0) parts.push(identities === 1 ? "1 identity" : `${identities} identities`);
      return ` from ${fileName}: ${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}.`;
    },
    convertedToNotes: (n: number) =>
      `${n} ${n === 1 ? "item" : "items"} of other kinds (identities, keys…) became secure notes with all their fields.`,
    skippedDuplicates: (n: number) => `${n} ${n === 1 ? "item was" : "items were"} already in your vault and skipped.`,
    skippedArchived: (n: number) => `${n} ${n === 1 ? "archived item was" : "archived items were"} left out.`,
    attachmentsSkipped: (n: number) =>
      `${n} ${n === 1 ? "file attachment was" : "file attachments were"} left out (not supported yet).`,
    passwordHistorySkipped: (n: number) =>
      `${n} ${n === 1 ? "old password" : "old passwords"} from password history ${n === 1 ? "was" : "were"} left out.`,
    urlsMovedToNotes: (n: number) =>
      `${n} ${n === 1 ? "website entry wasn’t" : "website entries weren’t"} a web address and ${n === 1 ? "was" : "were"} kept in the item’s notes.`,
    fieldsToNotes: (n: number) =>
      n === 1 ? "1 field over a login's limit was kept in its notes." : `${n} fields over a login's limit were kept in their notes.`,
    ssoUpgraded: (n: number) =>
      `${n} ${n === 1 ? "login already in your vault now signs" : "logins already in your vault now sign"} in with a provider such as Google or GitHub.`,
    passkeysSkipped: (n: number) =>
      `${n} ${n === 1 ? "passkey was" : "passkeys were"} left out (importing passkeys is not supported yet).`,
    failedItems: (n: number) =>
      `${n} ${n === 1 ? "item" : "items"} couldn’t be imported (a field was over the size limits).`,
    restoreFailedItems: (n: number) =>
      `${n} ${n === 1 ? "item" : "items"} couldn’t be restored (invalid or unreadable).`,
    wasDeleted: "The export file was deleted.",
    confirmDelete: (fileName: string) => `Delete ${fileName}?`,
    deleteFile: "Delete file",
    keepFile: "Keep it",
    deleteExport: "Delete the export file",
  },

  deleteAccount: {
    title: "Delete account and all data",
    explain:
      "This deletes your vault from the server, signs out every device, and erases this computer's copy. It cannot be undone: neither you nor the server's operator can recover it.",
    keptNote:
      "Copies set aside by an earlier \u201cRemove this device\u201d are kept, and so are backups you exported.",
    start: "Delete…",
    backupFirst: "Make an encrypted backup first",
    continueWithout: "Continue without a backup",
    typeToConfirm: (email: string) => `Type ${email} to confirm`,
    masterPassword: "Master password",
    confirm: "Delete account",
    failed: "The account could not be deleted.",
    offline: "Connect to your server to delete the account.",
    done: "Your account and its data were deleted.",
  },
  export: {
    title: "Export",
    note: "Keep an encrypted backup, or move your data to another password manager.",
    formatLabel: "Export as",
    formats: {
      backup: "HavenKeys backup",
      bitwardenJson: "Bitwarden",
      csv: "CSV",
    },
    recommended: "Recommended",
    contains: "This file will contain",
    formatHelp: {
      backup: "Encrypted with a backup password you choose. Holds everything, passkeys included; restore it from Import on any HavenKeys.",
      bitwardenJson: "For Bitwarden, Proton Pass, KeePassXC or 1Password. Passkeys are never included.",
      csv: "Logins only, for browsers and other managers. Passkeys are never included.",
    },
    plaintextWarning:
      "Export contains all passwords in plaintext. Anyone who gets this file can read every password. Delete it once you’ve imported it elsewhere, and don’t open it in a spreadsheet.",
    understand: "I understand",
    backupPassword: "Backup password",
    backupPasswordAgain: "Backup password again",
    backupHint: "Use a long passphrase (the generator can make one). If you lose it, the backup can’t be opened.",
    mismatch: "The backup passwords don’t match.",
    masterPassword: "Master password",
    export: "Export…",
    exporting: "Exporting…",
    failed: "Export failed.",
    summaryFailed: "Couldn’t count what this export would hold, so it can’t be written. Try again in a moment.",
    includes: (logins: number, notes: number, cards: number, identities: number) =>
      `${logins} ${logins === 1 ? "login" : "logins"}, ${notes} ${notes === 1 ? "secure note" : "secure notes"}, ${cards} ${cards === 1 ? "card" : "cards"}, ${identities} ${identities === 1 ? "identity" : "identities"}.`,
    passkeysLeftOut: (n: number) => `${n} ${n === 1 ? "passkey is" : "passkeys are"} not included.`,
    historyLeftOut: (n: number) => `${n} old ${n === 1 ? "password" : "passwords"} from password history ${n === 1 ? "is" : "are"} not included.`,
    fieldsLeftOut: (n: number) => `${n} custom ${n === 1 ? "field is" : "fields are"} not included.`,
    itemsLeftOut: (n: number) => `${n} ${n === 1 ? "item that isn’t a login is" : "items that aren’t logins are"} not included.`,
    unreadable: (n: number) => `${n} ${n === 1 ? "item can’t be read and is" : "items can’t be read and are"} not included.`,
    done: (n: number, fileName: string) => `Exported ${n} ${n === 1 ? "item" : "items"} to ${fileName}.`,
    deleteReminder: "This file isn’t encrypted. Delete it once you’re done with it.",
  },

  updates: {
    available: (version: string) => `HavenKeys ${version} is available.`,
    whatsNew: "What’s new",
    hideNotes: "Hide notes",
    update: "Update",
    download: "Download",
    later: "Later",
    restartNote: "Updating locks HavenKeys and restarts it.",
    downloading: (version: string, percent: number | null) =>
      percent === null ? `Downloading HavenKeys ${version}…` : `Downloading HavenKeys ${version}… ${percent}%`,
    installing: "Installing the update…",
    failed: "The update failed. Try again later.",
    tryAgain: "Try again",
    title: "Updates",
    autoCheck: "Check for updates automatically",
    checkNow: "Check now",
    checking: "Checking…",
    upToDate: "HavenKeys is up to date.",
    checkFailed: "Could not check for updates.",
    version: (version: string) => `Version ${version}`,
    note: "HavenKeys asks GitHub, where its releases are published, whether a newer version exists. Updates are signed, and nothing is installed until you choose Update.",
    manualNote: "This copy was installed from a .deb or .rpm package. New versions are downloaded from the release page.",
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
