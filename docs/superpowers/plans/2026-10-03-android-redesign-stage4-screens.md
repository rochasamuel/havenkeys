# Android Redesign, Stage 4: Existing Screens Rebuilt Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild every screen still drawn with Material (item detail, editors, generator, unlock, onboarding, devices, autofill setup, "Search HavenKeys…", the autofill confirmation dialog, the passkey save screen) from `ui/kit`, with their behaviour and security rules unchanged, the monogram tile travelling from a row into the item header, and no Material import left anywhere but `ui/theme/MaterialBridge.kt`.

**Architecture:** The kit grows by the pieces these screens need and the stage 2 and 3 reviews deferred here (read-only rows as one TalkBack stop, field hints, equal-height segments, tall sheets that scroll, a dialog with a third answer or none but its button, a choice sheet). Full-screen screens over the shell share one bar (`ui/components/ScreenBar.kt`) and one way to draw a hidden or revealed value (`ui/components/SecretText.kt`). Each screen keeps its ViewModel, its public signature (plus a `tileModifier` on the item screen) and its tests; it gains Robolectric screen tests where behaviour lives in the composable (revealed values, typed secrets, dialogs). Each task widens the `forbidMaterialInKit` Gradle check to the package it rebuilt; the last code task makes it cover every source file but the bridge.

**Tech Stack:** Kotlin 2.4, Jetpack Compose BOM 2026.06.01 (foundation, animation, ui 1.11.4), Navigation Compose 2.9.8, lifecycle 2.10.0, Robolectric 4.17 with `ui-test-junit4` (`createComposeRule`, `StateRestorationTester`), detekt 1.23.

**Spec:** `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §6.9 (existing screens rebuilt from the kit, behaviour unchanged; editor copies record a use), §7 (motion: the monogram tile into the detail header, deferred here from stage 3), §8 stage 4, §9 (security), §10 (testing). Security rules screens keep: `docs/superpowers/specs/2026-10-01-android-app-design.md` §9.3–9.4, `docs/android.md`, `CLAUDE.md` §9, §19, §34, §39, §44. Kit as shipped: `apps/android/DESIGN.md` and the sources in `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit/` and `ui/theme/`. Previous stage: `docs/superpowers/plans/2026-10-03-android-redesign-stage3-shell.md`.

## Global Constraints

- Every screen this plan touches uses `ui/kit`, `ui/components` (kit-built parts only) and `androidx.compose.foundation`: no `import androidx.compose.material…`. Each task adds its package to `forbidMaterialInKit` (`apps/android/app/build.gradle.kts`); Task 16 makes the check cover `**/*.kt` but `ui/theme/MaterialBridge.kt`.
- Behaviour unchanged: every existing test keeps passing. A test is changed only where the UI's structure changed (a node found another way); a security assertion is never weakened or removed.
- A revealed value (item detail) lives only in the `remember`ed `RevealState` of the row showing it: cleared after 30 s (`RevealState.REVEAL_MS`), when the row leaves the composition, on `ON_STOP` and on lock (the lock replaces the back stack). Never in a ViewModel, `SavedStateHandle`, `rememberSaveable`, a route, a toast, a content description or a log.
- A typed secret (master password, Secret Key, invite, a password or setup key in the editor) is held by a `TextFieldState` created with `remember { TextFieldState(…) }`: **never** `rememberTextFieldState()`, which saves its text with the instance state. Non-secret onboarding fields (server, email) keep today's saveable behaviour with `rememberTextFieldState()`.
- Secret inputs use the kit's `SecretTextField` (password semantics, password keyboard, no autocorrect, no personalised learning, no cut or copy). Multi-line secure-note content stays a plain multi-line `HavenTextField` with autocorrect off, as today.
- A hidden value is drawn as `MaskedValue` (the fixed 12-dot mask, never the value's length; TalkBack hears "Hidden <label>"). A revealed one is `RevealedValue` (mono, digits and symbols coloured, not selectable).
- Toasts say what happened, never a value: `R.string.copied` ("%1$s copied. The clipboard clears in %2$d s."), `R.string.generator_copied`, or an error from `errorText(code)` with `ToastTone.Alert`. Material snackbars are gone.
- `FLAG_SECURE`, autofill exclusion and touch filtering are unchanged: `MainActivity`, `hardenWindow()` and the autofill activities set them on their windows; the kit's `HavenSheet`, `HavenDialog` and `HavenMenu` set them on their own windows. No activity loses a flag.
- Copy goes only through `SensitiveClipboard.copy` after an explicit tap; an item copy then calls `ItemViewModel.copied()` (`record_use`), as today. The generator's copy is not an item and records nothing.
- No ripple: kit controls only (`havenClickable`/`HavenPress` inside the kit). Every control is at least 48 x 48 dp.
- Our own words wrap and are never cut with an ellipsis; only user data (titles, usernames, URLs) may ellipsize.
- Every new string has an English and a Brazilian Portuguese version, appended before `</resources>` under the comment `<!-- Existing screens rebuilt (spec 2026-10-03 §6.9) -->` (Task 8 opens the block in both files). `lintGithubDebug` fails on a missing translation. Task 16 removes strings nothing uses any more.
- No logging: the `forbidLogging` check covers every file.
- detekt runs on all of it. `MaxLineLength` is 120: where a line of this plan's code is longer, wrap its arguments one per line and change nothing else. `MagicNumber` ignores `12.dp`-style receivers; any other bare number goes into a named `private const val`. `LongParameterList` counts parameters without defaults from 6. `TooManyFunctions` allows 11 per file: split as this plan does, do not suppress.
- Unit tests: `cd apps/android && ./gradlew testGithubDebugUnitTest`. Screen tests run under Robolectric (SDK 34) with `createComposeRule()` and the kit's `setKit`, `hasRole`, `assertTouchTarget`, `removeAnimations` (`ANDROID_TEST/ui/kit/KitTestSupport.kt`). Build a screen test's ViewModel outside `setKit { }`. Look strings up with `RuntimeEnvironment.getApplication().getString(R.string.x)` rather than copying English text into tests (Robolectric runs in English, so either works; the lookup survives copy edits).
- No emulator: verification is JVM tests, detekt, lint and assemble. Device and TalkBack checks go to the owner's checklist in `docs/android.md` (Task 18).
- Commits without Co-Authored-By lines (owner's rule). Each task leaves `testGithubDebugUnitTest detekt assembleGithubDebug` green.

## Entry checks

Run these before Task 2.

1. **Stage 3 is done.** In `docs/superpowers/plans/2026-10-03-android-redesign-index.md` the stage 3 row reads `Done`. `apps/android/DESIGN.md` has "Review notes (stage 3)" and `docs/android.md` has the section "### Redesign stage 3 (shell)". Read both review sections: an open item that names a screen of this plan goes into that screen's task; one that changes a kit component this plan changes goes into Task 2 or 2. The three items stage 3 deferred to this stage are already planned: `HavenSheet` scroll, maximum height and IME padding (Task 3), equal-height `SegmentedControl` segments when labels wrap (Task 2), `GroupRow` merging a read-only label and value into one TalkBack stop (Task 2).
2. **Kit signatures as shipped.** Stage 3 Task 16 may have changed kit or screen files after this plan was written. This plan calls, in `net.havenkeys.android.ui.kit`: `GroupRow(modifier, onClick, onClickLabel, icon, trailing, chevron, content)`, `GroupRowText(title, detail)`, `GroupRowField(label, value, valueStyle)`, `TrailingText(text)`, `InsetGroup(modifier, content)` with `row { }`, `ItemRow(title, subtitle, leading, onClick, modifier, titleModifier, hasPasskey, hasCode)`, `ItemTile(leading, modifier, size)`, `RowLeading.Monogram/Glyph`, `SectionHeader(text, modifier, action)`, `HavenSheet(onDismiss, modifier, title, content)`, internal `SheetSurface(title, modifier, content)`, `HavenDialog(title, onDismiss, confirm, modifier, message, dismiss, content, confirmEnabled, busy, answerKey)`, internal `DialogSurface(…same…)`, `DialogAction(label, onClick, danger)`, `HavenMenu(expanded, onDismiss, items, modifier)`, `MenuItem(label, onClick, icon, danger)`, `HavenTextField(state, label, modifier, placeholder, error, enabled, keyboardOptions, onKeyboardAction, lineLimits, inputTransformation)`, `SecretTextField(state, label, revealed, onRevealChange, modifier, error, enabled, imeAction, onKeyboardAction)`, internal `FieldRow(label, error, focused, trailing, field)`, `SegmentedControl(options, selectedIndex, onSelect, modifier)`, `HavenSlider(value, onValueChange, valueRange, label, modifier, steps, valueText, enabled)`, `ToggleRow(title, checked, onCheckedChange, modifier, detail, enabled)`, `HavenButton(text, onClick, modifier, style, enabled, icon)`, `ButtonStyle.{Primary, Secondary, Quiet, Danger}`, `HavenIconButton(icon, contentDescription, onClick, modifier, enabled, tint)`, `CopyButton(fieldLabel, onCopy, modifier)`, `Pill(text, modifier, tone)`, `ProgressRing(progress, modifier, size, warn, contentDescription)`, `HavenScaffold(modifier, topBar, bottomBar, floatingButton, toastState, content)`, `rememberToastState()`, `ToastState.show(text, tone)`, `ToastTone.{Done, Alert}`, `HavenText(text: String | AnnotatedString, modifier, style, color, maxLines, overflow)`, `IconGlyph(icon, contentDescription, modifier, tint, size)`, `HavenIcon.{Lock, Key, Note, Qr, Dice, Plus, Copy, Check, Eye, EyeOff, Edit, Trash, X, Alert, CloudOff, ChevronLeft, More, Refresh}`, internal `HavenPress`. In `ui.shell`: `LargeTitle(text, modifier)`, `ErrorLine(code, modifier)`, `EmptyLine(text, modifier)`, `SummaryRow`, `SharedTitle`, `NoSharedTitle`, `LazyListScope.insetGroup(items, key, around, row)`, internal `ItemSummary.leading()`. In `ui.search`: internal `SearchField(state, modifier)`. In `ui.nav`: `sharedIfMoving`, `TitleTravel`, `titleKey`. Check with `grep -n "^fun \|^internal fun \|^class \|^enum class \|^typealias \|^val " apps/android/app/src/main/kotlin/net/havenkeys/android/ui/{kit,shell,nav,search}/*.kt`. If one was renamed or reshaped, change this plan's call to match; do not undo the stage 3 change.
3. **APIs on the current BOM** (checked 2026-10-03 against foundation 1.11.4 and ui-test 1.11.4): `TextFieldLineLimits.MultiLine(minHeightInLines, maxHeightInLines)`, `TextFieldState.setTextAndPlaceCursorAtEnd`, `TextFieldState.clearText`, `BoxWithConstraints`, `Modifier.verticalScroll(state, enabled)`, `ScrollState.maxValue`, `IntrinsicSize.Min`, `StateRestorationTester` (used by `SearchScreenTest`). Recheck only if `gradle/libs.versions.toml` changed.
4. **Stage 3's final fixes are in.** They change files this plan touches: `ANDROID/ui/nav/HavenNavHost.kt` (`ifSettled`: `Nav` ignores a push unless the current entry is `RESUMED`, against double taps; `Routes.SEARCH` is pushed with `launchSingleTop`) and the kit's `HavenPress.kt` (`FocusDrawSeam` becomes per node). Re-read `HavenNavHost.kt` and `DoubleTapTest.kt` before Tasks 1 and 7 (Task 1 replaces the `RESUMED` guard and keeps the rest); this plan does not touch `HavenPress.kt`, but a kit test that draws focus (Tasks 2, 3) must use the per-node seam as shipped.
5. **Screenshots.** `apps/android/app/src/testDebug/kotlin/net/havenkeys/android/screens/ShellScreenshots.kt` exists and `-PscreensDir` reaches the system property `havenkeys.screens.dir` (`grep -n screensDir apps/android/app/build.gradle.kts`). Task 17 reuses it.

## Decisions recorded for the owner

- **A seam for the app's navigation (`NavServices`).** The lock wipe and Back are pinned through the real `HavenNavHost` (Task 1, asked for by the controller). The NavHost took the `AppContainer`, which needs Rust and the Keystore; it now takes `NavServices` (repositories, events, clipboard, and the two activity-bound pieces as composable lambdas: the unlock screen and Settings' biometric actions). The app builds it from the container; the test builds it from fakes. No behaviour changes.
- **Double taps are de-duplicated by destination, not by settling.** Stage 3's `ifSettled` dropped any push while the current entry was not `RESUMED`, which swallowed a tap made during a transition (spec §7: "a tap mid-transition goes straight to its destination"). `pushOnce` ignores a push only when the same route with the same arguments is already on top. Navigation's own `launchSingleTop` is not used for `item/{id}`: it compares destinations, not arguments, and would hand a second item's id to the first item's entry and ViewModel.
- **One bar for full-screen screens.** Item, editor, generator, devices and autofill setup get `ScreenBar`: Back, then the offline marker ("Offline, read-only"), Lock and the screen's own actions, the order the old Material bar had. The bar has no title; each screen shows its large serif title under it (the category lists' pattern from stage 3). `ui/components/HavenTopBar.kt` is deleted.
- **The editor has no copy action, so it records nothing.** Spec §6.9 says "the editor's copy actions call `record_use` like the detail's", but the editor has never had a copy action, and the kit's `SecretTextField` refuses cut and copy by design. Adding copy buttons to the editor would be a new editor capability, which spec §3 puts out of scope. The detail's copies keep recording (`ItemViewModel.copied()`). If the owner wants editor copies, they go through an `EditViewModel.copied()` that calls `recordUse` for an existing item only.
- **The one-time code's setup key is typed in a `SecretTextField`.** Today it is a visible field with a password keyboard; it is a secret, and the stage's security rule says secret fields use the kit's secret field. It is masked until the eye shows it; the placeholder ("Setup key or otpauth:// link") becomes the field's hint.
- **The website match is picked from a choice sheet.** Its three labels are sentences ("Whole site, any subdomain"); a segmented control cannot hold them on a phone. Settings' private choice sheet becomes the kit's `ChoiceSheet` (with `ChoiceRow`), used by Settings, the editor and the passkey sheet.
- **Delete sits behind More, and More is disabled offline.** `HavenMenu` items have no disabled state; Delete is More's only entry, so More itself is disabled offline (deleting needs the server, as today). Edit is disabled offline as today. The identity has no More.
- **The passkey save screen becomes a sheet over the calling app.** `PasskeyCreateActivity` takes the translucent theme the other credential activities use, and its content is a `HavenSheet` titled "Save a passkey to HavenKeys?". Dragging it down, tapping the backdrop or Back answer as its own Cancel (or Close, when the passkey already exists) would. When the vault is locked, unlock still shows full screen first, as today.
- **HavenDialog gains a third answer and an undismissible mode.** The card and identity confirmation asks with three buttons (fill, the alternative, cancel); with an `alternative` the buttons stack full width so long labels wrap instead of squeezing. `dismissible = false` makes Back and an outside tap do nothing, for the editor's conflict dialog, whose only answer is Reload (today's Material dialog ignored them too); without it, the kit's answer-once guard would treat Back as the answer and disable Reload.
- **The biometric button has no glyph.** The set has no fingerprint glyph; drawing one means adding it to the desktop set too (spec §5.1) for one button whose label already says it.
- **The autofill dropdown row stays a RemoteViews row.** Android draws it in its own window from `res/layout/autofill_item.xml`; Compose cannot draw there. Only its copy glyph changes, to the desktop's copy path (it was Material's `content_copy`). Keyboard chips stay the system's `InlineSuggestionUi`. Credential Manager's account selector is drawn by Android; HavenKeys' own screens there are unlock and the passkey sheet.
- **Errors in the editor's offline state read as a line, not a banner.** The editor's offline note becomes the cloud-off glyph and a muted line: brass marks controls and thin lines, never large surfaces (DESIGN.md).
- **The generator gets no segmented control.** Rust's generator has no modes; the length shows as "Length … 24" above the slider, since `HavenSlider` shows no value of its own (stage 2 open item, settled here for this screen). The equal-height segment fix still lands in the kit (Task 2) because the stage 3 review deferred it here.
- **A reload clears a revealed value rather than moving it.** `InsetGroup` composes its rows by position; a reveal is keyed by the field inside its row, so a reload that inserts a field above it starts that row fresh (masked) instead of showing a value on the wrong row.
- **Onboarding's three ways** use the set's glyphs: Scan → `Qr`, Type → `Edit`, Invite → `Plus`.

## Review Focus

1. **A revealed password when the screen is restored or left alone.** It hides itself after 30 s and a restored screen comes back masked. Pinned in Task 6 (`aRevealedPasswordHidesItselfAfter30Seconds`, `aRestoredItemScreenComesBackMasked`).
2. **The master password after a submit or a restore.** The field is emptied the moment Unlock is tapped, before Rust answers, and a restored unlock screen brings nothing back. Pinned in Task 11 (`submittingSendsThePasswordOnceAndEmptiesTheField`, `aRestoredUnlockScreenBringsNoPasswordBack`).
3. **A field whose value arrives after the editor shows.** It takes no typing until Rust's value is in, then shows it, and the draft is still clean. Pinned in Task 8 (`aFieldWaitingForItsValueTakesNoTypingThenShowsIt`).
4. **A passkey sheet with many logins at a large font.** Save stays reachable by scrolling; the sheet never covers the whole window. Pinned in Task 3 (`aTallSheetStopsShortOfTheTopAndScrollsToItsLastRow`) and Task 15 (`aLongListAtALargeFontStillReachesSave`).
5. **Back on a dialog that must be answered, and double answers.** Back on the editor's conflict leaves Reload working; a third answer answers once. Pinned in Task 3 (`anUndismissibleDialogIgnoresBackAndStillAnswers`, `aThirdAnswerStacksTheButtonsAndAnswersOnce`) and Task 9 (`backOnTheConflictLeavesReloadWorking`).

---

## File Structure

Paths abbreviate `apps/android/app/src/main/kotlin/net/havenkeys/android` as `ANDROID/`, `apps/android/app/src/test/kotlin/net/havenkeys/android` as `ANDROID_TEST/`, and `apps/android/app/src/main/res` as `RES/`. Gradle commands run in `apps/android`.

| File | Responsibility |
|---|---|
| `ANDROID/ui/nav/NavServices.kt` (new), `HavenNavHost.kt`, `ShellDestinations.kt` | What the navigation needs from the app; the NavHost under test |
| `ANDROID_TEST/ui/nav/HavenNavHostTest.kt` (new) | Lock wipe from item, search and a category list; Back through Items |
| `ANDROID/ui/kit/GroupRow.kt` | A read-only row's text is one TalkBack stop |
| `ANDROID/ui/kit/HavenTextField.kt`, `SecretTextField.kt` | `hint`: a muted line under the value |
| `ANDROID/ui/kit/SegmentedControl.kt` | Segments share one height when a label wraps |
| `ANDROID/ui/kit/ItemRow.kt` | `tileModifier`: the tile can travel |
| `ANDROID/ui/kit/HavenSheet.kt` | At most 90% of the window, scrolls when taller, rises over the keyboard |
| `ANDROID/ui/kit/HavenDialog.kt` | `alternative` (stacked answers), `dismissible` |
| `ANDROID/ui/kit/ChoiceSheet.kt` (new) | `ChoiceSheet`, `ChoiceRow` |
| `ANDROID/ui/settings/SettingsChoices.kt` | Uses the kit's `ChoiceSheet` |
| `apps/android/app/src/debug/…/catalogue/KitCatalogue.kt`, `CatalogueComponents.kt` | The catalogue shows the new kit pieces |
| `ANDROID/ui/components/ScreenBar.kt` (new) | `ScreenBar`, `OfflineNote` |
| `ANDROID/ui/components/SecretText.kt` (new) | `MASK`, `colourised`, `MaskedValue`, `RevealedValue` |
| `ANDROID/ui/item/DetailRows.kt` (new) | `ShownRow`, `SecretRow`, `CodeRow`, `groupedCode` |
| `ANDROID/ui/item/ItemScreen.kt` | The item screen from the kit |
| `ANDROID/ui/shell/SummaryRow.kt`, `ANDROID/ui/nav/SharedMotion.kt`, `HavenNavHost.kt` | `SharedPart`: title and tile travel |
| `ANDROID/ui/edit/DraftText.kt` (new), `WebsiteEditors.kt` (new), `EditFields.kt`, `EditScreen.kt` | Editors from the kit |
| `ANDROID/ui/generator/GeneratorScreen.kt` | Generator from the kit |
| `ANDROID/ui/unlock/UnlockScreen.kt` | Unlock from the kit; `UnlockForm` internal for tests |
| `ANDROID/ui/onboarding/OnboardingScreen.kt`, `KitScanner.kt` | Onboarding from the kit |
| `ANDROID/ui/settings/DevicesScreen.kt`, `ANDROID/ui/autofillsetup/AutofillSetupScreen.kt` | Devices and autofill setup from the kit |
| `ANDROID/autofill/AutofillSearchScreen.kt` (new), `AutofillSearchActivity.kt`, `WalletConfirmDialog.kt`, `RES/drawable/ic_autofill_copy.xml` | "Search HavenKeys…", the confirmation, the copy-row glyph |
| `ANDROID/credentials/PasskeyCreateScreen.kt`, `AndroidManifest.xml` | The passkey sheet |
| `ANDROID/ui/components/HavenTopBar.kt`, `SecretField.kt`, `ANDROID/ui/item/TotpRing.kt` | Deleted |
| `apps/android/app/build.gradle.kts` | `forbidMaterialInKit` widened task by task, then to everything but the bridge |
| `RES/values/strings.xml`, `RES/values-pt-rBR/strings.xml` | `edit_match`; unused strings removed |
| `apps/android/app/src/testDebug/…/screens/ScreenScreenshots.kt` (new) | PNGs of the rebuilt screens for `/impeccable` |
| `apps/android/DESIGN.md`, `docs/android.md`, plan index | Kit additions, review notes, device checks, stage status |

---
### Task 1: The app's navigation under test, and taps mid-transition that reach their destination

The controller asked for this first: before any screen is rebuilt, a Robolectric test drives the real `HavenNavHost` and pins the lock wipe (from an item, from search, from a category list) and Back through the Items tab, so every later task runs against it. `HavenNavHost` takes the `AppContainer`, which needs Rust and the Keystore; this task puts a seam (`NavServices`) between them without changing what the app does.

It also replaces stage 3's double-tap guard. `NavHostController.ifSettled` (in `HavenNavHost.kt`) ignores a push unless the current entry is `RESUMED`, which swallows a tap made during a push, pop or crossfade, while spec §7 says "a tap mid-transition goes straight to its destination". The replacement de-duplicates by destination: `pushOnce(route)` pushes unless the entry on top is that same route with the same arguments, so two quick taps on one row (or the pill, a tile, Generate) open it once, while a tap on a different target during a transition still navigates.

**Re-read first:** stage 3's final fixes changed `ANDROID/ui/nav/HavenNavHost.kt` (`ifSettled`, `Nav.ifSettled`, `launchSingleTop` for `Routes.SEARCH`) and added `ANDROID_TEST/ui/nav/DoubleTapTest.kt`. Read both as they are now; this task replaces `ifSettled` wherever it is called and keeps everything else those fixes did.

**Files:**
- Create: `ANDROID/ui/nav/NavServices.kt`
- Modify: `ANDROID/ui/nav/HavenNavHost.kt` (the seam; `ifSettled` replaced by `pushOnce`), `ANDROID/ui/nav/ShellDestinations.kt`
- Modify: `docs/android.md` (the stage 3 checklist line on taps during a transition)
- Test: `ANDROID_TEST/ui/nav/HavenNavHostTest.kt` (new), `ANDROID_TEST/ui/nav/DoubleTapTest.kt` (rewritten for `pushOnce`)

**Interfaces:**
- Consumes: the repositories' interfaces, `VaultEventsHub`, `SensitiveClipboard`, `SettingsActions`, `rememberSettingsActions(container, activity)`, `UnlockScreen(viewModel, activity, container, onUnlocked)`, `UnlockViewModel`.
- Produces (package `net.havenkeys.android.ui.nav`):
  - `class NavServices(vault: VaultRepository, accounts: AccountRepository, settings: SettingsRepository, events: VaultEventsHub, clipboard: SensitiveClipboard, hasBiometricUnlock: () -> Boolean, unlockScreen: @Composable (onUnlocked: () -> Unit) -> Unit, settingsActions: @Composable () -> SettingsActions)`
  - `@Composable internal fun rememberNavServices(container: AppContainer, activity: FragmentActivity): NavServices`
  - `@Composable internal fun HavenNavHost(services: NavServices, modifier: Modifier = Modifier, navController: NavHostController = rememberNavController())`; the public `HavenNavHost(container, modifier)` keeps its signature and calls it.
  - `internal fun shellScreens(services: NavServices, navController: NavHostController, open: OpenItem, sharedTitle: SharedTitle): ShellScreens`
  - `internal fun NavHostController.pushOnce(route: String)` and `internal fun NavBackStackEntry.concreteRoute(): String?`; `ifSettled` is deleted.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/nav/HavenNavHostTest.kt`:

```kotlin
package net.havenkeys.android.ui.nav

import androidx.activity.ComponentActivity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.status
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.removeAnimations
import net.havenkeys.android.ui.settings.SettingsActions
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.LockState

/** The real app NavHost and shell over fakes; Unlock is a stand-in button that unlocks the fake vault. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class HavenNavHostTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam", null, false, false, 0, 0)
    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github))
        recent = Outcome.Ok(listOf(github))
        view = Outcome.Ok(ItemView(github, emptyList()))
    }
    private lateinit var nav: NavHostController

    private val services = NavServices(
        vault = vault,
        accounts = FakeAccountRepository(),
        settings = FakeSettingsRepository(),
        events = events,
        clipboard = SensitiveClipboard(RuntimeEnvironment.getApplication(), CoroutineScope(SupervisorJob())),
        hasBiometricUnlock = { false },
        unlockScreen = { onUnlocked ->
            HavenButton(
                UNLOCK,
                onClick = {
                    vault.nextStatus = Outcome.Ok(status())
                    events.unlocked()
                    onUnlocked()
                },
            )
        },
        settingsActions = { SettingsActions(false, {}, { Outcome.Ok(Unit) }) },
    )

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    @Before
    fun show() {
        removeAnimations()
        rule.setContent {
            HavenTheme {
                nav = rememberNavController()
                HavenNavHost(services, navController = nav)
            }
        }
        rule.waitForIdle()
    }

    @After
    fun animationsBack() = removeAnimations(false)

    private fun lock() {
        vault.nextStatus = Outcome.Ok(status(LockState.LOCKED))
        events.locked("user")
        rule.waitForIdle()
    }

    private fun assertUnlockWithNothingBehind() {
        rule.onNodeWithText(UNLOCK).assertIsDisplayed()
        rule.runOnIdle {
            assertEquals(Routes.UNLOCK, nav.currentDestination?.route)
            assertNull(nav.previousBackStackEntry)
        }
    }

    private fun back() {
        rule.runOnIdle { rule.activity.onBackPressedDispatcher.onBackPressed() }
        rule.waitForIdle()
    }

    private fun openLogins() {
        rule.onNode(hasText(text(R.string.tab_items)) and hasRole(Role.Tab)).performClick()
        rule.onNode(hasText(text(Category.LOGINS.label)) and hasClickAction()).performClick()
        rule.onNode(hasText(text(Category.LOGINS.label)) and isHeading()).assertIsDisplayed()
    }

    @Test
    fun lockingFromAnItemLeavesOnlyUnlock() {
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(Routes.ITEM, nav.currentDestination?.route) }
        lock()
        assertUnlockWithNothingBehind()
    }

    @Test
    fun lockingFromSearchLeavesOnlyUnlock() {
        rule.onNode(hasText(text(R.string.shell_search)) and hasClickAction()).performClick()
        rule.onNode(hasSetTextAction()).performTextInput("git")
        rule.runOnIdle { assertEquals(Routes.SEARCH, nav.currentDestination?.route) }
        lock()
        assertUnlockWithNothingBehind()
    }

    @Test
    fun lockingFromACategoryListLeavesOnlyUnlockAndUnlockingOpensAFreshHome() {
        openLogins()
        lock()
        assertUnlockWithNothingBehind()
        rule.onNodeWithText(UNLOCK).performClick()
        rule.waitForIdle()
        rule.onNode(hasText(text(R.string.home_recent)) and isHeading()).assertIsDisplayed()
        rule.onAllNodes(hasText(text(Category.LOGINS.label)) and isHeading()).assertCountEquals(0)
        rule.runOnIdle {
            assertEquals(Routes.SHELL, nav.currentDestination?.route)
            assertNull(nav.previousBackStackEntry)
        }
    }

    @Test
    fun backFromACategoryGoesToTheItemsRootThenHome() {
        openLogins()
        back()
        rule.onNode(hasText(text(R.string.tab_items)) and isHeading()).assertIsDisplayed()
        back()
        rule.onNode(hasText(text(R.string.home_recent)) and isHeading()).assertIsDisplayed()
    }

    private companion object {
        const val UNLOCK = "Unlock (test)"
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HavenNavHostTest*'`
Expected: compile errors (`NavServices`, `HavenNavHost(services, …)` unresolved).

- [ ] **Step 3: Write `NavServices.kt`**

```kotlin
package net.havenkeys.android.ui.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.viewmodel.compose.viewModel
import net.havenkeys.android.AppContainer
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import net.havenkeys.android.ui.settings.SettingsActions
import net.havenkeys.android.ui.settings.rememberSettingsActions
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel

/**
 * What the app's navigation needs from the app. [rememberNavServices] builds
 * it from the AppContainer and the activity; a test builds it from fakes, so
 * the lock wipe runs through the real NavHost without Rust or a Keystore.
 * [unlockScreen] and [settingsActions] are the two places that need the
 * activity (biometric prompts).
 */
