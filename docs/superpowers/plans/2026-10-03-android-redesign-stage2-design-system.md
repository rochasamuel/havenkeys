# Android Redesign, Stage 2: Design System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Android app its own design system: a Material-free theme (the desktop's colours, bundled fonts, radii, springs), the desktop icon set as `HavenIcon`, and every `ui/kit` component of spec §5.2 with previews and semantics tests, shown together in a debug-only catalogue that `/impeccable` reviews against the desktop. No screen uses the kit yet.

**Architecture:** `ui/theme` becomes foundation-only (`HavenColors`, `HavenTypography`, `HavenRadius`/`HavenShape`/`HavenSpacing`, `HavenMotion` with named springs); the Material pieces existing screens still need move, unchanged, into one file (`ui/theme/MaterialBridge.kt`) that stage 5 deletes. `ui/kit` holds one file per component, built on `androidx.compose.foundation` with `HavenPress` as the only press feedback. Component tests run on the JVM under Robolectric inside `testGithubDebugUnitTest`. A debug-only activity shows the whole kit in light and dark.

**Tech Stack:** Kotlin 2.4, Jetpack Compose BOM 2026.06.01 (ui/foundation/animation 1.11.4), Robolectric 4.17, compose `ui-test-junit4`, fontTools 4.66.1 (run once, through `uvx`, to build the font files), detekt 1.23.

