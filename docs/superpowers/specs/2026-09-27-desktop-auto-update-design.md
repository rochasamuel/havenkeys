# Desktop auto-update — Design

Status: proposed, 2026-09-27.
Amends CLAUDE.md §1 ("Minimal network exposure", no third-party APIs) for one
bounded case: the desktop app may contact GitHub Releases to check for and
download **signed** updates. Users can turn the automatic check off. See §7.

> This software has not undergone an independent security audit.

## 1. Goal

Users on Windows, macOS and Linux (AppImage) get new versions without
uninstalling or downloading installers by hand:

1. The app checks for a newer release on its own (a few seconds after launch,
   then every 24 hours while it runs).
2. When one exists, a banner offers it, with the release notes.
3. The user clicks **Update**; the app downloads it, verifies its signature,
   locks the vault, installs it and restarts.

Nothing is downloaded or installed without that click.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Audience | Anyone who downloads HavenKeys, all three platforms | Owner-only, Windows-only |
| Behaviour | **Check and ask**: automatic check, install only on click | Manual check only (users stay on old versions); silent install on quit (changes code without the user knowing) |
| Mechanism | `tauri-plugin-updater`, driven **only from Rust** | Plugin JS API exposed to the webview; a hand-written updater |
| Linux | In-place update for AppImage; `.deb`/`.rpm` get a banner that opens the release page | Self-hosted apt/rpm repo (ongoing ops cost); no Linux notification |
| Setting | `autoCheck` in `updates.json` in the app data folder, **default on**, readable while locked | In the encrypted vault settings (unreadable while locked); in `device.json` (identity and Secret Key file) |
| Endpoint | `https://github.com/rochasamuel/havenkeys/releases/latest/download/latest.json` | Own server; the website |
| Review gate | Releases stay drafts; `/releases/latest` ignores drafts, so nothing is offered until the owner publishes | Auto-publish on tag |
| Windows installer | NSIS, `installMode: "passive"` | MSI |

## 3. Components

### 3.1 Rust — `apps/desktop/src-tauri/src/updates.rs`

Owns everything about updates. The webview never calls the plugin.

- **State machine** (held in `AppState` or its own managed state):
  `Idle → Checking → Available{version, notes} → Downloading{done, total} →
  Installing`, plus `Error{kind}` which returns to `Idle` on the next check.
  `install` is allowed only from `Available`. Concurrent checks are
  collapsed into one.
- **Scheduler:** when `autoCheck` is on, one check 5 s after start, then
  every 24 h. Turning the switch off cancels future checks; turning it on
  schedules a check.
- **`can_install_in_place()`:** `true` on Windows and macOS; on Linux only
  when the `APPIMAGE` environment variable is set.
- **Settings file `updates.json`:** `{ "autoCheck": bool }`, next to
  `device.json`, mode 0600 on Unix, written atomically. Missing or
  unreadable → `autoCheck: true`. `deny_unknown_fields` is **not** used, so
  a later version can add fields without an older one resetting the file.
- **Release notes:** taken from `latest.json`, truncated to 4 000
  characters, passed to the UI as plain text.

### 3.2 Tauri commands (added to the `main` capability allowlist)

| Command | Returns / does |
|---|---|
| `update_status` | `{ state, version?, notes?, progress?, autoCheck, canInstallInPlace, currentVersion }` |
| `check_for_update` | Runs a check now (the "Check now" button); errors are reported |
| `install_update` | From `Available` only: download → verify → lock → install → restart |
| `set_update_auto_check` | Saves `autoCheck` and reschedules |

State changes are pushed with an `updates://status` event so the UI does not
poll. Choosing the tray's "Update available" emits `updates://show`, which
brings back a banner dismissed with "Later". No updater plugin permission is added to the capability file.

### 3.3 Desktop UI

- **Banner** at the top of the main window, on both the unlock and the vault
  screens: "HavenKeys 0.9.0 is available · What's new · Update". "What's new"
  expands the notes as text (never HTML). While downloading it shows a
  progress bar; on error, a fixed message ("Update failed. Try again later.").
  "Later" dismisses that version until the next launch; the tray's "Update
  available" brings it back, and Settings → Updates always shows the offer
  with its own Update/Download button. A failure banner can be dismissed
  until the status next changes. Checks time out after 30 s and downloads
  after 10 minutes; a background check that fails keeps an offer already
  on screen.
- When `canInstallInPlace` is false the button reads "Download" and opens
  the release page through the existing `open_url` path. That URL is a
  constant in Rust (`https://github.com/rochasamuel/havenkeys/releases/latest`),
  never taken from `latest.json`.
- **Settings → Updates:** current version, "Check for updates automatically"
  switch, "Check now" button with its result ("You're up to date" /
  the banner appears / error message).
- **Tray:** an "Update available" item while in `Available`; it shows the
  window.
- Strings added in English and Portuguese (pt-BR).

### 3.4 Install flow

1. Download with progress events.
2. The plugin verifies the minisign signature against the public key in
   `tauri.conf.json`. **A bad or missing signature aborts; nothing is
   installed and the vault is not locked.**
3. `state.lock(app, "update")` — the same lock path as quitting (keys
   dropped, extension authorization ended).
4. Install: Windows runs the NSIS installer in passive mode (it closes and
   relaunches the app); macOS and AppImage replace the bundle, then
   `app.restart()`.
5. The app starts on the unlock screen.

### 3.5 Windows native host