class NavServices(
    val vault: VaultRepository,
    val accounts: AccountRepository,
    val settings: SettingsRepository,
    val events: VaultEventsHub,
    val clipboard: SensitiveClipboard,
    val hasBiometricUnlock: () -> Boolean,
    val unlockScreen: @Composable (onUnlocked: () -> Unit) -> Unit,
    val settingsActions: @Composable () -> SettingsActions,
)

@Composable
internal fun rememberNavServices(container: AppContainer, activity: FragmentActivity): NavServices =
    remember(container, activity) {
        NavServices(
            vault = container.vaultRepository,
            accounts = container.accountRepository,
            settings = container.settingsRepository,
            events = container.events,
            clipboard = container.clipboard,
            hasBiometricUnlock = container::hasBiometricUnlock,
            unlockScreen = { onUnlocked ->
                UnlockScreen(
                    viewModel = viewModel {
                        UnlockViewModel(
                            container.vaultRepository,
                            biometricAvailable = container.biometricGate.available(activity),
                            hasBundle = container::hasBiometricUnlock,
                            deleteBundle = container::forgetBiometricUnlock,
                        )
                    },
                    activity = activity,
                    container = container,
                    onUnlocked = onUnlocked,
                )
            },
            settingsActions = { rememberSettingsActions(container, activity) },
        )
    }
```

(`viewModel { }` runs where `unlockScreen` is invoked, inside the Unlock destination, so the ViewModel still belongs to that back stack entry, as before.)

- [ ] **Step 4: The NavHost reads the services**

In `ANDROID/ui/nav/HavenNavHost.kt`:

1. Split the public entry point:

```kotlin
@Composable
fun HavenNavHost(container: AppContainer, modifier: Modifier = Modifier) {
    val activity = requireNotNull(LocalActivity.current as? FragmentActivity)
    HavenNavHost(rememberNavServices(container, activity), modifier)
}

/**
 * The app's navigation (the KDoc that was on the public function moves here,
 * unchanged). [navController] is a parameter so a test can watch the back stack.
 */
@Composable
internal fun HavenNavHost(
    services: NavServices,
    modifier: Modifier = Modifier,
    navController: NavHostController = rememberNavController(),
) {
```

and in the body: `RootViewModel(services.vault, services.events)`; delete the body's own `val navController = rememberNavController()` (the parameter replaces it; it is created before the early return, which is fine); `Nav(services, navController, this, motion, travel)`.

2. In `Nav`, replace the `container` and `activity` properties with `val services: NavServices`, and `val lock: () -> Unit = services.vault::lock`. (Step 4b replaces stage 3's `ifSettled` guard.)

3. In every builder function, replace `nav.container.vaultRepository` / `accountRepository` / `settingsRepository` / `events` / `clipboard` with `nav.services.vault` / `accounts` / `settings` / `events` / `clipboard` (write `val services = nav.services` at the top of each instead of `val container = nav.container`). In `entryScreens`, the Unlock destination becomes:

```kotlin
    composable(Routes.UNLOCK) {
        nav.services.unlockScreen { nav.controller.replaceAll(Routes.SHELL) }
    }
```

and Onboarding takes `OnboardingViewModel(nav.services.accounts)`. The shell destination calls `shellScreens(nav.services, nav.controller, nav.open, nav.travel.from(nav.shared, this, nav.motion))`. Drop the imports that are no longer used (`AppContainer` stays for the public overload; `UnlockViewModel`, `UnlockScreen` move to `NavServices.kt`).

4. In `ANDROID/ui/nav/ShellDestinations.kt`, change the signature to `internal fun shellScreens(services: NavServices, navController: NavHostController, open: OpenItem, sharedTitle: SharedTitle): ShellScreens`, read `services.vault`, `services.accounts`, `services.settings`, `services.events.online`, `biometricEnrolled = services.hasBiometricUnlock`, and `actions = services.settingsActions()` instead of the container and `rememberSettingsActions(container, activity)`.

- [ ] **Step 4b: Replace `ifSettled` with `pushOnce`**

Rewrite `ANDROID_TEST/ui/nav/DoubleTapTest.kt`:

```kotlin
package net.havenkeys.android.ui.nav

import androidx.compose.foundation.layout.Box
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Pushes made in one frame: the second lands while the first is still moving in (spec §7). */
@RunWith(RobolectricTestRunner::class)
class DoubleTapTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var nav: NavHostController

    @Before
    fun show() {
        rule.setKit {
            nav = rememberNavController()
            NavHost(nav, startDestination = Routes.SHELL) {
                composable(Routes.SHELL) { Box {} }
                composable(Routes.SEARCH) { Box {} }
                composable(Routes.GENERATOR) { Box {} }
                composable(Routes.ITEM, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
                    Box {}
                }
            }
        }
        rule.waitForIdle()
    }

    private fun stack() = nav.currentBackStack.value.mapNotNull { it.concreteRoute() }.filter { it != Routes.SHELL }

    @Test
    fun twoQuickTapsOnOneTargetOpenItOnce() {
        rule.runOnIdle { repeat(2) { nav.pushOnce(Routes.SEARCH) } }
        rule.runOnIdle { repeat(2) { nav.pushOnce(Routes.item("1")) } }
        rule.waitForIdle()
        assertEquals(listOf(Routes.SEARCH, "item/1"), stack())
    }

    @Test
    fun aTapOnAnotherTargetDuringAPushStillGoesThere() {
        rule.runOnIdle {
            nav.pushOnce(Routes.item("1"))
            nav.pushOnce(Routes.GENERATOR)
        }
        rule.waitForIdle()
        assertEquals(listOf("item/1", Routes.GENERATOR), stack())
    }

    @Test
    fun anotherItemIsAnotherTarget() {
        rule.runOnIdle {
            nav.pushOnce(Routes.item("1"))
            nav.pushOnce(Routes.item("2"))
        }
        rule.waitForIdle()
        assertEquals(listOf("item/1", "item/2"), stack())
    }

    @Test
    fun aRouteIsReadBackWithItsArguments() {
        rule.runOnIdle { nav.pushOnce(Routes.item("abc")) }
        rule.waitForIdle()
        assertEquals("item/abc", nav.currentBackStackEntry?.concreteRoute())
    }
}
```

In `ANDROID/ui/nav/HavenNavHost.kt`, delete `NavHostController.ifSettled` and `Nav.ifSettled`, and add in their place:

```kotlin
/**
 * Pushes [route] unless the screen on top is already that route with the
 * same arguments. Two quick taps on one target open it once; a tap on
 * another target goes there at once, even mid-transition (spec §7). The
 * back stack changes synchronously, so the second tap of a pair sees the
 * first one's entry on top.
 */
internal fun NavHostController.pushOnce(route: String) {
    if (currentBackStackEntry?.concreteRoute() == route) return
    navigate(route)
}

/** This entry's route with its arguments filled in ("item/{id}" → "item/abc"). Ids and kinds only. */
internal fun NavBackStackEntry.concreteRoute(): String? {
    val pattern = destination.route ?: return null
    return ArgumentSlot.replace(pattern) { slot -> arguments?.getString(slot.groupValues[1]) ?: slot.value }
}

private val ArgumentSlot = Regex("\\{([^}]+)\\}")
```

(import `androidx.navigation.NavBackStackEntry`; drop `androidx.lifecycle.Lifecycle` if nothing else uses it). Then every push the user starts goes through it. `Nav.open` becomes:

```kotlin
    val open: OpenItem = { id, origin ->
        travel.tap(id, origin)
        controller.pushOnce(Routes.item(id))
    }
```

The shell's navigation becomes `onSearch = { nav.controller.pushOnce(Routes.SEARCH) }`, `onNew = { kind -> nav.controller.pushOnce(Routes.new(kind)) }`, `onGenerator = { nav.controller.pushOnce(Routes.GENERATOR) }`; the item screen's `onEdit = { nav.controller.pushOnce(Routes.edit(id)) }`; and in `ShellDestinations.kt` Settings' `onDevices = { navController.pushOnce(Routes.DEVICES) }`, `onAutofillSetup = { navController.pushOnce(Routes.AUTOFILL_SETUP) }`. The editor's `onDone` (a replacement with `popUpTo`) and the lock's `replaceAll` keep `navigate`. Check: `grep -rn "ifSettled\|launchSingleTop" apps/android/app/src` prints nothing.

In `docs/android.md`, in "### Redesign stage 3 (shell)", replace the sentence "A row tapped during a tab crossfade or a push opens at once." with "A row tapped during a tab crossfade, a push or a pop opens at once; the same row (or the search pill, an add tile, Generate) tapped twice quickly opens once."

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, `HavenNavHostTest` and the rewritten `DoubleTapTest` included; `RootViewModelTest`, `RoutesTest`, `ShellNavHostTest` unchanged. If a lock test finds Unlock but a previous entry remains, the wipe is broken: fix `HavenNavHost`, never the test.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/nav apps/android/app/src/test/kotlin/net/havenkeys/android/ui/nav docs/android.md
git commit -m "fix(android): a tap mid-transition reaches its destination and a double tap opens once; the lock wipe tested through the real NavHost"
```

---

### Task 2: Kit: read-only rows as one stop, field hints, equal-height segments, a tile modifier

**Files:**
- Modify: `ANDROID/ui/kit/GroupRow.kt`, `ANDROID/ui/kit/HavenTextField.kt`, `ANDROID/ui/kit/SecretTextField.kt`, `ANDROID/ui/kit/SegmentedControl.kt`, `ANDROID/ui/kit/ItemRow.kt`
- Test: `ANDROID_TEST/ui/kit/StructureTest.kt`, `ANDROID_TEST/ui/kit/TextFieldsTest.kt`, `ANDROID_TEST/ui/kit/ChoicesTest.kt`

**Interfaces:**
- Consumes: the kit as shipped (Entry check 2).
- Produces:
  - `GroupRow`: with `onClick == null`, its text column merges its descendants (one TalkBack stop); trailing controls stay separate nodes. Signature unchanged.
  - `HavenTextField(…, inputTransformation: InputTransformation? = null, hint: String? = null)`
  - `SecretTextField(…, onKeyboardAction: KeyboardActionHandler? = null, hint: String? = null)`
  - internal `FieldRow(label: String, error: String?, focused: Boolean, hint: String? = null, trailing: (@Composable () -> Unit)? = null, field: @Composable () -> Unit)`
  - `SegmentedControl`: every segment as tall as the tallest. Signature unchanged.
  - `ItemRow(title, subtitle, leading, onClick, modifier, titleModifier, tileModifier: Modifier = Modifier, hasPasskey, hasCode)`

- [ ] **Step 1: Write the failing tests**

Add to `ANDROID_TEST/ui/kit/StructureTest.kt` (imports: `androidx.compose.ui.layout.onSizeChanged`, `androidx.compose.ui.test.onNodeWithContentDescription`, `androidx.compose.ui.unit.IntSize`, `net.havenkeys.android.ui.theme.HavenSpacing`):

```kotlin
    @Test
    fun aReadOnlyRowReadsItsLabelAndValueAsOneStopAndKeepsItsCopyButton() {
        rule.setKit {
            InsetGroup {
                row {
                    GroupRow(trailing = { CopyButton("Username", onCopy = {}) }) {
                        GroupRowField("Username", "sam@example.com")
                    }
                }
            }
        }
        rule.onNode(hasText("Username") and hasText("sam@example.com")).assertExists()
        rule.onNodeWithContentDescription("Copy Username").assert(hasRole(Role.Button))
    }

    @Test
    fun anItemRowsTileTakesItsOwnModifier() {
        var tile = IntSize.Zero
        rule.setKit {
            ItemRow(
                "GitHub",
                null,
                RowLeading.Monogram("GitHub"),
                onClick = {},
                tileModifier = Modifier.onSizeChanged { tile = it },
            )
        }
        rule.runOnIdle { assertEquals(with(rule.density) { HavenSpacing.tile.roundToPx() }, tile.width) }
    }
```

Add to `ANDROID_TEST/ui/kit/TextFieldsTest.kt`:

```kotlin
    @Test
    fun aHintIsReadWithTheFieldAndAnErrorTakesItsPlace() {
        var error by mutableStateOf<String?>(null)
        rule.setKit {
            SecretTextField(
                TextFieldState(),
                "Secret Key",
                revealed = false,
                onRevealChange = {},
                error = error,
                hint = "On your Emergency Kit",
            )
        }
        val field = rule.onNode(hasSetTextAction())
        assertTrue(texts(field).contains("On your Emergency Kit"))
        error = "Check the Secret Key"
        rule.waitForIdle()
        assertTrue(texts(field).none { it == "On your Emergency Kit" })
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Check the Secret Key"))
    }

    @Test
    fun anOrdinaryFieldShowsItsHintToo() {
        rule.setKit { HavenTextField(TextFieldState(), "Server", hint = "The address your server answers on") }
        assertTrue(texts(rule.onNode(hasSetTextAction())).contains("The address your server answers on"))
    }
```

Add to `ANDROID_TEST/ui/kit/ChoicesTest.kt` (imports: `androidx.compose.foundation.layout.Box`, `androidx.compose.foundation.layout.width`, `androidx.compose.ui.Modifier`):

```kotlin
    @Test
    fun segmentsKeepOneHeightWhenALabelWraps() {
        rule.setKit(fontScale = 1.5f) {
            Box(Modifier.width(320.dp)) {
                SegmentedControl(listOf("Site inteiro, qualquer subdomínio", "Somente este site", "Página"), 0, {})
            }
        }
        val heights = rule.onAllNodes(hasRole(Role.Tab)).fetchSemanticsNodes().map { it.size.height }
        assertEquals(3, heights.size)
        assertEquals(1, heights.distinct().size)
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*StructureTest*' --tests '*TextFieldsTest*' --tests '*ChoicesTest*'`
Expected: compile errors (`tileModifier`, `hint` unknown); once those exist, `aReadOnlyRow…` and `segmentsKeep…` fail on behaviour.

- [ ] **Step 3: GroupRow merges a read-only row's text**

In `ANDROID/ui/kit/GroupRow.kt` (import `androidx.compose.ui.semantics.semantics`), replace the KDoc's last sentence with "A tappable row is one button for TalkBack; a read-only row's text (a label and its value) is one stop, and its trailing controls stay their own." Replace the `Column(Modifier.weight(1f), …)` line with:

```kotlin
        // Read-only: label and value are one TalkBack stop; Copy, Show and the like stay separate.
        val text = if (onClick == null) Modifier.semantics(mergeDescendants = true) {} else Modifier
        Column(
            Modifier.weight(1f).then(text),
            verticalArrangement = Arrangement.spacedBy(2.dp),
            content = content,
        )
```

- [ ] **Step 4: Fields take a hint**

In `ANDROID/ui/kit/HavenTextField.kt`, add `hint: String? = null,` as the last parameter of `HavenTextField`, add to its KDoc "[hint] is a muted line under the value, read with the field; an error takes its place.", and change its decorator call to `FieldRow(label, error, focused, hint) {`. Change `FieldRow`:

```kotlin
@Composable
internal fun FieldRow(
    label: String,
    error: String?,
    focused: Boolean,
    hint: String? = null,
    trailing: (@Composable () -> Unit)? = null,
    field: @Composable () -> Unit,
) {
```

and replace its error block with:

```kotlin
            if (error != null) {
                HavenText(error, Modifier.clearAndSetSemantics {}, style = HavenTheme.type.label, color = colors.danger)
            } else if (hint != null) {
                // Not cleared: it merges into the field's node, so TalkBack reads it after the text.
                HavenText(hint, style = HavenTheme.type.label, color = colors.muted)
            }
```

In `ANDROID/ui/kit/SecretTextField.kt`, add `hint: String? = null,` as the last parameter and pass `hint = hint,` in the `FieldRow(…)` call (it already uses named arguments).

- [ ] **Step 5: Segments share one height**

In `ANDROID/ui/kit/SegmentedControl.kt` (imports `androidx.compose.foundation.layout.IntrinsicSize`, `androidx.compose.foundation.layout.fillMaxHeight`, `androidx.compose.foundation.layout.height`), add `.height(IntrinsicSize.Min)` right after `.fillMaxWidth()` on the `Row`, and `.fillMaxHeight()` right after `.weight(1f)` on each segment's `Box`. Add to the KDoc: "When a label wraps (pt-BR, a large font), every segment takes the tallest one's height."

- [ ] **Step 6: The tile takes a modifier**