**Spec:** `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §5 (design system), §7 (motion values), §8 stage 2, §9 (security), §10 (testing), §11 (`apps/android/DESIGN.md`). Desktop source of truth: `apps/desktop/DESIGN.md`, `packages/ui/src/tokens.ts`, the per-theme layer at the top of `apps/desktop/src/styles.css`, `apps/desktop/src/components/Icon.tsx`.

## Global Constraints

- Foundation only: no `import androidx.compose.material…` in `ui/kit`, in `ui/theme` (except `ui/theme/MaterialBridge.kt`) or in the catalogue. Task 3 adds the `forbidMaterialInKit` Gradle check that enforces it.
- No screen uses the kit in this stage. `material3` and `material-icons-extended` stay in the build until stage 5. Existing screens keep their look; the only visible change to them is light-theme `brassInk` darkening to `#7d632f` (Task 3).
- Do not edit files stage 1 owns while stage 1 is in flight (`ItemScreen.kt`, `ItemViewModel.kt`, `ItemRow.kt` in `ui/components`, `VaultRepository.kt`, `AutofillRepository.kt`, the UniFFI bindings and their tests). This plan touches exactly one existing screen file: one import line in `ui/edit/EditScreen.kt` (Task 5).
- Each component lives in its own file under `ANDROID/ui/kit/`, has a `@PreviewLightDark` preview, and has Robolectric tests for its TalkBack role, state and label. Every interactive element is at least 48 x 48 dp.
- No ripple anywhere: clickable kit elements pass `indication = HavenPress` (scale to 0.97 with a brass-soft wash); switches and segments pass `indication = null`.
- Brass marks controls and thin lines, never large surfaces. One primary button per screen.
- Every animation takes its spec from `HavenMotion` (`springSpec`, `fadeSpec`, `tickSpec`…), so "Remove animations" (animator scale 0) turns all of them into instant cuts.
- The kit's own windows (sheet, dialog, menu) set `SecureFlagPolicy.SecureOn` and call `SecureDialogWindow(ignoreObscuredTouches = true)`.
- Text fields ask the keyboard not to learn (`NoPersonalizedLearning`). A secret field uses `TextObfuscationMode.Hidden` until the caller reveals it. Labels, content descriptions and toasts name a field, never its value.
- Our own words wrap and are never cut with an ellipsis; only user data (titles, usernames, URLs) may ellipsize (`docs/development.md` → Adding a translated string, rule 4).
- Every new string has an English and a Brazilian Portuguese version (`values/strings.xml`, `values-pt-rBR/strings.xml`; `lintGithubDebug` fails on a missing translation).
- No logging: the existing `forbidLogging` check covers new files.
- Fonts: exactly the four files `scripts/build-android-fonts.py` writes, under `res/font`, nothing downloaded at run time, OFL notices in `THIRD-PARTY-NOTICES.md`.
- Unit tests: `cd apps/android && ./gradlew testGithubDebugUnitTest`. Robolectric runs SDK 34 (its 35+ images need a Java 21 runtime; the build's toolchain is 17).
- detekt runs on all of it. `MaxLineLength` is 120: where a line of this plan's code is longer, wrap its arguments one per line and change nothing else. `MatchingDeclarationName`: a file whose first declaration is a class or interface not named like the file (`ButtonStyle` in `HavenButton.kt`, `PillTone` in `Pill.kt`, `SectionAction` in `SectionHeader.kt`, `InsetGroupScope` in `InsetGroup.kt`, `RowLeading` in `ItemRow.kt`, `DialogAction` in `HavenDialog.kt`, `MenuItem` in `HavenMenu.kt`) fails, so in those files put the file's main composable first and the helper types after it.
- Commits without Co-Authored-By lines (owner's rule).

## Entry checks (done 2026-10-03, while writing this plan)

1. **Licences.** All three families are SIL OFL 1.1 (google/fonts `ofl/*/OFL.txt`). Hanken Grotesk and JetBrains Mono declare no Reserved Font Name. Source Serif 4's binaries (`name` ID 0: "© 2014 - 2021 Adobe Systems Incorporated …, with Reserved Font Name 'Source'") do. A subset is a Modified Version under the OFL, so the Android serif files are renamed "HavenKeys Serif" (OFL condition 3), with the copyright and trademark strings kept. The script checks that no other name record still contains "Source".
2. **Where from.** google/fonts at commit `08dc85da6bca7ae308a6f1d38d0b137465646071` (2026-04-24, newest commit touching any of the three directories), raw URLs below, SHA-256 pinned in the script:
   - `ofl/hankengrotesk/HankenGrotesk[wght].ttf`: 132,892 bytes, `813b3f8f…662f`
   - `ofl/sourceserif4/SourceSerif4[opsz,wght].ttf`: 1,209,508 bytes, `97b2d4da…0a0b`
   - `ofl/sourceserif4/SourceSerif4-Italic[opsz,wght].ttf`: 855,432 bytes, `15fbc7e4…1f42`
   - `ofl/jetbrainsmono/JetBrainsMono[wght].ttf`: 187,208 bytes, `48715a42…ffeda`
3. **Weights the desktop uses** (counted in `apps/desktop/src/styles.css` and `packages/ui` tokens): Hanken Grotesk 400, 500, 550, 560, 600, 640, 650, 660; Source Serif 4 roman 400, 460, 470, 480, 500, 520 and italic 400 (the unlock headline only); JetBrains Mono 500.
4. **Variable or static.** Variable, cut down. Compose's `Font(resId, weight, style, variationSettings = FontVariation.Settings(FontVariation.weight(n)))` is stable on this BOM and Android applies variation settings from API 26; the app's minSdk is 28. Static files would need 8 Hanken + 6 serif files (about 60-120 KB each, over 1 MB). The script subsets to Latin (Basic, Latin-1, Extended-A/B, punctuation, €, arrows, bullet), limits Hanken to `wght` 400-660, pins the serif's optical size at 24 with `wght` 400-520, and makes the italic and the mono static instances. Result: `hanken_grotesk.ttf` 68,016 B, `havenkeys_serif.ttf` 230,896 B, `havenkeys_serif_italic.ttf` 87,080 B, `jetbrains_mono.ttf` 63,028 B: **449,020 bytes** in all, rebuilt byte-identical on a second run. Characters outside the subset (CJK, emoji) fall back to the system fonts, as Android does for any typeface.
5. **Compose BOM 2026.06.01** resolves `ui`, `foundation`, `animation` and `ui-test-junit4` to 1.11.4 (material3 1.4.0). Checked in the 1.11.4 jars:
   - Sheets: `androidx.compose.foundation.gestures.AnchoredDraggableState`, `DraggableAnchors { … at … }`, `Modifier.anchoredDraggable(state, orientation, …, flingBehavior)`, `AnchoredDraggableDefaults.flingBehavior(state, positionalThreshold, animationSpec)`, `animateTo`, `updateAnchors` — none marked experimental.
   - Text fields: `BasicTextField(state: TextFieldState, …, decorator: TextFieldDecorator)` and `BasicSecureTextField(…, textObfuscationMode = TextObfuscationMode.Hidden|Visible|RevealLastTyped)` — stable; `BasicSecureTextField` sets `password()` semantics itself.
   - Windows: `Dialog` with `DialogProperties(usePlatformDefaultWidth, decorFitsSystemWindows, securePolicy)`, `Popup` with `PopupProperties(focusable, securePolicy)`.
   - Pull to refresh: **foundation has none** (it lives in material/material3). `PullToRefresh` is built on `Modifier.nestedScroll` (Task 13).
   - Press: `IndicationNodeFactory` (foundation) for a custom indication; `HapticFeedbackType.Confirm`, `SegmentTick`, `ToggleOn` exist.
   - Previews: `androidx.compose.ui.tooling.preview.PreviewLightDark` exists.
6. **Component tests.** The repo has no Compose UI test on the JVM: `ui-test-junit4` is only an `androidTestImplementation`, there is no Robolectric, and the unit tests are plain JUnit. Smallest addition that runs under `testGithubDebugUnitTest`: `org.robolectric:robolectric:4.17` (2026-09-10) plus `ui-test-junit4` from the BOM as `testImplementation`, `unitTests.isIncludeAndroidResources = true`, and `src/test/resources/robolectric.properties` (`sdk=34`; `application=android.app.Application` so `HavenApp` does not load the Rust library). `ui-test-manifest` is already a `debugImplementation`.

## Decisions recorded for the owner

- **Serif titles.** Spec §5.1 lists Source Serif 4 "(monogram tiles)". The desktop sets every title in it (DESIGN.md: "Do set titles in Source Serif 4 roman"), and §5 makes the desktop the source of truth, so `display`, `headline`, `title`, `titleSmall` and `monogram` are serif. `/impeccable` (Task 15) confirms or reverts this on the catalogue.
- **Tab icons.** `home` and `items` are drawn new; Settings uses the desktop's existing `gear`. Also added for the phone: `chevronRight` (disclosure), `chevronLeft` (back), `more` (menu trigger). All five go into the desktop's `Icon.tsx` too, and a test keeps the two sets identical.
- **Phone contrast departures** (WCAG AA 4.5:1 at 13-16sp, pinned by `ContrastTest`): light `muted` `#61706a` (desktop `#66756f` is 4.45:1 on a group), light `brassInk`/`digit` `#7d632f` (desktop `#8f7236` is 4.17:1 on a group, 4.0:1 on a pill), dark `onDanger` `#121a17` (white on `#e0775f` is 3.0:1). `/impeccable` decides whether the desktop should follow.
- **Names.** The spec's `HavenType` is taken by the legacy text styles screens still import; the new scale is `HavenTypography`, read as `HavenTheme.type`. The legacy `HavenType`, the Material colour schemes, typography and shapes move unchanged into `MaterialBridge.kt` (stage 5 deletes it).
- **Text field label.** Foundation has no label semantics, so a kit field's node carries its label as `contentDescription` and the visible label is hidden from TalkBack (read once, not twice). The TalkBack pass (Task 15) checks how it reads.
- **Catalogue screenshots.** The catalogue activity is debug-only and shows no vault data, so it does not set `FLAG_SECURE` (screenshots for review). The kit's own windows still do, so the catalogue also draws the sheet, dialog and menu surfaces inline.
- **`Pill`** is a non-interactive marker, as on the desktop.

## Review Focus

1. **Largest system font (200%)**: a button or item row grows to fit; text is not clipped and targets stay at least 48dp. Pinned in Task 7 (`theLargestFontGrowsTheButtonInsteadOfCuttingIt`) and Task 8 (`theLargestFontGrowsTheRow`).
2. **Titles that start with an emoji, an accented or non-Latin letter, or are blank**: the monogram is the first character as a reader sees it (whole surrogate pair, `ß` not `SS`), and a blank title shows the key glyph. Pinned in Task 8 (`monogramsTakeTheFirstCharacterAsTheReaderSeesIt`).
3. **A secret typed into a kit field**: the node is a password node named by its label; the value is in no content description, and the reveal button names the field, not the value. Pinned in Task 11 (`aSecretIsAPasswordFieldNamedByItsLabel`, `theEyeShowsAndHidesAndSaysWhich`).
4. **"Remove animations" on, and a tap while something is still animating**: springs and fades cut; a sheet closes within a few frames; a close tapped while the sheet is still rising closes it, once. Pinned in Task 4 (`springsCutUnderRemoveAnimations`) and Task 12 (`underRemoveAnimationsTheSheetClosesAtOnce`, `aTapWhileTheSheetIsStillOpeningClosesItOnce`).
5. **Light theme contrast**: the desktop's muted and brass-ink text fall under 4.5:1 on phone surfaces. Pinned in Task 3 (`ContrastTest`). A double tap on a dialog's confirm button answers once: Task 12 (`aDialogAnswersOnce`).

---

## File Structure

Paths abbreviate `apps/android/app/src/main/kotlin/net/havenkeys/android` as `ANDROID/` and `apps/android/app/src/test/kotlin/net/havenkeys/android` as `ANDROID_TEST/`. Gradle commands run in `apps/android`.

| File | Responsibility |
|---|---|
| `apps/android/gradle/libs.versions.toml`, `apps/android/app/build.gradle.kts` | Robolectric and ui-test for unit tests; `forbidMaterialInKit`; detekt sources |
| `apps/android/app/detekt.yml` | MagicNumber off for `ui/kit` and the catalogue (geometry is design tokens); `PreviewLightDark` previews count as used |
| `apps/android/app/src/test/resources/robolectric.properties` (new) | SDK 34, plain `Application` |
| `ANDROID_TEST/ui/kit/KitTestSupport.kt` (new) | `setKit`, `hasRole`, `assertTouchTarget`, `removeAnimations` |
| `scripts/build-android-fonts.py` (new) | Downloads, checks, subsets, instances and renames the fonts |
| `apps/android/app/src/main/res/font/*.ttf` (new) | The four bundled font files |
| `ANDROID/ui/theme/Fonts.kt` (new) | `HankenGrotesk`, `HavenSerif`, `JetBrainsMono` families |
| `ANDROID/ui/theme/Color.kt` (rewritten) | `HavenColors` with every desktop role, light and dark |
| `ANDROID/ui/theme/MaterialBridge.kt` (new) | The Material scheme, typography, shapes and legacy `HavenType` that existing screens use; deleted in stage 5 |
| `ANDROID/ui/theme/Theme.kt` (rewritten) | `HavenTheme` without Material types; `HavenTheme.colors/type/motion` |
| `ANDROID/ui/theme/Type.kt` (rewritten) | `HavenTypography`, the phone's type scale |
| `ANDROID/ui/theme/Dimens.kt` (new) | `HavenRadius`, `HavenShape`, `HavenSpacing` |
| `ANDROID/ui/theme/Motion.kt` | `HouseEasing`, `HavenSpring`, `HavenSprings`, `springSpec`, `fadeSpec` |
| `ANDROID/ui/kit/HavenText.kt`, `HavenPress.kt`, `KitPreview.kt`, `NoPersonalizedLearning.kt` (moved from `ui/edit`) | Kit primitives |
| `ANDROID/ui/kit/HavenIcon.kt` + `apps/desktop/src/components/Icon.tsx` | The icon set, one library |
| `ANDROID/ui/kit/HavenButton.kt`, `HavenIconButton.kt`, `CopyButton.kt`, `Pill.kt`, `AddButton.kt` | Actions |
| `ANDROID/ui/kit/SectionHeader.kt`, `InsetGroup.kt`, `GroupRow.kt`, `ItemRow.kt` | Structure |
| `ANDROID/ui/kit/HavenScaffold.kt`, `Toast.kt` | Screen frame and the glass toast |
| `ANDROID/ui/kit/HavenSwitch.kt` (with `ToggleRow`), `HavenSlider.kt`, `SegmentedControl.kt` | Inputs I |
| `ANDROID/ui/kit/HavenTextField.kt`, `SecretTextField.kt` | Inputs II |
| `ANDROID/ui/kit/HavenSheet.kt`, `HavenDialog.kt` | Overlays I |
| `ANDROID/ui/kit/HavenMenu.kt`, `ProgressRing.kt`, `PullToRefresh.kt` | Overlay II, feedback |
| `apps/android/app/src/debug/…/catalogue/*.kt`, `src/debug/AndroidManifest.xml`, `src/debug/res/values/strings.xml` | Debug-only catalogue |
| `apps/android/app/src/testDebug/…/catalogue/CatalogueTest.kt` | Catalogue coverage, rendering, manifest |
| `res/values/strings.xml`, `res/values-pt-rBR/strings.xml` | `kit_*` strings |
| `THIRD-PARTY-NOTICES.md`, `docs/development.md` | Font notices; how to rebuild fonts |
| `apps/android/DESIGN.md` (new), `apps/android/.impeccable/review/*.png` | Design system as shipped; review screenshots |

---
### Task 1: Compose component tests on the JVM

**Files:**
- Modify: `apps/android/gradle/libs.versions.toml`
- Modify: `apps/android/app/build.gradle.kts` (`android { }` block and `dependencies { }`)
- Modify: `apps/android/app/detekt.yml`
- Create: `apps/android/app/src/test/resources/robolectric.properties`
- Create: `ANDROID_TEST/ui/kit/KitTestSupport.kt`
- Test: `ANDROID_TEST/ui/kit/ComposeOnJvmTest.kt`

**Interfaces:**
- Produces (test code, package `net.havenkeys.android.ui.kit`):
  - `fun ComposeContentTestRule.setKit(dark: Boolean = false, fontScale: Float = 1f, content: @Composable () -> Unit)`
  - `fun hasRole(role: Role): SemanticsMatcher`
  - `fun SemanticsNodeInteraction.assertTouchTarget(): SemanticsNodeInteraction` (at least 48 x 48 dp)
  - `fun removeAnimations(on: Boolean = true)` (sets the animator scale `HavenTheme` reads)

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/ComposeOnJvmTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The harness the kit's tests stand on: Compose, semantics and the theme on the JVM. */
@RunWith(RobolectricTestRunner::class)
class ComposeOnJvmTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aThemedComposableRendersWithItsSemantics() {
        rule.setKit {
            BasicText("HavenKeys", Modifier.size(48.dp).semantics { role = Role.Button })
        }
        rule.onNodeWithText("HavenKeys").assertIsDisplayed().assert(hasRole(Role.Button)).assertTouchTarget()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ComposeOnJvmTest*'`
Expected: compile errors (`createComposeRule`, `RobolectricTestRunner`, `setKit` unresolved).

- [ ] **Step 3: Add the dependencies and configuration**

`gradle/libs.versions.toml`, under `[versions]` after `androidxJunit`:

```toml
robolectric = "4.17"
```

and under `[libraries]` after `androidx-test-junit`:

```toml
robolectric = { module = "org.robolectric:robolectric", version.ref = "robolectric" }
```

`app/build.gradle.kts`, inside `android { }` after `packaging { … }`:

```kotlin
    testOptions {
        // The ui/kit component tests run on the JVM under Robolectric and
        // need the merged resources (strings, fonts).
        unitTests { isIncludeAndroidResources = true }
    }
```

and in `dependencies { }` after `testImplementation(libs.coroutines.test)`:

```kotlin
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test)
    testImplementation(libs.robolectric)
```

`app/src/test/resources/robolectric.properties`:

```properties
# Robolectric's SDK 35+ images need a Java 21 runtime; the build's toolchain is 17.
sdk=34
# HavenApp loads the Rust library and the platform TLS verifier; the kit needs neither.
application=android.app.Application
```

`app/detekt.yml`: replace the `MagicNumber` and `UnusedPrivateMember` entries with

```yaml
  MagicNumber:
    # The theme is token values from packages/ui/src/tokens.css by design, and
    # the kit's geometry is design tokens too (apps/android/DESIGN.md).
    excludes: ['**/test/**', '**/testDebug/**', '**/androidTest/**', '**/ui/theme/**', '**/ui/kit/**', '**/catalogue/**']
  UnusedPrivateMember:
    ignoreAnnotated: ['Preview', 'PreviewLightDark']
```

- [ ] **Step 4: Write the test support**

`ANDROID_TEST/ui/kit/KitTestSupport.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenTheme
import org.robolectric.RuntimeEnvironment

/** Sets [content] inside [HavenTheme], optionally at a larger system font size. */
fun ComposeContentTestRule.setKit(dark: Boolean = false, fontScale: Float = 1f, content: @Composable () -> Unit) {
    setContent {
        val density = LocalDensity.current
        CompositionLocalProvider(LocalDensity provides Density(density.density, fontScale)) {
            HavenTheme(darkTheme = dark, content = content)
        }
    }
}

fun hasRole(role: Role): SemanticsMatcher = SemanticsMatcher.expectValue(SemanticsProperties.Role, role)

/** Spec §5.2: every touch target is at least 48dp each way. */
fun SemanticsNodeInteraction.assertTouchTarget(): SemanticsNodeInteraction =
    assertHeightIsAtLeast(48.dp).assertWidthIsAtLeast(48.dp)

/** The system's "Remove animations": the animator scale HavenTheme follows. Call before setKit. */
fun removeAnimations(on: Boolean = true) {
    Settings.Global.putFloat(
        RuntimeEnvironment.getApplication().contentResolver,
        Settings.Global.ANIMATOR_DURATION_SCALE,
        if (on) 0f else 1f,
    )
}
```

- [ ] **Step 5: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ComposeOnJvmTest*'`
Expected: PASS. (The first run downloads Robolectric's SDK 34 image from Maven Central.)

- [ ] **Step 6: Run the whole unit suite and detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt`
Expected: all pass; the existing plain JUnit tests are unaffected.

- [ ] **Step 7: Commit**

```bash
git add apps/android/gradle/libs.versions.toml apps/android/app/build.gradle.kts apps/android/app/detekt.yml apps/android/app/src/test/resources/robolectric.properties apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit
git commit -m "test(android): Compose component tests on the JVM with Robolectric"
```

---

### Task 2: Bundled fonts

**Files:**
- Create: `scripts/build-android-fonts.py`
- Create: `apps/android/app/src/main/res/font/hanken_grotesk.ttf`, `havenkeys_serif.ttf`, `havenkeys_serif_italic.ttf`, `jetbrains_mono.ttf` (written by the script)
- Create: `ANDROID/ui/theme/Fonts.kt`
- Modify: `THIRD-PARTY-NOTICES.md` (the intro paragraph and the three font entries)
- Modify: `docs/development.md` (Commands table, after the "refresh the privileged browser list" row)
- Test: `ANDROID_TEST/ui/theme/FontsTest.kt`

**Interfaces:**
- Produces: `val HankenGrotesk: FontFamily`, `val HavenSerif: FontFamily` (roman 400-520 and italic 400), `val JetBrainsMono: FontFamily`, `internal val HankenWeights: List<Int>`, `internal val SerifWeights: List<Int>` (package `net.havenkeys.android.ui.theme`).

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/theme/FontsTest.kt`:

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontListFontFamily
import androidx.compose.ui.text.font.FontStyle
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FontsTest {
    private fun FontFamily.weights(style: FontStyle = FontStyle.Normal): List<Int> =
        (this as FontListFontFamily).fonts.filter { it.style == style }.map { it.weight.weight }

    @Test
    fun exactlyTheFourBuiltFilesAreBundledAndTheyStaySmall() {
        val files = File("src/main/res/font").listFiles().orEmpty().associate { it.name to it.length() }
        assertEquals(
            setOf("hanken_grotesk.ttf", "havenkeys_serif.ttf", "havenkeys_serif_italic.ttf", "jetbrains_mono.ttf"),
            files.keys,
        )
        assertTrue("fonts add ${files.values.sum()} bytes", files.values.sum() < 500_000)
    }

    @Test
    fun theFamiliesDeclareTheWeightsTheDesktopUses() {
        assertEquals(listOf(400, 500, 550, 560, 600, 640, 650, 660), HankenGrotesk.weights())
        assertEquals(listOf(400, 460, 470, 480, 500, 520), HavenSerif.weights())
        assertEquals(listOf(400), HavenSerif.weights(FontStyle.Italic))
        assertEquals(listOf(500), JetBrainsMono.weights())
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*FontsTest*'`
Expected: compile error, `HankenGrotesk` unresolved.

- [ ] **Step 3: Write the font build script**

`scripts/build-android-fonts.py`:

```python
"""Builds the Android app's bundled fonts (apps/android/app/src/main/res/font).

Run from the repository root:
    uvx --from fonttools==4.66.1 python scripts/build-android-fonts.py

Downloads the three OFL families from google/fonts at a pinned commit,
checks their SHA-256, keeps the Latin glyphs HavenKeys needs, fixes the
axes the app does not use, and writes four TTFs. Source Serif 4 declares
the Reserved Font Name "Source", and a subset is a Modified Version under
the OFL, so the serif files are renamed "HavenKeys Serif" (OFL condition 3).
Nothing here runs at build time; the outputs are committed.
"""

import hashlib
import io
import os
import sys
import urllib.request

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

COMMIT = "08dc85da6bca7ae308a6f1d38d0b137465646071"
BASE = f"https://raw.githubusercontent.com/google/fonts/{COMMIT}/ofl"

SOURCES = {
    "hanken": (
        "hankengrotesk/HankenGrotesk%5Bwght%5D.ttf",
        "813b3f8fa0965405669a89b38e51bbefd95eef6b8e20d1cb2d8c10cce062662f",
    ),
    "serif": (
        "sourceserif4/SourceSerif4%5Bopsz%2Cwght%5D.ttf",
        "97b2d4da6e3cb494b5a1e66ae176914d852ccabef49e0c02c0df25f3e39aca0b",
    ),
    "serif_italic": (
        "sourceserif4/SourceSerif4-Italic%5Bopsz%2Cwght%5D.ttf",
        "15fbc7e4679489a501998c3669272637a6646388ef7e4bd77eebb5bf967a1f42",
    ),
    "mono": (
        "jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf",
        "48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda",
    ),
}

# Basic Latin, Latin-1, Latin Extended-A/B (names in European languages),
# general punctuation, the euro sign, arrows, minus, bullet and the dot the
# mask is drawn with. Anything else falls back to the system fonts.
UNICODES = (
    "U+0000-024F,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+2000-206F,U+2074,"
    "U+20AC,U+2122,U+2190-2193,U+2212,U+2215,U+2022,U+25CF,U+FEFF,U+FFFD"
)

# (output name, source, axis limits, rename to HavenKeys Serif)
OUTPUTS = [
    # Weights 400-660: body 400/500, controls 550, labels 560, titles 600-660.
    ("hanken_grotesk.ttf", "hanken", {"wght": (400, 660)}, False),
    # Roman 400-520 at optical size 24: monograms and titles.
    ("havenkeys_serif.ttf", "serif", {"opsz": 24, "wght": (400, 520)}, True),
    # The one italic (the unlock headline): 400 only.
    ("havenkeys_serif_italic.ttf", "serif_italic", {"opsz": 30, "wght": 400}, True),
    # Secrets and codes: 500 only.
    ("jetbrains_mono.ttf", "mono", {"wght": 500}, False),
]

SERIF_NAME = "HavenKeys Serif"


def fetch(path: str, sha256: str) -> bytes:
    with urllib.request.urlopen(f"{BASE}/{path}") as response:
        data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != sha256:
        sys.exit(f"{path}: SHA-256 {digest}, expected {sha256}")
    return data


def subset_font(font: TTFont) -> None:
    options = subset.Options()
    options.layout_features = ["*"]
    options.name_IDs = ["*"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=subset.parse_unicodes(UNICODES))
    subsetter.subset(font)


def rename(font: TTFont, italic: bool) -> None:
    style = "Italic" if italic else "Regular"
    names = {
        1: SERIF_NAME,
        2: style,
        3: f"{SERIF_NAME} {style};HavenKeys",
        4: f"{SERIF_NAME} {style}",
        6: SERIF_NAME.replace(" ", "") + "-" + style,
        16: SERIF_NAME,
        17: style,
    }
    table = font["name"]
    for name_id in (21, 22, 25):
        table.removeNames(nameID=name_id)
    for name_id, value in names.items():
        table.removeNames(nameID=name_id)
        table.setName(value, name_id, 3, 1, 0x409)
    if "fvar" in font:
        # Named instances carry PostScript names that start with the
        # original family; Android does not use them.
        font["fvar"].instances = []
    if "STAT" in font:
        del font["STAT"]
    # The copyright (0) and trademark (7) notices must stay as they are.
    left = [n.nameID for n in table.names if n.nameID not in (0, 7) and "Source" in n.toUnicode()]
    if left:
        sys.exit(f"name IDs still carrying the reserved name: {left}")


def main() -> None:
    out_dir = os.path.join("apps", "android", "app", "src", "main", "res", "font")
    os.makedirs(out_dir, exist_ok=True)
    raw = {key: fetch(path, sha) for key, (path, sha) in SOURCES.items()}
    for name, key, limits, renamed in OUTPUTS:
        font = TTFont(io.BytesIO(raw[key]))
        subset_font(font)
        font = instancer.instantiateVariableFont(font, limits)
        if renamed:
            rename(font, italic=key.endswith("italic"))
        path = os.path.join(out_dir, name)
        # Keep the source's head.modified, so a rerun gives the same bytes.
        font.recalcTimestamp = False
        font.save(path)
        print(f"{path}: {os.path.getsize(path)} bytes")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Build the fonts**

Run (repository root): `uvx --from fonttools==4.66.1 python scripts/build-android-fonts.py && sha256sum apps/android/app/src/main/res/font/*`
Expected:

```
apps/android/app/src/main/res/font/hanken_grotesk.ttf: 68016 bytes
apps/android/app/src/main/res/font/havenkeys_serif.ttf: 230896 bytes
apps/android/app/src/main/res/font/havenkeys_serif_italic.ttf: 87080 bytes
apps/android/app/src/main/res/font/jetbrains_mono.ttf: 63028 bytes
73058e0c19006929590921f27ddc03eb6ef882ff7b7ab76cc723a7f2176529e1  …/hanken_grotesk.ttf
dc1a3e69816602cc646d7be27bdc2430795b9db1776638d194640645ed6cff6c  …/havenkeys_serif.ttf
ed054c0ac1cdbebee531f9541e70bde06244e3d52ca17d27b8a832db6ec74da4  …/havenkeys_serif_italic.ttf
71205c0ec3ccf5e9ef6d7a3eb22f4343e11a04439087f75c3cb0ca35023c79e2  …/jetbrains_mono.ttf
```

If a hash differs but the sizes match within a few hundred bytes, the fontTools version differs: rerun with exactly `fonttools==4.66.1`.

- [ ] **Step 5: Declare the families**

`ANDROID/ui/theme/Fonts.kt`:

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import net.havenkeys.android.R

/*
 * The desktop's three faces, bundled (res/font, built by
 * scripts/build-android-fonts.py; SIL OFL 1.1, see THIRD-PARTY-NOTICES.md).
 * Nothing is downloaded. The Hanken and serif files are variable: each
 * weight the desktop uses is declared once with its axis value, so the file
 * draws exactly that weight (variation settings apply from API 26; minSdk
 * is 28). Glyphs outside the Latin subset fall back to the system fonts.
 */

/** Hanken Grotesk weights in the desktop: body 400/500, controls 550, labels 560, emphasis 600, headings 640-660. */
internal val HankenWeights = listOf(400, 500, 550, 560, 600, 640, 650, 660)

/** Source Serif 4 roman weights in the desktop: 400 and the title weights 460-520. */
internal val SerifWeights = listOf(400, 460, 470, 480, 500, 520)

private fun variable(resId: Int, weight: Int): Font = Font(
    resId = resId,
    weight = FontWeight(weight),
    style = FontStyle.Normal,
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

/** Everything that is not a title or a secret. */
val HankenGrotesk: FontFamily = FontFamily(HankenWeights.map { variable(R.font.hanken_grotesk, it) })

/** Titles and monograms (Source Serif 4, renamed for the OFL's Reserved Font Name). */
val HavenSerif: FontFamily = FontFamily(
    SerifWeights.map { variable(R.font.havenkeys_serif, it) } +
        Font(R.font.havenkeys_serif_italic, FontWeight(400), FontStyle.Italic),
)

/** Secrets, one-time codes, keys: strings read or typed exactly. */
val JetBrainsMono: FontFamily = FontFamily(Font(R.font.jetbrains_mono, FontWeight(500)))
```

- [ ] **Step 6: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*FontsTest*'`
Expected: PASS.

- [ ] **Step 7: Notices and docs**

In `THIRD-PARTY-NOTICES.md`:
- In the paragraph starting "The fonts and data below are different", replace "the desktop application bundle, the browser extension package and the website's built assets" with "the desktop application bundle, the browser extension package, the Android app and the website's built assets".
- After the line "Distributed via the `@fontsource-variable/hanken-grotesk` package." add: "The Android app bundles a Latin subset (`apps/android/app/src/main/res/font/hanken_grotesk.ttf`) built from google/fonts by `scripts/build-android-fonts.py`."
- After "Distributed via the `@fontsource/jetbrains-mono` package." add the same sentence with `jetbrains_mono.ttf`.
- Replace "Distributed via the `@fontsource-variable/source-serif-4` package. Used by the website, the desktop app and the browser extension." with:

```markdown
Distributed via the `@fontsource-variable/source-serif-4` package. Used by
the website, the desktop app, the browser extension and the Android app.
The Android app bundles a Latin subset built from google/fonts by
`scripts/build-android-fonts.py` (`havenkeys_serif.ttf`,
`havenkeys_serif_italic.ttf`). A subset is a Modified Version under the OFL,
and the font declares the Reserved Font Name "Source", so those files are
renamed "HavenKeys Serif"; their copyright and trademark notices are
unchanged.
```

In `docs/development.md`, Commands table, after the "refresh the privileged browser list" row:

```markdown
| Android: rebuild the bundled fonts | `uvx --from fonttools==4.66.1 python scripts/build-android-fonts.py`; commit `apps/android/app/src/main/res/font` |
```

- [ ] **Step 8: Build and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add scripts/build-android-fonts.py apps/android/app/src/main/res/font apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme/Fonts.kt apps/android/app/src/test/kotlin/net/havenkeys/android/ui/theme/FontsTest.kt THIRD-PARTY-NOTICES.md docs/development.md
git commit -m "feat(android): bundle Hanken Grotesk, Source Serif 4 and JetBrains Mono"
```

---

### Task 3: Colours and a Material-free theme

**Files:**
- Rewrite: `ANDROID/ui/theme/Color.kt`
- Create: `ANDROID/ui/theme/MaterialBridge.kt`
- Rewrite: `ANDROID/ui/theme/Theme.kt`
- Delete: `ANDROID/ui/theme/Type.kt` (its contents move to `MaterialBridge.kt`; Task 4 writes a new `Type.kt`)
- Modify: `apps/android/app/build.gradle.kts` (add `forbidMaterialInKit`)
- Test: `ANDROID_TEST/ui/theme/ContrastTest.kt`, `ANDROID_TEST/ui/theme/HavenThemeTest.kt`

**Interfaces:**
- Consumes: `rememberHavenMotion()`, `havenMotion(Float)` (existing `Motion.kt`).
- Produces:
  - `data class HavenColors(isDark, pane, list, group, groupLine, field, raised, hover, line, lineStrong, text, textStrong, muted, brass, brassHi, brassInk, brassSoft, sel, primary, onPrimary, onBrass, ok, danger, onDanger, glass, onGlass, digit, symbol, avatarBg, avatarFg, thumb, scrim)`; `val DarkHavenColors`, `val LightHavenColors`.
  - `internal val LocalHavenColors`, `internal val LocalHavenMotion`.
  - `@Composable fun HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit)`; `object HavenTheme { val colors: HavenColors; val motion: HavenMotion }`.
  - Unchanged for existing screens: `HavenTheme.colors.textStrong/brass/brassInk/brassSoft/danger/avatarBg/avatarFg`, `HavenType.secret/masked/code`.

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/ui/theme/ContrastTest.kt`:

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.luminance
import kotlin.math.max
import kotlin.math.min
import org.junit.Assert.assertTrue
import org.junit.Test

/** WCAG AA (4.5:1) for every text colour on every surface it is drawn on. */
class ContrastTest {
    private val themes = listOf("dark" to DarkHavenColors, "light" to LightHavenColors)

    private fun assertReads(what: String, ink: Color, ground: Color) {
        val a = ink.compositeOver(ground).luminance()
        val b = ground.luminance()
        val ratio = (max(a, b) + 0.05f) / (min(a, b) + 0.05f)
        assertTrue("$what: ${"%.2f".format(ratio)}:1, needs 4.5:1", ratio >= 4.5f)
    }

    @Test
    fun textReadsOnEverySurface() {
        for ((theme, c) in themes) {
            val surfaces = mapOf("pane" to c.pane, "list" to c.list, "group" to c.group, "raised" to c.raised)
            val inks = mapOf(
                "text" to c.text,
                "textStrong" to c.textStrong,
                "muted" to c.muted,
                "brassInk" to c.brassInk,
                "danger" to c.danger,
            )
            for ((surface, ground) in surfaces) {
                for ((name, ink) in inks) assertReads("$theme $name on $surface", ink, ground)
            }
        }
    }

    @Test
    fun textReadsOnFilledControls() {
        for ((theme, c) in themes) {
            assertReads("$theme onPrimary", c.onPrimary, c.primary)
            assertReads("$theme onBrass", c.onBrass, c.brass)
            assertReads("$theme onDanger", c.onDanger, c.danger)
            assertReads("$theme toast", c.onGlass, c.glass.compositeOver(c.pane))
            assertReads("$theme pill", c.brassInk, c.brassSoft.compositeOver(c.group))
            assertReads("$theme monogram", c.avatarFg, c.avatarBg)
        }
    }
}
```

`ANDROID_TEST/ui/theme/HavenThemeTest.kt`:

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.ui.test.junit4.createComposeRule
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class HavenThemeTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun theFlagPicksTheColours() {
        var dark: HavenColors? = null
        var light: HavenColors? = null
        rule.setContent {
            HavenTheme(darkTheme = true) { dark = HavenTheme.colors }
            HavenTheme(darkTheme = false) { light = HavenTheme.colors }
        }
        rule.waitForIdle()
        assertEquals(DarkHavenColors, dark)
        assertEquals(LightHavenColors, light)
        assertEquals(true, dark?.isDark)
        assertEquals(false, light?.isDark)
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ContrastTest*' --tests '*HavenThemeTest*'`
Expected: compile errors (`pane`, `isDark`, … not members of `HavenColors`).

- [ ] **Step 3: Move the Material pieces into the bridge**

Create `ANDROID/ui/theme/MaterialBridge.kt` and move into it, **unchanged**:
- from `Color.kt`: the whole comment table that starts `Every value comes from packages/ui/src/tokens.css` (keep it as the file's header comment), `private val DarkBg`, `private val LightBg`, `internal val DarkScheme`, `internal val LightScheme`;
- from `Type.kt`: everything after the imports (`Body`, `Label`, `Heading`, `Title`, the trackings, `base`, `HavenTypography`, `object HavenType`), renaming `val HavenTypography` to `internal val MaterialTypography`;
- from `Theme.kt`: `val HavenShapes`, renamed `internal val MaterialShapes`.

Add at the top, under the package line, this comment, and at the end this function:

```kotlin
/*
 * Material, for the screens not yet rebuilt from ui/kit (spec 2026-10-03
 * §8: stage 4 rebuilds them, stage 5 deletes this file with material3).
 * Nothing in ui/kit or the rest of ui/theme may use what is here.
 */
```

```kotlin
/** The Material layer under [HavenTheme], until stage 5. */
@Composable
internal fun MaterialBridge(darkTheme: Boolean, content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkScheme else LightScheme,
        typography = MaterialTypography,
        shapes = MaterialShapes,
        content = content,
    )
}
```

Imports the moved code needs: `androidx.compose.foundation.shape.RoundedCornerShape`, `androidx.compose.material3.ColorScheme`, `MaterialTheme`, `Shapes`, `Typography`, `darkColorScheme`, `lightColorScheme`, `androidx.compose.runtime.Composable`, `androidx.compose.ui.graphics.Color`, `androidx.compose.ui.text.TextStyle`, `androidx.compose.ui.text.font.FontFamily`, `androidx.compose.ui.text.font.FontWeight`, `androidx.compose.ui.unit.dp`, `androidx.compose.ui.unit.em`, `androidx.compose.ui.unit.sp`. Delete `Type.kt`.

- [ ] **Step 4: Rewrite `Color.kt`**

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/*
 * The desktop's named colour roles (apps/desktop/DESIGN.md; packages/ui
 * tokens; the per-theme layer at the top of apps/desktop/src/styles.css),
 * light and dark, following the system setting. Dynamic colour is off on
 * purpose: the vault must look like HavenKeys on every phone.
 *
 * Where the phone departs from the desktop (WCAG AA at the phone's
 * 13-16sp text; ContrastTest pins it):
 * - light muted #61706a (desktop #66756f: 4.45:1 on a group)
 * - light brassInk and digit #7d632f (desktop #8f7236: 4.17:1 on a group,
 *   4.0:1 on a brass-soft pill)
 * - dark onDanger #121a17 (desktop white: 3.0:1 on #e0775f)
 * New for the phone: onGlass (toast text; the glass is dark in both themes),
 * thumb (switch and slider thumbs, the desktop's paper white) and scrim (the
 * dim behind a sheet; the desktop has no sheets).
 */
@Immutable
data class HavenColors(
    val isDark: Boolean,
    /** The window ground: screens, unlock. */
    val pane: Color,
    /** A list's ground. */
    val list: Color,
    /** An inset group of rows. */
    val group: Color,
    /** The border of a group and the hairlines between its rows. */
    val groupLine: Color,
    /** An input's or a segmented control's track. */
    val field: Color,
    /** Something lifted: a sheet, a dialog, a menu, the segmented thumb. */
    val raised: Color,
    val hover: Color,
    val line: Color,
    /** The edge of anything interactive. */
    val lineStrong: Color,
    val text: Color,
    val textStrong: Color,
    val muted: Color,
    /** The fitting: controls and thin lines, never large surfaces. */
    val brass: Color,
    val brassHi: Color,
    /** Brass that carries text. */
    val brassInk: Color,
    /** The brass wash: pills, press feedback, a secure note's tile. */
    val brassSoft: Color,
    /** The selection wash: a focused field row. */
    val sel: Color,
    /** The one committing button: brass in dark, forest in light. */
    val primary: Color,
    val onPrimary: Color,
    val onBrass: Color,
    val ok: Color,
    val danger: Color,
    val onDanger: Color,
    /** The toast's ground, dark in both themes. */
    val glass: Color,
    val onGlass: Color,
    /** Digits inside a generated password. */
    val digit: Color,
    /** Symbols inside a generated password. */
    val symbol: Color,
    /** The monogram tile and its initial. */
    val avatarBg: Color,
    val avatarFg: Color,
    /** Switch and slider thumbs. */
    val thumb: Color,
    /** The dim behind a sheet. */
    val scrim: Color,
)

val DarkHavenColors = HavenColors(
    isDark = true,
    pane = Color(0xFF0F1614),
    list = Color(0xFF121B18),
    group = Color(0xFF152120),
    groupLine = Color(0x12E2EAE6),
    field = Color(0xFF0B110F),
    raised = Color(0xFF172320),
    hover = Color(0xFF1B2925),
    line = Color(0xFF1F2C28),
    lineStrong = Color(0xFF2C3D37),
    text = Color(0xFFE2EAE6),
    textStrong = Color(0xFFF3F7F5),
    muted = Color(0xFF86968F),
    brass = Color(0xFFC9A45C),
    brassHi = Color(0xFFE3C483),
    brassInk = Color(0xFFE3C483),
    brassSoft = Color(0x24C9A45C),
    sel = Color(0x26C9A45C),
    primary = Color(0xFFC9A45C),
    onPrimary = Color(0xFF121A17),
    onBrass = Color(0xFF121A17),
    ok = Color(0xFF5FA785),
    danger = Color(0xFFE0775F),
    onDanger = Color(0xFF121A17),
    glass = Color(0xE60F1614),
    onGlass = Color(0xFFF3F7F5),
    digit = Color(0xFFE3C483),
    symbol = Color(0xFF8FC4AD),
    avatarBg = Color(0xFF1D2B27),
    avatarFg = Color(0xFFE3C483),
    thumb = Color(0xFFFBFCFB),
    scrim = Color(0x8C000000),
)

val LightHavenColors = HavenColors(
    isDark = false,
    pane = Color(0xFFFFFFFF),
    list = Color(0xFFF4F7F5),
    group = Color(0xFFF3F6F4),
    groupLine = Color(0xFFE2E8E5),
    field = Color(0xFFFFFFFF),
    raised = Color(0xFFFFFFFF),
    hover = Color(0xFFE9EFEC),
    line = Color(0xFFE2E8E5),
    lineStrong = Color(0xFFCBD5D0),
    text = Color(0xFF1E2B27),
    textStrong = Color(0xFF121A17),
    muted = Color(0xFF61706A),
    brass = Color(0xFFC9A45C),
    brassHi = Color(0xFFE3C483),
    brassInk = Color(0xFF7D632F),
    brassSoft = Color(0x29C9A45C),
    sel = Color(0x2EC9A45C),
    primary = Color(0xFF16231F),
    onPrimary = Color(0xFFF1F5F3),
    onBrass = Color(0xFF121A17),
    ok = Color(0xFF3D6B58),
    danger = Color(0xFFB4412F),
    onDanger = Color(0xFFFFFFFF),
    glass = Color(0xF016231F),
    onGlass = Color(0xFFF3F7F5),
    digit = Color(0xFF7D632F),
    symbol = Color(0xFF3D6B58),
    avatarBg = Color(0xFF16231F),
    avatarFg = Color(0xFFE3C483),
    thumb = Color(0xFFFBFCFB),
    scrim = Color(0x4D16231F),
)
```

- [ ] **Step 5: Rewrite `Theme.kt`**

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf

internal val LocalHavenColors = staticCompositionLocalOf { DarkHavenColors }
internal val LocalHavenMotion = staticCompositionLocalOf { havenMotion(1f) }

/** Light and dark follow the system, like the desktop's "match system". */
@Composable
fun HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalHavenColors provides if (darkTheme) DarkHavenColors else LightHavenColors,
        LocalHavenMotion provides rememberHavenMotion(),
    ) {
        // Screens not yet rebuilt from ui/kit still read MaterialTheme.
        MaterialBridge(darkTheme, content)
    }
}

object HavenTheme {
    val colors: HavenColors
        @Composable @ReadOnlyComposable
        get() = LocalHavenColors.current

    val motion: HavenMotion
        @Composable @ReadOnlyComposable
        get() = LocalHavenMotion.current
}
```

- [ ] **Step 6: Guard the kit against Material**

In `app/build.gradle.kts`, after the `forbidLogging` registration, replace `tasks.named("detekt") { dependsOn(forbidLogging) }` with:

```kotlin
// Spec 2026-10-03 §5: the theme, the kit and the catalogue are built on
// foundation only. Stage 5 widens this to the whole app.
val forbidMaterialInKit by tasks.registering {
    description = "Fails on a Material import in ui/kit, ui/theme (but MaterialBridge.kt) or the catalogue."
    val sources = fileTree("src") {
        include("**/ui/kit/**/*.kt", "**/ui/theme/**/*.kt", "**/catalogue/**/*.kt")
        exclude("**/ui/theme/MaterialBridge.kt")
    }
    inputs.files(sources)
    doLast {
        val material = Regex("""^\s*import\s+androidx\.compose\.material""")
        val hits = sources.flatMap { file ->
            file.readLines().mapIndexedNotNull { i, line ->
                if (material.containsMatchIn(line)) "${file.path}:${i + 1}" else null
            }
        }
        if (hits.isNotEmpty()) {
            throw GradleException("Material is not used in the HavenKeys kit (spec 2026-10-03 §5):\n" + hits.joinToString("\n"))
        }
    }
}
tasks.named("detekt") { dependsOn(forbidLogging, forbidMaterialInKit) }
```

- [ ] **Step 7: Run the tests and the guard**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass. Then add `import androidx.compose.material3.Text` under the imports of `Color.kt`, run `./gradlew forbidMaterialInKit`, expect FAIL naming `Color.kt`, and remove the line again.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/build.gradle.kts apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme apps/android/app/src/test/kotlin/net/havenkeys/android/ui/theme
git commit -m "feat(android): the desktop's colour roles in a theme without Material types"
```

---

### Task 4: Type scale, shapes, spacing and springs

**Files:**
- Create: `ANDROID/ui/theme/Type.kt`
- Create: `ANDROID/ui/theme/Dimens.kt`
- Modify: `ANDROID/ui/theme/Motion.kt`
- Modify: `ANDROID/ui/theme/Theme.kt` (`HavenTheme.type`)
- Test: `ANDROID_TEST/ui/theme/HavenTypographyTest.kt`, `ANDROID_TEST/ui/theme/HavenMotionTest.kt` (extend)

**Interfaces:**
- Consumes: `HankenGrotesk`, `HavenSerif`, `JetBrainsMono`, `HankenWeights`, `SerifWeights` (Task 2).
- Produces:
  - `object HavenTypography { display, headline, title, titleSmall, monogram, body, value, rowTitle, rowSubtitle, label, groupTitle, button, pill, secret, masked, code: TextStyle }`; `HavenTheme.type: HavenTypography`.
  - `object HavenRadius { control = 8.dp, row = 10.dp, group = 12.dp, output = 14.dp, sheet = 18.dp }`.
  - `object HavenShape { control, row, group, output, dialog: Shape; sheet: Shape (top corners); pill: Shape }`.
  - `object HavenSpacing { gutter = 16.dp, rowX = 16.dp, groupGap = 24.dp, touch = 48.dp, rowMin = 52.dp, itemRowMin = 64.dp, tile = 40.dp }`.
  - `val HouseEasing: Easing`; `const val PRESS_SCALE = 0.97f`, `STAGGER_MILLIS = 30`, `FADE_MILLIS = 160`.
  - `data class HavenSpring(dampingRatio: Float, stiffness: Float)` with `HavenSpring.of(responseSeconds, damping)`; `object HavenSprings { smooth, sheet, toast, press }`.
  - On `HavenMotion`: `fun <T> springSpec(kind: HavenSpring): FiniteAnimationSpec<T>`, `fun <T> fadeSpec(): FiniteAnimationSpec<T>`.

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/ui/theme/HavenTypographyTest.kt`:

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.ui.unit.sp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class HavenTypographyTest {
    private val t = HavenTypography
    private val serif = listOf(t.display, t.headline, t.title, t.titleSmall, t.monogram)
    private val mono = listOf(t.secret, t.code)
    private val sans = listOf(t.body, t.value, t.rowTitle, t.rowSubtitle, t.label, t.groupTitle, t.button, t.pill, t.masked)

    @Test
    fun theSpecsPhoneSizes() {
        assertEquals(15.sp, t.body.fontSize)
        assertEquals(16.sp, t.rowTitle.fontSize)
        assertEquals(13.sp, t.label.fontSize)
        assertEquals(28.sp, t.headline.fontSize)
        assertEquals(22.sp, t.code.fontSize)
    }

    @Test
    fun eachFaceKeepsItsJob() {
        serif.forEach { assertSame(HavenSerif, it.fontFamily) }
        mono.forEach { assertSame(JetBrainsMono, it.fontFamily) }
        sans.forEach { assertSame(HankenGrotesk, it.fontFamily) }
    }

    @Test
    fun everyWeightIsOneTheFileWasCutFor() {
        serif.forEach { assertTrue("${it.fontWeight}", it.fontWeight!!.weight in SerifWeights) }
        mono.forEach { assertEquals(500, it.fontWeight!!.weight) }
        sans.forEach { assertTrue("${it.fontWeight}", it.fontWeight!!.weight in HankenWeights) }
    }

    @Test
    fun codesAreTabularSoTheyDoNotJitter() = assertTrue(t.code.fontFeatureSettings!!.contains("tnum"))
}
```

Append to `HavenMotionTest` (add imports `androidx.compose.animation.core.SnapSpec`, `androidx.compose.animation.core.SpringSpec`):

```kotlin
    @Test
    fun springsCutUnderRemoveAnimations() {
        val motion = havenMotion(0f)
        listOf(HavenSprings.smooth, HavenSprings.sheet, HavenSprings.toast, HavenSprings.press).forEach {
            assertTrue(motion.springSpec<Float>(it) is SnapSpec<*>)
        }
        assertTrue(motion.fadeSpec<Float>() is SnapSpec<*>)
    }

    @Test
    fun springsUseApplesResponseAndDamping() {
        val motion = havenMotion(1f)
        val smooth = motion.springSpec<Float>(HavenSprings.smooth) as SpringSpec<Float>
        assertEquals(1f, smooth.dampingRatio, 0f)
        assertEquals(157.91f, smooth.stiffness, 0.01f)
        val sheet = motion.springSpec<Float>(HavenSprings.sheet) as SpringSpec<Float>
        assertEquals(0.86f, sheet.dampingRatio, 0f)
        assertEquals(246.74f, sheet.stiffness, 0.01f)
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HavenTypographyTest*' --tests '*HavenMotionTest*'`
Expected: compile errors (`HavenTypography`, `HavenSprings`, `springSpec` unresolved).

- [ ] **Step 3: Write `Type.kt`**

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * The phone's type scale (spec 2026-10-03 §5.1): the desktop's faces,
 * weights and hierarchy (apps/desktop/DESIGN.md, Typography), with the
 * reading size raised for a phone. Sizes are sp, so text follows the system
 * font size. Serif names things, Hanken does the work, mono is for strings
 * that must be read exactly.
 */
object HavenTypography {
    // The desktop asks Hanken for "ss01", "cv11"; the font has ss01 only.
    private const val SANS_FEATURES = "'ss01'"

    private fun serif(weight: Int, size: Int, line: Int, tracking: Double = 0.0) = TextStyle(
        fontFamily = HavenSerif,
        fontWeight = FontWeight(weight),
        fontSize = size.sp,
        lineHeight = line.sp,
        letterSpacing = tracking.em,
    )

    private fun sans(weight: Int, size: Int, line: Int) = TextStyle(
        fontFamily = HankenGrotesk,
        fontWeight = FontWeight(weight),
        fontSize = size.sp,
        lineHeight = line.sp,
        fontFeatureSettings = SANS_FEATURES,
    )

    /** The unlock headline (its one brass italic is a SpanStyle there). */
    val display = serif(weight = 460, size = 32, line = 36, tracking = -0.015)

    /** Item and editor titles, a list screen's large title. */
    val headline = serif(weight = 480, size = 28, line = 32, tracking = -0.015)

    /** Sheet and dialog titles. */
    val title = serif(weight = 480, size = 22, line = 28, tracking = -0.012)

    /** Empty-state lines, small notices. */
    val titleSmall = serif(weight = 500, size = 18, line = 24)

    /** The initial in a monogram tile (ItemTile scales it to the tile). */
    val monogram = serif(weight = 520, size = 18, line = 22)

    /** The reading size. */
    val body = sans(weight = 400, size = 15, line = 22)

    /** A row's value: what the user opened the item to read. */
    val value = sans(weight = 400, size = 16, line = 22)

    /** The first line of an item row. */
    val rowTitle = sans(weight = 600, size = 16, line = 21)

    /** The second line of a row. */
    val rowSubtitle = sans(weight = 400, size = 14, line = 19)

    /** The label above a value. */
    val label = sans(weight = 560, size = 13, line = 17)

    /** A group's title, sentence case. */
    val groupTitle = sans(weight = 600, size = 14, line = 19)

    /** Button labels. */
    val button = sans(weight = 550, size = 16, line = 20)

    /** Pills and markers. */
    val pill = sans(weight = 600, size = 12, line = 16)

    /** A revealed password, key or setup string. */
    val secret = TextStyle(
        fontFamily = JetBrainsMono,
        fontWeight = FontWeight(500),
        fontSize = 16.sp,
        lineHeight = 22.sp,
        letterSpacing = 0.02.em,
    )

    /** The fixed row of dots that stands for any hidden value. */
    val masked = sans(weight = 500, size = 16, line = 22).copy(letterSpacing = 0.18.em)

    /** A one-time code: tabular, so it does not jitter as it changes. */
    val code = TextStyle(
        fontFamily = JetBrainsMono,
        fontWeight = FontWeight(500),
        fontSize = 22.sp,
        lineHeight = 28.sp,
        letterSpacing = 0.05.em,
        fontFeatureSettings = "'tnum'",
    )
}
```

- [ ] **Step 4: Write `Dimens.kt`**

```kotlin
package net.havenkeys.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp

/** The desktop's radii (spec §5.1: 8, 10, 12, 14, 18 dp). */
object HavenRadius {
    /** Small controls, menu rows, the segmented thumb. */
    val control = 8.dp

    /** Buttons, monogram tiles, the segmented track. */
    val row = 10.dp

    /** Inset groups and field rows. */
    val group = 12.dp

    /** Menus, the generator's output. */
    val output = 14.dp

    /** Sheets and dialogs. */
    val sheet = 18.dp
}

object HavenShape {
    val control: Shape = RoundedCornerShape(HavenRadius.control)
    val row: Shape = RoundedCornerShape(HavenRadius.row)
    val group: Shape = RoundedCornerShape(HavenRadius.group)
    val output: Shape = RoundedCornerShape(HavenRadius.output)
    val dialog: Shape = RoundedCornerShape(HavenRadius.sheet)

    /** A bottom sheet: rounded where it meets the screen above it. */
    val sheet: Shape = RoundedCornerShape(topStart = HavenRadius.sheet, topEnd = HavenRadius.sheet)

    /** Pills, the toast, round buttons. */
    val pill: Shape = RoundedCornerShape(percent = 50)
}

/** The phone's rhythm: the desktop's density, opened up for a thumb. */
object HavenSpacing {
    /** A screen's side margin. */
    val gutter = 16.dp

    /** Horizontal padding inside a row; also where a row's hairline starts. */
    val rowX = 16.dp

    /** Between groups. */
    val groupGap = 24.dp

    /** The smallest touch target (spec §5.2). */
    val touch = 48.dp

    /** A group row's minimum height. */
    val rowMin = 52.dp

    /** An item row's minimum height (40dp tile plus padding). */
    val itemRowMin = 64.dp

    /** A list row's monogram tile. */
    val tile = 40.dp
}
```

- [ ] **Step 5: Extend `Motion.kt`**

Add imports `androidx.compose.animation.core.spring` and `kotlin.math.PI`. After `MechanicalEasing` add:

```kotlin
/** --ease in apps/desktop/src/styles.css: the desktop's one curve. */
val HouseEasing: Easing = CubicBezierEasing(0.32f, 0.72f, 0f, 1f)

/** How far a pressed control shrinks (desktop: 0.97). */
const val PRESS_SCALE = 0.97f

/** The gap between items that arrive in sequence (sheet tiles, Home's groups). */
const val STAGGER_MILLIS = 30

/** --t-fast: fades, colour changes, the copy glyph. */
const val FADE_MILLIS = 160

/**
 * A spring in Apple's terms (SwiftUI's response and damping fraction),
 * converted for Compose: stiffness = (2π / response)², dampingRatio = damping.
 */
@Immutable
data class HavenSpring(val dampingRatio: Float, val stiffness: Float) {
    companion object {
        fun of(responseSeconds: Float, damping: Float): HavenSpring {
            val omega = 2f * PI.toFloat() / responseSeconds
            return HavenSpring(dampingRatio = damping, stiffness = omega * omega)
        }
    }
}

/** The named springs of spec §7. */
object HavenSprings {
    /** Pushes, tab changes, Home's groups settling, the search pill growing: no bounce. */
    val smooth = HavenSpring.of(responseSeconds = 0.5f, damping = 1f)

    /** Sheets and menus: a hint of settle. */
    val sheet = HavenSpring.of(responseSeconds = 0.4f, damping = 0.86f)

    /** The toast rising. */
    val toast = HavenSpring.of(responseSeconds = 0.35f, damping = 0.8f)

    /** Press scale, switch thumb, copy check: quick, no bounce. */
    val press = HavenSpring.of(responseSeconds = 0.2f, damping = 1f)
}
```

Inside `data class HavenMotion`, after `sealSpec()`:

```kotlin
    /** A named spring, or an instant cut under "Remove animations". */
    fun <T> springSpec(kind: HavenSpring): FiniteAnimationSpec<T> =
        if (reduced) snap() else spring(dampingRatio = kind.dampingRatio, stiffness = kind.stiffness)

    /** --t-fast on the house curve: fades and colour changes. */
    fun <T> fadeSpec(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(FADE_MILLIS, easing = HouseEasing)
```

- [ ] **Step 6: Expose the type scale**

In `Theme.kt`, inside `object HavenTheme`, after `colors`:

```kotlin
    val type: HavenTypography
        get() = HavenTypography
```

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ui.theme*' detekt`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme apps/android/app/src/test/kotlin/net/havenkeys/android/ui/theme
git commit -m "feat(android): the phone's type scale, radii, spacing and named springs"
```

---
### Task 5: Kit primitives: text, press, previews, private input

**Files:**
- Create: `ANDROID/ui/kit/HavenText.kt`, `ANDROID/ui/kit/HavenPress.kt`, `ANDROID/ui/kit/KitPreview.kt`
- Move: `ANDROID/ui/edit/NoPersonalizedLearning.kt` → `ANDROID/ui/kit/NoPersonalizedLearning.kt`
- Modify: `ANDROID/ui/edit/EditScreen.kt` (one import)
- Test: `ANDROID_TEST/ui/kit/PrimitivesTest.kt`

**Interfaces:**
- Consumes: `HavenTheme.colors/type/motion`, `LocalHavenColors`, `LocalHavenMotion`, `HavenSprings.press`, `PRESS_SCALE`.
- Produces (package `net.havenkeys.android.ui.kit`):
  - `val LocalHavenContentColor: ProvidableCompositionLocal<Color>`; `@Composable fun ProvideContentColor(color: Color, content: @Composable () -> Unit)`; `@Composable internal fun contentColor(): Color`.
  - `@Composable fun HavenText(text: String, modifier: Modifier = Modifier, style: TextStyle = HavenTheme.type.body, color: Color = Color.Unspecified, maxLines: Int = Int.MAX_VALUE, overflow: TextOverflow = TextOverflow.Clip)` and the same with `text: AnnotatedString`.
  - `object HavenPress : IndicationNodeFactory` (use as `Modifier.clip(shape).clickable(interactionSource = null, indication = HavenPress, …).background(…)`).
  - `@Composable internal fun KitPreview(content: @Composable ColumnScope.() -> Unit)`.
  - `@Composable fun NoPersonalizedLearning(content: @Composable () -> Unit)` (now public, package `ui.kit`).

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/PrimitivesTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.LightHavenColors
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class PrimitivesTest {
    @get:Rule
    val rule = createComposeRule()

    private fun colorOf(text: String): Color {
        val layouts = mutableListOf<TextLayoutResult>()
        rule.onNodeWithText(text).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
        return layouts.single().layoutInput.style.color
    }

    @Test
    fun textTakesTheContentColourUnlessItNamesOne() {
        rule.setKit {
            Column {
                ProvideContentColor(Color.Red) {
                    HavenText("inherits")
                    HavenText("own", color = Color.Blue)
                }
                HavenText("plain")
            }
        }
        assertEquals(Color.Red, colorOf("inherits"))
        assertEquals(Color.Blue, colorOf("own"))
        assertEquals(LightHavenColors.text, colorOf("plain"))
    }

    @Test
    fun pressFeedbackLetsEveryClickThrough() {
        var clicks = 0
        rule.setKit {
            Box(Modifier.size(48.dp).clickable(interactionSource = null, indication = HavenPress, role = Role.Button) { clicks++ })
        }
        val target = rule.onNode(hasClickAction())
        target.performClick()
        target.performTouchInput {
            down(center)
            up()
        }
        rule.waitForIdle()
        assertEquals(2, clicks)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*PrimitivesTest*'`
Expected: compile errors (`ProvideContentColor`, `HavenText`, `HavenPress` unresolved).

- [ ] **Step 3: Write `HavenText.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import net.havenkeys.android.ui.theme.HavenTheme

/** The colour text and glyphs take when the caller names none (foundation has no content colour). */
val LocalHavenContentColor = compositionLocalOf { Color.Unspecified }

@Composable
fun ProvideContentColor(color: Color, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalHavenContentColor provides color, content = content)
}

@Composable
internal fun contentColor(): Color {
    val provided = LocalHavenContentColor.current
    return if (provided.isSpecified) provided else HavenTheme.colors.text
}

/**
 * Text in the kit. Our own words wrap and are never cut; only user data
 * (titles, usernames, URLs) passes `overflow = TextOverflow.Ellipsis`.
 */
@Composable
fun HavenText(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = HavenTheme.type.body,
    color: Color = Color.Unspecified,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val ink = if (color.isSpecified) color else contentColor()
    BasicText(text, modifier, style.copy(color = ink), overflow = overflow, maxLines = maxLines)
}

/** Styled text, such as a generated password with coloured digits and symbols. */
@Composable
fun HavenText(
    text: AnnotatedString,
    modifier: Modifier = Modifier,
    style: TextStyle = HavenTheme.type.body,
    color: Color = Color.Unspecified,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val ink = if (color.isSpecified) color else contentColor()
    BasicText(text, modifier, style.copy(color = ink), overflow = overflow, maxLines = maxLines)
}
```

- [ ] **Step 4: Write `HavenPress.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.currentValueOf
import kotlinx.coroutines.launch
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.LocalHavenColors
import net.havenkeys.android.ui.theme.LocalHavenMotion
import net.havenkeys.android.ui.theme.PRESS_SCALE

/**
 * The kit's only press feedback (spec §5.2: no ripple anywhere). While
 * pressed, the element scales to 0.97 and takes a brass-soft wash, on the
 * press spring; under "Remove animations" both cut. Put it after the clip
 * and before the background, so the background scales with the content:
 * `Modifier.clip(shape).clickable(interactionSource = null, indication = HavenPress) { … }.background(…)`.
 */
object HavenPress : IndicationNodeFactory {
    override fun create(interactionSource: InteractionSource): DelegatableNode = PressNode(interactionSource)

    override fun equals(other: Any?): Boolean = other === this

    override fun hashCode(): Int = javaClass.hashCode()
}

private class PressNode(private val source: InteractionSource) :
    Modifier.Node(),
    DrawModifierNode,
    CompositionLocalConsumerModifierNode {
    private val scale = Animatable(1f)
    private val wash = Animatable(0f)

    override fun onAttach() {
        coroutineScope.launch {
            var held = 0
            source.interactions.collect { interaction ->
                when (interaction) {
                    is PressInteraction.Press -> held++
                    is PressInteraction.Release, is PressInteraction.Cancel -> held = (held - 1).coerceAtLeast(0)
                    else -> return@collect
                }
                settle(pressed = held > 0)
            }
        }
    }

    private fun settle(pressed: Boolean) {
        val spec = currentValueOf(LocalHavenMotion).springSpec<Float>(HavenSprings.press)
        coroutineScope.launch { scale.animateTo(if (pressed) PRESS_SCALE else 1f, spec) }
        coroutineScope.launch { wash.animateTo(if (pressed) 1f else 0f, spec) }
    }

    override fun ContentDrawScope.draw() {
        val tint = currentValueOf(LocalHavenColors).brassSoft
        scale(scale.value) {
            this@draw.drawContent()
            if (wash.value > 0f) drawRect(tint, alpha = wash.value)
        }
    }
}
```

- [ ] **Step 5: Write `KitPreview.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenTheme

/** A preview frame: the theme (light or dark from @PreviewLightDark) on the window ground. */
@Composable
internal fun KitPreview(content: @Composable ColumnScope.() -> Unit) {
    HavenTheme {
        Column(
            Modifier.background(HavenTheme.colors.pane).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}
```

- [ ] **Step 6: Move `NoPersonalizedLearning`**

```bash
git mv apps/android/app/src/main/kotlin/net/havenkeys/android/ui/edit/NoPersonalizedLearning.kt apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit/NoPersonalizedLearning.kt
```

In the moved file change `package net.havenkeys.android.ui.edit` to `package net.havenkeys.android.ui.kit` and `internal fun NoPersonalizedLearning` to `fun NoPersonalizedLearning`. In `ui/edit/EditScreen.kt` add `import net.havenkeys.android.ui.kit.NoPersonalizedLearning` to the imports (sorted). Nothing else in `EditScreen.kt` changes.

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt`
Expected: PASS (the editor tests still pass).

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/main/kotlin/net/havenkeys/android/ui/edit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/PrimitivesTest.kt
git commit -m "feat(android): kit text, press feedback without ripple, preview frame"
```

---

### Task 6: HavenIcon, one icon library with the desktop

**Files:**
- Create: `ANDROID/ui/kit/HavenIcon.kt`
- Modify: `apps/desktop/src/components/Icon.tsx` (five new paths after `printer`)
- Test: `ANDROID_TEST/ui/kit/HavenIconTest.kt`

**Interfaces:**
- Consumes: `contentColor()` (Task 5).
- Produces: `enum class HavenIcon { Lock, Unlock, Search, Key, Note, Grid, Qr, Dice, Gear, Plus, Copy, Check, Eye, EyeOff, Edit, Trash, Globe, Refresh, X, Shield, Clock, ArrowRight, ChevronDown, ChevronUp, Grip, Cloud, CloudOff, Laptop, Alert, Download, IdCard, Card, Printer, Home, Items, ChevronRight, ChevronLeft, More }` with `internal val path: String`, `internal val desktopName: String`; `@Composable fun IconGlyph(icon: HavenIcon, contentDescription: String?, modifier: Modifier = Modifier, tint: Color = Color.Unspecified, size: Dp = 20.dp)`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/HavenIconTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.assertCountEquals
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class HavenIconTest {
    @get:Rule
    val rule = createComposeRule()

    private val desktop: Map<String, String> = Regex("""^\s+(\w+): "([^"]+)",$""", RegexOption.MULTILINE)
        .findAll(File("../../desktop/src/components/Icon.tsx").readText())
        .associate { it.groupValues[1] to it.groupValues[2] }

    @Test
    fun theSetIsTheDesktopsPathForPath() {
        assertEquals(desktop.keys, HavenIcon.entries.map { it.desktopName }.toSet())
        HavenIcon.entries.forEach { assertEquals(it.name, desktop.getValue(it.desktopName), it.path) }
    }

    @Test
    fun everyPathParses() {
        HavenIcon.entries.forEach { assertTrue(it.name, PathParser().parsePathString(it.path).toNodes().isNotEmpty()) }
    }

    @Test
    fun aNamedGlyphIsAnImageAndAnUnnamedOneIsDecoration() {
        rule.setKit {
            IconGlyph(HavenIcon.Lock, contentDescription = "Locked")
            IconGlyph(HavenIcon.Copy, contentDescription = null)
        }
        rule.onNodeWithContentDescription("Locked").assert(hasRole(Role.Image))
        rule.onAllNodesWithContentDescription("Copy").assertCountEquals(0)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HavenIconTest*'`
Expected: compile error, `HavenIcon` unresolved.

- [ ] **Step 3: Draw the five new glyphs in the desktop set**

In `apps/desktop/src/components/Icon.tsx`, after the `printer` entry and before `} as const;`:

```ts
  // Drawn for the phone (tab bar, disclosure, back, menus) to the same rules;
  // kept here so both apps draw from one set (apps/android …/ui/kit/HavenIcon.kt).
  home: "M4.5 10.5 12 4.5l7.5 6V19a1 1 0 0 1-1 1H15v-5.5H9V20H5.5a1 1 0 0 1-1-1v-8.5z",
  items: "M9 7h10.5M9 12h10.5M9 17h10.5M5 7h.01M5 12h.01M5 17h.01",
  chevronRight: "M9.5 6.5 15 12l-5.5 5.5",
  chevronLeft: "M14.5 6.5 9 12l5.5 5.5",
  more: "M5.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM11.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM17.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0z",
```

Run: `pnpm --filter @havenkeys/desktop typecheck`
Expected: no errors.

- [ ] **Step 4: Write `HavenIcon.kt`**

Copy each path string **character for character** from `Icon.tsx` (the test compares them).

```kotlin
@file:Suppress("MaxLineLength") // Path data is copied verbatim from the desktop's Icon.tsx.

package net.havenkeys.android.ui.kit

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The HavenKeys icon set: apps/desktop/src/components/Icon.tsx, path for
 * path (24 grid, 1.6 stroke, round caps and joins, drawn to sit with Hanken
 * Grotesk). It replaces material-icons-extended. A glyph the phone needs is
 * drawn to the same rules and added to the desktop set too; HavenIconTest
 * keeps the two identical.
 */
enum class HavenIcon(internal val path: String) {
    Lock("M7.5 10.5V8a4.5 4.5 0 0 1 9 0v2.5M6 10.5h12a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-7.5a1 1 0 0 1 1-1zM12 14.5v2"),
    Unlock("M7.5 10.5V8a4.5 4.5 0 0 1 8.7-1.6M6 10.5h12a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-7.5a1 1 0 0 1 1-1z"),
    Search("M10.5 17.5a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4.5-4.5"),
    Key("M14.5 3.5a6 6 0 1 1-5.2 9L4 17.8V20h2.5v-2h2v-2h2l1.4-1.4A6 6 0 0 1 14.5 3.5zM16 8h.01"),
    Note("M7 3.5h7l4.5 4.5v11.5a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-15a1 1 0 0 1 1-1zM13.5 3.5V8.5h5M9 12.5h6M9 16h4"),
    Grid("M5 4.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5H5a.5.5 0 0 1-.5-.5V5a.5.5 0 0 1 .5-.5zM14 4.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5V5a.5.5 0 0 1 .5-.5zM5 13.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5H5a.5.5 0 0 1-.5-.5v-5a.5.5 0 0 1 .5-.5zM14 13.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5v-5a.5.5 0 0 1 .5-.5z"),
    Qr("M4.5 4.5h6v6h-6zM13.5 4.5h6v6h-6zM4.5 13.5h6v6h-6zM7 7h1M16 7h1M7 16h1M13.5 13.5h2.5v2.5M19.5 13.5v2.5M13.5 19.5h2.5M18.5 18.5h1v1"),
    Dice("M12 3.5 19.5 7.5v9L12 20.5 4.5 16.5v-9L12 3.5zM4.5 7.5 12 11.5l7.5-4M12 11.5v9"),
    Gear("M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM10.3 3.5h3.4l.5 2.3 1.6.9 2.2-.8 1.7 3-1.8 1.5v1.8l1.8 1.5-1.7 3-2.2-.8-1.6.9-.5 2.3h-3.4l-.5-2.3-1.6-.9-2.2.8-1.7-3 1.8-1.5v-1.8L4.3 8.9l1.7-3 2.2.8 1.6-.9.5-2.3z"),
    Plus("M12 5v14M5 12h14"),
    Copy("M9 8.5h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1zM16 8.5V6a1 1 0 0 0-1-1H6a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h2"),
    Check("M5 12.5l4.5 4.5L19 7.5"),
    Eye("M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"),
    EyeOff("M4 4l16 16M10.2 5.7A9 9 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a16 16 0 0 1-2.9 3.7M6.6 6.9A15.6 15.6 0 0 0 2.5 12S6 18.5 12 18.5a8.8 8.8 0 0 0 4.6-1.3M9.9 9.9a3 3 0 0 0 4.2 4.2"),
    Edit("M4.5 19.5h4l10-10a2.1 2.1 0 0 0-3-3l-10 10v3zM14 8l3 3"),
    Trash("M4.5 7h15M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2M6.5 7l.9 12.1a1 1 0 0 0 1 .9h7.2a1 1 0 0 0 1-.9L17.5 7"),
    Globe("M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM3.5 12h17M12 3.5c2.3 2.3 3.5 5.2 3.5 8.5s-1.2 6.2-3.5 8.5c-2.3-2.3-3.5-5.2-3.5-8.5S9.7 5.8 12 3.5z"),
    Refresh("M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4.5H15"),
    X("M6.5 6.5l11 11M17.5 6.5l-11 11"),
    Shield("M12 3.5l7 2.8V12c0 4.3-3 7.4-7 8.8-4-1.4-7-4.5-7-8.8V6.3l7-2.8z"),
    Clock("M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM12 7.5V12l3 2"),
    ArrowRight("M5 12h14M13 6l6 6-6 6"),
    ChevronDown("M6.5 9.5 12 15l5.5-5.5"),
    ChevronUp("M6.5 14.5 12 9l5.5 5.5"),
    Grip("M8.4 6a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM14.4 6a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM8.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM14.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM8.4 18a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM14.4 18a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0z"),
    Cloud("M7 18.5h10a4 4 0 0 0 .6-8A5.5 5.5 0 0 0 7 9.5a4.5 4.5 0 0 0 0 9z"),
    CloudOff("M4 4l16 16M9 6.4A5.5 5.5 0 0 1 17.6 10.5 4 4 0 0 1 19.8 17M17 18.5H7a4.5 4.5 0 0 1-1.7-8.7"),
    Laptop("M5.5 6h13a1 1 0 0 1 1 1v8.5h-15V7a1 1 0 0 1 1-1zM2.5 18h19"),
    Alert("M12 4 21 19.5H3L12 4zM12 10v4M12 17h.01"),
    Download("M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14"),
    IdCard("M4.5 6h15a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM9 12a1.8 1.8 0 1 0 0-3.6A1.8 1.8 0 0 0 9 12zM6 15.5c.5-1.3 1.6-2 3-2s2.5.7 3 2M14 10h3.5M14 13.5h3.5"),
    Card("M4.5 6h15a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM3.5 10h17M7 14.5h3"),
    Printer("M7 9V4h10v5M7 17H5a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-2M7 14h10v6H7z"),
    Home("M4.5 10.5 12 4.5l7.5 6V19a1 1 0 0 1-1 1H15v-5.5H9V20H5.5a1 1 0 0 1-1-1v-8.5z"),
    Items("M9 7h10.5M9 12h10.5M9 17h10.5M5 7h.01M5 12h.01M5 17h.01"),
    ChevronRight("M9.5 6.5 15 12l-5.5 5.5"),
    ChevronLeft("M14.5 6.5 9 12l5.5 5.5"),
    More("M5.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM11.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM17.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0z"),
    ;

    /** This glyph's key in Icon.tsx. */
    internal val desktopName: String get() = name.replaceFirstChar { it.lowercaseChar() }

    internal val vector: ImageVector by lazy { glyph(name, path) }
}

private const val GRID = 24f
private const val STROKE = 1.6f

private fun glyph(name: String, path: String): ImageVector =
    ImageVector.Builder(
        name = name,
        defaultWidth = GRID.dp,
        defaultHeight = GRID.dp,
        viewportWidth = GRID,
        viewportHeight = GRID,
    ).addPath(
        pathData = PathParser().parsePathString(path).toNodes(),
        fill = null,
        stroke = SolidColor(Color.Black),
        strokeLineWidth = STROKE,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    ).build()

/**
 * One glyph. With a [contentDescription] it is an image TalkBack reads;
 * with null it is decoration (the control around it carries the label).
 */
@Composable
fun IconGlyph(
    icon: HavenIcon,
    contentDescription: String?,
    modifier: Modifier = Modifier,
    tint: Color = Color.Unspecified,
    size: Dp = 20.dp,
) {
    Image(
        painter = rememberVectorPainter(icon.vector),
        contentDescription = contentDescription,
        modifier = modifier.size(size),
        colorFilter = ColorFilter.tint(if (tint.isSpecified) tint else contentColor()),
    )
}

@PreviewLightDark
@Composable
private fun HavenIconPreview() {
    KitPreview {
        HavenIcon.entries.chunked(8).forEach { row ->
            Row {
                row.forEach { IconGlyph(it, contentDescription = null, size = 28.dp) }
            }
        }
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HavenIconTest*' detekt`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit/HavenIcon.kt apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/HavenIconTest.kt apps/desktop/src/components/Icon.tsx
git commit -m "feat(android): HavenIcon from the desktop's paths; home, items, chevrons and more in both sets"
```

---
### Task 7: Actions: buttons, icon button, copy, pill, add

**Files:**
- Create: `ANDROID/ui/kit/HavenButton.kt`, `HavenIconButton.kt`, `CopyButton.kt`, `Pill.kt`, `AddButton.kt`
- Modify: `apps/android/app/src/main/res/values/strings.xml`, `values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/kit/ActionsTest.kt`

**Interfaces:**
- Consumes: `HavenText`, `HavenPress`, `IconGlyph`, `HavenIcon`, `KitPreview`, `HavenShape`, `HavenSpacing`, `HavenTheme.colors/type/motion`; strings `R.string.copy` ("Copy %1$s").
- Produces:
  - `enum class ButtonStyle { Primary, Secondary, Quiet, Danger }`; `@Composable fun HavenButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, style: ButtonStyle = ButtonStyle.Primary, enabled: Boolean = true, icon: HavenIcon? = null)`; `internal const val DISABLED_ALPHA = 0.42f`.
  - `@Composable fun HavenIconButton(icon: HavenIcon, contentDescription: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, tint: Color = Color.Unspecified)`.
  - `@Composable fun CopyButton(fieldLabel: String, onCopy: () -> Unit, modifier: Modifier = Modifier)`; `internal const val COPIED_MILLIS = 1_500L`.
  - `enum class PillTone { Brass, Outline }`; `@Composable fun Pill(text: String, modifier: Modifier = Modifier, tone: PillTone = PillTone.Brass)`.
  - `@Composable fun AddButton(onClick: () -> Unit, modifier: Modifier = Modifier, contentDescription: String = stringResource(R.string.kit_new_item))`.
  - Strings `kit_copied`, `kit_new_item`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/ActionsTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ActionsTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aButtonIsAButtonNamedByItsLabel() {
        var clicks = 0
        rule.setKit { HavenButton("Save", onClick = { clicks++ }) }
        rule.onNode(hasText("Save") and hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, clicks)
    }

    @Test
    fun aDisabledButtonSaysSoAndDoesNothing() {
        var clicks = 0
        rule.setKit { HavenButton("Save", onClick = { clicks++ }, enabled = false) }
        rule.onNodeWithText("Save").assertIsNotEnabled().performClick()
        assertEquals(0, clicks)
    }

    @Test
    fun everyStyleKeepsTheTouchTarget() {
        rule.setKit { Column { ButtonStyle.entries.forEach { HavenButton(it.name, onClick = {}, style = it) } } }
        ButtonStyle.entries.forEach { rule.onNodeWithText(it.name).assert(hasRole(Role.Button)).assertTouchTarget() }
    }

    @Test
    fun theLargestFontGrowsTheButtonInsteadOfCuttingIt() {
        rule.setKit(fontScale = 2f) {
            HavenButton("Remove this device", onClick = {}, modifier = Modifier.width(200.dp))
        }
        rule.onNodeWithText("Remove this device").assertIsDisplayed().assertHeightIsAtLeast(64.dp)
    }

    @Test
    fun anIconButtonIsNamedAndBigEnough() {
        var clicks = 0
        rule.setKit { HavenIconButton(HavenIcon.Lock, "Lock", onClick = { clicks++ }) }
        rule.onNodeWithContentDescription("Lock").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, clicks)
    }

    @Test
    fun copyNamesTheFieldThenSaysCopiedForAMoment() {
        rule.mainClock.autoAdvance = false
        var copies = 0
        rule.setKit { CopyButton("Password", onCopy = { copies++ }) }
        val button = rule.onNodeWithContentDescription("Copy Password")
        button.assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.mainClock.advanceTimeBy(100)
        assertEquals(1, copies)
        button.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Copied"))
        rule.mainClock.advanceTimeBy(COPIED_MILLIS + 500)
        button.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.StateDescription))
    }

    @Test
    fun aPillIsAMarkerNotAControl() {
        rule.setKit { Pill("This device") }
        rule.onNodeWithText("This device").assertIsDisplayed().assertHasNoClickAction()
    }

    @Test
    fun theAddButtonIsNamed() {
        var clicks = 0
        rule.setKit { AddButton(onClick = { clicks++ }) }
        rule.onNodeWithContentDescription("New item").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, clicks)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ActionsTest*'`
Expected: compile errors (`HavenButton`, `CopyButton`, … unresolved).

- [ ] **Step 3: Add the strings**

`res/values/strings.xml`, before `</resources>`:

```xml
    <!-- ui/kit (spec 2026-10-03 §5.2) -->
    <string name="kit_copied">Copied</string>
    <string name="kit_new_item">New item</string>
```

`res/values-pt-rBR/strings.xml`, before `</resources>`:

```xml
    <!-- ui/kit (spec 2026-10-03 §5.2) -->
    <string name="kit_copied">Copiado</string>
    <string name="kit_new_item">Novo item</string>
```

- [ ] **Step 4: Write `HavenButton.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

enum class ButtonStyle { Primary, Secondary, Quiet, Danger }

/** Disabled controls fade to this opacity (desktop: 42%). */
internal const val DISABLED_ALPHA = 0.42f

/**
 * A button. One [ButtonStyle.Primary] per screen: brass in dark, forest in
 * light (the desktop's One Fitting Rule). Secondary is outlined, quiet is
 * bare text, danger fills ember. The label wraps; it is never cut.
 */
@Composable
fun HavenButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    style: ButtonStyle = ButtonStyle.Primary,
    enabled: Boolean = true,
    icon: HavenIcon? = null,
) {
    val colors = HavenTheme.colors
    val ground = when (style) {
        ButtonStyle.Primary -> colors.primary
        ButtonStyle.Danger -> colors.danger
        ButtonStyle.Secondary, ButtonStyle.Quiet -> Color.Transparent
    }
    val ink = when (style) {
        ButtonStyle.Primary -> colors.onPrimary
        ButtonStyle.Danger -> colors.onDanger
        ButtonStyle.Secondary, ButtonStyle.Quiet -> colors.textStrong
    }
    val edge = if (style == ButtonStyle.Secondary) colors.lineStrong else Color.Transparent
    Row(
        modifier = modifier
            .defaultMinSize(minWidth = HavenSpacing.touch, minHeight = HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(HavenShape.row)
            .clickable(interactionSource = null, indication = HavenPress, enabled = enabled, role = Role.Button, onClick = onClick)
            .background(ground, HavenShape.row)
            .border(1.dp, edge, HavenShape.row)
            .padding(horizontal = 18.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            IconGlyph(icon, contentDescription = null, tint = ink, size = 18.dp)
            Spacer(Modifier.width(8.dp))
        }
        HavenText(text, style = HavenTheme.type.button.copy(textAlign = TextAlign.Center), color = ink)
    }
}

@PreviewLightDark
@Composable
private fun HavenButtonPreview() {
    KitPreview {
        HavenButton("Unlock", onClick = {}, modifier = Modifier.fillMaxWidth())
        HavenButton("Generate", onClick = {}, style = ButtonStyle.Secondary, icon = HavenIcon.Dice)
        HavenButton("Not now", onClick = {}, style = ButtonStyle.Quiet)
        HavenButton("Remove this device", onClick = {}, style = ButtonStyle.Danger)
        HavenButton("Disabled", onClick = {}, enabled = false)
    }
}
```

- [ ] **Step 5: Write `HavenIconButton.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** A glyph that is a button: 48dp round target, muted unless [tint] says otherwise. */
@Composable
fun HavenIconButton(
    icon: HavenIcon,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    tint: Color = Color.Unspecified,
) {
    val ink = if (tint.isSpecified) tint else HavenTheme.colors.muted
    Box(
        modifier = modifier
            .size(HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(CircleShape)
            .clickable(interactionSource = null, indication = HavenPress, enabled = enabled, role = Role.Button, onClick = onClick)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        IconGlyph(icon, contentDescription = null, tint = ink, size = 22.dp)
    }
}

@PreviewLightDark
@Composable
private fun HavenIconButtonPreview() {
    KitPreview {
        Row {
            HavenIconButton(HavenIcon.Lock, "Lock", onClick = {})
            HavenIconButton(HavenIcon.More, "More", onClick = {})
            HavenIconButton(HavenIcon.Trash, "Delete", onClick = {}, tint = HavenTheme.colors.danger)
        }
    }
}
```

- [ ] **Step 6: Write `CopyButton.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** How long the check stays after a copy. */
internal const val COPIED_MILLIS = 1_500L

/**
 * Copies one field: the glyph turns into a green check for a moment, with a
 * light haptic (spec §7). The caller does the copy, its toast and clearing
 * the clipboard. The label names the field ("Copy Password"), never its value.
 */
@Composable
fun CopyButton(fieldLabel: String, onCopy: () -> Unit, modifier: Modifier = Modifier) {
    var copied by remember { mutableStateOf(false) }
    val haptics = LocalHapticFeedback.current
    val colors = HavenTheme.colors
    val label = stringResource(R.string.copy, fieldLabel)
    val done = stringResource(R.string.kit_copied)
    LaunchedEffect(copied) {
        if (copied) {
            delay(COPIED_MILLIS)
            copied = false
        }
    }
    Box(
        modifier = modifier
            .size(HavenSpacing.touch)
            .clip(CircleShape)
            .clickable(interactionSource = null, indication = HavenPress, role = Role.Button) {
                onCopy()
                haptics.performHapticFeedback(HapticFeedbackType.Confirm)
                copied = true
            }
            .semantics {
                contentDescription = label
                if (copied) stateDescription = done
            },
        contentAlignment = Alignment.Center,
    ) {
        Crossfade(targetState = copied, animationSpec = HavenTheme.motion.fadeSpec(), label = "copy") { isDone ->
            IconGlyph(
                if (isDone) HavenIcon.Check else HavenIcon.Copy,
                contentDescription = null,
                tint = if (isDone) colors.ok else colors.muted,
                size = 20.dp,
            )
        }
    }
}

@PreviewLightDark
@Composable
private fun CopyButtonPreview() {
    KitPreview { CopyButton("Password", onCopy = {}) }
}
```

- [ ] **Step 7: Write `Pill.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

enum class PillTone {
    /** Brass wash, brass ink: a state or a device marker ("This device"). */
    Brass,

    /** Outlined, muted: how something applies ("Whole site"). */
    Outline,
}

/** A small marker. Not a control: it has no action and needs no touch target. */
@Composable
fun Pill(text: String, modifier: Modifier = Modifier, tone: PillTone = PillTone.Brass) {
    val colors = HavenTheme.colors
    val ground = if (tone == PillTone.Brass) colors.brassSoft else Color.Transparent
    val edge = if (tone == PillTone.Outline) colors.lineStrong else Color.Transparent
    HavenText(
        text,
        modifier
            .clip(HavenShape.pill)
            .background(ground)
            .border(1.dp, edge, HavenShape.pill)
            .padding(horizontal = 9.dp, vertical = 3.dp),
        style = HavenTheme.type.pill,
        color = if (tone == PillTone.Brass) colors.brassInk else colors.muted,
    )
}

@PreviewLightDark
@Composable
private fun PillPreview() {
    KitPreview {
        Row {
            Pill("This device")
            Pill("Whole site", Modifier.padding(start = 8.dp), tone = PillTone.Outline)
        }
    }
}
```

- [ ] **Step 8: Write `AddButton.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme

private val AddSize = 56.dp

/**
 * The floating add button (spec §6.6): the primary colour, a plus, a soft
 * shadow because it floats. The screen places it; HavenScaffold leaves room.
 */
@Composable
fun AddButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    contentDescription: String = stringResource(R.string.kit_new_item),
) {
    val colors = HavenTheme.colors
    Box(
        modifier = modifier
            .size(AddSize)
            .shadow(elevation = 10.dp, shape = CircleShape)
            .clip(CircleShape)
            .clickable(interactionSource = null, indication = HavenPress, role = Role.Button, onClick = onClick)
            .background(colors.primary)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        IconGlyph(HavenIcon.Plus, contentDescription = null, tint = colors.onPrimary, size = 26.dp)
    }
}

@PreviewLightDark
@Composable
private fun AddButtonPreview() {
    KitPreview { AddButton(onClick = {}) }
}
```

- [ ] **Step 9: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ActionsTest*' detekt lintGithubDebug`
Expected: PASS, no MissingTranslation.

- [ ] **Step 10: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/ActionsTest.kt
git commit -m "feat(android): kit buttons, icon button, copy button, pill and add button"
```

---

### Task 8: Structure: section header, inset group, group row, item row

**Files:**
- Create: `ANDROID/ui/kit/SectionHeader.kt`, `InsetGroup.kt`, `GroupRow.kt`, `ItemRow.kt`
- Test: `ANDROID_TEST/ui/kit/StructureTest.kt`

**Interfaces:**
- Consumes: Task 5-7 primitives; strings `R.string.item_passkey` ("Passkey"), `R.string.field_totp` ("One-time code").
- Produces:
  - `class SectionAction(val label: String, val onClick: () -> Unit)`; `@Composable fun SectionHeader(text: String, modifier: Modifier = Modifier, action: SectionAction? = null)`.
  - `class InsetGroupScope { fun row(content: @Composable () -> Unit) }`; `@Composable fun InsetGroup(modifier: Modifier = Modifier, content: InsetGroupScope.() -> Unit)`.
  - `@Composable fun GroupRow(modifier: Modifier = Modifier, onClick: (() -> Unit)? = null, onClickLabel: String? = null, icon: HavenIcon? = null, trailing: (@Composable RowScope.() -> Unit)? = null, chevron: Boolean = onClick != null && trailing == null, content: @Composable ColumnScope.() -> Unit)`; `@Composable fun GroupRowText(title: String, detail: String? = null)`; `@Composable fun GroupRowField(label: String, value: String, valueStyle: TextStyle = HavenTheme.type.value)`; `@Composable fun TrailingText(text: String)`.
  - `sealed interface RowLeading { data class Monogram(val title: String); data class Glyph(val icon: HavenIcon, val soft: Boolean = false) }`; `@Composable fun ItemRow(title: String, subtitle: String?, leading: RowLeading, onClick: () -> Unit, modifier: Modifier = Modifier, titleModifier: Modifier = Modifier, hasPasskey: Boolean = false, hasCode: Boolean = false)`; `@Composable fun ItemTile(leading: RowLeading, modifier: Modifier = Modifier, size: Dp = HavenSpacing.tile)`; `internal fun monogramOf(title: String): String?`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/StructureTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class StructureTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aHeaderIsAHeadingAndItsActionAButton() {
        var cleared = 0
        rule.setKit { SectionHeader("Recent searches", action = SectionAction("Clear") { cleared++ }) }
        rule.onNodeWithText("Recent searches").assert(isHeading())
        rule.onNode(hasText("Clear") and hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, cleared)
    }

    @Test
    fun aGroupStacksItsRowsInOrder() {
        rule.setKit {
            InsetGroup {
                row { GroupRow { GroupRowText("One") } }
                row { GroupRow { GroupRowText("Two") } }
                row { GroupRow { GroupRowText("Three") } }
            }
        }
        val tops = listOf("One", "Two", "Three").map { rule.onNodeWithText(it).getUnclippedBoundsInRoot().top }
        assertEquals(tops.sorted(), tops)
    }

    @Test
    fun aTappableRowIsAButtonAndAPlainRowIsNot() {
        var opened = 0
        rule.setKit {
            InsetGroup {
                row { GroupRow(onClick = { opened++ }, onClickLabel = "Open") { GroupRowText("Auto-lock", "After 5 minutes") } }
                row { GroupRow { GroupRowField("Username", "sam@example.com") } }
            }
        }
        rule.onNode(hasText("Auto-lock") and hasRole(Role.Button)).assert(hasText("After 5 minutes"))
            .assertHeightIsAtLeast(52.dp).performClick()
        assertEquals(1, opened)
        rule.onNodeWithText("sam@example.com").assertHasNoClickAction()
    }

    @Test
    fun anItemRowReadsAsOneButtonWithItsMarks() {
        var opened = 0
        rule.setKit {
            ItemRow("GitHub", "sam@example.com", RowLeading.Monogram("GitHub"), onClick = { opened++ }, hasPasskey = true, hasCode = true)
        }
        rule.onNode(hasClickAction())
            .assert(hasRole(Role.Button))
            .assert(hasText("GitHub"))
            .assert(hasText("sam@example.com"))
            .assert(hasContentDescription("Passkey"))
            .assert(hasContentDescription("One-time code"))
            .assertHeightIsAtLeast(64.dp)
            .performClick()
        assertEquals(1, opened)
        // The monogram is decoration: TalkBack reads the title once.
        rule.onNodeWithText("G").assertDoesNotExist()
    }

    @Test
    fun noMarksNoMarkLabels() {
        rule.setKit { ItemRow("Wi-Fi", null, RowLeading.Glyph(HavenIcon.Note, soft = true), onClick = {}) }
        val row = rule.onNode(hasClickAction()).fetchSemanticsNode()
        assertFalse(hasContentDescription("Passkey").matches(row))
        assertFalse(hasContentDescription("One-time code").matches(row))
    }

    @Test
    fun theLargestFontGrowsTheRow() {
        rule.setKit(fontScale = 2f) { ItemRow("GitHub", "sam@example.com", RowLeading.Monogram("GitHub"), onClick = {}) }
        rule.onNodeWithText("GitHub", useUnmergedTree = true).assertIsDisplayed()
        rule.onNode(hasClickAction()).assertHeightIsAtLeast(80.dp)
    }

    @Test
    fun monogramsTakeTheFirstCharacterAsTheReaderSeesIt() {
        assertEquals("G", monogramOf("github"))
        assertEquals("É", monogramOf("  émile"))
        assertEquals("😀", monogramOf("😀 Fun"))
        assertEquals("東", monogramOf("東京"))
        assertEquals("ß", monogramOf("ßeta"))
        assertNull(monogramOf("   "))
        assertNull(monogramOf(""))
        assertTrue(monogramOf("😀 Fun")!!.length == 2)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*StructureTest*'`
Expected: compile errors (`SectionHeader`, `InsetGroup`, … unresolved).

- [ ] **Step 3: Write `SectionHeader.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** A trailing action on a header, such as "Clear" over recent searches. */
class SectionAction(val label: String, val onClick: () -> Unit)

/** A group's title: sentence case, muted, a heading for TalkBack; text lines up with the rows' text. */
@Composable
fun SectionHeader(text: String, modifier: Modifier = Modifier, action: SectionAction? = null) {
    val colors = HavenTheme.colors
    Row(
        modifier.fillMaxWidth().heightIn(min = 36.dp).padding(start = HavenSpacing.rowX),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HavenText(
            text,
            Modifier.weight(1f).padding(vertical = 8.dp).semantics { heading() },
            style = HavenTheme.type.groupTitle,
            color = colors.muted,
        )
        if (action != null) {
            Box(
                Modifier
                    .heightIn(min = HavenSpacing.touch)
                    .clip(HavenShape.control)
                    .clickable(interactionSource = null, indication = HavenPress, role = Role.Button, onClick = action.onClick)
                    .padding(horizontal = HavenSpacing.rowX),
                contentAlignment = Alignment.Center,
            ) {
                HavenText(action.label, style = HavenTheme.type.groupTitle, color = colors.brassInk)
            }
        }
    }
}

@PreviewLightDark
@Composable
private fun SectionHeaderPreview() {
    KitPreview {
        SectionHeader("Frequently used")
        SectionHeader("Recent searches", action = SectionAction("Clear") {})
    }
}
```

- [ ] **Step 4: Write `InsetGroup.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Collects an [InsetGroup]'s rows so the group can draw the hairlines between them. */
class InsetGroupScope internal constructor() {
    internal val rows = mutableListOf<@Composable () -> Unit>()

    fun row(content: @Composable () -> Unit) {
        rows += content
    }
}

/**
 * The desktop's inset group: a 12dp-rounded surface with a hairline border,
 * its rows separated by hairlines that start where the rows' text starts.
 * Rows never get a card or border of their own.
 */
@Composable
fun InsetGroup(modifier: Modifier = Modifier, content: InsetGroupScope.() -> Unit) {
    val colors = HavenTheme.colors
    val rows = InsetGroupScope().apply(content).rows
    Column(
        modifier
            .fillMaxWidth()
            .clip(HavenShape.group)
            .background(colors.group)
            .border(1.dp, colors.groupLine, HavenShape.group),
    ) {
        rows.forEachIndexed { index, row ->
            if (index > 0) {
                Box(Modifier.padding(start = HavenSpacing.rowX).fillMaxWidth().height(1.dp).background(colors.groupLine))
            }
            row()
        }
    }
}

@PreviewLightDark
@Composable
private fun InsetGroupPreview() {
    KitPreview {
        InsetGroup {
            row { GroupRow(onClick = {}) { GroupRowText("All items") } }
            row { GroupRow(onClick = {}) { GroupRowText("Logins") } }
        }
    }
}
```

- [ ] **Step 5: Write `GroupRow.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * One row of an [InsetGroup]: an optional glyph, the row's text
 * ([GroupRowText] or [GroupRowField]), then [trailing] controls or, when it
 * opens something, a chevron. A tappable row is one button for TalkBack.
 */
@Composable
fun GroupRow(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    onClickLabel: String? = null,
    icon: HavenIcon? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
    chevron: Boolean = onClick != null && trailing == null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = HavenTheme.colors
    val click = if (onClick == null) {
        Modifier
    } else {
        Modifier.clickable(
            interactionSource = null,
            indication = HavenPress,
            onClickLabel = onClickLabel,
            role = Role.Button,
            onClick = onClick,
        )
    }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .then(click)
            .padding(
                start = HavenSpacing.rowX,
                end = if (trailing != null || chevron) 8.dp else HavenSpacing.rowX,
                top = 10.dp,
                bottom = 10.dp,
            ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            IconGlyph(icon, contentDescription = null, tint = colors.muted, size = 22.dp)
            Spacer(Modifier.width(14.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp), content = content)
        trailing?.invoke(this)
        if (chevron) {
            IconGlyph(HavenIcon.ChevronRight, contentDescription = null, Modifier.padding(start = 4.dp, end = 6.dp), tint = colors.muted, size = 18.dp)
        }
    }
}

/** A row's text: a title, and an optional muted line under it. */
@Composable
fun GroupRowText(title: String, detail: String? = null) {
    HavenText(title, style = HavenTheme.type.value, color = HavenTheme.colors.textStrong)
    if (detail != null) HavenText(detail, style = HavenTheme.type.rowSubtitle, color = HavenTheme.colors.muted)
}

/** A field as an item's detail shows it: its label above its value. */
@Composable
fun GroupRowField(label: String, value: String, valueStyle: TextStyle = HavenTheme.type.value) {
    HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
    HavenText(value, style = valueStyle, color = HavenTheme.colors.textStrong)
}

/** A muted value at a row's end: a count, the current setting. */
@Composable
fun TrailingText(text: String) {
    HavenText(text, style = HavenTheme.type.value, color = HavenTheme.colors.muted)
}

@PreviewLightDark
@Composable
private fun GroupRowPreview() {
    KitPreview {
        InsetGroup {
            row { GroupRow(onClick = {}, icon = HavenIcon.Clock) { GroupRowText("Auto-lock", "After 5 minutes") } }
            row { GroupRow(onClick = {}, trailing = { TrailingText("12") }) { GroupRowText("Logins") } }
            row { GroupRow(trailing = { CopyButton("Username", onCopy = {}) }) { GroupRowField("Username", "sam@example.com") } }
        }
    }
}
```

- [ ] **Step 6: Write `ItemRow.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import java.text.BreakIterator
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** What stands at the start of an item row: the title's initial, or the kind's glyph. */
sealed interface RowLeading {
    data class Monogram(val title: String) : RowLeading

    /** [soft]: on the brass wash, as the desktop draws secure notes. */
    data class Glyph(val icon: HavenIcon, val soft: Boolean = false) : RowLeading
}

private const val MONOGRAM_RATIO = 0.45f

/**
 * One vault item in a list: tile, title, non-secret subtitle, and marks for
 * a passkey and a one-time code. One button for TalkBack; the tile is
 * decoration. [titleModifier] lets the title travel to the detail screen.
 * Only the title and subtitle (user data) may be cut with an ellipsis.
 */
@Composable
fun ItemRow(
    title: String,
    subtitle: String?,
    leading: RowLeading,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
    hasPasskey: Boolean = false,
    hasCode: Boolean = false,
) {
    val colors = HavenTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.itemRowMin)
            .clickable(interactionSource = null, indication = HavenPress, role = Role.Button, onClick = onClick)
            .padding(horizontal = HavenSpacing.rowX, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ItemTile(leading)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            HavenText(title, titleModifier, style = HavenTheme.type.rowTitle, color = colors.textStrong, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle != null) {
                HavenText(subtitle, style = HavenTheme.type.rowSubtitle, color = colors.muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        if (hasCode) {
            IconGlyph(HavenIcon.Clock, stringResource(R.string.field_totp), Modifier.padding(start = 8.dp), tint = colors.muted, size = 16.dp)
        }
        if (hasPasskey) {
            IconGlyph(HavenIcon.Key, stringResource(R.string.item_passkey), Modifier.padding(start = 8.dp), tint = colors.muted, size = 16.dp)
        }
    }
}

/**
 * The tile: a serif initial on the avatar ground, or a glyph. A blank title
 * shows the key. The initial keeps its size at any system font size, so it
 * never spills out of the tile.
 */
@Composable
fun ItemTile(leading: RowLeading, modifier: Modifier = Modifier, size: Dp = HavenSpacing.tile) {
    val colors = HavenTheme.colors
    val initial = (leading as? RowLeading.Monogram)?.let { monogramOf(it.title) }
    val glyph = when {
        leading is RowLeading.Glyph -> leading
        initial == null -> RowLeading.Glyph(HavenIcon.Key)
        else -> null
    }
    val soft = glyph?.soft == true
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(size / 4))
            .background(if (soft) colors.brassSoft else colors.avatarBg)
            .clearAndSetSemantics {},
        contentAlignment = Alignment.Center,
    ) {
        if (glyph != null) {
            IconGlyph(glyph.icon, contentDescription = null, tint = if (soft) colors.brassInk else colors.avatarFg, size = size / 2)
        } else {
            val fontSize = with(LocalDensity.current) { (size * MONOGRAM_RATIO).toSp() }
            HavenText(initial.orEmpty(), style = HavenTheme.type.monogram.copy(fontSize = fontSize, lineHeight = fontSize), color = colors.avatarFg)
        }
    }
}

/** The first character as a reader sees it (a whole emoji), upper-cased unless that changes its length ("ß"). */
internal fun monogramOf(title: String): String? {
    val text = title.trim()
    if (text.isEmpty()) return null
    val breaks = BreakIterator.getCharacterInstance().apply { setText(text) }
    val first = text.substring(0, breaks.next())
    val upper = first.uppercase()
    return if (upper.length == first.length) upper else first
}

@PreviewLightDark
@Composable
private fun ItemRowPreview() {
    KitPreview {
        InsetGroup {
            row { ItemRow("GitHub", "sam@example.com", RowLeading.Monogram("GitHub"), onClick = {}, hasPasskey = true, hasCode = true) }
            row { ItemRow("Wi-Fi at home", "Secure note", RowLeading.Glyph(HavenIcon.Note, soft = true), onClick = {}) }
            row { ItemRow("Visa ending 4242", "Card", RowLeading.Glyph(HavenIcon.Card), onClick = {}) }
        }
    }
}
```

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*StructureTest*' detekt`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/StructureTest.kt
git commit -m "feat(android): kit section header, inset group, group row and item row"
```

---
### Task 9: HavenScaffold and the glass toast

**Files:**
- Create: `ANDROID/ui/kit/Toast.kt`, `ANDROID/ui/kit/HavenScaffold.kt`
- Test: `ANDROID_TEST/ui/kit/ScaffoldTest.kt`

**Interfaces:**
- Consumes: `HavenText`, `IconGlyph`, `AddButton`, `DarkHavenColors`, `HavenSprings.toast`, `HavenMotion.fadeSpec/springSpec`.
- Produces:
  - `enum class ToastTone { Done, Alert }`; `data class ToastMessage(val text: String, val tone: ToastTone, val id: Long)`; `class ToastState { val current: ToastMessage?; fun show(text: String, tone: ToastTone = ToastTone.Done) }`; `@Composable fun rememberToastState(): ToastState`; `@Composable fun ToastHost(state: ToastState, modifier: Modifier = Modifier)`; `internal const val TOAST_MILLIS = 2_500L`.
  - `@Composable fun HavenScaffold(modifier: Modifier = Modifier, topBar: @Composable () -> Unit = {}, bottomBar: (@Composable () -> Unit)? = null, floatingButton: (@Composable () -> Unit)? = null, toastState: ToastState? = null, content: @Composable (PaddingValues) -> Unit)`; the content's bottom padding is 88dp with a floating button, else 0.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/ScaffoldTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ScaffoldTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun theSlotsStackTopContentBottom() {
        rule.setKit {
            HavenScaffold(topBar = { HavenText("Top") }, bottomBar = { HavenText("Bottom") }) { HavenText("Content") }
        }
        val top = rule.onNodeWithText("Top").getUnclippedBoundsInRoot()
        val content = rule.onNodeWithText("Content").getUnclippedBoundsInRoot()
        val bottom = rule.onNodeWithText("Bottom").getUnclippedBoundsInRoot()
        assertTrue(top.bottom <= content.top)
        assertTrue(content.bottom <= bottom.top)
    }

    @Test
    fun aFloatingButtonReservesRoomUnderTheContent() {
        rule.setKit {
            HavenScaffold(floatingButton = { AddButton(onClick = {}) }) { padding ->
                HavenText("pad ${padding.calculateBottomPadding()}")
            }
        }
        rule.onNodeWithText("pad 88.0.dp").assertExists()
        rule.onNodeWithContentDescription("New item").assertIsDisplayed()
    }

    @Test
    fun withoutOneTheContentKeepsAllItsRoom() {
        rule.setKit { HavenScaffold { padding -> HavenText("pad ${padding.calculateBottomPadding()}") } }
        rule.onNodeWithText("pad 0.0.dp").assertExists()
    }

    @Test
    fun aToastIsAnnouncedPolitelyThenLeaves() {
        rule.mainClock.autoAdvance = false
        rule.setKit {
            val toasts = rememberToastState()
            LaunchedEffect(Unit) { toasts.show("Password copied") }
            HavenScaffold(toastState = toasts) { HavenText("Content") }
        }
        rule.mainClock.advanceTimeBy(500)
        rule.onNodeWithText("Password copied")
            .assertIsDisplayed()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Polite))
        rule.mainClock.advanceTimeBy(TOAST_MILLIS + 1_000)
        rule.onNodeWithText("Password copied").assertDoesNotExist()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ScaffoldTest*'`
Expected: compile errors (`HavenScaffold`, `rememberToastState` unresolved).

- [ ] **Step 3: Write `Toast.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import net.havenkeys.android.ui.theme.DarkHavenColors
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

/** How long a toast stays. */
internal const val TOAST_MILLIS = 2_500L

enum class ToastTone { Done, Alert }

/** One toast; [id] tells two identical messages apart, so the second one shows too. */
@Immutable
data class ToastMessage(val text: String, val tone: ToastTone, val id: Long)

/**
 * The glass pill's queue of one: a new toast replaces the current one. A
 * toast never carries a secret: callers say what happened ("Password
 * copied · clears in 30 s"), never the value.
 */
@Stable
class ToastState {
    var current: ToastMessage? by mutableStateOf(null)
        private set
    private var next = 0L

    fun show(text: String, tone: ToastTone = ToastTone.Done) {
        current = ToastMessage(text, tone, next++)
    }

    internal fun expire(message: ToastMessage) {
        if (current == message) current = null
    }
}

@Composable
fun rememberToastState(): ToastState = remember { ToastState() }

/** Where toasts appear; HavenScaffold places one above its bottom edge. */
@Composable
fun ToastHost(state: ToastState, modifier: Modifier = Modifier) {
    val motion = HavenTheme.motion
    val message = state.current
    LaunchedEffect(message) {
        if (message != null) {
            delay(TOAST_MILLIS)
            state.expire(message)
        }
    }
    AnimatedContent(
        targetState = message,
        modifier = modifier,
        transitionSpec = {
            (fadeIn(motion.fadeSpec()) + slideInVertically(motion.springSpec(HavenSprings.toast)) { it / 2 }) togetherWith
                fadeOut(motion.fadeSpec())
        },
        label = "toast",
    ) { shown ->
        if (shown != null) ToastPill(shown)
    }
}

@Composable
private fun ToastPill(message: ToastMessage) {
    val colors = HavenTheme.colors
    // The glass is dark in both themes, so its glyphs take the dark theme's inks.
    val (icon, tint) = when (message.tone) {
        ToastTone.Done -> HavenIcon.Check to DarkHavenColors.ok
        ToastTone.Alert -> HavenIcon.Alert to DarkHavenColors.danger
    }
    Row(
        Modifier
            .padding(horizontal = HavenSpacing.gutter)
            .shadow(16.dp, HavenShape.pill)
            .clip(HavenShape.pill)
            .background(colors.glass)
            .padding(start = 13.dp, end = 16.dp, top = 10.dp, bottom = 10.dp)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconGlyph(icon, contentDescription = null, tint = tint, size = 18.dp)
        Spacer(Modifier.width(8.dp))
        HavenText(message.text, style = HavenTheme.type.body, color = colors.onGlass)
    }
}

@PreviewLightDark
@Composable
private fun ToastPreview() {
    KitPreview {
        ToastPill(ToastMessage("Password copied · clears in 30 s", ToastTone.Done, 0))
        ToastPill(ToastMessage("HavenKeys is offline", ToastTone.Alert, 1))
    }
}
```

- [ ] **Step 4: Write `HavenScaffold.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Room under the content for the floating button (56dp + 16dp margin + 16dp air). */
private val FloatingClearance = 88.dp

/**
 * A screen's frame on the window ground: [topBar] under the status bar,
 * the content, [bottomBar] over the navigation bar, the floating button at
 * the bottom end and toasts above the bottom edge. The keyboard pushes the
 * whole frame up. [content] receives the bottom room it must leave for the
 * floating button.
 */
@Composable
fun HavenScaffold(
    modifier: Modifier = Modifier,
    topBar: @Composable () -> Unit = {},
    bottomBar: (@Composable () -> Unit)? = null,
    floatingButton: (@Composable () -> Unit)? = null,
    toastState: ToastState? = null,
    content: @Composable (PaddingValues) -> Unit,
) {
    val clearance = if (floatingButton != null) FloatingClearance else 0.dp
    Column(modifier.fillMaxSize().background(HavenTheme.colors.pane).imePadding()) {
        Box(Modifier.fillMaxWidth().statusBarsPadding()) { topBar() }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            content(PaddingValues(bottom = clearance))
            if (floatingButton != null) {
                Box(Modifier.align(Alignment.BottomEnd).padding(HavenSpacing.gutter)) { floatingButton() }
            }
            if (toastState != null) {
                ToastHost(toastState, Modifier.align(Alignment.BottomCenter).padding(bottom = clearance + 22.dp))
            }
        }
        if (bottomBar != null) {
            Box(Modifier.fillMaxWidth().navigationBarsPadding()) { bottomBar() }
        } else {
            Spacer(Modifier.navigationBarsPadding())
        }
    }
}

@PreviewLightDark
@Composable
private fun HavenScaffoldPreview() {
    HavenTheme {
        HavenScaffold(
            topBar = { HavenText("Top bar", Modifier.padding(16.dp)) },
            bottomBar = { HavenText("Bottom bar", Modifier.padding(16.dp)) },
            floatingButton = { AddButton(onClick = {}) },
        ) { padding -> HavenText("Content", Modifier.padding(padding).padding(16.dp)) }
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ScaffoldTest*' detekt`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/ScaffoldTest.kt
git commit -m "feat(android): kit scaffold and the glass toast"
```

---

### Task 10: Inputs I: switch, toggle row, slider, segmented control

**Files:**
- Create: `ANDROID/ui/kit/HavenSwitch.kt` (with `ToggleRow`), `HavenSlider.kt`, `SegmentedControl.kt`
- Test: `ANDROID_TEST/ui/kit/ChoicesTest.kt`

**Interfaces:**
- Consumes: `GroupRowText`, `HavenPress`, `DISABLED_ALPHA`, `HavenRadius`, `HavenSpacing`, `HavenTheme.motion.springSpec/fadeSpec`.
- Produces:
  - `@Composable fun HavenSwitch(checked: Boolean, onCheckedChange: ((Boolean) -> Unit)?, modifier: Modifier = Modifier, label: String? = null, enabled: Boolean = true)` (null `onCheckedChange`: drawn only, the row toggles).
  - `@Composable fun ToggleRow(title: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, detail: String? = null, enabled: Boolean = true)`.
  - `@Composable fun HavenSlider(value: Float, onValueChange: (Float) -> Unit, valueRange: ClosedFloatingPointRange<Float>, label: String, modifier: Modifier = Modifier, steps: Int = 0, valueText: String? = null, enabled: Boolean = true)`; `internal fun snapToStep(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int): Float`.
  - `@Composable fun SegmentedControl(options: List<String>, selectedIndex: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier)`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/ChoicesTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertRangeInfoEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChoicesTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aSwitchIsASwitchWithItsState() {
        var last = false
        rule.setKit {
            var checked by remember { mutableStateOf(false) }
            HavenSwitch(checked, onCheckedChange = { checked = it; last = it }, label = "Biometrics")
        }
        rule.onNodeWithContentDescription("Biometrics")
            .assert(hasRole(Role.Switch)).assertIsOff().assertTouchTarget()
            .performClick().assertIsOn()
        assertTrue(last)
    }

    @Test
    fun aToggleRowTogglesFromAnywhereOnTheRow() {
        rule.setKit {
            var checked by remember { mutableStateOf(true) }
            ToggleRow("Lock when the screen turns off", checked, { checked = it }, detail = "Recommended")
        }
        rule.onNodeWithText("Lock when the screen turns off")
            .assert(hasRole(Role.Switch)).assertIsOn().assertTouchTarget()
            .performClick().assertIsOff()
    }

    @Test
    fun aSliderTakesTalkBacksSetProgressSnappedToItsSteps() {
        var last = 20f
        rule.setKit {
            var length by remember { mutableFloatStateOf(20f) }
            HavenSlider(length, { length = it; last = it }, 8f..64f, label = "Length", steps = 55, valueText = "${length.roundToInt()}")
        }
        val slider = rule.onNodeWithContentDescription("Length")
        slider.assertRangeInfoEquals(ProgressBarRangeInfo(20f, 8f..64f, 55))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "20"))
            .assertHeightIsAtLeast(48.dp)
        slider.performSemanticsAction(SemanticsActions.SetProgress) { it(30.4f) }
        assertEquals(30f, last, 0.001f)
        slider.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "30"))
    }

    @Test
    fun tappingNearTheEndPicksAValueNearTheEnd() {
        var last = 20f
        rule.setKit { HavenSlider(20f, { last = it }, 8f..64f, label = "Length", steps = 55) }
        rule.onNodeWithContentDescription("Length").performTouchInput { click(Offset(width - 2f, centerY)) }
        assertTrue("picked $last", last > 60f)
    }

    @Test
    fun stepsSnapToTheNearestStep() {
        assertEquals(30f, snapToStep(30.4f, 8f..64f, 55), 0f)
        assertEquals(64f, snapToStep(99f, 8f..64f, 55), 0f)
        assertEquals(0.37f, snapToStep(0.37f, 0f..1f, 0), 0f)
    }

    @Test
    fun segmentsAreTabsWithOneSelected() {
        var picked = -1
        rule.setKit {
            var index by remember { mutableIntStateOf(0) }
            SegmentedControl(listOf("Random", "Words"), index, { index = it; picked = it })
        }
        rule.onNodeWithText("Random").assert(hasRole(Role.Tab)).assertIsSelected().assertHeightIsAtLeast(48.dp)
        rule.onNodeWithText("Words").assertIsNotSelected().performClick().assertIsSelected()
        assertEquals(1, picked)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ChoicesTest*'`
Expected: compile errors (`HavenSwitch`, `HavenSlider`, … unresolved).

- [ ] **Step 3: Write `HavenSwitch.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.toggleable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The desktop's switch at phone size: line-strong track off, brass on, a
 * paper-white thumb. With [onCheckedChange] it is its own control (give it
 * a [label]); with null it is drawn only, inside a [ToggleRow] that toggles.
 */
@Composable
fun HavenSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    label: String? = null,
    enabled: Boolean = true,
) {
    val colors = HavenTheme.colors
    val motion = HavenTheme.motion
    val position by animateFloatAsState(if (checked) 1f else 0f, motion.springSpec(HavenSprings.press), label = "thumb")
    val track by animateColorAsState(if (checked) colors.brass else colors.lineStrong, motion.fadeSpec(), label = "track")
    val control = if (onCheckedChange == null) {
        Modifier
    } else {
        Modifier
            .toggleable(
                value = checked,
                interactionSource = null,
                indication = null,
                enabled = enabled,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .semantics { if (label != null) contentDescription = label }
    }
    Box(
        modifier
            .then(control)
            .defaultMinSize(minWidth = 52.dp, minHeight = HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(width = 44.dp, height = 26.dp)) {
            val radius = size.height / 2
            drawRoundRect(track, cornerRadius = CornerRadius(radius))
            val thumb = radius - 3.dp.toPx()
            val x = radius + (size.width - 2 * radius) * position
            drawCircle(Color.Black.copy(alpha = 0.25f), thumb, Offset(x, radius + 1.dp.toPx()))
            drawCircle(colors.thumb, thumb, Offset(x, radius))
        }
    }
}

/** A settings row with a switch: the whole row is the switch, named by its title. */
@Composable
fun ToggleRow(
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    detail: String? = null,
    enabled: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .toggleable(
                value = checked,
                interactionSource = null,
                indication = HavenPress,
                enabled = enabled,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .padding(start = HavenSpacing.rowX, end = 8.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) { GroupRowText(title, detail) }
        HavenSwitch(checked, onCheckedChange = null, enabled = enabled)
    }
}

@PreviewLightDark
@Composable
private fun HavenSwitchPreview() {
    KitPreview {
        InsetGroup {
            row { ToggleRow("Unlock with biometrics", checked = true, onCheckedChange = {}) }
            row { ToggleRow("Confirm before filling", checked = false, onCheckedChange = {}, detail = "Ask before each fill") }
        }
    }
}
```

- [ ] **Step 4: Write `HavenSlider.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

private val Thumb = 24.dp

/**
 * The desktop's slider: brass fill on a line-strong track, a paper-white
 * thumb. Tap or drag anywhere on its 48dp height; TalkBack adjusts it with
 * its own gestures (setProgress), snapped to [steps]. [valueText] is what
 * TalkBack reads as the value (a length reads "24", not a percentage).
 */
@Composable
fun HavenSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    valueRange: ClosedFloatingPointRange<Float>,
    label: String,
    modifier: Modifier = Modifier,
    steps: Int = 0,
    valueText: String? = null,
    enabled: Boolean = true,
) {
    val colors = HavenTheme.colors
    val latest by rememberUpdatedState(onValueChange)
    val thumbPx = with(LocalDensity.current) { Thumb.toPx() }
    var trackPx by remember { mutableFloatStateOf(1f) }
    val span = valueRange.endInclusive - valueRange.start
    fun valueAt(x: Float): Float {
        val fraction = ((x - thumbPx / 2) / trackPx).coerceIn(0f, 1f)
        return snapToStep(valueRange.start + fraction * span, valueRange, steps)
    }
    val fraction = if (span > 0f) ((value - valueRange.start) / span).coerceIn(0f, 1f) else 0f
    Box(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .onSizeChanged { trackPx = (it.width - thumbPx).coerceAtLeast(1f) }
            .pointerInput(enabled, valueRange, steps) {
                if (enabled) detectTapGestures { latest(valueAt(it.x)) }
            }
            .pointerInput(enabled, valueRange, steps) {
                if (enabled) {
                    detectHorizontalDragGestures { change, _ ->
                        change.consume()
                        latest(valueAt(change.position.x))
                    }
                }
            }
            .semantics {
                contentDescription = label
                if (valueText != null) stateDescription = valueText
                progressBarRangeInfo = ProgressBarRangeInfo(value, valueRange, steps)
                if (enabled) {
                    setProgress { target ->
                        latest(snapToStep(target, valueRange, steps))
                        true
                    }
                } else {
                    disabled()
                }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Canvas(Modifier.fillMaxWidth().height(Thumb)) {
            val y = size.height / 2
            val start = thumbPx / 2
            val end = size.width - thumbPx / 2
            val x = start + (end - start) * fraction
            val stroke = 4.dp.toPx()
            drawLine(colors.lineStrong, Offset(start, y), Offset(end, y), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(colors.brass, Offset(start, y), Offset(x, y), strokeWidth = stroke, cap = StrokeCap.Round)
            drawCircle(Color.Black.copy(alpha = 0.3f), thumbPx / 2, Offset(x, y + 1.dp.toPx()))
            drawCircle(colors.thumb, thumbPx / 2, Offset(x, y))
        }
    }
}

/** [value] in [range], on the nearest of [steps] evenly spaced stops (0: continuous). */
internal fun snapToStep(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int): Float {
    val clamped = value.coerceIn(range)
    if (steps <= 0) return clamped
    val step = (range.endInclusive - range.start) / (steps + 1)
    return (range.start + ((clamped - range.start) / step).roundToInt() * step).coerceIn(range)
}

@PreviewLightDark
@Composable
private fun HavenSliderPreview() {
    KitPreview { HavenSlider(24f, {}, 8f..64f, label = "Length", steps = 55, valueText = "24") }
}
```

- [ ] **Step 5: Write `SegmentedControl.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenRadius
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The desktop's segmented control: a field track with the chosen segment
 * raised on a hairline. Each segment is a tab for TalkBack and a full 48dp
 * target; the track is drawn inset inside that height. In light theme the
 * track is the hover green (the field is white there, like the pane).
 */
@Composable
fun SegmentedControl(options: List<String>, selectedIndex: Int, onSelect: (Int) -> Unit, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val track = if (colors.isDark) colors.field else colors.hover
    Row(
        modifier
            .fillMaxWidth()
            .selectableGroup()
            .drawBehind {
                val inset = 4.dp.toPx()
                drawRoundRect(
                    color = track,
                    topLeft = Offset(0f, inset),
                    size = Size(size.width, size.height - 2 * inset),
                    cornerRadius = CornerRadius(HavenRadius.row.toPx()),
                )
            }
            .padding(horizontal = 3.dp),
    ) {
        options.forEachIndexed { index, option ->
            val selected = index == selectedIndex
            val thumb by animateFloatAsState(if (selected) 1f else 0f, HavenTheme.motion.fadeSpec(), label = "segment")
            Box(
                Modifier
                    .weight(1f)
                    .heightIn(min = HavenSpacing.touch)
                    .selectable(selected = selected, interactionSource = null, indication = null, role = Role.Tab) { onSelect(index) }
                    .drawBehind {
                        if (thumb > 0f) {
                            val inset = 7.dp.toPx()
                            val topLeft = Offset(0f, inset)
                            val area = Size(size.width, size.height - 2 * inset)
                            val corner = CornerRadius(HavenRadius.control.toPx())
                            drawRoundRect(colors.raised, topLeft, area, corner, alpha = thumb)
                            drawRoundRect(colors.lineStrong, topLeft, area, corner, style = Stroke(1.dp.toPx()), alpha = thumb)
                        }
                    }
                    .padding(horizontal = 10.dp, vertical = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                HavenText(option, style = HavenTheme.type.label, color = if (selected) colors.textStrong else colors.muted)
            }
        }
    }
}

@PreviewLightDark
@Composable
private fun SegmentedControlPreview() {
    KitPreview { SegmentedControl(listOf("Random", "Words", "PIN"), selectedIndex = 1, onSelect = {}) }
}
```

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ChoicesTest*' detekt`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/ChoicesTest.kt
git commit -m "feat(android): kit switch, toggle row, slider and segmented control"
```

---

### Task 11: Inputs II: text field and secret field

**Files:**
- Create: `ANDROID/ui/kit/HavenTextField.kt` (with the shared `FieldRow` and `KitTextInput`), `ANDROID/ui/kit/SecretTextField.kt`
- Test: `ANDROID_TEST/ui/kit/TextFieldsTest.kt`

**Interfaces:**
- Consumes: `NoPersonalizedLearning` (Task 5), `HavenIconButton`, `HavenIcon.Eye/EyeOff`, strings `R.string.reveal` ("Show %1$s") and `R.string.hide` ("Hide %1$s").
- Produces:
  - `@Composable fun HavenTextField(state: TextFieldState, label: String, modifier: Modifier = Modifier, placeholder: String? = null, error: String? = null, enabled: Boolean = true, keyboardOptions: KeyboardOptions = KeyboardOptions.Default, onKeyboardAction: KeyboardActionHandler? = null, lineLimits: TextFieldLineLimits = TextFieldLineLimits.SingleLine, inputTransformation: InputTransformation? = null)`.
  - `@Composable fun SecretTextField(state: TextFieldState, label: String, revealed: Boolean, onRevealChange: (Boolean) -> Unit, modifier: Modifier = Modifier, error: String? = null, enabled: Boolean = true, imeAction: ImeAction = ImeAction.Done, onKeyboardAction: KeyboardActionHandler? = null)` (the caller owns `revealed`, so its lock wipe can reset it).
  - `@Composable internal fun FieldRow(label: String, error: String?, focused: Boolean, trailing: (@Composable () -> Unit)? = null, field: @Composable () -> Unit)`; `@Composable internal fun KitTextInput(content: @Composable () -> Unit)`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/TextFieldsTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class TextFieldsTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun typingFillsTheStateAndTheLabelNamesTheFieldOnce() {
        val state = TextFieldState()
        rule.setKit { HavenTextField(state, label = "Username") }
        val field = rule.onNode(hasSetTextAction())
        field.assertContentDescriptionEquals("Username").assertHeightIsAtLeast(48.dp)
        field.performTextInput("sam")
        rule.runOnIdle { assertEquals("sam", state.text.toString()) }
        // The visible label is not a second TalkBack stop.
        rule.onAllNodesWithText("Username").assertCountEquals(0)
    }

    @Test
    fun anErrorIsAnnouncedAsTheFieldsError() {
        rule.setKit { HavenTextField(TextFieldState("http://vault"), label = "Server", error = "Use https://") }
        rule.onNode(hasSetTextAction()).assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Use https://"))
    }

    @Test
    fun anOrdinaryFieldIsNotAPasswordField() {
        rule.setKit { HavenTextField(TextFieldState(), label = "Title") }
        rule.onNode(hasSetTextAction()).assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Password))
    }

    @Test
    fun aSecretIsAPasswordFieldNamedByItsLabel() {
        val state = TextFieldState()
        rule.setKit {
            var shown by remember { mutableStateOf(false) }
            SecretTextField(state, "Master password", revealed = shown, onRevealChange = { shown = it })
        }
        val field = rule.onNode(hasSetTextAction())
        field.performTextInput("hunter2")
        field.assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
            .assertContentDescriptionEquals("Master password")
            .assertHeightIsAtLeast(48.dp)
        rule.runOnIdle { assertEquals("hunter2", state.text.toString()) }
    }

    @Test
    fun theEyeShowsAndHidesAndSaysWhich() {
        var revealed = false
        rule.setKit {
            var shown by remember { mutableStateOf(false) }
            SecretTextField(TextFieldState("hunter2"), "Master password", revealed = shown, onRevealChange = { shown = it; revealed = it })
        }
        rule.onNodeWithContentDescription("Show Master password").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(true, revealed)
        rule.onNodeWithContentDescription("Hide Master password").assertExists()
        rule.onNodeWithContentDescription("Show Master password").assertDoesNotExist()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*TextFieldsTest*'`
Expected: compile errors (`HavenTextField`, `SecretTextField` unresolved).

- [ ] **Step 3: Write `HavenTextField.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A text field the desktop way: the row is the field (no box, no border of
 * its own). A muted label above the value; on focus the row takes the
 * selection wash and an inset brass ring. Put it in an [InsetGroup] row.
 * TalkBack reads the label as the field's name, once; an error is the
 * field's error. The keyboard is asked not to learn what is typed.
 */
@Composable
fun HavenTextField(
    state: TextFieldState,
    label: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    error: String? = null,
    enabled: Boolean = true,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    onKeyboardAction: KeyboardActionHandler? = null,
    lineLimits: TextFieldLineLimits = TextFieldLineLimits.SingleLine,
    inputTransformation: InputTransformation? = null,
) {
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val colors = HavenTheme.colors
    KitTextInput {
        BasicTextField(
            state = state,
            modifier = modifier.fillMaxWidth().semantics {
                contentDescription = label
                if (error != null) error(error)
            },
            enabled = enabled,
            inputTransformation = inputTransformation,
            textStyle = HavenTheme.type.value.copy(color = colors.textStrong),
            keyboardOptions = keyboardOptions,
            onKeyboardAction = onKeyboardAction,
            lineLimits = lineLimits,
            interactionSource = interaction,
            cursorBrush = SolidColor(colors.brass),
            decorator = TextFieldDecorator { field ->
                FieldRow(label, error, focused) {
                    Box {
                        if (placeholder != null && state.text.isEmpty()) {
                            HavenText(placeholder, Modifier.clearAndSetSemantics {}, style = HavenTheme.type.value, color = colors.muted)
                        }
                        field()
                    }
                }
            },
        )
    }
}

/** The row every kit field draws: label, value, error, optional trailing control. */
@Composable
internal fun FieldRow(
    label: String,
    error: String?,
    focused: Boolean,
    trailing: (@Composable () -> Unit)? = null,
    field: @Composable () -> Unit,
) {
    val colors = HavenTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .background(if (focused) colors.sel else Color.Transparent)
            .then(if (focused) Modifier.border(1.5.dp, colors.brass) else Modifier)
            .padding(start = HavenSpacing.rowX, end = if (trailing != null) 4.dp else HavenSpacing.rowX, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            HavenText(
                label,
                Modifier.clearAndSetSemantics {},
                style = HavenTheme.type.label,
                color = if (error != null) colors.danger else colors.muted,
            )
            field()
            if (error != null) {
                HavenText(error, Modifier.clearAndSetSemantics {}, style = HavenTheme.type.label, color = colors.danger)
            }
        }
        trailing?.invoke()
    }
}

/** Brass selection handles, and no learning by the keyboard. */
@Composable
internal fun KitTextInput(content: @Composable () -> Unit) {
    val colors = HavenTheme.colors
    val selection = TextSelectionColors(handleColor = colors.brass, backgroundColor = colors.brass.copy(alpha = 0.32f))
    CompositionLocalProvider(LocalTextSelectionColors provides selection) {
        NoPersonalizedLearning(content)
    }
}

@PreviewLightDark
@Composable
private fun HavenTextFieldPreview() {
    KitPreview {
        InsetGroup {
            row { HavenTextField(rememberTextFieldState("sam@example.com"), "Username") }
            row { HavenTextField(rememberTextFieldState(), "Website", placeholder = "example.com") }
            row { HavenTextField(rememberTextFieldState("http://vault"), "Server", error = "The address must start with https://") }
        }
    }
}
```

- [ ] **Step 4: Write `SecretTextField.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.BasicSecureTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.TextObfuscationMode
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.tooling.preview.PreviewLightDark
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A secret field: mono, fully masked (no last-character flash) until the
 * user reveals it with the eye. The caller owns [revealed], so its lock
 * wipe resets it. Password semantics, password keyboard, no autocorrect,
 * no learning, no cut or copy; TalkBack names it by [label] and the eye by
 * "Show"/"Hide" and the label, never by the value.
 */
@Composable
fun SecretTextField(
    state: TextFieldState,
    label: String,
    revealed: Boolean,
    onRevealChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    error: String? = null,
    enabled: Boolean = true,
    imeAction: ImeAction = ImeAction.Done,
    onKeyboardAction: KeyboardActionHandler? = null,
) {
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val colors = HavenTheme.colors
    val eyeLabel = stringResource(if (revealed) R.string.hide else R.string.reveal, label)
    KitTextInput {
        BasicSecureTextField(
            state = state,
            modifier = modifier.fillMaxWidth().semantics {
                contentDescription = label
                if (error != null) error(error)
            },
            enabled = enabled,
            textStyle = HavenTheme.type.secret.copy(color = colors.textStrong),
            keyboardOptions = KeyboardOptions(imeAction = imeAction),
            onKeyboardAction = onKeyboardAction,
            interactionSource = interaction,
            cursorBrush = SolidColor(colors.brass),
            textObfuscationMode = if (revealed) TextObfuscationMode.Visible else TextObfuscationMode.Hidden,
            decorator = TextFieldDecorator { field ->
                FieldRow(
                    label = label,
                    error = error,
                    focused = focused,
                    trailing = {
                        HavenIconButton(
                            if (revealed) HavenIcon.EyeOff else HavenIcon.Eye,
                            eyeLabel,
                            onClick = { onRevealChange(!revealed) },
                        )
                    },
                    field = field,
                )
            },
        )
    }
}

@PreviewLightDark
@Composable
private fun SecretTextFieldPreview() {
    KitPreview {
        InsetGroup {
            row { SecretTextField(rememberTextFieldState("hunter2hunter2"), "Password", revealed = false, onRevealChange = {}) }
            row { SecretTextField(rememberTextFieldState("hunter2hunter2"), "Password", revealed = true, onRevealChange = {}) }
        }
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*TextFieldsTest*' detekt`
Expected: PASS. If `assertContentDescriptionEquals("Master password")` fails because the merged node also carries the eye button's description, the eye button is merging into the field: make sure it is a `HavenIconButton` (its `clickable` is a merge boundary), not a bare glyph.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/TextFieldsTest.kt
git commit -m "feat(android): kit text field and secret field, the row is the field"
```

---
### Task 12: Overlays I: sheet and dialog

**Files:**
- Create: `ANDROID/ui/kit/HavenSheet.kt`, `ANDROID/ui/kit/HavenDialog.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml` (`kit_close`)
- Test: `ANDROID_TEST/ui/kit/OverlaysTest.kt`

**Interfaces:**
- Consumes: `SecureDialogWindow(ignoreObscuredTouches: Boolean)` (existing, `ui/components/SecureDialogWindow.kt`), `HavenButton`, `ButtonStyle`, `HavenSprings.sheet`, `HavenShape.sheet/dialog`.
- Produces:
  - `@Composable fun HavenSheet(onDismiss: () -> Unit, modifier: Modifier = Modifier, title: String? = null, content: @Composable ColumnScope.() -> Unit)`; `@Composable internal fun SheetSurface(title: String?, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit)`; `internal const val SHEET_TAG = "haven-sheet"`.
  - `class DialogAction(val label: String, val onClick: () -> Unit, val danger: Boolean = false)`; `@Composable fun HavenDialog(title: String, onDismiss: () -> Unit, confirm: DialogAction, modifier: Modifier = Modifier, message: String? = null, dismiss: DialogAction? = null)`; `@Composable internal fun DialogSurface(title: String, confirm: DialogAction, modifier: Modifier = Modifier, message: String? = null, dismiss: DialogAction? = null)`.
  - String `kit_close`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/OverlaysTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class OverlaysTest {
    @get:Rule
    val rule = createComposeRule()

    private var dismissed = 0

    private fun showSheet() = rule.setKit {
        HavenSheet(onDismiss = { dismissed++ }, title = "New item") { HavenText("Login") }
    }

    @Test
    fun aSheetIsADialogWithItsTitleAndClosesFromTheBackdrop() {
        showSheet()
        rule.onNode(isDialog()).assertExists()
        rule.onNodeWithText("New item").assert(isHeading())
        rule.onNodeWithText("Login").assertIsDisplayed()
        rule.onNodeWithContentDescription("Close").assert(hasRole(Role.Button)).performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun draggingTheSheetDownDismissesIt() {
        showSheet()
        rule.onNodeWithTag(SHEET_TAG).performTouchInput { swipeDown(startY = top + 1f, endY = bottom + height) }
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun underRemoveAnimationsTheSheetClosesAtOnce() {
        removeAnimations()
        rule.mainClock.autoAdvance = false
        showSheet()
        repeat(3) { rule.mainClock.advanceTimeByFrame() }
        rule.onNodeWithContentDescription("Close").performSemanticsAction(SemanticsActions.OnClick)
        repeat(3) { rule.mainClock.advanceTimeByFrame() }
        assertEquals(1, dismissed)
    }

    @Test
    fun aTapWhileTheSheetIsStillOpeningClosesItOnce() {
        rule.mainClock.autoAdvance = false
        showSheet()
        rule.mainClock.advanceTimeBy(48)
        rule.onNodeWithContentDescription("Close").performSemanticsAction(SemanticsActions.OnClick)
        rule.mainClock.autoAdvance = true
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun aDialogNamesItselfAndAnswersOnce() {
        var removed = 0
        rule.setKit {
            HavenDialog(
                title = "Remove this device?",
                onDismiss = { dismissed++ },
                confirm = DialogAction("Remove", { removed++ }, danger = true),
                message = "Its local copy of the vault is erased.",
                dismiss = DialogAction("Cancel", { dismissed++ }),
            )
        }
        rule.onNode(isDialog()).assertExists()
        rule.onNodeWithText("Remove this device?").assert(isHeading())
        val remove = rule.onNodeWithText("Remove")
        remove.assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        remove.performClick()
        assertEquals(1, removed)
        rule.onNodeWithText("Cancel").assertIsNotEnabled()
        assertEquals(0, dismissed)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*OverlaysTest*'`
Expected: compile errors (`HavenSheet`, `HavenDialog`, … unresolved).

- [ ] **Step 3: Add the string**

`values/strings.xml`, in the ui/kit block: `<string name="kit_close">Close</string>`; `values-pt-rBR/strings.xml`: `<string name="kit_close">Fechar</string>`.

- [ ] **Step 4: Write `HavenSheet.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.AnchoredDraggableDefaults
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.gestures.animateTo
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.compose.ui.window.SecureFlagPolicy
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.dropWhile
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

private enum class SheetValue { Hidden, Expanded }

internal const val SHEET_TAG = "haven-sheet"

/** Released past this share of its height, a dragged sheet closes. */
private const val DRAG_DISMISS_FRACTION = 0.4f

/**
 * A bottom sheet (spec §7: springs up, the backdrop dims, drag down or tap
 * outside to close). It lives in a window of its own that sets FLAG_SECURE,
 * drops taps that pass through another app's overlay and is excluded from
 * autofill. [onDismiss] is called once, after the sheet has gone; a close
 * tapped while it is still rising goes straight to closing.
 */
@Composable
fun HavenSheet(
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    title: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val state = remember { AnchoredDraggableState(SheetValue.Hidden) }
    val scope = rememberCoroutineScope()
    val motion = HavenTheme.motion
    val dismiss by rememberUpdatedState(onDismiss)
    var closing by remember { mutableStateOf(false) }
    val close: () -> Unit = {
        if (!closing) {
            closing = true
            scope.launch {
                state.animateTo(SheetValue.Hidden, motion.springSpec(HavenSprings.sheet))
                dismiss()
            }
        }
    }
    val draggedAway: () -> Unit = {
        if (!closing) {
            closing = true
            dismiss()
        }
    }
    Dialog(
        onDismissRequest = close,
        properties = DialogProperties(
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
            securePolicy = SecureFlagPolicy.SecureOn,
        ),
    ) {
        SecureDialogWindow(ignoreObscuredTouches = true)
        NoWindowDim()
        OpenWhenMeasured(state)
        CloseWhenDraggedAway(state, draggedAway)
        SheetFrame(state, close, modifier, title, content)
    }
}

@Composable
private fun SheetFrame(
    state: AnchoredDraggableState<SheetValue>,
    close: () -> Unit,
    modifier: Modifier,
    title: String?,
    content: @Composable ColumnScope.() -> Unit,
) {
    val closeLabel = stringResource(R.string.kit_close)
    val motion = HavenTheme.motion
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier
                .fillMaxSize()
                .graphicsLayer { alpha = shownFraction(state) }
                .background(HavenTheme.colors.scrim)
                .clickable(interactionSource = null, indication = null, onClickLabel = closeLabel, role = Role.Button, onClick = close)
                .semantics { contentDescription = closeLabel },
        )
        SheetSurface(
            title,
            modifier
                .align(Alignment.BottomCenter)
                .onSizeChanged { size ->
                    state.updateAnchors(
                        DraggableAnchors {
                            SheetValue.Hidden at size.height.toFloat()
                            SheetValue.Expanded at 0f
                        },
                    )
                }
                .offset { IntOffset(0, state.offset.takeUnless { it.isNaN() }?.roundToInt() ?: 0) }
                .graphicsLayer { alpha = if (state.offset.isNaN()) 0f else 1f }
                .anchoredDraggable(
                    state = state,
                    orientation = Orientation.Vertical,
                    flingBehavior = AnchoredDraggableDefaults.flingBehavior(
                        state = state,
                        positionalThreshold = { distance -> distance * DRAG_DISMISS_FRACTION },
                        animationSpec = motion.springSpec(HavenSprings.sheet),
                    ),
                )
                .testTag(SHEET_TAG),
            content,
        )
    }
}

/** The sheet as it draws, without its window: the catalogue shows it inline. */
@Composable
internal fun SheetSurface(title: String?, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val colors = HavenTheme.colors
    Column(
        modifier
            .fillMaxWidth()
            .shadow(24.dp, HavenShape.sheet)
            .clip(HavenShape.sheet)
            .background(colors.raised)
            .navigationBarsPadding()
            .padding(start = HavenSpacing.gutter, end = HavenSpacing.gutter, bottom = HavenSpacing.gutter)
            .semantics { if (title != null) paneTitle = title },
    ) {
        Box(
            Modifier
                .align(Alignment.CenterHorizontally)
                .padding(vertical = 10.dp)
                .size(width = 36.dp, height = 5.dp)
                .clip(HavenShape.pill)
                .background(colors.lineStrong),
        )
        if (title != null) {
            HavenText(
                title,
                Modifier.padding(top = 4.dp, bottom = 12.dp).semantics { heading() },
                style = HavenTheme.type.title,
                color = colors.textStrong,
            )
        }
        content()
    }
}

private fun shownFraction(state: AnchoredDraggableState<SheetValue>): Float {
    val hidden = state.anchors.positionOf(SheetValue.Hidden)
    val offset = state.offset
    if (hidden.isNaN() || offset.isNaN() || hidden <= 0f) return 0f
    return (1f - offset / hidden).coerceIn(0f, 1f)
}

/** Rises once its height is known (the anchors need it). */
@Composable
private fun OpenWhenMeasured(state: AnchoredDraggableState<SheetValue>) {
    val motion = HavenTheme.motion
    LaunchedEffect(state) {
        snapshotFlow { state.anchors.size }.first { it > 0 }
        state.animateTo(SheetValue.Expanded, motion.springSpec(HavenSprings.sheet))
    }
}

/** Opened, then settled hidden again without a close: the user dragged it away. */
@Composable
private fun CloseWhenDraggedAway(state: AnchoredDraggableState<SheetValue>, onGone: () -> Unit) {
    val latest by rememberUpdatedState(onGone)
    LaunchedEffect(state) {
        snapshotFlow { state.settledValue }
            .dropWhile { it == SheetValue.Hidden }
            .first { it == SheetValue.Hidden }
        latest()
    }
}

/** The sheet draws its own animated scrim; the window's fixed dim would double it. */
@Composable
private fun NoWindowDim() {
    val view = LocalView.current
    SideEffect { (view.parent as? DialogWindowProvider)?.window?.setDimAmount(0f) }
}

@PreviewLightDark
@Composable
private fun HavenSheetPreview() {
    KitPreview {
        SheetSurface("New item") {
            HavenText("Login")
            HavenText("Secure note")
        }
    }
}
```

- [ ] **Step 5: Write `HavenDialog.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.SecureFlagPolicy
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

/** A dialog's button: its label, what it does, and whether it destroys something. */
class DialogAction(val label: String, val onClick: () -> Unit, val danger: Boolean = false)

/**
 * A question that needs an answer. Its window sets FLAG_SECURE, drops taps
 * through another app's overlay and is excluded from autofill. The first
 * tap on either button answers; a second tap does nothing.
 */
@Composable
fun HavenDialog(
    title: String,
    onDismiss: () -> Unit,
    confirm: DialogAction,
    modifier: Modifier = Modifier,
    message: String? = null,
    dismiss: DialogAction? = null,
) {
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(securePolicy = SecureFlagPolicy.SecureOn)) {
        SecureDialogWindow(ignoreObscuredTouches = true)
        DialogSurface(title, confirm, modifier, message, dismiss)
    }
}

/** The dialog as it draws, without its window: the catalogue shows it inline. */
@Composable
internal fun DialogSurface(
    title: String,
    confirm: DialogAction,
    modifier: Modifier = Modifier,
    message: String? = null,
    dismiss: DialogAction? = null,
) {
    val colors = HavenTheme.colors
    var answered by remember { mutableStateOf(false) }
    fun once(action: () -> Unit): () -> Unit = {
        if (!answered) {
            answered = true
            action()
        }
    }
    Column(
        modifier
            .widthIn(max = 360.dp)
            .fillMaxWidth()
            .shadow(24.dp, HavenShape.dialog)
            .clip(HavenShape.dialog)
            .background(colors.raised)
            .padding(24.dp)
            .semantics { paneTitle = title },
    ) {
        HavenText(title, Modifier.semantics { heading() }, style = HavenTheme.type.title, color = colors.textStrong)
        if (message != null) {
            HavenText(message, Modifier.padding(top = 8.dp), style = HavenTheme.type.body, color = colors.text)
        }
        Row(Modifier.align(Alignment.End).padding(top = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (dismiss != null) {
                HavenButton(dismiss.label, once(dismiss.onClick), style = ButtonStyle.Quiet, enabled = !answered)
            }
            HavenButton(
                confirm.label,
                once(confirm.onClick),
                style = if (confirm.danger) ButtonStyle.Danger else ButtonStyle.Primary,
                enabled = !answered,
            )
        }
    }
}

@PreviewLightDark
@Composable
private fun HavenDialogPreview() {
    KitPreview {
        DialogSurface(
            title = "Remove this device?",
            confirm = DialogAction("Remove", {}, danger = true),
            message = "Its local copy of the vault is erased.",
            dismiss = DialogAction("Cancel", {}),
        )
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*OverlaysTest*' detekt lintGithubDebug`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/OverlaysTest.kt
git commit -m "feat(android): kit sheet with spring and drag to dismiss, and dialog, in secure windows"
```

---

### Task 13: Menu, progress ring, pull to refresh

**Files:**
- Create: `ANDROID/ui/kit/HavenMenu.kt`, `ProgressRing.kt`, `PullToRefresh.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml` (`kit_refresh`, `kit_refreshing`)
- Test: `ANDROID_TEST/ui/kit/MenuAndFeedbackTest.kt`

**Interfaces:**
- Consumes: `SecureDialogWindow`, `HavenPress`, `HavenSprings.sheet/smooth`, `HavenMotion.tickSpec/fadeSpec`.
- Produces:
  - `class MenuItem(val label: String, val onClick: () -> Unit, val icon: HavenIcon? = null, val danger: Boolean = false)`; `@Composable fun HavenMenu(expanded: Boolean, onDismiss: () -> Unit, items: List<MenuItem>, modifier: Modifier = Modifier)` (place it in a `Box` with its trigger; it opens below the trigger, end-aligned, above it when there is no room); `@Composable internal fun MenuSurface(items: List<MenuItem>, onDismiss: () -> Unit, modifier: Modifier = Modifier)`.
  - `@Composable fun ProgressRing(progress: Float?, modifier: Modifier = Modifier, size: Dp = 28.dp, warn: Boolean = false, contentDescription: String? = null)` (null progress: indeterminate).
  - `@Composable fun PullToRefresh(refreshing: Boolean, onRefresh: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit)`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/kit/MenuAndFeedbackTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertRangeInfoEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class MenuAndFeedbackTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aMenuItemIsAButtonThatClosesTheMenuAndActs() {
        var edits = 0
        var closes = 0
        rule.setKit {
            Box {
                HavenIconButton(HavenIcon.More, "More", onClick = {})
                HavenMenu(
                    expanded = true,
                    onDismiss = { closes++ },
                    items = listOf(MenuItem("Edit", { edits++ }, HavenIcon.Edit), MenuItem("Delete", {}, HavenIcon.Trash, danger = true)),
                )
            }
        }
        rule.onNodeWithText("Delete").assert(hasRole(Role.Button)).assertTouchTarget()
        rule.onNodeWithText("Edit").assert(hasRole(Role.Button)).performClick()
        assertEquals(1, edits)
        assertEquals(1, closes)
    }

    @Test
    fun aClosedMenuShowsNothing() {
        rule.setKit { HavenMenu(expanded = false, onDismiss = {}, items = listOf(MenuItem("Edit", {}))) }
        rule.onNodeWithText("Edit").assertDoesNotExist()
    }

    @Test
    fun aRingReportsItsProgressOrThatItIsWorking() {
        rule.setKit {
            Box {
                ProgressRing(0.25f, contentDescription = "8 seconds left")
                ProgressRing(null, contentDescription = "Loading")
            }
        }
        rule.onNodeWithContentDescription("8 seconds left").assertRangeInfoEquals(ProgressBarRangeInfo(0.25f, 0f..1f))
        rule.onNodeWithContentDescription("Loading").assertRangeInfoEquals(ProgressBarRangeInfo.Indeterminate)
    }

    @Composable
    private fun Rows() {
        LazyColumn(Modifier.fillMaxSize().testTag("list")) {
            items(30) { HavenText("Row $it", Modifier.height(48.dp)) }
        }
    }

    @Test
    fun whileRefreshingTheRingSaysSo() {
        rule.setKit { PullToRefresh(refreshing = true, onRefresh = {}) { Rows() } }
        rule.onNodeWithContentDescription("Refreshing").assertRangeInfoEquals(ProgressBarRangeInfo.Indeterminate)
    }

    @Test
    fun pullingFarEnoughRefreshesOnce() {
        var refreshes = 0
        rule.setKit { PullToRefresh(refreshing = false, onRefresh = { refreshes++ }) { Rows() } }
        rule.onNodeWithTag("list").performTouchInput { swipeDown(startY = top + 10f, endY = bottom - 10f) }
        rule.waitForIdle()
        assertEquals(1, refreshes)
    }

    @Test
    fun aShortPullDoesNot() {
        var refreshes = 0
        rule.setKit { PullToRefresh(refreshing = false, onRefresh = { refreshes++ }) { Rows() } }
        rule.onNodeWithTag("list").performTouchInput { swipeDown(startY = top + 10f, endY = top + 10f + 40.dp.toPx()) }
        rule.waitForIdle()
        assertEquals(0, refreshes)
    }

    @Test
    fun talkBackCanRefreshWithoutPulling() {
        var refreshes = 0
        rule.setKit { PullToRefresh(refreshing = false, onRefresh = { refreshes++ }) { Rows() } }
        val node = rule.onNode(SemanticsMatcher.keyIsDefined(SemanticsActions.CustomActions)).fetchSemanticsNode()
        node.config[SemanticsActions.CustomActions].single { it.label == "Refresh" }.action()
        assertEquals(1, refreshes)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*MenuAndFeedbackTest*'`
Expected: compile errors (`HavenMenu`, `ProgressRing`, `PullToRefresh` unresolved).

- [ ] **Step 3: Add the strings**

`values/strings.xml` ui/kit block: `<string name="kit_refresh">Refresh</string>` and `<string name="kit_refreshing">Refreshing</string>`; `values-pt-rBR/strings.xml`: `<string name="kit_refresh">Atualizar</string>` and `<string name="kit_refreshing">Atualizando</string>`.

- [ ] **Step 4: Write `HavenMenu.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import androidx.compose.ui.window.SecureFlagPolicy
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

/** One action in a [HavenMenu]; [danger] draws it in ember. */
class MenuItem(val label: String, val onClick: () -> Unit, val icon: HavenIcon? = null, val danger: Boolean = false)

private val MenuGap = 4.dp

/**
 * A small menu under its trigger (put both in one Box). Its window sets
 * FLAG_SECURE, drops taps through another app's overlay and takes focus so
 * Back and a tap outside close it. Picking an item closes the menu, then acts.
 */
@Composable
fun HavenMenu(expanded: Boolean, onDismiss: () -> Unit, items: List<MenuItem>, modifier: Modifier = Modifier) {
    if (!expanded) return
    val gap = with(LocalDensity.current) { MenuGap.roundToPx() }
    Popup(
        popupPositionProvider = remember(gap) { BelowAnchor(gap) },
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true, securePolicy = SecureFlagPolicy.SecureOn),
    ) {
        SecureDialogWindow(ignoreObscuredTouches = true)
        MenuSurface(items, onDismiss, modifier)
    }
}

/** The menu as it draws, without its window: the catalogue shows it inline. */
@Composable
internal fun MenuSurface(items: List<MenuItem>, onDismiss: () -> Unit, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val motion = HavenTheme.motion
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    AnimatedVisibility(
        visibleState = shown,
        enter = fadeIn(motion.fadeSpec()) +
            scaleIn(motion.springSpec(HavenSprings.sheet), initialScale = 0.96f, transformOrigin = TransformOrigin(1f, 0f)),
    ) {
        Column(
            modifier
                .widthIn(min = 200.dp, max = 280.dp)
                .width(IntrinsicSize.Max)
                .shadow(16.dp, HavenShape.output)
                .clip(HavenShape.output)
                .background(colors.raised)
                .border(1.dp, colors.lineStrong, HavenShape.output)
                .padding(vertical = 6.dp),
        ) {
            items.forEach { item -> MenuRow(item, onDismiss) }
        }
    }
}

@Composable
private fun MenuRow(item: MenuItem, onDismiss: () -> Unit) {
    val colors = HavenTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.touch)
            .clickable(interactionSource = null, indication = HavenPress, role = Role.Button) {
                onDismiss()
                item.onClick()
            }
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (item.icon != null) {
            IconGlyph(item.icon, contentDescription = null, tint = if (item.danger) colors.danger else colors.muted, size = 20.dp)
            Spacer(Modifier.width(12.dp))
        }
        HavenText(item.label, style = HavenTheme.type.body, color = if (item.danger) colors.danger else colors.textStrong)
    }
}

/** Below the anchor and end-aligned with it; above it when the window has no room below. */
private class BelowAnchor(private val gap: Int) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset {
        val start = if (layoutDirection == LayoutDirection.Ltr) anchorBounds.right - popupContentSize.width else anchorBounds.left
        val x = start.coerceIn(0, (windowSize.width - popupContentSize.width).coerceAtLeast(0))
        val below = anchorBounds.bottom + gap
        val y = if (below + popupContentSize.height <= windowSize.height) {
            below
        } else {
            (anchorBounds.top - gap - popupContentSize.height).coerceAtLeast(0)
        }
        return IntOffset(x, y)
    }
}

@PreviewLightDark
@Composable
private fun HavenMenuPreview() {
    KitPreview {
        MenuSurface(
            listOf(MenuItem("Edit", {}, HavenIcon.Edit), MenuItem("Copy username", {}, HavenIcon.Copy), MenuItem("Delete", {}, HavenIcon.Trash, danger = true)),
            onDismiss = {},
        )
    }
}
```

- [ ] **Step 5: Write `ProgressRing.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenTheme

private const val SPIN_MILLIS = 900
private const val SWEEP = 270f
private const val TOP = -90f

/**
 * A ring: [progress] 0..1 drains or fills it on the tick spec (the one-time
 * code's ring); null spins it while something is working. Brass, or ember
 * when [warn]. Under "Remove animations" it steps and does not spin.
 */
@Composable
fun ProgressRing(
    progress: Float?,
    modifier: Modifier = Modifier,
    size: Dp = 28.dp,
    warn: Boolean = false,
    contentDescription: String? = null,
) {
    val colors = HavenTheme.colors
    val motion = HavenTheme.motion
    val arc = if (warn) colors.danger else colors.brass
    val shown by animateFloatAsState((progress ?: 0f).coerceIn(0f, 1f), motion.tickSpec(), label = "progress")
    val spin = if (progress == null && !motion.reduced) {
        rememberInfiniteTransition(label = "spin").animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infiniteRepeatable(tween(SPIN_MILLIS, easing = LinearEasing)),
            label = "angle",
        )
    } else {
        null
    }
    Canvas(
        modifier.size(size).semantics {
            progressBarRangeInfo = if (progress == null) {
                ProgressBarRangeInfo.Indeterminate
            } else {
                ProgressBarRangeInfo(progress.coerceIn(0f, 1f), 0f..1f)
            }
            if (contentDescription != null) this.contentDescription = contentDescription
        },
    ) {
        val stroke = Stroke(width = 3.dp.toPx(), cap = StrokeCap.Round)
        val inset = stroke.width / 2
        val box = Size(this.size.width - stroke.width, this.size.height - stroke.width)
        drawArc(colors.line, 0f, 360f, false, Offset(inset, inset), box, style = stroke)
        if (progress == null) {
            drawArc(arc, TOP + (spin?.value ?: 0f), SWEEP, false, Offset(inset, inset), box, style = stroke)
        } else {
            drawArc(arc, TOP, 360f * shown, false, Offset(inset, inset), box, style = stroke)
        }
    }
}

@PreviewLightDark
@Composable
private fun ProgressRingPreview() {
    KitPreview {
        Row {
            ProgressRing(0.7f)
            ProgressRing(0.12f, warn = true)
            ProgressRing(null)
        }
    }
}
```

- [ ] **Step 6: Write `PullToRefresh.kt`**

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.snap
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

private val PullThreshold = 72.dp
private val PullMax = 120.dp

/** The pull follows the finger at half speed. */
private const val RESISTANCE = 0.5f

/** Where the ring rests while refreshing, as a share of the threshold. */
private const val HOLD_FRACTION = 0.75f

/**
 * Pull down past the top of a scrolling list to refresh (foundation has no
 * pull-to-refresh). The content follows the finger; past 72dp a release
 * calls [onRefresh] once and the ring spins until [refreshing] goes false.
 * TalkBack users get a "Refresh" action on the container instead of a pull.
 */
@Composable
fun PullToRefresh(refreshing: Boolean, onRefresh: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val density = LocalDensity.current
    val state = remember(density) { with(density) { PullState(PullThreshold.toPx(), PullMax.toPx()) } }
    val settle = HavenTheme.motion.springSpec<Float>(HavenSprings.smooth)
    SideEffect {
        state.onRefresh = onRefresh
        state.settle = settle
    }
    LaunchedEffect(refreshing) {
        if (!refreshing && state.pull > 0f) state.settleTo(0f)
    }
    val refreshLabel = stringResource(R.string.kit_refresh)
    val refreshingLabel = stringResource(R.string.kit_refreshing)
    Box(
        modifier
            .nestedScroll(state)
            .semantics {
                customActions = listOf(
                    CustomAccessibilityAction(refreshLabel) {
                        onRefresh()
                        true
                    },
                )
            },
    ) {
        Box(Modifier.graphicsLayer { translationY = state.pull }) { content() }
        if (refreshing || state.pull > 0f) {
            ProgressRing(
                progress = if (refreshing) null else (state.pull / state.threshold).coerceIn(0f, 1f),
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .graphicsLayer { translationY = (maxOf(state.pull, if (refreshing) state.hold else 0f) - size.height) / 2 },
                contentDescription = if (refreshing) refreshingLabel else null,
            )
        }
    }
}

/** The pull distance, fed by the list's leftover scroll at its top. */
@Stable
private class PullState(val threshold: Float, private val maxPull: Float) : NestedScrollConnection {
    var pull by mutableFloatStateOf(0f)
        private set
    var onRefresh: () -> Unit = {}
    var settle: AnimationSpec<Float> = snap()
    val hold: Float get() = threshold * HOLD_FRACTION

    override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
        // Scrolling back up while pulled: give back the pull first.
        if (source != NestedScrollSource.UserInput || available.y >= 0f || pull <= 0f) return Offset.Zero
        val used = maxOf(available.y, -pull)
        pull += used
        return Offset(0f, used)
    }

    override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
        if (source != NestedScrollSource.UserInput || available.y <= 0f) return Offset.Zero
        pull = (pull + available.y * RESISTANCE).coerceAtMost(maxPull)
        return Offset(0f, available.y)
    }

    override suspend fun onPreFling(available: Velocity): Velocity {
        if (pull <= 0f) return Velocity.Zero
        val armed = pull >= threshold
        if (armed) onRefresh()
        settleTo(if (armed) hold else 0f)
        return available
    }

    suspend fun settleTo(target: Float) {
        animate(pull, target, animationSpec = settle) { value, _ -> pull = value }
    }
}
```

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*MenuAndFeedbackTest*' detekt lintGithubDebug`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit/MenuAndFeedbackTest.kt
git commit -m "feat(android): kit menu, progress ring and pull to refresh"
```

---
### Task 14: Debug-only catalogue

**Files:**
- Create: `apps/android/app/src/debug/AndroidManifest.xml`
- Create: `apps/android/app/src/debug/res/values/strings.xml`
- Create: `apps/android/app/src/debug/kotlin/net/havenkeys/android/catalogue/KitCatalogueActivity.kt`, `KitCatalogue.kt`, `CatalogueFoundations.kt`, `CatalogueComponents.kt`
- Modify: `apps/android/app/build.gradle.kts` (detekt sources)
- Test: `apps/android/app/src/testDebug/kotlin/net/havenkeys/android/catalogue/CatalogueTest.kt`

**Interfaces:**
- Consumes: every `ui/kit` component, and the internal surfaces `SheetSurface`, `DialogSurface`, `MenuSurface`.
- Produces: `internal class CatalogueSection(val title: String, val components: List<String>, val content: @Composable () -> Unit)`; `internal fun catalogueSections(): List<CatalogueSection>`; `@Composable internal fun KitCatalogue()`; `internal const val CATALOGUE_LIST = "catalogue"`.

- [ ] **Step 1: Write the failing test**

`src/testDebug/kotlin/net/havenkeys/android/catalogue/CatalogueTest.kt` (the `testDebug` source set: release unit tests never compile it, since the catalogue exists only in debug builds):

```kotlin
package net.havenkeys.android.catalogue

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performScrollToNode
import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.w3c.dom.Element

@RunWith(RobolectricTestRunner::class)
class CatalogueTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun everyKitComponentHasItsPlace() {
        val helpers = setOf("HavenText", "HavenPress", "KitPreview", "NoPersonalizedLearning")
        val kit = File("src/main/kotlin/net/havenkeys/android/ui/kit").listFiles().orEmpty()
            .map { it.nameWithoutExtension }.toSet() - helpers
        val shown = catalogueSections().flatMap { it.components }.toSet()
        assertEquals(kit.sorted(), shown.sorted())
    }

    @Test
    fun everySectionRendersInBothThemes() {
        rule.setContent { KitCatalogue() }
        for (theme in listOf("Light", "Dark")) {
            rule.onNodeWithTag(CATALOGUE_LIST).performScrollToIndex(0)
            rule.onNodeWithText(theme).performClick()
            catalogueSections().forEach { section ->
                val heading = hasText(section.title) and isHeading()
                rule.onNodeWithTag(CATALOGUE_LIST).performScrollToNode(heading)
                rule.onNode(heading).assertIsDisplayed()
            }
        }
    }

    @Test
    fun onlyDebugBuildsDeclareTheCatalogue() {
        val ns = "http://schemas.android.com/apk/res/android"
        val debug = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }.newDocumentBuilder()
            .parse(File("src/debug/AndroidManifest.xml")).documentElement
        val activities = debug.getElementsByTagName("activity")
        assertEquals(1, activities.length)
        assertEquals(".catalogue.KitCatalogueActivity", (activities.item(0) as Element).getAttributeNS(ns, "name"))
        assertFalse(File("src/main/AndroidManifest.xml").readText().contains("catalogue"))
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*CatalogueTest*'`
Expected: compile errors (`catalogueSections`, `KitCatalogue`, `CATALOGUE_LIST` unresolved).

- [ ] **Step 3: Declare the debug activity**

`src/debug/AndroidManifest.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <application>
        <!-- Debug builds only (spec 2026-10-03 §8, stage 2): every ui/kit
             component in both themes, for design review. It shows no vault
             data. It has a launcher entry so a reviewer can open it; release
             builds do not contain it. -->
        <activity
            android:name=".catalogue.KitCatalogueActivity"
            android:exported="true"
            android:label="@string/kit_catalogue_label"
            android:theme="@style/Theme.HavenKeys">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
```

`src/debug/res/values/strings.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="kit_catalogue_label" translatable="false">HavenKeys kit</string>
</resources>
```

- [ ] **Step 4: Write `KitCatalogueActivity.kt`**

```kotlin
package net.havenkeys.android.catalogue

import android.os.Bundle
import android.view.View
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge

/**
 * The kit catalogue (debug builds only). It holds no vault data, so unlike
 * every release activity it leaves FLAG_SECURE off: the catalogue is
 * screenshotted for design review. The kit's own windows (sheet, dialog,
 * menu) still set it, which is why the catalogue also draws them inline.
 */
class KitCatalogueActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.decorView.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
        window.decorView.filterTouchesWhenObscured = true
        enableEdgeToEdge()
        setContent { KitCatalogue() }
    }
}
```

- [ ] **Step 5: Write `KitCatalogue.kt`**

```kotlin
package net.havenkeys.android.catalogue

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.AddButton
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.SegmentedControl
import net.havenkeys.android.ui.kit.ToastState
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

internal const val CATALOGUE_LIST = "catalogue"

/** One group of the catalogue; [components] are the ui/kit file names it shows (CatalogueTest checks them all). */
internal class CatalogueSection(val title: String, val components: List<String>, val content: @Composable () -> Unit)

internal val LocalCatalogueToasts = staticCompositionLocalOf<ToastState> { error("no toast state") }

internal fun catalogueSections(): List<CatalogueSection> = listOf(
    CatalogueSection("Foundations", listOf("HavenIcon")) { Foundations() },
    CatalogueSection("Actions", listOf("HavenButton", "HavenIconButton", "CopyButton", "Pill", "AddButton")) { ActionsSection() },
    CatalogueSection("Structure", listOf("HavenScaffold", "SectionHeader", "InsetGroup", "GroupRow", "ItemRow")) { StructureSection() },
    CatalogueSection("Inputs", listOf("HavenTextField", "SecretTextField", "HavenSwitch", "HavenSlider", "SegmentedControl")) { InputsSection() },
    CatalogueSection("Overlays", listOf("HavenSheet", "HavenDialog", "Toast", "HavenMenu")) { OverlaysSection() },
    CatalogueSection("Feedback", listOf("ProgressRing", "PullToRefresh")) { FeedbackSection() },
)

private val ThemeChoices = listOf("System", "Light", "Dark")

/** Every kit component, in a theme picked at the top. HavenScaffold frames it; the add button and toasts are live. */
@Composable
internal fun KitCatalogue() {
    var choice by rememberSaveable { mutableIntStateOf(0) }
    val dark = when (choice) {
        1 -> false
        2 -> true
        else -> isSystemInDarkTheme()
    }
    HavenTheme(darkTheme = dark) {
        val toasts = rememberToastState()
        CompositionLocalProvider(LocalCatalogueToasts provides toasts) {
            HavenScaffold(
                topBar = {
                    Column(Modifier.padding(horizontal = HavenSpacing.gutter, vertical = 8.dp)) {
                        HavenText("HavenKeys kit", style = HavenTheme.type.title, color = HavenTheme.colors.textStrong)
                        SegmentedControl(ThemeChoices, choice, { choice = it })
                    }
                },
                floatingButton = { AddButton(onClick = { toasts.show("Add pressed") }) },
                toastState = toasts,
            ) { padding ->
                LazyColumn(
                    Modifier.fillMaxSize().testTag(CATALOGUE_LIST),
                    contentPadding = PaddingValues(
                        start = HavenSpacing.gutter,
                        end = HavenSpacing.gutter,
                        bottom = padding.calculateBottomPadding() + 24.dp,
                    ),
                ) {
                    catalogueSections().forEach { section ->
                        item(key = section.title) {
                            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                HavenText(
                                    section.title,
                                    Modifier.padding(top = 28.dp).semantics { heading() },
                                    style = HavenTheme.type.headline,
                                    color = HavenTheme.colors.textStrong,
                                )
                                section.content()
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 6: Write `CatalogueFoundations.kt`**

```kotlin
package net.havenkeys.android.catalogue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

@Composable
internal fun Foundations() {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SectionHeader("Colours")
        Swatches()
        SectionHeader("Type")
        TypeSamples()
        SectionHeader("Icons")
        HavenIcon.entries.chunked(6).forEach { row ->
            Row {
                row.forEach { icon ->
                    Column(Modifier.width(56.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        IconGlyph(icon, contentDescription = null, size = 24.dp)
                        HavenText(icon.name, style = HavenTheme.type.pill, color = HavenTheme.colors.muted)
                    }
                }
            }
        }
    }
}

@Composable
private fun Swatches() {
    val c = HavenTheme.colors
    val swatches = listOf(
        "pane" to c.pane, "list" to c.list, "group" to c.group, "field" to c.field,
        "raised" to c.raised, "line" to c.line, "lineStrong" to c.lineStrong, "text" to c.text,
        "textStrong" to c.textStrong, "muted" to c.muted, "brass" to c.brass, "brassHi" to c.brassHi,
        "brassInk" to c.brassInk, "brassSoft" to c.brassSoft, "primary" to c.primary, "ok" to c.ok,
        "danger" to c.danger, "glass" to c.glass, "avatar" to c.avatarBg, "digit" to c.digit,
        "symbol" to c.symbol, "sel" to c.sel, "thumb" to c.thumb, "scrim" to c.scrim,
    )
    swatches.chunked(4).forEach { row ->
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            row.forEach { (name, color) -> Swatch(name, color) }
        }
    }
}

@Composable
private fun Swatch(name: String, color: Color) {
    Column(Modifier.width(72.dp)) {
        Box(Modifier.size(40.dp).clip(HavenShape.control).background(color).border(1.dp, HavenTheme.colors.lineStrong, HavenShape.control))
        HavenText(name, style = HavenTheme.type.pill, color = HavenTheme.colors.muted)
    }
}

@Composable
private fun TypeSamples() {
    val t = HavenTheme.type
    val c = HavenTheme.colors
    listOf(
        "Display: HavenKeys is locked." to t.display,
        "Headline: GitHub" to t.headline,
        "Title: New item" to t.title,
        "Small title: Nothing here yet" to t.titleSmall,
        "Body: the reading size, for sentences that explain." to t.body,
        "Value: sam@example.com" to t.value,
        "Row title: GitHub" to t.rowTitle,
        "Row subtitle: sam@example.com" to t.rowSubtitle,
        "Label: Username" to t.label,
        "Group title: Frequently used" to t.groupTitle,
        "Button: Unlock" to t.button,
        "Pill: This device" to t.pill,
    ).forEach { (text, style) -> HavenText(text, style = style, color = c.textStrong) }
    HavenText("k7#Rq2!vL9pX", style = t.secret, color = c.textStrong)
    HavenText("381 492", style = t.code, color = c.brassInk)
    HavenText("••••••••••••", style = t.masked, color = c.textStrong)
}
```

- [ ] **Step 7: Write `CatalogueComponents.kt`**

```kotlin
package net.havenkeys.android.catalogue

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.CopyButton
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.DialogSurface
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenSlider
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.ItemTile
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.MenuSurface
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.kit.PillTone
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.kit.RowLeading
import net.havenkeys.android.ui.kit.SecretTextField
import net.havenkeys.android.ui.kit.SectionAction
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.SegmentedControl
import net.havenkeys.android.ui.kit.SheetSurface
import net.havenkeys.android.ui.kit.ToggleRow
import net.havenkeys.android.ui.kit.TrailingText
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

private const val COPIED_TOAST = "Password copied · clears in 30 s"

@Composable
internal fun ActionsSection() {
    val toasts = LocalCatalogueToasts.current
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        HavenButton("Unlock", onClick = {}, modifier = Modifier.fillMaxWidth())
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            HavenButton("Cancel", onClick = {}, style = ButtonStyle.Secondary)
            HavenButton("Skip", onClick = {}, style = ButtonStyle.Quiet)
            HavenButton("Remove", onClick = {}, style = ButtonStyle.Danger)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            HavenButton("Generate", onClick = {}, style = ButtonStyle.Secondary, icon = HavenIcon.Dice)
            HavenButton("Disabled", onClick = {}, enabled = false)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            HavenIconButton(HavenIcon.Lock, "Lock", onClick = {})
            HavenIconButton(HavenIcon.More, "More", onClick = {})
            CopyButton("Password", onCopy = { toasts.show(COPIED_TOAST) })
            Pill("This device")
            Pill("Whole site", Modifier.padding(start = 8.dp), tone = PillTone.Outline)
        }
    }
}

@Composable
internal fun StructureSection() {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        HavenText(
            "HavenScaffold frames this screen: the bar above, the add button and the toasts below.",
            style = HavenTheme.type.body,
            color = HavenTheme.colors.muted,
        )
        SectionHeader("Recently added", action = SectionAction("Clear") {})
        InsetGroup {
            row { ItemRow("GitHub", "sam@example.com", RowLeading.Monogram("GitHub"), onClick = {}, hasPasskey = true, hasCode = true) }
            row { ItemRow("Wi-Fi at home", "Secure note", RowLeading.Glyph(HavenIcon.Note, soft = true), onClick = {}) }
            row { ItemRow("Visa ending 4242", "Card", RowLeading.Glyph(HavenIcon.Card), onClick = {}) }
            row { ItemRow("😀 A title that is long enough to be cut at the end of the row", null, RowLeading.Monogram("😀"), onClick = {}) }
        }
        SectionHeader("Settings")
        InsetGroup {
            row { GroupRow(onClick = {}, icon = HavenIcon.Clock) { GroupRowText("Auto-lock", "After 5 minutes") } }
            row { GroupRow(onClick = {}, trailing = { TrailingText("12") }) { GroupRowText("Logins") } }
            row { GroupRow(trailing = { CopyButton("Username", onCopy = {}) }) { GroupRowField("Username", "sam@example.com") } }
        }
    }
}

@Composable
internal fun InputsSection() {
    val username = rememberTextFieldState("sam@example.com")
    val server = rememberTextFieldState("http://vault.local")
    val secret = rememberTextFieldState("correct horse battery")
    var revealed by remember { mutableStateOf(false) }
    var biometrics by remember { mutableStateOf(true) }
    var screenOff by remember { mutableStateOf(false) }
    var length by remember { mutableFloatStateOf(24f) }
    var mode by remember { mutableIntStateOf(0) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        InsetGroup {
            row { HavenTextField(username, "Username") }
            row { HavenTextField(server, "Server", error = "The address must start with https://") }
            row { SecretTextField(secret, "Password", revealed, { revealed = it }) }
        }
        InsetGroup {
            row { ToggleRow("Unlock with biometrics", biometrics, { biometrics = it }) }
            row { ToggleRow("Lock when the screen turns off", screenOff, { screenOff = it }, detail = "Recommended") }
        }
        HavenSlider(length, { length = it }, 8f..64f, label = "Length", steps = 55, valueText = "${length.roundToInt()}")
        SegmentedControl(listOf("Random", "Words", "PIN"), mode, { mode = it })
    }
}

private fun sampleMenu() = listOf(
    MenuItem("Edit", {}, HavenIcon.Edit),
    MenuItem("Copy username", {}, HavenIcon.Copy),
    MenuItem("Delete", {}, HavenIcon.Trash, danger = true),
)

@Composable
private fun SampleTiles() {
    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        listOf("Login" to HavenIcon.Key, "Secure note" to HavenIcon.Note, "Card" to HavenIcon.Card, "Generate" to HavenIcon.Dice)
            .forEach { (label, icon) ->
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    ItemTile(RowLeading.Glyph(icon, soft = true), size = 52.dp)
                    HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.text)
                }
            }
    }
}

@Composable
internal fun OverlaysSection() {
    val toasts = LocalCatalogueToasts.current
    var sheet by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        HavenText(
            "Drawn inline below; the buttons open the real ones, in secure windows that screenshots show black.",
            style = HavenTheme.type.body,
            color = HavenTheme.colors.muted,
        )
        SheetSurface("New item") { SampleTiles() }
        DialogSurface(
            title = "Remove this device?",
            confirm = DialogAction("Remove", {}, danger = true),
            message = "Its local copy of the vault is erased.",
            dismiss = DialogAction("Cancel", {}),
        )
        MenuSurface(sampleMenu(), onDismiss = {})
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            HavenButton("Sheet", onClick = { sheet = true }, style = ButtonStyle.Secondary)
            HavenButton("Dialog", onClick = { dialog = true }, style = ButtonStyle.Secondary)
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box {
                HavenButton("Menu", onClick = { menu = true }, style = ButtonStyle.Secondary)
                HavenMenu(menu, onDismiss = { menu = false }, items = sampleMenu())
            }
            HavenButton("Toast", onClick = { toasts.show(COPIED_TOAST) }, style = ButtonStyle.Secondary)
        }
    }
    if (sheet) HavenSheet(onDismiss = { sheet = false }, title = "New item") { SampleTiles() }
    if (dialog) {
        HavenDialog(
            title = "Remove this device?",
            onDismiss = { dialog = false },
            confirm = DialogAction("Remove", { dialog = false }, danger = true),
            message = "Its local copy of the vault is erased.",
            dismiss = DialogAction("Cancel", { dialog = false }),
        )
    }
}