Browsers keep `havenkeys-native-host.exe` running (it retries while the app
is gone), and Windows cannot overwrite a running exe. An NSIS pre-install
hook (`bundle.windows.nsis.installerHooks`, `NSIS_HOOK_PREINSTALL`) ends
`havenkeys-native-host.exe` processes before files are copied. This also
fixes today's manual in-place installs. The implementation must first
confirm the extension reconnects (a new `connectNative`) after its port
drops. macOS and Linux can replace running binaries; the AppImage's
data-folder copy of the host must be replaced by rename, not written in
place (the implementation checks `native_host.rs` does this).

## 4. Release pipeline

- `tauri.conf.json`:
  - `bundle.createUpdaterArtifacts: true`
  - `plugins.updater.pubkey`: the public key (committed)
  - `plugins.updater.endpoints`: the endpoint in §2
  - `plugins.updater.windows.installMode: "passive"`
- `.github/workflows/release.yml`, tauri-action step:
  - env `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
    from repository secrets
  - `includeUpdaterJson: true`, `updaterJsonPreferNsis: true`
  - `releaseDraft: true` stays
- The action merges the three platforms into one `latest.json` asset.
- The build fails if the signing secret is missing (the Tauri CLI refuses to
  create updater artifacts without it), so an unsigned updater release
  cannot be published by accident.

## 5. The signing key

- Generated once **by the owner**, never by an agent or in CI:
  `pnpm tauri signer generate -w ~/.tauri/havenkeys-updater.key`, with a
  strong passphrase.
- Private key and passphrase go into repository secrets; an offline backup
  is kept. Losing it means every user must install the next version by hand.
- `docs/development.md` documents rotation: ship one release signed with the
  old key whose config trusts the new public key, then sign with the new key.

## 6. Rollout

0.8.0 and earlier have no updater. The first updater-enabled release is
installed by hand, over the existing install (no uninstall). Every later
release is offered in the app. Before relying on it, a throwaway pair of
releases (e.g. 0.9.0 → 0.9.1) is tested on each OS.

## 7. Security

- **New network destination:** GitHub (`github.com` and the
  `objects.githubusercontent.com` redirect) over HTTPS only. Requests carry
  no account, device ID or vault data. GitHub learns the IP address, the
  time, and that HavenKeys is checking. Users can turn the automatic check
  off; the manual check and the download then happen only on click.
- **Integrity comes from the signature, not TLS.** A compromised GitHub
  account, repository, CDN or network cannot install code without the
  private key.
- **No downgrade:** `latest.json` itself is not signed; only the artifacts
  are. So downgrade protection rests on `requireSignedVersion: true` in the
  updater config: the signature's trusted comment binds the app version it
  was signed for, and a manifest announcing any other version than the one
  signed is rejected (as is a signature carrying no version). The plugin's
  default comparator then refuses any version not greater than the running
  one. Together, an old signed release cannot be replayed under a new
  version number. Releases must be built with `@tauri-apps/cli` 2.12.0 or
  later, which writes the version into the signature.
- **Residual trust:** whoever holds the private key and passphrase can ship
  code to every install, including the Rust core that holds the vault key.
  Documented as a trust assumption in `docs/threat-model.md`.
- **Webview stays unprivileged:** no plugin permissions; four named
  commands; the release notes are text; the "Download" URL is a constant.
- **Lock before install:** verification first, then lock, then install; a
  failed update never leaves keys in a half-exited process.
- **Errors:** fixed messages to the UI; plugin error text is logged by kind
  only, without URLs or paths.
- CLAUDE.md §1 gets an amendment note pointing to this spec.

## 8. Code layout

| File | Change |
|---|---|
| `apps/desktop/src-tauri/Cargo.toml` | add `tauri-plugin-updater = "2"` |
| `apps/desktop/src-tauri/src/updates.rs` | new: state machine, scheduler, settings file, install |
| `apps/desktop/src-tauri/src/commands.rs`, `lib.rs` | four commands, plugin registration, startup scheduling |
| `apps/desktop/src-tauri/src/tray.rs` | "Update available" item |
| `apps/desktop/src-tauri/capabilities/*.json` + permissions | allow the four commands |
| `apps/desktop/src-tauri/tauri.conf.json` | updater config, `createUpdaterArtifacts`, NSIS hook |
| `apps/desktop/src-tauri/windows/hooks.nsh` | new: end native host processes |
| `apps/desktop/src/…` | `UpdateBanner`, Settings → Updates, API bindings, i18n |
| `.github/workflows/release.yml` | secrets, `includeUpdaterJson`, `updaterJsonPreferNsis` |

## 9. Testing

- **Rust unit tests:** state transitions (install refused outside
  `Available`; concurrent checks collapse; error returns to idle);
  `can_install_in_place` per platform/env; `updates.json` default,
  round-trip, corrupt file, unknown fields ignored on read.
- **Frontend tests:** banner for each state; notes rendered as text
  (an `<img onerror>` payload in notes appears as literal text);
  "Download" vs "Update" by `canInstallInPlace`; settings switch.
- **Manual end-to-end** per OS with a test release pair: update offered,
  signature verified, vault locked, app restarts on the new version; a
  release with a tampered asset is rejected; Windows update while a browser
  holds the native host succeeds and the extension reconnects.
- `cargo audit` / `cargo deny` with the new dependency.

## 10. Docs

- `docs/security-model.md`: network destinations, updater, signing.
- `docs/threat-model.md`: malicious update, key compromise, downgrade.
- `docs/development.md`: key generation, secrets, rotation, release steps.
- `docs/website.md` / README: updates are in-app from 0.9.0.
- CLAUDE.md §1 amendment note.

## 11. Out of scope

- Browser extension updates (handled by the browser stores).
- Code-signing the installers (still a known limitation).
- Update channels (beta/stable), staged rollouts, delta updates.
- `.deb`/`.rpm` in-place updates.
