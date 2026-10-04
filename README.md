# HavenKeys

A personal password manager: a Tauri desktop app with a Rust security core,
a browser extension, an Android app, and a small server you run yourself. Your vault lives on
your own server, encrypted with keys it never sees. Reads work offline;
changes need the server.

> **This software has not undergone an independent security audit and should
> not be considered a replacement for professionally audited password managers
> for high-value production use.**

## Status

| Area | State |
|---|---|
| Security core (crypto, vault, lock, TOTP, generator) | Implemented and tested |
| Desktop app (Tauri + React) | Implemented; builds and runs on Linux (WSLg) |
| Domain matching + origin binding for autofill | Implemented in the core ([docs/autofill.md](docs/autofill.md)) |
| Native messaging (native host + desktop bridge) | Implemented ([docs/native-messaging.md](docs/native-messaging.md)) |
| Browser extension (MV3, Chrome + Firefox) | Toolbar popup with Fill; in-page suggestions for logins, one-time codes and generated passwords (on by default, can be turned off); save/update prompts ([docs/autofill.md](docs/autofill.md)) |
| Passkeys (WebAuthn, ES256) | Implemented and unit-tested: create, save to a login, sign in (chooser and passkey autofill), an automatic upgrade after a password fill, a field-menu hint for known passkey sites, delete from the desktop. Not yet checked against real sites in a browser ([docs/security-review.md](docs/security-review.md), Passkeys) |
| Import | 1Password `.1pux` (logins, notes, TOTP; other item kinds become secure notes) |
| Export | Not implemented |
| Secret Key + Emergency Kit | Every vault needs the master password **and** a 128-bit Secret Key ([docs/server-sync.md](docs/server-sync.md)) |
| Account server (`havenkeys-server`) | Implemented and tested against Postgres; deployed ([docs/deployment.md](docs/deployment.md)). The backup restore drill (§5) and real cross-device use are not yet done — see [docs/roadmap.md](docs/roadmap.md) §3 |
| Marketing/download website (havenkeys.net) | Implemented; static site on Vercel, download page reads GitHub Releases ([docs/website.md](docs/website.md)) |
| Sync between your computers | Through your own server: it is the single writer, and each device keeps an encrypted read-only replica ([docs/server-sync.md](docs/server-sync.md)) |
| Android app | **Status: early release (Android M4).** M1 (sign-in, unlock, browsing, login autofill) is unit-tested and was tried on one phone (Galaxy S24+, Android 16); M2 (editing, saving from Autofill), M3 (passkeys) and M4 (cards and identity in Autofill) are unit-tested but have not run on a device, and the instrumented test suite has not run yet ([docs/android.md](docs/android.md), manual checklists in [docs/security-review.md](docs/security-review.md), Android M1–M4) |

What the desktop app does today:

* Create a vault, unlock and lock it; auto-lock (never / 5 / 15 / 30 / 60 min), lock on sleep and on quit
* Lives in the system tray: closing the window hides it; the tray menu opens, locks or quits
* Dark theme by default, with light and match-system options
* English and Brazilian Portuguese, in both the desktop app and the browser
  extension. The desktop app follows the OS language by default, or a choice
  in Settings; the extension follows the browser's language
* Logins (username, password, websites with match rules, TOTP, notes)
* Secure notes
* An **Identity** per account (name, documents such as CPF and RG, contact,
  address, anything else in custom fields): created automatically, shown and
  edited in the desktop app, each value copyable. Filling web forms from it
  comes later
* **Cards** (holder name, number, verification number, expiry) with the
  network's logo, detected from the number; numbers stay masked until you
  reveal them, and 1Password credit cards import as cards
* A **HavenKeys Account** item pinned first in All items: your email,
  server, account ID and Secret Key, to read or copy when setting up
  another computer (read-only; never stored in the vault or sent to the
  browser extension)
* In-memory search over titles, usernames and websites
* Password generator (OS CSPRNG, unbiased)
* TOTP codes (SHA-1/256/512, 6/8 digits, `otpauth://` import)
* Copy to clipboard from Rust with automatic clearing
* Master password change (rewraps the vault key; items are untouched)
* Import from 1Password (`.1pux`): Settings → Import from 1Password
* Browser extension connection through native messaging (opt-in: Settings →
  Browser extension)