@Composable
internal fun FeedbackSection() {
    var refreshing by remember { mutableStateOf(false) }
    LaunchedEffect(refreshing) {
        if (refreshing) {
            delay(1_500)
            refreshing = false
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
            ProgressRing(0.7f, contentDescription = "21 seconds left")
            ProgressRing(0.12f, warn = true, contentDescription = "4 seconds left")
            ProgressRing(null, contentDescription = "Loading")
        }
        HavenText("Pull the list below down to refresh.", style = HavenTheme.type.body, color = HavenTheme.colors.muted)
        PullToRefresh(
            refreshing = refreshing,
            onRefresh = { refreshing = true },
            modifier = Modifier.fillMaxWidth().height(220.dp).clip(HavenShape.group).background(HavenTheme.colors.group),
        ) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(12) { HavenText("Row ${it + 1}", Modifier.padding(16.dp)) }
            }
        }
    }
}
```

- [ ] **Step 8: Lint the debug sources too**

In `app/build.gradle.kts`, change the detekt block's source line to:

```kotlin
    source.setFrom("src/main/kotlin", "src/test/kotlin", "src/debug/kotlin", "src/testDebug/kotlin")
```

- [ ] **Step 9: Run the tests, lint and both builds**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease`
Expected: all pass. Then confirm the release build has no catalogue:

Run: `cd apps/android && grep -rl "KitCatalogueActivity" app/build/intermediates --include=AndroidManifest.xml | grep -i release`
Expected: no output.

- [ ] **Step 10: Commit**

```bash
git add apps/android/app/build.gradle.kts apps/android/app/src/debug apps/android/app/src/testDebug
git commit -m "feat(android): debug-only catalogue of every kit component in both themes"
```

---

### Task 15: Device pass, `/impeccable` review and `apps/android/DESIGN.md`

The controller runs this task (it needs an emulator and `/impeccable`). Fixes that come out of it are separate commits, each with a test where the fix has behaviour.

**Files:**
- Create: `apps/android/.impeccable/review/kit-*.png` (screenshots)
- Create: `apps/android/DESIGN.md` (and the `apps/android/.impeccable/design.json` sidecar the documenter writes)
- Modify: kit files, as the review requires
- Modify: `docs/superpowers/plans/2026-10-03-android-redesign-index.md` (stage 2 row → Done)

- [ ] **Step 1: Full verification**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug` and `pnpm --filter @havenkeys/desktop typecheck`
Expected: all pass.

- [ ] **Step 2: Emulator screenshots, both themes, animations on and off**

```bash
cd apps/android
adb install -r app/build/outputs/apk/github/debug/app-github-debug.apk
mkdir -p .impeccable/review
for mode in no yes; do
  adb shell cmd uimode night $mode
  adb shell am start -n net.havenkeys.android/.catalogue.KitCatalogueActivity
  sleep 2
  for page in 1 2 3 4 5 6 7 8; do
    adb exec-out screencap -p > .impeccable/review/kit-night-$mode-$page.png
    adb shell input swipe 540 1600 540 500 300
    sleep 1
  done
