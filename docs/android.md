# Android

The Android app (`apps/android`, application ID `net.havenkeys.android`) is
a full client of your `havenkeys-server` account, like the desktop app.
Design: `docs/superpowers/specs/2026-10-01-android-app-design.md`. Security:
`docs/security-model.md` §22 and `docs/threat-model.md` T12.

> This software has not undergone an independent security audit.

**Status: Android M3.** Sign in with the Emergency Kit (QR code or typed) or
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
(below). Not yet: editing custom fields (kept as they are), scanning a TOTP
QR code (type or paste the key), saving a login typed while locked, cards and
identities in Autofill, and an in-app updater.

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
| Lint, including the no-logging scan | `./gradlew detekt` (runs `forbidLogging` first) |
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
