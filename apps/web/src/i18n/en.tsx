import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Icon } from "../components/Icon";
import type { IconName } from "../components/Icon";

/*
 * Every word the site shows, in English. pt-BR.tsx must match this shape
 * exactly (it is typed as Messages), so a string added here and forgotten
 * there fails the typecheck instead of shipping half-translated.
 *
 * Product names that appear in the app's own UI (Secret Key, Recovery Sheet,
 * havenkeys-server) stay in English in every locale, because that is what
 * the user will see on screen.
 */

const GH = "https://github.com/rochasamuel/havenkeys";
const DOCS = `${GH}/blob/main/docs/`;
const CHROME_STORE = "https://chromewebstore.google.com/detail/havenkeys/fmmfkakdkkcfpdnfmbngnlelbfaogafo";
const FIREFOX_STORE = "https://addons.mozilla.org/firefox/addon/havenkeys/";

function Ext({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noreferrer">
      {children}
    </a>
  );
}

export type SceneId = "signin" | "otp" | "signup" | "save" | "popup";

export const en = {
  meta: {
    title: "HavenKeys — passwords only you can open",
    description:
      "HavenKeys is a free, open-source password manager for your computer, phone and browser. Your vault is locked on your devices; nobody else can open it.",
  },

  common: {
    disclaimer:
      "This software has not undergone an independent security audit and should not be considered a replacement for professionally audited password managers for high-value production use.",
    downloadCta: "Download HavenKeys",
    addToChrome: "Add to Chrome",
    addToFirefox: "Add to Firefox",
    createAccount: "Create account",
    beta: "Beta",
    betaNotice:
      "HavenKeys is a beta. It works and it is what we use every day, but expect rough edges, and keep your Recovery Sheet somewhere safe.",
    installerWarning:
      "The installers aren’t code-signed yet, so Windows SmartScreen and macOS Gatekeeper will warn you on first run. Android verifies the APK’s signature itself.",
  },

  nav: {
    homeAria: "HavenKeys home (beta)",
    mainAria: "Main",
    security: "Security",
    selfHost: "Self-host",
    developers: "Developers",
    pricing: "Pricing",
    githubAria: "HavenKeys on GitHub",
    download: "Download",
    switchShort: "PT",
    switchAria: "Ver o site em português",
  },

  footer: {
    tagline: "Passwords only you can open.",
    aria: "Footer",
    download: "Download",
    security: "Security",
    selfHost: "Self-host",
    developers: "Developers",
    pricing: "Pricing",
    github: "GitHub",
    privacy: "Privacy",
    terms: "Terms & license",
    deleteAccount: "Delete account",
    license: "MIT or Apache-2.0",
    languageAria: "Language",
  },

  home: {
    heroTitle: (
      <>
        Your passwords, locked — and only <em>yours</em>.
      </>
    ),
    heroLede:
      "A password manager for your computer, your phone and your browser. Your vault is locked on your own devices, and nobody else — not even the server that keeps your copy — can open it.",
    heroMeta: "Free and open source · Windows, macOS, Linux and Android · Chrome and Firefox",
    desktopAlt:
      "The HavenKeys desktop app: a sidebar with the vault's sections, a list of logins, and the Fernway login open with its username, hidden password and a live one-time code.",
    popupAlt: "The HavenKeys browser popup: two saved Fernway logins, a Fill button, and a one-time code.",
    menuAlt: "The HavenKeys in-page menu offering two saved logins.",

    cardsTitle: "Everything you need. Nothing you don’t.",
    cardsLede: "The essentials of a modern password manager, made with care.",
    cards: [
      { icon: "key" as IconName, title: "Fills logins when you click", text: "On a sign-in page, HavenKeys offers what you saved for that site and waits for you to choose." },
      { icon: "lock" as IconName, title: "Passkeys", text: "Create passkeys and sign in with them, on your computer and your phone." },
      { icon: "check" as IconName, title: "One-time codes built in", text: "Two-step codes live next to the password. No separate authenticator app." },
      { icon: "laptop" as IconName, title: "Works offline", text: "Every device keeps its own locked copy, so your passwords are there even without internet." },
      { icon: "monitor" as IconName, title: "On all your devices", text: "Desktop app, Android app and a browser extension for Chrome and Firefox, kept in sync." },
      { icon: "download" as IconName, title: "Bring your passwords", text: "Import from 1Password, Bitwarden, LastPass, KeePassXC, Chrome or Firefox. Export any time." },
    ],

    trustTitle: "Nobody else can open it.",
    trustBody:
      "Your vault is locked on your device with two things only you have: your master password and a Secret Key HavenKeys creates for you. What reaches the server is already locked, so the people who run it — us included — can’t read a single password.",
    trustLink: "How it works, in detail",

    stepsTitle: "Start in five steps.",
    steps: [
      { title: "Install the app", text: "Get HavenKeys for Windows, macOS, Linux or Android." },
      { title: "Get your account", text: "Create an account on our server, or run your own." },
      { title: "Print your Recovery Sheet", text: "It holds your Secret Key. Keep it somewhere safe: it’s how you get back in on a new device." },
      { title: "Bring your passwords", text: "Import them from your old password manager, or add them as you go: HavenKeys offers to save new ones." },
      { title: "Add the extension, then click", text: "Install it in Chrome or Firefox. On a sign-in page, click the field and pick your login." },
    ],

    selfHostTitle: "Prefer your own server?",
    selfHostBody:
      "Run HavenKeys on a small server of your own or on Railway. Setup takes a few commands, and the Docker setup includes nightly backups.",
    selfHostCta: "Run your own server",

    closerTitle: (
      <>
        Bring your passwords <em>home</em>.
      </>
    ),
    closerLede: "Install the app, create an account, and add the extension.",
    readSource: "Read the source",
  },

  journey: {
    chapters: [
      {
        label: "Typed",
        title: (
          <>
            You type it <em>once</em>.
          </>
        ),
        body: [
          "Unlocking starts with two secrets. Your master password never becomes a key itself: it goes through Argon2id, which is deliberately slow and memory-hungry so each guess costs an attacker real hardware.",
          "The result is mixed with your Secret Key, 128 random bits made on your device. Together they open the vault key, and all of it happens in Rust, inside the desktop app.",
        ],
      },
      {
        label: "Sealed",
        title: (
          <>
            It’s sealed <em>before</em> it’s saved.
          </>
        ),
        body: [
          "Every item is encrypted on your device with AES-256-GCM. That covers the title, the username and the website, not just the password, so nobody reading the database learns which sites you use.",
          "Each save gets a fresh nonce, and each blob is bound to its vault and its item. Move one anywhere else and it refuses to open.",
        ],
      },
      {
        label: "Stored",
        title: (
          <>
            Your server keeps it. It <em>can’t</em> read it.
          </>
        ),
        body: [
          <>
            HavenKeys syncs through <code>havenkeys-server</code>, a small server you run yourself,
            or ours if you create an account. Whoever runs it holds only locked data it can’t open.
          </>,
          "The server is the one place changes are written. Every computer keeps its own encrypted copy, so unlocking, search, one-time codes and autofill keep working offline.",
        ],
      },
      {
        label: "Filled",
        title: (
          <>
            It comes back when <em>you</em> ask.
          </>
        ),
        body: [
          "Click a login field and HavenKeys offers the logins saved for that site. Nothing fills on page load, and nothing fills until you pick one.",
          "The desktop app checks the page’s address against the item’s saved websites before it sends anything, so a look-alike domain gets nothing, whatever the page or the extension claims.",
        ],
      },
    ] as Array<{ label: string; title: ReactNode; body: ReactNode[] }>,
    stageFoot: "One login, followed from keyboard to autofill · demo data",
  },

  scenes: {
    keys: {
      masterLabel: "Master password",
      masterWhere: "Only in your head",
      secretLabel: "Secret Key",
      secretWhere: "On your devices and your Recovery Sheet",
      argonParam: "128 MiB of memory · 4 passes · 4 lanes",
      hkdfParam: "master key + Secret Key → key-encryption key",
      vaultKey: "Vault key",
      vaultParam: "256 random bits, unwrapped in memory, gone when you lock",
    },
    seal: {
      labels: ["Title", "Username", "Password", "Website", "One-time code"],
      totpPlain: "set up",
      plaintext: "Plaintext, in memory",
      sealed: "Sealed",
      notes: ["AES-256-GCM", "fresh 96-bit nonce every save", "bound to its vault, item and role"],
    },
    store: {
      where: "on a server you run, or ours",
      blobsAria: "What the server stores",
      cantOpen: "Can’t open titles, usernames, websites, passwords, codes or notes",
      doesSee: "Does see your email, how many items, their sizes and when they changed",
      laptop: "Laptop",
      desktop: "Desktop",
      deviceNote: "encrypted copy, works offline",
    },
    fill: {
      emailLabel: "Email",
      menuAlt: "The HavenKeys suggestion menu under an email field, listing two saved Fernway logins.",
      origins: [
        "Saved here. Pick it and it fills.",
        "Same site, so it’s offered.",
        "Denied. Different site.",
        "Denied. Looks alike, isn’t.",
      ],
    },
  },

  showcase: {
    tabsAria: "Browser extension scenarios",
    scenarios: {
      signin: {
        tab: "Sign in",
        title: "Only the logins saved for this site.",
        body: "Click the email field and the logins saved for this site appear under it. Pick one and both fields fill. The menu runs in the extension’s own frame, where the page can’t read or script it.",
        alt: "HavenKeys menu listing two Fernway logins",
      },
      otp: {
        tab: "One-time code",
        title: "The code, never the secret.",
        body: "The desktop app works out the six digits and sends just those. The TOTP secret stays in the vault, so the page and the extension never see it.",
        alt: "HavenKeys menu offering to fill the one-time code",
      },
      signup: {
        tab: "New account",
        title: "A strong password in one click.",
        body: "On a sign-up form, HavenKeys offers a generated password. The desktop app makes it from your operating system’s secure random source and fills every new-password field.",
        alt: "HavenKeys menu offering to generate a strong password",
      },
      save: {
        tab: "Save",
        title: "It asks. It never assumes.",
        body: "After you sign in with something new, HavenKeys offers to save it or update the old password. It offers only what you typed, so a page can’t use the prompt to fish for passwords.",
        alt: "HavenKeys asking whether to save the new Fernway login",
      },
      popup: {
        tab: "Toolbar",
        title: "Works without the in-page menu too.",
        body: "Prefer no menu under login fields? Turn in-page suggestions off in the extension’s options: the toolbar button fills the current tab with the same origin checks, and offers to save logins and passkeys keep working.",
        alt: "The HavenKeys toolbar popup with two logins, Fill buttons and a one-time code",
      },
    } as Record<SceneId, { tab: string; title: string; body: string; alt: string }>,
    soloCaption: (url: ReactNode) => (
      <>
        On {url} · demo site
      </>
    ),
    demo: {
      badge: "Demo site",
      signIn: "Sign in",
      welcomeBack: "Welcome back to Fernway.",
      email: "Email",
      password: "Password",
      continue: "Continue",
      twoStep: "Two-step verification",
      enterCode: "Enter the 6-digit code from your authenticator app.",
      verificationCode: "Verification code",
      verify: "Verify",
      createTitle: "Create your account",
      createLede: "It takes less than a minute.",
      newPassword: "New password",
      createButton: "Create account",
      welcomeSam: "Welcome, Sam",
      signedIn: "You’re signed in to Fernway.",
      dashboard: "Go to dashboard",
    },
  },

  kit: {
    aria: "Illustration of a HavenKeys Recovery Sheet",
    caption: "Illustration. The key shown is made up.",
  },

  developers: {
    heroTitle: "How HavenKeys works, end to end.",
    heroLede:
      "HavenKeys is small on purpose. The cryptography comes from established Rust libraries, the rules are written down, and the attacks it claims to stop are tests in the code. Here’s the whole design in one page.",

    journeyTitle: "Follow one password home.",
    journeyLede:
      "HavenKeys is a password manager with careful autofill, one-time codes and a vault that works offline. What sets it apart is what happens to a password between the moment you type it and the moment it’s filled. Here’s that trip, one step at a time.",

    browserTitle: "Autofill that waits for your click.",
    browserLede:
      "The extension for Chrome and Firefox reads the form the way you do, offers what’s saved for that site, and does nothing until you choose. These are the real menus.",
    extensionListTitle: "Also in the extension",
    extensionFeatures: [
      {
        term: "Passkeys",
        text: "Sign in with a saved passkey from the field menu, and save new ones. After you sign in with a password on a site that supports passkeys, HavenKeys can add a passkey for you (you can turn this off).",
      },
      {
        term: "Automatic sign-in",
        text: "After you pick a login, HavenKeys can press the sign-in button and fill the next step and the one-time code, on the same site, within two minutes. Nothing happens without your pick. Off per login or for the whole vault.",
      },
      { term: "Edit in HavenKeys", text: "From the popup, open a login in the desktop app to change it." },
    ],

    desktopTitle: "A vault that lives on your desk.",
    desktopLede:
      "The desktop app holds the keys. It sits in the tray, locks itself when you step away, and does all the cryptography in its Rust core. The interface never sees a key, and a password reaches the screen only when you reveal it.",
    features: [
      { term: "Logins", text: "Usernames, passwords, websites with match rules, one-time codes and notes." },
      { term: "Secure notes", text: "Recovery codes, passphrases, anything that isn’t a login. Encrypted whole." },
      {
        term: "Passkeys",
        text: "Create passkeys and sign in with them. The private key is made and used only in the Rust core; the site’s identity is checked there too.",
      },
      { term: "One-time codes", text: "SHA-1, SHA-256 or SHA-512, six or eight digits. Paste an otpauth:// link once." },
      {
        term: "Scan a QR code",
        text: "Set up a one-time code by scanning the QR code from the clipboard or the screen. The secret stays in Rust until you save.",
      },
      { term: "Password generator", text: "Length and character sets are up to you. Random from the OS, with no bias." },
      { term: "Search", text: "Titles, usernames and websites, searched in memory. No plaintext index on disk." },
      { term: "Password history", text: "The last five passwords of every login, even ones changed from the browser." },
      { term: "Import and export", text: "Bring passwords from 1Password, Bitwarden, LastPass, KeePassXC, Chrome or Firefox. Export a Bitwarden file, a CSV, or an encrypted HavenKeys backup." },
      {
        term: "Auto-lock",
        text: "After 5 to 60 minutes idle, on sleep and on quit. On Windows and Linux, when your session locks too.",
      },
      {
        term: "Secret Key in the keychain",
        text: "Stored in your system’s keychain (Windows Credential Manager, macOS Keychain, Secret Service on Linux).",
      },
      {
        term: "Opens at login",
        text: "Optionally starts with your computer, locked, in the tray, so the extension can reach it.",
      },
      {
        term: "Updates itself",
        text: "Checks for signed updates and installs only when you click Update. You can turn the check off.",
      },
      {
        term: "English and Portuguese",
        text: "The app follows your system language, or the one you pick in Settings. The extension follows your browser’s language.",
      },
    ],

    paperTitle: "Two secrets. One of them lives on paper.",
    paperP1:
      "Your master password is the one you remember. Your Secret Key is 128 random bits made on your device when you set up. It’s stored on each of your computers and printed on your Recovery Sheet, and it never goes to the server.",
    paperP2:
      "So a stolen copy of the server’s database isn’t a password-guessing exercise. Without the Secret Key, an attacker has to guess both.",
    paperWarn:
      "There’s no account recovery. Lose the sheet and every device that holds the key, and the vault is gone. That’s the price of nobody else being able to open it.",

    ledgerTitle: "What it defends. What it doesn’t.",
    ledgerLede:
      "Security software earns trust by being specific. This is the short version of the threat model, limitations included.",
    defendsTitle: "Designed to stop",
    defends: [
      "Someone with a copy of the server’s database or a backup. Without your Secret Key they’d also have to guess 128 random bits.",
      "A server operator reading your vault. It stores ciphertext and has no key for it.",
      "Pages that fake forms, hide fields, frame other sites or synthesize clicks to trigger a fill.",
      "Look-alike domains. Matching uses the public suffix list, so fernway.example.evil.com is a different site.",
      "Secrets leaking into logs, error messages, URLs, notifications or window titles.",
    ],
    doesntTitle: "Not defended",
    doesnt: [
      "Malware running as you while the vault is unlocked. No local password manager can stop that.",
      "A server that deletes your data. It’s the single writer, so tested backups are part of running one.",
      "A weak master password on a device that’s been copied whole, Secret Key included.",
      "It has not had an independent audit, and the installers aren’t code-signed yet.",
    ],

    chainTitle: "One chain of keys, no shortcuts.",
    chainLede:
      "Your master password is never used to encrypt anything directly. It feeds a memory-hard function, is combined with a random Secret Key, and unwraps a vault key that was random from the start. Changing your master password rewraps that key. None of your items change.",
    chainLink: "Full cryptography design",
    hierarchy: [
      { name: "Master password", detail: "Never stored. Only ever fed to Argon2id.", tone: "input" },
      { name: "Argon2id", detail: "128 MiB, 4 passes, 4 lanes, a random 16-byte salt.", tone: "op" },
      { name: "Master key", detail: "32 bytes, in memory only.", tone: "key" },
      { name: "HKDF-SHA-256", detail: "Mixes in your 128-bit Secret Key, bound to your account and email.", tone: "op" },
      {
        name: "Key-encryption key",
        detail: "Unwraps the vault key. A sibling key signs you in to the server and unwraps nothing.",
        tone: "key",
      },
      { name: "Vault key", detail: "256 random bits from the OS. Stored only wrapped.", tone: "key" },
      { name: "Data key", detail: "Derived per vault. Lives in memory while unlocked.", tone: "key" },
      { name: "Your items", detail: "AES-256-GCM, fresh nonce per save, bound to vault, item and role.", tone: "out" },
    ],

    zonesTitle: "Five places, five levels of trust.",
    zonesLede:
      "Each part of HavenKeys gets only what its job needs. The line that matters most runs between the Rust core and everything else: it decides, and nothing else can decide for it.",
    zoneGets: "Gets",
    zoneLimits: "Limits",
    zones: [
      {
        name: "Rust core",
        where: "Inside the desktop app",
        holds: "Keys, decryption, the origin check for every fill",
        limit: "Trusted. It’s the part you’re trusting.",
      },
      {
        name: "Desktop interface",
        where: "The app’s window",
        holds: "One revealed field at a time",
        limit: "No keys, no crypto, no file or network access, a strict CSP",
      },
      {
        name: "Browser extension",
        where: "Chrome or Firefox",
        holds: "Titles and usernames for this site; a password when you pick one",
        limit: "Every request is re-checked in Rust. It never sees the vault key.",
      },
      {
        name: "Web pages",
        where: "Everywhere you browse",
        holds: "Nothing, until you pick a login for that page",
        limit: "Treated as hostile. They can’t message the extension at all.",
      },
      {
        name: "Your server",
        where: "A server you run, or ours",
        holds: "Ciphertext, plus your email and item counts, sizes and times",
        limit: "No key to any of it. It can delete data, so keep backups.",
      },
    ],

    permTitle: "An extension that asks for less.",
    permLede:
      "Offers to save logins and passkeys need the extension on the sites you visit, so it asks for that at install. It asks for nothing else, never reads pages you don’t interact with, and the desktop app decides which logins each site may use. You can take site access back in your browser at any time.",
    notRequested: "Not requested:",
    permHead: ["Permission", "Why"],
    always: "Always",
    permissions: [
      { name: "nativeMessaging", optional: false, why: "The extension’s only route to the desktop app." },
      {
        name: "activeTab",
        optional: false,
        why: "Read the address of the tab you clicked the toolbar button on, and fill it. Only that tab.",
      },
      {
        name: "scripting",
        optional: false,
        why: "Put the fill script into that tab, and register it on the sites the extension has access to.",
      },
      {
        name: "https://*/*, http://*/*",
        optional: false,
        why: "Offers to save logins, passkeys and in-page suggestions. You can take it back in your browser’s extension settings.",
      },
      {
        name: "storage",
        optional: false,
        why: "One setting: whether logins are suggested under login fields. Nothing else.",
      },
    ],
    optionalTag: "Optional, off by default",

    attacksTitle: "Twelve attacks, written as tests.",
    attacksLede:
      "Each one is a regression test in the Rust or extension test suite, so a change that reopens it fails the tests.",
    attacksHead: ["Attempt", "Result"],
    attacks: [
      ["A page on evil.com asks for the github.com login", "Denied"],
      ["The extension asks for an item by ID on the wrong site", "Denied, and looks the same as “not saved here”"],
      ["A password is requested while the vault is locked", "Denied"],
      ["Ciphertext is modified", "Fails authentication, no plaintext"],
      ["A malformed native message arrives", "Rejected, no crash"],
      ["An oversized native message arrives", "Rejected by the size limit"],
      ["A page creates thousands of inputs", "No significant slowdown"],
      ["An encrypted blob is swapped between items or roles", "Fails authentication"],
      ["A vault with an unknown format version", "Refused safely"],
      ["A github.com login frame embedded in evil.com", "Denied: the top page must match too"],
      ["A page synthesizes clicks or keys to trigger a fill", "Ignored"],
      ["The frame navigates away before the fill lands", "Refused"],
    ] as Array<[string, string]>,

    scopeTitle: "What it won’t protect you from.",
    scopeLede:
      "No local password manager can promise everything. These are the limits, stated up front rather than discovered later.",
    outOfScope: [
      "Malware running as you while the vault is unlocked: it can read memory, log keys or ask the desktop app for logins as the extension does.",
      "Kernel or root compromise, hardware attacks, cold-boot and DMA attacks.",
      "Memory forensics after lock. Keys are zeroed where the code controls them, but copies can survive in places it doesn’t.",
      "Rollback of the vault file, and a server that replays an item’s older password.",
      "A weak master password, and clipboard managers reading a copied password before it clears.",
    ],

    readingTitle: "Read the source documents.",
    readingLede: "Everything above is a summary. These are the documents it summarizes.",
    reading: [
      { title: "Threat model", file: "threat-model.md", body: "What HavenKeys defends against, and what it explicitly doesn’t." },
      { title: "Security model", file: "security-model.md", body: "How each defense is enforced, permission by permission." },
      { title: "Cryptography", file: "crypto.md", body: "The key hierarchy, the blob format, and the exact parameters." },
      {
        title: "Security review",
        file: "security-review.md",
        body: "Findings from reviewing this code against its own threat model, open ones included.",
      },
      { title: "Self-hosting", file: "self-hosting.md", body: "Run your own server with Docker or Railway." },
      { title: "Architecture", file: "architecture.md", body: "How the desktop app, the extension and the Rust core fit together." },
      { title: "Native messaging", file: "native-messaging.md", body: "The protocol between the extension and the desktop app, and how it is validated." },
      { title: "Deployment reference", file: "deployment.md", body: "Environment, Railway, backups." },
    ],

    buildTitle: "Build it yourself",
    buildBody: (
      <>
        HavenKeys is open source under MIT or Apache-2.0. The{" "}
        <Ext href={`${DOCS}development.md`}>development guide</Ext> covers building the desktop app,
        the extension, Android and the server; <Ext href={`${DOCS}self-hosting.md`}>self-hosting.md</Ext>{" "}
        covers running your own server.
      </>
    ),
  },

  security: {
    heroTitle: "Built so that only you can open your vault.",
    heroLede: "What HavenKeys protects, what it can’t, and where to check for yourself.",
    protectsTitle: "What it protects",
    protects: [
      { title: "Your vault, wherever it’s stored", text: "Passwords, notes, codes and passkeys are locked on your device before they’re saved or sent. The server keeps a copy it has no key for." },
      { title: "A stolen server", text: "Someone who copies the server’s data still needs your master password and your Secret Key, which never reach the server in a form anyone can read (your Secret Key also lives on your Recovery Sheet)." },
      { title: "Fake websites", text: "The extension offers a login only on the site it was saved for, and fills only after you click. Look-alike addresses don’t match." },
      { title: "Accidental leaks", text: "Passwords never show up in logs, notifications or window titles, and copied passwords are cleared from the clipboard." },
    ],
    limitsTitle: "What it can’t do",
    limits: [
      "Protect you from malware already running on your computer while your vault is unlocked.",
      "Bring back a vault if you lose your Recovery Sheet and every device. There’s no account recovery: that’s the price of nobody else holding a key.",
      "Restore a server you run yourself that was lost without a backup.",
    ],
    auditTitle: "Honest about where it stands",
    auditBody: "HavenKeys is open source and its security design is written down in full, but it hasn’t had an independent audit yet.",
    deepTitle: "Want the details?",
    deepBody: "How your keys are made, what the extension may access, the attacks it’s tested against and the full threat model are on the Developers page.",
    deepCta: "Read the technical details",
  },

  download: {
    title: "Download HavenKeys",
    latest: "Latest release",
    checking: "Checking the latest release…",
    unavailable: "The latest release couldn’t be loaded. There may not be one published yet.",
    seeReleases: "See releases on GitHub",
    installersAria: "Installers",
    yourSystem: "Your system",
    download: "Download",
    viewReleases: "View releases",
    android: {
      label: "Android",
      format: "APK for Android 9 and later",
      early: "Early release",
      downloadApk: "Download APK",
      checksum: "SHA-256 checksum",
      certificate: "Signing certificate (SHA-256)",
      comingSoon: "The Android app isn’t released yet.",
      stepsTitle: "Installing on Android",
      steps: [
        { title: "Install the APK", body: "Download it and open it. Android asks once to allow installing apps from your browser." },
        { title: "Sign in", body: "Open HavenKeys and scan your Recovery Sheet, or use your setup code." },
        { title: "Turn on autofill", body: "In HavenKeys, open Settings → Autofill setup." },
        { title: "Use it in Chrome", body: "Open Settings → Autofill services and choose “Autofill using another service”." },
      ],
      browsers:
        "Autofill works in apps, Chrome and Firefox. Samsung Internet only lets password managers on Samsung’s own list fill, and HavenKeys isn’t on it.",
    },
    platforms: {
      windows: { label: "Windows", format: ".msi installer" },
      macos: { label: "macOS", format: ".dmg disk image" },
      "linux-appimage": { label: "Linux", format: ".AppImage, runs anywhere" },
      "linux-deb": { label: "Debian & Ubuntu", format: ".deb package" },
    },
    beforeTitle: "Before you install",
    steps: [
      {
        title: "Get an account",
        body: (
          <>
            HavenKeys keeps your locked vault on a server. Create an account on ours, or{" "}
            <Link to="/self-host">run your own server</Link>. Then print your Recovery Sheet: it’s how you
            sign in on a new device.
          </>
        ),
        actions: (
          <Link className="btn btn--ghost btn--sm" to="/signup">
            Create account
          </Link>
        ),
      },
      {
        title: "Add the browser extension",
        body: (
          <>
            Install it from the <Ext href={CHROME_STORE}>Chrome Web Store</Ext> or{" "}
            <Ext href={FIREFOX_STORE}>Firefox Add-ons</Ext>. The installers already include the
            piece that connects the browser to the app, and register it with Chrome and Firefox.
            Then unlock HavenKeys and turn on <strong>Settings → Browser extension</strong>.
          </>
        ),
        actions: (
          <>
            <a className="btn btn--ghost btn--sm" href={CHROME_STORE} target="_blank" rel="noopener noreferrer">
              <Icon name="external" size={15} />
              Add to Chrome
            </a>
            <a className="btn btn--ghost btn--sm" href={FIREFOX_STORE} target="_blank" rel="noopener noreferrer">
              <Icon name="external" size={15} />
              Add to Firefox
            </a>
          </>
        ),
      },
      {
        title: "It updates itself",
        body: "From 0.9.0, HavenKeys checks for signed updates and installs one when you click Update. .deb and .rpm installs get a Download button that opens the release page instead. On 0.8.0 or earlier, install 0.9.0 once by hand, over the existing install — no need to uninstall first.",
      },
      {
        title: "Expect a warning on first run",
        body: "The installers aren’t code-signed yet, so Windows SmartScreen and macOS Gatekeeper will warn you. The Windows and macOS builds are newer and less tested than Linux.",
      },
    ] as Array<{ title: string; body: ReactNode; actions?: ReactNode }>,
  },

  privacy: {
    title: "Privacy Policy",
    updated: "Last updated October 9, 2026.",
    body: (
      <>
        <h2>This website</h2>
        <p>
          havenkeys.net sets no cookies. It is hosted on Vercel, which processes standard request
          logs (such as IP address and browser user agent) to serve the site, and it uses Vercel
          Analytics, which counts page views in aggregate without cookies or cross-site tracking.
          The Download page asks GitHub's public API for the latest release directly from your
          browser, so GitHub also sees that request. If you pick a language with the switcher, the
          site remembers that choice in your browser's local storage; it is never sent anywhere.
          There is no advertising and no other third-party script on the site.
        </p>
        <p>
          The Create account page sends your email address, your language and the version of the
          Terms you accepted to our server at api.havenkeys.net, which emails you a code and, once
          you confirm it, a setup code. The setup code is shown on the page and kept only in the
          page's memory; it is not stored in your browser or in our analytics. Our server records
          your email address, the language you chose (to write to you in it) and the version of the
          Terms you accepted, and sends account notices (the code, the setup code, and when a trial
          is about to end or has ended) through an email provider acting as an operator for us; your
          address is used for nothing else beyond account notices. The sign-up code is kept only as
          a keyed hash, for 15 minutes. To limit abuse, sign-up attempts are counted per network
          address and per email address for one hour. We never send
          marketing email.
        </p>

        <h2>The desktop app and browser extension</h2>
        <p>
          HavenKeys the application contains no telemetry, no analytics, and no crash reporting.
          Your master password and Secret Key are never sent to the server or to the browser
          extension; to sign in, the app sends the server a key derived from them. Your Secret Key
          leaves your device only in ways you choose: on your Recovery Sheet, and when you enter it
          on another device of your own. Vault keys are generated on your device and never leave it
          unencrypted.
        </p>
        <p>
          Every vault belongs to an account on a <code>havenkeys-server</code> that you, or the server you
          created your account on, or your own, runs. That server stores your vault encrypted, which it cannot decrypt, plus
          the metadata it needs to serve it: the vault ID, your account's email address, the
          key-derivation parameters and salt, the wrapped vault key, item revisions, and the number
          and rough size of your items. It also stores your account's plan status (trial, active, frozen) and when it changed. To rate-limit sign-in, it also counts failed attempts per
          account and per network address, and clears them after a successful sign-in. The{" "}
          <Ext href={`${DOCS}server-sync.md`}>server-sync design</Ext> describes this in full. The
          developers also operate a server for other people, run by SAMUEL DA SILVA ROCHA
          DESENVOLVIMENTO DE SOFTWARE LTDA, which is the controller of the personal data listed here
          for accounts on it. That server cannot decrypt vaults either.
        </p>

        <h2>The browser extension</h2>
        <p>
          The HavenKeys extension has one purpose: to fill, save and generate logins from the
          HavenKeys desktop app on the websites you use. To do that it handles the following data,
          and only when you use it:
        </p>
        <ul>
          <li>
            <strong>The address of the page</strong> (and of the frame holding the login form) where
            you open the extension or its suggestions, so the desktop app can find the logins saved
            for that site.
          </li>
          <li>
            <strong>The login fields on that page</strong>: their types, names, labels and nearby
            text, to recognise username, password and one-time-code fields. This is read inside the
            page and never sent anywhere, not even to the desktop app.
          </li>
          <li>
            <strong>Usernames, passwords and one-time codes</strong> you choose to fill, which the
            desktop app sends for that one site, and passwords the extension generates for you.
          </li>
          <li>
            <strong>A username and password you submit on a login form</strong>, so it can ask
            whether to save them. Nothing is saved unless you confirm.
          </li>
        </ul>
        <p>
          Page addresses and the logins you save go only to the HavenKeys desktop app on the same
          computer, through the
          browser's native-messaging channel. It is never sent to us or to any third party. When
          you confirm a save, the desktop app encrypts the login on your device and sends it,
          encrypted, to your own <code>havenkeys-server</code> over HTTPS (plain HTTP is accepted
          only for a server on the same computer). The server cannot decrypt it.
        </p>
        <p>
          The extension keeps no logins, page contents or addresses in browser storage, cookies
          or on disk. Its only stored value is one setting, whether logins are suggested under
          login fields. A login it fills stays in memory only while it fills the page. A login
          waiting to be saved is kept in memory for at most three minutes, and is dropped as soon
          as you save it, dismiss the prompt, or the vault locks. The extension runs on the
          websites you visit so it can offer to save logins and handle passkeys; you can take that
          access back in your browser's extension settings.
        </p>

        <h2>What we never collect</h2>
        <p>
          We, the developers of HavenKeys, receive none of your data. In particular we never
          receive:
        </p>
        <ul>
          <li>Master passwords or Secret Keys</li>
          <li>Vault encryption keys</li>
          <li>Stored passwords, usernames, TOTP secrets, or secure notes</li>
          <li>
            Your browsing history or the content of pages you visit. The extension handles page
            addresses and login forms on your device only, as described above.
          </li>
        </ul>

        <h2>How data is used and shared</h2>
        <p>
          Data is used only to provide the features described above. It is never sold, never used
          or shared for advertising, and never used to determine creditworthiness or for lending.
          Nobody reads your data: we have no access to it, and the server stores your vault only in
          encrypted form.
        </p>
        <p>
          The use of information received by the HavenKeys browser extension adheres to the{" "}
          <Ext href="https://developer.chrome.com/docs/webstore/program-policies/">
            Chrome Web Store User Data Policy
          </Ext>
          , including the Limited Use requirements.
        </p>

        <h2>Keeping and deleting your data</h2>
        <p>
          Your vault is kept until you delete it. Deleting an item removes it from every device. On
          the server, its encrypted content is erased and only a marker remains (the item's random
          ID and the time of deletion), so your other devices know to remove it too. "Remove this
          device" in the desktop app signs the computer out and sets its copy of the vault aside;
          you can then delete that file. You can delete your account yourself from the app;{" "}
          <a href="/delete-account">Delete your account</a> says what is erased and when. Uninstalling the extension removes it completely,
          since it keeps no data of its own. Accounts never activated within 7 days of sign-up are deleted automatically.
        </p>

        <h2>Contact</h2>
        <p>
          Questions about this policy can be opened as an issue on{" "}
          <Ext href={`${GH}/issues`}>GitHub</Ext>.{" "}
          For privacy requests, including deletion:{" "}
          <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a>.
        </p>
      </>
    ),
  },

  deleteAccount: {
    title: "Delete your account",
    updated: "Last updated October 5, 2026.",
    body: (
      <>
        <h2>From the app</h2>
        <p>
          On the desktop: Settings → <strong>Delete account and all data</strong>. On Android:
          Settings → Account → <strong>Delete account and all data</strong>. You confirm with your
          account's email and your master password. Make an encrypted backup first if you may want
          your data later: deletion cannot be undone.
        </p>

        <h2>What is deleted, and when</h2>
        <ul>
          <li>
            <strong>Immediately:</strong> your encrypted vault and items, your devices and sessions,
            your email address, your key-derivation parameters and your account's failed-sign-in counter. The
            copy on the device you used is erased too, and your other devices erase theirs the next time they connect, if that is within 30 days.
          </li>
          <li>
            <strong>Within 30 days:</strong> anonymous fingerprints of your former sessions, kept
            only so your other devices learn the account is gone. They contain no email, name or
            account identifier.
          </li>
          <li>
            <strong>Within 30 days:</strong> copies in database backups and server logs, which
            expire on their own.
          </li>
          <li>
            <strong>Not linked to you:</strong> counts of failed sign-ins kept per network address,
            which never name an account, are not part of the deletion.
          </li>
        </ul>
        <p>
          Backups you exported yourself, and copies set aside on a device by an earlier "Remove
          this device", are yours and are not touched.
        </p>

        <h2>Lost access to your account?</h2>
        <p>
          Email <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a> from the
          address of the account. We will confirm the request and delete the account the same way
          the app does.
        </p>
      </>
    ),
  },

  terms: {
    title: "Terms of Service",
    updated: "Last updated October 9, 2026.",
    body: (
      <>
        <h2>License</h2>
        <p>
          HavenKeys is open-source software, licensed under the{" "}
          <Ext href={`${GH}/blob/main/LICENSE-APACHE`}>Apache License 2.0</Ext>. You may use,
          modify, and redistribute it under its terms.
        </p>
        <p>
          This site, the desktop app and the browser extension embed the Hanken Grotesk, Source
          Serif 4 and JetBrains Mono typefaces, which are licensed separately under the SIL Open Font
          License 1.1. Their copyright notices and that license are in{" "}
          <Ext href={`${GH}/blob/main/THIRD-PARTY-NOTICES.md`}>THIRD-PARTY-NOTICES.md</Ext>.
        </p>

        <h2>No warranty</h2>
        <p>
          HavenKeys is provided "as is," without warranty of any kind, express or implied,
          including but not limited to fitness for a particular purpose. This software has not
          undergone an independent security audit and should not be considered a replacement for
          professionally audited password managers for high-value production use. You use it at
          your own risk.
        </p>

        <h2>Self-hosting</h2>
        <p>
          If you run <code>havenkeys-server</code> yourself, you are solely responsible for its
          deployment, its uptime, and its backups. The server is the authoritative copy of your
          vault; losing it without a tested backup means losing your data. See{" "}
          <Ext href={`${DOCS}self-hosting.md`}>the self-hosting guide</Ext> and{" "}
          <Ext href={`${DOCS}deployment.md`}>the deployment guide</Ext>, particularly its section on
          backups, before storing anything you can't afford to lose.
        </p>

        <h2>Our server and yours</h2>
        <p>
          SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA (“we”) runs a{" "}
          <code>havenkeys-server</code> at api.havenkeys.net. Anyone can instead run their own
          server; downloading the software creates no account with us.
        </p>

        <h2>Accounts, trial and plans</h2>
        <p>
          Creating an account on our server starts a 14-day free trial with no card. After it, the
          account continues on the Personal plan once you subscribe; until subscriptions open, we
          keep accounts open on request. Accounts we invited directly are complimentary with no end
          date unless we tell you otherwise.
        </p>
        <p>
          An account whose trial has ended, or whose payment has lapsed, is <strong>frozen</strong>:
          the server refuses changes, new devices and new passkeys, and the apps stop filling. You
          keep reading your vault on every device, signing in with saved passkeys, and exporting it,
          in plain or encrypted form, at any time. We never delete a frozen vault for being frozen;
          you can delete the account yourself from the app at any time.
        </p>
        <p>
          The service is provided on a best-effort basis, with no service-level agreement, and may
          change or end with notice. Nothing here limits your right to export and leave.
        </p>
      </>
    ),
  },

  pricing: {
    title: "One plan, your keys",
    lede: "HavenKeys is open source and free to self-host. The hosted service pays for the server and the work.",
    plan: "Personal",
    priceSoon: "Price coming soon",
    trial: "14 days free, no card",
    includes: [
      "No limit on logins, cards, identities, notes and passkeys",
      "Desktop, browser extension and Android",
      "Phone-approved sign-in on a new computer",
      "Export at any time, in plain or encrypted form",
    ],
    afterTitle: "After the trial",
    keepsTitle: "Keeps working",
    keeps: [
      "Your vault stays readable on every device",
      "Export, in plain or encrypted form",
      "Signing in with a passkey you already saved",
    ],
    pausesTitle: "Pauses until you subscribe",
    pauses: [
      "Changes to your vault",
      "New devices",
      "Autofill",
      "Saving new passkeys",
    ],
    subscribeSoon: (
      <>
        Subscriptions open soon. Until then, write to{" "}
        <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a> and we will keep
        your account open.
      </>
    ),
    cta: "Create account",
    selfHost: "Or run your own server for free",
  },

  selfHost: {
    title: "Run your own HavenKeys server",
    lede: "Keep your locked vault on a server you control. You need a domain, a small server and a few minutes.",
    needTitle: "What you need",
    needs: [
      "A small Linux server with Docker, Docker Compose v2 and curl — about US$5 a month, or a computer at home that’s always on.",
      "A domain or subdomain you can point at it, like vault.example.com.",
      "Or, instead of both: a Railway account.",
    ],
    stepsTitle: "Four steps with Docker",
    steps: [
      { title: "Point your domain at the server", text: "Add an A record for your domain with the server’s IP address, and open ports 80 and 443.", code: "" },
      { title: "Download the bundle", text: "Six small files: the services, HTTPS, backups and the setup script.", code: "mkdir havenkeys && cd havenkeys\nfor f in compose.yaml Caddyfile .env.example setup.sh restore.sh backup.sh; do\n  curl -fsSLO \"https://raw.githubusercontent.com/rochasamuel/havenkeys/main/deploy/compose/$f\"\ndone\nchmod +x setup.sh restore.sh backup.sh" },
      { title: "Run the setup", text: "It asks for your domain and email, creates the secrets, and starts everything with HTTPS.", code: "./setup.sh" },
      { title: "Create your account", text: "It prints an invite once. Paste it into the HavenKeys app on your computer.", code: "docker compose exec server havenkeys-server admin new-account \\\n  --email you@example.com --server-url https://vault.example.com" },
    ],
    railwayTitle: "Or deploy on Railway",
    railwayBody: "No server to look after: Railway runs HavenKeys and its database for you, for about US$5 a month.",
    railwayCta: "Deploy on Railway",
    railwaySoon: "The one-click Railway template is coming soon. Until then, the guide has the Railway steps.",
    backupsTitle: "Backups are included",
    backupsBody: "The bundle saves a copy of the database every night and keeps the last 14 days in a backups folder. Copy that folder somewhere else regularly, and try a restore once so you know it works.",
    chargeTitle: "You’re in charge",
    chargeBody: "When you run the server, keeping it online and backed up is up to you. HavenKeys can’t recover a vault from a server that was lost without a backup.",
    guideCta: "Read the full guide",
    inviteInstead: "Rather not run a server? Create an account on ours.",
  },

  signup: {
    title: "Create your HavenKeys account",
    lede: "Three steps: confirm your email, then set up the app on your computer or phone.",
    emailLabel: "Email",
    emailHint: "We send a six-digit code to confirm it is yours.",
    terms: (terms: string, privacy: string) => (
      <>
        I have read and accept the <Link to={terms}>Terms</Link> and the{" "}
        <Link to={privacy}>Privacy Policy</Link>.
      </>
    ),
    create: "Create account",
    sending: "Sending…",
    codeTitle: "Check your email",
    codeLede: (email: string) => (
      <>
        We sent a six-digit code to <strong>{email}</strong>. It is valid for 15 minutes. If this address
        already has an account, the email says so instead.
      </>
    ),
    codeLabel: "Code",
    verify: "Continue",
    verifying: "Checking…",
    resend: "Resend code",
    resendIn: "Resend in {s} s",
    changeEmail: "Use another email",
    doneTitle: "Your setup code",
    doneLede:
      "Install HavenKeys, open it, choose “I have a setup code” and paste this code. Then pick a master password and keep your Recovery Sheet safe.",
    copy: "Copy",
    copied: "Copied",
    alsoEmailed: "We also emailed it. It is valid for 24 hours and works once.",
    inviteAria: "Setup code",
    downloadsTitle: "Get the app",
    otherDownloads: "All downloads",
    keepTab: "Keep this tab open until you have pasted the code into the app.",
    errors: {
      email: "That does not look like an email address.",
      terms: "Please accept the Terms and the Privacy Policy to continue.",
      code: "Enter the six digits from the email.",
      invalid: "That code is not valid. Check the email, or request a new code.",
      rate_limited: "Too many attempts. Wait an hour and try again.",
      unavailable: "We could not send the email right now. Try again in a few minutes.",
      closed: "Sign-up opens soon. Write to samuelsilv.rocha@gmail.com and we will set you up meanwhile.",
      emailRejected: "That email address was not accepted.",
      network: "We could not reach the server. Check your connection and try again.",
    },
  },

  notFound: {
    title: "Page not found",
    body: (home: string) => (
      <>
        There's nothing at this address. <Link to={home}>Go to the homepage</Link>.
      </>
    ),
  },
};

export type Messages = typeof en;