done
```

Then by hand on the emulator, once with `adb shell settings put global animator_duration_scale 1` and once with `0` (restore `1` afterwards): open the real sheet (rises on a spring, backdrop dims, drag down and backdrop tap close it), dialog, menu, toast, copy check, switch, segmented control, pull to refresh. With scale 0 every one of them must cut instantly with nothing blocked; a tap during the sheet's rise must close it.

- [ ] **Step 3: TalkBack pass**

Turn TalkBack on (`adb shell settings put secure enabled_accessibility_services com.google.android.marvin.talkback/com.google.android.marvin.talkback.TalkBackService`, or by hand) and walk the catalogue. Check: every control says its name, role and state once (button, switch on/off, tab selected, slider value "24"); a text field reads its label once (the `contentDescription` decision); the secret field reads as a password field and never speaks its value; the copy button says "Copy Password" then "Copied"; the toast is announced; the sheet, dialog and menu announce their titles; the item row reads title, subtitle, "One-time code", "Passkey" as one stop; the pull-to-refresh "Refresh" action is reachable. If the custom action on the pull container is not reachable from any focusable node, record it for stage 3 (Home then needs a visible way to sync). Record what you found in the DESIGN.md review notes (Step 5).

- [ ] **Step 4: `/impeccable` review of the catalogue against the desktop**

Invoke `/impeccable` (skill `impeccable:impeccable`) in critique/audit mode with this brief:

> Review the HavenKeys Android design system before any screen uses it. Artefacts: the catalogue screenshots in `apps/android/.impeccable/review/kit-*.png` (light `night-no`, dark `night-yes`), the kit sources in `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit/` and `ui/theme/`. Reference: `apps/desktop/DESIGN.md`, `apps/desktop/.impeccable/design.json`, the desktop screenshots in `apps/desktop/.impeccable/review/`, and spec `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §5 and §7. Judge: is it recognisably the desktop's Vault Room adapted to touch, and does nothing read as Material (no ripple, no Material sheet feel, no Material type scale)? Check the One Fitting Rule (brass only on controls and thin lines; one primary per screen; light primary is forest), the serif-for-titles decision (spec §5.1 lists serif for monograms; the plan extends it to titles as the desktop does — keep or revert), the type sizes against spec §5.1, inset groups and the hairline inset, the row is the field, the segmented control's light track, the new glyphs (home, items, chevronRight, chevronLeft, more) against the desktop set's drawing rules, the phone contrast departures (light muted #61706a, light brass ink #7d632f, dark onDanger #121a17: should the desktop adopt them?), touch targets, and the springs' feel (§7). Return an ordered list of material fixes.

