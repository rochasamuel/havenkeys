# Android

The Android app (`apps/android`, application ID `net.havenkeys.android`) is
a full client of your `havenkeys-server` account, like the desktop app.
Design: `docs/superpowers/specs/2026-10-01-android-app-design.md`. Security:
`docs/security-model.md` §22 and `docs/threat-model.md` T12.

> This software has not undergone an independent security audit.

**Status: Android M1.** Sign in with the Emergency Kit (QR code or typed) or
an invite; unlock with the master password or, once turned on, a fingerprint
or face; browse, search, reveal, copy and read TOTP codes offline; generate
passwords; fill logins and TOTP codes in apps and browsers through Android
Autofill. Creating and editing items, saving from Autofill, passkeys, cards
and identities in Autofill, and an in-app updater come in later milestones.

**Nothing in `apps/android` has run on a phone or an emulator yet.** It
compiles, its JVM unit tests pass, and the Rust it calls is tested on the
host. The instrumented tests (`src/androidTest`) are compiled but have not
run. The manual checklist at the end of this file is what remains.

Minimum Android 9 (API 28); compiled against and targeting API 36.

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
| Release APK (unsigned) | `./gradlew assembleGithubRelease` → `app/build/outputs/apk/github/release/app-github-release-unsigned.apk` |
| Instrumented tests (needs a device or emulator) | `./gradlew connectedGithubDebugAndroidTest` |
| Compile the instrumented tests only | `./gradlew assembleGithubDebugAndroidTest` |

The two flavors, `github` and `play`, are identical in M1; they will differ
when the `github` flavor gains its updater. Release builds are minified with
R8. No signing configuration is committed, so a release build is unsigned.

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

## Running against a local server

The server address must be `https://`, except `http://localhost` (or
`127.0.0.1`), which Rust's transport accepts in every build. On a phone,
`localhost` is the phone itself; `adb reverse` forwards it to your machine.

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
as `10.0.2.2`, but Rust refuses plain HTTP to anything but `localhost`, so
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

To be completed when the first Android release is cut. What is decided
(spec §12.2):

* APKs are signed with one release key, generated by the project owner,
  never by an agent or in CI.
* The key and its passwords stay outside the repository, with an offline
  backup. They are never committed, pasted into an issue or PR, or printed
  in a build log.
* If the app moves to Google Play, the same key is uploaded to Play App
  Signing, so sideloaded installs keep updating.
* Android refuses an update signed by another key, so losing the key means
  every user must uninstall and reinstall, which deletes the phone's replica
  (the server keeps the vault; signing in again needs the Emergency Kit).

Where the key is stored, who holds the backup, how releases are signed (and
whether CI ever holds the key), and the rotation procedure are not written
yet.

## Manual checklist

None of these has been run. Record results in `docs/security-review.md`
("Android M1"), where the same list lives.

- [ ] Real phone, Android 14+: sign in by scanning the kit; unlock with password; enroll fingerprint; unlock with fingerprint; reboot → password required; add a fingerprint → bundle refused, password required.
- [ ] Chrome (Autofill using another service) and Firefox: login on github.com fills after a tap; github.com.evil.com (hosts file or a test domain) offers nothing; an http page does not get an https login.
- [ ] GitHub app: offered through Digital Asset Links or after "Search HavenKeys…" binding; a sideloaded app with the same package name and another key gets nothing.
- [ ] Google app (WebView sign-in): its WebView's domain is not trusted.
- [ ] Locked vault: "Unlock HavenKeys" appears; nothing fills without unlocking.
- [ ] Confirm before filling on: each fill asks; off: one tap fills.
- [ ] TOTP: OTP field after a login offers the code.
- [ ] Recents thumbnail is blank; screenshots are blocked.
- [ ] Airplane mode: unlock, reveal, TOTP and autofill work; sync shows offline.

On "Confirm before filling on: each fill asks": with the setting on, each
row opens HavenKeys, which unlocks first if the vault is locked; while it is
unlocked, HavenKeys shows nothing and answers at once (`security-model.md`
§22.4). Check that the row carries no value until it is tapped, not that a
prompt appears.

Also owed: one run of the instrumented tests (`connectedGithubDebugAndroidTest`,
including `KeystoreTest`) on an emulator or phone, and a check that Android
11+ shows HavenKeys the signing certificates of the app being filled
(package visibility, `security-model.md` §22.5).