In `ANDROID/ui/kit/ItemRow.kt`, add `tileModifier: Modifier = Modifier,` after `titleModifier`, change `ItemTile(leading)` to `ItemTile(leading, tileModifier)`, and extend the KDoc: "[titleModifier] and [tileModifier] let the title and the tile travel to the detail screen."

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass (the Settings, Home and search tests too: merging only joins a read-only row's own text).

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit
git commit -m "feat(android): kit read-only rows read as one stop, fields take a hint, segments share a height, item rows take a tile modifier"
```

---

### Task 3: Kit: tall sheets, a dialog's third answer and undismissible mode, the choice sheet

**Files:**
- Modify: `ANDROID/ui/kit/HavenSheet.kt`, `ANDROID/ui/kit/HavenDialog.kt`
- Create: `ANDROID/ui/kit/ChoiceSheet.kt`
- Modify: `ANDROID/ui/settings/SettingsChoices.kt` (use the kit's `ChoiceSheet`)
- Modify: `apps/android/app/src/debug/kotlin/net/havenkeys/android/catalogue/KitCatalogue.kt`, `CatalogueComponents.kt`
- Test: `ANDROID_TEST/ui/kit/OverlaysTest.kt`, `ANDROID_TEST/ui/kit/ChoiceSheetTest.kt` (new)

**Interfaces:**
- Consumes: Task 2's kit.
- Produces (package `net.havenkeys.android.ui.kit`):
  - `HavenSheet`: unchanged signature; at most `SHEET_MAX_FRACTION` (0.9) of the window's height, its content scrolls when taller, it rises above the keyboard.
  - internal `SheetSurface(title: String?, modifier: Modifier = Modifier, maxHeight: Dp = Dp.Unspecified, content: @Composable ColumnScope.() -> Unit)`
  - `HavenDialog(…, answerKey: Any? = null, alternative: DialogAction? = null, dismissible: Boolean = true)`; internal `DialogSurface(…, answerKey: Any? = null, alternative: DialogAction? = null)`.
  - `fun <T> ChoiceSheet(title: String, values: List<T>, selected: T, label: @Composable (T) -> String, onSelect: (T) -> Unit, onDismiss: () -> Unit)`
  - `fun ChoiceRow(label: String, selected: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier, detail: String? = null)`

- [ ] **Step 1: Write the failing tests**

Add to `ANDROID_TEST/ui/kit/OverlaysTest.kt` (imports: `androidx.compose.foundation.layout.heightIn`, `androidx.compose.ui.Modifier`, `androidx.compose.ui.test.assertIsEnabled`, `androidx.compose.ui.test.performScrollTo`, `androidx.compose.ui.unit.dp`, `org.junit.Assert.assertTrue`):

```kotlin
    @Test
    fun aTallSheetStopsShortOfTheTopAndScrollsToItsLastRow() {
        rule.setKit {
            HavenSheet(onDismiss = {}, title = "Save to") {
                repeat(TALL_ROWS) { HavenText("Login $it", Modifier.heightIn(min = 48.dp)) }
            }
        }
        rule.waitForIdle()
        assertTrue(rule.onNodeWithTag(SHEET_TAG).fetchSemanticsNode().boundsInRoot.top > 0f)
        rule.onNodeWithText("Login ${TALL_ROWS - 1}").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun aShortSheetStillClosesWhenDraggedFromItsContent() {
        showSheet()
        rule.onNodeWithText("Login").performTouchInput { swipeDown(startY = centerY, endY = centerY + 2_000f) }
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun aThirdAnswerStacksTheButtonsAndAnswersOnce() {
        val answers = mutableListOf<String>()
        rule.setKit {
            HavenDialog(
                title = "Fill your identity?",
                onDismiss = { answers += "dismiss" },
                confirm = DialogAction("Fill with documents", { answers += "confirm" }),
                dismiss = DialogAction("Cancel", { answers += "cancel" }),
                alternative = DialogAction("Fill without documents", { answers += "alternative" }),
            )
        }
        rule.onNodeWithText("Fill without documents").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.onNodeWithText("Fill with documents").performClick()
        pressBack()
        rule.waitForIdle()
        assertEquals(listOf("alternative"), answers)
    }

    @Test
    fun anUndismissibleDialogIgnoresBackAndStillAnswers() {
        var reloads = 0
        var backs = 0
        rule.setKit {
            HavenDialog(
                title = "Changed on another device",
                onDismiss = { backs++ },
                confirm = DialogAction("Reload", { reloads++ }),
                dismissible = false,
            )
        }
        pressBack()
        rule.onNodeWithText("Reload").assertIsEnabled().performClick()
        assertEquals(0, backs)
        assertEquals(1, reloads)
    }

    private companion object {
        const val TALL_ROWS = 60
    }
```

Create `ANDROID_TEST/ui/kit/ChoiceSheetTest.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChoiceSheetTest {
    @get:Rule
    val rule = createComposeRule()

    private val picked = mutableListOf<Int>()
    private var closed = 0

    private fun show() = rule.setKit {
        ChoiceSheet(
            title = "Lock automatically",
            values = listOf(1, 5, 15),
            selected = 5,
            label = { "After $it minutes" },
            onSelect = { picked += it },
            onDismiss = { closed++ },
        )
    }

    @Test
    fun eachValueIsARadioButtonAndTheCurrentOneIsSelected() {
        show()
        rule.onNode(hasText("After 5 minutes") and hasRole(Role.RadioButton)).assertIsSelected().assertTouchTarget()
        rule.onNode(hasText("After 1 minutes") and hasRole(Role.RadioButton)).assertIsNotSelected()
    }

    @Test
    fun pickingAnotherValueClosesThenSaves() {
        show()
        rule.onNodeWithText("After 15 minutes").performClick()
        assertEquals(listOf(15), picked)
        assertEquals(1, closed)
    }

    @Test
    fun pickingTheCurrentValueOnlyCloses() {
        show()
        rule.onNodeWithText("After 5 minutes").performClick()
        assertEquals(emptyList<Int>(), picked)
        assertEquals(1, closed)
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*OverlaysTest*' --tests '*ChoiceSheetTest*'`
Expected: compile errors (`alternative`, `dismissible`, `ChoiceSheet` unknown).

- [ ] **Step 3: Tall sheets**

In `ANDROID/ui/kit/HavenSheet.kt` (imports `androidx.compose.foundation.layout.BoxWithConstraints`, `androidx.compose.foundation.layout.heightIn`, `androidx.compose.foundation.layout.imePadding`, `androidx.compose.foundation.rememberScrollState`, `androidx.compose.foundation.verticalScroll`, `androidx.compose.ui.unit.Dp`, `androidx.compose.ui.unit.isSpecified`), add under `DRAG_DISMISS_FRACTION`:

```kotlin
/** A sheet never covers more of the window than this; taller content scrolls inside it. */
private const val SHEET_MAX_FRACTION = 0.9f
```

Add to `HavenSheet`'s KDoc: "It takes at most 90% of the window's height (taller content scrolls inside it) and rises above the keyboard."

In `SheetFrame`, replace `Box(Modifier.fillMaxSize()) {` with:

```kotlin
    BoxWithConstraints(Modifier.fillMaxSize().imePadding()) {
        val tallest = maxHeight * SHEET_MAX_FRACTION
```

(the backdrop `Box` and the `SheetSurface` stay inside it), and in the `SheetSurface(…)` call pass `maxHeight = tallest,` between the modifier chain and `content`, written with named arguments:

```kotlin
        SheetSurface(
            title = title,
            modifier = modifier
                .align(Alignment.BottomCenter)
                // …the existing chain, unchanged…
                .testTag(SHEET_TAG),
            maxHeight = tallest,
            content = content,
        )
```

Replace `SheetSurface` with:

```kotlin
/**
 * The sheet as it draws, without its window: the catalogue shows it inline.
 * With a [maxHeight] (the window's sheet) it stops there and its content
 * scrolls; inline it takes its content's height.
 */
@Composable
internal fun SheetSurface(
    title: String?,
    modifier: Modifier = Modifier,
    maxHeight: Dp = Dp.Unspecified,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = HavenTheme.colors
    val capped = maxHeight.isSpecified
    Column(
        modifier
            .fillMaxWidth()
            .then(if (capped) Modifier.heightIn(max = maxHeight) else Modifier)
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
        if (capped) {
            val scroll = rememberScrollState()
            // Only a sheet taller than its cap scrolls; a short one still drags down from anywhere.
            val scrolls = scroll.maxValue in 1 until Int.MAX_VALUE
            Column(Modifier.weight(1f, fill = false).verticalScroll(scroll, enabled = scrolls), content = content)
        } else {
            content()
        }
    }
}
```

- [ ] **Step 4: The dialog's third answer and undismissible mode**

In `ANDROID/ui/kit/HavenDialog.kt`:

1. Add to `HavenDialog`'s parameters, after `answerKey`: `alternative: DialogAction? = null,` and `dismissible: Boolean = true,`. Add to its KDoc: "With an [alternative] (a second way to say yes) the three buttons stack full width: confirm, alternative, dismiss. With [dismissible] false, Back and an outside tap do nothing; only a button answers."
2. Replace its `Dialog(…)` opening with:

```kotlin
    Dialog(
        onDismissRequest = { if (!busy && dismissible) answer.run(onDismiss) },
        properties = DialogProperties(
            dismissOnBackPress = dismissible,
            dismissOnClickOutside = dismissible,
            securePolicy = SecureFlagPolicy.SecureOn,
        ),
    ) {
```

and pass `alternative` to `DialogContent(title, confirm, modifier, message, dismiss, content, confirmEnabled, busy, answer, alternative)`.
3. Add `alternative: DialogAction? = null,` to `DialogSurface` (after `answerKey`) and pass it to `DialogContent`. `DialogSurface` gets no `dismissible`: drawn inline it has no window to dismiss.
4. Give `DialogContent` a last parameter `alternative: DialogAction?` and replace its button `Row(…) { … }` with:

```kotlin
        val enabled = !answered && !busy
        if (alternative == null) {
            AnswerRow(confirm, dismiss, ::once, enabled, confirmEnabled, busy)
        } else {
            AnswerStack(confirm, alternative, dismiss, ::once, enabled, confirmEnabled, busy)
        }
```

with `fun once(…)` kept as it is (a local function; `::once` passes it), and add below `DialogContent`:

```kotlin
/** Confirm and dismiss side by side, at the end. */
@Suppress("LongParameterList")
@Composable
private fun ColumnScope.AnswerRow(
    confirm: DialogAction,
    dismiss: DialogAction?,
    once: (() -> Unit) -> () -> Unit,
    enabled: Boolean,
    confirmEnabled: Boolean,
    busy: Boolean,
) {
    Row(
        Modifier.align(Alignment.End).padding(top = 24.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (dismiss != null) HavenButton(dismiss.label, once(dismiss.onClick), style = ButtonStyle.Quiet, enabled = enabled)
        if (busy) ProgressRing(progress = null, size = 20.dp)
        HavenButton(
            confirm.label,
            once(confirm.onClick),
            style = if (confirm.danger) ButtonStyle.Danger else ButtonStyle.Primary,
            enabled = enabled && confirmEnabled,
        )
    }
}

/** Three answers, stacked full width so long labels wrap: confirm, the alternative, dismiss. */
@Suppress("LongParameterList")
@Composable
private fun AnswerStack(
    confirm: DialogAction,
    alternative: DialogAction,
    dismiss: DialogAction?,
    once: (() -> Unit) -> () -> Unit,
    enabled: Boolean,
    confirmEnabled: Boolean,
    busy: Boolean,
) {
    Column(
        Modifier.fillMaxWidth().padding(top = 24.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        if (busy) ProgressRing(progress = null, size = 20.dp)
        HavenButton(
            confirm.label,
            once(confirm.onClick),
            Modifier.fillMaxWidth(),
            style = if (confirm.danger) ButtonStyle.Danger else ButtonStyle.Primary,
            enabled = enabled && confirmEnabled,
        )
        HavenButton(
            alternative.label,
            once(alternative.onClick),
            Modifier.fillMaxWidth(),
            style = if (alternative.danger) ButtonStyle.Danger else ButtonStyle.Secondary,
            enabled = enabled,
        )
        if (dismiss != null) {
            HavenButton(dismiss.label, once(dismiss.onClick), Modifier.fillMaxWidth(), style = ButtonStyle.Quiet, enabled = enabled)
        }
    }
}
```

(`fun once(action: () -> Unit): () -> Unit` is the local function `DialogContent` already declares; a reference to a local function is `::once`.)

- [ ] **Step 5: The choice sheet**

Create `ANDROID/ui/kit/ChoiceSheet.kt`:

```kotlin
package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A sheet of single choices: each value a radio button for TalkBack, a brass
 * check on the current one. A pick closes the sheet at once, then saves the
 * value if it changed (the pick is the motion; the sheet does not linger).
 */
@Composable
fun <T> ChoiceSheet(
    title: String,
    values: List<T>,
    selected: T,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    onDismiss: () -> Unit,
) {
    HavenSheet(onDismiss = onDismiss, title = title) {
        InsetGroup(Modifier.selectableGroup()) {
            values.forEach { value ->
                row {
                    ChoiceRow(
                        label(value),
                        selected = value == selected,
                        onClick = {
                            onDismiss()
                            if (value != selected) onSelect(value)
                        },
                    )
                }
            }
        }
    }
}

/** One choice in an [InsetGroup] marked `selectableGroup()`: a radio button, a brass check when chosen. */
@Composable
fun ChoiceRow(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    detail: String? = null,
) {
    val colors = HavenTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .selectable(
                selected = selected,
                interactionSource = null,
                indication = HavenPress,
                role = Role.RadioButton,
                onClick = onClick,
            )
            .padding(horizontal = HavenSpacing.rowX, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) { GroupRowText(label, detail) }
        if (selected) IconGlyph(HavenIcon.Check, contentDescription = null, tint = colors.brass, size = 20.dp)
    }
}

@PreviewLightDark
@Composable
private fun ChoiceRowPreview() {
    KitPreview {
        InsetGroup(Modifier.selectableGroup()) {
            row { ChoiceRow("After 5 minutes", selected = true, onClick = {}) }
            row { ChoiceRow("New login", selected = false, onClick = {}, detail = "With this passkey only") }
        }
    }
}
```

- [ ] **Step 6: Settings uses it**

In `ANDROID/ui/settings/SettingsChoices.kt`, replace the body of `SettingsChoiceSheet` with:

```kotlin
    when (choice) {
        SettingsChoice.AUTO_LOCK -> ChoiceSheet(
            title = stringResource(R.string.settings_auto_lock),
            values = SettingsViewModel.AUTO_LOCK_CHOICES,
            selected = settings.autoLockMinutes,
            label = { autoLockText(it) },
            onSelect = viewModel::setAutoLock,
            onDismiss = onClose,
        )
        SettingsChoice.CLIPBOARD -> ChoiceSheet(
            title = stringResource(R.string.settings_clipboard),
            // A value set on the desktop outside the phone's list still shows, selected.
            values = (SettingsViewModel.CLIPBOARD_CHOICES + settings.clipboardClearSeconds).distinct().sorted(),
            selected = settings.clipboardClearSeconds,
            label = { clipboardText(it) },
            onSelect = viewModel::setClipboardSeconds,
            onDismiss = onClose,
        )
    }
```

Delete the private `Choices` class, `ChoiceSheet` and `ChoiceOption` from that file, import `net.havenkeys.android.ui.kit.ChoiceSheet`, and drop the imports nothing uses any more.

- [ ] **Step 7: The catalogue shows them**

In `apps/android/app/src/debug/kotlin/net/havenkeys/android/catalogue/KitCatalogue.kt`, change the Overlays section's list to `listOf("HavenSheet", "HavenDialog", "Toast", "HavenMenu", "ChoiceSheet")` (`CatalogueTest.everyKitComponentHasItsPlace` requires it). In `CatalogueComponents.kt`'s `OverlaysSection`, add a `var choice by remember { mutableIntStateOf(5) }` and `var choosing by remember { mutableStateOf(false) }`, add after `MenuSurface(…)`:

```kotlin
        InsetGroup(Modifier.selectableGroup()) {
            row { ChoiceRow("After 5 minutes", selected = choice == 5, onClick = { choice = 5 }) }
            row { ChoiceRow("After 15 minutes", selected = choice == 15, onClick = { choice = 15 }, detail = "Recommended") }
        }
        DialogSurface(
            title = "Fill your identity?",
            confirm = DialogAction("Fill with documents", {}),
            dismiss = DialogAction("Cancel", {}),
            alternative = DialogAction("Fill without documents", {}),
        )
```

a `HavenButton("Choice", onClick = { choosing = true }, style = ButtonStyle.Secondary)` in the second button row, and after the `if (dialog)` block:

```kotlin
    if (choosing) {
        ChoiceSheet(
            "Lock automatically",
            listOf(5, 15, 30),
            choice,
            label = { "After $it minutes" },
            onSelect = { choice = it },
            onDismiss = { choosing = false },
        )
    }
```

(imports: `androidx.compose.foundation.selection.selectableGroup`, `net.havenkeys.android.ui.kit.ChoiceRow`, `net.havenkeys.android.ui.kit.ChoiceSheet`, `androidx.compose.runtime.mutableIntStateOf`).

- [ ] **Step 8: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, including `SettingsScreenTest.aChoiceOpensASheetAndSavesThePick`, the stage 3 add-sheet tests and `CatalogueTest`.

- [ ] **Step 9: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit apps/android/app/src/main/kotlin/net/havenkeys/android/ui/settings/SettingsChoices.kt apps/android/app/src/debug apps/android/app/src/test/kotlin/net/havenkeys/android/ui/kit
git commit -m "feat(android): kit sheets stop at 90% and scroll, dialogs take a third answer or none but a button, a choice sheet"
```

---

### Task 4: The full-screen bar and how a secret value draws

**Files:**
- Create: `ANDROID/ui/components/ScreenBar.kt`, `ANDROID/ui/components/SecretText.kt`
- Modify: `ANDROID/ui/components/SecretField.kt` (its `MASK` and `colourised` move out)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers the two new files)
- Test: `ANDROID_TEST/ui/components/ScreenPartsTest.kt` (new)

**Interfaces:**
- Consumes: kit `HavenIconButton`, `Pill`, `IconGlyph`, `HavenText`; `HavenTheme.type.{masked, secret, body}`.
- Produces (package `net.havenkeys.android.ui.components`):
  - `@Composable fun ScreenBar(onBack: () -> Unit, online: Boolean, onLock: () -> Unit, modifier: Modifier = Modifier, actions: @Composable RowScope.() -> Unit = {})`
  - `@Composable fun OfflineNote(text: String, modifier: Modifier = Modifier)`
  - `internal const val MASK`, `internal fun colourised(value: String, colors: HavenColors): AnnotatedString`
  - `@Composable fun MaskedValue(label: String, modifier: Modifier = Modifier)`
  - `@Composable fun RevealedValue(value: String, modifier: Modifier = Modifier)`

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/components/ScreenPartsTest.kt`:

```kotlin
package net.havenkeys.android.ui.components

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.DarkHavenColors
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class ScreenPartsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    @Test
    fun theBarHasBackLockAndItsActionsAndSaysOfflineOnlyWhenOffline() {
        var online by mutableStateOf(true)
        val taps = mutableListOf<String>()
        rule.setKit {
            ScreenBar(onBack = { taps += "back" }, online = online, onLock = { taps += "lock" }) {
                HavenButton("Save", onClick = { taps += "save" })
            }
        }
        rule.onNodeWithContentDescription(text(R.string.item_back)).assert(hasRole(Role.Button)).assertTouchTarget()
            .performClick()
        rule.onNodeWithContentDescription(text(R.string.vault_lock_now)).assertTouchTarget().performClick()
        rule.onNodeWithText("Save").performClick()
        assertEquals(listOf("back", "lock", "save"), taps)
        rule.onAllNodesWithText(text(R.string.vault_offline)).assertCountEquals(0)
        online = false
        rule.onNodeWithText(text(R.string.vault_offline)).assertExists()
    }

    @Test
    fun aMaskedValueSaysHiddenAndNeverItsLength() {
        rule.setKit { MaskedValue("Password") }
        rule.onNodeWithContentDescription(
            RuntimeEnvironment.getApplication().getString(R.string.hidden, "Password"),
        ).assertExists()
        rule.onAllNodesWithText(MASK).assertCountEquals(0)
    }

    @Test
    fun digitsAndSymbolsTakeTheirOwnColours() {
        val shown = colourised("a1!", DarkHavenColors)
        assertEquals("a1!", shown.text)
        val colours = shown.spanStyles.associate { it.start to it.item.color }
        assertEquals(DarkHavenColors.digit, colours[1])
        assertEquals(DarkHavenColors.symbol, colours[2])
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ScreenPartsTest*'`
Expected: compile errors (`ScreenBar`, `MaskedValue` unresolved).

- [ ] **Step 3: Write `ScreenBar.kt`**

```kotlin
package net.havenkeys.android.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The bar of a full-screen screen over the shell (item, editor, generator,
 * devices, autofill setup): Back, then the offline marker, Lock and the
 * screen's own [actions]. No title: the screen's large title sits under it.
 */
@Composable
fun ScreenBar(
    onBack: () -> Unit,
    online: Boolean,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
    actions: @Composable RowScope.() -> Unit = {},
) {
    Row(
        modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HavenIconButton(HavenIcon.ChevronLeft, stringResource(R.string.item_back), onClick = onBack)
        Spacer(Modifier.weight(1f))
        if (!online) Pill(stringResource(R.string.vault_offline), Modifier.padding(end = 4.dp))
        HavenIconButton(HavenIcon.Lock, stringResource(R.string.vault_lock_now), onClick = onLock)
        actions()
    }
}

/** Why a screen cannot save now: the offline glyph and a muted line (brass never fills a surface). */
@Composable
fun OfflineNote(text: String, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    Row(
        modifier.fillMaxWidth().padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconGlyph(HavenIcon.CloudOff, contentDescription = null, tint = colors.muted, size = 18.dp)
        HavenText(text, style = HavenTheme.type.body, color = colors.muted)
    }
}
```

- [ ] **Step 4: Write `SecretText.kt` and empty `SecretField.kt` of what moved**

```kotlin
package net.havenkeys.android.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.theme.HavenColors
import net.havenkeys.android.ui.theme.HavenTheme

// Always the same count: the mask must not tell the value's length.
internal const val MASK = "••••••••••••"

/** Digits and symbols in their own colours, as on the desktop, so 0/O and l/1 read apart. */
internal fun colourised(value: String, colors: HavenColors): AnnotatedString = buildAnnotatedString {
    for (c in value) {
        when {
            c.isDigit() -> withStyle(SpanStyle(color = colors.digit)) { append(c) }
            !c.isLetter() && !c.isWhitespace() -> withStyle(SpanStyle(color = colors.symbol)) { append(c) }
            else -> append(c)
        }
    }
}

/** A hidden value: the fixed row of dots; TalkBack hears "Hidden <label>", never the value or its length. */
@Composable
fun MaskedValue(label: String, modifier: Modifier = Modifier) {
    val hidden = stringResource(R.string.hidden, label)
    HavenText(
        MASK,
        modifier.clearAndSetSemantics { contentDescription = hidden },
        style = HavenTheme.type.masked,
        color = HavenTheme.colors.muted,
        maxLines = 1,
    )
}

/**
 * A revealed secret in mono, digits and symbols coloured. Never selectable:
 * a system copy would skip the clipboard's clearing.
 */
@Composable
fun RevealedValue(value: String, modifier: Modifier = Modifier) {
    HavenText(
        colourised(value, HavenTheme.colors),
        modifier,
        style = HavenTheme.type.secret,
        color = HavenTheme.colors.textStrong,
    )
}
```

In `ANDROID/ui/components/SecretField.kt`, delete `MASK` (and its comment) and `colourised` (and its KDoc), and the imports only they used (`AnnotatedString`, `SpanStyle`, `buildAnnotatedString`, `withStyle`, `HavenColors`). The file still compiles (same package) and is deleted in Task 6.

- [ ] **Step 5: The Material check covers the new files**

In `apps/android/app/build.gradle.kts`, add to `forbidMaterialInKit`'s `include(…)` list:

```kotlin
            "**/ui/components/ScreenBar.kt",
            "**/ui/components/SecretText.kt",
```

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components apps/android/app/src/test/kotlin/net/havenkeys/android/ui/components apps/android/app/build.gradle.kts
git commit -m "feat(android): one bar for full-screen screens; masked and revealed values drawn from the kit"
```

---
### Task 5: The item detail's rows

**Files:**
- Create: `ANDROID/ui/item/DetailRows.kt`
- Modify: `ANDROID/ui/item/TotpRing.kt` (`groupedCode` moves out; the file goes in Task 6)
- Test: `ANDROID_TEST/ui/item/DetailRowsTest.kt` (new)

**Interfaces:**
- Consumes: Task 2's read-only `GroupRow`; Task 4's `MaskedValue`, `RevealedValue`; kit `CopyButton`, `HavenIconButton`, `ProgressRing`; `uniffi.havenkeys_mobile.TotpNow(code, period, secondsRemaining)`.
- Produces (package `net.havenkeys.android.ui.item`):
  - `@Composable internal fun ShownRow(label: String, value: String, onCopy: () -> Unit)`
  - `@Composable internal fun SecretRow(label: String, revealed: String?, onReveal: () -> Unit, onCopy: () -> Unit)`
  - `@Composable internal fun CodeRow(label: String, now: TotpNow?, failed: Boolean, onCopy: (String) -> Unit)`
  - `internal fun groupedCode(code: String): String` (moved, unchanged)

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/item/DetailRowsTest.kt`:

```kotlin
package net.havenkeys.android.ui.item

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.TotpNow

@RunWith(RobolectricTestRunner::class)
class DetailRowsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    @Test
    fun aShownValueIsOneStopWithItsCopy() {
        var copies = 0
        rule.setKit { InsetGroup { row { ShownRow("Username", "sam@example.com", onCopy = { copies++ }) } } }
        rule.onNode(hasText("Username") and hasText("sam@example.com")).assertExists()
        rule.onNodeWithContentDescription(text(R.string.copy, "Username")).assertTouchTarget().performClick()
        assertEquals(1, copies)
    }

    @Test
    fun aSecretShowsTheMaskUntilRevealed() {
        rule.setKit { InsetGroup { row { SecretRow("Password", revealed = null, onReveal = {}, onCopy = {}) } } }
        rule.onNode(hasText("Password") and hasContentDescription(text(R.string.hidden, "Password"))).assertExists()
        rule.onNodeWithContentDescription(text(R.string.reveal, "Password")).assert(hasRole(Role.Button))
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }

    @Test
    fun aRevealedSecretShowsItsValueAndOffersToHideIt() {
        rule.setKit { InsetGroup { row { SecretRow("Password", revealed = "hunter2", onReveal = {}, onCopy = {}) } } }
        rule.onNodeWithText("hunter2").assertExists()
        rule.onNodeWithContentDescription(text(R.string.hide, "Password")).assertExists()
    }

    @Test
    fun theCodeReadsInTwoHalvesWithItsTimeAndCopiesWhole() {
        val copied = mutableListOf<String>()
        rule.setKit {
            InsetGroup { row { CodeRow("One-time code", TotpNow("381492", 30u, 12u), failed = false, onCopy = { copied += it }) } }
        }
        rule.onNodeWithText("381 492", substring = true).assertExists()
        rule.onNodeWithContentDescription(
            RuntimeEnvironment.getApplication().resources.getQuantityString(R.plurals.item_seconds_remaining, 12, 12),
        ).assertExists()
        rule.onNodeWithContentDescription(text(R.string.copy, "One-time code")).performClick()
        assertEquals(listOf("381492"), copied)
    }

    @Test
    fun aFailedCodeSaysSoAndOffersNoCopy() {
        rule.setKit { InsetGroup { row { CodeRow("One-time code", now = null, failed = true, onCopy = {}) } } }
        rule.onNodeWithText(text(R.string.item_code_failed), substring = true).assertExists()
        rule.onAllNodesWithContentDescription(text(R.string.copy, "One-time code")).assertCountEquals(0)
    }

    @Test
    fun codesSplitInTheMiddle() {
        assertEquals("381 492", groupedCode("381492"))
        assertEquals("1234 5678", groupedCode("12345678"))
        assertEquals("1234", groupedCode("1234"))
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*DetailRowsTest*'`
Expected: compile errors (`ShownRow`, `SecretRow`, `CodeRow` unresolved).

- [ ] **Step 3: Write `DetailRows.kt` and move `groupedCode`**

Delete `groupedCode` and `GROUP_MIN` from `ANDROID/ui/item/TotpRing.kt`, then create:

```kotlin
package net.havenkeys.android.ui.item

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.MaskedValue
import net.havenkeys.android.ui.components.RevealedValue
import net.havenkeys.android.ui.kit.CopyButton
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.TotpNow

/** The ring turns ember for the last seconds of a code. */
private const val ENDING_SECONDS = 5

private const val GROUP_MIN = 6

/** A value Rust gave with the overview (a username, a website, a note): shown, with Copy. */
@Composable
internal fun ShownRow(label: String, value: String, onCopy: () -> Unit) {
    GroupRow(trailing = { CopyButton(label, onCopy) }) { GroupRowField(label, value) }
}

/**
 * A hidden field: the fixed mask until the eye reveals it, then the value in
 * mono. The caller's RevealState holds [revealed] and clears it (30 s, leave,
 * background, lock); this row keeps nothing.
 */
@Composable
internal fun SecretRow(label: String, revealed: String?, onReveal: () -> Unit, onCopy: () -> Unit) {
    GroupRow(
        trailing = {
            HavenIconButton(
                if (revealed == null) HavenIcon.Eye else HavenIcon.EyeOff,
                stringResource(if (revealed == null) R.string.reveal else R.string.hide, label),
                onClick = onReveal,
            )
            CopyButton(label, onCopy)
        },
    ) {
        HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
        if (revealed == null) MaskedValue(label) else RevealedValue(revealed)
    }
}

/** The live code in two halves, the time it has left as a ring, and Copy (the whole code). */
@Composable
internal fun CodeRow(label: String, now: TotpNow?, failed: Boolean, onCopy: (String) -> Unit) {
    val colors = HavenTheme.colors
    val copy: (@Composable RowScope.() -> Unit)? = if (now == null) {
        null
    } else {
        { CopyButton(label, onCopy = { onCopy(now.code) }) }
    }
    GroupRow(trailing = copy) {
        HavenText(label, style = HavenTheme.type.label, color = colors.muted)
        when {
            now != null -> Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                HavenText(groupedCode(now.code), style = HavenTheme.type.code, color = colors.textStrong)
                CodeRing(now.secondsRemaining.toInt(), now.period.toInt())
            }
            failed -> HavenText(stringResource(R.string.item_code_failed), color = colors.danger)
        }
    }
}

/** How long the code has left: drains once a second; ember in the last five. */
@Composable
private fun CodeRing(secondsRemaining: Int, period: Int, modifier: Modifier = Modifier) {
    ProgressRing(
        progress = if (period > 0) secondsRemaining.toFloat() / period else 0f,
        modifier = modifier,
        size = 24.dp,
        warn = secondsRemaining <= ENDING_SECONDS,
        contentDescription = pluralStringResource(R.plurals.item_seconds_remaining, secondsRemaining, secondsRemaining),
    )
}

/** "381492" → "381 492", "12345678" → "1234 5678": easier to read and type. */
internal fun groupedCode(code: String): String =
    if (code.length < GROUP_MIN) code else code.substring(0, code.length / 2) + " " + code.substring(code.length / 2)
```

- [ ] **Step 4: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass (`ItemScreen.kt` still compiles: `groupedCode` is in the same package).

- [ ] **Step 5: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/item apps/android/app/src/test/kotlin/net/havenkeys/android/ui/item
git commit -m "feat(android): item detail rows from the kit: shown values, secrets behind the mask, the live code"
```

---

### Task 6: The item screen

**Files:**
- Modify: `ANDROID/ui/item/ItemScreen.kt` (rewritten from the kit)
- Delete: `ANDROID/ui/item/TotpRing.kt`, `ANDROID/ui/components/SecretField.kt`
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `ui/item`)
- Test: `ANDROID_TEST/ui/item/ItemScreenTest.kt` (new)

**Interfaces:**
- Consumes: Task 4's `ScreenBar`; Task 5's rows; kit `HavenScaffold`, `InsetGroup`, `ItemTile`, `HavenMenu`, `MenuItem`, `HavenDialog`, `rememberToastState`, `ToastTone`; `ui.shell.ErrorLine`, internal `ItemSummary.leading()`; `ItemViewModel` (unchanged), `RevealState`/`rememberRevealState` (unchanged), `ItemNavigation` (unchanged).
- Produces: `ItemScreen(viewModel: ItemViewModel, clipboard: SensitiveClipboard, online: Boolean, navigation: ItemNavigation, modifier: Modifier = Modifier, titleModifier: Modifier = Modifier, tileModifier: Modifier = Modifier)`; `internal fun fieldLabel(label: String): Int` (unchanged, still used by the editor).

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/item/ItemScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.item

import android.content.ClipboardManager
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

/** Views here have no one-time code: CodeRow is tested alone (DetailRowsTest). */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class ItemScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val app = RuntimeEnvironment.getApplication()
    private fun text(id: Int, vararg args: Any) = app.getString(id, *args)

    private fun summary(kind: ItemKind = ItemKind.LOGIN, passkey: Boolean = false) =
        ItemSummary("id", kind, "GitHub", "sam", "github.com", false, passkey, 0, 0)

    private fun login(passkey: Boolean = false) = ItemView(
        summary(passkey = passkey),
        listOf(
            ViewField("username", "username", FieldKind.TEXT, "sam"),
            ViewField("password", "password", FieldKind.SECRET, null),
        ),
    )

    private val vault = FakeVaultRepository().apply {
        view = Outcome.Ok(login())
        revealed = Outcome.Ok("hunter2")
    }
    private val clipboard = SensitiveClipboard(app, CoroutineScope(SupervisorJob()))
    private val went = mutableListOf<String>()
    private val navigation = ItemNavigation(
        onBack = { went += "back" },
        onLock = { went += "lock" },
        onEdit = { went += "edit" },
        onDeleted = { went += "deleted" },
    )

    private fun vm() = ItemViewModel(vault, FakeSettingsRepository(), VaultEventsHub(), "id")

    private fun show(online: Boolean = true) {
        val vm = vm()
        rule.setKit { ItemScreen(vm, clipboard, online, navigation) }
    }

    @Test
    fun theTitleIsTheScreensHeadingAndTheUsernameOneStop() {
        show()
        rule.onNode(hasText("GitHub") and isHeading()).assertExists()
        rule.onNode(hasText(text(R.string.field_username)) and hasText("sam")).assertExists()
    }

    @Test
    fun aRevealedPasswordHidesItselfAfter30Seconds() {
        show()
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText("hunter2").assertExists()
        rule.mainClock.advanceTimeBy(RevealState.REVEAL_MS + 1)
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
    }

    @Test
    fun copyingThePasswordAsksRustCopiesAndCountsAUse() {
        show()
        rule.onNodeWithContentDescription(text(R.string.copy, text(R.string.field_password))).performClick()
        rule.waitForIdle()
        assertTrue("reveal:password" in vault.calls)
        assertEquals(listOf("id"), vault.usesRecorded)
        val clip = app.getSystemService(ClipboardManager::class.java).primaryClip!!.getItemAt(0).text.toString()
        assertEquals("hunter2", clip)
        rule.onNodeWithText(text(R.string.copied, text(R.string.field_password), 30)).assertExists()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }

    @Test
    fun aFailedRevealSaysWhyAndShowsNothing() {
        vault.revealed = Outcome.Failed("locked")
        show()
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText(text(errorText("locked"))).assertExists()
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
    }

    @Test
    fun deleteAsksFirstWarnsOfPasskeysAndLeavesAfterRust() {
        vault.view = Outcome.Ok(login(passkey = true))
        show()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).performClick()
        rule.onNodeWithText(text(R.string.item_confirm_delete, "GitHub")).assertExists()
        rule.onNodeWithText(text(R.string.item_passkey_warning)).assertExists()
        rule.onNode(hasText(text(R.string.item_delete)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        assertTrue("delete:id" in vault.calls)
        assertEquals(listOf("deleted"), went)
    }

    @Test
    fun offlineEditAndMoreAreDisabled() {
        show(online = false)
        rule.onNodeWithContentDescription(text(R.string.item_edit)).assertIsNotEnabled()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).assertIsNotEnabled()
    }

    @Test
    fun theIdentityHasNoDelete() {
        vault.view = Outcome.Ok(ItemView(summary(ItemKind.IDENTITY), emptyList()))
        show()
        rule.onAllNodesWithContentDescription(text(R.string.vault_more)).assertCountEquals(0)
    }

    @Test
    fun aRestoredItemScreenComesBackMasked() {
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { ItemScreen(vm, clipboard, true, navigation) } }
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText("hunter2").assertExists()
        restoration.emulateSavedInstanceStateRestore()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ItemScreenTest*'`
Expected: FAIL: the Material screen has no heading, no `hasAnyAncestor(isDialog())` Delete inside a kit dialog, and Snackbar text instead of a toast (several tests fail; the restoration test may already pass).

- [ ] **Step 3: Rewrite `ItemScreen.kt`**

Keep the `labels` map and `fieldLabel` at the end of the file exactly as they are. Replace everything above them with:

```kotlin
package net.havenkeys.android.ui.item

import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ItemTile
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.ToastState
import net.havenkeys.android.ui.kit.ToastTone
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.leading
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

/** The header's tile: larger than a row's, so the row's tile grows into it. */
private val HeaderTile = 56.dp

/**
 * One item: its overview from the ViewModel, and each hidden field read from
 * Rust only when the user taps reveal or copy. A revealed value or a code
 * lives in this composition only (spec §9.4). [titleModifier] and
 * [tileModifier] carry the tapped row's title and tile into the header.
 */
@Composable
fun ItemScreen(
    viewModel: ItemViewModel,
    clipboard: SensitiveClipboard,
    online: Boolean,
    navigation: ItemNavigation,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
    tileModifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val toasts = rememberToastState()
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val actions = remember(viewModel, clipboard, toasts, scope, resources) {
        FieldActions(viewModel, clipboard, toasts, scope, resources)
    }
    var confirmDelete by remember { mutableStateOf(false) }

    HavenScaffold(
        modifier = modifier,
        topBar = {
            ScreenBar(onBack = navigation.onBack, online = online, onLock = navigation.onLock) {
                ItemActions(state.view, online, navigation.onEdit, onDelete = { confirmDelete = true })
            }
        },
        toastState = toasts,
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            state.view?.let { view ->
                ItemHeader(view.summary, titleModifier, tileModifier)
                ItemFields(view, actions)
            }
            state.errorCode?.let { ErrorLine(it) }
        }
    }
    val view = state.view
    if (confirmDelete && view != null) {
        DeleteDialog(
            view,
            onCancel = { confirmDelete = false },
            onDelete = {
                confirmDelete = false
                actions.delete(navigation.onDeleted)
            },
        )
    }
}

@Composable
private fun ItemHeader(summary: ItemSummary, titleModifier: Modifier, tileModifier: Modifier) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 20.dp), verticalAlignment = Alignment.CenterVertically) {
        ItemTile(summary.leading(), tileModifier, size = HeaderTile)
        Spacer(Modifier.width(14.dp))
        HavenText(
            summary.title,
            Modifier.weight(1f).then(titleModifier).semantics { heading() },
            style = HavenTheme.type.headline,
            color = HavenTheme.colors.textStrong,
        )
    }
}

/**
 * The fields as one group. Rows are composed by position; each reveal is
 * keyed by its field inside its row, so a reload that moves a field starts
 * that row masked rather than showing a value on the wrong row.
 */
@Composable
private fun ItemFields(view: ItemView, actions: FieldActions) {
    if (view.fields.isEmpty()) return
    InsetGroup {
        view.fields.forEach { field -> row { key(field.key) { FieldRow(field, actions) } } }
    }
}

/** Edit, and Delete behind More; both write to the server, so both need it online. */
@Composable
private fun ItemActions(view: ItemView?, online: Boolean, onEdit: () -> Unit, onDelete: () -> Unit) {
    HavenIconButton(HavenIcon.Edit, stringResource(R.string.item_edit), onClick = onEdit, enabled = online && view != null)
    // The identity is never deleted from the phone.
    if (view == null || view.summary.kind == ItemKind.IDENTITY) return
    var open by remember { mutableStateOf(false) }
    Box {
        // Delete is More's only entry, so More itself waits for the server.
        HavenIconButton(HavenIcon.More, stringResource(R.string.vault_more), onClick = { open = true }, enabled = online)
        HavenMenu(
            expanded = open,
            onDismiss = { open = false },
            items = listOf(MenuItem(stringResource(R.string.item_delete), onDelete, HavenIcon.Trash, danger = true)),
        )
    }
}

@Composable
private fun DeleteDialog(view: ItemView, onCancel: () -> Unit, onDelete: () -> Unit) {
    HavenDialog(
        title = stringResource(R.string.item_confirm_delete, view.summary.title),
        onDismiss = onCancel,
        confirm = DialogAction(stringResource(R.string.item_delete), onDelete, danger = true),
        message = if (view.summary.hasPasskey) stringResource(R.string.item_passkey_warning) else null,
        dismiss = DialogAction(stringResource(R.string.item_cancel), onCancel),
    )
}

/** What a field (and the item) can do; every value passes through here without being kept. */
private class FieldActions(
    val viewModel: ItemViewModel,
    private val clipboard: SensitiveClipboard,
    private val toasts: ToastState,
    private val scope: CoroutineScope,
    private val resources: Resources,
) {
    fun reveal(key: String, into: RevealState) {
        if (into.value != null) {
            into.clear()
            return
        }
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> into.show(r.value)
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    fun copyField(key: String, label: String) {
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> copy(label, r.value)
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    fun delete(onDeleted: () -> Unit) {
        scope.launch {
            when (val r = viewModel.delete()) {
                is Outcome.Ok -> onDeleted()
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    fun copyShown(label: String, value: String) {
        scope.launch { copy(label, value) }
    }

    private suspend fun copy(label: String, value: String) {
        val seconds = viewModel.clipboardClearSeconds()
        clipboard.copy(label, value, seconds)
        viewModel.copied()
        toasts.show(resources.getString(R.string.copied, label, seconds))
    }

    private fun fail(code: String) = toasts.show(resources.getString(errorText(code)), ToastTone.Alert)
}

@Composable
private fun FieldRow(field: ViewField, actions: FieldActions) {
    val label = stringResource(fieldLabel(field.label))
    val shown = field.value
    when {
        field.kind == FieldKind.TOTP -> CodeField(label, actions)
        shown != null -> ShownRow(label, shown, onCopy = { actions.copyShown(label, shown) })
        else -> {
            val reveal = rememberRevealState()
            SecretRow(
                label,
                reveal.value,
                onReveal = { actions.reveal(field.key, reveal) },
                onCopy = { actions.copyField(field.key, label) },
            )
        }
    }
}

/** The live code, asked of Rust once a second only while the screen is visible. */
@Composable
private fun CodeField(label: String, actions: FieldActions) {
    val ticks = remember(actions) { actions.viewModel.totpTicks() }
    val now by ticks.collectAsStateWithLifecycle(initialValue = null)
    CodeRow(
        label,
        (now as? Outcome.Ok)?.value,
        failed = now is Outcome.Failed,
        onCopy = { code -> actions.copyShown(label, code) },
    )
}
```

(`@StringRes` stays imported for `fieldLabel`.)

- [ ] **Step 4: Delete what nothing uses**

```bash
git rm apps/android/app/src/main/kotlin/net/havenkeys/android/ui/item/TotpRing.kt \
  apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components/SecretField.kt
```

Run `grep -rn "TotpRing\|SecretField(" apps/android/app/src`: expected no output.

- [ ] **Step 5: The Material check covers `ui/item`**

Add `"**/ui/item/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list in `apps/android/app/build.gradle.kts`.

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, `ItemViewModelTest` and `RevealStateTest` unchanged.

- [ ] **Step 7: Commit**

```bash
git add -A apps/android/app/src/main/kotlin/net/havenkeys/android/ui/item apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components apps/android/app/src/test/kotlin/net/havenkeys/android/ui/item apps/android/app/build.gradle.kts
git commit -m "feat(android): the item screen from the kit: tile and serif title, reveal and copy rows, Delete behind More, toasts"
```

---

### Task 7: The monogram tile travels from the row into the item header

**Files:**
- Modify: `ANDROID/ui/shell/SummaryRow.kt` (`SharedPart`; `SharedTitle` takes the part)
- Modify: `ANDROID/ui/nav/SharedMotion.kt` (`tileKey`; `TitleTravel.from` shares both parts)
- Modify: `ANDROID/ui/nav/HavenNavHost.kt` (the item route passes `tileModifier`)
- Test: `ANDROID_TEST/ui/shell/ShellPartsTest.kt`, `ANDROID_TEST/ui/nav/TitleTravelTest.kt`

**Interfaces:**
- Consumes: Task 2's `ItemRow.tileModifier`; Task 6's `ItemScreen(tileModifier)`.
- Produces:
  - `enum class SharedPart { Title, Tile }` (package `ui.shell`)
  - `typealias SharedTitle = @Composable (id: String, origin: String, part: SharedPart) -> Modifier`; `NoSharedTitle` takes three arguments.
  - `internal fun tileKey(id: String): String` (package `ui.nav`)

Home, category lists and search pass `SharedTitle` through without calling it, so they do not change.

- [ ] **Step 1: Write the failing tests**

Add to `ANDROID_TEST/ui/shell/ShellPartsTest.kt` (import `androidx.compose.ui.Modifier`):

```kotlin
    @Test
    fun aSummaryRowAsksForItsTitleAndItsTileToTravel() {
        val asked = mutableListOf<SharedPart>()
        rule.setKit {
            SummaryRow(
                summary(ItemKind.LOGIN),
                Origins.RECENT,
                onOpen = { _, _ -> },
                sharedTitle = { _, _, part ->
                    asked += part
                    Modifier
                },
            )
        }
        rule.runOnIdle { assertEquals(setOf(SharedPart.Title, SharedPart.Tile), asked.toSet()) }
    }
```

Add to `ANDROID_TEST/ui/nav/TitleTravelTest.kt` (import `org.junit.Assert.assertNotEquals`):

```kotlin
    @Test
    fun theTileAndTheTitleTravelUnderTheirOwnKeys() {
        assertNotEquals(titleKey("a"), tileKey("a"))
        assertNotEquals(tileKey("a"), tileKey("b"))
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellPartsTest*' --tests '*TitleTravelTest*'`
Expected: compile errors (`SharedPart`, `tileKey` unresolved).

- [ ] **Step 3: The row shares both parts**

In `ANDROID/ui/shell/SummaryRow.kt`, replace the `SharedTitle` typealias and `NoSharedTitle` with:

```kotlin
/** Which part of a row travels to the item screen (spec §7: the title and the monogram tile). */
enum class SharedPart { Title, Tile }

/** The modifier that carries one part of a row to the item screen; nothing by default. */
typealias SharedTitle = @Composable (id: String, origin: String, part: SharedPart) -> Modifier

val NoSharedTitle: SharedTitle = { _, _, _ -> Modifier }
```

and in `SummaryRow`'s `ItemRow(…)` call:

```kotlin
        titleModifier = sharedTitle(summary.id, origin, SharedPart.Title),
        tileModifier = sharedTitle(summary.id, origin, SharedPart.Tile),
```

- [ ] **Step 4: Travel shares the tile under its own key**

In `ANDROID/ui/nav/SharedMotion.kt` (import `net.havenkeys.android.ui.shell.SharedPart`), add under `titleKey`:

```kotlin
/** A row's monogram tile and the item screen's header tile. */
internal fun tileKey(id: String): String = "tile-$id"
```

and replace `TitleTravel.from` with:

```kotlin
    @OptIn(ExperimentalSharedTransitionApi::class)
    fun from(shared: SharedTransitionScope, visibility: AnimatedVisibilityScope, motion: HavenMotion): SharedTitle =
        { id, origin, part ->
            if (isTapped(id, origin)) {
                val key = if (part == SharedPart.Title) titleKey(id) else tileKey(id)
                Modifier.sharedIfMoving(shared, key, visibility, motion)
            } else {
                Modifier
            }
        }
```

Update the class KDoc's first line to "Which row's title and tile travel to the item screen: the one tapped last."

- [ ] **Step 5: The item route receives the tile**

In `ANDROID/ui/nav/HavenNavHost.kt`, in `itemScreens`' `ItemScreen(…)` call, add after `titleModifier = …`:

```kotlin
            tileModifier = Modifier.sharedIfMoving(nav.shared, tileKey(id), this, nav.motion),
```

Under "Remove animations" `sharedIfMoving` returns the plain modifier, so the tile cuts with its screen.

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass (`HomeScreenTest.anItemInBothListsOpensFromTheRowTapped`, `NavMotionTest`, `ShellNavHostTest` unchanged).

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/shell/SummaryRow.kt apps/android/app/src/main/kotlin/net/havenkeys/android/ui/nav apps/android/app/src/test/kotlin/net/havenkeys/android/ui/shell apps/android/app/src/test/kotlin/net/havenkeys/android/ui/nav
git commit -m "feat(android): the tapped row's monogram tile grows into the item header with its title"
```

---
### Task 8: The editor's fields

**Files:**
- Create: `ANDROID/ui/edit/DraftText.kt`, `ANDROID/ui/edit/WebsiteEditors.kt`
- Modify: `ANDROID/ui/edit/EditFields.kt` (rewritten from the kit)
- Modify: `RES/values/strings.xml`, `RES/values-pt-rBR/strings.xml` (`edit_match`, opening the stage's block)
- Test: `ANDROID_TEST/ui/edit/EditFieldsTest.kt` (new)

**Interfaces:**
- Consumes: Task 2's field `hint`; Task 3's `ChoiceSheet`; Task 4's `MaskedValue`; `EditorState`, `WebsiteRow` (unchanged), `FieldValues(viewModel: EditViewModel, loading: SnapshotStateList<String>)` and `EditViewModel.reveal/generate` (unchanged), `fieldLabel(key)` from `ui.item`.
- Produces (package `net.havenkeys.android.ui.edit`):
  - `@Composable internal fun rememberDraftText(vararg keys: Any?, initial: () -> String, onEdit: (String) -> Unit): TextFieldState`
  - `@Composable internal fun Websites(editor: EditorState)` (in `WebsiteEditors.kt`)
  - `@Composable internal fun EditFields(editor: EditorState, edit: ItemEdit, values: FieldValues)` (signature unchanged)
  - string `edit_match` ("Matches" / "Corresponde a")

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/edit/EditFieldsTest.kt`:

```kotlin
package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.MatchKind
import uniffi.havenkeys_mobile.Website

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class EditFieldsTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    private fun login(fields: List<EditField>, websites: List<Website> = emptyList()) =
        ItemEdit(ItemKind.LOGIN, "GitHub", websites, fields, false, true, 3L)

    private fun show(edit: ItemEdit, loading: List<String> = emptyList()): Pair<EditorState, SnapshotStateList<String>> {
        vault.edit = Outcome.Ok(edit)
        val vm = EditViewModel(vault, FakeAccountRepository(), VaultEventsHub(), EditTarget.Existing("id"))
        val editor = EditorState(edit)
        val pending = mutableStateListOf<String>().apply { addAll(loading) }
        rule.setKit {
            Column(Modifier.verticalScroll(rememberScrollState())) { EditFields(editor, edit, FieldValues(vm, pending)) }
        }
        return editor to pending
    }

    private fun field(label: String) = rule.onNode(hasSetTextAction() and hasText(label))

    @Test
    fun typingTheTitleChangesTheDraft() {
        val (editor, _) = show(login(emptyList()))
        field(text(R.string.edit_title)).performTextReplacement("GitHub work")
        rule.runOnIdle {
            assertEquals("GitHub work", editor.toDraft().title)
            assertTrue(editor.dirty)
        }
    }

    @Test
    fun aHiddenPasswordIsReadFromRustOnlyWhenTheUserChangesIt() {
        vault.revealed = Outcome.Ok("hunter2")
        val (editor, _) = show(login(listOf(EditField("password", FieldKind.SECRET, true, null))))
        val password = text(R.string.field_password)
        rule.onNodeWithContentDescription(text(R.string.hidden, password)).assertExists()
        assertTrue(vault.calls.none { it == "reveal:password" })
        rule.onNodeWithText(text(R.string.edit_change)).performClick()
        rule.waitForIdle()
        assertTrue("reveal:password" in vault.calls)
        field(password).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        rule.runOnIdle {
            assertEquals("hunter2", editor.shown("password"))
            assertFalse(editor.dirty)
        }
    }

    @Test
    fun generateFillsThePasswordAndShowsIt() {
        vault.generated = Outcome.Ok(Generated("Gen-3rated!", 120.0))
        val (editor, _) = show(login(listOf(EditField("password", FieldKind.SECRET, false, null))))
        rule.onNodeWithContentDescription(text(R.string.edit_generate)).performClick()
        rule.waitForIdle()
        rule.runOnIdle { assertEquals("Gen-3rated!", editor.shown("password")) }
        rule.onNodeWithContentDescription(text(R.string.hide, text(R.string.field_password))).assertExists()
    }

    @Test
    fun aFieldWaitingForItsValueTakesNoTypingThenShowsIt() {
        val username = text(R.string.field_username)
        val (editor, pending) = show(
            login(listOf(EditField("username", FieldKind.TEXT, true, null))),
            loading = listOf("username"),
        )
        field(username).assertIsNotEnabled()
        rule.runOnIdle {
            editor.load("username", "sam")
            pending.remove("username")
        }
        field(username).assertIsEnabled()
        assertEquals(
            "sam",
            field(username).fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text,
        )
        rule.runOnIdle { assertFalse(editor.dirty) }
    }

    @Test
    fun theWebsiteMatchIsPickedFromASheet() {
        val (editor, _) = show(login(emptyList(), listOf(Website("github.com", MatchKind.DOMAIN))))
        rule.onNode(hasText(text(R.string.edit_match)) and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.edit_match_exact)) and hasRole(Role.RadioButton)).performClick()
        rule.runOnIdle { assertEquals(MatchKind.EXACT, editor.websites.single().match) }
    }

    @Test
    fun aWebsiteCanBeAddedAndRemoved() {
        val (editor, _) = show(login(emptyList(), listOf(Website("github.com", MatchKind.DOMAIN))))
        rule.onNode(hasText(text(R.string.edit_add_website)) and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(2, editor.websites.size) }
        rule.onAllNodes(hasClickAction() and SemanticsMatcher.expectValue(
            SemanticsProperties.ContentDescription,
            listOf(text(R.string.edit_remove_website)),
        ))[0].performClick()
        rule.runOnIdle { assertEquals(listOf(""), editor.websites.map { it.url }) }
    }

    @Test
    fun aRemovedFieldShowsNoValueAndCanComeBack() {
        val (editor, _) = show(login(listOf(EditField("password", FieldKind.SECRET, true, null))))
        rule.onNodeWithText(text(R.string.edit_remove)).performClick()
        rule.onNodeWithText(text(R.string.edit_will_be_removed), substring = true).assertExists()
        rule.runOnIdle { assertTrue(editor.isRemoved("password")) }
        rule.onNodeWithText(text(R.string.edit_undo)).performClick()
        rule.runOnIdle { assertFalse(editor.isRemoved("password")) }
    }

    @Test
    fun theCodesSetupKeyIsTypedLikeAPassword() {
        show(login(listOf(EditField("totp", FieldKind.TOTP, false, null))))
        field(text(R.string.field_totp)).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*EditFieldsTest*'`
Expected: compile error (`R.string.edit_match` unknown); then failures (Material fields have no `Password` semantics on the setup key, no "Matches" row).

- [ ] **Step 3: The string**

Append to both string files, before `</resources>`:

`RES/values/strings.xml`:

```xml

    <!-- Existing screens rebuilt (spec 2026-10-03 §6.9) -->
    <string name="edit_match">Matches</string>
```

`RES/values-pt-rBR/strings.xml`:

```xml

    <!-- Existing screens rebuilt (spec 2026-10-03 §6.9) -->
    <string name="edit_match">Corresponde a</string>
```

- [ ] **Step 4: Write `DraftText.kt`**

```kotlin
package net.havenkeys.android.ui.edit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.drop

/**
 * A kit field's text over one draft value. `remember`ed, never saved (spec
 * §9.4: never `rememberTextFieldState`, which writes its text into the saved
 * instance state). It starts from [initial] and starts again when a key
 * changes: a new draft, or a value that just arrived from Rust. Every edit
 * after that goes to [onEdit]; the starting value is not an edit.
 */
@Composable
internal fun rememberDraftText(vararg keys: Any?, initial: () -> String, onEdit: (String) -> Unit): TextFieldState {
    val state = remember(*keys) { TextFieldState(initial()) }
    val latest by rememberUpdatedState(onEdit)
    LaunchedEffect(state) {
        snapshotFlow { state.text.toString() }.drop(1).collect { latest(it) }
    }
    return state
}
```

- [ ] **Step 5: Write `WebsiteEditors.kt`**

```kotlin
package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ChoiceSheet
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SectionHeader
import uniffi.havenkeys_mobile.MatchKind

/** The three rules in the order the desktop lists them; their labels are sentences, hence a sheet. */
private val matchOrder = listOf(MatchKind.DOMAIN, MatchKind.ORIGIN, MatchKind.EXACT)

private fun matchLabel(match: MatchKind): Int = when (match) {
    MatchKind.DOMAIN -> R.string.edit_match_domain
    MatchKind.ORIGIN -> R.string.edit_match_origin
    MatchKind.EXACT -> R.string.edit_match_exact
}

/** A login's websites: each address with Remove, its match rule under it, then Add website. */
@Composable
internal fun Websites(editor: EditorState) {
    var choosing by remember { mutableStateOf<WebsiteRow?>(null) }
    Column {
        SectionHeader(stringResource(R.string.edit_websites))
        InsetGroup {
            editor.websites.forEach { site ->
                row { key(site) { WebsiteEditor(site, onRemove = { editor.websites.remove(site) }) } }
                row { key(site) { MatchRow(site.match, onClick = { choosing = site }) } }
            }
            row {
                GroupRow(onClick = editor::addWebsite, icon = HavenIcon.Plus, chevron = false) {
                    GroupRowText(stringResource(R.string.edit_add_website))
                }
            }
        }
    }
    choosing?.let { site ->
        ChoiceSheet(
            title = stringResource(R.string.edit_match),
            values = matchOrder,
            selected = site.match,
            label = { stringResource(matchLabel(it)) },
            onSelect = { site.match = it },
            onDismiss = { choosing = null },
        )
    }
}

@Composable
private fun WebsiteEditor(site: WebsiteRow, onRemove: () -> Unit) {
    val url = rememberDraftText(site, initial = { site.url }, onEdit = { site.url = it })
    Row(verticalAlignment = Alignment.CenterVertically) {
        HavenTextField(
            url,
            stringResource(R.string.field_website),
            Modifier.weight(1f),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
        )
        HavenIconButton(HavenIcon.X, stringResource(R.string.edit_remove_website), onClick = onRemove)
    }
}

@Composable
private fun MatchRow(match: MatchKind, onClick: () -> Unit) {
    GroupRow(onClick = onClick) {
        GroupRowField(stringResource(R.string.edit_match), stringResource(matchLabel(match)))
    }
}
```

- [ ] **Step 6: Rewrite `EditFields.kt`**

```kotlin
package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.MaskedValue
import net.havenkeys.android.ui.item.fieldLabel
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind

/** Long text the user edits: multi-line, and not masked once opened. */
private val longText = setOf("notes", "content", "card.notes", "identity.notes")

private const val LONG_TEXT_LINES = 3

@Composable
internal fun EditFields(editor: EditorState, edit: ItemEdit, values: FieldValues) {
    Column(verticalArrangement = Arrangement.spacedBy(HavenSpacing.groupGap)) {
        // The identity's title is its name, built by Rust.
        if (edit.kind != ItemKind.IDENTITY) InsetGroup { row { TitleEditor(editor) } }
        if (edit.kind == ItemKind.LOGIN) Websites(editor)
        if (edit.fields.isNotEmpty()) {
            InsetGroup {
                edit.fields.forEach { field -> row { key(field.key) { FieldEditor(field, editor, values) } } }
            }
        }
        if (edit.hasCustomFields) {
            HavenText(
                stringResource(R.string.edit_custom_kept),
                Modifier.padding(horizontal = HavenSpacing.rowX),
                color = HavenTheme.colors.muted,
            )
        }
    }
}

@Composable
private fun TitleEditor(editor: EditorState) {
    val title = rememberDraftText(editor, initial = { editor.title }, onEdit = { editor.title = it })
    HavenTextField(title, stringResource(R.string.edit_title))
}

@Composable
private fun FieldEditor(field: EditField, editor: EditorState, values: FieldValues) {
    val key = field.key
    val label = stringResource(fieldLabel(key))
    // Removal first: the old value is never shown for a field being removed.
    when {
        editor.isRemoved(key) -> RemovedRow(label, onUndo = { editor.undo(key) })
        field.kind == FieldKind.TEXT -> TextEditor(key, label, editor, values)
        field.kind == FieldKind.SECRET && field.present && !editor.isOpen(key) -> HiddenRow(
            label = label,
            masked = true,
            onChange = { loadSecret(key, editor, values) },
            onRemove = { editor.remove(key) },
        )
        field.kind == FieldKind.SECRET -> SecretEditor(key, label, editor, values)
        // A one-time code's key is never shown: it can be replaced or removed.
        field.present && !editor.isOpen(key) -> HiddenRow(
            label = label,
            masked = false,
            onChange = { editor.open(key) },
            onRemove = { editor.remove(key) },
        )
        else -> SetupKeyEditor(key, label, editor)
    }
}

/** A hidden secret is read from Rust only when the user chooses to change it. */
private suspend fun loadSecret(key: String, editor: EditorState, values: FieldValues) {
    (values.viewModel.reveal(key) as? Outcome.Ok)?.let { editor.load(key, it.value) }
    // On failure it still opens, empty: typing replaces, leaving it empty keeps.
    editor.open(key)
}

@Composable
private fun TextEditor(key: String, label: String, editor: EditorState, values: FieldValues) {
    val loading = key in values.loading
    // Keyed on `loading`: Rust's value replaces the field that waited for it, disabled until then.
    val text = rememberDraftText(editor, key, loading, initial = { editor.shown(key) }, onEdit = { editor.type(key, it) })
    HavenTextField(
        text,
        label,
        enabled = !loading,
        placeholder = if (key == "card.expiry") stringResource(R.string.edit_expiry_placeholder) else null,
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Text,
            autoCorrectEnabled = !(key == "card.holder" || key.startsWith("identity.") || key == "username"),
        ),
        lineLimits = if (key in longText) {
            TextFieldLineLimits.MultiLine(minHeightInLines = LONG_TEXT_LINES)
        } else {
            TextFieldLineLimits.SingleLine
        },
    )
}

@Composable
private fun SecretEditor(key: String, label: String, editor: EditorState, values: FieldValues) {
    val text = rememberDraftText(editor, key, initial = { editor.shown(key) }, onEdit = { editor.type(key, it) })
    if (key in longText) {
        // A secure note's content: long text, shown while it is edited, as before.
        HavenTextField(
            text,
            label,
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
            lineLimits = TextFieldLineLimits.MultiLine(minHeightInLines = LONG_TEXT_LINES),
        )
        return
    }
    val scope = rememberCoroutineScope()
    var visible by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        SecretTextField(text, label, revealed = visible, onRevealChange = { visible = it }, modifier = Modifier.weight(1f))
        if (key == "password") {
            HavenIconButton(
                HavenIcon.Dice,
                stringResource(R.string.edit_generate),
                onClick = {
                    scope.launch {
                        val generated = values.viewModel.generate()
                        if (generated is Outcome.Ok) {
                            // Into the field, which hands it to the draft like typing would.
                            text.setTextAndPlaceCursorAtEnd(generated.value)
                            visible = true
                        }
                    }
                },
            )
        }
    }
}

/** A one-time code's setup key or otpauth:// link: a secret, typed masked; the eye shows it. */
@Composable
private fun SetupKeyEditor(key: String, label: String, editor: EditorState) {
    val text = rememberDraftText(editor, key, initial = { editor.shown(key) }, onEdit = { editor.type(key, it) })
    var visible by remember { mutableStateOf(false) }
    SecretTextField(
        text,
        label,
        revealed = visible,
        onRevealChange = { visible = it },
        hint = stringResource(R.string.edit_totp_placeholder),
    )
}

/** A present value that is not shown: the fixed mask (never its length) or "Set up.", with Change and Remove. */
@Composable
private fun HiddenRow(label: String, masked: Boolean, onChange: suspend () -> Unit, onRemove: () -> Unit) {
    val scope = rememberCoroutineScope()
    GroupRow(
        trailing = {
            HavenButton(
                stringResource(if (masked) R.string.edit_change else R.string.edit_replace),
                onClick = { scope.launch { onChange() } },
                style = ButtonStyle.Quiet,
            )
            HavenButton(stringResource(R.string.edit_remove), onClick = onRemove, style = ButtonStyle.Quiet)
        },
    ) {
        HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
        if (masked) {
            MaskedValue(label)
        } else {
            HavenText(stringResource(R.string.edit_set_up), style = HavenTheme.type.value, color = HavenTheme.colors.muted)
        }
    }
}

@Composable
private fun RemovedRow(label: String, onUndo: () -> Unit) {
    GroupRow(trailing = { HavenButton(stringResource(R.string.edit_undo), onClick = onUndo, style = ButtonStyle.Quiet) }) {
        HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
        HavenText(stringResource(R.string.edit_will_be_removed), style = HavenTheme.type.value, color = HavenTheme.colors.muted)
    }
}
```

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug`
Expected: all pass, `EditorStateTest` and `EditViewModelTest` unchanged. `EditScreen.kt` still compiles (Material until Task 9).

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/edit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/edit apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml
git commit -m "feat(android): editor fields from the kit; secrets and the setup key typed masked, the website match from a sheet"
```

---

### Task 9: The editor screen

**Files:**
- Modify: `ANDROID/ui/edit/EditScreen.kt` (rewritten from the kit)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `ui/edit`)
- Test: `ANDROID_TEST/ui/edit/EditScreenTest.kt` (new)

**Interfaces:**
- Consumes: Task 3's `HavenDialog(dismissible)`; Task 4's `ScreenBar`, `OfflineNote`; Task 8's `EditFields`; `ui.shell.LargeTitle`, `ErrorLine`; `EditViewModel`, `EditResult` (unchanged).
- Produces: `EditScreen(viewModel, isNew, online, navigation, modifier)` and `EditNavigation`, `FieldValues` unchanged.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/edit/EditScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.edit

import android.view.KeyEvent
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowDialog
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class EditScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val vault = FakeVaultRepository().apply {
        edit = Outcome.Ok(
            ItemEdit(ItemKind.LOGIN, "GitHub", emptyList(), listOf(EditField("username", FieldKind.TEXT, true, "sam")), false, true, 3L),
        )
    }
    private val done = mutableListOf<String>()
    private var backs = 0
    private val navigation = EditNavigation(onDone = { done += it }, onBack = { backs++ }, onLock = {})

    private fun vm() = EditViewModel(vault, FakeAccountRepository(), VaultEventsHub(), EditTarget.Existing("id"))

    private fun show(online: Boolean = true) {
        val vm = vm()
        rule.setKit { EditScreen(vm, isNew = false, online = online, navigation = navigation) }
    }

    private fun title() = rule.onNode(hasSetTextAction() and hasText(text(R.string.edit_title)))

    private fun pressBack() = rule.runOnIdle {
        val window = ShadowDialog.getLatestDialog().window!!
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
    }

    @Test
    fun savingSendsTheDraftAndGoesOn() {
        show()
        title().performTextReplacement("GitHub work")
        rule.onNodeWithText(text(R.string.edit_save)).assertIsEnabled().performClick()
        rule.waitForIdle()
        assertEquals("GitHub work", vault.drafts.single().title)
        assertEquals(listOf("id"), done)
    }

    @Test
    fun offlineTheSaveWaitsAndTheScreenSaysWhy() {
        show(online = false)
        rule.onNodeWithText(text(R.string.edit_save)).assertIsNotEnabled()
        rule.onNodeWithText(text(R.string.edit_offline)).assertExists()
    }

    @Test
    fun leavingWithChangesAsksFirst() {
        show()
        title().performTextReplacement("Changed")
        rule.onNodeWithContentDescription(text(R.string.item_back)).performClick()
        rule.onNodeWithText(text(R.string.edit_discard_changes)).assertExists()
        rule.onNodeWithText(text(R.string.edit_keep_editing)).performClick()
        assertEquals(0, backs)
        rule.onNodeWithContentDescription(text(R.string.item_back)).performClick()
        rule.onNodeWithText(text(R.string.edit_discard)).performClick()
        assertEquals(1, backs)
    }

    @Test
    fun backOnTheConflictLeavesReloadWorking() {
        vault.updated = Outcome.Failed("item_changed_elsewhere")
        show()
        title().performTextReplacement("Changed")
        rule.onNodeWithText(text(R.string.edit_save)).performClick()
        rule.onNodeWithText(text(R.string.error_item_changed_elsewhere)).assertExists()
        pressBack()
        rule.onNodeWithText(text(R.string.edit_reload)).assertIsEnabled().performClick()
        rule.waitForIdle()
        rule.onNodeWithText(text(R.string.error_item_changed_elsewhere)).assertDoesNotExist()
        assertTrue(vault.calls.count { it == "editable:id" } >= 2)
    }

    @Test
    fun aRestoredEditorBringsNothingTypedBack() {
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { EditScreen(vm, false, true, navigation) } }
        title().performTextReplacement("Typed before the restore")
        restoration.emulateSavedInstanceStateRestore()
        assertEquals("GitHub", title().fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*EditScreenTest*'`
Expected: FAIL (`backOnTheConflictLeavesReloadWorking` cannot find a Back-able kit dialog; the Back button has no "Back" description in the Material bar's arrow? it does: then the dialog tests differ). At least the conflict test fails.

- [ ] **Step 3: Rewrite `EditScreen.kt`**

Keep `EditNavigation`, `FieldValues`, `rememberTextLoads` and `screenTitle` exactly as they are. Replace `EditScreen`, `OfflineNote`, `ErrorLine`, `DiscardDialog` and `ConflictDialog` with:

```kotlin
/**
 * Create or edit one item. The draft ([EditorState]) is remembered here per
 * load generation and nowhere else: not in the ViewModel, not in saved
 * state (spec §9.4). Values are read from Rust only to be edited, one field
 * at a time.
 */
@Composable
fun EditScreen(
    viewModel: EditViewModel,
    isNew: Boolean,
    online: Boolean,
    navigation: EditNavigation,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val edit = state.edit
    val editor = remember(state.generation, edit) { edit?.let(::EditorState) }
    val loading = rememberTextLoads(editor, edit, viewModel)
    var confirmDiscard by remember { mutableStateOf(false) }
    val leave = { if (editor?.dirty == true) confirmDiscard = true else navigation.onBack() }
    val canSave = online && editor != null && loading.isEmpty() && !state.saving

    LaunchedEffect(viewModel) {
        viewModel.results.collect { if (it is EditResult.Saved) navigation.onDone(it.id) }
    }
    BackHandler(enabled = editor?.dirty == true) { confirmDiscard = true }

    HavenScaffold(
        modifier = modifier,
        topBar = {
            ScreenBar(onBack = leave, online = online, onLock = navigation.onLock) {
                HavenButton(
                    stringResource(R.string.edit_save),
                    onClick = { editor?.let { viewModel.save(it.toDraft()) } },
                    Modifier.padding(start = 4.dp, end = 8.dp),
                    enabled = canSave,
                )
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            edit?.let { LargeTitle(stringResource(screenTitle(it.kind, isNew))) }
            if (!online) OfflineNote(stringResource(R.string.edit_offline))
            state.errorCode?.let { ErrorLine(it) }
            if (editor != null && edit != null) EditFields(editor, edit, FieldValues(viewModel, loading))
        }
    }
    if (confirmDiscard) {
        DiscardDialog(isNew = isNew, onKeep = { confirmDiscard = false }, onDiscard = navigation.onBack)
    }
    if (state.conflict) ConflictDialog(onReload = viewModel::reload)
}

@Composable
private fun DiscardDialog(isNew: Boolean, onKeep: () -> Unit, onDiscard: () -> Unit) {
    HavenDialog(
        title = stringResource(if (isNew) R.string.edit_discard_new else R.string.edit_discard_changes),
        onDismiss = onKeep,
        confirm = DialogAction(stringResource(R.string.edit_discard), onDiscard, danger = true),
        dismiss = DialogAction(stringResource(R.string.edit_keep_editing), onKeep),
    )
}

/** Not dismissible: the draft was made against a version that no longer exists; Reload is the only way on. */
@Composable
private fun ConflictDialog(onReload: () -> Unit) {
    HavenDialog(
        title = stringResource(R.string.error_item_changed_elsewhere),
        onDismiss = {},
        confirm = DialogAction(stringResource(R.string.edit_reload), onReload),
        message = stringResource(R.string.edit_conflict_text),
        dismissible = false,
    )
}
```

Imports: drop every `androidx.compose.material…`, `HavenTopBar`, `NoPersonalizedLearning` (the kit's fields do it), `Surface`, `Icons`; add `androidx.compose.foundation.layout.fillMaxSize`, `androidx.compose.ui.unit.dp`, `net.havenkeys.android.ui.components.OfflineNote`, `net.havenkeys.android.ui.components.ScreenBar`, `net.havenkeys.android.ui.kit.{DialogAction, HavenButton, HavenDialog, HavenScaffold}`, `net.havenkeys.android.ui.shell.{ErrorLine, LargeTitle}`, `net.havenkeys.android.ui.theme.HavenSpacing`.

- [ ] **Step 4: The Material check covers `ui/edit`**

Add `"**/ui/edit/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list.

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/edit apps/android/app/src/test/kotlin/net/havenkeys/android/ui/edit apps/android/app/build.gradle.kts
git commit -m "feat(android): the editor screen from the kit; the conflict dialog answers only with Reload"
```

---
### Task 10: The generator

**Files:**
- Modify: `ANDROID/ui/generator/GeneratorScreen.kt` (rewritten from the kit)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `ui/generator`)
- Test: `ANDROID_TEST/ui/generator/GeneratorScreenTest.kt` (new)

**Interfaces:**
- Consumes: Task 4's `ScreenBar`, `colourised` (internal, same module); kit `HavenScaffold`, `InsetGroup`, `HavenSlider`, `ToggleRow`, `HavenButton`, `SectionHeader`, `rememberToastState`; `ui.shell.LargeTitle`; `GeneratorViewModel`, `strengthOf`, `Strength` (unchanged).
- Produces: `GeneratorScreen(viewModel, clipboard, online, onBack, onLock, modifier)` unchanged.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/generator/GeneratorScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.generator

import android.content.ClipboardManager
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.Generated

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class GeneratorScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val app = RuntimeEnvironment.getApplication()
    private fun text(id: Int, vararg args: Any) = app.getString(id, *args)

    private val vault = FakeVaultRepository().apply { generated = Outcome.Ok(Generated("Pa55-word!", 120.0)) }
    private val clipboard = SensitiveClipboard(app, CoroutineScope(SupervisorJob()))

    private fun vm() = GeneratorViewModel(vault, FakeSettingsRepository())

    private fun show() {
        val vm = vm()
        rule.setKit { GeneratorScreen(vm, clipboard, online = true, onBack = {}, onLock = {}) }
    }

    @Test
    fun thePasswordShowsWithItsStrength() {
        show()
        rule.onNodeWithText("Pa55-word!").assertExists()
        rule.onNodeWithText(text(R.string.generator_strength, text(R.string.generator_excellent), 120)).assertExists()
    }

    @Test
    fun regenerateAsksRustAgain() {
        show()
        vault.generated = Outcome.Ok(Generated("Second-0ne", 120.0))
        rule.onNode(hasText(text(R.string.generator_regenerate)) and hasClickAction()).performClick()
        rule.onNodeWithText("Second-0ne").assertExists()
    }

    @Test
    fun copyPutsItOnTheClipboardAndSaysForHowLong() {
        show()
        rule.onNode(hasText(text(R.string.generator_copy)) and hasClickAction()).performClick()
        rule.waitForIdle()
        val clip = app.getSystemService(ClipboardManager::class.java).primaryClip!!.getItemAt(0).text.toString()
        assertEquals("Pa55-word!", clip)
        rule.onNodeWithText(text(R.string.generator_copied, 30)).assertExists()
        assertEquals(emptyList<String>(), vault.usesRecorded)
    }

    @Test
    fun theLengthReadsAsANumberAndSymbolsCanBeTurnedOff() {
        show()
        val slider = rule.onNodeWithContentDescription(text(R.string.generator_length))
        assertEquals("24", slider.fetchSemanticsNode().config.getOrNull(SemanticsProperties.StateDescription))
        rule.onNode(hasText(text(R.string.generator_symbols)) and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(false, vault.generatedWith?.symbols) }
    }

    @Test
    fun aRestoredGeneratorDoesNotBringTheOldPasswordBack() {
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { GeneratorScreen(vm, clipboard, true, {}, {}) } }
        rule.onNodeWithText("Pa55-word!").assertExists()
        vault.generated = Outcome.Ok(Generated("Fresh-0ne", 120.0))
        restoration.emulateSavedInstanceStateRestore()
        rule.onAllNodesWithText("Pa55-word!").assertCountEquals(0)
        rule.onNodeWithText("Fresh-0ne").assertExists()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*GeneratorScreenTest*'`
Expected: FAIL (the Material slider has no "Length" description with "24"; Snackbar instead of the kit toast).

- [ ] **Step 3: Rewrite `GeneratorScreen.kt`**

```kotlin
package net.havenkeys.android.ui.generator

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.components.colourised
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenSlider
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.ToggleRow
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.GeneratorOptions

/** Every whole length from the core's minimum to its maximum is a stop. */
private val LengthSteps = (GeneratorViewModel.MAX_LENGTH - GeneratorViewModel.MIN_LENGTH).toInt() - 1

/**
 * A new password for each change of the options. The password lives only in
 * this composition (`remember`, not saveable), so leaving the screen or the
 * lock wipe drops it.
 */
@Composable
fun GeneratorScreen(
    viewModel: GeneratorViewModel,
    clipboard: SensitiveClipboard,
    online: Boolean,
    onBack: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var password by remember { mutableStateOf<String?>(null) }
    var failure by remember { mutableStateOf<String?>(null) }
    // Bumped by Regenerate, so the same options give a new password.
    var round by remember { mutableIntStateOf(0) }
    val toasts = rememberToastState()
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val label = stringResource(R.string.field_password)

    LaunchedEffect(state.options, round) {
        when (val r = viewModel.generate()) {
            is Outcome.Ok -> {
                password = r.value
                failure = null
            }
            is Outcome.Failed -> {
                password = null
                failure = r.code
            }
        }
    }

    HavenScaffold(
        modifier = modifier,
        topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) },
        toastState = toasts,
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            LargeTitle(stringResource(R.string.generator_title))
            Output(password, failure, state.entropyBits)
            Actions(
                canCopy = password != null,
                onRegenerate = { round++ },
                onCopy = {
                    val value = password
                    if (value != null) {
                        scope.launch {
                            val seconds = viewModel.clipboardClearSeconds()
                            clipboard.copy(label, value, seconds)
                            toasts.show(resources.getString(R.string.generator_copied, seconds))
                        }
                    }
                },
            )
            Options(state.options, viewModel::setOptions)
        }
    }
}

/** The password on the output plate (the desktop's 14dp radius), digits and symbols coloured. */
@Composable
private fun Output(password: String?, failure: String?, entropyBits: Double?) {
    val colors = HavenTheme.colors
    Column(
        Modifier
            .fillMaxWidth()
            .clip(HavenShape.output)
            .background(colors.group)
            .border(1.dp, colors.groupLine, HavenShape.output)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        HavenText(stringResource(R.string.generator_lede), color = colors.muted)
        when {
            password != null -> HavenText(
                colourised(password, colors),
                style = HavenTheme.type.secret,
                color = colors.textStrong,
            )
            failure != null -> HavenText(
                stringResource(if (failure == "invalid_input") R.string.generator_failed else errorText(failure)),
                color = colors.danger,
            )
        }
        if (password != null && entropyBits != null) {
            HavenText(
                stringResource(
                    R.string.generator_strength,
                    stringResource(strengthLabel(strengthOf(entropyBits))),
                    entropyBits.roundToInt(),
                ),
                style = HavenTheme.type.label,
                color = colors.brassInk,
            )
        }
    }
}

@Composable
private fun Actions(canCopy: Boolean, onRegenerate: () -> Unit, onCopy: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        HavenButton(
            stringResource(R.string.generator_regenerate),
            onClick = onRegenerate,
            Modifier.weight(1f),
            style = ButtonStyle.Secondary,
            icon = HavenIcon.Refresh,
        )
        HavenButton(
            stringResource(R.string.generator_copy),
            onClick = onCopy,
            Modifier.weight(1f),
            enabled = canCopy,
            icon = HavenIcon.Copy,
        )
    }
}

@Composable
private fun Options(options: GeneratorOptions, onChange: (GeneratorOptions) -> Unit) {
    val length = stringResource(R.string.generator_length)
    SectionHeader(length)
    InsetGroup {
        row {
            Column(Modifier.padding(horizontal = HavenSpacing.rowX, vertical = 8.dp)) {
                // The slider shows no number of its own (DESIGN.md): the row says it.
                Row {
                    HavenText(length, Modifier.weight(1f), style = HavenTheme.type.value, color = HavenTheme.colors.text)
                    HavenText(options.length.toString(), style = HavenTheme.type.value, color = HavenTheme.colors.textStrong)
                }
                HavenSlider(
                    value = options.length.toFloat(),
                    onValueChange = { onChange(options.copy(length = it.roundToInt().toUInt())) },
                    valueRange = GeneratorViewModel.MIN_LENGTH.toFloat()..GeneratorViewModel.MAX_LENGTH.toFloat(),
                    label = length,
                    steps = LengthSteps,
                    valueText = options.length.toString(),
                )
            }
        }
    }
    Spacer(Modifier.height(HavenSpacing.groupGap))
    SectionHeader(stringResource(R.string.generator_characters))
    InsetGroup {
        row { Toggle(R.string.generator_uppercase, "A–Z", options.uppercase) { onChange(options.copy(uppercase = it)) } }
        row { Toggle(R.string.generator_lowercase, "a–z", options.lowercase) { onChange(options.copy(lowercase = it)) } }
        row { Toggle(R.string.generator_digits, "0–9", options.digits) { onChange(options.copy(digits = it)) } }
        row { Toggle(R.string.generator_symbols, "!@#…", options.symbols) { onChange(options.copy(symbols = it)) } }
        row {
            Toggle(R.string.generator_avoid_ambiguous, "l, 1, O, 0", options.avoidAmbiguous) {
                onChange(options.copy(avoidAmbiguous = it))
            }
        }
    }
}

@Composable
private fun Toggle(@StringRes label: Int, hint: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    ToggleRow(stringResource(label), checked, onChange, detail = hint)
}

@StringRes
private fun strengthLabel(strength: Strength): Int = when (strength) {
    Strength.WEAK -> R.string.generator_weak
    Strength.FAIR -> R.string.generator_fair
    Strength.STRONG -> R.string.generator_strong
    Strength.EXCELLENT -> R.string.generator_excellent
}
```

- [ ] **Step 4: The Material check covers `ui/generator`**

Add `"**/ui/generator/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list.

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, `GeneratorViewModelTest` unchanged.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/generator apps/android/app/src/test/kotlin/net/havenkeys/android/ui/generator apps/android/app/build.gradle.kts
git commit -m "feat(android): the generator from the kit: output plate, length row and slider, character switches, toast on copy"
```

---

### Task 11: Unlock

**Files:**
- Modify: `ANDROID/ui/unlock/UnlockScreen.kt` (the form rewritten from the kit; `UnlockForm` becomes `internal`)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `ui/unlock`)
- Test: `ANDROID_TEST/ui/unlock/UnlockScreenTest.kt` (new)

**Interfaces:**
- Consumes: Task 2's `SecretTextField(hint)`; kit `InsetGroup`, `HavenButton`, `IconGlyph`, `HavenText`; `UnlockUiState`, `UnlockViewModel` (unchanged).
- Produces: `UnlockScreen(viewModel, activity, container, onUnlocked, modifier)` unchanged; `@Composable internal fun UnlockForm(state: UnlockUiState, onSubmit: (password: String, secretKey: String?) -> Unit, onBiometric: () -> Unit, modifier: Modifier = Modifier)`.

`UnlockScreen` itself (the biometric flow, `autoOffered` with `rememberSaveable` of a Boolean, `biometricUnlock`, `keystoreOrNull`) does not change. Its window is the same full-screen ground in `MainActivity` and in the translucent autofill and credential activities (`unlockThen`), so the form draws the pane itself.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/unlock/UnlockScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.unlock

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class UnlockScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val submitted = mutableListOf<Pair<String, String?>>()
    private var biometrics = 0

    private fun show(state: UnlockUiState = UnlockUiState()) = rule.setKit {
        UnlockForm(state, onSubmit = { p, k -> submitted += p to k }, onBiometric = { biometrics++ })
    }

    private fun field(label: Int) = rule.onNode(hasSetTextAction() and hasText(text(label)))
    private fun unlock() = rule.onNode(hasText(text(R.string.unlock_button)) and hasClickAction())
    private fun typed(label: Int) =
        field(label).fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text.orEmpty()

    @Test
    fun theMasterPasswordIsAPasswordFieldAndUnlockWaitsForIt() {
        show()
        field(R.string.unlock_password_hint).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        unlock().assertIsNotEnabled()
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        unlock().assertIsEnabled()
    }

    @Test
    fun submittingSendsThePasswordOnceAndEmptiesTheField() {
        show()
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        unlock().performClick()
        assertEquals(listOf("hunter2" to null), submitted)
        assertTrue(typed(R.string.unlock_password_hint).isEmpty())
        unlock().assertIsNotEnabled()
    }

    @Test
    fun aDeviceWithoutItsSecretKeyAsksForBoth() {
        show(UnlockUiState(needsSecretKey = true))
        field(R.string.unlock_secret_key).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        unlock().assertIsNotEnabled()
        field(R.string.unlock_secret_key).performTextInput("A3-KEY")
        unlock().performClick()
        assertEquals(listOf("hunter2" to "A3-KEY"), submitted)
        assertTrue(typed(R.string.unlock_secret_key).isEmpty())
    }

    @Test
    fun aWrongPasswordIsTheFieldsError() {
        show(UnlockUiState(errorCode = "wrong_password"))
        field(R.string.unlock_password_hint)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(errorText("wrong_password"))))
    }

    @Test
    fun whileBusyNothingCanBeSentTwice() {
        show(UnlockUiState(busy = true, offerBiometric = true))
        rule.onNode(hasText(text(R.string.unlock_busy)) and hasClickAction()).assertIsNotEnabled()
        rule.onNode(hasText(text(R.string.unlock_biometric)) and hasClickAction()).assertIsNotEnabled()
    }

    @Test
    fun biometricsIsOfferedOnlyWhenSetUp() {
        show(UnlockUiState(offerBiometric = true))
        rule.onNodeWithText(text(R.string.unlock_biometric)).performClick()
        assertEquals(1, biometrics)
    }

    @Test
    fun aRestoredUnlockScreenBringsNoPasswordBack() {
        val restoration = StateRestorationTester(rule)
        restoration.setContent { HavenTheme { UnlockForm(UnlockUiState(), { _, _ -> }, {}) } }
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        restoration.emulateSavedInstanceStateRestore()
        assertTrue(typed(R.string.unlock_password_hint).isEmpty())
    }
}
```

(`wrong_password` is one of `errorText`'s codes; if `ErrorText.kt` names it differently, use any code it maps.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*UnlockScreenTest*'`
Expected: compile error (`UnlockForm` is private).

- [ ] **Step 3: Rewrite the form**

In `ANDROID/ui/unlock/UnlockScreen.kt`, keep `UnlockScreen`, `biometricUnlock` and `keystoreOrNull` as they are. Replace `UnlockForm`, `UnlockActions`, `UnlockHeader` and `MaskedField` with:

```kotlin
/**
 * The master password and the Secret Key live only in this composition
 * (`remember { TextFieldState() }`, never `rememberTextFieldState`, which is
 * saved with the instance state) and are emptied the moment they are sent,
 * before Rust answers.
 */
@Composable
internal fun UnlockForm(
    state: UnlockUiState,
    onSubmit: (password: String, secretKey: String?) -> Unit,
    onBiometric: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val password = remember { TextFieldState() }
    val secretKey = remember { TextFieldState() }
    var passwordShown by remember { mutableStateOf(false) }
    var keyShown by remember { mutableStateOf(false) }
    val ready = password.text.isNotEmpty() && (!state.needsSecretKey || secretKey.text.isNotBlank()) && !state.busy
    val submit = {
        if (ready) {
            onSubmit(password.text.toString(), if (state.needsSecretKey) secretKey.text.toString() else null)
            password.clearText()
            secretKey.clearText()
            passwordShown = false
            keyShown = false
        }
    }
    val error = state.errorCode?.let { stringResource(errorText(it)) }

    Column(
        modifier
            .fillMaxSize()
            .background(HavenTheme.colors.pane)
            .safeDrawingPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        UnlockHeader(state.needsSecretKey)
        InsetGroup {
            row {
                SecretTextField(
                    password,
                    stringResource(R.string.unlock_password_hint),
                    revealed = passwordShown,
                    onRevealChange = { passwordShown = it },
                    error = error,
                    enabled = !state.busy,
                    imeAction = if (state.needsSecretKey) ImeAction.Next else ImeAction.Done,
                    onKeyboardAction = if (state.needsSecretKey) null else KeyboardActionHandler { submit() },
                )
            }
            if (state.needsSecretKey) {
                row {
                    SecretTextField(
                        secretKey,
                        stringResource(R.string.unlock_secret_key),
                        revealed = keyShown,
                        onRevealChange = { keyShown = it },
                        enabled = !state.busy,
                        onKeyboardAction = { submit() },
                        hint = stringResource(R.string.unlock_secret_key_placeholder),
                    )
                }
            }
        }
        HavenButton(
            stringResource(if (state.busy) R.string.unlock_busy else R.string.unlock_button),
            onClick = submit,
            Modifier.fillMaxWidth(),
            enabled = ready,
        )
        if (state.offerBiometric) {
            HavenButton(
                stringResource(R.string.unlock_biometric),
                onClick = onBiometric,
                Modifier.fillMaxWidth(),
                style = ButtonStyle.Secondary,
                enabled = !state.busy,
            )
        }
    }
}

@Composable
private fun UnlockHeader(needsSecretKey: Boolean) {
    val colors = HavenTheme.colors
    IconGlyph(HavenIcon.Lock, contentDescription = null, tint = colors.brass, size = 40.dp)
    HavenText(
        stringResource(R.string.unlock_title),
        Modifier.semantics { heading() },
        style = HavenTheme.type.display.copy(textAlign = TextAlign.Center),
        color = colors.textStrong,
    )
    if (needsSecretKey) {
        HavenText(
            stringResource(R.string.unlock_enter_password_and_key),
            style = HavenTheme.type.body.copy(textAlign = TextAlign.Center),
            color = colors.muted,
        )
    }
}
```

Imports: drop every `androidx.compose.material…`, `KeyboardActions`, `KeyboardOptions`, `KeyboardType`, `PasswordVisualTransformation`, `VisualTransformation`, `size`; add `androidx.compose.foundation.background`, `androidx.compose.foundation.text.input.KeyboardActionHandler`, `androidx.compose.foundation.text.input.TextFieldState`, `androidx.compose.foundation.text.input.clearText`, `androidx.compose.ui.semantics.heading`, `androidx.compose.ui.semantics.semantics`, `androidx.compose.ui.text.style.TextAlign`, `net.havenkeys.android.ui.kit.{ButtonStyle, HavenButton, HavenIcon, HavenText, IconGlyph, InsetGroup, SecretTextField}`. Keep `androidx.compose.ui.text.input.ImeAction`.

The error now sits in the password field (its semantics error, read by TalkBack) instead of a line under the fields.

- [ ] **Step 4: The Material check covers `ui/unlock`**

Add `"**/ui/unlock/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list.

- [ ] **Step 5: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, `UnlockViewModelTest`, `EnrollmentTest`, `HavenNavHostTest` unchanged.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/unlock apps/android/app/src/test/kotlin/net/havenkeys/android/ui/unlock apps/android/app/build.gradle.kts
git commit -m "feat(android): unlock from the kit; the master password and Secret Key are emptied as they are sent and never restored"
```

---

### Task 12: Onboarding

**Files:**
- Modify: `ANDROID/ui/onboarding/OnboardingScreen.kt` (rewritten from the kit), `ANDROID/ui/onboarding/KitScanner.kt` (its message)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `ui/onboarding`)
- Test: `ANDROID_TEST/ui/onboarding/OnboardingScreenTest.kt` (new)

**Interfaces:**
- Consumes: Task 2's field `hint`; kit `HavenScaffold`, `InsetGroup`, `GroupRow`, `GroupRowText`, `GroupRowField`, `SectionHeader`, `HavenTextField`, `SecretTextField`, `HavenButton`, `HavenIconButton`; `ui.shell.LargeTitle`, `ErrorLine`; `OnboardingViewModel`, `OnboardingUiState` (unchanged).
- Produces: `OnboardingScreen(viewModel, onDone, modifier)` unchanged.

The file keeps its `@file:Suppress("TooManyFunctions")` and its comment. The camera code in `KitScanner.kt` (permission, CameraX, `toLumaFrame`) does not change.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/onboarding/OnboardingScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.onboarding

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.R
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class OnboardingScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val accounts = FakeAccountRepository()

    private fun show(vm: OnboardingViewModel = OnboardingViewModel(accounts)) =
        rule.setKit { OnboardingScreen(vm, onDone = {}) }

    private fun choose(id: Int) = rule.onNode(hasText(text(id)) and hasClickAction()).performClick()
    private fun field(id: Int) = rule.onNode(hasSetTextAction() and hasText(text(id)))
    private fun typed(id: Int) = field(id).fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text.orEmpty()

    @Test
    fun eachWayOpensItsStepAndBackReturns() {
        show()
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_server).assertExists()
        rule.onNodeWithContentDescription(text(R.string.onboarding_back)).performClick()
        rule.onNode(hasText(text(R.string.onboarding_invite_choice)) and hasClickAction()).assertExists()
    }

    @Test
    fun signingInWaitsForAllFourAndSendsThem() {
        show()
        choose(R.string.onboarding_type_kit)
        val signIn = rule.onNode(hasText(text(R.string.onboarding_sign_in)) and hasClickAction())
        field(R.string.onboarding_server).performTextInput("https://vault.example.com")
        field(R.string.onboarding_email).performTextInput("sam@example.com")
        field(R.string.onboarding_secret_key).performTextInput("A3-KEY")
        signIn.assertIsNotEnabled()
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        signIn.assertIsEnabled().performClick()
        rule.waitForIdle()
        assertTrue("signIn" in accounts.calls)
    }

    @Test
    fun theSecretsAreTypedAsPasswords() {
        show()
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_secret_key).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        field(R.string.onboarding_master_password).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
    }

    @Test
    fun aNewAccountNeedsALongPasswordTypedTwice() {
        show()
        choose(R.string.onboarding_invite_choice)
        field(R.string.onboarding_invite).performTextInput("invite-token")
        field(R.string.onboarding_master_password).performTextInput("short")
        field(R.string.onboarding_master_password)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.onboarding_too_short)))
        field(R.string.onboarding_master_password).performTextInput(" but longer now")
        field(R.string.onboarding_repeat_password).performTextInput("something else")
        field(R.string.onboarding_repeat_password)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.onboarding_mismatch)))
        rule.onNode(hasText(text(R.string.onboarding_create)) and hasClickAction()).assertIsNotEnabled()
    }

    @Test
    fun aRestoreKeepsTheServerButNotTheSecrets() {
        val restoration = StateRestorationTester(rule)
        val vm = OnboardingViewModel(accounts)
        restoration.setContent { HavenTheme { OnboardingScreen(vm, onDone = {}) } }
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_server).performTextInput("https://vault.example.com")
        field(R.string.onboarding_secret_key).performTextInput("A3-KEY")
        restoration.emulateSavedInstanceStateRestore()
        assertEquals("https://vault.example.com", typed(R.string.onboarding_server))
        assertTrue(typed(R.string.onboarding_secret_key).isEmpty())
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*OnboardingScreenTest*'`
Expected: FAIL (Material fields put the label elsewhere; the too-short state is a supporting text, not the field's error).

- [ ] **Step 3: Rewrite `OnboardingScreen.kt`**

Structure (exact code for the parts that carry behaviour; the rest is layout):

```kotlin
@Composable
fun OnboardingScreen(viewModel: OnboardingViewModel, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val mode = state.mode
    LaunchedEffect(state.done) { if (state.done) onDone() }
    BackHandler(enabled = mode != OnboardingUiState.Mode.CHOOSE) { viewModel.back() }
    HavenScaffold(
        modifier = modifier,
        topBar = { OnboardingBar(showBack = mode != OnboardingUiState.Mode.CHOOSE, onBack = viewModel::back) },
    ) { _ ->
        val content = Modifier.fillMaxSize()
        when (mode) {
            OnboardingUiState.Mode.CHOOSE -> ChooseStep(viewModel::choose, content)
            OnboardingUiState.Mode.SCAN -> ScanStep(state.errorCode, viewModel::onFrame, viewModel::back, content)
            OnboardingUiState.Mode.TYPE -> TypeStep(state, viewModel::signIn, content)
            OnboardingUiState.Mode.INVITE -> InviteStep(state, viewModel::activate, content)
            OnboardingUiState.Mode.PASSWORD -> KitPasswordStep(state, viewModel::signInWithKit, content)
        }
    }
}

/** Back (from a step) above the large title; the slot keeps its height so the title does not jump. */
@Composable
private fun OnboardingBar(showBack: Boolean, onBack: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = HavenSpacing.gutter)) {
        Box(Modifier.heightIn(min = HavenSpacing.touch)) {
            if (showBack) {
                HavenIconButton(
                    HavenIcon.ChevronLeft,
                    stringResource(R.string.onboarding_back),
                    onClick = onBack,
                    modifier = Modifier.offset(x = (-12).dp),
                )
            }
        }
        LargeTitle(stringResource(R.string.onboarding_title))
    }
}

@Composable
private fun ChooseStep(onChoose: (OnboardingUiState.Mode) -> Unit, modifier: Modifier) {
    FormColumn(modifier) {
        HavenText(
            stringResource(R.string.onboarding_how),
            style = HavenTheme.type.titleSmall,
            color = HavenTheme.colors.textStrong,
        )
        InsetGroup {
            row { Choice(HavenIcon.Qr, R.string.onboarding_scan_kit, R.string.onboarding_scan_kit_sub) { onChoose(OnboardingUiState.Mode.SCAN) } }
            row { Choice(HavenIcon.Edit, R.string.onboarding_type_kit, R.string.onboarding_type_kit_sub) { onChoose(OnboardingUiState.Mode.TYPE) } }
            row { Choice(HavenIcon.Plus, R.string.onboarding_invite_choice, R.string.onboarding_invite_sub) { onChoose(OnboardingUiState.Mode.INVITE) } }
        }
    }
}

@Composable
private fun Choice(icon: HavenIcon, @StringRes title: Int, @StringRes subtitle: Int, onClick: () -> Unit) {
    GroupRow(onClick = onClick, icon = icon) { GroupRowText(stringResource(title), stringResource(subtitle)) }
}

@Composable
private fun TypeStep(
    state: OnboardingUiState,
    onSignIn: (server: String, email: String, password: String, secretKey: String) -> Unit,
    modifier: Modifier,
) {
    // Not secrets: kept across a restore, as before. The secrets are `remember`ed only.
    val server = rememberTextFieldState()
    val email = rememberTextFieldState()
    val secretKey = remember { TextFieldState() }
    val password = remember { TextFieldState() }
    var keyShown by remember { mutableStateOf(false) }
    var passwordShown by remember { mutableStateOf(false) }
    val ready = server.text.isNotBlank() && email.text.isNotBlank() && secretKey.text.isNotBlank() &&
        password.text.isNotEmpty()
    FormColumn(modifier) {
        InsetGroup {
            row {
                HavenTextField(
                    server,
                    stringResource(R.string.onboarding_server),
                    enabled = !state.busy,
                    keyboardOptions = plainKeyboard(KeyboardType.Uri),
                    hint = stringResource(R.string.onboarding_server_hint),
                )
            }
            row {
                HavenTextField(
                    email,
                    stringResource(R.string.onboarding_email),
                    placeholder = stringResource(R.string.onboarding_email_placeholder),
                    enabled = !state.busy,
                    keyboardOptions = plainKeyboard(KeyboardType.Email),
                )
            }
            row {
                SecretTextField(
                    secretKey,
                    stringResource(R.string.onboarding_secret_key),
                    revealed = keyShown,
                    onRevealChange = { keyShown = it },
                    enabled = !state.busy,
                    imeAction = ImeAction.Next,
                    hint = stringResource(R.string.onboarding_secret_key_hint),
                )
            }
            row {
                SecretTextField(
                    password,
                    stringResource(R.string.onboarding_master_password),
                    revealed = passwordShown,
                    onRevealChange = { passwordShown = it },
                    enabled = !state.busy,
                    imeAction = ImeAction.Next,
                )
            }
        }
        MaybeError(state.errorCode)
        SubmitButton(
            text = stringResource(if (state.busy) R.string.onboarding_signing_in else R.string.onboarding_sign_in),
            enabled = ready && !state.busy,
            onClick = {
                onSignIn(server.text.toString(), email.text.toString(), password.text.toString(), secretKey.text.toString())
            },
        )
        Note(stringResource(R.string.onboarding_sign_in_note))
    }
}
```

`InviteStep`, written the same way: `invite`, `password`, `repeat` are `remember { TextFieldState() }` with their own `shown` flags; `tooShort = password.text.isNotEmpty() && password.text.length < MIN_PASSWORD_LENGTH`, `mismatch = repeat.text.isNotEmpty() && repeat.text.toString() != password.text.toString()`, `ready = invite.text.isNotBlank() && password.text.length >= MIN_PASSWORD_LENGTH && repeat.text.toString() == password.text.toString()`. The invite field is a `SecretTextField` with `hint = invite_hint`; the master password has `error = if (tooShort) stringResource(R.string.onboarding_too_short) else null` and `hint = stringResource(R.string.onboarding_password_hint)` (the error takes the hint's place, as before the supporting text switched); the repeat field has `error = if (mismatch) stringResource(R.string.onboarding_mismatch) else null`. Then `MaybeError`, `SubmitButton(create/creating, ready && !busy) { onActivate(invite.text.toString(), password.text.toString()) }`, `Note(create_note)`.

`KitPasswordStep`: `state.preview?.let { KitSummary(it) }`, `HavenText(onboarding_kit_password, style = body, color = text)`, one `InsetGroup { row { SecretTextField(password, master_password, …) } }` with `password = remember { TextFieldState() }`, `MaybeError`, `SubmitButton(sign_in/signing_in, password.text.isNotEmpty() && !busy) { onSignIn(password.text.toString()) }`, `Note(sign_in_note)`.

```kotlin
@Composable
private fun KitSummary(preview: KitPreview) {
    Column {
        SectionHeader(stringResource(R.string.onboarding_kit_found))
        InsetGroup {
            row { GroupRow { GroupRowField(stringResource(R.string.onboarding_email), preview.email) } }
            row { GroupRow { GroupRowField(stringResource(R.string.onboarding_server), preview.serverUrl) } }
        }
    }
}

@Composable
private fun ScanStep(errorCode: String?, onFrame: (LumaFrame) -> Unit, onCancel: () -> Unit, modifier: Modifier) {
    Box(modifier) {
        KitScanner(onFrame = onFrame, modifier = Modifier.fillMaxSize())
        Column(
            Modifier
                .align(Alignment.BottomCenter)
                .padding(HavenSpacing.gutter)
                .fillMaxWidth()
                .clip(HavenShape.group)
                .background(HavenTheme.colors.raised)
                .padding(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            HavenText(stringResource(R.string.onboarding_scanning), style = HavenTheme.type.value, color = HavenTheme.colors.textStrong)
            MaybeError(errorCode)
            HavenButton(stringResource(R.string.onboarding_cancel), onClick = onCancel, style = ButtonStyle.Secondary)
        }
    }
}

private fun plainKeyboard(type: KeyboardType) =
    KeyboardOptions(keyboardType = type, autoCorrectEnabled = false, imeAction = ImeAction.Next)

@Composable
private fun FormColumn(modifier: Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier
            .verticalScroll(rememberScrollState())
            .padding(horizontal = HavenSpacing.gutter, vertical = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        content = content,
    )
}

@Composable
private fun SubmitButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    HavenButton(text, onClick = onClick, Modifier.fillMaxWidth(), enabled = enabled)
}

@Composable
private fun MaybeError(code: String?) {
    if (code != null) ErrorLine(code)
}

@Composable
private fun Note(text: String) {
    HavenText(text, style = HavenTheme.type.rowSubtitle, color = HavenTheme.colors.muted)
}
```

(`HavenScaffold` already pads for the keyboard; `FormColumn` no longer needs `imePadding`.) `LabelledValue` and `SecretInput` are deleted. Imports: no `androidx.compose.material…`; `rememberTextFieldState` from `androidx.compose.foundation.text.input` only for server and email.

- [ ] **Step 4: The scanner's message from the kit**

In `ANDROID/ui/onboarding/KitScanner.kt`, replace the two Material lines of `ScannerMessage` with:

```kotlin
            HavenText(text, style = HavenTheme.type.body.copy(textAlign = TextAlign.Center), color = HavenTheme.colors.text)
            if (action != null) HavenButton(action, onClick = onAction, style = ButtonStyle.Secondary)
```

and swap the Material imports for `net.havenkeys.android.ui.kit.{ButtonStyle, HavenButton, HavenText}` and `net.havenkeys.android.ui.theme.HavenTheme`.

- [ ] **Step 5: The Material check covers `ui/onboarding`**

Add `"**/ui/onboarding/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list.

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, `OnboardingViewModelTest` unchanged.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/onboarding apps/android/app/src/test/kotlin/net/havenkeys/android/ui/onboarding apps/android/app/build.gradle.kts
git commit -m "feat(android): onboarding from the kit; secrets typed masked and never restored, server and email kept"
```

---

### Task 13: Devices and autofill setup

**Files:**
- Modify: `ANDROID/ui/settings/DevicesScreen.kt`, `ANDROID/ui/autofillsetup/AutofillSetupScreen.kt` (rewritten from the kit)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers all of `ui/settings` and `ui/autofillsetup`)
- Test: `ANDROID_TEST/ui/settings/DevicesScreenTest.kt`, `ANDROID_TEST/ui/autofillsetup/AutofillSetupScreenTest.kt` (new)

**Interfaces:**
- Consumes: Task 4's `ScreenBar`; kit `HavenScaffold`, `GroupRow`, `GroupRowText`, `Pill`, `HavenButton`, `HavenDialog`, `InsetGroup`, `SectionHeader`, `IconGlyph`; `ui.shell.{LargeTitle, ErrorLine, EmptyLine, insetGroup}`; `DevicesViewModel` (unchanged).
- Produces: `DevicesScreen(viewModel, online, onBack, onLock, modifier)` and `AutofillSetupScreen(online, onBack, onLock, modifier)` unchanged.

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/ui/settings/DevicesScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.settings

import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.DeviceInfo

@RunWith(RobolectricTestRunner::class)
class DevicesScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    private val accounts = FakeAccountRepository().apply {
        deviceList = Outcome.Ok(
            listOf(
                DeviceInfo("d1", "Pixel 8", "2026-01-01T00:00:00Z", null, true),
                DeviceInfo("d2", "Work laptop", "2026-01-01T00:00:00Z", "2026-10-03T10:00:00Z", false),
            ),
        )
    }

    private fun show() {
        val vm = DevicesViewModel(accounts, VaultEventsHub())
        rule.setKit { DevicesScreen(vm, online = true, onBack = {}, onLock = {}) }
    }

    @Test
    fun eachDeviceIsARowAndThisPhoneIsMarked() {
        show()
        rule.onNodeWithText(text(R.string.devices_this_phone)).assertExists()
        rule.onNode(hasText("Work laptop")).assertExists()
    }

    @Test
    fun revokingAsksFirstAndEndsThatSession() {
        show()
        rule.onAllNodesWithText(text(R.string.devices_revoke))[1].performClick()
        rule.onNodeWithText(text(R.string.devices_revoke_confirm, "Work laptop")).assertExists()
        rule.onNode(hasText(text(R.string.devices_revoke)) and hasAnyAncestor(isDialog()) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertTrue("revoke:d2" in accounts.calls)
    }

    @Test
    fun cancelEndsNothing() {
        show()
        rule.onAllNodesWithText(text(R.string.devices_revoke))[0].performClick()
        rule.onNodeWithText(text(R.string.devices_revoke_this)).assertExists()
        rule.onNodeWithText(text(R.string.settings_cancel)).performClick()
        rule.waitForIdle()
        assertTrue(accounts.calls.none { it.startsWith("revoke") })
    }
}
```

`ANDROID_TEST/ui/autofillsetup/AutofillSetupScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.autofillsetup

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.setKit
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class AutofillSetupScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    @Test
    fun whenAutofillIsOffItOffersSettingsAndExplainsChrome() {
        rule.setKit { AutofillSetupScreen(online = false, onBack = {}, onLock = {}) }
        rule.onNode(hasText(text(R.string.autofill_setup_title)) and isHeading()).assertIsDisplayed()
        rule.onNodeWithText(text(R.string.autofill_setup_off)).assertExists()
        rule.onNode(hasText(text(R.string.autofill_setup_open)) and hasClickAction()).assertExists()
        rule.onNode(hasText(text(R.string.autofill_setup_chrome_title)) and isHeading()).assertExists()
        rule.onNodeWithText(text(R.string.vault_offline)).assertExists()
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*DevicesScreenTest*' --tests '*AutofillSetupScreenTest*'`
Expected: FAIL (Material dialog has no kit title node to find; no headings).

- [ ] **Step 3: Rewrite `DevicesScreen.kt`**

```kotlin
/** The account's devices, this one marked; Revoke ends a device's session on the server. */
@Composable
fun DevicesScreen(
    viewModel: DevicesViewModel,
    online: Boolean,
    onBack: () -> Unit,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    var revoking by remember { mutableStateOf<DeviceInfo?>(null) }
    val gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

    HavenScaffold(modifier = modifier, topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) }) { padding ->
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") { LargeTitle(stringResource(R.string.devices_title), gutter) }
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, gutter) } }
            if (state.devices.isEmpty() && !state.loading && state.errorCode == null) {
                item(key = "empty") { EmptyLine(stringResource(R.string.devices_empty), gutter) }
            }
            insetGroup(state.devices, key = { it.id }) { device -> DeviceRow(device, onRevoke = { revoking = device }) }
        }
    }

    revoking?.let { device ->
        HavenDialog(
            title = if (device.current) {
                stringResource(R.string.devices_revoke_this)
            } else {
                stringResource(R.string.devices_revoke_confirm, device.name)
            },
            onDismiss = { revoking = null },
            confirm = DialogAction(
                stringResource(R.string.devices_revoke),
                {
                    revoking = null
                    viewModel.revoke(device.id)
                },
                danger = true,
            ),
            dismiss = DialogAction(stringResource(R.string.settings_cancel), { revoking = null }),
        )
    }
}

@Composable
private fun DeviceRow(device: DeviceInfo, onRevoke: () -> Unit) {
    val seen = device.lastSeenAt?.let(::relativeTime)
    GroupRow(
        trailing = {
            HavenButton(stringResource(R.string.devices_revoke), onClick = onRevoke, style = ButtonStyle.Quiet)
        },
    ) {
        if (device.current) Pill(stringResource(R.string.devices_this_phone))
        GroupRowText(
            device.name,
            if (seen != null) stringResource(R.string.devices_last_seen, seen) else stringResource(R.string.devices_never_seen),
        )
    }
}
```

`relativeTime` stays as it is. Imports: no Material, no `HavenTopBar`; add `ScreenBar`, the kit and `ui.shell` names above, `androidx.compose.foundation.layout.PaddingValues`.

- [ ] **Step 4: Rewrite `AutofillSetupScreen`'s drawing**

Keep the state (`enabled`, `passkeysOn`, `openFailed`, `providerOpenFailed`, the `LifecycleResumeEffect`) and every private function after `ServiceState` (`isOurAutofillService`, `requestAutofillService`, `isOurCredentialProvider`, `openProviderSettings`, `openCredentialProviderSettings`) as they are. Replace the `Scaffold(…)` and the three drawing composables with:

```kotlin
    HavenScaffold(modifier = modifier, topBar = { ScreenBar(onBack = onBack, online = online, onLock = onLock) }) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            LargeTitle(stringResource(R.string.autofill_setup_title))
            ServiceState(enabled)
            if (!enabled) {
                HavenButton(
                    stringResource(R.string.autofill_setup_open),
                    onClick = { openFailed = !requestAutofillService(context) },
                    Modifier.fillMaxWidth(),
                )
            }
            if (openFailed) Problem(stringResource(R.string.autofill_setup_unavailable))
            Heading(stringResource(R.string.autofill_setup_chrome_title))
            HavenText(stringResource(R.string.autofill_setup_chrome), color = HavenTheme.colors.muted)
            PasskeysSection(passkeysOn, providerOpenFailed) { providerOpenFailed = !openProviderSettings(context) }
        }
    }
```

```kotlin
@Composable
private fun PasskeysSection(passkeysOn: Boolean, openFailed: Boolean, onOpen: () -> Unit) {
    Heading(stringResource(R.string.autofill_setup_passkeys_title))
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        HavenText(stringResource(R.string.autofill_setup_passkeys_old), color = HavenTheme.colors.muted)
        return
    }
    HavenText(
        stringResource(if (passkeysOn) R.string.autofill_setup_passkeys_on else R.string.autofill_setup_passkeys_off),
        color = HavenTheme.colors.text,
    )
    if (!passkeysOn) {
        // Secondary: Open settings above may already be the screen's one primary.
        HavenButton(
            stringResource(R.string.autofill_setup_passkeys_open),
            onClick = onOpen,
            Modifier.fillMaxWidth(),
            style = ButtonStyle.Secondary,
        )
    }
    if (openFailed) Problem(stringResource(R.string.autofill_setup_passkeys_unavailable))
}

@Composable
private fun ServiceState(enabled: Boolean) {
    InsetGroup {
        row {
            GroupRow(icon = if (enabled) HavenIcon.Check else HavenIcon.Alert) {
                GroupRowText(stringResource(if (enabled) R.string.autofill_setup_on else R.string.autofill_setup_off))
            }
        }
    }
}

@Composable
private fun Heading(text: String) {
    HavenText(
        text,
        Modifier.padding(top = 12.dp).semantics { heading() },
        style = HavenTheme.type.groupTitle,
        color = HavenTheme.colors.textStrong,
    )
}

@Composable
private fun Problem(text: String) {
    HavenText(text, Modifier.semantics { liveRegion = LiveRegionMode.Polite }, color = HavenTheme.colors.danger)
}
```

- [ ] **Step 5: The Material check covers both packages**

In `forbidMaterialInKit`'s `include(…)` list, replace the four `"**/ui/settings/Settings….kt"` lines with `"**/ui/settings/**/*.kt",` and add `"**/ui/autofillsetup/**/*.kt",`.

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, `DevicesViewModelTest` unchanged.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/settings/DevicesScreen.kt apps/android/app/src/main/kotlin/net/havenkeys/android/ui/autofillsetup apps/android/app/src/test/kotlin/net/havenkeys/android/ui/settings apps/android/app/src/test/kotlin/net/havenkeys/android/ui/autofillsetup apps/android/app/build.gradle.kts
git commit -m "feat(android): devices and autofill setup from the kit; revoking asks in a kit dialog"
```

---
### Task 14: "Search HavenKeys…", the autofill confirmation and the copy row's glyph

**Files:**
- Create: `ANDROID/autofill/AutofillSearchScreen.kt` (the composables move out of the activity and are rebuilt)
- Modify: `ANDROID/autofill/AutofillSearchActivity.kt` (calls `AutofillSearchScreen`; its private composables and `CallerApp` move)
- Modify: `ANDROID/autofill/WalletConfirmDialog.kt` (a kit `HavenDialog` with the alternative)
- Modify: `RES/drawable/ic_autofill_copy.xml` (the desktop's copy glyph)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `autofill/`)
- Test: `ANDROID_TEST/autofill/AutofillSearchScreenTest.kt`, `ANDROID_TEST/autofill/WalletConfirmDialogTest.kt`, `ANDROID_TEST/autofill/CopyRowIconTest.kt` (new)

**Interfaces:**
- Consumes: Task 3's `HavenDialog(alternative)`; internal `SearchField(state, modifier)` (`ui.search`); `ui.shell.{insetGroup, ErrorLine, EmptyLine}`; kit `ItemRow`, `RowLeading`; `AutofillMatch(id, title, username, hasTotp)`.
- Produces (package `net.havenkeys.android.autofill`):
  - `internal data class CallerApp(val packageName: String, val label: String?)`
  - `@Composable internal fun AutofillSearchScreen(search: suspend (String) -> Outcome<List<AutofillMatch>>, app: CallerApp, onConfirmed: suspend (AutofillMatch) -> String?, modifier: Modifier = Modifier)`
  - `WalletConfirmDialog(question, detail, confirmLabel, alternativeLabel, onConfirm, onAlternative, onDismiss)` unchanged.

The activities' window flags (`FLAG_SECURE`, autofill exclusion, touch filtering) do not change; the kit dialog sets its own on its window as the Material one did through `SecureDialogWindow`. The dropdown row (`res/layout/autofill_item.xml`, a RemoteViews row Android draws) and the keyboard chips stay as they are; only the copy row's glyph changes.

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/autofill/AutofillSearchScreenTest.kt`:

```kotlin
package net.havenkeys.android.autofill

import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.AutofillMatch

@RunWith(RobolectricTestRunner::class)
class AutofillSearchScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    private val repo = FakeAutofillRepository().apply {
        matchList = Outcome.Ok(listOf(AutofillMatch("m1", "GitHub", "sam", false)))
    }
    private val confirmed = mutableListOf<String>()

    private fun show(answer: String?) = rule.setKit {
        AutofillSearchScreen(
            search = repo::search,
            app = CallerApp("com.example.app", "Example"),
            onConfirmed = { match ->
                confirmed += match.id
                answer
            },
        )
    }

    private fun search(query: String) {
        rule.onNode(hasSetTextAction()).performTextInput(query)
        rule.mainClock.advanceTimeBy(500)
        rule.waitForIdle()
    }

    @Test
    fun aPickedLoginIsUsedOnlyAfterTheAppIsNamedAndConfirmed() {
        show(answer = null)
        search("git")
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.onNodeWithText(text(R.string.autofill_use_in_app, "GitHub", "com.example.app")).assertExists()
        rule.onNodeWithText(text(R.string.autofill_app_label, "Example")).assertExists()
        assertEquals(emptyList<String>(), confirmed)
        rule.onNode(hasText(text(R.string.autofill_use)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        assertEquals(listOf("m1"), confirmed)
    }

    @Test
    fun aRefusedFillSaysWhy() {
        show(answer = "not_found")
        search("git")
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.autofill_use)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        rule.onNodeWithText(text(errorText("not_found"))).assertExists()
    }

    @Test
    fun cancelUsesNothing() {
        show(answer = null)
        search("git")
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.autofill_cancel)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        assertEquals(emptyList<String>(), confirmed)
    }

    @Test
    fun nothingFoundSaysSo() {
        repo.matchList = Outcome.Ok(emptyList())
        show(answer = null)
        search("zzz")
        rule.onNodeWithText(text(R.string.vault_no_matches)).assertExists()
    }
}
```

`ANDROID_TEST/autofill/WalletConfirmDialogTest.kt`:

```kotlin
package net.havenkeys.android.autofill

import android.view.KeyEvent
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowDialog

@RunWith(RobolectricTestRunner::class)
class WalletConfirmDialogTest {
    @get:Rule
    val rule = createComposeRule()

    private val answers = mutableListOf<String>()

    private fun show(alternative: String? = "Fill without documents") = rule.setKit {
        WalletConfirmDialog(
            question = "Fill your identity in shop.example?",
            detail = "The app calls itself Shop",
            confirmLabel = "Fill with documents",
            alternativeLabel = alternative,
            onConfirm = { answers += "confirm" },
            onAlternative = { answers += "alternative" },
            onDismiss = { answers += "dismiss" },
        )
    }

    private fun pressBack() = rule.runOnIdle {
        val window = ShadowDialog.getLatestDialog().window!!
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
    }

    @Test
    fun theQuestionAndTheAppsOwnNameShow() {
        show()
        rule.onNodeWithText("Fill your identity in shop.example?").assertExists()
        rule.onNodeWithText("The app calls itself Shop").assertExists()
    }

    @Test
    fun eachAnswerAnswersOnce() {
        show()
        rule.onNodeWithText("Fill without documents").performClick()
        rule.onNodeWithText("Fill with documents").performClick()
        pressBack()
        rule.waitForIdle()
        assertEquals(listOf("alternative"), answers)
    }

    @Test
    fun backBeforeAnyAnswerCancels() {
        show(alternative = null)
        pressBack()
        rule.waitForIdle()
        assertEquals(listOf("dismiss"), answers)
    }
}
```

`ANDROID_TEST/autofill/CopyRowIconTest.kt`:

```kotlin
package net.havenkeys.android.autofill

import java.io.File
import net.havenkeys.android.ui.kit.HavenIcon
import org.junit.Assert.assertEquals
import org.junit.Test

class CopyRowIconTest {
    @Test
    fun theCopyRowDrawsTheDesktopsCopyGlyph() {
        val xml = File("src/main/res/drawable/ic_autofill_copy.xml").readText()
        val path = Regex("android:pathData=\"([^\"]+)\"").find(xml)?.groupValues?.get(1)
        assertEquals(HavenIcon.Copy.path, path)
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*AutofillSearchScreenTest*' --tests '*WalletConfirmDialogTest*' --tests '*CopyRowIconTest*'`
Expected: compile error (`AutofillSearchScreen`, `CallerApp` are private to the activity file); `CopyRowIconTest` fails on Material's path.

- [ ] **Step 3: Write `AutofillSearchScreen.kt`**

```kotlin
package net.havenkeys.android.autofill

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.RowLeading
import net.havenkeys.android.ui.search.SearchField
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.AutofillMatch

private const val SEARCH_DEBOUNCE_MS = 200L

/** The app being filled: [packageName] identifies it; [label] is the app's own choice. */
internal data class CallerApp(val packageName: String, val label: String?)

/**
 * "Search HavenKeys…" (Android spec §7.2): the user finds a login for an app
 * nothing binds yet, then confirms "Use <login> in <app>?". [onConfirmed]
 * answers an error code, or null once it has filled. The query is
 * `remember`ed only, never saved.
 */
@Composable
internal fun AutofillSearchScreen(
    search: suspend (String) -> Outcome<List<AutofillMatch>>,
    app: CallerApp,
    onConfirmed: suspend (AutofillMatch) -> String?,
    modifier: Modifier = Modifier,
) {
    val query = remember { TextFieldState() }
    var results by remember { mutableStateOf(emptyList<AutofillMatch>()) }
    var picked by remember { mutableStateOf<AutofillMatch?>(null) }
    var errorCode by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    val gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

    LaunchedEffect(query) {
        snapshotFlow { query.text.toString() }.collectLatest { typed ->
            if (typed.isBlank()) {
                results = emptyList()
            } else {
                delay(SEARCH_DEBOUNCE_MS)
                results = (search(typed.trim()) as? Outcome.Ok)?.value.orEmpty()
            }
        }
    }
    LaunchedEffect(focus) { focus.requestFocus() }

    Column(modifier.fillMaxSize().background(HavenTheme.colors.pane).safeDrawingPadding().imePadding()) {
        SearchField(query, gutter.padding(vertical = 8.dp).fillMaxWidth().focusRequester(focus))
        errorCode?.let { ErrorLine(it, gutter) }
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = HavenSpacing.gutter)) {
            if (query.text.isNotBlank() && results.isEmpty()) {
                item(key = "none") { EmptyLine(stringResource(R.string.vault_no_matches), gutter) }
            }
            insetGroup(results, key = { it.id }) { match ->
                ItemRow(
                    match.title,
                    match.username,
                    RowLeading.Monogram(match.title),
                    onClick = { if (!busy) picked = match },
                    hasCode = match.hasTotp,
                )
            }
        }
    }

    picked?.let { match ->
        // The package name is what identifies the app (spec §7.2); the label is
        // the app's own choice and could name anything, even this login.
        HavenDialog(
            title = stringResource(R.string.autofill_use_in_app, match.title, app.packageName),
            onDismiss = { picked = null },
            confirm = DialogAction(stringResource(R.string.autofill_use), {
                busy = true
                scope.launch {
                    errorCode = onConfirmed(match)
                    busy = false
                    picked = null
                }
            }),
            message = app.label?.takeIf { it.isNotBlank() && it != app.packageName }
                ?.let { stringResource(R.string.autofill_app_label, it) },
            dismiss = DialogAction(stringResource(R.string.autofill_cancel), { picked = null }),
            busy = busy,
        )
    }
}
```

In `AutofillSearchActivity.kt`, delete `CallerApp`, `SearchScreen`, `Results`, `ConfirmUse` and `SEARCH_DEBOUNCE_MS`, call `AutofillSearchScreen(search = container.autofillRepository::search, app = app, onConfirmed = { match -> bindAndFill(tapped, match) })` inside `HavenTheme { }`, and drop the Material and now-unused imports. The activity's `onCreate` flags and `bindAndFill` do not change.

- [ ] **Step 4: The confirmation from the kit**

Replace `WalletConfirmDialog`'s body (keep its KDoc and signature):

```kotlin
    // HavenDialog answers once, sets FLAG_SECURE on its window, drops taps
    // through overlays and keeps the window from autofill.
    HavenDialog(
        title = question,
        onDismiss = onDismiss,
        confirm = DialogAction(confirmLabel, onConfirm),
        message = detail,
        dismiss = DialogAction(stringResource(R.string.autofill_cancel), onDismiss),
        alternative = alternativeLabel?.let { DialogAction(it, onAlternative) },
    )
```

with imports `androidx.compose.ui.res.stringResource`, `net.havenkeys.android.R`, `net.havenkeys.android.ui.kit.{DialogAction, HavenDialog}` only. `WalletAnswers.kt` keeps calling it unchanged.

- [ ] **Step 5: The copy row's glyph**

Replace `RES/drawable/ic_autofill_copy.xml` with:

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- The desktop's "copy" glyph (HavenIcon.Copy, Icon.tsx): 24 grid, 1.6 stroke, round caps and joins.
     Tinted by the dropdown row and the keyboard chip. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:tint="?android:attr/textColorPrimary"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:fillColor="@android:color/transparent"
        android:pathData="M9 8.5h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1zM16 8.5V6a1 1 0 0 0-1-1H6a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h2"
        android:strokeColor="@android:color/white"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:strokeWidth="1.6" />
</vector>
```

(The path must equal `HavenIcon.Copy.path` character for character; `CopyRowIconTest` checks it.)

- [ ] **Step 6: The Material check covers `autofill/`**

Add `"**/autofill/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list.

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug`
Expected: all pass, the autofill planner and wallet tests unchanged, `ManifestTest` unchanged.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/autofill apps/android/app/src/main/res/drawable/ic_autofill_copy.xml apps/android/app/src/test/kotlin/net/havenkeys/android/autofill apps/android/app/build.gradle.kts
git commit -m "feat(android): Search HavenKeys and the fill confirmation from the kit; the copy row draws the desktop's glyph"
```

---

### Task 15: The passkey save sheet

**Files:**
- Modify: `ANDROID/credentials/PasskeyCreateScreen.kt` (a `HavenSheet`)
- Modify: `apps/android/app/src/main/AndroidManifest.xml` (`PasskeyCreateActivity` translucent)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers `credentials/`)
- Test: `ANDROID_TEST/credentials/PasskeyCreateScreenTest.kt` (new), `ANDROID_TEST/ManifestTest.kt`

**Interfaces:**
- Consumes: Task 3's `HavenSheet` (capped and scrolling), `ChoiceRow`; kit `InsetGroup`, `SectionHeader`, `HavenButton`, `ProgressRing`; `ui.shell.ErrorLine`; `PasskeyCreateUiState` (unchanged).
- Produces: `PasskeyCreateScreen(state, onSelect, onSave, onCancel, onClose)` unchanged; `@Composable internal fun ColumnScope.PasskeyCreateContent(state, onSelect, onSave, onCancel, onClose)` (the sheet's content, for the screenshots).

`PasskeyCreateActivity` does not change: it still calls `PasskeyCreateScreen` inside `HavenTheme` after `unlockThen`, and the unlock it shows when locked still fills the window (Task 11's form draws the pane).

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/credentials/PasskeyCreateScreenTest.kt`:

```kotlin
package net.havenkeys.android.credentials

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.AutofillMatch

@RunWith(RobolectricTestRunner::class)
class PasskeyCreateScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val calls = mutableListOf<String>()

    private fun state(homes: Int = 1, excluded: Boolean = false) = PasskeyCreateUiState(
        loading = false,
        rpId = "github.com",
        userName = "sam",
        homes = (1..homes).map { AutofillMatch("h$it", "Login $it", "user$it", false) },
        selected = "h1",
        excluded = excluded,
    )

    private fun show(state: PasskeyCreateUiState, fontScale: Float = 1f) = rule.setKit(fontScale = fontScale) {
        PasskeyCreateScreen(
            state = state,
            onSelect = { calls += "select:$it" },
            onSave = { calls += "save" },
            onCancel = { calls += "cancel" },
            onClose = { calls += "close" },
        )
    }

    @Test
    fun theLoginsAreRadioButtonsWithTheChosenOneSelected() {
        show(state())
        rule.onNode(hasText("Login 1") and hasRole(Role.RadioButton)).assertIsSelected()
        rule.onNode(hasText(text(R.string.passkey_new_login)) and hasRole(Role.RadioButton)).performClick()
        assertEquals(listOf("select:null"), calls)
    }

    @Test
    fun saveAndCancelAnswer() {
        show(state())
        rule.onNode(hasText(text(R.string.passkey_save)) and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.passkey_cancel)) and hasClickAction()).performClick()
        assertEquals(listOf("save", "cancel"), calls)
    }

    @Test
    fun closingTheSheetIsACancel() {
        show(state())
        rule.onNodeWithContentDescription(text(R.string.kit_close)).performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        assertEquals(listOf("cancel"), calls)
    }

    @Test
    fun closingTheSheetOverAnExistingPasskeyIsAClose() {
        show(state(excluded = true))
        rule.onNodeWithContentDescription(text(R.string.kit_close)).performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        assertEquals(listOf("close"), calls)
    }

    @Test
    fun aLongListAtALargeFontStillReachesSave() {
        show(state(homes = 30), fontScale = 1.5f)
        rule.onNode(hasText(text(R.string.passkey_save)) and hasClickAction()).performScrollTo().assertIsDisplayed()
            .performClick()
        assertEquals(listOf("save"), calls)
    }
}
```

Add to `ANDROID_TEST/ManifestTest.kt`:

```kotlin
    @Test
    fun thePasskeySheetOpensOverTheCallingApp() {
        val activity = elements("activity").single { it.android("name") == ".credentials.PasskeyCreateActivity" }
        assertEquals("@style/Theme.HavenKeys.Translucent", activity.android("theme"))
    }
