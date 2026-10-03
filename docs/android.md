# Android

The Android app (`apps/android`, application ID `net.havenkeys.android`) is
a full client of your `havenkeys-server` account, like the desktop app.
Design: `docs/superpowers/specs/2026-10-01-android-app-design.md`. Security:
`docs/security-model.md` §22 and `docs/threat-model.md` T12.

> This software has not undergone an independent security audit.

**Status: Android M4.** Sign in with the Emergency Kit (QR code or typed) or
an invite; unlock with the master password or, once turned on, a fingerprint
or face; browse, search, reveal, copy and read TOTP codes offline; generate
passwords; fill logins and TOTP codes in apps and browsers through Android
Autofill. Android M2 adds editing: create, edit and delete logins, secure
notes and cards, and edit the identity, with a password generator in the
editor; writes need the server (offline, the editor is read-only) and a
concurrent edit shows "This item changed on another device." It also adds
saving from Autofill: after you confirm Android's save sheet, a login typed
into a browser is saved for that site, and one typed into an app is bound to
that app (`security-model.md` §22.16, §22.17). Android M3 adds passkeys
and Android M4 cards and the identity in Autofill (below). The redesign's
shell (2026-10-03 spec §6) replaces the vault list: Home, Items and Settings
tabs with their own back stacks, a search screen with recent searches, and
an add sheet. Not yet: editing
custom fields (kept as they are), scanning a TOTP QR code (type or paste the
key), saving a login typed while locked, and an in-app updater.

**Nothing in `apps/android` has run on a phone or an emulator yet.** It
compiles, its JVM unit tests pass, and the Rust it calls is tested on the
host. The instrumented tests (`src/androidTest`) are compiled but have not
run. The manual checklist at the end of this file is what remains.

Minimum Android 9 (API 28); compiled against and targeting API 36.

## Passkeys (Android M3)

HavenKeys is a passkey and password provider for Android's Credential
Manager, on **Android 14 or later** (the app itself still runs from Android
9; older phones simply have no passkey support). Security:
`security-model.md` §22.18.

* **Turn it on:** Settings → Passwords & passkeys (Passwords, passkeys &
  accounts) → select HavenKeys. HavenKeys' own Autofill setup screen has a
  button that opens that page.
* **Sign in:** in a browser or an app that offers a passkey, choose
  HavenKeys. A locked vault shows "Unlock HavenKeys" first. You confirm with
  your fingerprint, face or screen lock; a phone with no screen lock cannot
  use passkeys. Signing in works offline for websites in browsers. An app's
  passkey needs a fresh answer from the site's `assetlinks.json`, so after 7
  days without network an app's passkey is refused until the phone is online
  again.
* **Create:** HavenKeys opens a screen showing the site and the account, and
  creates the passkey when you tap Save. Creating needs the server (offline
  it says so and nothing is saved). If the vault already has a passkey for
  that account, it says so instead.
* **Passwords:** Credential Manager can also offer your saved logins (only
  those with a username), by the same rules as Autofill.
* **Browsers:** Chrome and Firefox and the others on the privileged list
  (`scripts/update-android-browsers.sh`); an unlisted browser cannot use
  passkeys through HavenKeys.
* **Limitations:** no hybrid (phone-as-security-key) sign-in, no conditional
  create (Chrome's automatic upgrade; HavenKeys never saves without your
  tap), and no saving a password through Credential Manager (saving stays
  with Autofill).

## Cards and identity (Android M4)

Autofill also fills payment cards and your identity (name, address, phone,
email, dates, document numbers). Security: `security-model.md` §22.4a.

* **Where:** cards on https pages in browsers and in apps, never on an http
  page in a browser. An app is not a page: an app (its WebView included, and
  a browser that is not on the privileged list, which is treated as an app)
  gets cards whatever the scheme of what it shows (spec §7.6 revision 7).
  The identity goes to any page or app. Document numbers (for example CPF)
  need a separate confirmation and never appear on an http page.
* **Direct and gated rows:** while unlocked with "Confirm before filling"
  off, up to 5 card rows (for example `•••• 1111 · 04/33`) and the identity
  row fill with one tap. Document numbers, "Confirm before filling" on, and
  every row shown after "Unlock HavenKeys" open HavenKeys, which asks "Fill
  ... in <site or package>?" before anything is filled.
