# Android APK Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a signed release APK on GitHub Releases from `android-v*` tags and offer it on the website's download page.

**Architecture:** Gradle signs the `release` build only when four environment values are present. A new tag-triggered workflow builds the release-mode Rust library and the signed `githubRelease` APK, verifies it (signed, not debuggable), and attaches it with its SHA-256 to a draft release that is never marked "latest". The website finds the newest published `android-v*` release through the GitHub releases list and shows an Android card with install steps.

**Tech Stack:** Gradle Kotlin DSL (AGP), GitHub Actions, `apksigner`/`aapt2` (build-tools 36.0.0), React + TypeScript + vitest (`apps/web`).

**Spec:** `docs/superpowers/specs/2026-10-02-android-apk-distribution-design.md`

## Global Constraints

- A debug APK is never published; the workflow fails if the APK is debuggable or unsigned.
- Release signing only from all four of `HAVENKEYS_KEYSTORE_FILE`, `HAVENKEYS_KEYSTORE_PASSWORD`, `HAVENKEYS_KEY_ALIAS`, `HAVENKEYS_KEY_PASSWORD`; otherwise unsigned, never the debug key, never a build failure.
- GitHub secrets: `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`.
- Tags `android-v<versionName>`; first release `android-v0.1.0` (`versionName 0.1.0`, `versionCode 1`).
- Android releases are drafts created with `--latest=false`; the desktop "latest" lookup is unchanged.
- Release assets: `HavenKeys-<version>.apk` and `HavenKeys-<version>.apk.sha256`.
- The release APK carries the release-mode Rust library (`scripts/build-android.sh --release`).
- Workflow actions pinned by commit SHA, as in `.github/workflows/android.yml`.
- Website: every string in `apps/web/src/i18n/en.tsx` and `pt-BR.tsx`; the APK is accepted only from `RELEASE_ASSET_PREFIX`.
- Commits carry no Co-Authored-By trailer.

## Review Focus

1. The GitHub API is unreachable or rate-limited (60 requests/hour without a token) → the Android card says it isn't released yet and links to the releases page; it never breaks the desktop cards. Pinned in Task 3 (`fetchLatestAndroidRelease` returns null on a failed fetch).
2. A newer desktop release, a draft or a pre-release sits ahead of the Android release in the list → the published `android-v*` release is still found; drafts and pre-releases are skipped. Pinned in Task 3.
3. Only some signing secrets are set → Gradle builds unsigned and the workflow fails at `apksigner verify` instead of publishing. Pinned in Task 2's verify step and Task 1's two-build check.
4. A release asset served from another host, or a release without an `.apk` → no download button for it. Pinned in Task 3.
5. An Android phone visiting the download page → the Android card is marked "Your system"; Linux detection still excludes Android. Pinned in Task 4.

---

### Task 1: Release signing in Gradle and key custody docs

**Files:**
- Modify: `apps/android/app/build.gradle.kts`
- Modify: `docs/android.md` (new section "Release key custody"), `docs/development.md` (one paragraph pointing to it)

**Interfaces:**
- Produces: `./gradlew assembleGithubRelease` signs with the release key when the four `HAVENKEYS_*` variables are set, writing `app/build/outputs/apk/github/release/app-github-release.apk`; otherwise `app-github-release-unsigned.apk`. Task 2 relies on these names.

- [ ] **Step 1: Add the signing config**

In `apps/android/app/build.gradle.kts`, before the `android {` block:

```kotlin
// Release signing comes only from the environment (the release workflow's
// secrets, or a local export). Without all four values the release build
// stays unsigned; it is never signed with the debug key.
val releaseSigning: Map<String, String>? = listOf(
    "HAVENKEYS_KEYSTORE_FILE",
    "HAVENKEYS_KEYSTORE_PASSWORD",
    "HAVENKEYS_KEY_ALIAS",
    "HAVENKEYS_KEY_PASSWORD",
).associateWith { providers.environmentVariable(it).orNull.orEmpty() }
    .takeIf { values -> values.values.all { it.isNotEmpty() } }
```

