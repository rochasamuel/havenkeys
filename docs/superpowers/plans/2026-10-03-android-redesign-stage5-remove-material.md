# Android Redesign, Stage 5: Remove Material Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take Material out of the Android app for good: delete `ui/theme/MaterialBridge.kt` and its call, drop `material3` and `material-icons-extended`, make the build fail on any Material use anywhere, prove nothing visible changed, and close the redesign with a whole-app `/impeccable` review and the owner's TalkBack checklist.

**Architecture:** Since stage 4 no screen reads `MaterialTheme`; `HavenTheme` still wraps its content in `MaterialBridge` only so the `material3` dependency has a theme. Removing the bridge changes one thing a screen could notice: `MaterialTheme` was what set `LocalIndication` (a ripple), so `HavenTheme` now provides the kit's `HavenPress` as the default indication instead. The stage 4 Gradle text scan `forbidMaterialInKit` becomes `forbidMaterial` (every Kotlin source and the version catalog, no exclusions) and detekt's `ForbiddenImport` gets the Material packages, as the spec asks. Robolectric screenshots taken before and after the removal are compared file by file; the after set is the evidence for the final review.

**Tech Stack:** Kotlin 2.4, Jetpack Compose BOM 2026.06.01 (foundation, ui 1.11.4), Gradle 9.8 with dependency verification (`gradle/verification-metadata.xml`, `verify-metadata` on), detekt 1.23.8, Robolectric 4.17.