Apply the fixes the owner would agree with without asking (visual tuning inside the kit, token values with their reasons). Anything that changes a spec decision (for example reverting serif titles) goes into DESIGN.md as an open question instead. Keep `ContrastTest` and every other test passing; add a test for any fix with behaviour. Commit each fix:

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui apps/android/app/src/test
git commit -m "fix(android): <what the review changed, in the kit's terms>"
```

- [ ] **Step 5: Write `apps/android/DESIGN.md` from what shipped**

Use the `impeccable:impeccable-documenter` agent (or `/impeccable` document mode) so the file follows the same format as `apps/desktop/DESIGN.md`: YAML front matter (`name`, `description`, `colors` with light and dark values, `typography`, `rounded`, `spacing`, `components`) derived from the shipped `ui/theme` and `ui/kit` code, not from this plan; then the sections Overview (the Vault Room in the hand), Colors (with the phone's departures and their contrast reasons), Typography (sizes in sp, the faces and their jobs, the font files and how to rebuild them), Layout (gutter, rows, touch targets, insets), Elevation (what floats: sheet, dialog, menu, toast, add button), Shapes, Motion (the four named springs in Apple terms and their Compose stiffness, the fade, the stagger, "Remove animations" cuts everything), Components (every kit component, one paragraph each: what it is for, its states, its TalkBack reading), Named Rules (No Ripple, One Fitting, Row Is the Field, Secure Windows, Words Wrap, Foundation Only), Do's and Don'ts, and "Review notes (stage 2)": the `/impeccable` findings with what was fixed and what stays open, and the TalkBack findings from Step 3.

- [ ] **Step 6: Close the stage in the index**

In `docs/superpowers/plans/2026-10-03-android-redesign-index.md`, set the stage 2 row's status to `Done`, and under "### Stage 3" add a bullet: "Builds on the kit as shipped: read `apps/android/DESIGN.md` and the `ui/kit` sources for component names and signatures; carry the open questions from DESIGN.md's review notes."

- [ ] **Step 7: Commit**

```bash
git add apps/android/DESIGN.md apps/android/.impeccable docs/superpowers/plans/2026-10-03-android-redesign-index.md
git commit -m "docs(android): the phone's design system as shipped, with the stage 2 review"
```

---

## Stage exit check

- [ ] `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease` passes.
- [ ] `pnpm --filter @havenkeys/desktop typecheck` passes.
- [ ] `grep -rln "androidx.compose.material" apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/debug apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme` lists only `ui/theme/MaterialBridge.kt`.
- [ ] `grep -rn "ui.kit" apps/android/app/src/main/kotlin --include=*.kt | grep -v "/ui/kit/"` shows only the `NoPersonalizedLearning` import in `ui/edit/EditScreen.kt` (no screen uses the kit yet).
- [ ] Every file in `ui/kit` except `HavenText`, `HavenPress`, `KitPreview` and `NoPersonalizedLearning` has a `@PreviewLightDark` preview and appears in `catalogueSections()` (CatalogueTest).
- [ ] The release APK has no catalogue activity, and the four font files total under 500 KB.