* **Frames:** a checkout's card fields fill in frames of the page's own
  site and of a fixed list of payment processors (for example Stripe
  Elements); any other frame is left out.
* **Empty fields only:** nothing you already typed is overwritten, and
  nothing is submitted. A list counts as empty when nothing or its first
  option is chosen. A card brand goes only into a list.
* **Lists and dates:** month and year lists are matched by their option
  labels (the only option text Kotlin sees); date pickers get local
  midnight in the device's time zone. A value that does not fit a field's
  maximum length is left out, never cut.
  A login field guess never takes a card field, and a sign-up routed to the
  identity keeps its "Save password?" sheet when there is no identity row.
* **Saving a card:** type a new card and submit; after you confirm Android's
  save sheet HavenKeys saves it. It needs the server, and a card whose
  number is already saved is not saved again.
* **Limitations:** the autofill framework holds up to 5 cards' values for a
  fill session (AN45; "Confirm before filling" avoids it); frames depend on
  the browser's report (AN47); a card typed into a processor's iframe is not
  saved (AN49). Not run on a device yet (AN50).

## Toolchain

Installed in user space, no `sudo`:

| Tool | Version used | Where |
|---|---|---|
| JDK | Temurin 17 (17.0.20.1+1) | `~/.local/opt/jdk-17` (a symlink to `jdk-17.0.20.1+1`) |
| Android SDK | `platform-tools`, `platforms;android-36`, `build-tools;36.0.0`, `ndk;30.0.16248370` | `~/Android/Sdk` |
| Rust targets | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` | `rustup target add …` |
| `cargo-ndk` | 4.1.2 | `cargo install cargo-ndk --version 4.1.2 --locked` |

```sh
# JDK 17 (Temurin)
mkdir -p ~/.local/opt && cd ~/.local/opt
curl -fsSL -o jdk17.tar.gz 'https://api.adoptium.net/v3/binary/latest/17/ga/linux/x64/jdk/hotspot/normal/eclipse'
tar xzf jdk17.tar.gz && rm jdk17.tar.gz && ln -sfn "$(ls -d jdk-17*)" jdk-17

# Android command-line tools: the "Command line tools only" zip for Linux
# from https://developer.android.com/studio#command-line-tools-only
mkdir -p ~/Android/Sdk/cmdline-tools && cd ~/Android/Sdk/cmdline-tools
curl -fsSL -o tools.zip "<that URL>"
unzip -q tools.zip && rm tools.zip && mv cmdline-tools latest

export JAVA_HOME=~/.local/opt/jdk-17 ANDROID_HOME=~/Android/Sdk
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/30.0.16248370