**Spec:** `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §1 ("nothing in the app reads as Material"), §2 (decision table: `material3` and `material-icons-extended` removed), §5 (theme and kit on foundation, no ripple), §8 stage 5 ("drop `material3` and `material-icons-extended`; a detekt rule forbids `androidx.compose.material` imports"), §10 (per stage: TalkBack pass, `/impeccable`). Index: `docs/superpowers/plans/2026-10-03-android-redesign-index.md` "Stage 5". Previous stage: `docs/superpowers/plans/2026-10-03-android-redesign-stage4-screens.md` (Task 16 left the bridge as the only Material file; Task 18 lists what is left).

## Global Constraints

- After this stage no file under `apps/android/app/src` mentions `androidx.compose.material`, `androidx.compose.material3` or `com.google.android.material`, and `gradle/libs.versions.toml` declares no Material module. `forbidMaterial` has no exclusions.
- The platform window theme (`res/values*/themes.xml`, parent `android:Theme.Material…NoActionBar`) is the framework's window behind Compose, not a library, and stays (stage 4 Task 18 recorded this; nothing here changes it).
- Behaviour and pixels unchanged: every existing test keeps passing, and the Robolectric screenshots before and after the removal are identical file for file (Task 3), except where a difference is explained by content that moves with the clock.
- Dependency verification is never relaxed: no `<trust>` entries, no `verify-metadata` change, no `--dependency-verification lenient|off`. Stale entries may only be deleted, never edited.
- No logging: `forbidLogging` keeps covering every file.
- detekt runs on all of it (`MaxLineLength` 120). Commands run in `apps/android` with the toolchain from `docs/android.md` "Toolchain": `export JAVA_HOME=~/.local/opt/jdk-17 ANDROID_HOME=~/Android/Sdk; export PATH="$JAVA_HOME/bin:$PATH"`.
- The release APK is built unsigned for measuring: the four `HAVENKEYS_*` variables must be unset (`env | grep HAVENKEYS_` prints nothing). `app/src/main/jniLibs` must be the same for the before and after builds: do not run `scripts/build-android.sh` during this stage.
- No emulator on the build machine: TalkBack and device checks go to the owner's checklist in `docs/android.md` (Task 4).
- Commits without Co-Authored-By lines (owner's rule). Each task leaves `testGithubDebugUnitTest detekt assembleGithubDebug` green.

## Entry checks

Run these before Task 1.

1. **Stage 4 is done.** In `docs/superpowers/plans/2026-10-03-android-redesign-index.md` the stage 4 row reads `Done`; `apps/android/DESIGN.md` has "Review notes (stage 4)"; `docs/android.md` has "### Redesign stage 4 (existing screens)". Read the index's "### Stage 5" bullets and DESIGN.md's stage 4 open questions: an open question about a kit component or a screen goes into Task 3's review brief as something to judge, not to settle (settling it is the owner's call).
2. **The bridge is the only Material file.**
   Run: `grep -rln "androidx.compose.material\|com.google.android.material" apps/android/app/src`
   Expected: exactly `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme/MaterialBridge.kt`. Anything else means stage 4 left a screen on Material: stop and report it; this plan does not rebuild screens.
3. **Nothing uses the bridge's contents.**
   Run: `grep -rn "HavenType\b\|DarkScheme\|LightScheme\|MaterialTypography\|MaterialShapes\|MaterialBridge" apps/android/app/src --include='*.kt' | grep -v ui/theme/MaterialBridge.kt`
   Expected: one line, the `MaterialBridge(darkTheme, content)` call in `ui/theme/Theme.kt`. (`HavenType` was the legacy text styles; `HavenTheme.type`, `HavenTypography`, replaced it in stage 2.)
4. **No other way to Material.** `grep -n "material" apps/android/app/build.gradle.kts apps/android/gradle/libs.versions.toml apps/android/app/proguard-rules.pro` shows only `forbidMaterialInKit`, `libs.compose.material3`, `libs.compose.icons` and the two catalog lines `compose-material3` and `compose-icons`. No `com.google.android.material` (Material Components) dependency exists.
5. **The screenshot tests exist.** `ls apps/android/app/src/testDebug/kotlin/net/havenkeys/android/{catalogue/CatalogueScreenshots.kt,screens/ShellScreenshots.kt,screens/ScreenScreenshots.kt}` lists three files, and `grep -n screensDir apps/android/app/build.gradle.kts` shows the `-PscreensDir` property feeding `havenkeys.screens.dir`.

## Decisions recorded for the owner

- **`HavenTheme` provides `HavenPress` as the default indication.** `MaterialTheme` set `LocalIndication` to a ripple; without it Compose falls back to foundation's grey debug highlight. Every kit control passes its indication explicitly (`havenClickable`, the switch, segmented control, choice rows, bottom bar), so nothing changes today; a future `Modifier.clickable { }` written without one now presses the HavenKeys way instead of rippling or greying (the No Ripple Rule). This makes `ui/theme` import one object from `ui/kit` (which already imports `ui/theme`); the cycle is inside one module and kept deliberately rather than moving `HavenPress` into the theme.
- **Both guards, as the spec and the earlier stages asked.** detekt's `ForbiddenImport` gets `androidx.compose.material.*`, `androidx.compose.material3.*` and `com.google.android.material.*` (the spec's "detekt rule"). The Gradle scan stays too, renamed `forbidMaterial`, because it covers what detekt does not: `src/androidTest` (outside detekt's source sets), fully qualified references without an import, and the version catalog (re-adding the dependency fails the build before anything uses it). It stays wired as `forbidMaterialInKit` was: `detekt` depends on it, so CI's `./gradlew detekt` runs it.
- **The debug build still carries `material3` at runtime, through `ui-tooling`.** `androidx.compose.ui:ui-tooling` 1.11.4 (`debugImplementation`, for previews) depends on `material3` at runtime. It is not on any compile classpath, so no source can use it (the compiler and both guards would stop it), and it is not in either release APK. Excluding it from `ui-tooling` could break Android Studio's previews for no shipped gain, so it stays; the `material3`, `material3-android`, `material-ripple` and `material-ripple-android` entries in `verification-metadata.xml` stay with it.
- **Stale verification entries are pruned by hand, six components exactly.** Gradle's `--write-verification-metadata` adds entries and never removes them, and regenerating the file from scratch would also re-pin everything else from whatever the cache holds. After the removal nothing resolves `material-icons-core` or `material-icons-extended` (with their `-android` and `-desktop` variants, all 1.7.8), so those six `<component>` blocks are deleted, nothing else. A full build with verification on (including lint, the instrumented-test APK and the play flavor) proves none of them is still needed: a missing entry fails resolution.
- **No notice changes.** `THIRD-PARTY-NOTICES.md` lists only bundled fonts and data; the Apache-2.0 AndroidX libraries were never listed, so nothing is removed there. No spec text changes: the redesign spec already describes the end state.
- **APK size is measured, not promised.** R8 already strips unused Material classes from the release build, so the change may be small. Task 1 records the unsigned `githubRelease` APK's size, its `classes*.dex` total and its `resources.arsc` before the removal; Task 4 measures again and records both in DESIGN.md. The Rust libraries dominate the APK and are identical in both builds (same `jniLibs`), so the dex total is the number that shows Material leaving.
- **The final review runs on Robolectric screenshots.** No emulator: `/impeccable` judges the catalogue, shell and screen PNGs (the existing gated `-PscreensDir` mechanism) in both themes; the whole-app TalkBack pass and the device checks (shadows, motion, FLAG_SECURE) go to the owner's checklist, "Redesign stage 5 (no Material)".

## Review Focus

1. **A tap on something whose indication was left to the theme.** Without `MaterialTheme` it would grey (foundation's debug indication) or, before this stage, ripple; it must press the HavenKeys way. Pinned in Task 1 (`HavenThemeTest.theDefaultIndicationIsTheKitPressNotARipple`).
2. **A screen that looked right only because MaterialTheme set a default** (indication, text selection colours, content colour). The before and after screenshots of every catalogue section, shell state and rebuilt screen, in both themes, must match. Pinned in Task 3, Step 2.
3. **A pruned verification entry that some variant still resolves** (play flavor, instrumented tests, lint's classpaths). The proof build covers `lintGithubDebug`, `assembleGithubDebugAndroidTest` and `assemblePlayRelease` besides the usual tasks. Pinned in Task 1, Step 9.
4. **Material coming back through a test source or a fully qualified name** (`src/androidTest` is outside detekt, and a qualified `androidx.compose.material3.Text(…)` has no import). Pinned in Task 2, Step 3 (probes in `androidTest` and as a qualified call).
5. **The release build after R8 without Material** (a keep rule or reflective use that leaned on a Material class). `assembleGithubRelease` runs in Tasks 1 and 4, and the owner's checklist keeps the release-APK smoke test.

---

## File Structure

Paths abbreviate `apps/android/app/src/main/kotlin/net/havenkeys/android` as `ANDROID/` and `apps/android/app/src/test/kotlin/net/havenkeys/android` as `ANDROID_TEST/`. Gradle commands run in `apps/android`.

| File | Responsibility |
|---|---|
| `ANDROID/ui/theme/MaterialBridge.kt` | Deleted (bridge, Material colour schemes, `MaterialTypography`, `HavenType`, `MaterialShapes`) |
| `ANDROID/ui/theme/Theme.kt` | `HavenTheme` without the bridge; `HavenPress` as the default indication |
| `ANDROID_TEST/ui/theme/HavenThemeTest.kt` | The default indication is the kit's press |
| `apps/android/gradle/libs.versions.toml`, `apps/android/app/build.gradle.kts` | `compose-material3` and `compose-icons` gone; `forbidMaterial` replaces `forbidMaterialInKit` |
| `apps/android/gradle/verification-metadata.xml` | Six stale `material-icons-*` components deleted |
| `apps/android/app/detekt.yml` | `ForbiddenImport` forbids the Material packages |
| `apps/android/DESIGN.md` | Foundation Only Rule, the default indication, "Review notes (stage 5)" with APK sizes |
| `docs/android.md` | The detekt row of "Building"; "### Redesign stage 5 (no Material)" checklist |
| `docs/superpowers/plans/2026-10-03-android-redesign-index.md` | Stage 5 row `Done` |
| `apps/android/.impeccable/review/stage5-*` | Before and after screenshots, APK numbers (git-ignored, never committed) |

---

### Task 1: HavenTheme without Material, and the dependencies gone

**Files:**
- Delete: `ANDROID/ui/theme/MaterialBridge.kt`
- Modify: `ANDROID/ui/theme/Theme.kt`
- Modify: `ANDROID_TEST/ui/theme/HavenThemeTest.kt`
- Modify: `apps/android/app/build.gradle.kts` (two `implementation` lines), `apps/android/gradle/libs.versions.toml` (two catalog lines)
- Modify: `apps/android/gradle/verification-metadata.xml` (six components deleted)

**Interfaces:**
- Consumes: `net.havenkeys.android.ui.kit.HavenPress` (public `object HavenPress : IndicationNodeFactory`), `LocalHavenColors`, `LocalHavenMotion`, `DarkHavenColors`, `LightHavenColors`, `rememberHavenMotion()` (all in `ui/theme`, unchanged).
- Produces: `HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit)` with the same signature, now providing `LocalIndication provides HavenPress`. Baseline files `apps/android/.impeccable/review/stage5-before/*.png` and `apps/android/.impeccable/review/stage5-apk-before.txt` for Tasks 3 and 4.

- [ ] **Step 1: Measure the release APK before**

```bash
cd apps/android
env | grep HAVENKEYS_ ; ./gradlew assembleGithubRelease
apk=app/build/outputs/apk/github/release/app-github-release-unsigned.apk
mkdir -p .impeccable/review
{
  echo "commit $(git rev-parse --short HEAD)"
  echo "apk $(stat -c %s "$apk")"
  echo "dex $(unzip -l "$apk" 'classes*.dex' | tail -1 | awk '{print $1}')"
  echo "arsc $(unzip -l "$apk" resources.arsc | tail -1 | awk '{print $1}')"
} > .impeccable/review/stage5-apk-before.txt
cat .impeccable/review/stage5-apk-before.txt
```

Expected: the `env` line prints nothing, the build succeeds, and the file holds four lines (`commit`, `apk`, `dex`, `arsc` with byte counts). `.impeccable/review/` is git-ignored (`apps/android/.gitignore`), so this file never shows in `git status`.

- [ ] **Step 2: Render the screenshots before**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*Screenshots*' -PscreensDir=.impeccable/review/stage5-before`
Expected: PASS; `ls .impeccable/review/stage5-before/*.png | wc -l` prints the same count as the three screenshot classes write (catalogue `kit-*`, shell `shell-*`, screens `screens-*`; note the number for Task 3).

- [ ] **Step 3: Write the failing test**

Add to `ANDROID_TEST/ui/theme/HavenThemeTest.kt` (imports `androidx.compose.foundation.Indication`, `androidx.compose.foundation.LocalIndication`, `net.havenkeys.android.ui.kit.HavenPress`, `org.junit.Assert.assertSame`):

```kotlin
    @Test
    fun theDefaultIndicationIsTheKitPressNotARipple() {
        var indication: Indication? = null
        rule.setContent {
            HavenTheme(darkTheme = false) { indication = LocalIndication.current }
        }
        rule.waitForIdle()
        // A clickable that names no indication presses like the kit: no ripple, no grey highlight.
        assertSame(HavenPress, indication)
    }
```

- [ ] **Step 4: Run it to see it fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HavenThemeTest*'`
Expected: `theDefaultIndicationIsTheKitPressNotARipple` FAILS with `expected same:<net.havenkeys.android.ui.kit.HavenPress@…> was not:<…Ripple…>` (MaterialTheme's ripple); `theFlagPicksTheColours` passes.

- [ ] **Step 5: HavenTheme without the bridge**

```bash
git rm apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme/MaterialBridge.kt
```

Replace `ANDROID/ui/theme/Theme.kt` with:

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import net.havenkeys.android.ui.kit.HavenPress

internal val LocalHavenColors = staticCompositionLocalOf { DarkHavenColors }
internal val LocalHavenMotion = staticCompositionLocalOf { havenMotion(1f) }

/**
 * Light and dark follow the system, like the desktop's "match system".
 * The kit's press is the default indication, so a clickable that names none
 * still presses the HavenKeys way: no ripple (spec 2026-10-03 §5).
 */
