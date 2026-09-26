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
export const en = {
  popup: {
    settings: "Settings",
    lock: "Lock HavenKeys",
    unreachable: "The extension could not be reached.",
    noUsername: "No username",
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
};

export type Messages = typeof en;