* Password history: a password replaced from the app or the browser stays
  recoverable (last 5)
* Passkeys: a login shows the passkeys it holds (site, account, created),
  and each can be deleted
* From version 0.9.0: checks GitHub Releases for signed updates (Settings →
  Updates, on by default, can be turned off) and installs one on your click,
  on Windows, macOS and the Linux AppImage. `.deb`/`.rpm` installs get a
  notice with a link instead. 0.8.0 and earlier have no updater and must be
  replaced by hand with 0.9.0, over the existing install

What the browser extension does:

* Toolbar popup: logins for the current site, **Fill**, one-time codes, lock
* In-page suggestions (on by default; the extension's options page can turn
  them off without affecting save prompts or passkeys): click a
  login field to pick a login, an OTP field to fill a code, or a new-password
  field to fill a password generated by the desktop app
* Offers to save new logins and changed passwords after you submit a form
* Fills sign-up and checkout forms from your Identity (name, email, phone,
  address). Documents such as CPF are filled only after you confirm them, and
  only on https pages
* Fills checkouts from your saved Cards (https pages only, payment
  processors' card frames included) after you pick one, and offers to save a
  card you typed into a checkout once you confirm
* Passkeys, on the sites the extension has access to (all by default): save a passkey a site offers to a login
  in HavenKeys, and sign in with it from a chooser or from the field menu.
  The private key is created, stored and used only in the desktop's Rust
  core, and the site is checked in Rust against the page the browser
  reports. "Use another device" always hands the request to the browser.
* Right after HavenKeys fills a password, a site that offers its own
  automatic passkey upgrade gets one saved to that same login — silently by
  default, or with a card if you turn that off in Settings. On sites known
  to support passkeys where you have a saved password but no passkey yet,
  the field menu offers a link to that site's passkey help
  ([docs/autofill.md](docs/autofill.md) §Passkeys).
* Logins are only ever offered on the sites they are saved for, and that is
  checked in Rust

Android (early release): download the APK from the website's download page or from GitHub Releases
(`android-v*`); it is signed with the HavenKeys release key.

What the Android app does (early release; tried on one phone, the instrumented tests have not run yet):

* Sign in by scanning the Emergency Kit's QR code (or typing it), or
  activate from an invite
* Approve a new desktop's sign-in: Settings → "Sign in a new device", scan
  its QR code, check the name and place, and tap Allow (behind biometrics)
* Unlock with the master password, or with a fingerprint or face once turned
  on in Settings. The master password is asked for again after 14 days, after
  a restart, and when a fingerprint or face is added
* Auto-lock (never / 5 / 15 / 30 / 60 min) and lock when the screen turns off
  (on by default)
* Browse and search the vault offline; reveal one field at a time; copy with
  automatic clearing; live TOTP codes; password generator
* Android Autofill for logins and TOTP codes, in browsers (Chrome, Firefox
  and the other browsers Google's privileged list names, by the page's
  domain, checked in Rust) and in apps (by the app's package and signing
  certificate: a binding you confirm, or the website's Digital Asset Links
  file). Nothing is filled without a tap. "Confirm before filling" keeps the
  logins' values away from Android until you pick one
* Cards and your identity in Autofill, and saving a typed card
* No screenshots of HavenKeys, no backups of its data, English and Brazilian
  Portuguese
* Creating, editing and deleting logins, secure notes and cards, and editing
  the identity (needs the server); saving a login typed into a browser or an
  app after you confirm Android's save sheet
* Passkeys through Credential Manager (Android 14+)
* Not yet: editing custom fields, scanning a TOTP QR code, saving a login
  typed while locked, an in-app updater

Setting up the first computer needs an invite from whoever runs the server
(`havenkeys-server admin new-account`); see
[docs/deployment.md](docs/deployment.md). A second computer needs the email,
the master password and the Secret Key from your Emergency Kit, or the
phone: scan the new desktop's QR code from Settings → "Sign in a new device"
and tap Allow (see [docs/security-model.md](docs/security-model.md) §23).

## How it protects your data

* Everything about an item is encrypted with AES-256-GCM, including its title,
  username, URLs and type. The only plaintext in the database is what's
  needed to unlock it (see [docs/security-model.md](docs/security-model.md) §4).
* The vault key is random. It is wrapped by a key derived from your master
  password with Argon2id (128 MiB, t=4, p=4 by default).
* Every ciphertext is bound to its vault, item and role, so blobs can't be
  swapped or replayed into another slot without detection.
* The React UI never sees keys and does no cryptography. Secrets reach it
  only when you reveal one field. Copying happens in Rust.
* The server stores ciphertext it has no key for, and identifies every
  request from its session token rather than from anything the client claims.
  It can still **destroy** data — it is the authority on what your vault
  contains — which is why tested backups are a prerequisite, not a
  nicety ([docs/deployment.md](docs/deployment.md) §5).

Read these before trusting it with anything:

* [Threat model](docs/threat-model.md): what is and isn't defended
* [Security model](docs/security-model.md): how it's enforced
* [Cryptography](docs/crypto.md): key hierarchy, formats, parameters
* [Architecture](docs/architecture.md)
* [Server sync and the Secret Key](docs/server-sync.md): the replica model, the account, and what the server can and cannot do
* [Deployment](docs/deployment.md): running the server, upgrading it together with the desktops (§6.1), and the backup restore drill that has to pass before you trust it
* [Native messaging](docs/native-messaging.md): browser ↔ desktop protocol and its checks
* [Autofill](docs/autofill.md): field detection, matching rules, in-page UI security
* [Android](docs/android.md): building and running the Android app, and its manual checklist
* [Security review](docs/security-review.md): findings from reviewing this implementation
* [Reporting a vulnerability](SECURITY.md): please report privately, not in a public issue
* [Development](docs/development.md): building, testing, auditing
* [Roadmap](docs/roadmap.md): what is left, including the proposed standalone extension mode

## Quick start

```sh
# Linux prerequisites (Debian/Ubuntu)
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config

pnpm install
cargo test                 # security core tests
scripts/test-server.sh     # server + sync client, against a disposable Postgres (needs Docker)
pnpm dev                   # run the desktop app
```

Browser extension: with an installed desktop app nothing else is needed. The
app registers its bundled native host with your browsers each time it starts
([docs/native-messaging.md](docs/native-messaging.md) §2). From source:

```sh
pnpm build:host                      # native messaging host (release)
scripts/install-native-host.sh       # register it with installed browsers
pnpm build:extension                 # apps/extension/dist/{chrome,firefox}
```

Then load `apps/extension/dist/chrome` as an unpacked extension (or the
Firefox build as a temporary add-on). See
[docs/native-messaging.md](docs/native-messaging.md) §2.

Details, including macOS/Windows, are in [docs/development.md](docs/development.md).

## Repository layout

```text
crates/havenkeys-core/          Rust security core (all crypto, vault, lock, TOTP, generator, origin binding)
crates/havenkeys-protocol/      Bridge wire protocol: typed messages, framing, socket endpoint
crates/havenkeys-bridge/        Desktop side of the browser bridge: authorization, rate limits, socket server
crates/havenkeys-native-host/   Native messaging host launched by the browser (relay only, no vault access)
crates/havenkeys-server/        The account server: blind relay for encrypted items, Postgres, admin CLI
crates/havenkeys-sync-client/   HTTP client for that server; treats every answer as hostile
crates/havenkeys-client/        Account, session and sync for every app: activation, unlock, sync, writes, devices, removal (no Tauri, no UI)
crates/havenkeys-mobile/        UniFFI API the Android app calls (and later iOS): every secret checked in Rust
apps/desktop/src-tauri/         Tauri shell: command allowlist, clipboard, auto-lock timer
apps/desktop/src/               React + TypeScript UI (no cryptography)
apps/extension/                 MV3 browser extension (background worker, popup)
apps/android/                   Android app: Kotlin, Jetpack Compose, AutofillService
packages/protocol/              TypeScript mirror of the wire protocol with strict validators
packages/ui/                    Shared design tokens (desktop, extension, and a future mobile app)
scripts/                        Native host registration (Linux/macOS, Windows)
docs/                      Threat model, security model, crypto, architecture, review
```

## Licence

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).

The desktop app and the website embed the Hanken Grotesk and JetBrains Mono
typefaces, which are licensed separately under the SIL Open Font License 1.1.
Their notices and that licence are in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), which ships inside the
desktop bundle alongside the two licences above.