Inside `android { … }`, before `buildTypes`:

```kotlin
    signingConfigs {
        if (releaseSigning != null) {
            create("release") {
                storeFile = file(releaseSigning.getValue("HAVENKEYS_KEYSTORE_FILE"))
                storePassword = releaseSigning.getValue("HAVENKEYS_KEYSTORE_PASSWORD")
                keyAlias = releaseSigning.getValue("HAVENKEYS_KEY_ALIAS")
                keyPassword = releaseSigning.getValue("HAVENKEYS_KEY_PASSWORD")
            }
        }
    }
```

In `buildTypes { release { … } }` add:

```kotlin
            signingConfig = signingConfigs.findByName("release")
```

- [ ] **Step 2: Build without the values → unsigned**

```bash
export JAVA_HOME=$HOME/.local/opt/jdk-17.0.20.1+1 PATH=$HOME/.local/opt/jdk-17.0.20.1+1/bin:$PATH ANDROID_HOME=$HOME/Android/Sdk
ANDROID_NDK_HOME=$ANDROID_HOME/ndk/30.0.16248370 scripts/build-android.sh --release
cd apps/android && ./gradlew assembleGithubRelease && ls app/build/outputs/apk/github/release/
```
Expected: `app-github-release-unsigned.apk` and no `app-github-release.apk`.

- [ ] **Step 3: Build with a throwaway keystore → signed**

Use a scratch directory outside the repo (e.g. `$TMPDIR`):

```bash
keytool -genkeypair -keystore "$TMPDIR/throwaway.jks" -alias throwaway -keyalg RSA -keysize 2048 \
  -validity 1 -storepass throwaway -keypass throwaway -dname "CN=Throwaway"
rm -rf app/build/outputs/apk/github/release
HAVENKEYS_KEYSTORE_FILE="$TMPDIR/throwaway.jks" HAVENKEYS_KEYSTORE_PASSWORD=throwaway \
HAVENKEYS_KEY_ALIAS=throwaway HAVENKEYS_KEY_PASSWORD=throwaway ./gradlew assembleGithubRelease
"$ANDROID_HOME/build-tools/36.0.0/apksigner" verify --print-certs app/build/outputs/apk/github/release/app-github-release.apk
"$ANDROID_HOME/build-tools/36.0.0/aapt2" dump badging app/build/outputs/apk/github/release/app-github-release.apk | grep -c application-debuggable
```
Expected: `apksigner` prints `Signer #1 certificate DN: CN=Throwaway`; the `grep -c` prints `0`. Then delete the throwaway keystore.

- [ ] **Step 4: Restore debug native libraries and run the normal checks**

```bash
cd ../.. && ANDROID_NDK_HOME=$ANDROID_HOME/ndk/30.0.16248370 scripts/build-android.sh
git status --porcelain apps/android/app/src/main/kotlin/uniffi   # must be empty
cd apps/android && ./gradlew detekt testGithubDebugUnitTest lintGithubDebug assembleGithubDebug
```
Expected: no binding changes; BUILD SUCCESSFUL.

- [ ] **Step 5: Document key custody**

Add to `docs/android.md` a section `## Release key custody` (replacing the existing release-key placeholder text if present) containing:

````markdown
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
  secure note and a copy on removable storage. Never commit them.
* Add four repository secrets (Settings → Secrets and variables → Actions):
  `ANDROID_KEYSTORE_BASE64` (`base64 -w0 havenkeys-release.jks`),
  `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` (`havenkeys`) and
  `ANDROID_KEY_PASSWORD`.
* Its certificate fingerprint (`keytool -list -v -keystore
  havenkeys-release.jks | grep SHA256`) is shown on the download page and in
  every release's notes.
* Gradle signs the release build only when `HAVENKEYS_KEYSTORE_FILE`,
  `HAVENKEYS_KEYSTORE_PASSWORD`, `HAVENKEYS_KEY_ALIAS` and
  `HAVENKEYS_KEY_PASSWORD` are all set; otherwise it builds unsigned.