```

(import `org.junit.Assert.assertEquals` if the file does not have it).

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*PasskeyCreateScreenTest*' --tests '*ManifestTest*'`
Expected: FAIL (no sheet, no "Close" backdrop; the activity has no theme attribute).

- [ ] **Step 3: Rewrite `PasskeyCreateScreen.kt`**

```kotlin
package net.havenkeys.android.credentials

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.ChoiceRow
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * "Save a passkey to HavenKeys?" as a sheet over the site asking. Closing it
 * (drag, backdrop, Back) answers as its own button would: Close when the
 * passkey already exists, Cancel otherwise. Taller than the window allows,
 * it scrolls (many logins, a large font).
 */
@Composable
fun PasskeyCreateScreen(
    state: PasskeyCreateUiState,
    onSelect: (String?) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
    onClose: () -> Unit,
) {
    HavenSheet(onDismiss = if (state.excluded) onClose else onCancel, title = stringResource(R.string.passkey_save_title)) {
        PasskeyCreateContent(state, onSelect, onSave, onCancel, onClose)
    }
}

/** The sheet's content, without its window: the screenshots draw it inline. */
@Composable
internal fun ColumnScope.PasskeyCreateContent(
    state: PasskeyCreateUiState,
    onSelect: (String?) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
    onClose: () -> Unit,
) {
    when {
        state.loading -> ProgressRing(progress = null, Modifier.align(Alignment.CenterHorizontally).padding(vertical = 24.dp))
        state.excluded -> {
            HavenText(stringResource(R.string.passkey_exists), style = HavenTheme.type.value, color = HavenTheme.colors.textStrong)
            HavenButton(
                stringResource(R.string.passkey_close),
                onClick = onClose,
                Modifier.fillMaxWidth().padding(top = 16.dp),
                style = ButtonStyle.Secondary,
            )
        }
        state.planFailed -> {
            state.error?.let { ErrorLine(it) }
            HavenButton(
                stringResource(R.string.passkey_cancel),
                onClick = onCancel,
                Modifier.fillMaxWidth().padding(top = 16.dp),
                style = ButtonStyle.Secondary,
            )
        }
        else -> Choice(state, onSelect, onSave, onCancel)
    }
}

@Composable
private fun Choice(state: PasskeyCreateUiState, onSelect: (String?) -> Unit, onSave: () -> Unit, onCancel: () -> Unit) {
    val colors = HavenTheme.colors
    HavenText(state.rpId, style = HavenTheme.type.rowTitle, color = colors.textStrong)
    HavenText(
        stringResource(
            R.string.passkey_account,
            state.userName.ifBlank { stringResource(R.string.passkey_no_account_name) },
        ),
        color = colors.muted,
    )
    SectionHeader(stringResource(R.string.passkey_save_to), Modifier.padding(top = 12.dp))
    InsetGroup(Modifier.selectableGroup()) {
        state.homes.forEach { home ->
            row { ChoiceRow(home.title, state.selected == home.id, onClick = { onSelect(home.id) }, detail = home.username) }
        }
        row {
            ChoiceRow(
                stringResource(R.string.passkey_new_login),
                state.selected == null,
                onClick = { onSelect(null) },
                detail = stringResource(R.string.passkey_new_login_detail),
            )
        }
    }
    state.error?.let { ErrorLine(it) }
    Row(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        HavenButton(stringResource(R.string.passkey_cancel), onClick = onCancel, Modifier.weight(1f), style = ButtonStyle.Quiet)
        HavenButton(stringResource(R.string.passkey_save), onClick = onSave, Modifier.weight(1f), enabled = !state.busy)
    }
}
```