yes | sdkmanager --licenses >/dev/null
sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "ndk;30.0.16248370"
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --version 4.1.2 --locked
```

Keep the NDK version in step in three places: `ndkVersion` in
`apps/android/app/build.gradle.kts`, `ANDROID_NDK_VERSION` in
`.github/workflows/android.yml`, and your `ANDROID_NDK_HOME`. Gradle uses
the NDK to strip the Rust library when packaging.

## Building

Two steps: the Rust library and its Kotlin bindings, then Gradle.

```sh
scripts/build-android.sh            # debug Rust, then regenerate the bindings
scripts/build-android.sh --release  # release Rust (LTO, panic = abort)
```

`scripts/build-android.sh` runs `cargo ndk` for `arm64-v8a`, `armeabi-v7a`
and `x86_64` into `apps/android/app/src/main/jniLibs/` (not committed), then
generates the UniFFI Kotlin bindings into
`apps/android/app/src/main/kotlin/uniffi/` (committed). Run it after any
change to `crates/havenkeys-mobile` or what it uses, and commit the bindings
it changes; CI fails when the committed bindings differ from a fresh build.
Rebuild the debug libraries after a release build, so your working tree's
`jniLibs` match the debug APK you install next.

Then, from `apps/android`:

| Task | Command |
|---|---|
| Lint, including the no-logging and no-Material scans | `./gradlew detekt` (runs `forbidLogging` and `forbidMaterial` first) |
| JVM unit tests | `./gradlew testGithubDebugUnitTest` |
| Android lint | `./gradlew lintGithubDebug` |
| Debug APK | `./gradlew assembleGithubDebug` → `app/build/outputs/apk/github/debug/` |
| Release APK | `./gradlew assembleGithubRelease` → `app/build/outputs/apk/github/release/app-github-release-unsigned.apk` (unsigned), or `app-github-release.apk` when the four `HAVENKEYS_*` values are set (signed) |
| Instrumented tests (needs a device or emulator) | `./gradlew connectedGithubDebugAndroidTest` |
| Compile the instrumented tests only | `./gradlew assembleGithubDebugAndroidTest` |

The two flavors, `github` and `play`, are identical in M1; they will differ
when the `github` flavor gains its updater. Release builds are minified with
R8. No signing key is committed: a release build is signed only when the
`HAVENKEYS_*` variables described under "Release key custody" are set,
otherwise it is unsigned.

Every Gradle dependency is pinned by SHA-256 in
`apps/android/gradle/verification-metadata.xml`. After changing a
dependency, regenerate it and review the diff:

```sh
./gradlew --write-verification-metadata sha256 help
```

CI (`.github/workflows/android.yml`) validates the Gradle wrapper, runs
`cargo test -p havenkeys-mobile --features testing`, builds the Rust
library, checks the bindings are current, and runs
`detekt testGithubDebugUnitTest lintGithubDebug assembleGithubDebug`. It has
not yet run on GitHub.

The Rust side is tested on the host:

```sh
cargo test -p havenkeys-mobile --features testing   # includes tests/regressions.rs
cargo test -p havenkeys-core                          # bundle, app targets, asset links
```

The round-trip test (two phones editing one vault through a real server)
needs Postgres; `scripts/test-server.sh` starts a container and exports the
`HAVENKEYS_TEST_DATABASE_URL` it uses:

```sh
cargo test -p havenkeys-mobile --features server-tests --test round_trip
```

## Running against a local server

The server address must be `https://`, except `http://localhost` (or
`127.0.0.1`), which Rust's transport accepts only when the native library
is a debug build (`scripts/build-android.sh` without `--release`); a release
build refuses plain HTTP everywhere. On a phone, `localhost` is the phone
itself; `adb reverse` forwards it to your machine.

1. Start a server (`docs/deployment.md` §7). It listens on port 8080.
2. Make an invite: `havenkeys-server admin new-account --email you@example.com`
   (`docs/deployment.md`), or sign in with an Emergency Kit from an existing
   account.
3. Forward the phone's port 8080 to your machine's:

   ```sh
   adb reverse tcp:8080 tcp:8080
   ```

4. Install the debug APK (`adb install -r app/build/outputs/apk/github/debug/app-github-debug.apk`)
   and use `http://localhost:8080` as the server address.

`adb reverse` lasts until the phone disconnects; run it again after
reconnecting. A server signed by your own CA works in any build once the CA
is installed on the phone, because the app trusts user-installed CAs
(`security-model.md` §22.9).

## Emulator

Not yet tried in this repository. An x86_64 system image with Google APIs is
the usual choice (the `x86_64` Rust library is built for it); it needs
hardware acceleration (`/dev/kvm` on Linux) and about 2 GB of downloads:

```sh
sdkmanager "emulator" "system-images;android-36;google_apis;x86_64"
avdmanager create avd -n havenkeys -k "system-images;android-36;google_apis;x86_64"
$ANDROID_HOME/emulator/emulator -avd havenkeys
```

With an emulator, `adb reverse` works as above; the host is also reachable
as `10.0.2.2`, but Rust refuses plain HTTP to anything but `localhost` (and,
in a release build, to `localhost` too), so
use `adb reverse`. To test biometric unlock, enrol a
fingerprint in the emulator's settings and touch it with
`adb -e emu finger touch 1`. The privileged-browser and Digital Asset Links
checks need the real Chrome or Firefox from the Play Store image and real
apps, so the autofill items of the checklist belong on a phone.

## Updating the privileged browser list

Rust trusts a browser to report the page's domain only if its package and
release certificate are in `crates/havenkeys-core/data/android-browsers.json`,
the list Google's Credential Manager uses. To refresh it:

```sh
scripts/update-android-browsers.sh
git diff crates/havenkeys-core/data/android-browsers.json
cargo test -p havenkeys-core app_target
```

Review every added package and certificate before committing.

## Release key custody

Android installs an update only when it is signed with the same key as the
installed app. The release key is therefore permanent: if it is lost, every
user has to uninstall HavenKeys (removing the phone's local copy of the
vault) and sign in again with their Emergency Kit.

Create it once, on your own machine:

```sh
keytool -genkeypair -v -keystore havenkeys-release.jks -alias havenkeys \
  -keyalg RSA -keysize 4096 -validity 12000
```

* Keep `havenkeys-release.jks` and both passwords offline: a HavenKeys
  secure note and a copy on removable storage, with both passwords kept
  alongside the copy. Never commit them. The key and passwords are never printed
  in build logs.
* If the app moves to Google Play, the same key goes to Play App Signing, so
  sideloaded installs keep updating.
* Add four repository secrets (Settings → Secrets and variables → Actions):
  `ANDROID_KEYSTORE_BASE64` (`base64 -w0 havenkeys-release.jks`),
  `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` (`havenkeys`) and
  `ANDROID_KEY_PASSWORD`.
* Its certificate fingerprint (the SHA256 line of `keytool -list -v
  -keystore havenkeys-release.jks`) is in every release's notes. After
  creating the key, put that SHA-256 into `ANDROID_CERT_SHA256` in
  `apps/web/src/lib/releases.ts`; the download page shows it once it is set
  (it is empty until then).
* Gradle signs the release build only when `HAVENKEYS_KEYSTORE_FILE`,
  `HAVENKEYS_KEYSTORE_PASSWORD`, `HAVENKEYS_KEY_ALIAS` and
  `HAVENKEYS_KEY_PASSWORD` are all set; otherwise it builds unsigned.

## Manual checklist

None of these has been run. Record results in `docs/security-review.md`
("Android M1" and "Android M2"), where the same lists live.

- [ ] Real phone, Android 14+: sign in by scanning the kit; unlock with password; enroll fingerprint; unlock with fingerprint; reboot → password required; add a fingerprint → bundle refused, password required.
- [ ] Chrome (Autofill using another service) and Firefox: login on github.com fills after a tap; github.com.evil.com (hosts file or a test domain) offers nothing; an http page does not get an https login.
- [ ] GitHub app: offered through Digital Asset Links or after "Search HavenKeys…" binding; a sideloaded app with the same package name and another key gets nothing.
- [ ] Google app (WebView sign-in): its WebView's domain is not trusted.
- [ ] Locked vault: "Unlock HavenKeys" appears; nothing fills without unlocking.
- [ ] Confirm before filling on: each fill asks; off: one tap fills.
- [ ] TOTP: OTP field after a login offers the code.
- [ ] Recents thumbnail is blank; screenshots are blocked.
- [ ] Airplane mode: unlock, reveal, TOTP and autofill work; sync shows offline.
- [ ] With Google Autofill (or another password manager) as the device's autofill service for other apps: HavenKeys' unlock, onboarding and biometric-enroll fields get no suggestion, and Google Autofill does not offer to save the master password or the Secret Key.
- [ ] An app with "display over other apps" covering the binding prompt or a gated row's activity: the tap through it is ignored.
- [ ] With the screen kept on and a TOTP login open, the vault locks at the auto-lock time; after the app sat frozen past the deadline, the next fill shows "Unlock HavenKeys".
- [ ] Before the first release: build a release APK with R8 (`scripts/build-android.sh --release`, then `./gradlew assembleGithubRelease`) and smoke-test it on a phone — unlock, sync, reveal, autofill — so R8 has not stripped anything JNI or JNA (UniFFI, `rustls-platform-verifier`) reaches by reflection.

Editing and saving (Android M2; the same list is in `security-review.md`):