````

Add to `docs/development.md`, where release processes are described, one sentence: "Android releases are signed with a key kept outside the repository; see `docs/android.md` → Release key custody."

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/build.gradle.kts docs/android.md docs/development.md
git commit -m "build(android): sign the release build from the environment only"
```

---

### Task 2: The Android release workflow

**Files:**
- Create: `.github/workflows/android-release.yml`
- Modify: `docs/website.md` (new section "Cutting an Android release")

**Interfaces:**
- Consumes: Task 1's signing variables and output path `apps/android/app/build/outputs/apk/github/release/app-github-release.apk`.
- Produces: a draft release for tag `android-vX.Y.Z` with assets `HavenKeys-X.Y.Z.apk` and `HavenKeys-X.Y.Z.apk.sha256`, not marked latest. Task 3 relies on the tag prefix and asset suffixes.

- [ ] **Step 1: Write the workflow**

`.github/workflows/android-release.yml`:

```yaml
name: android-release

on:
  push:
    tags:
      - "android-v*"

permissions:
  contents: write

env:
  # Same NDK as apps/android/app/build.gradle.kts (ndkVersion).
  ANDROID_NDK_VERSION: 30.0.16248370
  BUILD_TOOLS: 36.0.0

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0

      - name: Check the tag matches the app version
        run: |
          version=$(sed -n 's/^ *versionName = "\(.*\)"/\1/p' apps/android/app/build.gradle.kts)
          test -n "$version" || { echo "versionName not found"; exit 1; }
          test "${GITHUB_REF_NAME#android-v}" = "$version" || { echo "Tag $GITHUB_REF_NAME does not match versionName $version"; exit 1; }
          echo "VERSION=$version" >> "$GITHUB_ENV"

      - uses: gradle/actions/wrapper-validation@3f5f9adaf7d9fecd50b5935e54106014257a94e6 # v6.4.0
      - uses: actions/setup-java@cf277c60eb25467037889841efdb72551f06f6c3 # v4.9.1
        with: { distribution: temurin, java-version: "17" }
      - name: Android SDK packages
        run: |
          SDKMANAGER="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
          yes | "$SDKMANAGER" --licenses > /dev/null
          "$SDKMANAGER" "platforms;android-36" "build-tools;${BUILD_TOOLS}" "ndk;${ANDROID_NDK_VERSION}"
      - uses: dtolnay/rust-toolchain@6bed0761d98439e5a578e2877258200ad565ba87 # stable
        with: { targets: "aarch64-linux-android,armv7-linux-androideabi,x86_64-linux-android" }
      - run: cargo install cargo-ndk --version 4.1.2 --locked

      # Release-mode Rust: a debug library would accept plain HTTP to localhost.
      - run: ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION" scripts/build-android.sh --release

      - name: Build the signed APK
        working-directory: apps/android
        env:
          KEYSTORE_BASE64: ${{ secrets.ANDROID_KEYSTORE_BASE64 }}
          HAVENKEYS_KEYSTORE_PASSWORD: ${{ secrets.ANDROID_KEYSTORE_PASSWORD }}
          HAVENKEYS_KEY_ALIAS: ${{ secrets.ANDROID_KEY_ALIAS }}
          HAVENKEYS_KEY_PASSWORD: ${{ secrets.ANDROID_KEY_PASSWORD }}
        run: |
          export HAVENKEYS_KEYSTORE_FILE="$RUNNER_TEMP/release.jks"
          printf '%s' "$KEYSTORE_BASE64" | base64 -d > "$HAVENKEYS_KEYSTORE_FILE"
          ./gradlew assembleGithubRelease

      - name: Verify the APK is signed and not debuggable
        run: |
          apk=apps/android/app/build/outputs/apk/github/release/app-github-release.apk
          test -f "$apk" || { echo "No signed APK: are all four signing secrets set?"; exit 1; }
          tools="$ANDROID_HOME/build-tools/$BUILD_TOOLS"
          "$tools/apksigner" verify --print-certs "$apk" | tee certs.txt
          cert=$(sed -n 's/^Signer #1 certificate SHA-256 digest: //p' certs.txt)
          test -n "$cert" || { echo "No signing certificate"; exit 1; }
          if "$tools/aapt2" dump badging "$apk" | grep -q application-debuggable; then
            echo "The APK is debuggable"; exit 1
          fi
          mkdir dist
          cp "$apk" "dist/HavenKeys-$VERSION.apk"
          (cd dist && sha256sum "HavenKeys-$VERSION.apk" > "HavenKeys-$VERSION.apk.sha256")
          echo "CERT_SHA256=$cert" >> "$GITHUB_ENV"

      - name: Attach to a draft release (never "latest")
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          cat > notes.md <<EOF
          HavenKeys for Android $VERSION.

          Signing certificate SHA-256: \`$CERT_SHA256\`

          Check a download with \`sha256sum -c HavenKeys-$VERSION.apk.sha256\`.
          EOF
          gh release create "$GITHUB_REF_NAME" --draft --latest=false \
            --title "HavenKeys Android $VERSION" --notes-file notes.md \
            "dist/HavenKeys-$VERSION.apk" "dist/HavenKeys-$VERSION.apk.sha256"

      - name: Remove the keystore
        if: always()
        run: rm -f "$RUNNER_TEMP/release.jks"
```

