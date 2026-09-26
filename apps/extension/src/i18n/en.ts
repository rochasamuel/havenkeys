/*
 * Every word the extension shows, in English. pt-BR.ts must match this
 * shape exactly (it is typed as Messages), so a string added here and
 * forgotten there fails the typecheck instead of shipping half-translated.
 *
 * Product names (HavenKeys, Secret Key, Emergency Kit, havenkeys-server)
 * stay in English in every locale. Strings are inserted with textContent
 * only; parameterised strings are functions.
 *
 * The manifest's name and description live in manifest/_locales instead,
 * because the browser reads them before any script runs.
 */
import type { ErrorCode } from "@havenkeys/protocol";

/**
 * The desktop's error messages, by protocol code. The native host sends a
 * fixed English message per code (havenkeys-protocol ErrorCode::message);
 * the English here matches it word for word, and the extension shows the
 * entry for the code instead of the wire text.
 */
const bridge = {
  locked: "HavenKeys is locked.",
  busy: "HavenKeys is busy. Try again in a moment.",
  no_vault: "No vault has been created yet.",
  not_found: "Item not found.",
  denied: "This item is not saved for this website.",
  invalid_input: "Invalid request.",
  decryption: "Failed to decrypt vault item.",
  corrupted: "The vault item is damaged.",
  malformed: "Malformed message.",
  too_large: "Message too large.",
  unsupported_version: "Unsupported protocol version.",
  rate_limited: "Too many requests. Try again shortly.",
  integration_disabled: "Browser integration is turned off in HavenKeys settings.",
  desktop_unavailable: "The HavenKeys app is not running.",
  offline: "HavenKeys is offline. The vault is read-only until it reconnects.",
  internal: "Internal error.",
} satisfies Record<ErrorCode, string>;

