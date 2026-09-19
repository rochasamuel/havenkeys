# HavenKeys

A local-first personal password manager: a Tauri desktop app with a Rust
security core. It works offline, needs no account, and runs no server.

> **This software has not undergone an independent security audit and should
> not be considered a replacement for professionally audited password managers
> for high-value production use.**

## Status

| Area | State |
|---|---|
| Security core (crypto, vault, lock, TOTP, generator) | Implemented, 82 Rust tests |
| Desktop app (Tauri + React) | Implemented; builds and runs on Linux (WSLg) |
| Domain matching + origin binding for autofill | Implemented in the core ([docs/autofill.md](docs/autofill.md)) |
| Browser extension, native messaging | Not started (next phases) |
| Import | 1Password `.1pux` (logins, notes, TOTP; other item kinds become secure notes) |
| Export | Not implemented |

What the desktop app does today:

* Create a vault, unlock and lock it; auto-lock (never / 5 / 15 / 30 / 60 min), lock on sleep and on quit
* Lives in the system tray: closing the window hides it; the tray menu opens, locks or quits
* Dark theme by default, with light and match-system options
* Logins (username, password, websites with match rules, TOTP, notes)
* Secure notes
* In-memory search over titles, usernames and websites
* Password generator (OS CSPRNG, unbiased)
* TOTP codes (SHA-1/256/512, 6/8 digits, `otpauth://` import)
* Copy to clipboard from Rust with automatic clearing
* Master password change (rewraps the vault key; items are untouched)
* Import from 1Password (`.1pux`): Settings → Import from 1Password

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

Read these before trusting it with anything:

* [Threat model](docs/threat-model.md): what is and isn't defended
* [Security model](docs/security-model.md): how it's enforced
* [Cryptography](docs/crypto.md): key hierarchy, formats, parameters
* [Architecture](docs/architecture.md)
* [Security review](docs/security-review.md): findings from reviewing this implementation
* [Development](docs/development.md): building, testing, auditing

## Quick start

```sh
# Linux prerequisites (Debian/Ubuntu)
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config

pnpm install
cargo test                 # security core tests
pnpm dev                   # run the desktop app
```

Details, including macOS/Windows, are in [docs/development.md](docs/development.md).

## Repository layout

```text
crates/havenkeys-core/     Rust security core (all crypto, vault, lock, TOTP, generator)
apps/desktop/src-tauri/    Tauri shell: command allowlist, clipboard, auto-lock timer
apps/desktop/src/          React + TypeScript UI (no cryptography)
docs/                      Threat model, security model, crypto, architecture, review
```

## Licence

MIT OR Apache-2.0.