- [ ] **Step 2: Lint the workflow locally**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/android-release.yml')); print('yaml ok')"
sed -n 's/^ *versionName = "\(.*\)"/\1/p' apps/android/app/build.gradle.kts
```
Expected: `yaml ok`, then `0.1.0` (the version-check expression extracts exactly the version).

- [ ] **Step 3: Simulate the verify step on Task 1's two APKs**

Run the "Verify" step's commands by hand against an unsigned build (expect "No signed APK" and exit 1) and against a throwaway-signed build (expect a certificate line and success). Record both outputs in the report.

- [ ] **Step 4: Document cutting a release**

Add to `docs/website.md`, after "Cutting a desktop release":

````markdown
## Cutting an Android release

The download page links to the newest **published** release whose tag starts
with `android-v`. Android releases are never marked "latest", so the desktop
download and the desktop auto-updater keep pointing at desktop releases.

1. Raise `versionCode` and `versionName` in `apps/android/app/build.gradle.kts`
   (Android refuses an update whose `versionCode` is not higher), commit, push.
2. Tag and push: `git tag android-v0.1.0 && git push origin android-v0.1.0`.
3. `.github/workflows/android-release.yml` builds the release-mode Rust
   library and the signed APK, refuses an unsigned or debuggable APK, and
   attaches `HavenKeys-<version>.apk` and its `.sha256` to a **draft**
   release with the signing certificate's fingerprint in the notes.
4. Download the draft APK, install it on a phone, then click **Publish
   release**.

The signing key's custody is in `docs/android.md` → Release key custody.
````

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/android-release.yml docs/website.md
git commit -m "ci(android): build, verify and attach a signed APK on android-v tags"
```

---

### Task 3: Finding the Android release on the website

**Files:**
- Modify: `apps/web/src/lib/releases.ts`
- Test: `apps/web/src/lib/releases.test.ts`

**Interfaces:**
- Produces (exact names, used by Task 4):
  - `interface ReleaseListing extends LatestRelease { draft: boolean; prerelease: boolean }`
  - `function pickAndroidRelease(releases: ReleaseListing[]): LatestRelease | null`
  - `function pickAndroidAssets(assets: ReleaseAsset[]): { apk: ReleaseAsset; checksum: ReleaseAsset | null } | null`
  - `async function fetchLatestAndroidRelease(): Promise<LatestRelease | null>`
  - `const ANDROID_CERT_SHA256: string | null` (null until the release key exists)

- [ ] **Step 1: Write the failing tests**

Append to `apps/web/src/lib/releases.test.ts` (and add the new names to its import line):

