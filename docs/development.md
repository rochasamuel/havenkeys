# Development

## Prerequisites

* Rust ≥ 1.88 (`rustup`)
* Node.js ≥ 20 and pnpm 10
* Tauri system dependencies:
  * **Debian/Ubuntu:** `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config libpipewire-0.3-dev libclang-dev libgbm-dev`
    `libpipewire-0.3-dev`, `libclang-dev` and `libgbm-dev` are for screen
    capture (the one-time code QR scan); the built app needs
    `libpipewire-0.3` at runtime, which current desktop distributions ship.
    The AppImage build likewise relies on the host's `libpipewire-0.3` and
    `libgbm` already being present; it does not bundle them.
  * **macOS:** Xcode command-line tools
  * **Windows:** Microsoft C++ Build Tools and WebView2 (preinstalled on Windows 11)

The security core, protocol, bridge, native host, server, sync client and
client crates need only Rust. They are the Cargo workspace's `default-members`, so
`cargo test` works without the WebKit libraries.

The server and sync-client suites additionally need a **Postgres**, because
what they test is SQL-level (account isolation, row locks, one transaction
per write) and a mock would not test it. `scripts/test-server.sh` starts a
disposable one in Docker and creates a fresh database per test. It also runs
`havenkeys-client`'s tests (`cargo test -p havenkeys-client`), including a
round trip of two clients against the real server:

```sh
scripts/test-server.sh              # or: pnpm test:server
docker rm -f havenkeys-test-pg      # when you are done with it
```

`cargo test -p havenkeys-mobile --features server-tests --test round_trip` — the phone's writes, conflict and Autofill save against the real server (needs `scripts/test-server.sh`).

Running the server itself, and the backup drill that has to pass before it
holds a real vault, are in `docs/deployment.md`.

The Android app needs a JDK 17, the Android SDK and NDK, three Rust Android
targets and `cargo-ndk`; `docs/android.md` lists them, with the exports the
commands below assume.

## Commands

| Task | Command |
|---|---|
| Install JS deps | `pnpm install` |
| Run desktop app (dev) | `pnpm dev` |
| Build installers | `pnpm build` |
| Rust tests (core, protocol, bridge, native host, OS lock) | `cargo test` |
| Server, sync client and client tests (needs Docker) | `scripts/test-server.sh` |
| Regenerate design tokens after editing them | `pnpm --filter @havenkeys/ui generate` |
| Rust lint (same crates) | `pnpm lint:rust` |
| Native host (release) | `pnpm build:host` |
| Register native host (source builds; an installed app registers its own) | `scripts/install-native-host.sh` (Windows: `scripts\install-native-host.ps1`) |
| Desktop installers, with the native host bundled | `pnpm build` (runs `scripts/build-native-host-sidecar.mjs`, then `tauri build --config src-tauri/tauri.bundle.conf.json`) |
| Extension build | `pnpm build:extension` → `apps/extension/dist/{chrome,firefox}` |
| Extension store zips | `pnpm package:extension` → `apps/extension/dist/havenkeys-{chrome,firefox}-<version>.zip` (builds first; fails if `manifest/base.json` and `package.json` versions differ) |
| Desktop crate lint | `cargo clippy -p havenkeys-desktop -- -D warnings` (needs WebKit libs) |
| TS type check (desktop UI, extension, protocol) | `pnpm typecheck` |
| TS tests | `pnpm -r test` |
| Layout check, both languages (dev-only, not shipped) | `pnpm ui:check` |
| KDF benchmark | `cargo run --release -p havenkeys-core --example kdf_bench` |
| Rust advisories | `cargo audit` |
| Rust policy (advisories, licences, sources) | `cargo deny check` |
| JS advisories | `pnpm audit` |
| Check the Windows-only code from Linux | `cargo clippy -p havenkeys-oslock --target x86_64-pc-windows-gnu -- -D warnings` |
| Mobile API tests, with the Android attack regressions | `cargo test -p havenkeys-mobile --features testing` |
| Android: Rust library + Kotlin bindings | `scripts/build-android.sh` (`--release` for release builds); commit the bindings it changes |
| Android: lint, unit tests, Android lint, release APK | `cd apps/android && ./gradlew detekt testGithubDebugUnitTest lintGithubDebug assembleGithubRelease` |
| Android: refresh the privileged browser list | `scripts/update-android-browsers.sh`, then review the diff |
| Android: rebuild the bundled fonts | `uvx --from fonttools==4.66.1 python scripts/build-android-fonts.py`; commit `apps/android/app/src/main/res/font` |

Install the audit tools with `cargo install cargo-audit cargo-deny --locked`.

### Type-checking the Tauri crate without WebKit headers