- [ ] **Step 4: The activity opens over the caller**

In `apps/android/app/src/main/AndroidManifest.xml`, add `android:theme="@style/Theme.HavenKeys.Translucent"` to the `.credentials.PasskeyCreateActivity` element, after `android:excludeFromRecents`.

- [ ] **Step 5: The Material check covers `credentials/`**

Add `"**/credentials/**/*.kt",` to `forbidMaterialInKit`'s `include(…)` list.

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug`
Expected: all pass, `PasskeyCreateViewModelTest`, `CredentialPlannerTest` unchanged.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/credentials apps/android/app/src/main/AndroidManifest.xml apps/android/app/src/test/kotlin/net/havenkeys/android/credentials apps/android/app/src/test/kotlin/net/havenkeys/android/ManifestTest.kt apps/android/app/build.gradle.kts
git commit -m "feat(android): saving a passkey is a kit sheet over the site; closing it cancels"
```

---

### Task 16: No Material outside the bridge

**Files:**
- Delete: `ANDROID/ui/components/HavenTopBar.kt`
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit` covers every source file but `MaterialBridge.kt`)
- Modify: `ANDROID/ui/theme/MaterialBridge.kt`, `ANDROID/ui/theme/Theme.kt` (comments only)
- Modify: `RES/values/strings.xml`, `RES/values-pt-rBR/strings.xml` (strings nothing uses)

**Interfaces:**
- Consumes: Tasks 1–15. Produces: the check stage 5 turns into the permanent rule.

- [ ] **Step 1: Delete the old bar and confirm what is left**

```bash
git rm apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components/HavenTopBar.kt
grep -rln "androidx.compose.material" apps/android/app/src
```

Expected output: only `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme/MaterialBridge.kt`. Anything else is a screen this plan missed: rebuild it from the kit in this task (with a test for any behaviour it has) before going on.

- [ ] **Step 2: The check covers everything**

In `apps/android/app/build.gradle.kts`, replace `forbidMaterialInKit`'s `description` and `include(…)` with:

```kotlin
    description = "Fails on a Material import anywhere but ui/theme/MaterialBridge.kt, which stage 5 deletes."
    val sources = fileTree("src") {
        include("**/*.kt")
        exclude("**/ui/theme/MaterialBridge.kt")
    }