```ts
describe("pickAndroidRelease", () => {
  const release = (tag: string, extra: Partial<ReleaseListing> = {}): ReleaseListing => ({
    tag_name: tag,
    html_url: `https://github.com/rochasamuel/havenkeys/releases/tag/${tag}`,
    assets: [],
    draft: false,
    prerelease: false,
    ...extra,
  });

  it("finds the newest Android release behind newer desktop releases", () => {
    const list = [release("desktop-v0.14.0"), release("android-v0.2.0"), release("android-v0.1.0")];
    expect(pickAndroidRelease(list)?.tag_name).toBe("android-v0.2.0");
  });

  it("skips drafts and pre-releases", () => {
    const list = [
      release("android-v0.3.0", { draft: true }),
      release("android-v0.2.0", { prerelease: true }),
      release("android-v0.1.0"),
    ];
    expect(pickAndroidRelease(list)?.tag_name).toBe("android-v0.1.0");
  });

  it("returns null without an Android release", () => {
    expect(pickAndroidRelease([release("desktop-v0.13.0")])).toBeNull();
  });
});

describe("pickAndroidAssets", () => {
  const asset = (name: string, url = `${RELEASE_ASSET_PREFIX}android-v0.1.0/${name}`): ReleaseAsset => ({
    name,
    browser_download_url: url,
  });

  it("finds the APK and its checksum", () => {
    const picked = pickAndroidAssets([asset("HavenKeys-0.1.0.apk.sha256"), asset("HavenKeys-0.1.0.apk")]);
    expect(picked?.apk.name).toBe("HavenKeys-0.1.0.apk");
    expect(picked?.checksum?.name).toBe("HavenKeys-0.1.0.apk.sha256");
  });

  it("ignores an APK hosted anywhere else", () => {
    expect(pickAndroidAssets([asset("HavenKeys-0.1.0.apk", "https://evil.example/HavenKeys-0.1.0.apk")])).toBeNull();
  });

  it("returns null when the release has no APK", () => {
    expect(pickAndroidAssets([asset("HavenKeys-0.1.0.apk.sha256")])).toBeNull();
  });
});

describe("fetchLatestAndroidRelease", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the newest published Android release", async () => {
    const list = [
      { tag_name: "desktop-v0.13.0", html_url: "x", assets: [], draft: false, prerelease: false },
      { tag_name: "android-v0.1.0", html_url: "y", assets: [], draft: false, prerelease: false },
    ];
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => list }));
    expect((await fetchLatestAndroidRelease())?.tag_name).toBe("android-v0.1.0");
  });

  it("returns null when the request fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    expect(await fetchLatestAndroidRelease()).toBeNull();
  });

  it("returns null on a rate-limit response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({}) }));
    expect(await fetchLatestAndroidRelease()).toBeNull();
  });

  it("returns null when the answer is not a list", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => ({ message: "x" }) }));
    expect(await fetchLatestAndroidRelease()).toBeNull();
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `pnpm --filter @havenkeys/web test -- releases`
Expected: FAIL (the new functions are not exported).

- [ ] **Step 3: Implement**

Append to `apps/web/src/lib/releases.ts`:

```ts
export interface ReleaseListing extends LatestRelease {
  draft: boolean;
  prerelease: boolean;
}

/** SHA-256 of the Android release key's certificate; null until the key exists. */
export const ANDROID_CERT_SHA256: string | null = null;

const ANDROID_TAG_PREFIX = "android-v";
const RELEASES_LIST_URL = "https://api.github.com/repos/rochasamuel/havenkeys/releases?per_page=100";

/** The newest published Android release (GitHub lists newest first). */
export function pickAndroidRelease(releases: ReleaseListing[]): LatestRelease | null {
  return (
    releases.find((r) => !r.draft && !r.prerelease && r.tag_name.startsWith(ANDROID_TAG_PREFIX)) ?? null
  );
}

export function pickAndroidAssets(
  assets: ReleaseAsset[],
): { apk: ReleaseAsset; checksum: ReleaseAsset | null } | null {
  const trusted = assets.filter((a) => a.browser_download_url.startsWith(RELEASE_ASSET_PREFIX));
  const apk = trusted.find((a) => a.name.endsWith(".apk"));
  if (!apk) return null;
  const checksum = trusted.find((a) => a.name === `${apk.name}.sha256`) ?? null;
  return { apk, checksum };
}

export async function fetchLatestAndroidRelease(): Promise<LatestRelease | null> {
  try {
    const response = await fetch(RELEASES_LIST_URL, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!response.ok) return null;
    const data = (await response.json()) as unknown;
    if (!Array.isArray(data)) return null;
    return pickAndroidRelease(data as ReleaseListing[]);
  } catch {
    return null;
  }
}
```