The `-sys` crates only query `pkg-config` during `cargo check`, so a stub that
answers every query lets you type-check (not link) the desktop crate on a
machine without the GTK/WebKit dev packages:

```sh
mkdir -p /tmp/fakepc && cat > /tmp/fakepc/pkg-config <<'EOF'
#!/bin/sh
for a in "$@"; do case "$a" in --modversion) echo 99.0; exit 0;; --version) echo 0.29.2; exit 0;; esac; done
exit 0
EOF
chmod +x /tmp/fakepc/pkg-config
PKG_CONFIG=/tmp/fakepc/pkg-config PATH=/tmp/fakepc:$PATH cargo clippy -p havenkeys-desktop -- -D warnings
```

## Running on Windows (native)

Build in a normal Windows folder, not inside `\\wsl.localhost\...`: building
over the WSL share is slow and breaks file watching.

1. **Install the tools** (once):
   * [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
     with the **Desktop development with C++** workload
   * [Rust](https://rustup.rs) (`rustup-init.exe`, default MSVC toolchain)
   * [Node.js LTS](https://nodejs.org), then in a new terminal: `npm install -g pnpm`
   * [Git for Windows](https://git-scm.com/download/win)
   * WebView2 is already part of Windows 11.
2. **Get the code.** In PowerShell, clone straight from the WSL repository:
   ```powershell
   # Git refuses repos owned by another user (the WSL user) unless trusted.
   # Trust only this one path, not every repository:
   git config --global --add safe.directory '%(prefix)///wsl.localhost/Ubuntu/home/sams/havenkeys'
   git clone \\wsl.localhost\Ubuntu\home\sams\havenkeys C:\dev\havenkeys
   cd C:\dev\havenkeys
   ```
   Replace `Ubuntu` with your distro name (`wsl -l`) if it differs. To pull
   new commits later: `git pull`.
3. **Run it:**
   ```powershell
   pnpm install
   pnpm dev
   ```
   The first build compiles all Rust dependencies and takes a few minutes.
4. **Installer (optional):** `pnpm build` produces MSI and NSIS installers
   under `target\release\bundle\`.

The Windows app keeps its own vault in `%APPDATA%\com.havenkeys.desktop\`,
separate from the WSL one. Import your `.1pux` again there.

## Where the vault lives

| OS | Path |
|---|---|
| Linux | `~/.local/share/com.havenkeys.desktop/vault.sqlite3` |
| macOS | `~/Library/Application Support/com.havenkeys.desktop/vault.sqlite3` |
| Windows | `%APPDATA%\com.havenkeys.desktop\vault.sqlite3` |

Deleting this file deletes the vault. There is no recovery without the master
password.

## Adding a Tauri command

A command must be added in **three** places, or it will not be callable:

1. `apps/desktop/src-tauri/src/commands.rs` (implementation)
2. `apps/desktop/src-tauri/build.rs` (`COMMANDS`, which generates the permission)
3. `apps/desktop/src-tauri/capabilities/main.json` (grants `allow-<command>`)

Then register it in `generate_handler!` in `lib.rs` and add a typed wrapper in
`apps/desktop/src/lib/api.ts`. Update the command table in
`docs/security-model.md` §7.

## Adding a bridge request

The browser can reach the vault only through the bridge, so every new request
widens the attack surface. Add one only with a threat-model entry, and then:

1. `crates/havenkeys-protocol/src/message.rs`: request and result variants,
   and their limits.
2. `packages/protocol/src/index.ts`: the TypeScript types and validator, kept
   exactly in sync.
3. `crates/havenkeys-bridge/src/dispatch.rs`: map it to an origin-bound core
   function. Choose its rate class in `server.rs`.
4. Tests in `crates/havenkeys-bridge/tests/bridge.rs`: wrong origin, locked,
   integration off, malformed input.
5. The request table in `docs/native-messaging.md` §3.

pnpm 10 runs no dependency install scripts unless they are approved. The
warning about esbuild's install script is expected: esbuild works without it,
because its platform binary comes from an optional dependency. Leave it
unapproved.

## Adding a translated string

HavenKeys speaks English and Brazilian Portuguese (`pt-BR`). Each app keeps
every string it shows in `src/i18n/en.ts`, typed as `Messages`, and
`src/i18n/pt-BR.ts` typed as that same `Messages` — a string added to one and
forgotten in the other fails `pnpm typecheck` instead of shipping
half-translated. Parameterised strings are functions
(`(n: number) => string`), never template interpolation of raw values into a
stored string.

1. Add the English string to `apps/<app>/src/i18n/en.ts` (nested under the
   view/component it belongs to) and the Portuguese one at the same path in
   `pt-BR.ts`. Keep terminology consistent: *senha* (password), *cofre*
   (vault), *login*, *código de verificação* (one-time code), *chave de
   acesso* (passkey). Never translate product names (HavenKeys, Secret Key,
   Recovery Sheet, havenkeys-server), URLs, or key names, and never use `tu` —
   Brazilian Portuguese here is *você*.
2. Use it from `t.<path>` — `useI18n().t` in the desktop React tree
   (`apps/desktop/src/i18n/context.tsx`), or the module-level `t` in the
   extension (`apps/extension/src/i18n/index.ts`, resolved once per page:
   popup, options, background, content script, and each inline frame). Insert
   it as React text or with `textContent`; never `innerHTML` or
   `dangerouslySetInnerHTML`.
3. An error surfaced from Rust arrives as `{ code, message }`. Add a case to
   `t.errors.codes` (desktop: `apps/desktop/src/i18n/en.ts`, matched in
   `errors.ts`) or `t.errors.bridge` (extension:
   `apps/extension/src/i18n/en.ts`) only if the UI needs its own wording for
   that code — an unrecognised code falls back to Rust's English `message`,
   which is intentional (Rust itself is never translated).
4. Never let a longer Portuguese string clip: prefer `min-width` and wrapping
   over fixed widths, and do not pair `white-space: nowrap` with `overflow:
   hidden` on a label. Ellipsis truncation is reserved for user data (titles,
   usernames, URLs), marked `data-truncate`; it must never apply to UI copy.
5. Run `pnpm --filter @havenkeys/<app> typecheck` (key parity) and
   `pnpm --filter @havenkeys/<app> test` (locale-resolution and any logic
   tests force `en`, so they keep passing). Then run `pnpm ui:check` (below)
   to confirm nothing clips in either language.

The extension has no language switcher: it follows
`chrome.i18n.getUILanguage()` (any `pt*` tag → `pt-BR`, else `en`). The
manifest's own `name`/`description` come from `manifest/_locales/{en,pt_BR}`
instead, because the browser reads them before any script runs. The desktop
app follows the OS language by default; Settings → Language offers Automatic
/ English / Português (Brasil), stored as `"auto" | "en" | "pt-BR"` in
`localStorage["hk-locale"]` (a non-sensitive UI preference, guarded so a
missing, invalid, or throwing read just means Automatic). Changing it also
asks Rust to relabel the tray menu (`set_ui_language`, added the same way as
any other Tauri command, above — it accepts only `"en"` and `"pt-BR"`).

### `pnpm ui:check`

A Playwright layout check (`tools/ui-check/`, dev dependency only — nothing
here ships) renders every extension page state (popup, inline menu variants,
save prompt, passkey prompt, options) and every desktop screen (welcome,
unlock, vault list/detail/editor, generator, settings) with `chrome.*` and
Tauri's `invoke` stubbed, in English and Portuguese, in light and dark
theme, at real sizes (the desktop window's minimum and default size from
`tauri.conf.json`). It fails when a visible element that hides overflow has
clipped text (`scrollWidth`/`scrollHeight` past its box, unless the text is
marked `data-truncate` as user data) or spills out of the viewport
horizontally, and saves screenshots to the git-ignored
`ui-check-output/` for a look by eye.

```sh
pnpm ui:check                 # builds both apps, then checks
pnpm ui:check --no-build      # reuse the existing builds
pnpm ui:check --only=menu     # only scenarios whose name contains "menu"
pnpm ui:check --app=desktop   # one app: extension | desktop
```

## Rules for contributors

* No cryptography outside `crates/havenkeys-core/src/crypto/`.
* No `println!`/logging in the core, the bridge or the native host (a test
  enforces this for the core; clippy denies print macros in all four crates).
  The native host's stdout carries protocol frames only.
* No `innerHTML`, `console.*`, `eval` or `chrome.storage` in the extension.
  Build the DOM with `createElement`/`textContent`. Enforced by
  `apps/extension/src/hygiene.test.ts`. The one storage exception is
  `src/shared/prefs.ts`, which keeps a single boolean (whether in-page
  suggestions are shown) in `chrome.storage.local`.
* Extension code acts only on trusted user events (`isTrusted`), and takes
  page URLs from the browser's sender and tab data, never from messages or
  the DOM.
* No `console.*`, `innerHTML`, `dangerouslySetInnerHTML`, `eval`, or browser
  storage in the UI.
* Types holding secrets implement a redacting `Debug`.
* Errors are fixed strings; never interpolate user input.

## Releases and updates

This section is about the desktop app. Android releases are signed with a key
kept outside the repository; see `docs/android.md` → Release key custody.

See `docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md` for the
full design and `docs/security-model.md` §18 / `docs/threat-model.md` T10 for
the security side.

* **The signing key** was generated once, by the project owner, never by an
  agent or in CI: `pnpm tauri signer generate -w ~/.tauri/havenkeys-updater.key`,
  with a strong passphrase. The private key and its passphrase live only in
  the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and in an offline backup. Never commit
  them, paste them into an issue or PR, or print them in a build log.
* **`createUpdaterArtifacts`** is set only in `apps/desktop/src-tauri/tauri.bundle.conf.json`,
  which only the release workflow passes with `--config`. A local `tauri
  build` (or `pnpm build`) does not create updater artifacts and does not need
  the signing key.
* **The key only reaches packaging.** The workflow compiles first, without
  the signing secrets (`tauri build --no-bundle … -- --locked`): every
  dependency build script, proc macro and the frontend build run there. The
  tauri-action step that holds the key adds `tauri.package.conf.json`, which
  swaps the frontend build for a no-op, so it only packages and signs; Cargo
  recompiles just `havenkeys-desktop` itself (its config is embedded). Keep
  that split when editing `release.yml`.
* **Releasing:**
  1. Bump the version (`apps/desktop/src-tauri/tauri.conf.json`,
     `apps/desktop/package.json`, and the extension's `manifest/base.json` /
     `package.json` if it also changed).
  2. Push a `desktop-v*` tag. `.github/workflows/release.yml` builds Windows,
     macOS and Linux installers, signs the updater artifacts, and attaches
     everything to a **draft** GitHub Release.
  3. Wait for the workflow to finish, then check the draft's `latest.json`
     asset lists all four platform keys: `windows-x86_64`, `darwin-aarch64`,
     `darwin-x86_64`, `linux-x86_64`.
  4. Check that every `.sig` asset binds the version: `base64 -d
     <asset>.sig | sed -n 3p` must show `trusted comment: … version:<the
     release version>`. The app sets `requireSignedVersion`, so a signature
     without it (built by a CLI older than 2.12.0) is refused by every
     install. Keep `@tauri-apps/cli` at 2.12.0 or later.
  5. Click **Publish release**. Publishing is what makes `/releases/latest`
     (and so the update check) see it — a draft is invisible to both.
* **The in-app "What's new"** comes from the release's `releaseBody` at
  **build** time, baked into `latest.json`'s `notes` field. The workflow's
  text is neutral ("HavenKeys desktop-vX.Y.Z. See … for details.") because
  every installed app shows it; installer warnings (SmartScreen, Gatekeeper)
  belong on the website's download page and the release page, not there. Editing the
  release page's description afterward does not change what installed apps
  show; to correct it after publishing, edit and re-upload the `latest.json`
  asset itself before anyone updates.
* **Key rotation, while the old key is still held** (planned, or suspected
  compromise): generate the new key, ship one release signed with the *old*
  key whose `tauri.conf.json` already trusts the *new* public key, so every
  existing install can still verify and accept that one transitional
  release; sign every release after that with the new key only. After a
  suspected compromise do this at once — until installs have moved to the
  transitional release, the thief can still sign updates they accept.
* **Lost key** (no copy of the private key or its passphrase): there is no
  in-app path. No installation can verify anything signed with a new key, so
  users must install the next version by hand from the release page (as they
  did the first updater-enabled release); from that version on, updates are
  in-app again under the new key. See `threat-model.md` T10.

## Fuzzing

The parsers that take untrusted input are fuzzed deterministically inside
the normal test suites. The generators are seeded, so a failure reproduces
exactly, and no nightly toolchain or `cargo-fuzz` is needed:

| Target | Test |
|---|---|
| Native messages (requests, desktop replies, re-encoding) | `crates/havenkeys-protocol/tests/messages.rs` |
| Encrypted blobs (no mutation of a sealed blob ever opens) | `crates/havenkeys-core/tests/fuzz.rs` |
| TOTP input / `otpauth://` URIs | `crates/havenkeys-core/tests/fuzz.rs` |
| Website rules, page URLs, domain matching, generated look-alike hosts | `crates/havenkeys-core/tests/fuzz.rs` |
| Item JSON from the UI | `crates/havenkeys-core/tests/fuzz.rs` |
| Digital Asset Links files (`assetlinks.json`) | `crates/havenkeys-core/tests/fuzz.rs` |
| 1Password `.1pux` archives and their JSON | `crates/havenkeys-core/src/import/onepux.rs` |
| Bitwarden JSON exports | `crates/havenkeys-core/src/import/bitwarden.rs` |
| CSV exports (Bitwarden, Chrome, Firefox, KeePassXC, LastPass) | `crates/havenkeys-core/src/import/csv.rs` |
| Extension-side protocol validator | `packages/protocol/src/fuzz.test.ts` |
| Extension message validators, URL stripping, field classification over random DOM | `apps/extension/src/fuzz.test.ts` |

To run a target longer, raise its iteration count locally. Keep the
committed counts fast enough for every test run.