```

and the exception's message with `"Material is not used in HavenKeys; build from ui/kit (spec 2026-10-03 §5):\n"`.

Show it bites: add `import androidx.compose.material3.Text` as the last import of `ANDROID/ui/item/DetailRows.kt`; `./gradlew forbidMaterialInKit` fails naming that file; remove the line; it passes.

- [ ] **Step 3: Say what the bridge is now for**

In `ANDROID/ui/theme/MaterialBridge.kt`, replace the file's first comment with:

```kotlin
/*
 * Material, kept only so the material3 dependency still has a theme while it
 * is on the classpath. No screen reads it any more (stage 4); stage 5
 * deletes this file, the call in HavenTheme, and the material3 and
 * material-icons-extended dependencies. Nothing may use what is here.
 */
```

and in `ANDROID/ui/theme/Theme.kt` replace `// Screens not yet rebuilt from ui/kit still read MaterialTheme.` with `// Nothing reads MaterialTheme since stage 4; stage 5 removes this wrapper with material3.`

- [ ] **Step 4: Remove strings nothing uses**

```bash
cd apps/android/app/src/main
for name in $(grep -o 'name="[a-z_0-9]*"' res/values/strings.xml | cut -d'"' -f2); do
  grep -rqn "R\.\(string\|plurals\)\.$name\b" kotlin ../debug 2>/dev/null || grep -rqn "@string/$name\b" res AndroidManifest.xml || echo "$name"
done
```