@Composable
fun HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalHavenColors provides if (darkTheme) DarkHavenColors else LightHavenColors,
        LocalHavenMotion provides rememberHavenMotion(),
        LocalIndication provides HavenPress,
        content = content,
    )
}

object HavenTheme {
    val colors: HavenColors
        @Composable @ReadOnlyComposable
        get() = LocalHavenColors.current

    val type: HavenTypography
        get() = HavenTypography

    val motion: HavenMotion
        @Composable @ReadOnlyComposable
        get() = LocalHavenMotion.current
}
```

(If the shipped `Theme.kt` differs from the one this plan was written against beyond the `MaterialBridge(darkTheme, content)` call and its comment, keep the shipped code and change only those two lines plus the `LocalIndication` entry and its import.)

- [ ] **Step 6: Run the test to see it pass**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HavenThemeTest*'`
Expected: both tests PASS.

- [ ] **Step 7: Drop the dependencies**

In `apps/android/app/build.gradle.kts`, delete these two lines from `dependencies { }`:

```kotlin
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
```

In `apps/android/gradle/libs.versions.toml`, delete these two lines from `[libraries]`:

```toml
compose-material3 = { module = "androidx.compose.material3:material3" }
compose-icons = { module = "androidx.compose.material:material-icons-extended" }
```

