# Product

<!-- impeccable:product-schema 1 -->

Shared by every surface in this monorepo: the desktop app (`apps/desktop`),
the browser extension (`apps/extension`) and the marketing/download site
(`apps/web`). Design tokens are shared through `packages/ui`.

## Platform

web

(The desktop app is Tauri, a native wrapper around a React UI; its design
language is not per-OS native. The extension's popup, options page and
in-page menu are web surfaces.)

## Users

Today the only user is the author, dogfooding HavenKeys as their real password
manager across their own computers and browsers. They are technical, run their
own `havenkeys-server`, and use the product every day: unlocking, searching,
revealing, copying, filling logins and one-time codes in the browser, and
saving new credentials.

No other audience is confirmed. The public site (havenkeys.net) exists, but
nobody else relies on the product yet. Future work must not assume a team,
family or non-technical audience.

## Product Purpose

A personal, local-first password manager with the UX and autofill quality of
1Password, where the user owns everything: keys and plaintext never leave the
device, and the only server is one the user runs, which stores ciphertext it
cannot open. Reads work offline from an encrypted replica; writes go through
the server, which is the single writer.

Success means the author trusts it enough to use it instead of a commercial
manager, and filling a login in the browser is fast enough that they barely
notice the extension.

## Positioning

Two claims together, weighted equally:

1. **1Password-grade experience, on your own server.** Master password plus
   Secret Key, an Emergency Kit, and deliberate, explicit-action autofill,
   with no vendor, account service or subscription.
2. **Small, honest and auditable.** The security core is Rust, and the
   codebase is small enough for one person to read end to end. Security
   documentation says what is and is not protected.

The pairing is the point: polished browser integration without giving up
ownership or legibility.

## Operating Context

- Desktop app lives in the system tray. Closing the window hides it; the vault
  auto-locks (never / 5 / 15 / 30 / 60 min), on sleep and on quit.
- Browser extension (MV3, Chrome and Firefox): a toolbar popup with Fill,
  one-time codes and Lock. There are opt-in in-page suggestions on login, OTP
  and new-password fields, and save/update prompts after form submission.
- Every autofill action requires an explicit user click. Nothing fills on
  page load.
- Webpages are treated as hostile. In-page UI runs next to untrusted DOM and
  must not be spoofable or leak secrets.
- Setup needs an invite from the server operator. A second computer needs the
  email, master password and Secret Key from the Emergency Kit.
- The marketing site is static on Vercel and its download page reads GitHub
  Releases. Installers are unsigned, so OS warnings appear on first run.

## Capabilities and Constraints

- Logins (username, password, websites with match rules, TOTP, notes), secure
  notes, password generator, TOTP codes, in-memory search, 1Password `.1pux`
  import, password history (last 5), master password change, and account
  settings (devices, revoke, sign out, sync).
- Export is not implemented.
- The UI never does cryptography and never holds keys. Secrets reach the UI
  only when the user reveals one field. Copying happens in Rust, and the
  clipboard clears automatically.
- Passwords are never shown by default, and secrets never appear in
  notifications, window titles, URLs or DOM attributes.
- Lock states are explicit (locked / unlocking / unlocked / locking), and the
  UI must always make the current state clear.
- Offline mode is read-only. The UI shows an offline banner and gates writes.
- The desktop app has a dark theme by default, with light and match-system
  options.
- Terminology: vault, item, login, secure note, Secret Key, Emergency Kit,
  master password, `havenkeys-server`, invite, device.

## Brand Commitments

- Name: **HavenKeys**. Domain: havenkeys.net.
- Voice: plain, precise, honest and understated. Never use "unhackable",
  "military-grade", "100% secure" or equivalent claims.
- Any surface that describes the product's security must carry this
  disclaimer verbatim: "This software has not undergone an independent
  security audit and should not be considered a replacement for
  professionally audited password managers for high-value production use."
- All surfaces must be recognizably one product, drawing on the shared
  `packages/ui` tokens.
- App icons live in `apps/desktop/src-tauri/icons/`.

## Evidence on Hand

- Security documentation in `docs/`: threat model, security model, crypto,
  native messaging, autofill, and a self-review in `security-review.md`.
- The product is open source (MIT / Apache-2.0).
- There are no users besides the author, and no testimonials, customers,
  benchmarks, press or independent audit. Future work must not fabricate any
  of these.
- `havenkeys-server` is not deployed anywhere yet, and nothing from the
  extension has been verified in a real browser.

## Product Principles

1. **Security before convenience.** When they conflict, security wins and the
   UI explains why in plain language.
2. **Explicit action for anything sensitive.** Revealing, filling, copying and
   saving happen because the user asked, never on their own.
3. **Honest over impressive.** Claim only what is verified, and make
   limitations as visible as capabilities.
4. **Get out of the way.** Autofill and unlock should feel instant and quiet.
   The best interaction is one the user barely notices.
5. **Legible system.** The user should always know which state the vault is
   in (locked, offline, syncing) and what the product is about to do.
