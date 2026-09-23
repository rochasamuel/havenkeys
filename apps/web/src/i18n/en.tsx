import type { ReactNode } from "react";
import { Link } from "react-router-dom";

/*
 * Every word the site shows, in English. pt-BR.tsx must match this shape
 * exactly (it is typed as Messages), so a string added here and forgotten
 * there fails the typecheck instead of shipping half-translated.
 *
 * Product names that appear in the app's own UI (Secret Key, Emergency Kit,
 * havenkeys-server) stay in English in every locale, because that is what
 * the user will see on screen.
 */

const GH = "https://github.com/rochasamuel/havenkeys";
const DOCS = `${GH}/blob/main/docs/`;

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
    title: "HavenKeys — a password manager you actually own",
    description:
      "HavenKeys is a local-first password manager: a desktop app with a Rust security core, a browser extension, and a server you run yourself.",
  },

  common: {
    disclaimer:
      "This software has not undergone an independent security audit and should not be considered a replacement for professionally audited password managers for high-value production use.",
    downloadCta: "Download HavenKeys",
  },

  nav: {
    homeAria: "HavenKeys home",
    mainAria: "Main",
    howItWorks: "How it works",
    browser: "Browser",
    security: "Security",
    githubAria: "HavenKeys on GitHub",
    download: "Download",
    switchShort: "PT",
    switchAria: "Ver o site em português",
  },

  footer: {
    tagline: "A password manager you run yourself.",
    aria: "Footer",
    download: "Download",
    security: "Security",
    github: "GitHub",
    privacy: "Privacy",
    terms: "Terms & license",
    license: "MIT or Apache-2.0",
    languageAria: "Language",
  },

  home: {
    heroTitle: (
      <>
        Passwords that only ever leave home <em>locked</em>.
      </>
    ),
    heroLede:
      "Your keys stay on your devices; the only server is one you run, and it holds ciphertext it has no key for.",
    seeHow: "See how it works",
    heroMeta: "Free and open source · Windows, macOS and Linux · Chrome and Firefox",
    desktopAlt:
      "The HavenKeys desktop app: a sidebar with the vault's sections, a list of logins, and the Fernway login open with its username, hidden password and a live one-time code.",
    popupAlt: "The HavenKeys browser popup: two saved Fernway logins, a Fill button, and a one-time code.",
    menuAlt: "The HavenKeys in-page menu offering two saved logins.",

    journeyTitle: "Follow one password home.",
    journeyLede:
      "HavenKeys is a password manager with careful autofill, one-time codes and a vault that works offline. What sets it apart is what happens to a password between the moment you type it and the moment it’s filled. Here’s that trip, one step at a time.",

    browserTitle: "Autofill that waits for your click.",
    browserLede:
      "The extension for Chrome and Firefox reads the form the way you do, offers what’s saved for that site, and does nothing until you choose. These are the real menus.",

    desktopTitle: "A vault that lives on your desk.",
    desktopLede:
      "The desktop app holds the keys. It sits in the tray, locks itself when you step away, and does all the cryptography in its Rust core. The interface never sees a key, and a password reaches the screen only when you reveal it.",
    features: [
      { term: "Logins", text: "Usernames, passwords, websites with match rules, one-time codes and notes." },
      { term: "Secure notes", text: "Recovery codes, passphrases, anything that isn’t a login. Encrypted whole." },
      { term: "One-time codes", text: "SHA-1, SHA-256 or SHA-512, six or eight digits. Paste an otpauth:// link once." },
      { term: "Password generator", text: "Length and character sets are up to you. Random from the OS, with no bias." },
      { term: "Search", text: "Titles, usernames and websites, searched in memory. No plaintext index on disk." },
      { term: "Password history", text: "The last five passwords of every login, even ones changed from the browser." },
      { term: "Import from 1Password", text: "Bring a .1pux export: logins, notes and one-time codes come across." },
      {
        term: "Auto-lock",
        text: "After 5 to 60 minutes idle, on sleep and on quit. On Windows and Linux, when your session locks too.",
      },
    ],

    paperTitle: "Two secrets. One of them lives on paper.",
    paperP1:
      "Your master password is the one you remember. Your Secret Key is 128 random bits made on your device when you set up. It’s stored on each of your computers and printed on your Emergency Kit, and it never goes to the server.",
    paperP2:
      "So a stolen copy of the server’s database isn’t a password-guessing exercise. Without the Secret Key, an attacker has to guess both.",
    paperWarn:
      "There’s no account recovery. Lose the kit and every device that holds the key, and the vault is gone. That’s the price of nobody else being able to open it.",

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
    securityOverview: "Read the security overview",

    closerTitle: (
      <>
        Bring your passwords <em>home</em>.
      </>
    ),
    closerLede: "Install the desktop app, point it at your server, add the extension.",
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
            HavenKeys syncs through <code>havenkeys-server</code>, a small server you run yourself.
            There’s no vendor account and no company in the middle holding your vault.
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
      secretWhere: "On your devices and your Emergency Kit",
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
      where: "on hardware you choose",
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
        body: "In-page suggestions are off until you turn them on. Without them, the toolbar button fills the current tab, with the same origin checks and nothing running on pages you didn’t click.",
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
    aria: "Illustration of a HavenKeys Emergency Kit",
    caption: "Illustration. The key shown is made up.",
  },

  security: {
    heroTitle: "Security you can read end to end.",
    heroLede:
      "HavenKeys is small on purpose. The cryptography comes from established Rust libraries, the rules are written down, and the attacks it claims to stop are tests in the code. Here’s the whole design in one page.",

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
        where: "Hardware you choose",
        holds: "Ciphertext, plus your email and item counts, sizes and times",
        limit: "No key to any of it. It can delete data, so keep backups.",
      },
    ],

    permTitle: "An extension that asks for less.",
    permLede:
      "Most of the value, filling with origin checks, works with the tab you click on. Access to every site is what a malicious page or a compromised build would want most, so that’s yours to grant, not a condition of installing.",
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
        why: "Put the fill script into that tab, or register it when in-page suggestions are on.",
      },
      {
        name: "https://*/*, http://*/*",
        optional: true,
        why: "In-page suggestions and save prompts. Asked for only when you turn them on, and you can narrow it to chosen sites.",
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
    ],
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
    platforms: {
      windows: { label: "Windows", format: ".msi installer" },
      macos: { label: "macOS", format: ".dmg disk image" },
      "linux-appimage": { label: "Linux", format: ".AppImage, runs anywhere" },
      "linux-deb": { label: "Debian & Ubuntu", format: ".deb package" },
    },
    beforeTitle: "Before you install",
    steps: [
      {
        title: "You’ll need a server",
        body: (
          <>
            HavenKeys stores your vault on a <code>havenkeys-server</code>. Run your own with the{" "}
            <Ext href={`${DOCS}deployment.md`}>deployment guide</Ext>, or get an invite from someone
            who runs one. Set up backups before you store anything real.
          </>
        ),
      },
      {
        title: "The extension is built from source",
        body: (
          <>
            The browser extension and its native messaging host aren’t in these installers yet. The{" "}
            <Ext href={`${GH}#readme`}>README</Ext> has the steps.
          </>
        ),
      },
      {
        title: "Expect a warning on first run",
        body: "The installers aren’t code-signed yet, so Windows SmartScreen and macOS Gatekeeper will warn you. The Windows and macOS builds are newer and less tested than Linux.",
      },
    ] as Array<{ title: string; body: ReactNode }>,
  },

  privacy: {
    title: "Privacy Policy",
    updated: "Last updated September 23, 2026.",
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
          There is no advertising and no other third-party script.
        </p>

        <h2>The desktop app and browser extension</h2>
        <p>
          HavenKeys the application contains no telemetry, no analytics, and no crash reporting.
          Your master password and Secret Key are never sent to the server or to the browser
          extension; to sign in, the app sends the server a key derived from them. Your Secret Key
          leaves your device only in ways you choose: on your Emergency Kit, and when you enter it
          on another device of your own. Vault keys are generated on your device and never leave it
          unencrypted.
        </p>
        <p>
          Every vault belongs to an account on a <code>havenkeys-server</code> that you, or whoever
          invited you, runs. That server stores your vault encrypted, which it cannot decrypt, plus
          the metadata it needs to serve it: the vault ID, your account's email address, the
          key-derivation parameters and salt, the wrapped vault key, item revisions, and the number
          and rough size of your items. To rate-limit sign-in, it also counts failed attempts per
          account and per network address, and clears them after a successful sign-in. The{" "}
          <Ext href={`${DOCS}server-sync.md`}>server-sync design</Ext> describes this in full. We do
          not operate a hosted server and have no access to yours.
        </p>

        <h2>What we never collect</h2>
        <ul>
          <li>Master passwords</li>
          <li>Vault encryption keys</li>
          <li>Stored passwords, usernames, TOTP secrets, or secure notes</li>
          <li>Browsing history or the contents of pages you visit</li>
        </ul>

        <h2>Contact</h2>
        <p>
          Questions about this policy can be opened as an issue on{" "}
          <Ext href={`${GH}/issues`}>GitHub</Ext>.
        </p>
      </>
    ),
  },

  terms: {
    title: "Terms of Service",
    updated: "Last updated September 23, 2026.",
    body: (
      <>
        <h2>License</h2>
        <p>
          HavenKeys is open-source software, dual-licensed under the{" "}
          <Ext href={`${GH}/blob/main/LICENSE-MIT`}>MIT License</Ext> and the{" "}
          <Ext href={`${GH}/blob/main/LICENSE-APACHE`}>Apache License 2.0</Ext>. You may use,
          modify, and redistribute it under the terms of either.
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
          <Ext href={`${DOCS}deployment.md`}>the deployment guide</Ext>, particularly its section on
          backups, before storing anything you can't afford to lose.
        </p>

        <h2>No service, no account</h2>
        <p>
          We do not operate a hosted version of HavenKeys and do not maintain accounts on your
          behalf. There is no subscription, no SLA, and no support obligation implied by
          downloading this software.
        </p>
      </>
    ),
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