Leave `forbidMaterialInKit` as it is for now (its exclusion names a file that no longer exists, which is harmless; Task 2 replaces the task).

Check what is left on the classpaths:

```bash
cd apps/android
for c in githubReleaseRuntimeClasspath playReleaseRuntimeClasspath githubDebugCompileClasspath githubDebugUnitTestCompileClasspath githubDebugAndroidTestCompileClasspath; do
  echo "$c $(./gradlew -q :app:dependencies --configuration $c | grep -c 'androidx\.compose\.material\|com\.google\.android\.material')"
done
./gradlew -q :app:dependencies --configuration githubDebugRuntimeClasspath | grep -o 'androidx\.compose\.material[^ ]*' | sort -u
```

Expected: each of the five configurations prints `0`. The debug runtime list is `androidx.compose.material3:material3:1.3.1`, `androidx.compose.material3:material3-android:1.4.0`, `androidx.compose.material:material-ripple:1.8.1`, `androidx.compose.material:material-ripple-android:1.11.4` (all under `ui-tooling-android`; exact version strings may show as `1.3.1 -> 1.4.0`), and no `material-icons`. If a release or compile classpath prints anything but `0`, find the parent with `./gradlew -q :app:dependencyInsight --configuration <it> --dependency material` and stop: something other than `ui-tooling` brings Material in, which this plan did not foresee.

