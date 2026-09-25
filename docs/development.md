# Development

## Prerequisites

* Rust ≥ 1.88 (`rustup`)
* Node.js ≥ 20 and pnpm 10
* Tauri system dependencies:
  * **Debian/Ubuntu:** `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config`
  * **macOS:** Xcode command-line tools
  * **Windows:** Microsoft C++ Build Tools and WebView2 (preinstalled on Windows 11)

The security core, protocol, bridge, native host, server and sync client
crates need only Rust. They are the Cargo workspace's `default-members`, so
`cargo test` works without the WebKit libraries.

The server and sync-client suites additionally need a **Postgres**, because
what they test is SQL-level (account isolation, row locks, one transaction
per write) and a mock would not test it. `scripts/test-server.sh` starts a
disposable one in Docker and creates a fresh database per test:

```sh
scripts/test-server.sh              # or: pnpm test:server
docker rm -f havenkeys-test-pg      # when you are done with it
```

Running the server itself, and the backup drill that has to pass before it
holds a real vault, are in `docs/deployment.md`.

## Commands

| Task | Command |
|---|---|
| Install JS deps | `pnpm install` |
| Run desktop app (dev) | `pnpm dev` |
| Build installers | `pnpm build` |
| Rust tests (core, protocol, bridge, native host, OS lock) | `cargo test` |
| Server + sync client tests (needs Docker) | `scripts/test-server.sh` |
| Regenerate design tokens after editing them | `pnpm --filter @havenkeys/ui generate` |
| Rust lint (same crates) | `pnpm lint:rust` |
| Native host (release) | `pnpm build:host` |
| Register native host | `scripts/install-native-host.sh` (Windows: `scripts\install-native-host.ps1`) |
| Extension build | `pnpm build:extension` → `apps/extension/dist/{chrome,firefox}` |
| Extension store zips | `pnpm package:extension` → `apps/extension/dist/havenkeys-{chrome,firefox}-<version>.zip` (builds first; fails if `manifest/base.json` and `package.json` versions differ) |
| Desktop crate lint | `cargo clippy -p havenkeys-desktop -- -D warnings` (needs WebKit libs) |
| TS type check (desktop UI, extension, protocol) | `pnpm typecheck` |
| TS tests | `pnpm -r test` |
| KDF benchmark | `cargo run --release -p havenkeys-core --example kdf_bench` |
| Rust advisories | `cargo audit` |
| Rust policy (advisories, licences, sources) | `cargo deny check` |
| JS advisories | `pnpm audit` |
| Check the Windows-only code from Linux | `cargo clippy -p havenkeys-oslock --target x86_64-pc-windows-gnu -- -D warnings` |

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

## Rules for contributors

* No cryptography outside `crates/havenkeys-core/src/crypto/`.
* No `println!`/logging in the core, the bridge or the native host (a test
  enforces this for the core; clippy denies print macros in all four crates).
  The native host's stdout carries protocol frames only.
* No `innerHTML`, `console.*`, `eval` or `chrome.storage` in the extension.
  Build the DOM with `createElement`/`textContent`. Enforced by
  `apps/extension/src/hygiene.test.ts`.
* Extension code acts only on trusted user events (`isTrusted`), and takes
  page URLs from the browser's sender and tab data, never from messages or
  the DOM.
* No `console.*`, `innerHTML`, `dangerouslySetInnerHTML`, `eval`, or browser
  storage in the UI.
* Types holding secrets implement a redacting `Debug`.
* Errors are fixed strings; never interpolate user input.

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
| 1Password `.1pux` archives and their JSON | `crates/havenkeys-core/src/import/onepux.rs` |
| Extension-side protocol validator | `packages/protocol/src/fuzz.test.ts` |
| Extension message validators, URL stripping, field classification over random DOM | `apps/extension/src/fuzz.test.ts` |

To run a target longer, raise its iteration count locally. Keep the
committed counts fast enough for every test run.