- [ ] **Step 4: Run the tests**

Run: `pnpm --filter @havenkeys/web test`
Expected: all pass, including the existing desktop `pickAsset`/`fetchLatestRelease` tests.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/lib/releases.ts apps/web/src/lib/releases.test.ts
git commit -m "feat(web): find the newest published Android release"
```

---

### Task 4: The Android card, install steps and home page copy

**Files:**
- Create: `apps/web/src/components/AndroidDownload.tsx`
- Modify: `apps/web/src/pages/Download.tsx`, `apps/web/src/i18n/en.tsx`, `apps/web/src/i18n/pt-BR.tsx`, `apps/web/src/styles/global.css`
- Test: `apps/web/src/pages/Download.test.tsx` (new) — only if the web package already has a DOM test setup (`jsdom`/`@testing-library/react` in `apps/web/package.json`); otherwise test `detectOs` as an exported pure function in `apps/web/src/lib/os.test.ts`.

**Interfaces:**
- Consumes: Task 3's `fetchLatestAndroidRelease`, `pickAndroidAssets`, `ANDROID_CERT_SHA256`, `RELEASES_PAGE_URL`.
- Produces: `detectOs(ua: string): "windows" | "macos" | "linux" | "android" | null` exported from `apps/web/src/lib/os.ts` (moved out of `Download.tsx`).

- [ ] **Step 1: Write the failing test for OS detection**

`apps/web/src/lib/os.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { detectOs } from "./os";