- [ ] **Step 8: Prune the six stale verification entries**

```bash
cd apps/android
python3 - <<'EOF'
import pathlib, re
p = pathlib.Path("gradle/verification-metadata.xml")
s = p.read_text()
pattern = (
    r'      <component group="androidx\.compose\.material" '
    r'name="material-icons-(?:core|extended)(?:-android|-desktop)?" version="[^"]+">\n'
    r'.*?      </component>\n'
)
new, n = re.subn(pattern, "", s, flags=re.S)
assert n == 6, n
p.write_text(new)
EOF
git diff --stat gradle/verification-metadata.xml
grep -c 'material-icons' gradle/verification-metadata.xml
```

Expected: the script exits quietly (exactly six blocks removed), the diff touches only `verification-metadata.xml` with deletions only, and `grep -c` prints `0`. `git diff gradle/verification-metadata.xml | grep '^+' | grep -v '^+++'` prints nothing (no line added or edited).

- [ ] **Step 9: Run everything, with verification on**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease assembleGithubDebugAndroidTest assemblePlayRelease`
Expected: all pass. A `Dependency verification failed` naming a `material-icons-*` artifact means a pruned entry is still resolved by that task's configuration: restore just that block from `git show HEAD:apps/android/gradle/verification-metadata.xml` (never regenerate or add `<trust>`), note it for DESIGN.md's stage 5 notes, and run again.

- [ ] **Step 10: Commit**

```bash
git add -A apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme apps/android/app/src/test/kotlin/net/havenkeys/android/ui/theme/HavenThemeTest.kt apps/android/app/build.gradle.kts apps/android/gradle/libs.versions.toml apps/android/gradle/verification-metadata.xml
git commit -m "refactor(android): no Material: material3 and material-icons-extended dropped, the bridge deleted, the kit press as the default indication"
```

---

### Task 2: The build refuses Material anywhere

**Files:**
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` becomes `forbidMaterial`)
- Modify: `apps/android/app/detekt.yml` (`ForbiddenImport`)
- Modify: `docs/android.md` ("Building" table, the detekt row)

**Interfaces:**
- Consumes: Task 1 (no Material file left, so the check can have no exclusions).
- Produces: Gradle task `:app:forbidMaterial`, run by `detekt`; detekt `ForbiddenImport` entries for the three Material packages. Task 4 names both in DESIGN.md.

- [ ] **Step 1: Replace the check**

In `apps/android/app/build.gradle.kts`, replace everything from the comment `// Spec 2026-10-03 §5: the theme, the kit and the catalogue are built on` through the line `tasks.named("detekt") { dependsOn(forbidLogging, forbidMaterialInKit) }` with:

```kotlin
// Spec 2026-10-03 §5 and §8: everything is drawn from ui/kit on Compose
// foundation; the Material libraries were removed in stage 5. This scan keeps
// them out of every source set (androidTest too, which detekt does not read),
// catches qualified names that need no import, and fails on a Material module
// in the version catalog before anything uses it. detekt's ForbiddenImport
// (detekt.yml) says the same for imports.
val forbidMaterial by tasks.registering {
    description = "Fails on any Material: Compose Material, Material 3 or Material Components."
    val sources = fileTree("src") { include("**/*.kt") }
    val catalog = rootProject.file("gradle/libs.versions.toml")
    inputs.files(sources)
    inputs.file(catalog)
    doLast {
        val material = Regex("""\bandroidx\.compose\.material3?\b|\bcom\.google\.android\.material\b""")
        val hits = (sources.files + catalog).flatMap { file ->
            file.readLines().mapIndexedNotNull { i, line ->
                if (material.containsMatchIn(line)) "${file.path}:${i + 1}" else null
            }
        }
        if (hits.isNotEmpty()) {
            throw GradleException("Material is not used in HavenKeys; build from ui/kit (spec 2026-10-03 §5):\n" + hits.joinToString("\n"))
        }
    }
}
tasks.named("detekt") { dependsOn(forbidLogging, forbidMaterial) }
```