Delete each printed name from both `res/values/strings.xml` and `res/values-pt-rBR/strings.xml` (expected: some of `vault_*`, `item_*`, `autofill_search` stays because `DatasetFactory` uses it; follow the output, not this guess). Run the loop again: no output.

- [ ] **Step 5: Run everything**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add -A apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme apps/android/app/build.gradle.kts apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml
git commit -m "refactor(android): no Material outside the bridge stage 5 deletes; strings nothing uses removed"
```

---

### Task 17: Screenshots of the rebuilt screens and the `/impeccable` review

The controller runs this task (it needs `/impeccable`). Fixes that come out of it are separate commits, each with a test where the fix has behaviour.

**Files:**
- Create: `apps/android/app/src/testDebug/kotlin/net/havenkeys/android/screens/ScreenScreenshots.kt`
- Create: `apps/android/.impeccable/review/screens-*.png` (written by the test; review screenshots stay out of git as in stage 2, so commit them only if `ShellScreenshots`' PNGs were committed)
- Modify: rebuilt-screen files as the review requires; `apps/android/DESIGN.md` ("Review notes (stage 4)")

- [ ] **Step 1: Write the screenshot test**

It reuses `ShellScreenshots.kt`'s mechanism (read that file first and copy its `save(name)` and its rule set-up exactly as they are now). The screens, each built fresh from fakes:

```kotlin
/**
 * Renders the stage 4 screens with fake data for design review
 * (apps/android/.impeccable/review). Runs only when the build passes a
 * directory: `./gradlew testGithubDebugUnitTest --tests '*ScreenScreenshots*' -PscreensDir=.impeccable/review`.
 * Sheets and dialogs are drawn inline (SheetSurface, DialogSurface): their own windows do not reach the decor view.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class ScreenScreenshots {
    // rule, dir and save(name): as in ShellScreenshots.

    private val app = RuntimeEnvironment.getApplication()
    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam@example.com", "github.com", true, true, 0, 0)
    private val vault = FakeVaultRepository().apply {
        view = Outcome.Ok(
            ItemView(
                github,
                listOf(
                    ViewField("username", "username", FieldKind.TEXT, "sam@example.com"),
                    ViewField("password", "password", FieldKind.SECRET, null),
                    ViewField("website", "website", FieldKind.URL, "https://github.com"),
                ),
            ),
        )
        edit = Outcome.Ok(
            ItemEdit(
                ItemKind.LOGIN,
                "GitHub",
                listOf(Website("github.com", MatchKind.DOMAIN)),
                listOf(
                    EditField("username", FieldKind.TEXT, true, "sam@example.com"),
                    EditField("password", FieldKind.SECRET, true, null),
                    EditField("totp", FieldKind.TOTP, true, null),
                    EditField("notes", FieldKind.TEXT, false, null),
                ),
                false,
                true,
                3L,
            ),
        )
        generated = Outcome.Ok(Generated("vR7#kq2-Lm9!xT4w", 104.0))
    }
    private val accounts = FakeAccountRepository().apply {
        deviceList = Outcome.Ok(
            listOf(
                DeviceInfo("d1", "Pixel 8", "2026-01-01T00:00:00Z", null, true),
                DeviceInfo("d2", "Work laptop", "2026-01-01T00:00:00Z", "2026-10-03T10:00:00Z", false),
            ),
        )
    }
    private val clipboard = SensitiveClipboard(app, CoroutineScope(SupervisorJob()))
    private val events = VaultEventsHub()
    private val passkey = PasskeyCreateUiState(
        loading = false,
        rpId = "github.com",
        userName = "sam",
        homes = listOf(AutofillMatch("h1", "GitHub", "sam@example.com", true), AutofillMatch("h2", "GitHub work", "sam@work.example", false)),
        selected = "h1",
    )

    private class Shot(val name: String, val after: () -> Unit = {}, val content: @Composable () -> Unit)

    private val shots = listOf(
        Shot("item") {
            ItemScreen(remember { ItemViewModel(vault, FakeSettingsRepository(), events, "1") }, clipboard, true, ItemNavigation({}, {}, {}, {}))
        },
        Shot("editor") {
            EditScreen(
                remember { EditViewModel(vault, accounts, events, EditTarget.Existing("1")) },
                isNew = false,
                online = true,
                navigation = EditNavigation({}, {}, {}),
            )
        },
        Shot("generator") {
            GeneratorScreen(remember { GeneratorViewModel(vault, FakeSettingsRepository()) }, clipboard, false, {}, {})
        },
        Shot("unlock") { UnlockForm(UnlockUiState(needsSecretKey = true, offerBiometric = true), { _, _ -> }, {}) },
        Shot("onboarding") { OnboardingScreen(remember { OnboardingViewModel(accounts) }, onDone = {}) },
        Shot("onboarding-type") {
            OnboardingScreen(remember { OnboardingViewModel(accounts).also { it.choose(OnboardingUiState.Mode.TYPE) } }, onDone = {})
        },
        Shot("devices") { DevicesScreen(remember { DevicesViewModel(accounts, events) }, true, {}, {}) },
        Shot("autofill-setup") { AutofillSetupScreen(online = false, onBack = {}, onLock = {}) },
        Shot(
            "autofill-search",
            after = {
                rule.onNode(hasSetTextAction()).performTextInput("git")
                rule.mainClock.advanceTimeBy(500)
            },
        ) {
            val repo = remember {
                FakeAutofillRepository().apply { matchList = Outcome.Ok(listOf(AutofillMatch("m1", "GitHub", "sam@example.com", true))) }
            }
            AutofillSearchScreen(repo::search, CallerApp("com.example.app", "Example"), onConfirmed = { null })
        },
        Shot("wallet-confirm") {
            Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
                DialogSurface(
                    title = "Fill your identity in shop.example?",
                    confirm = DialogAction("Fill with documents", {}),
                    message = "The app calls itself Shop",
                    dismiss = DialogAction("Cancel", {}),
                    alternative = DialogAction("Fill without documents", {}),
                )
            }
        },
        Shot("passkey-sheet") {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                SheetSurface(stringResource(R.string.passkey_save_title)) { PasskeyCreateContent(passkey, {}, {}, {}, {}) }
            }
        },
    )

    @Test
    fun everyRebuiltScreenInBothThemes() {
        assumeTrue(dir != null)
        var dark by mutableStateOf(false)
        var index by mutableIntStateOf(0)
        rule.setContent {
            HavenTheme(darkTheme = dark) {
                Box(Modifier.fillMaxSize().background(HavenTheme.colors.pane)) { key(index) { shots[index].content() } }
            }
        }
        for (theme in listOf(false, true)) {
            shots.forEachIndexed { i, shot ->
                dark = theme
                index = i
                rule.mainClock.advanceTimeBy(2_000)
                shot.after()
                rule.mainClock.advanceTimeBy(500)
                save("screens-${if (theme) "dark" else "light"}-${shot.name}")
            }
        }
    }
}
```

(Imports follow from the names; long lines follow the Global Constraints' wrapping rule. `key(index)` gives each shot fresh `remember`ed ViewModels. `UnlockForm`, `AutofillSearchScreen`, `CallerApp`, `PasskeyCreateContent`, `SheetSurface`, `DialogSurface` are `internal`: the test is in the same module.)

- [ ] **Step 2: Render**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ScreenScreenshots*' -PscreensDir=.impeccable/review`
Expected: PASS, and `ls .impeccable/review/screens-*.png` lists 22 files. Without `-PscreensDir` the test is skipped.

- [ ] **Step 3: `/impeccable` review**

Invoke `/impeccable` (skill `impeccable:impeccable`) in critique mode with this brief:

> Review the HavenKeys Android screens rebuilt in stage 4 of the redesign. Artefacts: `apps/android/.impeccable/review/screens-*.png` (light and dark: item detail, editor, generator, unlock with Secret Key and biometrics, onboarding choose and type, devices, autofill setup offline, Search HavenKeys with a result, the card and identity confirmation with three answers, the passkey save sheet) and the sources under `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/{item,edit,generator,unlock,onboarding,settings,autofillsetup,components}`, `autofill/AutofillSearchScreen.kt`, `credentials/PasskeyCreateScreen.kt`. Reference: `apps/android/DESIGN.md` (the kit and the stage 3 shell), `apps/desktop/DESIGN.md`, the stage 3 screenshots `shell-*.png`, spec `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §6.9 and §7. Judge: the full-screen bar (Back, offline, Lock, actions) against the shell's top bar; the item header (tile and serif title) and its rows; the editor's density on a 360dp phone (two quiet buttons on hidden rows, the website group); the generator's output plate and length row; unlock's hierarchy; the three stacked answers; the passkey sheet; the One Fitting Rule (brass only on controls and thin lines; one primary per screen); that nothing reads as Material; that the rebuilt screens and the shell read as one app. Return an ordered list of material fixes.

Apply the fixes the owner would agree with without asking (spacing, sizes, ink, copy inside these screens). Anything that changes a decision recorded in this plan goes into `apps/android/DESIGN.md` under "Review notes (stage 4)" as an open question. Re-render after fixes. Commit each fix:

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android apps/android/app/src/test
git commit -m "fix(android): <what the review changed, in the screen's terms>"
```

- [ ] **Step 4: Commit the test**

```bash
git add apps/android/app/src/testDebug/kotlin/net/havenkeys/android/screens/ScreenScreenshots.kt
git commit -m "test(android): stage 4 screens rendered for design review"
```

---

### Task 18: Documentation, the owner's device checks, closing the stage

**Files:**
- Modify: `apps/android/DESIGN.md` (the kit additions; "Full-screen screens"; "Review notes (stage 4)" from Task 17)
- Modify: `docs/android.md` ("Redesign stage 4 (existing screens)" checklist)
- Modify: `docs/superpowers/plans/2026-10-03-android-redesign-index.md` (stage 4 row; stage 5 bullets)

- [ ] **Step 1: `apps/android/DESIGN.md`**

In "Components", add to the right groups, in the file's voice:
- Structure: "`GroupRow` without an action merges its text (a label and its value) into one TalkBack stop; its trailing controls stay their own." "`ItemRow` takes a `tileModifier`; the tapped row's tile and title travel into the item header."
- Inputs: "`HavenTextField` and `SecretTextField` take a `hint`: a muted line under the value, read with the field; an error takes its place." "`SegmentedControl` segments share the tallest one's height when a label wraps."
- Overlays: "`HavenSheet` stops at 90% of the window and scrolls inside when taller (a short sheet still drags down from anywhere); it rises above the keyboard." "`HavenDialog` takes an `alternative` (the three answers stack full width) and `dismissible = false` (only a button answers)." "`ChoiceSheet` and `ChoiceRow`: single choices as radio rows with a brass check; a pick closes the sheet, then saves."

Add a section "## Full-screen screens" after the Shell section: `ScreenBar` (Back, the "Offline, read-only" marker, Lock, the screen's actions; no title: the large serif title sits under it), `OfflineNote`, `MaskedValue` and `RevealedValue`, the item header (56dp tile, headline title), and the rule that typed secrets use `remember { TextFieldState() }`, never `rememberTextFieldState()`.

- [ ] **Step 2: `docs/android.md`**

Add after "### Redesign stage 3 (shell)":

```markdown
### Redesign stage 4 (existing screens)

Run on an emulator or phone (Android 14+), in light and dark, once with
`adb shell settings put global animator_duration_scale 1` and once with `0`
(restore `1` afterwards), and once in Portuguese (Brazil) at the largest
font size.

- [ ] Item: from Home, Items and search, the row's tile grows into the header with its title; with animations off it cuts. Show reveals the password in mono with coloured digits; it hides after 30 s, on leaving the screen, on Home (the app going to the background) and on lock. Copy turns the glyph to a check with a light haptic and the toast "Password copied. The clipboard clears in 30 s."; the item then appears under Frequently used. Offline, Edit and More are dimmed. More → Delete asks, warns about passkeys when the login has one, and returns to the list.
- [ ] Editor: a new login's Generate fills and shows the password; a saved login's password shows the mask until Change (which reads it); Remove then Undo; the website's "Matches" row opens a sheet of three rules; the one-time code's setup key is typed masked with the eye to show it. Gboard: no suggestions and nothing learned in the title, the secret fields and the setup key (Gboard's incognito marker shows on secret fields). Back with changes asks. Edit the same item on the desktop meanwhile, then Save on the phone: the conflict dialog ignores Back and an outside tap; Reload loads the new version.
- [ ] Generator: the length row and the slider agree; TalkBack reads the slider as "Length, 24"; the switches regenerate; Copy shows its toast.
- [ ] Unlock: a wrong password is read by TalkBack as the field's error; the field is empty after each attempt; rotating or killing the process (`adb shell am kill net.havenkeys.android`) never brings the password back; biometrics as before.
- [ ] Onboarding: scan (camera), type and invite; the hints under server, Secret Key and the new password; too short and mismatch read as the fields' errors.
- [ ] Devices: this phone is marked; Revoke asks in a dialog; revoking this phone signs it out.
- [ ] Autofill setup: the state row, Open settings, the Chrome help, the passkeys section; coming back from Android's settings updates it.
- [ ] Autofill: in an app with no saved login, "Search HavenKeys…" opens the search, a pick asks "Use … in <package>?" with the app's own name under it, and fills. A card and an identity with documents ask with three stacked answers. The dropdown's "Copy one-time code" row shows the outlined copy glyph.
- [ ] Passkeys: create a passkey on a site in Chrome: the sheet rises over Chrome; with many logins it stops short of the top and scrolls to Save; dragging it down, a backdrop tap and Back cancel (the site reports a cancellation); locked, unlock fills the screen first, then the sheet.
- [ ] FLAG_SECURE: screenshots blocked and the recents thumbnail blank on every screen above, the passkey sheet, the dialogs and the menu.
- [ ] TalkBack: each read-only detail row is one stop (label and value), with Show and Copy separate; a hidden value reads "Hidden Password", never dots or a length; the code row reads its digits and "12 seconds remaining"; dialog and sheet titles are announced; the passkey sheet's logins are radio buttons, one selected.
- [ ] pt-BR at the largest font: the stacked dialog answers, the editor's hidden rows and the generator's switches wrap without cutting; the catalogue's segmented control keeps one height.
```

- [ ] **Step 3: The plan index**

In `docs/superpowers/plans/2026-10-03-android-redesign-index.md`, set the stage 4 row's status to `Done (\`<first commit>..<last commit>\`)` with this stage's real range. Under "### Stage 5", add:

```markdown
- What is left: `ui/theme/MaterialBridge.kt` (the whole file: `MaterialBridge`,
  the colour schemes, `MaterialTypography`, `HavenType`, `MaterialShapes`), the
  `MaterialBridge(...)` call in `ui/theme/Theme.kt`, `compose-material3` and
  `compose-icons` in `gradle/libs.versions.toml` and `app/build.gradle.kts`,
  and the `MaterialBridge.kt` exclusion in `forbidMaterialInKit` (which then
  becomes the permanent rule). The platform theme's `android:Theme.Material`
  parent in `res/values*/themes.xml` is the window behind Compose, not
  Compose Material, and stays.
- Open questions from `apps/android/DESIGN.md` "Review notes (stage 4)".
```