export const en = {
  common: {
    noUsername: "No username",
    passkey: "Passkey",
  },

  popup: {
    settings: "Settings",
    lock: "Lock HavenKeys",
    unreachable: "The extension could not be reached.",
    fill: "Fill",
    fillTitle: "Fill this login into the page",
    code: "Code",
    codeTitle: "Show the one-time code",
    fillCodeTitle: "Fill this code into the page",
    pill: {
      offline: "Offline",
      locked: "Locked",
      off: "Off",
      unlocked: "Unlocked",
    },
    hostUnavailable: {
      title: "Not connected",
      body: "Install or update the HavenKeys app on this computer, then open it once. It connects this browser for you.",
    },
    desktopUnavailable: {
      title: "HavenKeys is not running",
      body: "Open the HavenKeys app on this computer.",
    },
    noVault: {
      title: "No vault yet",
      body: "Create your vault in the HavenKeys app.",
    },
    locked: {
      title: "HavenKeys is locked",
      body: "Unlock it in the HavenKeys app to use your logins.",
    },
    disabled: {
      title: "Browser integration is off",
      body: "Turn it on in HavenKeys → Settings → Browser extension.",
    },
    error: {
      title: "Something went wrong",
    },
    noPage: {
      title: "No saved logins here",
      body: "This page can't use saved logins.",
    },
    noMatches: {
      title: "No saved logins for this site",
      body: "Logins saved in HavenKeys for this website appear here.",
    },
    offer: {
      title: "Suggestions in login fields are off",
      body: "Turn them on to pick logins right under the field and to save and use passkeys with HavenKeys.",
      turnOn: "Turn on",
      turnOnTitle: "Show your logins under login fields on websites",
      doneTitle: "Suggestions are on",
      doneBody: "Reload open tabs to use them there.",
    },
  },

  options: {
    pageTitle: "HavenKeys settings",
    subtitle: "Browser extension settings",
    suggestionsTitle: "Suggestions and passkeys",
    toggleLabel: "Show my logins under login fields, and save passkeys to HavenKeys",
    notes: [
      "When this is on, clicking a username, password or one-time-code field on a website shows your matching HavenKeys logins right under the field. Nothing is filled until you pick a login, and logins are only offered on the websites they are saved for.",
      "The same switch lets HavenKeys answer when a website creates or asks for a passkey, instead of your browser or operating system. Without it, websites get the browser's own passkey dialog.",
      "It needs permission to run on the websites you visit. The extension reads login fields when you interact with them and never sends page contents anywhere; the desktop app decides which logins a website may use.",
    ],
    withoutTitle: "Without it",
    /** Split around the popup's "Fill" button label, which is shown emphasised. */
    withoutBefore: "Click the HavenKeys button in the toolbar and choose ",
    withoutAfter:
      ". The extension then has access only to the tab you clicked on, and only until you leave the page. Offers to save new logins need suggestions to be on.",
    statusOff: "Off. HavenKeys works from the toolbar button only.",
    statusHttps: "On for secure (https) websites.",
    statusOn: "On. Reload open tabs to see suggestions there.",
    turnOn: "Turn on",
    turnOff: "Turn off",
  },

  menu: {
    pageTitle: "HavenKeys suggestions",
    lockedTitle: "HavenKeys is locked",
    lockedBody: "Unlock the HavenKeys app to fill.",
    unavailable: "Unavailable",
    couldNotFill: "Could not fill",
    fillCode: "Fill one-time code",
    generateTitle: "Generate strong password",
    generateBody: "Fills the new password fields",
    passkeyRow: (account: string) => `Passkey · ${account}`,
    passkeyAccountFallback: "account",
    usePasskeyTitle: (site: string) => `You have a passkey for ${site}`,
    usePasskeyBody: "Use the site’s “Sign in with a passkey” option",
    addPasskeyTitle: (name: string) => `${name} supports passkeys`,
    addPasskeyBody: "How to add one",
    noCodesTitle: "No one-time codes here",
    noCodesBody: "No login for this site has a one-time code.",
    noLoginsTitle: "No logins for this site",
    noLoginsBody: "Save one in the HavenKeys app.",
  },

  save: {
    pageTitle: "Save to HavenKeys",
    loading: "Save this login?",
    addQuestion: "Save this login to HavenKeys?",
    updateQuestion: "Update the saved password?",
    save: "Save",
    update: "Update",
    notNow: "Not now",
  },

  passkey: {
    pageTitle: "HavenKeys passkeys",
    loading: "Loading…",
    useAnotherDevice: "Use another device",
    cancel: "Cancel",
    save: "Save",
    close: "Close",
    saveTo: "Save to",
    newLogin: "New login",
    newLoginDetail: "Create a login for this site",
    lockedTitle: "HavenKeys is locked",
    lockedBody: "Unlock the HavenKeys app — this card will update.",
    chooserTitle: "Sign in with a passkey",
    chooseAccount: "Choose an account",
    addTitle: "Add a passkey?",
    saveTitle: "Save a passkey to HavenKeys?",
    accountLabel: "Account:",
    noAccountName: "No account name",
    existsTitle: "This account already has a passkey in HavenKeys",
    savedTitle: "Passkey saved to HavenKeys",
    savedBody: "Manage it in the HavenKeys app",
  },

  content: {
    iconLabel: "HavenKeys: show logins",
  },

  /** Messages the background and the frames send to the UI. */
  errors: {
    unreachable: "HavenKeys could not be reached.",
    generic: "Something went wrong.",
    invalidRequest: "Invalid request.",
    menuExpired: "This menu has expired.",
    promptExpired: "This prompt has expired.",
    unknownItem: "Unknown item.",
    unknownPasskey: "Unknown passkey.",
    unknownLogin: "Unknown login.",
    unknownRequest: "Unknown request.",
    pleaseWait: "Please wait…",
    pageNotSupported: "This page can't use saved logins.",
    noLoginForm: "No login form found on this page.",
    hostUnavailable: "The HavenKeys native messaging host is not installed or failed to start.",
    timeout: "HavenKeys did not respond.",
    unexpectedResponse: "Unexpected response.",
    bridge,
  },
};

export type Messages = typeof en;