(`build.gradle.kts` is not scanned, and detekt's `source` is the four Kotlin source directories, so the long exception line needs no wrapping.)

- [ ] **Step 2: The detekt rule**

In `apps/android/app/detekt.yml`, replace the `ForbiddenImport` block with:

```yaml
  ForbiddenImport:
    active: true
    imports:
      - value: 'android.util.Log'
        reason: 'Never log in HavenKeys (CLAUDE.md §40).'
      - value: 'androidx.compose.material.*'
        reason: 'No Material in HavenKeys; build from ui/kit (spec 2026-10-03 §5).'
      - value: 'androidx.compose.material3.*'
        reason: 'No Material in HavenKeys; build from ui/kit (spec 2026-10-03 §5).'
      - value: 'com.google.android.material.*'
        reason: 'No Material in HavenKeys; build from ui/kit (spec 2026-10-03 §5).'
```

- [ ] **Step 3: Show both bite, then pass**

Each probe is a text change only (the compiler would also refuse it, which is why these runs skip compilation). Undo each probe before the next.

```bash
cd apps/android
f=app/src/main/kotlin/net/havenkeys/android/ui/theme/Theme.kt
sed -i 's/^import androidx.compose.foundation.LocalIndication$/&\nimport androidx.compose.material3.Text/' $f
./gradlew forbidMaterial            # FAILS naming Theme.kt:<line of the probe>
./gradlew detekt -x forbidMaterial  # FAILS: ForbiddenImport … androidx.compose.material3.Text … Theme.kt
git checkout -- $f

t=$(find app/src/androidTest -name '*.kt' | head -1)
echo "// androidx.compose.material.icons.Icons.Default" >> "$t"
./gradlew forbidMaterial            # FAILS naming that androidTest file (a qualified name, no import)
git checkout -- "$t"

sed -i 's/^compose-ui = .*/&\ncompose-material3 = { module = "androidx.compose.material3:material3" }/' gradle/libs.versions.toml
./gradlew forbidMaterial            # FAILS naming gradle/libs.versions.toml
git checkout -- gradle/libs.versions.toml

./gradlew forbidMaterial detekt     # passes
git status --short                  # only build.gradle.kts and detekt.yml modified
```

Expected: the five commented outcomes, in order. If `find` finds no file under `src/androidTest`, create a throwaway `app/src/androidTest/kotlin/Probe.kt` with that one comment line instead, and delete it after. If the detekt run passes instead of failing (its glob did not match the subpackage), replace the three Material `imports` entries with `forbiddenPatterns: 'androidx\.compose\.material3?\..*|com\.google\.android\.material\..*'` under `ForbiddenImport` and run the probe again.

- [ ] **Step 4: `docs/android.md`**

In the "Building" table, replace the row

```markdown
| Lint, including the no-logging scan | `./gradlew detekt` (runs `forbidLogging` first) |
```

with

```markdown
| Lint, including the no-logging and no-Material scans | `./gradlew detekt` (runs `forbidLogging` and `forbidMaterial` first) |
```

- [ ] **Step 5: Run everything**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass; the output lists `:app:forbidMaterial` before `:app:detekt`, and no task named `forbidMaterialInKit` exists (`./gradlew tasks --all | grep -c forbidMaterialInKit` prints `0`).

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/build.gradle.kts apps/android/app/detekt.yml docs/android.md
git commit -m "build(android): forbidMaterial covers every source and the version catalog; detekt forbids Material imports"
```

---

### Task 3: The pixels did not move, and the final `/impeccable` review

The controller runs this task (it needs `/impeccable`). Fixes that come out of the review are separate commits, each with a test where the fix has behaviour.

**Files:**
- Create: `apps/android/.impeccable/review/stage5-after/*.png` (git-ignored, never committed)
- Modify: whatever the review's accepted fixes touch; notes for Task 4 (kept in the controller's context, written into DESIGN.md in Task 4)

**Interfaces:**
- Consumes: `stage5-before/` from Task 1, Step 2.
- Produces: the review's verdict, fixed items with commit hashes, items kept, and open questions, for Task 4's "Review notes (stage 5)".

- [ ] **Step 1: Render after**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*Screenshots*' -PscreensDir=.impeccable/review/stage5-after`
Expected: PASS, and `ls .impeccable/review/stage5-after/*.png | wc -l` equals the count from Task 1, Step 2.

- [ ] **Step 2: Compare before and after**

```bash
cd apps/android/.impeccable/review
for f in stage5-before/*.png; do
  n=$(basename "$f")
  [ -f "stage5-after/$n" ] || { echo "missing $n"; continue; }
  cmp -s "$f" "stage5-after/$n" || echo "differs $n"
done
```

Expected: no output. For each `differs` line, open both PNGs (Read tool) and look: a difference only in content that follows the clock (a one-time code, its countdown ring) is explained, and is recorded as such; any other difference is something `MaterialTheme` used to provide. Find which default it was (`LocalIndication`, `LocalTextSelectionColors`, a content colour), provide it from the kit or `HavenTheme` with a test that pins it, re-render, and compare again until only clock-driven differences remain. Commit such a fix as `fix(android): <what looked different, in the screen's terms> without MaterialTheme`.

- [ ] **Step 3: `/impeccable` over the whole app**

Invoke `/impeccable` (skill `impeccable:impeccable`) in critique mode with this brief:

> Final review of the HavenKeys Android redesign, now with no Material underneath (stage 5). Artefacts: `apps/android/.impeccable/review/stage5-after/*.png`, light and dark: the kit catalogue (`kit-*`), the shell, Home, Items, a category list, Settings, search and the add sheet (`shell-*`, including 360dp and Portuguese), and every rebuilt screen (`screens-*`: item, editor, generator, unlock, onboarding, devices, autofill setup, Search HavenKeys, the fill confirmation, the passkey sheet). Sources: `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/` and `autofill/AutofillSearchScreen.kt`, `credentials/PasskeyCreateScreen.kt`. References: `apps/android/DESIGN.md` (its Named Rules and the stage 2, 3 and 4 review notes), `apps/desktop/DESIGN.md`, spec `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §1, §5, §6, §7. Judge the app as one product: the same type scale, row rhythm, hairlines and radii on every screen; the One Fitting Rule (brass only on controls and thin lines; one primary per screen) across all screens at once; that nothing anywhere reads as Material (no ripple, no Material sheet, type scale, icon or text-field box); that the shell and the full-screen screens read as one app; accessibility you can judge from images and code (contrast in both themes, 48dp targets, text that wraps and is never cut, each control named). Also judge these open questions from stage 4, without settling them: <paste the open questions from DESIGN.md "Review notes (stage 4)" and the index's stage 5 bullets>. Return an ordered list of material fixes.

Apply the fixes the owner would agree with without asking (spacing, sizes, ink, copy). Anything that changes a recorded decision, or settles an open question, goes to Task 4's notes as an open question. Re-render to `stage5-after` after fixes. Commit each fix:

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android apps/android/app/src/test apps/android/app/src/main/res
git commit -m "fix(android): <what the review changed, in the screen's terms>"
```

- [ ] **Step 4: Run everything**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug`
Expected: all pass. (If the review produced no fix, this task has no commit.)

---

### Task 4: Documentation, the owner's checks, closing the stage

**Files:**
- Modify: `apps/android/DESIGN.md` (the Foundation Only Rule; "Text and plumbing"; "Review notes (stage 5)")
- Modify: `docs/android.md` ("### Redesign stage 5 (no Material)")
- Modify: `docs/superpowers/plans/2026-10-03-android-redesign-index.md` (stage 5 row)

**Interfaces:**
- Consumes: `stage5-apk-before.txt` (Task 1), the check names (Task 2), the review's results (Task 3).
- Produces: the closed redesign.

- [ ] **Step 1: Measure the release APK after**

```bash
cd apps/android
env | grep HAVENKEYS_ ; ./gradlew assembleGithubRelease
apk=app/build/outputs/apk/github/release/app-github-release-unsigned.apk
{
  echo "commit $(git rev-parse --short HEAD)"
  echo "apk $(stat -c %s "$apk")"
  echo "dex $(unzip -l "$apk" 'classes*.dex' | tail -1 | awk '{print $1}')"
  echo "arsc $(unzip -l "$apk" resources.arsc | tail -1 | awk '{print $1}')"
} > .impeccable/review/stage5-apk-after.txt
paste .impeccable/review/stage5-apk-before.txt .impeccable/review/stage5-apk-after.txt
```

Expected: the `env` line prints nothing; two columns of four lines. The `dex` number is expected to be smaller or equal; if any number grew, say so in the notes with the reason if found (for example a Task 3 fix), never round it away.

- [ ] **Step 2: `apps/android/DESIGN.md`**

Replace the Foundation Only Rule paragraph with:

```markdown
**The Foundation Only Rule.** Everything builds on `androidx.compose.foundation`; HavenKeys has no Material dependency (removed in stage 5). `forbidMaterial`, which `detekt` runs first, fails the build on any mention of `androidx.compose.material`, `androidx.compose.material3` or `com.google.android.material` in any Kotlin source (tests included) or in the version catalog, and detekt's `ForbiddenImport` refuses the same imports. The debug build still carries `material3` at runtime because `ui-tooling` depends on it; no source can compile against it.
```

In "Text and plumbing", change the HavenPress bullet to:

```markdown
- **HavenPress / havenClickable:** the kit's only press feedback and its clickable (role button by default, no ripple). `HavenTheme` provides `HavenPress` as the default indication, so a clickable that names none still presses this way.
```

Append, after "Review notes (stage 4)", a section "## Review notes (stage 5)" in the earlier notes' voice, with these parts filled from Tasks 1–3:
- **How the review ran.** The three screenshot tests (`CatalogueScreenshots`, `ShellScreenshots`, `ScreenScreenshots`) rendered before and after the removal into `apps/android/.impeccable/review/stage5-before` and `stage5-after` (git-ignored) with `./gradlew testGithubDebugUnitTest --tests '*Screenshots*' -PscreensDir=…`; the files compared byte for byte (state the count, and each difference with its explanation, or "all identical"). `/impeccable` critiqued the after set in a single context; its detector does not scan Kotlin.
- **What left.** `ui/theme/MaterialBridge.kt` (the bridge, both Material colour schemes, `MaterialTypography`, the unused legacy `HavenType`, `MaterialShapes`), `compose-material3` and `compose-icons` (`material-icons-extended`), and the six `material-icons-*` verification entries (state any restored in Task 1, Step 9).
- **Release APK** (unsigned `githubRelease`, same Rust libraries both times), as a table with before, after and the difference in bytes and KiB: total APK, `classes*.dex`, `resources.arsc`, with the two commits.
- **Verdict**, **Fixed in the review** (numbered, `[P1]`–`[P3]`, with commits and pinning tests), **Checked and kept**, **Decided** (the default indication; the debug `ui-tooling` runtime carrying `material3`; both guards), **Open for the owner** (everything Task 3 left open).

- [ ] **Step 3: `docs/android.md`**

Add after "### Redesign stage 4 (existing screens)":

```markdown
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
```

- [ ] **Step 4: The plan index**

In `docs/superpowers/plans/2026-10-03-android-redesign-index.md`, set the stage 5 row's status to `Done (\`<first commit>..<last commit>\`)` with this stage's real range (Task 1's commit to this task's).

- [ ] **Step 5: Final verification**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease assembleGithubDebugAndroidTest`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/android/DESIGN.md docs/android.md docs/superpowers/plans/2026-10-03-android-redesign-index.md
git commit -m "docs: Android without Material, the final review and device checks, stage 5 done"
```

- [ ] **Step 7: Hand the device checks to the owner**

Report to the owner: the "Redesign stage 5 (no Material)" checklist (nothing in it has run on a device), the APK numbers, and DESIGN.md's "Open for the owner" items from stages 2 to 5.

---

## Stage exit check

- [ ] `grep -rln "androidx.compose.material\|com.google.android.material" apps/android/app/src apps/android/gradle/libs.versions.toml` prints nothing.
- [ ] `ls apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme/MaterialBridge.kt` fails; `grep -rn "HavenType\b" apps/android/app/src` prints nothing.
- [ ] `grep -n "forbidMaterial\b" apps/android/app/build.gradle.kts` shows the task and `dependsOn(forbidLogging, forbidMaterial)`; the task has no `exclude`.
- [ ] `grep -n "androidx.compose.material" apps/android/app/detekt.yml` shows the two `ForbiddenImport` entries.
- [ ] `grep -c "material-icons" apps/android/gradle/verification-metadata.xml` prints `0` (or only the blocks Task 1, Step 9 had to restore, each named in DESIGN.md), and `git log -p -- apps/android/gradle/verification-metadata.xml` for this stage shows deletions only.
- [ ] `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease` passes.
- [ ] DESIGN.md has "Review notes (stage 5)" with the APK table; the index shows stage 5 `Done`; the owner has the device checklist (Task 4, Step 7).