describe("detectOs", () => {
  it("recognises an Android phone", () => {
    expect(detectOs("Mozilla/5.0 (Linux; Android 16; SM-S926B) AppleWebKit/537.36 Chrome/141 Mobile")).toBe("android");
  });
  it("keeps desktop Linux", () => {
    expect(detectOs("Mozilla/5.0 (X11; Linux x86_64) Gecko/20100101 Firefox/143.0")).toBe("linux");
  });
  it("keeps Windows and macOS", () => {
    expect(detectOs("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("windows");
    expect(detectOs("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)")).toBe("macos");
  });
});
```

Run: `pnpm --filter @havenkeys/web test -- os` → FAIL (module missing).

- [ ] **Step 2: Move and extend `detectOs`**

`apps/web/src/lib/os.ts`:

```ts
export type Os = "windows" | "macos" | "linux" | "android";

export function detectOs(ua: string): Os | null {
  if (/Android/i.test(ua)) return "android";
  if (/Windows/i.test(ua)) return "windows";
  if (/Macintosh|Mac OS X/i.test(ua)) return "macos";
  if (/Linux|X11/i.test(ua)) return "linux";
  return null;
}
```

In `Download.tsx` delete the local `detectOs`, import it, and use
`useState(() => (typeof navigator === "undefined" ? null : detectOs(navigator.userAgent)))`.
Widen the `PLATFORMS` `os` field type to `Os` from `lib/os.ts`.

Run: `pnpm --filter @havenkeys/web test` → PASS.

- [ ] **Step 3: Add the copy**

In `apps/web/src/i18n/en.tsx`, inside `download`, add:

```tsx
    android: {
      label: "Android",
      format: "APK for Android 9 and later",
      early: "Early release",
      downloadApk: "Download APK",
      checksum: "SHA-256 checksum",
      certificate: "Signing certificate (SHA-256)",
      comingSoon: "The Android app isn’t released yet.",
      stepsTitle: "Installing on Android",
      steps: [
        "Download the APK and open it. Android asks once to allow installing apps from your browser.",
        "Open HavenKeys and sign in by scanning your Emergency Kit, or with an invite.",
        "Turn on autofill: in HavenKeys, Settings → Autofill setup.",
        "In Chrome, open Settings → Autofill services and choose “Autofill using another service”.",
      ],
      browsers:
        "Autofill works in apps, Chrome and Firefox. Samsung Internet only lets password managers on Samsung’s own list fill, and HavenKeys isn’t on it.",
    },
```

Change `home.heroMeta` to `"Free and open source · Windows, macOS, Linux and Android · Chrome and Firefox"` and `home.closerLede` to `"Install the app on your computer or phone, point it at your server, add the extension."`.

In `apps/web/src/i18n/pt-BR.tsx`, the same keys:

```tsx
    android: {
      label: "Android",
      format: "APK para Android 9 ou mais recente",
      early: "Versão inicial",
      downloadApk: "Baixar APK",
      checksum: "Checksum SHA-256",
      certificate: "Certificado de assinatura (SHA-256)",
      comingSoon: "O app para Android ainda não foi lançado.",
      stepsTitle: "Instalando no Android",
      steps: [
        "Baixe o APK e abra. O Android pede uma vez para permitir instalar apps pelo navegador.",
        "Abra o HavenKeys e entre escaneando o seu Emergency Kit, ou com um convite.",
        "Ative o preenchimento automático: no HavenKeys, Configurações → Configurar preenchimento automático.",
        "No Chrome, abra Configurações → Serviços de preenchimento automático e escolha “Preencher automaticamente usando outro serviço”.",
      ],
      browsers:
        "O preenchimento funciona em apps, no Chrome e no Firefox. O Samsung Internet só deixa preencher os gerenciadores de senhas da lista da própria Samsung, e o HavenKeys não está nela.",
    },
```

`home.heroMeta`: `"Gratuito e open source · Windows, macOS, Linux e Android · Chrome e Firefox"`; `home.closerLede`: `"Instale o app no computador ou no celular, aponte para o seu servidor e adicione a extensão."`.

Check the Android app's own pt-BR strings (`apps/android/app/src/main/res/values-pt-rBR/strings.xml`) for the exact names of "Settings" and "Autofill setup" and use them in step 3 of both languages.

- [ ] **Step 4: Build the card**

`apps/web/src/components/AndroidDownload.tsx`:

```tsx
import { useEffect, useState } from "react";
import { Icon } from "./Icon";
import { useI18n } from "../i18n/context";
import {
  ANDROID_CERT_SHA256,
  fetchLatestAndroidRelease,
  pickAndroidAssets,
  RELEASES_PAGE_URL,
  type LatestRelease,
} from "../lib/releases";

type State = { status: "loading" } | { status: "ready"; release: LatestRelease } | { status: "none" };

export function AndroidDownload({ yours }: { yours: boolean }) {
  const { t } = useI18n();
  const a = t.download.android;
  const [state, setState] = useState<State>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    fetchLatestAndroidRelease().then((release) => {
      if (!cancelled) setState(release ? { status: "ready", release } : { status: "none" });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const assets = state.status === "ready" ? pickAndroidAssets(state.release.assets) : null;
  const version = state.status === "ready" ? state.release.tag_name.replace(/^android-v/, "") : null;

  return (
    <div className={`platform platform--android ${yours ? "platform--yours" : ""}`}>
      <div className="platform__text">
        <h2>{a.label}</h2>
        <p>{a.format}</p>
        <span className="tag">{a.early}</span>
        {yours && <span className="tag">{t.download.yourSystem}</span>}
        {assets?.checksum && (
          <p className="platform__meta">
            <a href={assets.checksum.browser_download_url}>{a.checksum}</a>
          </p>
        )}
        {assets && ANDROID_CERT_SHA256 && (
          <p className="platform__meta">
            {a.certificate}: <code>{ANDROID_CERT_SHA256}</code>
          </p>
        )}
        {state.status === "none" && <p className="platform__meta">{a.comingSoon}</p>}
      </div>
      {assets ? (
        <a className={`btn ${yours ? "btn--primary" : "btn--ghost"}`} href={assets.apk.browser_download_url}>
          <Icon name="download" />
          {a.downloadApk} ({version})
        </a>
      ) : (
        <a className="btn btn--ghost" href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
          {t.download.viewReleases}
          <Icon name="external" size={15} />
        </a>
      )}
    </div>
  );
}
```

In `Download.tsx`, render `<AndroidDownload yours={os === "android"} />` as the last child of the `download__grid` section, and after that section add:

```tsx
      <section className="setup setup--android">
        <h2>{d.android.stepsTitle}</h2>
        <ol className="setup__steps">
          {d.android.steps.map((step) => (
            <li key={step}>
              <p>{step}</p>
            </li>
          ))}
        </ol>
        <p className="setup__note">{d.android.browsers}</p>
      </section>
```

In `apps/web/src/styles/global.css`, after the `.platform__text p` rule:

```css
.platform__meta {
  margin-top: 0.35rem;
  font-size: 0.85rem;
  overflow-wrap: anywhere;
}

.setup__note {
  margin-top: 1rem;
  color: var(--text-muted, inherit);
}
```

(Use the muted-text token already defined in `global.css`; check its exact name with `grep -n "muted" apps/web/src/styles/global.css` and replace `--text-muted` if it differs.)

- [ ] **Step 5: Typecheck, test and build**

Run: `pnpm --filter @havenkeys/web test && pnpm --filter @havenkeys/web build`
Expected: tests pass; build succeeds (the build includes the typecheck that catches a missing pt-BR key).

- [ ] **Step 6: Check it in a browser**

Run `pnpm --filter @havenkeys/web dev` and open `/download` and `/pt-br/download` (and with a phone user agent via the browser's device toolbar). Expected: the Android card shows "Early release" and, with no Android release published yet, "The Android app isn't released yet." plus "View releases"; the desktop cards are unchanged; the steps and browser note render in both languages; with an Android user agent the card is marked "Your system". Record what was checked in the report.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): Android download card and install steps"
```

---

### Task 5: Security docs and README

**Files:**
- Modify: `docs/security-model.md`, `docs/security-review.md`, `README.md`

- [ ] **Step 1: Security model — distribution**

In `docs/security-model.md`, in the Android section (§22), add a subsection `### 22.15 Distribution`:

```markdown
### 22.15 Distribution

* The APK is published on GitHub Releases from `android-v*` tags by
  `.github/workflows/android-release.yml`, never through a store. It is a
  release build: the workflow refuses an APK that is unsigned or
  debuggable, and builds the release-mode Rust library.
* It is signed with the HavenKeys release key (custody:
  `docs/android.md`). Android installs an update only with the same key, so
  a tampered or re-signed APK cannot replace an installed HavenKeys.
* Each release lists the signing certificate's SHA-256, also shown on the
  download page, and ships `HavenKeys-<version>.apk.sha256`. A first install
  can be checked with `apksigner verify --print-certs`.
* The download page fetches the release list from GitHub's API and accepts
  the APK only from this repository's release-download URL.
```

- [ ] **Step 2: Security review**

In `docs/security-review.md`, find the deferred/parked item about a release APK built after a debug `build-android.sh` (search "jniLibs" or "release APK"). Mark it addressed: the release workflow always runs `scripts/build-android.sh --release` before `assembleGithubRelease` on a clean runner. Keep the note that a local release build still needs `--release` first.

- [ ] **Step 3: README**

In `README.md`, where Android M1 status is described, add: "Android: download the APK from the website's download page or from GitHub Releases (`android-v*`); it is signed with the HavenKeys release key."

- [ ] **Step 4: Commit**

```bash
git add docs/security-model.md docs/security-review.md README.md
git commit -m "docs: how the Android app is distributed"
```
