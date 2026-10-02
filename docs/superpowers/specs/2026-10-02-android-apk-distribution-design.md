# Android APK distribution — Design

Date: 2026-10-02. Amends the release part of
`docs/superpowers/specs/2026-10-01-android-app-design.md` §12.2.

## 1. Goal

The website says HavenKeys has an Android app and offers a direct APK
download. The APK is a signed release build published on GitHub Releases.
Google Play, the in-app updater (§12.2 `github` flavor updater) and the
`play` flavor's package-visibility answer are out of scope.

## 2. Constraints

* A debug APK is never published: a debuggable app lets anyone with USB
  debugging on the phone read its files and attach a debugger to an
  unlocked vault. Only a release build signed with the HavenKeys release key
  is published, and the workflow fails if the APK is debuggable.
* Android installs an update only when it is signed with the same key. The
  key is permanent: losing it forces every user to uninstall and sign in
  again. It stays outside the repo with an offline backup.
* The website's desktop download and the desktop auto-updater both read
  GitHub's "latest release". An Android release must never become it.
* The release APK carries the release-mode Rust library, never a debug one
  (final review: a debug library accepts plain HTTP to localhost).

## 3. Release key

* The user creates it once on their machine:
  `keytool -genkeypair -v -keystore havenkeys-release.jks -alias havenkeys
  -keyalg RSA -keysize 4096 -validity 12000` with a strong password.
* Backups: the `.jks` file and its passwords offline (for example a
  HavenKeys secure note plus a USB copy). Never committed.
* GitHub Actions secrets: `ANDROID_KEYSTORE_BASE64`,
  `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`.
* Custody is documented in `docs/development.md` and `docs/android.md`.

## 4. Gradle signing

* `app/build.gradle.kts` defines a `release` signing config only when all
  four values are present in the environment (`HAVENKEYS_KEYSTORE_FILE`,
  `HAVENKEYS_KEYSTORE_PASSWORD`, `HAVENKEYS_KEY_ALIAS`,
  `HAVENKEYS_KEY_PASSWORD`). With any one missing, the release build stays
  unsigned, as today. It never falls back to the debug key, and a missing
  key never fails a normal build.
* The decision ("all four present → sign") is a small function with a JVM
  unit test.

## 5. Release workflow

`.github/workflows/android-release.yml`, on a pushed tag `android-v*`:

1. Check the tag equals `android-v<versionName>` from `build.gradle.kts`.
2. Install the SDK packages with the runner's `sdkmanager` (as `android.yml`).
3. `scripts/build-android.sh --release`, then decode the keystore secret to
   a file in the runner's temp directory and run `./gradlew
   assembleGithubRelease` with the four signing values set.
4. Verify: `apksigner verify --print-certs` succeeds and its certificate
   SHA-256 is printed; `aapt2 dump badging` (or the manifest) shows the APK
   is not debuggable — fail otherwise.
5. Rename to `HavenKeys-<version>.apk`, write `HavenKeys-<version>.apk.sha256`.
6. Create a **draft** GitHub Release for the tag with `--latest=false`,
   attach both files, and put the certificate's SHA-256 fingerprint in the
   release notes.
7. The keystore file is deleted at the end of the job (`if: always()`).

The user reviews the draft and publishes it, as with desktop releases.
Actions are pinned by commit SHA, like the other workflows.

## 6. Website

### 6.1 Finding the release (`apps/web/src/lib/releases.ts`)

* `fetchLatestAndroidRelease()`: `GET /repos/rochasamuel/havenkeys/releases`
  (first page), the newest entry that is not a draft and whose `tag_name`
  starts with `android-v`.
* `pickAndroidAsset(assets)`: the `.apk` whose `browser_download_url` starts
  with `RELEASE_ASSET_PREFIX` (the existing trust check); also the matching
  `.apk.sha256` asset when present.
* The desktop path (`fetchLatestRelease`, `pickAsset`) is unchanged.

### 6.2 Download page

* An Android card beside the desktop platforms, marked as "yours" on an
  Android user agent. It shows: "Download APK (vX.Y.Z)", an "Early release"
  label, a link to the `.sha256` file, and the signing certificate's SHA-256
  fingerprint (a constant in the site, set when the key exists; until then
  the line is omitted).
* Install steps: allow installing from the browser when Android asks; open
  HavenKeys and sign in with the Emergency Kit; turn on autofill (Settings →
  Autofill setup); in Chrome, Settings → Autofill services → "Autofill using
  another service".
* Browser note: Chrome and Firefox work; Samsung Internet only lets
  password managers on its own approved list fill.
* No Android release yet, or the GitHub API unreachable: the card says
  "Coming soon" and links to the releases page.

### 6.3 Home page

Android appears where the site names its platforms, linking to the download
page.

### 6.4 Copy

Every string in `apps/web/src/i18n/en.tsx` and `pt-BR.tsx`.

## 7. Versions

The first release is `android-v0.1.0` (`versionName 0.1.0`, `versionCode 1`).
Each later release raises both in `build.gradle.kts`; the workflow refuses a
tag that does not match `versionName`.

## 8. Documentation

* `docs/website.md`: "Cutting an Android release".
* `docs/android.md`, `docs/development.md`: release key custody.
* `docs/security-model.md`: distribution (sideloaded signed APK, checking
  the certificate fingerprint, no debuggable build ships).
* `docs/security-review.md`: the release-mode Rust library item closed by
  the workflow.
* `README.md`: Android download line.

## 9. Testing

* Website (vitest): the Android release is found among desktop releases;
  drafts and untrusted URLs are rejected; no Android release gives null; the
  desktop lookup is unaffected.
* Gradle (JVM): the signing decision.
* Workflow: first exercised by the first tag, which only makes a draft.
  The draft APK is then downloaded, checked with `apksigner` (certificate,
  not debuggable) and installed on the user's phone. The debug build must be
  uninstalled first (different key), which removes the phone's local vault
  copy; the server copy is untouched and the user signs in again.