- [ ] Create a login, a secure note and a card; edit each; delete one. Each shows on the desktop too.
- [ ] Edit a login that has a custom field, a passkey and an app binding: all three survive a phone edit.
- [ ] Change a password: the field stays read-only until the current value loads; an untouched field keeps its stored value.
- [ ] Rotate, change theme and font size with an open draft: the draft stays. Change the locale or kill the process: it is gone.
- [ ] Lock with a draft open: the editor closes, the draft is gone, recents are blank.
- [ ] With another autofill service active: the editor's fields get no suggestion and no save offer.
- [ ] Airplane mode: the editor shows the offline banner and will not save.
- [ ] Edit the same item on the desktop and the phone, save on the phone second: "This item changed on another device."
- [ ] Autofill save in Chrome and Firefox: a new login is saved for that site only; a changed password updates it; the same password is "unchanged"; github.com.evil.com never updates github.com's login.
- [ ] Autofill save in an app: the login is bound to that app with no website; an app with another certificate does not update it.
- [ ] Username-first sign-in: one login on Android 10+; password step only on Android 9.
- [ ] Lock before submitting: "HavenKeys locked before saving."; offline: "HavenKeys is offline. The login was not saved."; HavenKeys' own screens never offer to save.

### Redesign stage 3 (shell)

Run on an emulator or phone (Android 14+), in light and dark, once with
`adb shell settings put global animator_duration_scale 1` and once with `0`
(restore `1` afterwards).