- [ ] **Step 4: Final verification**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add apps/android/DESIGN.md docs/android.md docs/superpowers/plans/2026-10-03-android-redesign-index.md
git commit -m "docs: Android stage 4 screens in the design system, their device checks, stage 4 done"
```

- [ ] **Step 6: Hand the device checks to the owner**

Report the "Redesign stage 4 (existing screens)" checklist to the owner as the remaining work of this stage: nothing in it has run on a device.

---

## Stage exit check

- [ ] `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease` passes.
- [ ] `grep -rln "androidx.compose.material" apps/android/app/src` prints only `ui/theme/MaterialBridge.kt`; `forbidMaterialInKit` includes `**/*.kt` and excludes only that file.
- [ ] `grep -rn "rememberTextFieldState" apps/android/app/src/main/kotlin` shows only onboarding's server and email.
- [ ] `grep -rn "ifSettled\|launchSingleTop" apps/android/app/src` prints nothing; every user-started push goes through `pushOnce`.
- [ ] `HavenTopBar.kt`, `SecretField.kt` and `TotpRing.kt` no longer exist.
- [ ] Screen tests exist for item, editor (fields and screen), generator, unlock, onboarding, devices, autofill setup, Search HavenKeys, the fill confirmation and the passkey sheet; `HavenNavHostTest` pins the lock wipe and Back.
- [ ] The owner has the device checklist (Task 18, Step 6).