- [ ] Unlock: Home appears on the slower reveal; the identity card and both groups settle in sequence; nothing settles again on a tab return. With animations removed, it is an instant cut.
- [ ] Tabs: Items → Logins → Home → Items shows Logins again; tapping Items again shows the Items root; Back from Items or Settings root goes to Home; the top bar does not move or flicker on a tab change; a light tick on a change, none on a reselect.
- [ ] Per-tab back stacks survive tab switches, rotation and a theme change: open Items → Logins, switch tabs and back, rotate, change theme: Logins is still there. After a process kill (`adb shell am kill net.havenkeys.android`) or any lock, unlocking opens a fresh Home with nothing behind it.
- [ ] Push and pop: a category list and an item slide in from the right while the old screen shifts left and dims; Back reverses; the predictive back gesture scrubs the item screen (Android 14+). A row tapped during a tab crossfade, a push or a pop opens at once; the same row (or the search pill, an add tile, Generate) tapped twice quickly opens once.
- [ ] Shared title and bounds: an item in both Recently added and Frequently used: tapping either row moves that row's title into the item screen; the search pill's bounds grow into the search field and shrink back, across the two NavHosts, without a jump.
- [ ] Search: the pill grows into the focused field with the keyboard up; recent searches fade in after; Clear empties them; typing shows results and records nothing; opening a result records the query (it is at the top of recents next time); Cancel and Back shrink the field into the pill; Back from an opened result returns to the results.
- [ ] Search and the lock: with a query typed, lock (button, screen off, auto-lock): Unlock appears at once; after unlocking the query and results are gone. Kill the process with a query typed (`adb shell am kill net.havenkeys.android` after Home): the query never comes back.
- [ ] Lock from any tab (Home, Items, a category list, Settings, the add sheet): Unlock appears and nothing survives (query, lists, recents); after unlocking, Home reloads its identity card and groups.
- [ ] Add sheet: springs up, backdrop dims, tiles stagger; drag down and a backdrop tap close it; Login, Secure note and Card open their editors; Generate password opens the generator. Airplane mode: the item tiles are dimmed with "Adding needs a connection"; the generator still opens. The identity is never offered.
- [ ] Offline: the top bar shows "Offline" and Sync now is disabled (dimmed, not tappable, read as disabled by TalkBack). Online, Sync now shows a ring while syncing and TalkBack announces that it is syncing.
- [ ] Top bar at the largest font size on a narrow (360dp) phone, offline: nothing is cut and the pill's words wrap (the bar may grow); Home's identity card summary may wrap to two lines in Portuguese.
- [ ] Pull to refresh on Home and a category list syncs; Sync now in the top bar does the same, and doing both at once runs one sync.
- [ ] Settings: every row works (auto-lock and clipboard sheets, the three switches, biometric enrolment dialog, autofill setup, devices, sign out, remove device with a wrong then right email).
- [ ] FLAG_SECURE: screenshots blocked and the recents thumbnail blank on Home, search, the add sheet and the Settings dialogs.
- [ ] TalkBack: tabs read "Home, tab, 1 of 3, selected"; the pill reads "Search HavenKeys, button"; Sync now and Lock now are named; the offline badge is read; the add sheet's dimmed tiles read as disabled; Home's headings are headings; the identity card reads its title and summary as one button; the category back chevron reads "Back"; Sync now is the reachable way to sync (the pull's custom action may not be).
- [ ] Stage 2 kit checks (`apps/android/DESIGN.md` "Review notes (stage 2)", in the debug catalogue `net.havenkeys.android/.catalogue.KitCatalogueActivity`): emulator or device screenshots including the elevation shadows of sheet, dialog, menu, toast and add button; the animations by hand at scale 1 and 0 (sheet spring, drag down and backdrop tap, a tap during the rise closes once; dialog, menu, toast, copy glyph, switch, segmented control, pull to refresh; at 0 each cuts, nothing blocked); the full TalkBack pass (button, switch on/off, tab selected, slider "24"; a text field reads its label once, and whether its label and typed value are both read; a secret field reads as a password and never speaks its value; "Copy Password" then "Copied"; the toast is announced; sheet, dialog and menu titles are announced; an item row is one stop; the pull-to-refresh "Refresh" action is reachable, and if not Home needs a visible sync control; hairlines skipped); FLAG_SECURE on the sheet, dialog and menu windows.
- [ ] Stage 1's deferred checks: pick a direct-fill row in Chrome, open another form, then see that login under Frequently used; fill event history still delivers `TYPE_DATASET_SELECTED` on Android 14+ (`FillEventHistory` is deprecated in API 36 with no replacement); a pick after a null response is counted once; a confirmed login fill counts once.

### Redesign stage 4 (screens)

Run on an emulator or phone (Android 14+), in light and dark, once with
`adb shell settings put global animator_duration_scale 1` and once with `0`
(restore `1` afterwards), and once in Portuguese (Brazil) at the largest
font size.

- [ ] Item: from Home, Items and search, the row's tile and title grow into the header; with animations off it cuts. Show reveals the password in mono with coloured digits; it hides after 30 s, on leaving the screen, when the app goes to the background (ON_STOP) and on lock. Copy turns the glyph to a check with a light haptic and the toast "Password copied. The clipboard clears in 30 s."; the item then appears under Frequently used. Offline, Edit and More are dimmed. More, Delete asks, warns about passkeys when the login has one, and returns to the list.
- [ ] Editor: a new login's Generate fills and shows the password; a saved login's password shows the mask until Change (which reads it); Remove then Undo; the website's "Matches" row opens a sheet of three rules; the one-time code's setup key is typed masked with the eye to show it. Gboard: no suggestions and nothing learned in the title, the secret fields and the setup key (Gboard's incognito marker shows on secret fields). Back with changes asks. Edit the same item on the desktop meanwhile, then Save on the phone: the conflict dialog ignores Back and an outside tap; Reload loads the new version.
- [ ] Generator: the length row and the slider agree; the switches regenerate; Copy shows its toast.
- [ ] Unlock: a wrong password is read by TalkBack as the field's error; the field is empty after each attempt; rotating or killing the process (`adb shell am kill net.havenkeys.android`) never brings the password back; the Secret Key field shows its format as a placeholder only while empty; biometrics as before.
- [ ] Onboarding: scan (camera), type and invite; the hints under server, Secret Key and the new password; too short and mismatch read as the fields' errors.
- [ ] Devices: this phone is marked; Revoke asks in a dialog; revoking this phone signs it out.
- [ ] Autofill setup: the state row, Open settings, the Chrome help, the passkeys state row; coming back from Android's settings updates it.
- [ ] Autofill: in an app with no saved login, "Search HavenKeys..." opens the search, a pick asks "Use ... in <package>?" with the app's own name under it, and fills. A card and an identity with documents ask with three stacked answers. The outlined copy glyph of "Copy one-time code" is legible in the dropdown and in the keyboard chip.
- [ ] Passkeys: create a passkey on a site in Chrome: the translucent sheet rises over the real calling app (Chrome), with Save reachable; its footer (Save and Cancel) stays pinned above the keyboard and the navigation bar, also with many logins and a large font; dragging it down, a backdrop tap and Back cancel and return to the site (which reports a cancellation); locked, unlock fills the screen first, then the sheet.
- [ ] FLAG_SECURE: screenshots blocked and the recents thumbnail blank on every screen above, the passkey sheet, the dialogs and the menu.
- [ ] TalkBack: unlock reads the password as a password field (never speaking it), with the error as the field's; item detail has one stop per read-only row (label and value) with Show and Copy separate, a hidden value reads "Hidden Password", never dots or a length, the code row reads its digits and "12 seconds remaining"; the editor's and onboarding's secret fields read as password fields and never speak their value; the generator's slider reads "Length, 24" once; dialog and sheet titles are announced; the passkey sheet's logins are radio buttons, one selected.
- [ ] pt-BR at the largest font: the stacked dialog answers, the editor's hidden rows and the generator's switches wrap without cutting; the catalogue's segmented control keeps one height; unlock's italic word wraps with the line.

### Redesign stage 5 (no Material)

The whole app, on an emulator or phone (Android 14+), in light and dark, in
English and Portuguese (Brazil), at the default and the largest font size.
This is the redesign's final pass; the stage 2–4 lists above still apply.

- [ ] Press: every tappable thing (rows, buttons, tabs, tiles, switches, chips, the search pill, menu and sheet rows, dialog buttons) scales slightly with a brass-soft wash; nothing ripples and nothing flashes grey, anywhere in the app, the autofill screens and the passkey sheet included.
- [ ] Text selection: long-press text in a field (title, a website, search, notes): the handles and highlight are brass, not blue or purple.
- [ ] TalkBack, the whole app in one sitting: unlock, Home, search, Items and a category list, an item (reveal, copy, the code row), the editor (each field, hidden rows, Matches), the generator, Settings with each sheet and dialog, devices, autofill setup, onboarding (on a second install), "Search HavenKeys…", the fill confirmation and the passkey sheet. Each control is read once with its name, role and state; headings are headings; nothing reads a secret, a length of dots, or "unlabelled"; focus never lands on a hairline or a decoration; every dialog and sheet title is announced; the order follows the screen top to bottom.
- [ ] Switch Access or a keyboard (Tab and Enter) reaches every control the same way, and the focused one is visibly marked.
- [ ] Release APK (`scripts/build-android.sh --release`, then `./gradlew assembleGithubRelease` with the release key, see "Release key custody"): unlock, Home, an item, reveal, copy, edit, sync, autofill in Chrome, a passkey. Nothing crashes for a missing class (R8 with no Material).
- [ ] FLAG_SECURE: screenshots blocked and the recents thumbnail blank on every screen, sheet, dialog and menu.

### Android M4
- [ ] Chrome, https checkout with number, expiry (one field) and CVV: rows show `•••• 1111 · 04/33`; tapping fills all three; a field already typed in stays.
- [ ] Chrome, checkout with month and year lists: both chosen.
- [ ] Chrome, Stripe Elements checkout: the card fills inside Stripe's frames.
- [ ] Chrome, http checkout: no card rows.
- [ ] An app's card form: rows; "Confirm before filling" on: HavenKeys asks "Fill ... in <package>?" first.
- [ ] Locked: "Unlock HavenKeys", then the rows, then HavenKeys asks before filling.
- [ ] An address form: the identity fills names, address and phone; a CPF field: "Fill CPF too" asks first; on http no document row.
- [ ] Type a new card and submit: Android's save sheet; saved; type it again: nothing new.
- [ ] Offline: saving a new card says the card was not saved.
- [ ] Cards fill in both Chrome and Firefox (each browser's first field may be its own address bar; the page's site must still be read from the first field inside the page).
- [ ] TalkBack reads the card rows; dark theme.

Passkeys (Android M3; the same list is in `security-review.md`): see "Android
M3" there. Run it on a real Android 14+ phone with HavenKeys enabled in
Passwords & passkeys.

On "Confirm before filling on: each fill asks": with the setting on, each
row opens HavenKeys, which unlocks first if the vault is locked; while it is
unlocked, HavenKeys shows nothing and answers at once (`security-model.md`
§22.4). Check that the row carries no value until it is tapped, not that a
prompt appears.

Also owed: one run of the instrumented tests (`connectedGithubDebugAndroidTest`,
including `KeystoreTest`) on an emulator or phone, and a check that Android
11+ shows HavenKeys the signing certificates of the app being filled
(package visibility, `security-model.md` §22.5).
