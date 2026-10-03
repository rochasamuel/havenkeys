# Android Redesign, Stage 3: Shell and New Screens Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the old vault screen with the redesign's shell: a fixed top bar (search pill, lock, offline badge), a bottom bar with Home, Items and Settings, each tab with its own back stack, a full-screen search with recent searches, Home (identity card, Recently added, Frequently used), the add button and its sheet, Items with its category lists, and Settings as grouped rows, all built from `ui/kit` and moving as spec §7 describes.

**Architecture:** Two NavHosts. The app's NavHost (`ui/nav/HavenNavHost.kt`) holds onboarding, unlock, the shell, search and the full-screen screens over the shell (item, editors, generator, devices, autofill setup). The shell (`ui/shell/ShellScreen.kt`) is one destination of it: a `HavenScaffold` whose top bar and bottom bar sit outside a second NavHost (`ShellNavHost`) with one nested graph per tab, so tabs keep their own stacks with Navigation Compose's saved and restored state. New screens live in `ui/home`, `ui/items`, `ui/search`, with a ViewModel each over the existing repositories and fakes; Settings keeps its ViewModel and is redrawn from the kit. Every move takes its spec from `HavenMotion` through one file of transitions (`ui/nav/NavMotion.kt`).

**Tech Stack:** Kotlin 2.4, Jetpack Compose BOM 2026.06.01 (foundation, animation, ui 1.11.4), Navigation Compose 2.9.8, lifecycle-runtime-compose 2.10.0, Robolectric 4.17 with `ui-test-junit4`, detekt 1.23.

**Spec:** `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §6.1–6.8 (shell and screens), §7 (motion), §8 stage 3, §9 (security), §10 (testing). Security rules screens keep: `docs/superpowers/specs/2026-10-01-android-app-design.md` §9.3–9.4, `docs/android.md`, `CLAUDE.md`. Kit as shipped: `apps/android/DESIGN.md` and the sources in `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit/` and `ui/theme/`.

## Global Constraints

- Screens inside the shell (shell frame, bars, Home, Items, category lists, Settings rows, the add sheet) and search use `ui/kit` and `androidx.compose.foundation` only: no `import androidx.compose.material…`. Task 15 widens the `forbidMaterialInKit` Gradle check to enforce it.
- Screens inside the shell include the Settings tab and its dialogs (`ui/settings/Settings{Screen,Dialog,Choices,Actions}.kt`).
- Full-screen screens over the shell (item detail, editors, generator, devices, autofill setup) keep their Material implementation and their files unchanged in this stage; they are only re-hosted. Onboarding and unlock are unchanged. Stage 4 rebuilds them.
- Route arguments are never secret and never a query: an item UUID (`item/{id}`, `edit/{id}`), a kind (`new/{kind}`), a category (`items/{category}`). `search` takes no argument.
- The search query lives only in `SearchViewModel` memory and in the field's `remember`ed (never `rememberSaveable`) `TextFieldState`. Never in a route, `SavedStateHandle`, saved instance state or a log. `record_search` is called only when a result is opened.
- Activity data (Recently added, Frequently used, recent searches) and overviews live only in ViewModel `StateFlow`s and are dropped on `Locked`, `Removed` and `SignedOut`. Nothing from the vault goes to DataStore, SharedPreferences, Room or files.
- No revealed value in any ViewModel of this stage: Home's identity card is built from the identity's overview and the names of its filled fields, never from `reveal()`.
- Every animation takes its spec from `HavenMotion` (`springSpec`, `fadeSpec`, `sealSpec`); under "Remove animations" (`motion.reduced`) every transition is `EnterTransition.None`/`ExitTransition.None` or a snap. Nothing blocks input while animating.
- No ripple: clickables use the kit's `havenClickable` or pass `indication = HavenPress`; touch targets are at least 48 x 48 dp.
- `FLAG_SECURE`, autofill exclusion and touch filtering stay as they are: `MainActivity` sets them; the kit's sheet, dialog and menu windows set their own.
- Our own words wrap and are never cut with an ellipsis; only user data (titles, usernames, URLs, queries) may ellipsize.
- Every new string has an English and a Brazilian Portuguese version (`values/strings.xml`, `values-pt-rBR/strings.xml`), appended before `</resources>` after the `kit_*` block under the comment `<!-- Shell and new screens (spec 2026-10-03 §6) -->` (Task 3 opens the block in both files). `lintGithubDebug` fails on a missing translation.
- No logging: the `forbidLogging` check covers new files.
- detekt runs on all of it. `MaxLineLength` is 120: where a line of this plan's code is longer, wrap its arguments one per line and change nothing else. `MagicNumber` ignores `12.dp`-style receivers; any other bare number goes into a named `private const val`. `LongParameterList` counts parameters without defaults from 6.
- Unit tests: `cd apps/android && ./gradlew testGithubDebugUnitTest`. ViewModel tests are plain JUnit with `Dispatchers.setMain(UnconfinedTestDispatcher())` and the fakes in `ANDROID_TEST/fakes/FakeRepositories.kt`; screen tests run under Robolectric (SDK 34) with `createComposeRule()` and the kit's `setKit`, `hasRole`, `assertTouchTarget`, `removeAnimations` (`ANDROID_TEST/ui/kit/KitTestSupport.kt`). Build a screen test's ViewModel outside `setKit { }`, never inside it.
- No emulator in this stage's tasks: verification is JVM tests, detekt, lint and assemble. The device checks are a list for the owner in Task 17.
- Commits without Co-Authored-By lines (owner's rule). Each task leaves `testGithubDebugUnitTest detekt assembleGithubDebug` green.

## Entry checks

Run these before Task 1. Items 3–6 were checked on 2026-10-03 while writing this plan; recheck only if `gradle/libs.versions.toml` changed since.

1. **Stage 2 is done.** In `docs/superpowers/plans/2026-10-03-android-redesign-index.md` the stage 2 row reads `Done`; `apps/android/DESIGN.md` exists. Read its "Review notes (stage 2)": an open question that changes a component this plan uses (for example the TalkBack finding on `PullToRefresh`'s custom action) is carried into the task that uses the component.
2. **Kit signatures as shipped.** This plan calls, in package `net.havenkeys.android.ui.kit`: `HavenScaffold(modifier, topBar, bottomBar: (@Composable () -> Unit)?, floatingButton: (@Composable () -> Unit)?, toastState, content: @Composable (PaddingValues) -> Unit)`; `InsetGroup(modifier, content: InsetGroupScope.() -> Unit)` with `row { }`; `GroupRow(modifier, onClick, onClickLabel, icon, trailing, chevron, content)`; `GroupRowText(title, detail)`; `TrailingText(text)`; `ItemRow(title, subtitle, leading: RowLeading, onClick, modifier, titleModifier, hasPasskey, hasCode)`; `RowLeading.Monogram(title)`, `RowLeading.Glyph(icon, soft)`; `SectionHeader(text, modifier, action: SectionAction?)`; `HavenSheet(onDismiss, modifier, title, content: ColumnScope.() -> Unit)`; `HavenDialog(title, onDismiss, confirm: DialogAction, modifier, message, dismiss, content: (ColumnScope.() -> Unit)?, confirmEnabled, busy, answerKey)` (the stage 2 review's version); `HavenTextField(state, label, modifier, placeholder, error, enabled, keyboardOptions, …)`; `SecretTextField(state, label, revealed, onRevealChange, modifier, error, enabled, imeAction, onKeyboardAction)`; `ProgressRing(progress, modifier, size, warn, contentDescription)`; `rememberToastState()`, `ToastState.show(text, tone)`; `SheetSurface(title, modifier, content)` (internal); `ToggleRow(title, checked, onCheckedChange, modifier, detail, enabled)`; `PullToRefresh(refreshing, onRefresh, modifier, content)`; `Pill(text, modifier, tone)`; `HavenIconButton(icon, contentDescription, onClick, modifier, enabled, tint)`; `HavenButton(text, onClick, modifier, style: ButtonStyle, enabled, icon)`; `AddButton(onClick, modifier, contentDescription)`; `IconGlyph(icon, contentDescription, modifier, tint, size)`; `HavenText(text, modifier, style, color, maxLines, overflow)`; internal `Modifier.havenClickable(enabled, role, onClickLabel, onClick)`, `HavenPress`, `KitTextInput(content)`, `DISABLED_ALPHA`; `HavenIcon.{Home, Items, Gear, Search, Lock, Refresh, Plus, Key, Note, Card, IdCard, Grid, Globe, Dice, Clock, Check, X, ChevronLeft}`. Theme: `HavenTheme.colors.{pane, group, groupLine, field, muted, text, textStrong, brass, brassInk, danger, line}`, `HavenTheme.type.{headline, titleSmall, body, value, rowTitle, rowSubtitle, label}`, `HavenTheme.motion`, `HavenSprings.smooth`, `STAGGER_MILLIS`, `HavenSpacing.{gutter, rowX, groupGap, touch, rowMin}`, `HavenShape.{group, pill}`, `HavenRadius.group`. Check with `grep -n "^fun \|^internal fun \|^class \|^enum class " apps/android/app/src/main/kotlin/net/havenkeys/android/ui/kit/*.kt`. If a stage 2 review fix renamed or reshaped one, change this plan's call to match; do not change the kit for this plan. Kit text fields name themselves by merged label text, not `contentDescription` (stage 2 review): tests in this plan find fields with `hasSetTextAction()`, never by content description.
3. **Navigation Compose 2.9.8** (resolved, per `app/build/intermediates/incremental/lintAnalyzeGithubDebug/githubDebug-artifact-libraries.xml`): `androidx.navigation.compose.navigation(startDestination: String, route: String, builder)`, `NavGraph.Companion.findStartDestination`, `NavDestination.Companion.hierarchy`, `NavOptionsBuilder.popUpTo { saveState }`, `restoreState`, `currentBackStackEntryAsState()` all exist. Its `NavHost` drives predictive back itself (`PredictiveBackHandler`, seekable transitions) and keeps a popped screen above the one it reveals (`zIndices`); predictive back needs `android:enableOnBackInvokedCallback="true"` on the activity (Task 2).
4. **lifecycle-runtime-compose 2.10.0** has `LifecycleResumeEffect`. **Foundation 1.11.4** has `Modifier.selectable(selected, interactionSource: MutableInteractionSource?, indication: Indication?, …)`, `TextFieldState.setTextAndPlaceCursorAtEnd`, `TextFieldState.clearText`; **animation 1.11.4** has `SharedTransitionScope.ResizeMode.RemeasureToBounds` and `scaleToBounds()`.
5. **Screenshots without an emulator.** Stage 2 Task 15 renders the catalogue to PNGs under Robolectric native graphics (`src/testDebug/…/catalogue/CatalogueScreenshots.kt`, reading the system property `havenkeys.screens.dir`). Confirm that `app/build.gradle.kts` passes it from `-PscreensDir`: `grep -n "screens" apps/android/app/build.gradle.kts`. Task 16 reuses it for the new screens.
6. **What Rust gives the identity card.** `crates/havenkeys-mobile/src/items.rs`: an identity's `ItemSummary.subtitle` is `None`, and `item_view` for an identity lists the filled fields as keys `identity.<name>` with `value: None`; values come only from `reveal`. Hence the identity-card decision below.

## Decisions recorded for the owner

- **Two NavHosts.** The shell is one destination of the app's NavHost and hosts its own NavHost with one nested graph per tab. Full-screen screens are pushed on the app's NavHost, so they cover the bars by construction ("over the shell, bars hidden"), the lock's `popUpTo(graph)` wipe still removes everything at once, and the bars live outside the tab content so a tab change never moves or recomposes them. One NavHost would have needed every full-screen destination duplicated per tab graph.
- **Search is a route of the app's NavHost** (`search`, no argument), not an overlay. Its ViewModel lives with that entry, so Back from an opened result returns to the results, and the lock wipe or a process death removes it with the rest. The pill grows into the field through a shared bounds key across the two destinations.
- **The identity card shows which details exist, not the name.** Spec §6.5 asks for "name, or 'Add your details' when empty". The name is not in the overview; showing it would mean `reveal()` calls on every Home load and a revealed value kept in Home's ViewModel, which Android spec §9.4 forbids. The card shows the identity's title and the kinds of detail it holds ("Name · Email · Address"), from the field names `item_view` already lists without values, or "Add your details". If the owner wants the name, the clean route is a `display_name` on the Rust overview, not a reveal.
- **Settings "updates" row omitted.** Spec §6.8 lists updates among today's settings, but the Android app has no updater (`docs/android.md`: "Not yet: … an in-app updater"). The account row shows the signed-in email.
- **Settings is kit-only now, dialogs included.** The stage 2 review gave `HavenDialog` a `content` slot, caller-owned `confirmEnabled` and `busy`, and re-arming after a failed confirm; the biometric enrolment (master password in a `SecretTextField`), sign-out and remove-device (typed email, Rust's answer under the field) dialogs use it. The auto-lock and clipboard choices open a sheet of single-choice rows written in `SettingsChoices.kt` (not added to the kit): a `SegmentedControl` cannot hold five labels like "After 15 minutes" on a phone.
- **Sync now in the top bar.** TalkBack probably cannot reach `PullToRefresh`'s custom "Refresh" action (stage 2 review), so a visible way to sync is needed. The least intrusive one that serves every list is a "Sync now" icon button in the fixed top bar, between the offline badge and Lock (a spinning ring while syncing, a toast if it fails). Pull to refresh stays on Home, Items and the category lists. This adds one control to spec §6.2's top bar; the bar stays identical on every tab. The lists reload on Rust's `items_changed`.
- **Shared elements.** The shared title (existing) travels from Home, Items, category and search rows to the item screen. Because an item can be in Recently added and Frequently used at once, the shared key is applied only to the row that was tapped (`TitleTravel`, in memory, ids only). The monogram tile into the detail header waits for stage 4, when the header is a kit `ItemTile`.
- **Push "dims".** Compose transitions cannot draw a scrim over the outgoing screen; the old screen slides 30% left and fades to 60% over the window's ground (`pane`), which reads as dimmed in both themes.
- **Add sheet picks cut.** A tile closes the sheet at once and pushes its screen (the push is the motion); dragging down, the backdrop and Back still close it on its spring.
- **Back on category lists.** The top bar has no back button (spec §6.2: identical everywhere), so a category list shows a back chevron above its large title. Items and Settings roots get a large title; Home has none.
- **A–Z is a reader's A–Z.** Category lists sort with the locale's `Collator` at secondary strength, so "Água" sits with A and case does not split the alphabet (Rust's `list_items` sorts by lowercased bytes).
- **The add button** floats on the Home and Items tabs (category lists included), not on Settings.
- **Home refreshes its lists each time it is shown** (`LifecycleResumeEffect`): a copy in the detail records a use without any items event. Home's groups settle in sequence the first time a Home ViewModel shows, not on every tab return.
- **Predictive back** is turned on for `MainActivity` only (`android:enableOnBackInvokedCallback`), so the push can be scrubbed (spec §7).

## Review Focus

1. **An item that is in both Recently added and Frequently used.** Both rows render (lazy keys are prefixed by list, so no duplicate-key crash), and the title that travels is the tapped row's. Pinned in Task 7 (`anItemInBothListsOpensFromTheRowTapped`).
2. **A query typed, then the process dies or the screen's state is restored.** The query does not come back (the field is `remember`ed, the ViewModel is new), and a restored `search` entry while locked gives way to Unlock. Pinned in Task 11 (`aRestoredScreenDoesNotBringBackTheQuery`) and Task 14 (`aRestoredSearchGivesWayToUnlock`).
3. **A forged or stale category route** (`items/identity`, `items/../x`, a category removed in a later version and restored from saved state). Nothing opens; the Items root shows. Pinned in Task 3 (`anUnknownCategoryOpensNothing`, `onlyTheFiveCategoriesParse`).
4. **Titles that differ only by accent or case** ("Água", "amazon", "Banco"). A category list reads A–Z as a reader expects. Pinned in Task 8 (`aToZIgnoresCaseAndAccents`).
5. **A tap while something is still settling, and "Remove animations".** A settling row or tile takes the tap at once; with animations removed every navigation move is a cut. Pinned in Task 1 (`aSettlingRowTakesATapBeforeItHasSettled`) and Task 2 (`removeAnimationsCutsEveryMove`).

---

## File Structure

Paths abbreviate `apps/android/app/src/main/kotlin/net/havenkeys/android` as `ANDROID/` and `apps/android/app/src/test/kotlin/net/havenkeys/android` as `ANDROID_TEST/`. Gradle commands run in `apps/android`.

| File | Responsibility |
|---|---|
| `ANDROID/ui/theme/Motion.kt` | `fadeSpec(delayMillis)` for the recents that fade in after the pill |
| `ANDROID/ui/shell/SummaryRow.kt` (new) | `ItemSummary` → kit `ItemRow`; `OpenItem`, `SharedTitle`, `Origins` |
| `ANDROID/ui/shell/InsetList.kt` (new) | `LazyListScope.insetGroup`: the kit's inset group for lazy lists |
| `ANDROID/ui/shell/Settle.kt` (new) | Staggered settle-in for Home's groups and the sheet's tiles |
| `ANDROID/ui/shell/Lines.kt` (new) | `LargeTitle`, `ErrorLine`, `EmptyLine` |
| `ANDROID/ui/nav/NavMotion.kt` (new) | `Move`, `outerMove`, `enterFor`, `exitFor`: every screen transition |
| `ANDROID/ui/nav/SharedMotion.kt` (new) | `sharedIfMoving`, `TitleTravel`, `titleKey`, `SEARCH_KEY` |
| `ANDROID/ui/items/Category.kt` (new) | The five Items categories and their route argument |
| `ANDROID/ui/shell/Tab.kt` (new) | `Tab`, `ShellRoutes`, `tab()`, `selectTab`, `innerMove` |
| `ANDROID/ui/shell/ShellNavHost.kt` (new) | `ShellScreens`, the per-tab NavHost |
| `ANDROID/ui/shell/ShellTopBar.kt`, `BottomBar.kt` (new) | The fixed bars |
| `ANDROID/ui/shell/ShellViewModel.kt`, `AddSheet.kt` (new) | Online state, Sync now, lock, the add tiles and their sheet |
| `ANDROID/ui/shell/ShellScreen.kt` (new) | The shell: scaffold, bars, add button and sheet, tab host |
| `ANDROID/ui/home/IdentityPart.kt`, `HomeViewModel.kt`, `HomeScreen.kt` (new) | Home |
| `ANDROID/ui/items/ItemListViewModel.kt`, `ItemsScreen.kt`, `CategoryScreen.kt` (new) | Items tab and category lists |
| `ANDROID/ui/search/SearchViewModel.kt`, `SearchField.kt`, `SearchScreen.kt` (new) | Search |
| `ANDROID/ui/settings/SettingsScreen.kt`, `SettingsDialog.kt` (rewritten), `SettingsChoices.kt`, `SettingsActions.kt` (new), `SettingsNavigation.kt` | Settings as grouped rows, kit dialogs, choice sheet |
| `ANDROID/ui/nav/Routes.kt`, `HavenNavHost.kt` (rewritten), `ShellDestinations.kt` (new) | The app's navigation |
| `ANDROID/ui/vault/*`, `ANDROID/ui/components/ItemRow.kt`, `ANDROID_TEST/ui/vault/VaultViewModelTest.kt` | Deleted in Task 15 |
| `apps/android/app/src/main/AndroidManifest.xml` | Predictive back on `MainActivity` |
| `apps/android/app/build.gradle.kts` | `forbidMaterialInKit` widened to the new screens |
| `res/values/strings.xml`, `res/values-pt-rBR/strings.xml` | New strings; the old vault screen's removed |
| `ANDROID_TEST/fakes/FakeRepositories.kt` | Records the activity calls' sizes |
| `apps/android/app/src/testDebug/…/screens/ShellScreenshots.kt` (new) | PNGs of the new screens for `/impeccable` |
| `docs/android.md`, `docs/security-model.md`, plan index | Device checks, wording, stage status |

---
### Task 1: Shell building blocks: summary rows, lazy inset groups, settle, lines

**Files:**
- Modify: `ANDROID/ui/theme/Motion.kt` (`fadeSpec`)
- Create: `ANDROID/ui/shell/SummaryRow.kt`, `ANDROID/ui/shell/InsetList.kt`, `ANDROID/ui/shell/Settle.kt`, `ANDROID/ui/shell/Lines.kt`
- Test: `ANDROID_TEST/ui/shell/ShellPartsTest.kt`, `ANDROID_TEST/ui/theme/HavenMotionTest.kt`

**Interfaces:**
- Consumes: kit `ItemRow`, `RowLeading`, `HavenIcon`, `HavenText`; `errorText(code)` from `ui/components/ErrorText.kt`; `STAGGER_MILLIS`, `HavenSprings.smooth`.
- Produces (package `net.havenkeys.android.ui.shell`):
  - `typealias OpenItem = (id: String, origin: String) -> Unit`
  - `typealias SharedTitle = @Composable (id: String, origin: String) -> Modifier`, `val NoSharedTitle: SharedTitle`
  - `object Origins { IDENTITY, RECENT, FREQUENT, SEARCH, CATEGORY }` (string constants)
  - `internal fun ItemSummary.leading(): RowLeading`, `internal fun ItemSummary.secondLine(): String?`
  - `@Composable fun SummaryRow(summary: ItemSummary, origin: String, onOpen: OpenItem, modifier: Modifier = Modifier, sharedTitle: SharedTitle = NoSharedTitle)`
  - `fun <T> LazyListScope.insetGroup(items: List<T>, key: ((T) -> Any)?, around: @Composable (content: @Composable () -> Unit) -> Unit = { it() }, row: @Composable (T) -> Unit)`
  - `internal enum class SlicePosition { Single, First, Middle, Last }`, `internal fun slicePosition(index: Int, count: Int): SlicePosition`
  - `@Composable fun Settle(index: Int, modifier: Modifier = Modifier, active: Boolean = true, content: @Composable () -> Unit)`
  - `@Composable fun LargeTitle(text: String, modifier: Modifier = Modifier)`, `@Composable fun ErrorLine(code: String, modifier: Modifier = Modifier)`, `@Composable fun EmptyLine(text: String, modifier: Modifier = Modifier)`
  - `HavenMotion.fadeSpec(delayMillis: Int = 0)`

- [ ] **Step 1: Write the failing tests**

Add to `ANDROID_TEST/ui/theme/HavenMotionTest.kt` (import `androidx.compose.animation.core.TweenSpec`):

```kotlin
    @Test
    fun aDelayedFadeWaitsAndStillCutsUnderRemoveAnimations() {
        val delayed = havenMotion(1f).fadeSpec<Float>(delayMillis = 160) as TweenSpec<Float>
        assertEquals(160, delayed.delay)
        assertEquals(FADE_MILLIS, delayed.durationMillis)
        assertTrue(havenMotion(0f).fadeSpec<Float>(delayMillis = 160) is SnapSpec<*>)
    }
```

`ANDROID_TEST/ui/shell/ShellPartsTest.kt`:

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.RowLeading
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
class ShellPartsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun summary(
        kind: ItemKind,
        subtitle: String? = null,
        website: String? = null,
        totp: Boolean = false,
        passkey: Boolean = false,
    ) = ItemSummary("id-1", kind, "GitHub", subtitle, website, totp, passkey, 0, 0)

    @Test
    fun eachKindHasItsTile() {
        assertEquals(RowLeading.Monogram("GitHub"), summary(ItemKind.LOGIN).leading())
        assertEquals(RowLeading.Glyph(HavenIcon.Note, soft = true), summary(ItemKind.SECURE_NOTE).leading())
        assertEquals(RowLeading.Glyph(HavenIcon.Card), summary(ItemKind.CARD).leading())
        assertEquals(RowLeading.Glyph(HavenIcon.IdCard), summary(ItemKind.IDENTITY).leading())
    }

    @Test
    fun theSecondLineIsTheSubtitleElseTheWebsite() {
        assertEquals("sam", summary(ItemKind.LOGIN, subtitle = "sam", website = "github.com").secondLine())
        assertEquals("github.com", summary(ItemKind.LOGIN, website = "github.com").secondLine())
        assertNull(summary(ItemKind.SECURE_NOTE).secondLine())
    }

    @Test
    fun aSummaryRowOpensWithItsOrigin() {
        var opened: Pair<String, String>? = null
        rule.setKit {
            SummaryRow(
                summary(ItemKind.LOGIN, subtitle = "sam@example.com", totp = true, passkey = true),
                Origins.RECENT,
                onOpen = { id, origin -> opened = id to origin },
            )
        }
        rule.onNode(hasClickAction()).assert(hasText("GitHub")).assert(hasText("sam@example.com")).performClick()
        assertEquals("id-1" to Origins.RECENT, opened)
    }

    @Test
    fun slicesKnowTheirPlace() {
        assertEquals(SlicePosition.Single, slicePosition(0, 1))
        assertEquals(SlicePosition.First, slicePosition(0, 3))
        assertEquals(SlicePosition.Middle, slicePosition(1, 3))
        assertEquals(SlicePosition.Last, slicePosition(2, 3))
    }

    @Test
    fun aLazyGroupShowsEveryRowAsAButton() {
        rule.setKit {
            LazyColumn {
                insetGroup(listOf("One", "Two", "Three"), key = { it }) { GroupRow(onClick = {}) { GroupRowText(it) } }
            }
        }
        listOf("One", "Two", "Three").forEach { rule.onNode(hasText(it) and hasRole(Role.Button)).assertIsDisplayed() }
    }

    @Test
    fun aSettlingRowTakesATapBeforeItHasSettled() {
        var taps = 0
        rule.mainClock.autoAdvance = false
        rule.setKit { Settle(index = 3) { HavenButton("Login", onClick = { taps++ }) } }
        rule.mainClock.advanceTimeByFrame()
        rule.onNodeWithText("Login").performClick()
        assertEquals(1, taps)
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellPartsTest*' --tests '*HavenMotionTest*'`
Expected: compile errors (`fadeSpec` takes no `delayMillis`; `SummaryRow`, `insetGroup`, `Settle` unresolved).

- [ ] **Step 3: Add the delayed fade**

In `ANDROID/ui/theme/Motion.kt`, replace `fadeSpec` with:

```kotlin
    /** --t-fast on the house curve: fades and colour changes; [delayMillis] holds it back (recents after the pill). */
    fun <T> fadeSpec(delayMillis: Int = 0): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(FADE_MILLIS, delayMillis = delayMillis, easing = HouseEasing)
```

- [ ] **Step 4: Write `SummaryRow.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.RowLeading
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** Opens an item; [origin] names the list the row was in (an item can be in two lists at once). */
typealias OpenItem = (id: String, origin: String) -> Unit

/** The modifier that carries a row's title to the item screen; nothing by default. */
typealias SharedTitle = @Composable (id: String, origin: String) -> Modifier

val NoSharedTitle: SharedTitle = { _, _ -> Modifier }

/** The lists a row can be opened from. */
object Origins {
    const val IDENTITY = "identity"
    const val RECENT = "recent"
    const val FREQUENT = "frequent"
    const val SEARCH = "search"
    const val CATEGORY = "category"
}

/** A login shows its title's initial; the other kinds show what they are. */
internal fun ItemSummary.leading(): RowLeading = when (kind) {
    ItemKind.LOGIN -> RowLeading.Monogram(title)
    ItemKind.SECURE_NOTE -> RowLeading.Glyph(HavenIcon.Note, soft = true)
    ItemKind.CARD -> RowLeading.Glyph(HavenIcon.Card)
    ItemKind.IDENTITY -> RowLeading.Glyph(HavenIcon.IdCard)
}

/** A username or a card's ending from Rust, else the website's host. Never a secret. */
internal fun ItemSummary.secondLine(): String? = subtitle ?: website

/** One vault item as a kit row: overview fields only (Android spec §9.4). */
@Composable
fun SummaryRow(
    summary: ItemSummary,
    origin: String,
    onOpen: OpenItem,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    ItemRow(
        title = summary.title,
        subtitle = summary.secondLine(),
        leading = summary.leading(),
        onClick = { onOpen(summary.id, origin) },
        modifier = modifier,
        titleModifier = sharedTitle(summary.id, origin),
        hasPasskey = summary.hasPasskey,
        hasCode = summary.hasTotp,
    )
}
```

- [ ] **Step 5: Write `InsetList.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenRadius
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Where a row sits in its group, which decides its corners and borders. */
internal enum class SlicePosition { Single, First, Middle, Last }

internal fun slicePosition(index: Int, count: Int): SlicePosition = when {
    count == 1 -> SlicePosition.Single
    index == 0 -> SlicePosition.First
    index == count - 1 -> SlicePosition.Last
    else -> SlicePosition.Middle
}

/**
 * The kit's InsetGroup for a lazy list: each row is a slice of one rounded,
 * hairline-bordered group, and the hairline between rows starts where the
 * rows' text starts. [key] must be unique in the whole list (prefix it when
 * an item can be in two groups); null keys rows by position. [around] wraps
 * each slice, for instance in a [Settle].
 */
fun <T> LazyListScope.insetGroup(
    items: List<T>,
    key: ((T) -> Any)?,
    around: @Composable (content: @Composable () -> Unit) -> Unit = { it() },
    row: @Composable (T) -> Unit,
) {
    itemsIndexed(items, key = key?.let { k -> { _: Int, item: T -> k(item) } }) { index, item ->
        around { InsetSlice(slicePosition(index, items.size)) { row(item) } }
    }
}

@Composable
internal fun InsetSlice(position: SlicePosition, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val colors = HavenTheme.colors
    val radius = HavenRadius.group
    val shape = when (position) {
        SlicePosition.Single -> HavenShape.group
        SlicePosition.First -> RoundedCornerShape(topStart = radius, topEnd = radius)
        SlicePosition.Last -> RoundedCornerShape(bottomStart = radius, bottomEnd = radius)
        SlicePosition.Middle -> RectangleShape
    }
    val opensTop = position == SlicePosition.Middle || position == SlicePosition.Last
    val opensBottom = position == SlicePosition.First || position == SlicePosition.Middle
    Box(
        modifier
            .padding(horizontal = HavenSpacing.gutter)
            .fillMaxWidth()
            .clip(shape)
            .background(colors.group)
            .drawWithContent {
                drawContent()
                val line = 1.dp.toPx()
                val r = radius.toPx()
                // The group's outline, drawn past this slice's open edges so only its own part shows.
                val top = if (opensTop) -2 * r else line / 2
                val bottom = if (opensBottom) size.height + 2 * r else size.height - line / 2
                drawRoundRect(
                    color = colors.groupLine,
                    topLeft = Offset(line / 2, top),
                    size = Size(size.width - line, bottom - top),
                    cornerRadius = CornerRadius(r),
                    style = Stroke(line),
                )
                if (opensTop) {
                    drawLine(colors.groupLine, Offset(HavenSpacing.rowX.toPx(), line / 2), Offset(size.width, line / 2), line)
                }
            },
    ) { content() }
}
```

- [ ] **Step 6: Write `Settle.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.theme.STAGGER_MILLIS

private val SettleShift = 12.dp

/**
 * Content that settles into place (spec §7: Home's groups after unlock, the
 * add sheet's tiles): it fades in and rises a few dp on the smooth spring,
 * [index] × 30 ms after it first appears. Under "Remove animations", or when
 * not [active], it is simply there. It takes taps from the first frame:
 * nothing waits for the animation.
 */
@Composable
fun Settle(index: Int, modifier: Modifier = Modifier, active: Boolean = true, content: @Composable () -> Unit) {
    val motion = HavenTheme.motion
    val shift = with(LocalDensity.current) { SettleShift.toPx() }
    val shown = remember { Animatable(if (!active || motion.reduced) 1f else 0f) }
    LaunchedEffect(shown) {
        if (shown.value < 1f) {
            delay(index * STAGGER_MILLIS.toLong())
            shown.animateTo(1f, motion.springSpec(HavenSprings.smooth))
        }
    }
    Box(
        modifier.graphicsLayer {
            alpha = shown.value
            translationY = (1f - shown.value) * shift
        },
    ) { content() }
}
```

- [ ] **Step 7: Write `Lines.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** A screen's large title (Items, a category, Settings): serif, a heading for TalkBack. */
@Composable
fun LargeTitle(text: String, modifier: Modifier = Modifier) {
    HavenText(
        text,
        modifier.padding(top = 8.dp, bottom = 12.dp).semantics { heading() },
        style = HavenTheme.type.headline,
        color = HavenTheme.colors.textStrong,
    )
}

/** A failed load or sync in the app's words for its code; Rust's own detail is never shown. */
@Composable
fun ErrorLine(code: String, modifier: Modifier = Modifier) {
    HavenText(
        stringResource(errorText(code)),
        modifier.padding(vertical = 8.dp).semantics { liveRegion = LiveRegionMode.Polite },
        color = HavenTheme.colors.danger,
    )
}

/** What an empty list says, lined up with the rows' text. */
@Composable
fun EmptyLine(text: String, modifier: Modifier = Modifier) {
    HavenText(
        text,
        modifier.padding(horizontal = HavenSpacing.rowX, vertical = 8.dp),
        color = HavenTheme.colors.muted,
    )
}
```

- [ ] **Step 8: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellPartsTest*' --tests '*HavenMotionTest*'`
Expected: PASS.

- [ ] **Step 9: Run the suite and detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/theme/Motion.kt apps/android/app/src/main/kotlin/net/havenkeys/android/ui/shell apps/android/app/src/test/kotlin/net/havenkeys/android/ui/shell apps/android/app/src/test/kotlin/net/havenkeys/android/ui/theme/HavenMotionTest.kt
git commit -m "feat(android): shell rows, lazy inset groups and settle-in from the kit"
```

---

### Task 2: Navigation motion, shared elements and predictive back

**Files:**
- Modify: `ANDROID/ui/nav/Routes.kt` (add `SHELL`, `SEARCH`)
- Create: `ANDROID/ui/nav/NavMotion.kt`, `ANDROID/ui/nav/SharedMotion.kt`
- Modify: `apps/android/app/src/main/AndroidManifest.xml` (`MainActivity`)
- Test: `ANDROID_TEST/ui/nav/NavMotionTest.kt`, `ANDROID_TEST/ManifestTest.kt`

**Interfaces:**
- Consumes: `HavenMotion` (`reduced`, `springSpec`, `fadeSpec`, `sealSpec`), `HavenSprings.smooth`, `SharedTitle` (Task 1).
- Produces (package `net.havenkeys.android.ui.nav`):
  - `Routes.SHELL = "shell"`, `Routes.SEARCH = "search"`
  - `internal enum class Move { PUSH, TAB, SEAL, FADE, CUT }`
  - `internal val PUSHED: Set<String>`
  - `internal fun outerMove(from: String?, to: String?): Move`
  - `internal fun enterFor(move: Move, motion: HavenMotion, pop: Boolean, tabShiftPx: Int = 0): EnterTransition`
  - `internal fun exitFor(move: Move, motion: HavenMotion, pop: Boolean): ExitTransition`
  - `internal const val SEARCH_KEY = "search-pill"`, `internal fun titleKey(id: String): String`
  - `@Composable internal fun Modifier.sharedIfMoving(shared: SharedTransitionScope, key: String, visibility: AnimatedVisibilityScope, motion: HavenMotion, resize: SharedTransitionScope.ResizeMode = SharedTransitionScope.ResizeMode.scaleToBounds()): Modifier`
  - `@Stable internal class TitleTravel { fun tap(id: String, origin: String); fun from(shared, visibility, motion): SharedTitle }`

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/ui/nav/NavMotionTest.kt`:

```kotlin
package net.havenkeys.android.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import net.havenkeys.android.ui.theme.havenMotion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class NavMotionTest {
    private val full = havenMotion(1f)
    private val reduced = havenMotion(0f)

    @Test
    fun theLockAndTheRemovalCut() {
        assertEquals(Move.CUT, outerMove(Routes.ITEM, Routes.UNLOCK))
        assertEquals(Move.CUT, outerMove(Routes.SHELL, Routes.UNLOCK))
        assertEquals(Move.CUT, outerMove(Routes.SHELL, Routes.ONBOARDING))
        assertEquals(EnterTransition.None, enterFor(Move.CUT, full, pop = false))
        assertEquals(ExitTransition.None, exitFor(Move.CUT, full, pop = false))
    }

    @Test
    fun unlockingRevealsTheShellOnTheSeal() {
        assertEquals(Move.SEAL, outerMove(Routes.UNLOCK, Routes.SHELL))
        assertEquals(Move.SEAL, outerMove(Routes.ONBOARDING, Routes.SHELL))
    }

    @Test
    fun screensOverTheShellArePushedAndPopped() {
        for (route in PUSHED) {
            assertEquals(route, Move.PUSH, outerMove(Routes.SHELL, route))
            assertEquals(route, Move.PUSH, outerMove(route, Routes.SHELL))
        }
        assertEquals(Move.PUSH, outerMove(Routes.SEARCH, Routes.ITEM))
        assertEquals(Move.PUSH, outerMove(Routes.ITEM, Routes.EDIT))
    }

    @Test
    fun searchFadesUnderThePill() {
        assertEquals(Move.FADE, outerMove(Routes.SHELL, Routes.SEARCH))
        assertEquals(Move.FADE, outerMove(Routes.SEARCH, Routes.SHELL))
    }

    @Test
    fun removeAnimationsCutsEveryMove() {
        for (move in Move.entries) {
            for (pop in listOf(false, true)) {
                assertEquals("$move", EnterTransition.None, enterFor(move, reduced, pop))
                assertEquals("$move", ExitTransition.None, exitFor(move, reduced, pop))
            }
        }
    }

    @Test
    fun withAnimationsEveryMoveButTheCutMoves() {
        for (move in Move.entries - Move.CUT) {
            for (pop in listOf(false, true)) {
                assertNotEquals("$move", EnterTransition.None, enterFor(move, full, pop, tabShiftPx = 12))
                assertNotEquals("$move", ExitTransition.None, exitFor(move, full, pop))
            }
        }
    }
}
```

Add to `ANDROID_TEST/ManifestTest.kt`:

```kotlin
    @Test
    fun theMainActivityScrubsBackPredictively() {
        val main = elements("activity").single { it.android("name") == ".MainActivity" }
        assertEquals("true", main.android("enableOnBackInvokedCallback"))
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*NavMotionTest*' --tests '*ManifestTest*'`
Expected: compile errors (`Move`, `outerMove`, `Routes.SHELL` unresolved).

- [ ] **Step 3: Add the routes**

In `ANDROID/ui/nav/Routes.kt`, inside `object Routes`, after `const val VAULT = "vault"`:

```kotlin
    /** The shell: top bar, the tabs, bottom bar (spec §6.1). The tabs have routes of their own inside it. */
    const val SHELL = "shell"

    /** Search takes no argument: the query lives only in its ViewModel. */
    const val SEARCH = "search"
```

- [ ] **Step 4: Write `NavMotion.kt`**

```kotlin
package net.havenkeys.android.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenSprings

/** How one screen replaces another (spec §7). */
internal enum class Move {
    /** Over the shell, or a category list: in from the right; the old screen shifts left and dims. */
    PUSH,

    /** A tab change: a short crossfade with a few dp of rise; the bars stay. */
    TAB,

    /** Unlock or onboarding to the shell: the slower seal. */
    SEAL,

    /** Search: a plain fade under the pill's shared bounds. */
    FADE,

    /** The lock and the removal wipe: nothing animates (spec §7, Lock). */
    CUT,
}

/** The app's screens that cover the shell, full screen (spec §6.1). */
internal val PUSHED = setOf(
    Routes.ITEM,
    Routes.EDIT,
    Routes.NEW,
    Routes.GENERATOR,
    Routes.DEVICES,
    Routes.AUTOFILL_SETUP,
)

/** The old screen moves this share of its width left under a push (spec §7: about 30%)… */
private const val UNDER_SHIFT = 0.3f

/** …and fades to this over the window's ground, which reads as dimmed. */
private const val UNDER_ALPHA = 0.6f

/** The move between two of the app's routes; the same pair names a push and its pop. */
internal fun outerMove(from: String?, to: String?): Move = when {
    to == Routes.UNLOCK || to == Routes.ONBOARDING -> Move.CUT
    from == Routes.UNLOCK || from == Routes.ONBOARDING -> Move.SEAL
    to in PUSHED || from in PUSHED -> Move.PUSH
    else -> Move.FADE
}

/** The incoming screen's transition; [pop] when Back reveals it. Every spec comes from [motion]. */
internal fun enterFor(move: Move, motion: HavenMotion, pop: Boolean, tabShiftPx: Int = 0): EnterTransition {
    if (motion.reduced || move == Move.CUT) return EnterTransition.None
    return when (move) {
        Move.PUSH -> if (pop) {
            slideInHorizontally(motion.springSpec(HavenSprings.smooth)) { -(it * UNDER_SHIFT).toInt() } +
                fadeIn(motion.springSpec(HavenSprings.smooth), initialAlpha = UNDER_ALPHA)
        } else {
            slideInHorizontally(motion.springSpec(HavenSprings.smooth)) { it }
        }
        Move.TAB -> fadeIn(motion.fadeSpec()) + slideInVertically(motion.springSpec(HavenSprings.smooth)) { tabShiftPx }
        Move.SEAL -> fadeIn(motion.sealSpec())
        Move.FADE, Move.CUT -> fadeIn(motion.fadeSpec())
    }
}

/** The outgoing screen's transition; [pop] when Back removes it. */
internal fun exitFor(move: Move, motion: HavenMotion, pop: Boolean): ExitTransition {
    if (motion.reduced || move == Move.CUT) return ExitTransition.None
    return when (move) {
        Move.PUSH -> if (pop) {
            slideOutHorizontally(motion.springSpec(HavenSprings.smooth)) { it }
        } else {
            slideOutHorizontally(motion.springSpec(HavenSprings.smooth)) { -(it * UNDER_SHIFT).toInt() } +
                fadeOut(motion.springSpec(HavenSprings.smooth), targetAlpha = UNDER_ALPHA)
        }
        Move.TAB -> fadeOut(motion.fadeSpec())
        Move.SEAL -> fadeOut(motion.sealSpec())
        Move.FADE, Move.CUT -> fadeOut(motion.fadeSpec())
    }
}
```

- [ ] **Step 5: Write `SharedMotion.kt`**

```kotlin
package net.havenkeys.android.ui.nav

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.theme.HavenMotion

/** The search pill in the shell's top bar and the search screen's field are one element. */
internal const val SEARCH_KEY = "search-pill"

/** A row's title and the item screen's title. */
internal fun titleKey(id: String): String = "title-$id"

/** A shared element that moves with its screen; under "Remove animations" nothing is shared and screens cut. */
@Composable
internal fun Modifier.sharedIfMoving(
    shared: SharedTransitionScope,
    key: String,
    visibility: AnimatedVisibilityScope,
    motion: HavenMotion,
    resize: SharedTransitionScope.ResizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(),
): Modifier = if (motion.reduced) {
    this
} else {
    with(shared) {
        this@sharedIfMoving.sharedBounds(rememberSharedContentState(key = key), visibility, resizeMode = resize)
    }
}

/**
 * Which row's title travels to the item screen: the one tapped last. An
 * item can be in Recently added and Frequently used at once, and two
 * elements with one key would fight. Ids and list names only, in memory.
 */
@Stable
internal class TitleTravel {
    private var tapped: String? by mutableStateOf(null)

    fun tap(id: String, origin: String) {
        tapped = "$origin/$id"
    }

    fun from(shared: SharedTransitionScope, visibility: AnimatedVisibilityScope, motion: HavenMotion): SharedTitle =
        { id, origin ->
            if (tapped == "$origin/$id") Modifier.sharedIfMoving(shared, titleKey(id), visibility, motion) else Modifier
        }
}
```

- [ ] **Step 6: Turn on predictive back for `MainActivity`**

In `apps/android/app/src/main/AndroidManifest.xml`, on the `.MainActivity` element, after `android:windowSoftInputMode="adjustResize"`, add:

```xml
            android:enableOnBackInvokedCallback="true"
```

(The closing `>` stays after it.) Navigation's `NavHost` then scrubs the push and pop transitions with the back gesture on Android 14+; older versions ignore the attribute.

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*NavMotionTest*' --tests '*ManifestTest*'`
Expected: PASS.

- [ ] **Step 8: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass (lint may warn `UnusedAttribute` for the API 33+ attribute; a warning, not an error).

- [ ] **Step 9: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/nav apps/android/app/src/main/AndroidManifest.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/nav/NavMotionTest.kt apps/android/app/src/test/kotlin/net/havenkeys/android/ManifestTest.kt
git commit -m "feat(android): navigation moves from HavenMotion, shared title per tapped row, predictive back"
```

---

### Task 3: Tabs, categories and the shell's own NavHost

**Files:**
- Create: `ANDROID/ui/items/Category.kt`, `ANDROID/ui/shell/Tab.kt`, `ANDROID/ui/shell/ShellNavHost.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/items/CategoryTest.kt`, `ANDROID_TEST/ui/shell/ShellNavHostTest.kt`

**Interfaces:**
- Consumes: `Move`, `enterFor`, `exitFor` (Task 2); kit `HavenIcon`.
- Produces:
  - `net.havenkeys.android.ui.items.Category` — `enum class Category(val arg: String, @StringRes val label: Int, val icon: HavenIcon) { ALL, LOGINS, PASSKEYS, NOTES, CARDS }`, `fun keeps(item: ItemSummary): Boolean`, `companion fun fromArg(arg: String?): Category?`
  - `net.havenkeys.android.ui.shell.Tab` — `enum class Tab(val graph: String, val root: String, @StringRes val label: Int, val icon: HavenIcon) { HOME, ITEMS, SETTINGS }`
  - `object ShellRoutes { CATEGORY_ARG, CATEGORY = "items/{category}"; fun category(category: Category): String }`
  - `internal fun NavDestination.tab(): Tab?`, `internal fun NavHostController.selectTab(tab: Tab)`, `internal fun innerMove(from: NavDestination, to: NavDestination): Move`
  - `class ShellScreens(home: @Composable (PaddingValues) -> Unit, items: @Composable (PaddingValues, (Category) -> Unit) -> Unit, category: @Composable (PaddingValues, Category, () -> Unit) -> Unit, settings: @Composable (PaddingValues) -> Unit)`
  - `@Composable fun ShellNavHost(navController: NavHostController, screens: ShellScreens, padding: PaddingValues, modifier: Modifier = Modifier)`
  - Strings `tab_home`, `tab_items`.

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/ui/items/CategoryTest.kt`:

```kotlin
package net.havenkeys.android.ui.items

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

class CategoryTest {
    private fun item(kind: ItemKind, passkey: Boolean = false) =
        ItemSummary("id", kind, "Title", null, null, false, passkey, 0, 0)

    private val all = listOf(
        item(ItemKind.LOGIN),
        item(ItemKind.LOGIN, passkey = true),
        item(ItemKind.SECURE_NOTE),
        item(ItemKind.CARD),
        item(ItemKind.IDENTITY),
    )

    @Test
    fun eachCategoryKeepsItsKind() {
        assertEquals(5, all.count(Category.ALL::keeps))
        assertEquals(2, all.count(Category.LOGINS::keeps))
        assertEquals(1, all.count(Category.PASSKEYS::keeps))
        assertEquals(1, all.count(Category.NOTES::keeps))
        assertEquals(1, all.count(Category.CARDS::keeps))
    }

    @Test
    fun onlyTheFiveCategoriesParse() {
        Category.entries.forEach { assertEquals(it, Category.fromArg(it.arg)) }
        assertNull("the identity is on Home, not a list", Category.fromArg("identity"))
        assertNull(Category.fromArg("../vault"))
        assertNull(Category.fromArg(""))
        assertNull(Category.fromArg(null))
    }
}
```

`ANDROID_TEST/ui/shell/ShellNavHostTest.kt`:

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShellNavHostTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var nav: NavHostController

    private val screens = ShellScreens(
        home = { HavenText("Home root") },
        items = { _, onCategory -> HavenButton("Open logins", onClick = { onCategory(Category.LOGINS) }) },
        category = { _, category, onBack -> HavenButton("List ${category.arg}", onClick = onBack) },
        settings = { HavenText("Settings root") },
    )

    @Before
    fun show() {
        rule.setKit {
            nav = rememberNavController()
            ShellNavHost(nav, screens, PaddingValues())
        }
    }

    private fun select(tab: Tab) {
        rule.runOnIdle { nav.selectTab(tab) }
        rule.waitForIdle()
    }

    @Test
    fun itStartsOnHome() {
        rule.onNodeWithText("Home root").assertIsDisplayed()
    }

    @Test
    fun eachTabKeepsItsOwnStack() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        rule.onNodeWithText("List logins").assertIsDisplayed()
        select(Tab.HOME)
        rule.onNodeWithText("Home root").assertIsDisplayed()
        select(Tab.ITEMS)
        rule.onNodeWithText("List logins").assertIsDisplayed()
    }

    @Test
    fun reselectingATabPopsItToItsRoot() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").assertIsDisplayed()
        rule.onNodeWithText("List logins").assertDoesNotExist()
    }

    @Test
    fun backFromAnotherTabsRootGoesHome() {
        select(Tab.SETTINGS)
        rule.onNodeWithText("Settings root").assertIsDisplayed()
        rule.runOnIdle { nav.popBackStack() }
        rule.onNodeWithText("Home root").assertIsDisplayed()
    }

    @Test
    fun aCategoryListBacksOutToItems() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        rule.onNodeWithText("List logins").performClick()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun anUnknownCategoryOpensNothing() {
        select(Tab.ITEMS)
        rule.runOnIdle { nav.navigate("items/identity") }
        rule.waitForIdle()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun aCategoryListBelongsToItems() {
        select(Tab.ITEMS)
        rule.onNodeWithText("Open logins").performClick()
        rule.runOnIdle { assertEquals(Tab.ITEMS, nav.currentDestination?.tab()) }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*CategoryTest*' --tests '*ShellNavHostTest*'`
Expected: compile errors (`Category`, `ShellScreens`, `Tab` unresolved).

- [ ] **Step 3: Add the strings**

`res/values/strings.xml`, before `</resources>`:

```xml

    <!-- Shell and new screens (spec 2026-10-03 §6) -->
    <string name="tab_home">Home</string>
    <string name="tab_items">Items</string>
```

`res/values-pt-rBR/strings.xml`, before `</resources>`:

```xml

    <!-- Shell and new screens (spec 2026-10-03 §6) -->
    <string name="tab_home">Início</string>
    <string name="tab_items">Itens</string>
```

- [ ] **Step 4: Write `Category.kt`**

```kotlin
package net.havenkeys.android.ui.items

import androidx.annotation.StringRes
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** The lists Items offers (spec §6.7). A route carries [arg], never a query. */
enum class Category(val arg: String, @StringRes val label: Int, val icon: HavenIcon) {
    ALL("all", R.string.vault_filter_all, HavenIcon.Grid),
    LOGINS("logins", R.string.vault_filter_logins, HavenIcon.Globe),
    PASSKEYS("passkeys", R.string.vault_filter_passkeys, HavenIcon.Key),
    NOTES("notes", R.string.vault_filter_notes, HavenIcon.Note),
    CARDS("cards", R.string.vault_filter_cards, HavenIcon.Card),
    ;

    fun keeps(item: ItemSummary): Boolean = when (this) {
        ALL -> true
        LOGINS -> item.kind == ItemKind.LOGIN
        PASSKEYS -> item.kind == ItemKind.LOGIN && item.hasPasskey
        NOTES -> item.kind == ItemKind.SECURE_NOTE
        CARDS -> item.kind == ItemKind.CARD
    }

    companion object {
        /** Null for anything else, the identity included: a restored or forged route opens nothing. */
        fun fromArg(arg: String?): Category? = entries.firstOrNull { it.arg == arg }
    }
}
```

- [ ] **Step 5: Write `Tab.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.annotation.StringRes
import androidx.navigation.NavDestination
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import net.havenkeys.android.R
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.nav.Move

/** The bottom bar's tabs (spec §6.4); each is a nested graph with its own back stack. */
enum class Tab(val graph: String, val root: String, @StringRes val label: Int, val icon: HavenIcon) {
    HOME("tab-home", "home", R.string.tab_home, HavenIcon.Home),
    ITEMS("tab-items", "items", R.string.tab_items, HavenIcon.Items),
    SETTINGS("tab-settings", "settings", R.string.settings_title, HavenIcon.Gear),
}

/** Routes inside the shell. A category list carries only its category's name. */
object ShellRoutes {
    const val CATEGORY_ARG = "category"
    const val CATEGORY = "items/{$CATEGORY_ARG}"

    fun category(category: Category) = "items/${category.arg}"
}

/** The tab a destination belongs to. */
internal fun NavDestination.tab(): Tab? =
    hierarchy.firstNotNullOfOrNull { destination -> Tab.entries.firstOrNull { it.graph == destination.route } }

/**
 * A bottom bar tap. Another tab comes back as it was left (its stack saved
 * and restored); the current tab again goes back to its root (spec §6.1).
 */
internal fun NavHostController.selectTab(tab: Tab) {
    if (currentBackStackEntry?.destination?.tab() == tab) {
        popBackStack(tab.root, inclusive = false)
        return
    }
    navigate(tab.graph) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}

/** Inside the shell: another tab crossfades, a deeper screen of the same tab is pushed. */
internal fun innerMove(from: NavDestination, to: NavDestination): Move =
    if (from.tab() != to.tab()) Move.TAB else Move.PUSH
```

- [ ] **Step 6: Write `ShellNavHost.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.navigation
import androidx.navigation.navArgument
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.nav.enterFor
import net.havenkeys.android.ui.nav.exitFor
import net.havenkeys.android.ui.theme.HavenTheme

/** How far a tab's content rises as it fades in (spec §7: a few dp). */
private val TabShift = 6.dp

/**
 * The shell's tab screens. The app's NavHost builds them with their
 * ViewModels; tests pass plain text. [items] gets the way to open a
 * category, [category] the way back.
 */
class ShellScreens(
    val home: @Composable (PaddingValues) -> Unit,
    val items: @Composable (PaddingValues, onCategory: (Category) -> Unit) -> Unit,
    val category: @Composable (PaddingValues, Category, onBack: () -> Unit) -> Unit,
    val settings: @Composable (PaddingValues) -> Unit,
)

/**
 * The shell's content: one nested graph per tab, so each tab keeps its own
 * back stack (spec §6.1). [padding] is the room the scaffold leaves for the
 * floating button.
 */
@Composable
fun ShellNavHost(navController: NavHostController, screens: ShellScreens, padding: PaddingValues, modifier: Modifier = Modifier) {
    val motion = HavenTheme.motion
    val shift = with(LocalDensity.current) { TabShift.roundToPx() }
    NavHost(
        navController = navController,
        startDestination = Tab.HOME.graph,
        modifier = modifier,
        enterTransition = {
            enterFor(innerMove(initialState.destination, targetState.destination), motion, pop = false, tabShiftPx = shift)
        },
        exitTransition = { exitFor(innerMove(initialState.destination, targetState.destination), motion, pop = false) },
        popEnterTransition = {
            enterFor(innerMove(initialState.destination, targetState.destination), motion, pop = true, tabShiftPx = shift)
        },
        popExitTransition = { exitFor(innerMove(initialState.destination, targetState.destination), motion, pop = true) },
    ) {
        navigation(startDestination = Tab.HOME.root, route = Tab.HOME.graph) {
            composable(Tab.HOME.root) { screens.home(padding) }
        }
        navigation(startDestination = Tab.ITEMS.root, route = Tab.ITEMS.graph) {
            composable(Tab.ITEMS.root) {
                screens.items(padding) { category -> navController.navigate(ShellRoutes.category(category)) }
            }
            composable(
                ShellRoutes.CATEGORY,
                arguments = listOf(navArgument(ShellRoutes.CATEGORY_ARG) { type = NavType.StringType }),
            ) { entry ->
                val category = Category.fromArg(entry.arguments?.getString(ShellRoutes.CATEGORY_ARG))
                if (category == null) {
                    LaunchedEffect(Unit) { navController.popBackStack() }
                } else {
                    screens.category(padding, category) { navController.popBackStack() }
                }
            }
        }
        navigation(startDestination = Tab.SETTINGS.root, route = Tab.SETTINGS.graph) {
            composable(Tab.SETTINGS.root) { screens.settings(padding) }
        }
    }
}
```

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*CategoryTest*' --tests '*ShellNavHostTest*'`
Expected: PASS.

- [ ] **Step 8: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/items apps/android/app/src/main/kotlin/net/havenkeys/android/ui/shell apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/items apps/android/app/src/test/kotlin/net/havenkeys/android/ui/shell
git commit -m "feat(android): shell tabs with their own back stacks; reselect pops to the root"
```

---
### Task 4: The fixed top bar (search, sync, lock) and the bottom bar

**Files:**
- Create: `ANDROID/ui/shell/ShellTopBar.kt`, `ANDROID/ui/shell/BottomBar.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/shell/ShellBarsTest.kt`

**Interfaces:**
- Consumes: `Tab` (Task 3); kit `havenClickable`, `HavenPress`, `IconGlyph`, `HavenText`, `Pill`, `HavenIconButton`, `ProgressRing`; `R.string.vault_lock_now` ("Lock now").
- Produces:
  - `class TopBarActions(val onSearch: () -> Unit, val onSync: () -> Unit, val onLock: () -> Unit)`
  - `@Composable fun ShellTopBar(online: Boolean, syncing: Boolean, actions: TopBarActions, modifier: Modifier = Modifier, pillModifier: Modifier = Modifier)`
  - `@Composable fun BottomBar(selected: Tab, onSelect: (Tab) -> Unit, modifier: Modifier = Modifier)`
  - Strings `shell_search`, `shell_offline`, `shell_sync`, `shell_syncing`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/shell/ShellBarsTest.kt`:

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShellBarsTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun theBottomBarHasThreeTabsAndMarksTheSelectedOne() {
        val picked = mutableListOf<Tab>()
        rule.setKit { BottomBar(Tab.ITEMS, onSelect = { picked += it }) }
        rule.onNode(hasText("Items") and hasRole(Role.Tab)).assertIsSelected().assertTouchTarget()
        rule.onNode(hasText("Home") and hasRole(Role.Tab)).assertIsNotSelected().assertTouchTarget()
        rule.onNode(hasText("Settings") and hasRole(Role.Tab)).assertIsNotSelected().performClick()
        // A reselect is reported too: the shell pops that tab to its root.
        rule.onNode(hasText("Items") and hasRole(Role.Tab)).performClick()
        assertEquals(listOf(Tab.SETTINGS, Tab.ITEMS), picked)
    }

    private val done = mutableListOf<String>()
    private val actions = TopBarActions(
        onSearch = { done += "search" },
        onSync = { done += "sync" },
        onLock = { done += "lock" },
    )

    @Test
    fun theTopBarSearchesSyncsAndLocks() {
        rule.setKit { ShellTopBar(online = true, syncing = false, actions = actions) }
        rule.onNode(hasText("Search HavenKeys") and hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.onNodeWithContentDescription("Sync now").assertTouchTarget().performClick()
        rule.onNodeWithContentDescription("Lock now").assertTouchTarget().performClick()
        assertEquals(listOf("search", "sync", "lock"), done)
        rule.onNodeWithText("Offline").assertDoesNotExist()
    }

    @Test
    fun whileSyncingTheButtonSaysSoAndDoesNotSyncTwice() {
        rule.setKit { ShellTopBar(online = true, syncing = true, actions = actions) }
        rule.onNodeWithContentDescription("Sync now").assertDoesNotExist()
        rule.onNodeWithContentDescription("Syncing").assertIsDisplayed()
        assertEquals(emptyList<String>(), done)
    }

    @Test
    fun offlineTheTopBarSaysSo() {
        rule.setKit { ShellTopBar(online = false, syncing = false, actions = actions) }
        rule.onNodeWithText("Offline").assertIsDisplayed()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellBarsTest*'`
Expected: compile errors (`BottomBar`, `ShellTopBar` unresolved).

- [ ] **Step 3: Add the strings**

English, appended to the shell block:

```xml
    <string name="shell_search">Search HavenKeys</string>
    <string name="shell_offline">Offline</string>
    <string name="shell_sync">Sync now</string>
    <string name="shell_syncing">Syncing</string>
```

Portuguese:

```xml
    <string name="shell_search">Buscar no HavenKeys</string>
    <string name="shell_offline">Offline</string>
    <string name="shell_sync">Sincronizar agora</string>
    <string name="shell_syncing">Sincronizando</string>
```

- [ ] **Step 4: Write `ShellTopBar.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.kit.havenClickable
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** What the top bar's controls do; built once by the shell, so the bar skips recomposition. */
class TopBarActions(val onSearch: () -> Unit, val onSync: () -> Unit, val onLock: () -> Unit)

/**
 * The top bar of every tab and category list (spec §6.2): the search pill,
 * the offline badge when offline, Sync now (a ring while syncing) and Lock.
 * Sync now is the visible, TalkBack-reachable twin of pull to refresh. The
 * bar sits outside the tabs' NavHost and takes only stable values, so a tab
 * change neither moves nor recomposes it. [pillModifier] carries the pill's
 * shared bounds into search.
 */
@Composable
fun ShellTopBar(
    online: Boolean,
    syncing: Boolean,
    actions: TopBarActions,
    modifier: Modifier = Modifier,
    pillModifier: Modifier = Modifier,
) {
    val colors = HavenTheme.colors
    Row(
        modifier.fillMaxWidth().padding(start = HavenSpacing.gutter, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier
                .weight(1f)
                .then(pillModifier)
                .heightIn(min = HavenSpacing.touch)
                .clip(HavenShape.pill)
                .havenClickable(onClick = actions.onSearch)
                .background(colors.field)
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconGlyph(HavenIcon.Search, contentDescription = null, tint = colors.muted, size = 18.dp)
            Spacer(Modifier.width(10.dp))
            HavenText(stringResource(R.string.shell_search), style = HavenTheme.type.value, color = colors.muted)
        }
        if (!online) Pill(stringResource(R.string.shell_offline), Modifier.padding(start = 8.dp))
        if (syncing) {
            Box(Modifier.size(HavenSpacing.touch), contentAlignment = Alignment.Center) {
                ProgressRing(progress = null, size = 20.dp, contentDescription = stringResource(R.string.shell_syncing))
            }
        } else {
            HavenIconButton(HavenIcon.Refresh, stringResource(R.string.shell_sync), onClick = actions.onSync)
        }
        HavenIconButton(HavenIcon.Lock, stringResource(R.string.vault_lock_now), onClick = actions.onLock)
    }
}
```

- [ ] **Step 5: Write `BottomBar.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.HavenPress
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

private val BarHeight = 60.dp

/**
 * Home, Items, Settings (spec §6.4): each a tab for TalkBack, the selected
 * one in strong ink with a brass marker under it, a light tick on a change.
 * A tap on the current tab is reported too (the shell pops it to its root).
 */
@Composable
fun BottomBar(selected: Tab, onSelect: (Tab) -> Unit, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val haptics = LocalHapticFeedback.current
    Column(modifier.fillMaxWidth().background(colors.pane)) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(colors.line))
        Row(Modifier.fillMaxWidth().selectableGroup()) {
            Tab.entries.forEach { tab ->
                val isSelected = tab == selected
                val ink = if (isSelected) colors.textStrong else colors.muted
                val marker by animateColorAsState(
                    if (isSelected) colors.brass else Color.Transparent,
                    HavenTheme.motion.fadeSpec(),
                    label = "tab-marker",
                )
                Column(
                    Modifier
                        .weight(1f)
                        .heightIn(min = BarHeight)
                        .selectable(
                            selected = isSelected,
                            interactionSource = null,
                            indication = HavenPress,
                            role = Role.Tab,
                        ) {
                            if (!isSelected) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                            onSelect(tab)
                        }
                        .padding(top = 8.dp, bottom = 6.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    IconGlyph(tab.icon, contentDescription = null, tint = ink, size = 24.dp)
                    HavenText(stringResource(tab.label), style = HavenTheme.type.label, color = ink)
                    Box(Modifier.size(width = 18.dp, height = 2.dp).clip(HavenShape.pill).background(marker))
                }
            }
        }
    }
}
```

- [ ] **Step 6: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellBarsTest*'`
Expected: PASS.

- [ ] **Step 7: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/shell apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/shell/ShellBarsTest.kt
git commit -m "feat(android): shell top bar with search pill, sync, lock and offline badge; bottom bar tabs"
```

---

### Task 5: Shell ViewModel (online, sync, lock) and the add sheet

**Files:**
- Create: `ANDROID/ui/shell/ShellViewModel.kt`, `ANDROID/ui/shell/AddSheet.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/shell/ShellViewModelTest.kt`, `ANDROID_TEST/ui/shell/AddSheetTest.kt`

**Interfaces:**
- Consumes: `VaultRepository.lock()`, `AccountRepository.syncNow()`, `VaultEventsHub.online`; `Settle` (Task 1); kit `HavenSheet`, `havenClickable`, `DISABLED_ALPHA`; strings `kit_new_item`, `vault_new_login`, `vault_new_note`, `vault_new_card`.
- Produces:
  - `enum class AddTile(@StringRes val label: Int, val icon: HavenIcon, val kind: ItemKind?) { LOGIN, NOTE, CARD, GENERATOR }`
  - `data class AddTileState(val tile: AddTile, val enabled: Boolean)`, `fun tilesFor(online: Boolean): List<AddTileState>`
  - `data class ShellUiState(val online: Boolean = false, val syncing: Boolean = false, val syncError: String? = null) { val addTiles: List<AddTileState> }`
  - `class ShellViewModel(vault: VaultRepository, accounts: AccountRepository, events: VaultEventsHub) : ViewModel` — `val state: StateFlow<ShellUiState>`, `fun sync()`, `fun syncErrorShown()`, `fun lock()`
  - `@Composable fun AddSheet(tiles: List<AddTileState>, onPick: (AddTile) -> Unit, onDismiss: () -> Unit)`, `@Composable internal fun AddTiles(tiles: List<AddTileState>, onPick: (AddTile) -> Unit)`
  - Strings `add_generate`, `add_offline`.

- [ ] **Step 1: Write the failing tests**

`ANDROID_TEST/ui/shell/ShellViewModelTest.kt`:

```kotlin
package net.havenkeys.android.ui.shell

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind

@OptIn(ExperimentalCoroutinesApi::class)
class ShellViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val vault = FakeVaultRepository()
    private val accounts = FakeAccountRepository()
    private val events = VaultEventsHub()

    private fun vm() = ShellViewModel(vault, accounts, events)

    private fun ShellViewModel.tiles() = state.value.addTiles.associate { it.tile to it.enabled }

    @Test
    fun offlineTheItemTilesAreDimmedButNotTheGenerator() {
        val vm = vm()
        assertEquals(
            mapOf(AddTile.LOGIN to false, AddTile.NOTE to false, AddTile.CARD to false, AddTile.GENERATOR to true),
            vm.tiles(),
        )
    }

    @Test
    fun onlineEveryTileIsOnAndOfflineAgainDimsThem() {
        val vm = vm()
        events.connectivity(true)
        assertTrue(vm.state.value.online)
        assertTrue(vm.tiles().values.all { it })
        events.connectivity(false)
        assertEquals(listOf(AddTile.GENERATOR), vm.tiles().filterValues { it }.keys.toList())
    }

    @Test
    fun theIdentityIsNeverATile() {
        assertTrue(AddTile.entries.none { it.kind == ItemKind.IDENTITY })
        assertEquals(listOf(ItemKind.LOGIN, ItemKind.SECURE_NOTE, ItemKind.CARD), AddTile.entries.mapNotNull { it.kind })
    }

    @Test
    fun lockLocks() {
        vm().lock()
        assertEquals(listOf("lock"), vault.calls)
    }

    @Test
    fun syncNowSyncsOnce() {
        val vm = vm()
        vm.sync()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertFalse(vm.state.value.syncing)
        assertNull(vm.state.value.syncError)
    }

    @Test
    fun aFailedSyncIsReportedOnce() {
        accounts.sync = Outcome.Failed("offline")
        val vm = vm()
        vm.sync()
        assertEquals("offline", vm.state.value.syncError)
        vm.syncErrorShown()
        assertNull(vm.state.value.syncError)
    }

    @Test
    fun aLockDropsAPendingSyncError() {
        accounts.sync = Outcome.Failed("offline")
        val vm = vm()
        vm.sync()
        events.locked("user")
        assertNull(vm.state.value.syncError)
    }
}
```

`ANDROID_TEST/ui/shell/AddSheetTest.kt`:

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class AddSheetTest {
    @get:Rule
    val rule = createComposeRule()

    private var picked: AddTile? = null

    private fun show(online: Boolean) = rule.setKit {
        AddSheet(tilesFor(online), onPick = { picked = it }, onDismiss = {})
    }

    private fun tile(label: String) = rule.onNode(hasText(label) and hasRole(Role.Button))

    @Test
    fun offlineTheItemTilesAreDimmedAndSayWhy() {
        show(online = false)
        rule.onNodeWithText("New item").assert(isHeading())
        listOf("Login", "Secure note", "Card").forEach { tile(it).assertIsNotEnabled() }
        rule.onNodeWithText("Adding needs a connection").assertIsDisplayed()
        tile("Generate password").assertIsEnabled().assertTouchTarget().performClick()
        assertEquals(AddTile.GENERATOR, picked)
    }

    @Test
    fun onlineEveryTileOpensItsForm() {
        show(online = true)
        rule.onNodeWithText("Adding needs a connection").assertDoesNotExist()
        tile("Card").assertTouchTarget().performClick()
        assertEquals(AddTile.CARD, picked)
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellViewModelTest*' --tests '*AddSheetTest*'`
Expected: compile errors (`ShellViewModel`, `AddTile`, `AddSheet` unresolved).

- [ ] **Step 3: Add the strings**

English:

```xml
    <string name="add_generate">Generate password</string>
    <string name="add_offline">Adding needs a connection</string>
```

Portuguese:

```xml
    <string name="add_generate">Gerar senha</string>
    <string name="add_offline">Adicionar precisa de conexão</string>
```

- [ ] **Step 4: Write `ShellViewModel.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.annotation.StringRes
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import net.havenkeys.android.ui.kit.HavenIcon
import uniffi.havenkeys_mobile.ItemKind

/** The add sheet's tiles (spec §6.6). The identity is never one: there is one per account. */
enum class AddTile(@StringRes val label: Int, val icon: HavenIcon, val kind: ItemKind?) {
    LOGIN(R.string.vault_new_login, HavenIcon.Globe, ItemKind.LOGIN),
    NOTE(R.string.vault_new_note, HavenIcon.Note, ItemKind.SECURE_NOTE),
    CARD(R.string.vault_new_card, HavenIcon.Card, ItemKind.CARD),
    GENERATOR(R.string.add_generate, HavenIcon.Dice, null),
}

data class AddTileState(val tile: AddTile, val enabled: Boolean)

/** Creating needs the server; the generator works offline. */
fun tilesFor(online: Boolean): List<AddTileState> =
    AddTile.entries.map { AddTileState(it, enabled = online || it.kind == null) }

data class ShellUiState(
    val online: Boolean = false,
    /** Sync now is running: the top bar shows a ring instead of the button. */
    val syncing: Boolean = false,
    /** A failed sync's code, shown once as a toast; nothing from the vault. */
    val syncError: String? = null,
) {
    val addTiles: List<AddTileState> get() = tilesFor(online)
}

/** The shell's own state: whether writes can reach the server, Sync now, and the lock. Nothing from the vault. */
class ShellViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(ShellUiState(online = events.online.value))
    val state: StateFlow<ShellUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch { events.online.collect { online -> _state.update { it.copy(online = online) } } }
        viewModelScope.launch {
            events.events.collect { event ->
                if (event is VaultEvent.Locked || event == VaultEvent.Removed || event == VaultEvent.SignedOut) {
                    _state.update { it.copy(syncing = false, syncError = null) }
                }
            }
        }
    }

    /**
     * The top bar's Sync now: the visible twin of pull to refresh, which
     * TalkBack cannot reach. The screens reload on Rust's `items_changed`.
     */
    fun sync() {
        if (_state.value.syncing) return
        _state.update { it.copy(syncing = true, syncError = null) }
        viewModelScope.launch {
            val result = accounts.syncNow()
            _state.update { it.copy(syncing = false, syncError = (result as? Outcome.Failed)?.code) }
        }
    }

    fun syncErrorShown() {
        _state.update { it.copy(syncError = null) }
    }

    fun lock() = vault.lock()
}
```

- [ ] **Step 5: Write `AddSheet.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.DISABLED_ALPHA
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.havenClickable
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

private val TileHeight = 96.dp

/**
 * The add button's sheet (spec §6.6): Login, Secure note, Card, Generate
 * password, as tiles that settle in one after another. Offline the item
 * tiles are dimmed and say why; the generator stays. A pick closes the
 * sheet at once and goes straight to its screen; dragging down, the
 * backdrop and Back close it on its spring.
 */
@Composable
fun AddSheet(tiles: List<AddTileState>, onPick: (AddTile) -> Unit, onDismiss: () -> Unit) {
    HavenSheet(onDismiss = onDismiss, title = stringResource(R.string.kit_new_item)) {
        AddTiles(tiles, onPick)
    }
}

/** The tiles without the sheet's window; the screenshots draw them inline. */
@Composable
internal fun AddTiles(tiles: List<AddTileState>, onPick: (AddTile) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        tiles.chunked(2).forEachIndexed { rowIndex, pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                pair.forEachIndexed { columnIndex, state ->
                    Settle(index = rowIndex * 2 + columnIndex, modifier = Modifier.weight(1f)) {
                        AddTileButton(state) { onPick(state.tile) }
                    }
                }
            }
        }
        if (tiles.any { !it.enabled }) {
            HavenText(stringResource(R.string.add_offline), color = HavenTheme.colors.muted)
        }
    }
}

@Composable
private fun AddTileButton(state: AddTileState, onClick: () -> Unit) {
    val colors = HavenTheme.colors
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = TileHeight)
            .alpha(if (state.enabled) 1f else DISABLED_ALPHA)
            .clip(HavenShape.group)
            .havenClickable(enabled = state.enabled, onClick = onClick)
            .background(colors.group)
            .border(1.dp, colors.groupLine, HavenShape.group)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        IconGlyph(state.tile.icon, contentDescription = null, tint = colors.brassInk, size = 24.dp)
        HavenText(stringResource(state.tile.label), style = HavenTheme.type.rowTitle, color = colors.textStrong)
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellViewModelTest*' --tests '*AddSheetTest*'`
Expected: PASS.

- [ ] **Step 7: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/shell apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/shell
git commit -m "feat(android): shell view model with sync now; add sheet tiles dimmed offline but for the generator"
```

---

### Task 6: Home ViewModel

**Files:**
- Create: `ANDROID/ui/home/IdentityPart.kt`, `ANDROID/ui/home/HomeViewModel.kt`
- Modify: `ANDROID_TEST/fakes/FakeRepositories.kt` (`frequentlyUsed`, `recentlyCreated` record their size)
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/home/HomeViewModelTest.kt`

**Interfaces:**
- Consumes: `VaultRepository.list/view/recentlyCreated/frequentlyUsed`, `AccountRepository.syncNow`, `VaultEventsHub.events`.
- Produces (package `net.havenkeys.android.ui.home`):
  - `enum class IdentityPart(@StringRes val label: Int, internal val fields: Set<String>) { NAME, EMAIL, PHONE, ADDRESS, WORK, DOCUMENTS, OTHER }`, `companion fun of(keys: List<String>): List<IdentityPart>`
  - `data class IdentityCard(val id: String, val title: String, val parts: List<IdentityPart>)`
  - `data class HomeUiState(identity: IdentityCard?, recent: List<ItemSummary>, frequent: List<ItemSummary>, refreshing: Boolean, errorCode: String?, loading: Boolean)`
  - `class HomeViewModel(vault: VaultRepository, accounts: AccountRepository, events: VaultEventsHub)` — `state: StateFlow<HomeUiState>`, `fun shown()`, `fun refresh()`, `var settled: Boolean`, `companion const val LIST_SIZE = 6`
  - Strings `identity_part_name`, `identity_part_email`, `identity_part_phone`, `identity_part_address`, `identity_part_work`, `identity_part_documents`, `identity_part_other`.

- [ ] **Step 1: Make the fake record the lists' sizes**

In `ANDROID_TEST/fakes/FakeRepositories.kt`, inside `FakeVaultRepository`, replace

```kotlin
    override suspend fun frequentlyUsed(n: Int) = frequent

    override suspend fun recentlyCreated(n: Int) = recent
```

with

```kotlin
    override suspend fun frequentlyUsed(n: Int): Outcome<List<ItemSummary>> {
        calls += "frequent:$n"
        return frequent
    }

    override suspend fun recentlyCreated(n: Int): Outcome<List<ItemSummary>> {
        calls += "recent:$n"
        return recent
    }
```

- [ ] **Step 2: Write the failing test**

`ANDROID_TEST/ui/home/HomeViewModelTest.kt`:

```kotlin
package net.havenkeys.android.ui.home

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

@OptIn(ExperimentalCoroutinesApi::class)
class HomeViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val login = item("1", ItemKind.LOGIN, "GitHub")
    private val note = item("2", ItemKind.SECURE_NOTE, "Wi-Fi")
    private val identity = item("9", ItemKind.IDENTITY, "Sam")

    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(login, note, identity))
        recent = Outcome.Ok(listOf(note, login))
        frequent = Outcome.Ok(listOf(login))
        view = Outcome.Ok(identityView("identity.first_name", "identity.email", "identity.cpf"))
    }

    private fun vm() = HomeViewModel(vault, accounts, events)

    @Test
    fun nothingLoadsBeforeHomeShows() {
        val vm = vm()
        assertTrue(vm.state.value.loading)
        assertTrue(vault.calls.none { it.startsWith("recent") || it.startsWith("frequent") })
    }

    @Test
    fun shownLoadsTheIdentityAndBothListsOfSix() {
        val vm = vm()
        vm.shown()
        val state = vm.state.value
        assertEquals(listOf(note, login), state.recent)
        assertEquals(listOf(login), state.frequent)
        assertEquals(IdentityCard("9", "Sam", listOf(IdentityPart.NAME, IdentityPart.EMAIL, IdentityPart.DOCUMENTS)), state.identity)
        assertFalse(state.loading)
        assertTrue("recent:6" in vault.calls)
        assertTrue("frequent:6" in vault.calls)
    }

    @Test
    fun anIdentityWithNothingFilledHasNoParts() {
        vault.view = Outcome.Ok(identityView())
        val vm = vm()
        vm.shown()
        assertEquals(emptyList<IdentityPart>(), vm.state.value.identity?.parts)
    }

    @Test
    fun withoutAnIdentityThereIsNoCard() {
        vault.items = Outcome.Ok(listOf(login, note))
        val vm = vm()
        vm.shown()
        assertNull(vm.state.value.identity)
    }

    @Test
    fun theIdentityCardNeverHoldsAValue() {
        vault.view = Outcome.Ok(identityView("identity.first_name", value = "Samuel"))
        val vm = vm()
        vm.shown()
        assertFalse(vm.state.value.toString().contains("Samuel"))
        assertTrue("nothing is revealed for Home", vault.calls.none { it.startsWith("reveal") })
    }

    @Test
    fun showingAgainPicksUpANewUse() {
        val vm = vm()
        vm.shown()
        vault.frequent = Outcome.Ok(listOf(note, login))
        vm.shown()
        assertEquals(listOf(note, login), vm.state.value.frequent)
    }

    @Test
    fun aLockDropsTheActivityData() {
        val vm = vm()
        vm.shown()
        events.locked("user")
        assertEquals(HomeUiState(), vm.state.value)
    }

    @Test
    fun signingOutOrRemovingDropsItToo() {
        val vm = vm()
        vm.shown()
        events.signedOut()
        assertEquals(HomeUiState(), vm.state.value)
        vm.shown()
        events.removed()
        assertEquals(HomeUiState(), vm.state.value)
    }

    @Test
    fun anItemsChangedEventReloads() {
        val vm = vm()
        vm.shown()
        vault.recent = Outcome.Ok(listOf(login))
        events.itemsChanged()
        assertEquals(listOf(login), vm.state.value.recent)
    }

    @Test
    fun aFailedLoadShowsItsCodeAndKeepsWhatLoaded() {
        vault.frequent = Outcome.Failed("locked")
        val vm = vm()
        vm.shown()
        assertEquals("locked", vm.state.value.errorCode)
        assertEquals(listOf(note, login), vm.state.value.recent)
        assertTrue(vm.state.value.frequent.isEmpty())
    }

    @Test
    fun refreshSyncsThenReloads() = runTest {
        val vm = vm()
        vm.shown()
        vault.recent = Outcome.Ok(listOf(login))
        vm.refresh()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertEquals(listOf(login), vm.state.value.recent)
        assertFalse(vm.state.value.refreshing)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aFailedRefreshShowsItsCodeAndKeepsTheLists() = runTest {
        val vm = vm()
        vm.shown()
        accounts.sync = Outcome.Failed("offline")
        vm.refresh()
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(listOf(note, login), vm.state.value.recent)
        assertFalse(vm.state.value.refreshing)
    }

    @Test
    fun partsFollowTheFieldNames() {
        val keys = listOf(
            "identity.last_name",
            "identity.city",
            "identity.gender",
            "identity.home_phone",
            "identity.company",
            "card.number",
        )
        assertEquals(
            listOf(IdentityPart.NAME, IdentityPart.PHONE, IdentityPart.ADDRESS, IdentityPart.WORK, IdentityPart.OTHER),
            IdentityPart.of(keys),
        )
        assertEquals(emptyList<IdentityPart>(), IdentityPart.of(emptyList()))
    }

    private fun identityView(vararg keys: String, value: String? = null) =
        ItemView(identity, keys.map { ViewField(it, it, FieldKind.TEXT, value) })

    private fun item(id: String, kind: ItemKind, title: String) =
        ItemSummary(id, kind, title, null, null, false, false, 0, 0)
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HomeViewModelTest*'`
Expected: compile errors (`HomeViewModel`, `IdentityPart` unresolved).

- [ ] **Step 4: Add the strings**

English:

```xml
    <string name="identity_part_name">Name</string>
    <string name="identity_part_email">Email</string>
    <string name="identity_part_phone">Phone</string>
    <string name="identity_part_address">Address</string>
    <string name="identity_part_work">Work</string>
    <string name="identity_part_documents">Documents</string>
    <string name="identity_part_other">Other details</string>
```

Portuguese:

```xml
    <string name="identity_part_name">Nome</string>
    <string name="identity_part_email">E-mail</string>
    <string name="identity_part_phone">Telefone</string>
    <string name="identity_part_address">Endereço</string>
    <string name="identity_part_work">Trabalho</string>
    <string name="identity_part_documents">Documentos</string>
    <string name="identity_part_other">Outros dados</string>
```

- [ ] **Step 5: Write `IdentityPart.kt`**

```kotlin
package net.havenkeys.android.ui.home

import androidx.annotation.StringRes
import net.havenkeys.android.R

/**
 * The kinds of detail an identity holds, for Home's identity card. Built
 * from the names of the filled fields that `item_view` lists without their
 * values (havenkeys-mobile items.rs), so Home never reveals anything.
 */
enum class IdentityPart(@StringRes val label: Int, internal val fields: Set<String>) {
    NAME(R.string.identity_part_name, setOf("first_name", "middle_name", "last_name")),
    EMAIL(R.string.identity_part_email, setOf("email")),
    PHONE(R.string.identity_part_phone, setOf("mobile_phone", "home_phone", "work_phone")),
    ADDRESS(
        R.string.identity_part_address,
        setOf("street", "number", "complement", "neighborhood", "city", "state", "postal_code", "country"),
    ),
    WORK(R.string.identity_part_work, setOf("occupation", "company", "job_title")),
    DOCUMENTS(R.string.identity_part_documents, setOf("cpf", "rg", "passport", "drivers_license")),

    /** Any identity field no other part lists (gender, birth date, username, website, notes). */
    OTHER(R.string.identity_part_other, emptySet()),
    ;

    companion object {
        private const val PREFIX = "identity."

        /** The parts that a view's field keys (`identity.<name>`) fill, in this order. */
        fun of(keys: List<String>): List<IdentityPart> {
            val names = keys.filter { it.startsWith(PREFIX) }.map { it.removePrefix(PREFIX) }.toSet()
            val listed = entries.flatMap { it.fields }.toSet()
            return entries.filter { part ->
                if (part == OTHER) names.any { it !in listed } else names.any { it in part.fields }
            }
        }
    }
}
```

- [ ] **Step 6: Write `HomeViewModel.kt`**

```kotlin
package net.havenkeys.android.ui.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** The identity as Home shows it: its title and which kinds of detail it holds, never a value. */
data class IdentityCard(val id: String, val title: String, val parts: List<IdentityPart>)

/** Overviews and activity data only (spec §9); all of it is dropped on lock. */
data class HomeUiState(
    val identity: IdentityCard? = null,
    val recent: List<ItemSummary> = emptyList(),
    val frequent: List<ItemSummary> = emptyList(),
    val refreshing: Boolean = false,
    val errorCode: String? = null,
    /** True until the first answer from Rust, so empty lists are not announced early. */
    val loading: Boolean = true,
)

class HomeViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(HomeUiState())
    val state: StateFlow<HomeUiState> = _state.asStateFlow()
    private var pending: Job? = null

    /** Home's groups settle in sequence the first time it shows, not on every return to the tab. */
    var settled = false

    init {
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> load()
                    else -> Unit
                }
            }
        }
    }

    /** Each time Home shows: a copy or a fill since then changed "Frequently used" without an items event. */
    fun shown() = load()

    /** Pull to refresh: a sync, then the lists again. */
    fun refresh() {
        viewModelScope.launch {
            _state.update { it.copy(refreshing = true, errorCode = null) }
            val sync = accounts.syncNow()
            _state.update { it.copy(refreshing = false, errorCode = (sync as? Outcome.Failed)?.code) }
            if (sync is Outcome.Ok) load()
        }
    }

    private fun load() {
        pending?.cancel()
        pending = viewModelScope.launch {
            val recent = vault.recentlyCreated(LIST_SIZE)
            val frequent = vault.frequentlyUsed(LIST_SIZE)
            val identity = identityCard()
            val failed = (recent as? Outcome.Failed) ?: (frequent as? Outcome.Failed)
            _state.update {
                it.copy(
                    identity = identity,
                    recent = (recent as? Outcome.Ok)?.value.orEmpty(),
                    frequent = (frequent as? Outcome.Ok)?.value.orEmpty(),
                    errorCode = failed?.code,
                    loading = false,
                )
            }
        }
    }

    /** The account's identity from its overview and the names of its filled fields; nothing is revealed. */
    private suspend fun identityCard(): IdentityCard? {
        val all = (vault.list() as? Outcome.Ok)?.value ?: return null
        val identity = all.firstOrNull { it.kind == ItemKind.IDENTITY } ?: return null
        val keys = (vault.view(identity.id) as? Outcome.Ok)?.value?.fields?.map { it.key }.orEmpty()
        return IdentityCard(identity.id, identity.title, IdentityPart.of(keys))
    }

    /** The lock wipe: no overview or activity data stays in this ViewModel. */
    private fun wipe() {
        pending?.cancel()
        _state.value = HomeUiState()
    }

    companion object {
        /** Spec §6.5: the 6 most recently added and the 6 most used. */
        const val LIST_SIZE = 6
    }
}
```

- [ ] **Step 7: Run the tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HomeViewModelTest*'`
Expected: PASS.

- [ ] **Step 8: Run the suite and detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass (the fake's new `calls` entries do not appear in any existing assertion: `VaultViewModel` and `ItemViewModel` never call the activity lists).

- [ ] **Step 9: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/home apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/home apps/android/app/src/test/kotlin/net/havenkeys/android/fakes/FakeRepositories.kt
git commit -m "feat(android): Home view model: identity card from field names, recent and frequent, wiped on lock"
```

---

### Task 7: Home screen

**Files:**
- Create: `ANDROID/ui/home/HomeScreen.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/home/HomeScreenTest.kt`

**Interfaces:**
- Consumes: `HomeViewModel`, `HomeUiState`, `IdentityCard` (Task 6); `SummaryRow`, `insetGroup`, `Settle`, `ErrorLine`, `EmptyLine`, `OpenItem`, `SharedTitle`, `Origins` (Task 1); kit `PullToRefresh`, `SectionHeader`, `InsetGroup`, `GroupRow`, `GroupRowText`; `R.string.vault_empty_hint` ("New items you add appear here.").
- Produces: `@Composable fun HomeScreen(viewModel: HomeViewModel, onOpen: OpenItem, contentPadding: PaddingValues, modifier: Modifier = Modifier, sharedTitle: SharedTitle = NoSharedTitle)`; strings `home_recent`, `home_frequent`, `home_frequent_empty`, `home_identity_empty`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/home/HomeScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.home

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.shell.Origins
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class HomeScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam@example.com", null, false, false, 0, 0)
    private val wifi = ItemSummary("2", ItemKind.SECURE_NOTE, "Wi-Fi", null, null, false, false, 0, 0)
    private val identity = ItemSummary("9", ItemKind.IDENTITY, "Sam", null, null, false, false, 0, 0)

    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github, wifi, identity))
        recent = Outcome.Ok(listOf(github, wifi))
        frequent = Outcome.Ok(listOf(github))
        view = Outcome.Ok(ItemView(identity, listOf(field("identity.first_name"), field("identity.email"))))
    }
    private val opened = mutableListOf<Pair<String, String>>()

    private fun show() {
        val vm = HomeViewModel(vault, FakeAccountRepository(), VaultEventsHub())
        rule.setKit { HomeScreen(vm, onOpen = { id, origin -> opened += id to origin }, contentPadding = PaddingValues()) }
    }

    @Test
    fun theIdentitySitsOnTopAndSaysWhatItHolds() {
        show()
        rule.onNode(hasText("Sam") and hasClickAction()).assert(hasText("Name · Email")).performClick()
        assertEquals(listOf("9" to Origins.IDENTITY), opened)
    }

    @Test
    fun anEmptyIdentityAsksForDetails() {
        vault.view = Outcome.Ok(ItemView(identity, emptyList()))
        show()
        rule.onNode(hasText("Sam") and hasClickAction()).assert(hasText("Add your details"))
    }

    @Test
    fun bothListsShowUnderTheirTitles() {
        show()
        rule.onNodeWithText("Recently added").assert(isHeading())
        rule.onNodeWithText("Frequently used").assert(isHeading())
        rule.onNode(hasText("Wi-Fi") and hasClickAction()).assertIsDisplayed()
    }

    @Test
    fun anItemInBothListsOpensFromTheRowTapped() {
        show()
        val rows = rule.onAllNodes(hasText("GitHub") and hasClickAction())
        rows.assertCountEquals(2)
        rows[1].performClick()
        assertEquals(listOf("1" to Origins.FREQUENT), opened)
        rows[0].performClick()
        assertEquals("1" to Origins.RECENT, opened.last())
    }

    @Test
    fun noUsesYetSaysHowTheyCome() {
        vault.frequent = Outcome.Ok(emptyList())
        show()
        rule.onNodeWithText("Items you fill or copy will show here.").assertIsDisplayed()
    }

    private fun field(key: String) = ViewField(key, key, FieldKind.TEXT, null)
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HomeScreenTest*'`
Expected: compile error (`HomeScreen` unresolved).

- [ ] **Step 3: Add the strings**

English:

```xml
    <string name="home_recent">Recently added</string>
    <string name="home_frequent">Frequently used</string>
    <string name="home_frequent_empty">Items you fill or copy will show here.</string>
    <string name="home_identity_empty">Add your details</string>
```

Portuguese:

```xml
    <string name="home_recent">Adicionados recentemente</string>
    <string name="home_frequent">Usados com frequência</string>
    <string name="home_frequent_empty">Os itens que você preencher ou copiar aparecem aqui.</string>
    <string name="home_identity_empty">Adicione seus dados</string>
```

- [ ] **Step 4: Write `HomeScreen.kt`**

```kotlin
package net.havenkeys.android.ui.home

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.NoSharedTitle
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.shell.Settle
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.SummaryRow
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import uniffi.havenkeys_mobile.ItemSummary

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/**
 * Home (spec §6.5): the identity on top, then Recently added and Frequently
 * used. Pull to refresh syncs. The lists reload each time Home shows. The
 * first time a Home shows, its groups settle in sequence (spec §7).
 */
@Composable
fun HomeScreen(
    viewModel: HomeViewModel,
    onOpen: OpenItem,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(viewModel) {
        viewModel.shown()
        onPauseOrDispose {}
    }
    // Rows composed in the first frame settle; rows scrolled in later are just there.
    var settle by remember { mutableStateOf(!viewModel.settled) }
    LaunchedEffect(viewModel) {
        viewModel.settled = true
        settle = false
    }
    val rows = HomeRows(onOpen, sharedTitle, settle)
    PullToRefresh(refreshing = state.refreshing, onRefresh = viewModel::refresh, modifier = modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = 8.dp, bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
            state.identity?.let { card -> item(key = "identity") { Settle(0, active = rows.settle) { IdentityCardRow(card, onOpen) } } }
            activityGroup(rows, Recent, state.recent, state.loading)
            activityGroup(rows, Frequent, state.frequent, state.loading)
        }
    }
}

/** What every Home row needs besides its item. */
private class HomeRows(val onOpen: OpenItem, val sharedTitle: SharedTitle, val settle: Boolean)

/** One of Home's two lists: its title, what it says when empty, its rows' origin, its place in the settle. */
private class GroupSpec(@StringRes val title: Int, @StringRes val emptyText: Int, val origin: String, val index: Int)

private val Recent = GroupSpec(R.string.home_recent, R.string.vault_empty_hint, Origins.RECENT, index = 1)
private val Frequent = GroupSpec(R.string.home_frequent, R.string.home_frequent_empty, Origins.FREQUENT, index = 2)

/** A titled group of item rows; keys carry the origin, since one item can be in both groups. */
private fun LazyListScope.activityGroup(rows: HomeRows, spec: GroupSpec, items: List<ItemSummary>, loading: Boolean) {
    item(key = "${spec.origin}-title") {
        Settle(spec.index, active = rows.settle) {
            SectionHeader(stringResource(spec.title), Gutter.padding(top = 12.dp))
        }
    }
    if (items.isEmpty() && !loading) {
        item(key = "${spec.origin}-empty") {
            Settle(spec.index, active = rows.settle) { EmptyLine(stringResource(spec.emptyText), Gutter) }
        }
    }
    insetGroup(
        items,
        key = { "${spec.origin}-${it.id}" },
        around = { Settle(spec.index, active = rows.settle, content = it) },
    ) { summary ->
        SummaryRow(summary, spec.origin, rows.onOpen, sharedTitle = rows.sharedTitle)
    }
}

/** The identity: its title and the kinds of detail it holds, or an invitation to add them. */
@Composable
private fun IdentityCardRow(card: IdentityCard, onOpen: OpenItem) {
    val summary = if (card.parts.isEmpty()) {
        stringResource(R.string.home_identity_empty)
    } else {
        card.parts.map { stringResource(it.label) }.joinToString(" · ")
    }
    InsetGroup(Gutter) {
        row {
            GroupRow(onClick = { onOpen(card.id, Origins.IDENTITY) }, icon = HavenIcon.IdCard) {
                GroupRowText(card.title, summary)
            }
        }
    }
}
```

- [ ] **Step 5: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HomeScreenTest*'`
Expected: PASS.

- [ ] **Step 6: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/home apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/home/HomeScreenTest.kt
git commit -m "feat(android): Home with the identity card, recently added and frequently used"
```

---
### Task 8: Items list ViewModel

**Files:**
- Create: `ANDROID/ui/items/ItemListViewModel.kt`
- Test: `ANDROID_TEST/ui/items/ItemListViewModelTest.kt`

**Interfaces:**
- Consumes: `Category` (Task 3); `VaultRepository.list`, `AccountRepository.syncNow`, `VaultEventsHub.events`.
- Produces (package `net.havenkeys.android.ui.items`):
  - `data class ItemListUiState(items: List<ItemSummary>, refreshing: Boolean, errorCode: String?, loading: Boolean)`
  - `fun categoryCounts(items: List<ItemSummary>): Map<Category, Int>`
  - `internal fun alphabetical(items: List<ItemSummary>): List<ItemSummary>`
  - `class ItemListViewModel(vault: VaultRepository, accounts: AccountRepository, events: VaultEventsHub)` — `state: StateFlow<ItemListUiState>`, `fun refresh()`. One instance per screen (the Items root and each category list).

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/items/ItemListViewModelTest.kt`:

```kotlin
package net.havenkeys.android.ui.items

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class ItemListViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val bank = item("1", ItemKind.LOGIN, "Banco")
    private val agua = item("2", ItemKind.LOGIN, "Água", passkey = true)
    private val amazon = item("3", ItemKind.SECURE_NOTE, "amazon")
    private val card = item("4", ItemKind.CARD, "Visa")
    private val identity = item("5", ItemKind.IDENTITY, "Sam")

    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply { items = Outcome.Ok(listOf(bank, agua, amazon, card, identity)) }

    private fun vm() = ItemListViewModel(vault, accounts, events)

    @Test
    fun aToZIgnoresCaseAndAccents() {
        assertEquals(listOf("amazon", "Água", "Banco", "Sam", "Visa"), vm().state.value.items.map { it.title })
    }

    @Test
    fun eachCategoryIsCounted() {
        val counts = categoryCounts(vm().state.value.items)
        assertEquals(
            mapOf(
                Category.ALL to 5,
                Category.LOGINS to 2,
                Category.PASSKEYS to 1,
                Category.NOTES to 1,
                Category.CARDS to 1,
            ),
            counts,
        )
    }

    @Test
    fun aFailedLoadShowsItsCode() {
        vault.items = Outcome.Failed("locked")
        val state = vm().state.value
        assertEquals("locked", state.errorCode)
        assertTrue(state.items.isEmpty())
        assertFalse(state.loading)
    }

    @Test
    fun anItemsChangedEventReloads() {
        val vm = vm()
        vault.items = Outcome.Ok(listOf(card))
        events.itemsChanged()
        assertEquals(listOf(card), vm.state.value.items)
    }

    @Test
    fun aLockEmptiesTheList() {
        val vm = vm()
        events.locked("user")
        assertEquals(ItemListUiState(), vm.state.value)
        assertFalse(vm.state.value.toString().contains("Banco"))
    }

    @Test
    fun refreshSyncsAndReloads() = runTest {
        val vm = vm()
        vault.items = Outcome.Ok(listOf(identity))
        vm.refresh()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertEquals(listOf(identity), vm.state.value.items)
        assertFalse(vm.state.value.refreshing)
    }

    @Test
    fun aFailedRefreshShowsItsCodeAndKeepsTheList() = runTest {
        val vm = vm()
        accounts.sync = Outcome.Failed("offline")
        vm.refresh()
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(5, vm.state.value.items.size)
    }

    private fun item(id: String, kind: ItemKind, title: String, passkey: Boolean = false) =
        ItemSummary(id, kind, title, null, null, false, passkey, 0, 0)
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ItemListViewModelTest*'`
Expected: compile error (`ItemListViewModel` unresolved).

- [ ] **Step 3: Write `ItemListViewModel.kt`**

```kotlin
package net.havenkeys.android.ui.items

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import java.text.Collator
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.ItemSummary

/** Overviews only (Android spec §9.4): titles, usernames, websites, never a secret. */
data class ItemListUiState(
    val items: List<ItemSummary> = emptyList(),
    val refreshing: Boolean = false,
    val errorCode: String? = null,
    /** True until the first answer from Rust, so an empty list is not announced early. */
    val loading: Boolean = true,
)

/** How many items each category holds, for the Items tab. */
fun categoryCounts(items: List<ItemSummary>): Map<Category, Int> =
    Category.entries.associateWith { category -> items.count(category::keeps) }

/** A–Z as a reader expects: accents and case do not split the alphabet. */
internal fun alphabetical(items: List<ItemSummary>): List<ItemSummary> {
    val collator = Collator.getInstance().apply { strength = Collator.SECONDARY }
    return items.sortedWith { a, b -> collator.compare(a.title, b.title) }
}

/** The vault's items for the Items tab and a category list; each screen has its own. */
class ItemListViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(ItemListUiState())
    val state: StateFlow<ItemListUiState> = _state.asStateFlow()
    private var pending: Job? = null

    init {
        load()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.Unlocked, VaultEvent.ItemsChanged -> load()
                    is VaultEvent.Connectivity -> Unit
                }
            }
        }
    }

    /** Pull to refresh: a sync, then the list again. */
    fun refresh() {
        viewModelScope.launch {
            _state.update { it.copy(refreshing = true, errorCode = null) }
            val sync = accounts.syncNow()
            _state.update { it.copy(refreshing = false, errorCode = (sync as? Outcome.Failed)?.code) }
            if (sync is Outcome.Ok) load()
        }
    }

    private fun load() {
        pending?.cancel()
        pending = viewModelScope.launch {
            when (val result = vault.list()) {
                is Outcome.Ok -> _state.update {
                    it.copy(items = alphabetical(result.value), errorCode = null, loading = false)
                }
                is Outcome.Failed -> _state.update { it.copy(errorCode = result.code, loading = false) }
            }
        }
    }

    /** The lock wipe: nothing read from the vault stays here. */
    private fun wipe() {
        pending?.cancel()
        _state.value = ItemListUiState()
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ItemListViewModelTest*'`
Expected: PASS. (If `aToZIgnoresCaseAndAccents` fails on a machine whose default JVM locale has unusual collation, pin `Locale.setDefault(Locale.US)` in a `@Before` of the test, not in the code.)

- [ ] **Step 5: Run the suite and detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/items/ItemListViewModel.kt apps/android/app/src/test/kotlin/net/havenkeys/android/ui/items/ItemListViewModelTest.kt
git commit -m "feat(android): items view model with category counts and a reader's A to Z"
```

---

### Task 9: Items tab and category lists

**Files:**
- Create: `ANDROID/ui/items/ItemsScreen.kt`, `ANDROID/ui/items/CategoryScreen.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/items/ItemsScreensTest.kt`

**Interfaces:**
- Consumes: `ItemListViewModel`, `categoryCounts`, `Category` (Tasks 3, 8); `SummaryRow`, `insetGroup`, `LargeTitle`, `ErrorLine`, `EmptyLine`, `OpenItem`, `SharedTitle`, `Origins` (Task 1); kit `PullToRefresh`, `InsetGroup`, `GroupRow`, `GroupRowText`, `TrailingText`, `HavenIconButton`; `R.string.item_back` ("Back"), `R.string.tab_items`.
- Produces:
  - `@Composable fun ItemsScreen(viewModel: ItemListViewModel, onCategory: (Category) -> Unit, contentPadding: PaddingValues, modifier: Modifier = Modifier)`
  - `@Composable fun CategoryScreen(viewModel: ItemListViewModel, category: Category, onOpen: OpenItem, onBack: () -> Unit, contentPadding: PaddingValues, modifier: Modifier = Modifier, sharedTitle: SharedTitle = NoSharedTitle)`
  - String `items_empty`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/items/ItemsScreensTest.kt`:

```kotlin
package net.havenkeys.android.ui.items

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.shell.Origins
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class ItemsScreensTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(
            listOf(
                item("1", ItemKind.LOGIN, "bank"),
                item("2", ItemKind.LOGIN, "Amazon", passkey = true),
                item("3", ItemKind.SECURE_NOTE, "Wi-Fi"),
                item("4", ItemKind.IDENTITY, "Sam"),
            ),
        )
    }

    private fun vm() = ItemListViewModel(vault, FakeAccountRepository(), VaultEventsHub())

    @Test
    fun theItemsTabCountsEachCategoryAndOpensIt() {
        val vm = vm()
        val opened = mutableListOf<Category>()
        rule.setKit { ItemsScreen(vm, onCategory = { opened += it }, contentPadding = PaddingValues()) }
        rule.onNodeWithText("Items").assert(isHeading())
        rule.onNode(hasText("All items") and hasClickAction()).assert(hasText("4"))
        rule.onNode(hasText("Logins") and hasClickAction()).assert(hasText("2"))
        rule.onNode(hasText("Cards") and hasClickAction()).assert(hasText("0"))
        rule.onNode(hasText("Passkeys") and hasClickAction()).assert(hasText("1")).performClick()
        assertEquals(listOf(Category.PASSKEYS), opened)
    }

    @Test
    fun aCategoryListsItsItemsAToZAndOpensThem() {
        val vm = vm()
        val opened = mutableListOf<Pair<String, String>>()
        var back = 0
        rule.setKit {
            CategoryScreen(
                vm,
                Category.LOGINS,
                onOpen = { id, origin -> opened += id to origin },
                onBack = { back++ },
                contentPadding = PaddingValues(),
            )
        }
        rule.onNodeWithText("Logins").assert(isHeading())
        val amazon = rule.onNode(hasText("Amazon") and hasClickAction()).getUnclippedBoundsInRoot().top
        val bank = rule.onNode(hasText("bank") and hasClickAction()).getUnclippedBoundsInRoot().top
        assertTrue(amazon < bank)
        rule.onNodeWithText("Wi-Fi").assertDoesNotExist()
        rule.onNode(hasText("bank") and hasClickAction()).performClick()
        assertEquals(listOf("1" to Origins.CATEGORY), opened)
        rule.onNodeWithContentDescription("Back").performClick()
        assertEquals(1, back)
    }

    @Test
    fun anEmptyCategorySaysSo() {
        val vm = vm()
        rule.setKit { CategoryScreen(vm, Category.CARDS, onOpen = { _, _ -> }, onBack = {}, contentPadding = PaddingValues()) }
        rule.onNodeWithText("Nothing here yet").assertIsDisplayed()
    }

    private fun item(id: String, kind: ItemKind, title: String, passkey: Boolean = false) =
        ItemSummary(id, kind, title, null, null, false, passkey, 0, 0)
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ItemsScreensTest*'`
Expected: compile errors (`ItemsScreen`, `CategoryScreen` unresolved).

- [ ] **Step 3: Add the strings**

English: `    <string name="items_empty">Nothing here yet</string>`
Portuguese: `    <string name="items_empty">Nada aqui ainda</string>`

- [ ] **Step 4: Write `ItemsScreen.kt`**

```kotlin
package net.havenkeys.android.ui.items

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.kit.TrailingText
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenSpacing

/** The Items tab (spec §6.7): each category with its count; each opens its own list. */
@Composable
fun ItemsScreen(
    viewModel: ItemListViewModel,
    onCategory: (Category) -> Unit,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val counts = remember(state.items) { categoryCounts(state.items) }
    PullToRefresh(refreshing = state.refreshing, onRefresh = viewModel::refresh, modifier = modifier.fillMaxSize()) {
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(top = 8.dp, bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            LargeTitle(stringResource(R.string.tab_items))
            state.errorCode?.let { ErrorLine(it) }
            InsetGroup {
                Category.entries.forEach { category ->
                    row {
                        GroupRow(
                            onClick = { onCategory(category) },
                            icon = category.icon,
                            trailing = { if (!state.loading) TrailingText(counts.getValue(category).toString()) },
                            chevron = true,
                        ) { GroupRowText(stringResource(category.label)) }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 5: Write `CategoryScreen.kt`**

```kotlin
package net.havenkeys.android.ui.items

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.shell.NoSharedTitle
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.SummaryRow
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/**
 * One category's items, A–Z, inside the shell (spec §6.7): a large title
 * under the shared top bar, and a back chevron because that bar has none.
 * Pull to refresh syncs.
 */
@Composable
fun CategoryScreen(
    viewModel: ItemListViewModel,
    category: Category,
    onOpen: OpenItem,
    onBack: () -> Unit,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val shown = remember(state.items, category) { state.items.filter(category::keeps) }
    PullToRefresh(refreshing = state.refreshing, onRefresh = viewModel::refresh, modifier = modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") {
                Column(Gutter) {
                    HavenIconButton(HavenIcon.ChevronLeft, stringResource(R.string.item_back), onClick = onBack)
                    LargeTitle(stringResource(category.label))
                }
            }
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
            if (shown.isEmpty() && !state.loading && state.errorCode == null) {
                item(key = "empty") { EmptyLine(stringResource(R.string.items_empty), Gutter) }
            }
            insetGroup(shown, key = { it.id }) { summary ->
                SummaryRow(summary, Origins.CATEGORY, onOpen, sharedTitle = sharedTitle)
            }
        }
    }
}
```

- [ ] **Step 6: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ItemsScreensTest*'`
Expected: PASS.

- [ ] **Step 7: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/items apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/items/ItemsScreensTest.kt
git commit -m "feat(android): Items tab with counts and category lists A to Z"
```

---

### Task 10: Search ViewModel

**Files:**
- Create: `ANDROID/ui/search/SearchViewModel.kt`
- Test: `ANDROID_TEST/ui/search/SearchViewModelTest.kt`

**Interfaces:**
- Consumes: `VaultRepository.search/recentSearches/recordSearch/clearRecentSearches/touch`, `VaultEventsHub.events`.
- Produces (package `net.havenkeys.android.ui.search`):
  - `data class SearchUiState(query: String, recents: List<String>, results: List<ItemSummary>, searched: Boolean, errorCode: String?)`
  - `class SearchViewModel(vault: VaultRepository, events: VaultEventsHub)` — `state`, `fun setQuery(query: String)`, `fun useRecent(query: String)`, `fun opened()`, `fun clearRecents()`

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/search/SearchViewModelTest.kt`:

```kotlin
package net.havenkeys.android.ui.search

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class SearchViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam", null, false, false, 0, 0)
    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github))
        searches += listOf("bank", "mail")
    }

    private fun vm() = SearchViewModel(vault, events)

    @Test
    fun itOpensOnTheRecentSearches() {
        val state = vm().state.value
        assertEquals("", state.query)
        assertEquals(listOf("bank", "mail"), state.recents)
        assertTrue(state.results.isEmpty())
    }

    @Test
    fun typingSearchesCountsAsActivityAndRecordsNothing() {
        val vm = vm()
        vm.setQuery("git")
        assertEquals(listOf(github), vm.state.value.results)
        assertTrue(vm.state.value.searched)
        assertTrue("touch" in vault.calls)
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun openingAResultRecordsTheQuery() {
        val vm = vm()
        vm.setQuery("git ")
        vm.opened()
        assertEquals(listOf("git", "bank", "mail"), vault.searches)
    }

    @Test
    fun openingWithABlankQueryRecordsNothing() {
        val vm = vm()
        vm.setQuery("   ")
        vm.opened()
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun clearingTheFieldBringsBackTheRecentsWithTheNewOne() {
        val vm = vm()
        vm.setQuery("git")
        vm.opened()
        vm.setQuery("")
        assertTrue(vm.state.value.results.isEmpty())
        assertFalse(vm.state.value.searched)
        assertEquals(listOf("git", "bank", "mail"), vm.state.value.recents)
    }

    @Test
    fun aRecentSearchRunsAgainWithoutBeingRecorded() {
        val vm = vm()
        vm.useRecent("mail")
        assertEquals("mail", vm.state.value.query)
        assertEquals(listOf(github), vm.state.value.results)
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun clearEmptiesTheRecents() {
        val vm = vm()
        vm.clearRecents()
        assertTrue(vm.state.value.recents.isEmpty())
        assertTrue(vault.searches.isEmpty())
    }

    @Test
    fun theSameQueryAgainDoesNothing() {
        val vm = vm()
        vm.setQuery("git")
        vault.calls.clear()
        vm.setQuery("git")
        assertTrue(vault.calls.isEmpty())
    }

    @Test
    fun anItemsChangedEventSearchesAgain() {
        val vm = vm()
        vm.setQuery("git")
        vault.items = Outcome.Ok(emptyList())
        events.itemsChanged()
        assertTrue(vm.state.value.results.isEmpty())
    }

    @Test
    fun aFailedSearchShowsItsCode() {
        val vm = vm()
        vault.items = Outcome.Failed("locked")
        vm.setQuery("git")
        assertEquals("locked", vm.state.value.errorCode)
    }

    @Test
    fun aLockWipesTheQueryResultsAndRecents() {
        val vm = vm()
        vm.setQuery("git")
        events.locked("user")
        assertEquals(SearchUiState(), vm.state.value)
        assertFalse(vm.state.value.toString().contains("git"))
    }

    @Test
    fun signingOutWipesThemToo() {
        val vm = vm()
        vm.setQuery("git")
        events.signedOut()
        assertEquals(SearchUiState(), vm.state.value)
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*SearchViewModelTest*'`
Expected: compile error (`SearchViewModel` unresolved).

- [ ] **Step 3: Write `SearchViewModel.kt`**

```kotlin
package net.havenkeys.android.ui.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.ItemSummary

/**
 * The search screen's state. The query lives here, in memory, and nowhere
 * else: never in a route, `SavedStateHandle` or a log (spec §9).
 */
data class SearchUiState(
    val query: String = "",
    /** From Rust's per-device activity slot, newest first. */
    val recents: List<String> = emptyList(),
    val results: List<ItemSummary> = emptyList(),
    /** A search for the current query has answered, so "No matches" may show. */
    val searched: Boolean = false,
    val errorCode: String? = null,
)

class SearchViewModel(private val vault: VaultRepository, events: VaultEventsHub) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()
    private var pending: Job? = null

    init {
        loadRecents()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> _state.value.query.takeIf { it.isNotBlank() }?.let(::run)
                    else -> Unit
                }
            }
        }
    }

    /** Live search as the user types. Typing is activity for the auto-lock; it records nothing. */
    fun setQuery(query: String) {
        if (query == _state.value.query) return
        // Soft-keyboard typing does not reach `onUserInteraction`, and reads never count (Rust).
        vault.touch()
        _state.update { it.copy(query = query, searched = false, errorCode = null) }
        if (query.isBlank()) {
            pending?.cancel()
            _state.update { it.copy(results = emptyList()) }
            loadRecents()
        } else {
            run(query)
        }
    }

    /** A recent search tapped: it runs again. It is recorded only if a result is then opened. */
    fun useRecent(query: String) = setQuery(query)

    /** A result was opened: the current query becomes a recent search (spec §6.3). */
    fun opened() {
        val query = _state.value.query.trim()
        if (query.isEmpty()) return
        viewModelScope.launch { vault.recordSearch(query) }
    }

    fun clearRecents() {
        viewModelScope.launch {
            if (vault.clearRecentSearches() is Outcome.Ok) _state.update { it.copy(recents = emptyList()) }
        }
    }

    private fun run(query: String) {
        pending?.cancel()
        pending = viewModelScope.launch {
            when (val result = vault.search(query)) {
                is Outcome.Ok -> _state.update { it.copy(results = result.value, searched = true, errorCode = null) }
                is Outcome.Failed -> _state.update { it.copy(results = emptyList(), errorCode = result.code) }
            }
        }
    }

    private fun loadRecents() {
        viewModelScope.launch {
            (vault.recentSearches() as? Outcome.Ok)?.let { recents -> _state.update { it.copy(recents = recents.value) } }
        }
    }

    /** The lock wipe: the query, the results and the recents go. */
    private fun wipe() {
        pending?.cancel()
        _state.value = SearchUiState()
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*SearchViewModelTest*'`
Expected: PASS.

- [ ] **Step 5: Run the suite and detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/search apps/android/app/src/test/kotlin/net/havenkeys/android/ui/search
git commit -m "feat(android): search view model: live results, recents, record on open only, wiped on lock"
```

---

### Task 11: Search screen

**Files:**
- Create: `ANDROID/ui/search/SearchField.kt`, `ANDROID/ui/search/SearchScreen.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/search/SearchScreenTest.kt`

**Interfaces:**
- Consumes: `SearchViewModel`, `SearchUiState` (Task 10); `SummaryRow`, `insetGroup`, `ErrorLine`, `OpenItem`, `SharedTitle`, `Origins` (Task 1); kit `KitTextInput`, `HavenIconButton`, `HavenButton`, `SectionHeader`, `SectionAction`, `GroupRow`, `GroupRowText`; `HavenMotion.fadeSpec(delayMillis)`; strings `shell_search`, `vault_no_matches`, `vault_search_hint`.
- Produces:
  - `@Composable internal fun SearchField(state: TextFieldState, modifier: Modifier = Modifier)`
  - `@Composable fun SearchScreen(viewModel: SearchViewModel, onOpen: OpenItem, onCancel: () -> Unit, modifier: Modifier = Modifier, fieldModifier: Modifier = Modifier, sharedTitle: SharedTitle = NoSharedTitle)`
  - Strings `search_recent`, `search_clear`, `search_cancel`, `search_clear_field`.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/search/SearchScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.search

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
class SearchScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam", null, false, false, 0, 0)
    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github))
        searches += listOf("bank", "mail")
    }
    private val opened = mutableListOf<Pair<String, String>>()
    private var cancelled = 0

    private fun show() {
        val vm = SearchViewModel(vault, events)
        rule.setKit {
            SearchScreen(vm, onOpen = { id, origin -> opened += id to origin }, onCancel = { cancelled++ })
        }
    }

    @Test
    fun anEmptyFieldShowsRecentSearchesWithClear() {
        show()
        rule.onNodeWithText("Recent searches").assert(isHeading())
        rule.onNode(hasText("bank") and hasClickAction()).assertIsDisplayed()
        rule.onNode(hasText("Clear") and hasRole(Role.Button)).performClick()
        rule.onNodeWithText("bank").assertDoesNotExist()
        assertEquals(emptyList<String>(), vault.searches)
    }

    @Test
    fun aRecentSearchFillsTheFieldAndRunsAgain() {
        show()
        rule.onNode(hasText("mail") and hasClickAction()).performClick()
        rule.onNode(hasSetTextAction()).assert(hasText("mail"))
        rule.onNode(hasText("GitHub") and hasClickAction()).assertIsDisplayed()
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun typingShowsResultsAndOpeningOneRecordsTheQuery() {
        show()
        rule.onNode(hasSetTextAction()).performTextInput("git")
        assertEquals(listOf("bank", "mail"), vault.searches)
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        assertEquals(listOf("1" to Origins.SEARCH), opened)
        assertEquals(listOf("git", "bank", "mail"), vault.searches)
    }

    @Test
    fun noMatchesSaysSo() {
        vault.items = Outcome.Ok(emptyList())
        show()
        rule.onNode(hasSetTextAction()).performTextInput("zzz")
        rule.onNodeWithText("No matches").assertIsDisplayed()
    }

    @Test
    fun cancelLeaves() {
        show()
        rule.onNode(hasText("Cancel") and hasRole(Role.Button)).performClick()
        assertEquals(1, cancelled)
    }

    @Test
    fun aRestoredScreenDoesNotBringBackTheQuery() {
        val restoration = StateRestorationTester(rule)
        var vm = SearchViewModel(vault, events)
        restoration.setContent {
            HavenTheme { SearchScreen(vm, onOpen = { _, _ -> }, onCancel = {}) }
        }
        rule.onNode(hasSetTextAction()).performTextInput("private query")
        // The process came back: a new ViewModel, and only what was saved.
        vm = SearchViewModel(vault, events)
        restoration.emulateSavedInstanceStateRestore()
        rule.onNode(hasSetTextAction()).assert(hasText("private query", substring = true).not())
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*SearchScreenTest*'`
Expected: compile error (`SearchScreen` unresolved).

- [ ] **Step 3: Add the strings**

English:

```xml
    <string name="search_recent">Recent searches</string>
    <string name="search_clear">Clear</string>
    <string name="search_cancel">Cancel</string>
    <string name="search_clear_field">Clear search</string>
```

Portuguese:

```xml
    <string name="search_recent">Buscas recentes</string>
    <string name="search_clear">Limpar</string>
    <string name="search_cancel">Cancelar</string>
    <string name="search_clear_field">Limpar busca</string>
```

- [ ] **Step 4: Write `SearchField.kt`**

```kotlin
package net.havenkeys.android.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.KitTextInput
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The pill as a field: the search glyph, the query, and a clear button once
 * something is typed. TalkBack names it "Search HavenKeys"; the keyboard is
 * asked not to learn or correct what is typed.
 */
@Composable
internal fun SearchField(state: TextFieldState, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val label = stringResource(R.string.shell_search)
    val keyboard = LocalSoftwareKeyboardController.current
    KitTextInput {
        BasicTextField(
            state = state,
            modifier = modifier.semantics { contentDescription = label },
            textStyle = HavenTheme.type.value.copy(color = colors.textStrong),
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
            onKeyboardAction = { keyboard?.hide() },
            lineLimits = TextFieldLineLimits.SingleLine,
            cursorBrush = SolidColor(colors.brass),
            decorator = TextFieldDecorator { field ->
                Row(
                    Modifier
                        .heightIn(min = HavenSpacing.touch)
                        .clip(HavenShape.pill)
                        .background(colors.field)
                        .padding(start = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconGlyph(HavenIcon.Search, contentDescription = null, tint = colors.muted, size = 18.dp)
                    Spacer(Modifier.width(10.dp))
                    Box(Modifier.weight(1f).padding(vertical = 12.dp)) {
                        if (state.text.isEmpty()) {
                            HavenText(label, Modifier.clearAndSetSemantics {}, style = HavenTheme.type.value, color = colors.muted)
                        }
                        field()
                    }
                    if (state.text.isNotEmpty()) {
                        HavenIconButton(HavenIcon.X, stringResource(R.string.search_clear_field), onClick = { state.clearText() })
                    } else {
                        Spacer(Modifier.width(14.dp))
                    }
                }
            },
        )
    }
}
```

- [ ] **Step 5: Write `SearchScreen.kt`**

```kotlin
package net.havenkeys.android.ui.search

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.SectionAction
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.NoSharedTitle
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.SummaryRow
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Recent searches wait this long, so they fade in after the pill has grown (spec §7). */
private const val RECENTS_DELAY_MILLIS = 160

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/**
 * Search (spec §6.3): the pill grown into a focused field. Empty, it shows
 * recent searches with Clear; typing shows live results; opening one
 * records the query. The field's text is `remember`ed, never saved, and
 * the ViewModel keeps the query in memory only: a query never goes into a
 * route, saved instance state or a log.
 */
@Composable
fun SearchScreen(
    viewModel: SearchViewModel,
    onOpen: OpenItem,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
    fieldModifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val field = remember(viewModel) { TextFieldState(viewModel.state.value.query) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(field) { snapshotFlow { field.text.toString() }.collect(viewModel::setQuery) }
    // The ViewModel changes the query too: a recent search tapped, the lock wipe.
    LaunchedEffect(state.query) {
        if (field.text.toString() != state.query) field.setTextAndPlaceCursorAtEnd(state.query)
    }
    LaunchedEffect(focus) { focus.requestFocus() }
    val open: OpenItem = remember(viewModel, onOpen) {
        { id, origin ->
            viewModel.opened()
            onOpen(id, origin)
        }
    }
    Column(
        modifier
            .fillMaxSize()
            .background(HavenTheme.colors.pane)
            .statusBarsPadding()
            .navigationBarsPadding()
            .imePadding(),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = HavenSpacing.gutter, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SearchField(field, Modifier.weight(1f).then(fieldModifier).focusRequester(focus))
            HavenButton(stringResource(R.string.search_cancel), onClick = onCancel, style = ButtonStyle.Quiet)
        }
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = HavenSpacing.gutter)) {
            if (state.query.isBlank()) {
                recents(state.recents, viewModel::useRecent, viewModel::clearRecents)
            } else {
                results(state, open, sharedTitle)
            }
        }
    }
}

private fun LazyListScope.recents(recents: List<String>, onUse: (String) -> Unit, onClear: () -> Unit) {
    if (recents.isEmpty()) return
    item(key = "recents-title") {
        Later {
            SectionHeader(
                stringResource(R.string.search_recent),
                Gutter.padding(top = 8.dp),
                action = SectionAction(stringResource(R.string.search_clear), onClear),
            )
        }
    }
    insetGroup(recents, key = null, around = { Later(it) }) { query ->
        GroupRow(onClick = { onUse(query) }, icon = HavenIcon.Clock) { GroupRowText(query) }
    }
}

private fun LazyListScope.results(state: SearchUiState, open: OpenItem, sharedTitle: SharedTitle) {
    state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
    if (state.searched && state.results.isEmpty() && state.errorCode == null) {
        item(key = "none") {
            Column(Gutter.padding(vertical = 16.dp)) {
                HavenText(
                    stringResource(R.string.vault_no_matches),
                    style = HavenTheme.type.titleSmall,
                    color = HavenTheme.colors.textStrong,
                )
                HavenText(stringResource(R.string.vault_search_hint), color = HavenTheme.colors.muted)
            }
        }
    }
    insetGroup(state.results, key = { it.id }) { summary ->
        SummaryRow(summary, Origins.SEARCH, open, sharedTitle = sharedTitle)
    }
}

/** Fades its content in after the pill has grown; under "Remove animations" it is simply there. */
@Composable
private fun Later(content: @Composable () -> Unit) {
    val motion = HavenTheme.motion
    val shown = remember { Animatable(if (motion.reduced) 1f else 0f) }
    LaunchedEffect(shown) { shown.animateTo(1f, motion.fadeSpec(delayMillis = RECENTS_DELAY_MILLIS)) }
    Box(Modifier.graphicsLayer { alpha = shown.value }) { content() }
}
```

- [ ] **Step 6: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*SearchScreenTest*'`
Expected: PASS. If `aRestoredScreenDoesNotBringBackTheQuery` fails, something made the field saveable (`rememberSaveable`, `rememberTextFieldState`): that is the bug, not the test.

- [ ] **Step 7: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/search apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/search/SearchScreenTest.kt
git commit -m "feat(android): search screen with recents, Clear and live results; the query is never saved"
```

---
### Task 12: Settings as grouped rows, with kit dialogs and a choice sheet

Re-read `ANDROID/ui/kit/HavenDialog.kt` first: this task uses the stage 2 review's `HavenDialog(…, content, confirmEnabled, busy, answerKey)` (a field inside the dialog, caller-owned enablement and busy state, re-armed after a failed confirm). If its parameter names differ from what is below, follow the source.

**Files:**
- Rewrite: `ANDROID/ui/settings/SettingsScreen.kt`, `ANDROID/ui/settings/SettingsDialog.kt`, `ANDROID/ui/settings/SettingsNavigation.kt`
- Create: `ANDROID/ui/settings/SettingsChoices.kt`, `ANDROID/ui/settings/SettingsActions.kt`
- Modify: `ANDROID/ui/nav/HavenNavHost.kt` (the `Routes.SETTINGS` destination only, until Task 14)
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Test: `ANDROID_TEST/ui/settings/SettingsScreenTest.kt`

**Interfaces:**
- Consumes: `SettingsViewModel` and `SettingsUiState` unchanged (`setAutoLock`, `setClipboardSeconds`, `setLockOnScreenOff`, `setConfirmBeforeFilling`, `setAssetLinks`, `biometricChanged(result)`, `signOut()`, `removeDevice(confirmation)`, `clearRemoveError()`, `AUTO_LOCK_CHOICES`, `CLIPBOARD_CHOICES`); `enrollBiometrics(activity, container, password)`; `LargeTitle` (Task 1); kit `InsetGroup`, `GroupRow`, `GroupRowText`, `ToggleRow`, `SectionHeader`, `HavenSheet`, `HavenDialog`, `DialogAction`, `HavenTextField`, `SecretTextField`, `HavenPress`, `IconGlyph`, `HavenText`.
- Produces:
  - `class SettingsActions(val biometricAvailable: Boolean, val forgetBiometric: () -> Unit, val enrollBiometric: suspend (password: String) -> Outcome<Unit>)`
  - `@Composable fun rememberSettingsActions(container: AppContainer, activity: FragmentActivity): SettingsActions`
  - `class SettingsNavigation(val onDevices: () -> Unit, val onAutofillSetup: () -> Unit)`
  - `@Composable fun SettingsScreen(viewModel: SettingsViewModel, online: Boolean, actions: SettingsActions, navigation: SettingsNavigation, contentPadding: PaddingValues, modifier: Modifier = Modifier)`
  - `enum class SettingsChoice { AUTO_LOCK, CLIPBOARD }`, `@Composable internal fun autoLockText(minutes: UInt): String`
  - String `settings_signed_in_as`.

Decision for the auto-lock and clipboard choices: a sheet of single-choice rows (`Role.RadioButton`, a brass check on the current one), written here in `SettingsChoices.kt` rather than added to the kit; a `SegmentedControl` would not fit five labels like "After 15 minutes" on a phone.

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/settings/SettingsScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.settings

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class SettingsScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val settings = FakeSettingsRepository()
    private val accounts = FakeAccountRepository()
    private val done = mutableListOf<String>()

    private fun show(online: Boolean = true, enrolled: Boolean = false) {
        val vm = SettingsViewModel(settings, accounts, FakeVaultRepository(), biometricEnrolled = { enrolled })
        val actions = SettingsActions(
            biometricAvailable = true,
            forgetBiometric = { done += "forget" },
            enrollBiometric = { password ->
                done += "enroll:$password"
                Outcome.Ok(Unit)
            },
        )
        val navigation = SettingsNavigation(onDevices = { done += "devices" }, onAutofillSetup = { done += "setup" })
        rule.setKit { SettingsScreen(vm, online, actions, navigation, PaddingValues()) }
    }

    private fun row(text: String) = rule.onNode(hasText(text) and hasClickAction())

    private fun switch(text: String) = rule.onNode(hasText(text) and hasRole(Role.Switch))

    /** A dialog's button; its row behind the dialog has the same words. */
    private fun dialogButton(text: String) =
        rule.onNode(hasText(text) and hasRole(Role.Button) and hasAnyAncestor(isDialog()))

    @Test
    fun everySettingIsAGroupedRow() {
        show()
        listOf("Settings", "Security", "Autofill", "Account").forEach { rule.onNodeWithText(it).assert(isHeading()) }
        row("Lock automatically").assert(hasText("After 15 minutes"))
        row("Clear copied items").assert(hasText("After 30 seconds"))
        switch("Lock when the screen turns off").assertIsOn()
        switch("Unlock with fingerprint or face").assertIsOff()
        switch("Confirm before filling").assertIsOff()
        switch("Check website–app links").assertIsOn()
        rule.onNodeWithText("user@example.com").assertExists()
        row("Autofill setup").performClick()
        row("Devices").performClick()
        assertEquals(listOf("setup", "devices"), done)
    }

    @Test
    fun aSwitchSavesTheSetting() {
        show()
        switch("Confirm before filling").performClick()
        assertEquals(true, settings.updates.last().confirmBeforeFilling)
        switch("Confirm before filling").assertIsOn()
    }

    @Test
    fun aChoiceOpensASheetAndSavesThePick() {
        show()
        row("Lock automatically").performClick()
        rule.onNode(isDialog()).assertExists()
        rule.onNode(hasText("After 15 minutes") and hasRole(Role.RadioButton)).assert(isSelected())
        rule.onNode(hasText("After 5 minutes") and hasRole(Role.RadioButton)).performClick()
        assertEquals(5u, settings.updates.last().autoLockMinutes)
        rule.onNode(isDialog()).assertDoesNotExist()
    }

    @Test
    fun turningBiometricsOffForgetsTheKey() {
        show(enrolled = true)
        switch("Unlock with fingerprint or face").assertIsOn().performClick()
        assertEquals(listOf("forget"), done)
    }

    @Test
    fun turningBiometricsOnAsksForTheMasterPasswordInADialog() {
        show()
        switch("Unlock with fingerprint or face").performClick()
        dialogButton("Turn on").assertIsNotEnabled()
        rule.onNode(hasSetTextAction()).performTextInput("correct horse")
        dialogButton("Turn on").performClick()
        rule.waitForIdle()
        assertEquals(listOf("enroll:correct horse"), done)
        rule.onNode(isDialog()).assertDoesNotExist()
    }

    @Test
    fun signingOutAsksFirst() {
        show()
        row("Sign out and lock").performClick()
        dialogButton("Sign out and lock").performClick()
        assertTrue("signOut" in accounts.calls)
    }

    @Test
    fun removingTheDeviceShowsRustsAnswerAndCanTryAgain() {
        accounts.done = Outcome.Failed("invalid_input")
        show()
        row("Remove this device").performClick()
        val remove = dialogButton("Remove this device")
        remove.assertIsNotEnabled()
        rule.onNode(hasSetTextAction()).performTextInput("someone@example.com")
        remove.performClick()
        rule.onNodeWithText("That is not this account’s email.").assertExists()
        accounts.done = Outcome.Ok(Unit)
        remove.performClick()
        assertEquals(
            listOf("removeDevice:someone@example.com", "removeDevice:someone@example.com"),
            accounts.calls.filter { it.startsWith("removeDevice") },
        )
    }

    @Test
    fun offlineAccountRowsSaySo() {
        show(online = false)
        row("Sign out and lock").assert(hasText("HavenKeys is offline — the vault is read-only until it reconnects."))
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*SettingsScreenTest*'`
Expected: compile errors (`SettingsActions`, the new `SettingsScreen` and `SettingsNavigation` signatures).

- [ ] **Step 3: Add the string**

English: `    <string name="settings_signed_in_as">Signed in as</string>`
Portuguese: `    <string name="settings_signed_in_as">Conectado como</string>`

- [ ] **Step 4: Write `SettingsActions.kt` and rewrite `SettingsNavigation.kt`**

`ANDROID/ui/settings/SettingsActions.kt`:

```kotlin
package net.havenkeys.android.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.fragment.app.FragmentActivity
import net.havenkeys.android.AppContainer
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.unlock.enrollBiometrics

/** What Settings needs from outside its ViewModel: biometric enrolment runs a prompt on the activity. */
class SettingsActions(
    val biometricAvailable: Boolean,
    val forgetBiometric: () -> Unit,
    /** Called on the main thread (BiometricPrompt requires it); the password goes straight to Rust. */
    val enrollBiometric: suspend (password: String) -> Outcome<Unit>,
)

/** The real actions: the container's biometric gate and keys, a prompt on [activity]. */
@Composable
fun rememberSettingsActions(container: AppContainer, activity: FragmentActivity): SettingsActions =
    remember(container, activity) {
        SettingsActions(
            biometricAvailable = container.biometricGate.available(activity),
            forgetBiometric = container::forgetBiometricUnlock,
            enrollBiometric = { password -> enrollBiometrics(activity, container, password) },
        )
    }
```

`ANDROID/ui/settings/SettingsNavigation.kt`:

```kotlin
package net.havenkeys.android.ui.settings

/** Where Settings leads; none carries anything from the vault. The shell's top bar has Lock. */
class SettingsNavigation(val onDevices: () -> Unit, val onAutofillSetup: () -> Unit)
```

- [ ] **Step 5: Write `SettingsChoices.kt`**

```kotlin
package net.havenkeys.android.ui.settings

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenPress
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.MobileSettings

/** The settings chosen from a short list. */
enum class SettingsChoice { AUTO_LOCK, CLIPBOARD }

private const val MINUTES_PER_HOUR = 60u

@Composable
internal fun autoLockText(minutes: UInt): String = when (minutes) {
    0u -> stringResource(R.string.settings_never)
    MINUTES_PER_HOUR -> stringResource(R.string.settings_after_hour)
    else -> stringResource(R.string.settings_after_minutes, minutes.toInt())
}

@Composable
internal fun clipboardText(seconds: UInt): String = stringResource(R.string.settings_after_seconds, seconds.toInt())

/** The sheet for [choice]; picking a value saves it and closes the sheet. */
@Composable
internal fun SettingsChoiceSheet(choice: SettingsChoice, settings: MobileSettings, viewModel: SettingsViewModel, onClose: () -> Unit) {
    when (choice) {
        SettingsChoice.AUTO_LOCK -> ChoiceSheet(
            Choices(
                stringResource(R.string.settings_auto_lock),
                SettingsViewModel.AUTO_LOCK_CHOICES,
                settings.autoLockMinutes,
            ) { autoLockText(it) },
            onSelect = viewModel::setAutoLock,
            onClose = onClose,
        )
        SettingsChoice.CLIPBOARD -> ChoiceSheet(
            Choices(
                stringResource(R.string.settings_clipboard),
                // A value set on the desktop outside the phone's list still shows, selected.
                (SettingsViewModel.CLIPBOARD_CHOICES + settings.clipboardClearSeconds).distinct().sorted(),
                settings.clipboardClearSeconds,
            ) { clipboardText(it) },
            onSelect = viewModel::setClipboardSeconds,
            onClose = onClose,
        )
    }
}

private class Choices(
    val title: String,
    val values: List<UInt>,
    val selected: UInt,
    val text: @Composable (UInt) -> String,
)

@Composable
private fun ChoiceSheet(choices: Choices, onSelect: (UInt) -> Unit, onClose: () -> Unit) {
    HavenSheet(onDismiss = onClose, title = choices.title) {
        InsetGroup(Modifier.selectableGroup()) {
            choices.values.forEach { value ->
                row {
                    ChoiceOption(choices.text(value), selected = value == choices.selected) {
                        onClose()
                        if (value != choices.selected) onSelect(value)
                    }
                }
            }
        }
    }
}

/** One value: a radio button for TalkBack, a brass check when it is the current one. */
@Composable
private fun ChoiceOption(label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = HavenTheme.colors
    Row(
        Modifier
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
        HavenText(label, Modifier.weight(1f), style = HavenTheme.type.value, color = colors.textStrong)
        if (selected) IconGlyph(HavenIcon.Check, contentDescription = null, tint = colors.brass, size = 20.dp)
    }
}
```

- [ ] **Step 6: Rewrite `SettingsDialog.kt` on `HavenDialog`**

```kotlin
package net.havenkeys.android.ui.settings

import androidx.annotation.StringRes
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField

enum class SettingsDialog { BIOMETRIC_PASSWORD, SIGN_OUT, REMOVE }

@Composable
internal fun SettingsDialogs(
    dialog: SettingsDialog?,
    state: SettingsUiState,
    viewModel: SettingsViewModel,
    onClose: () -> Unit,
    onPassword: (String) -> Unit,
) {
    when (dialog) {
        SettingsDialog.BIOMETRIC_PASSWORD -> PasswordDialog(
            onDismiss = onClose,
            onSubmit = { password ->
                onClose()
                onPassword(password)
            },
        )
        SettingsDialog.SIGN_OUT -> HavenDialog(
            title = stringResource(R.string.settings_sign_out_confirm),
            onDismiss = onClose,
            confirm = DialogAction(stringResource(R.string.settings_sign_out), {
                onClose()
                viewModel.signOut()
            }),
            dismiss = DialogAction(stringResource(R.string.settings_cancel), onClose),
        )
        SettingsDialog.REMOVE -> RemoveDialog(
            state = state,
            onDismiss = {
                viewModel.clearRemoveError()
                onClose()
            },
            onRemove = viewModel::removeDevice,
        )
        null -> Unit
    }
}

/**
 * The master password lives only in this dialog's `remember`ed field (never
 * saved) until it is handed to Rust, and the field is cleared as it goes.
 */
@Composable
private fun PasswordDialog(onDismiss: () -> Unit, onSubmit: (String) -> Unit) {
    val field = remember { TextFieldState() }
    var revealed by remember { mutableStateOf(false) }
    val submit = {
        val typed = field.text.toString()
        if (typed.isNotEmpty()) {
            field.clearText()
            onSubmit(typed)
        }
    }
    HavenDialog(
        title = stringResource(R.string.settings_biometric),
        onDismiss = onDismiss,
        confirm = DialogAction(stringResource(R.string.settings_turn_on), submit),
        message = stringResource(R.string.settings_biometric_password),
        dismiss = DialogAction(stringResource(R.string.settings_cancel), onDismiss),
        confirmEnabled = field.text.isNotEmpty(),
        content = {
            InsetGroup {
                row {
                    SecretTextField(
                        field,
                        stringResource(R.string.unlock_password_hint),
                        revealed = revealed,
                        onRevealChange = { revealed = it },
                        onKeyboardAction = { submit() },
                    )
                }
            }
        },
    )
}

/** Rust compares the typed email with the account's; this dialog only passes it on and shows the answer. */
@Composable
private fun RemoveDialog(state: SettingsUiState, onDismiss: () -> Unit, onRemove: (String) -> Unit) {
    val field = remember { TextFieldState() }
    val prompt = state.email?.let { stringResource(R.string.settings_type_to_confirm, it) }
        ?: stringResource(R.string.settings_type_email)
    val error = state.removeErrorCode?.let { stringResource(removeErrorText(it)) }
    HavenDialog(
        title = stringResource(R.string.settings_remove_title),
        onDismiss = onDismiss,
        confirm = DialogAction(
            stringResource(R.string.settings_remove_title),
            { onRemove(field.text.toString()) },
            danger = true,
        ),
        message = stringResource(R.string.settings_remove_note),
        dismiss = DialogAction(stringResource(R.string.settings_cancel), onDismiss),
        confirmEnabled = field.text.isNotBlank(),
        busy = state.removing,
        answerKey = state.removeErrorCode,
        content = {
            InsetGroup {
                row {
                    HavenTextField(
                        field,
                        prompt,
                        error = error,
                        enabled = !state.removing,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false),
                    )
                }
            }
        },
    )
}

@StringRes
private fun removeErrorText(code: String): Int = when (code) {
    INVALID_INPUT -> R.string.settings_remove_mismatch
    "internal" -> R.string.settings_remove_failed
    else -> errorText(code)
}
```

- [ ] **Step 7: Rewrite `SettingsScreen.kt`**

```kotlin
package net.havenkeys.android.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.ToggleRow
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.MobileSettings

/** Rust's code for a refused value; it reads as the desktop's "Could not save settings." */
internal const val INVALID_INPUT = "invalid_input"

/** Opens a choice sheet or a dialog; one handle so the groups stay short. */
private class SettingsOpen(val choose: (SettingsChoice) -> Unit, val dialog: (SettingsDialog) -> Unit)

/**
 * The Settings tab (spec §6.8): today's settings as grouped rows. The
 * shell's top bar above it has Lock and Sync now; Devices and Autofill
 * setup open full screen over the shell.
 */
@Composable
fun SettingsScreen(
    viewModel: SettingsViewModel,
    online: Boolean,
    actions: SettingsActions,
    navigation: SettingsNavigation,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var dialog by remember { mutableStateOf<SettingsDialog?>(null) }
    var choosing by remember { mutableStateOf<SettingsChoice?>(null) }
    val open = remember { SettingsOpen(choose = { choosing = it }, dialog = { dialog = it }) }
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = HavenSpacing.gutter)
            .padding(top = 8.dp, bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
    ) {
        LargeTitle(stringResource(R.string.settings_title))
        state.errorCode?.let { SettingsError(it) }
        state.settings?.let { settings ->
            SecurityGroup(settings, state.biometricEnrolled, actions, viewModel, open)
            AutofillGroup(settings, viewModel, navigation.onAutofillSetup)
        }
        AccountGroup(state.email, online, navigation, open)
    }
    val settings = state.settings
    val choice = choosing
    if (settings != null && choice != null) {
        SettingsChoiceSheet(choice, settings, viewModel, onClose = { choosing = null })
    }
    SettingsDialogs(
        dialog = dialog,
        state = state,
        viewModel = viewModel,
        onClose = { dialog = null },
        // The composition's scope: main thread, as BiometricPrompt requires.
        onPassword = { password -> scope.launch { viewModel.biometricChanged(actions.enrollBiometric(password)) } },
    )
}

@Composable
private fun SettingsError(code: String) {
    HavenText(
        stringResource(if (code == INVALID_INPUT) R.string.settings_save_failed else errorText(code)),
        Modifier.padding(vertical = 8.dp),
        color = HavenTheme.colors.danger,
    )
}

@Composable
private fun SecurityGroup(
    settings: MobileSettings,
    enrolled: Boolean,
    actions: SettingsActions,
    viewModel: SettingsViewModel,
    open: SettingsOpen,
) {
    // Turning off always works; turning on needs a strong biometric enrolled.
    val canEnroll = enrolled || actions.biometricAvailable
    SectionHeader(stringResource(R.string.settings_security), Modifier.padding(top = 8.dp))
    InsetGroup {
        row {
            GroupRow(onClick = { open.choose(SettingsChoice.AUTO_LOCK) }) {
                GroupRowText(stringResource(R.string.settings_auto_lock), autoLockText(settings.autoLockMinutes))
            }
        }
        row {
            GroupRow(onClick = { open.choose(SettingsChoice.CLIPBOARD) }) {
                GroupRowText(stringResource(R.string.settings_clipboard), clipboardText(settings.clipboardClearSeconds))
            }
        }
        row {
            ToggleRow(
                stringResource(R.string.settings_lock_on_screen_off),
                settings.lockOnScreenOff,
                viewModel::setLockOnScreenOff,
            )
        }
        row {
            ToggleRow(
                stringResource(R.string.settings_biometric),
                checked = enrolled,
                onCheckedChange = { on ->
                    if (on) {
                        open.dialog(SettingsDialog.BIOMETRIC_PASSWORD)
                    } else {
                        actions.forgetBiometric()
                        viewModel.biometricChanged()
                    }
                },
                detail = stringResource(
                    if (canEnroll) R.string.settings_biometric_note else R.string.error_biometric_unavailable,
                ),
                enabled = canEnroll,
            )
        }
    }
}

@Composable
private fun AutofillGroup(settings: MobileSettings, viewModel: SettingsViewModel, onAutofillSetup: () -> Unit) {
    SectionHeader(stringResource(R.string.settings_autofill), Modifier.padding(top = 16.dp))
    InsetGroup {
        row { GroupRow(onClick = onAutofillSetup) { GroupRowText(stringResource(R.string.settings_autofill_setup)) } }
        row {
            ToggleRow(
                stringResource(R.string.settings_confirm_before_filling),
                settings.confirmBeforeFilling,
                viewModel::setConfirmBeforeFilling,
                detail = stringResource(R.string.settings_confirm_before_filling_note),
            )
        }
        row {
            ToggleRow(
                stringResource(R.string.settings_asset_links),
                settings.assetLinks,
                viewModel::setAssetLinks,
                detail = stringResource(R.string.settings_asset_links_note),
            )
        }
    }
}

@Composable
private fun AccountGroup(email: String?, online: Boolean, navigation: SettingsNavigation, open: SettingsOpen) {
    val offline = if (online) null else stringResource(R.string.error_offline)
    SectionHeader(stringResource(R.string.settings_account), Modifier.padding(top = 16.dp))
    InsetGroup {
        if (email != null) row { GroupRow { GroupRowText(stringResource(R.string.settings_signed_in_as), email) } }
        row { GroupRow(onClick = navigation.onDevices) { GroupRowText(stringResource(R.string.settings_devices)) } }
        // Offline, signing out still locks; only the server's session end waits.
        row {
            GroupRow(onClick = { open.dialog(SettingsDialog.SIGN_OUT) }) {
                GroupRowText(stringResource(R.string.settings_sign_out), offline)
            }
        }
    }
    Spacer(Modifier.height(HavenSpacing.groupGap))
    InsetGroup {
        row {
            GroupRow(onClick = { open.dialog(SettingsDialog.REMOVE) }) {
                HavenText(
                    stringResource(R.string.settings_remove_title),
                    style = HavenTheme.type.value,
                    color = HavenTheme.colors.danger,
                )
                HavenText(
                    offline ?: stringResource(R.string.settings_remove_note),
                    style = HavenTheme.type.rowSubtitle,
                    color = HavenTheme.colors.muted,
                )
            }
        }
    }
}
```

- [ ] **Step 8: Keep the app's NavHost compiling until Task 14**

In `ANDROID/ui/nav/HavenNavHost.kt`, replace the body of `composable(Routes.SETTINGS) { … }` with:

```kotlin
    composable(Routes.SETTINGS) {
        val online by container.events.online.collectAsStateWithLifecycle()
        // A pushed screen until the shell hosts Settings as a tab (Task 14); system Back leaves it.
        SettingsScreen(
            viewModel = viewModel {
                SettingsViewModel(
                    container.settingsRepository,
                    container.accountRepository,
                    container.vaultRepository,
                    biometricEnrolled = container::hasBiometricUnlock,
                )
            },
            online = online,
            actions = rememberSettingsActions(container, activity),
            navigation = SettingsNavigation(
                onDevices = { navController.navigate(Routes.DEVICES) },
                onAutofillSetup = { navController.navigate(Routes.AUTOFILL_SETUP) },
            ),
            contentPadding = PaddingValues(),
            modifier = Modifier.background(HavenTheme.colors.pane).statusBarsPadding(),
        )
    }
```

Add the imports it needs (`androidx.compose.foundation.background`, `androidx.compose.foundation.layout.PaddingValues`, `androidx.compose.foundation.layout.statusBarsPadding`, `net.havenkeys.android.ui.settings.rememberSettingsActions`) and remove the now-unused `val lock` if detekt reports it.

- [ ] **Step 9: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*SettingsScreenTest*' --tests '*SettingsViewModelTest*'`
Expected: PASS (the ViewModel test is unchanged and still passes).

- [ ] **Step 10: Run the suite, detekt, lint and assemble**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug`
Expected: all pass.

- [ ] **Step 11: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/settings apps/android/app/src/main/kotlin/net/havenkeys/android/ui/nav/HavenNavHost.kt apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml apps/android/app/src/test/kotlin/net/havenkeys/android/ui/settings/SettingsScreenTest.kt
git commit -m "feat(android): Settings as grouped rows with kit dialogs and a choice sheet"
```

---
### Task 13: The shell screen

**Files:**
- Create: `ANDROID/ui/shell/ShellScreen.kt`
- Test: `ANDROID_TEST/ui/shell/ShellScreenTest.kt`

**Interfaces:**
- Consumes: `ShellViewModel`, `AddSheet`, `AddTile` (Task 5); `ShellTopBar`, `TopBarActions`, `BottomBar` (Task 4); `ShellNavHost`, `ShellScreens`, `Tab`, `tab()`, `selectTab` (Task 3); kit `HavenScaffold`, `AddButton`, `rememberToastState`, `ToastTone`; `errorText`.
- Produces:
  - `class ShellNavigation(val onSearch: () -> Unit, val onNew: (ItemKind) -> Unit, val onGenerator: () -> Unit)`
  - `@Composable fun ShellScreen(viewModel: ShellViewModel, screens: ShellScreens, navigation: ShellNavigation, modifier: Modifier = Modifier, searchPillModifier: Modifier = Modifier)`

- [ ] **Step 1: Write the failing test**

`ANDROID_TEST/ui/shell/ShellScreenTest.kt`:

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShellScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository()
    private val accounts = FakeAccountRepository()
    private val events = VaultEventsHub()
    private val picks = mutableListOf<String>()

    private val screens = ShellScreens(
        home = { HavenText("Home root") },
        items = { _, onCategory -> HavenButton("Open logins", onClick = { onCategory(Category.LOGINS) }) },
        category = { _, category, _ -> HavenText("List ${category.arg}") },
        settings = { HavenText("Settings root") },
    )

    private fun show() {
        val vm = ShellViewModel(vault, accounts, events)
        val navigation = ShellNavigation(
            onSearch = { picks += "search" },
            onNew = { picks += "new:$it" },
            onGenerator = { picks += "generator" },
        )
        rule.setKit { ShellScreen(vm, screens, navigation) }
    }

    private fun tab(label: String) = rule.onNode(hasText(label) and hasRole(Role.Tab))

    @Test
    fun theBarsFrameTheCurrentTab() {
        show()
        rule.onNodeWithText("Home root").assertIsDisplayed()
        tab("Home").assertIsSelected()
        rule.onNode(hasText("Search HavenKeys") and hasRole(Role.Button)).assertIsDisplayed()
    }

    @Test
    fun theBottomBarSwitchesTabsAndAReselectPopsToTheRoot() {
        show()
        tab("Items").performClick()
        rule.onNodeWithText("Open logins").performClick()
        rule.onNodeWithText("List logins").assertIsDisplayed()
        tab("Home").performClick()
        tab("Items").performClick()
        rule.onNodeWithText("List logins").assertIsDisplayed()
        tab("Items").performClick()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun settingsHasNoAddButton() {
        show()
        rule.onNodeWithContentDescription("New item").assertIsDisplayed()
        tab("Settings").performClick()
        rule.onNodeWithContentDescription("New item").assertDoesNotExist()
    }

    @Test
    fun offlineTheAddSheetOffersOnlyTheGenerator() {
        show()
        rule.onNodeWithContentDescription("New item").performClick()
        rule.onNode(hasText("Login") and hasRole(Role.Button)).assertIsNotEnabled()
        rule.onNode(hasText("Generate password") and hasRole(Role.Button)).performClick()
        assertEquals(listOf("generator"), picks)
        rule.onNode(isDialog()).assertDoesNotExist()
    }

    @Test
    fun onlineAPickedTileOpensItsForm() {
        events.connectivity(true)
        show()
        rule.onNodeWithContentDescription("New item").performClick()
        rule.onNode(hasText("Secure note") and hasRole(Role.Button)).performClick()
        assertEquals(listOf("new:SECURE_NOTE"), picks)
    }

    @Test
    fun theTopBarSearchesSyncsAndLocks() {
        show()
        rule.onNode(hasText("Search HavenKeys") and hasRole(Role.Button)).performClick()
        rule.onNodeWithContentDescription("Sync now").performClick()
        rule.onNodeWithContentDescription("Lock now").performClick()
        assertEquals(listOf("search"), picks)
        assertEquals(listOf("syncNow"), accounts.calls)
        assertTrue("lock" in vault.calls)
    }

    @Test
    fun aFailedSyncSaysWhyInAToast() {
        accounts.sync = Outcome.Failed("offline")
        show()
        rule.onNodeWithContentDescription("Sync now").performClick()
        rule.onNodeWithText("HavenKeys is offline — the vault is read-only until it reconnects.").assertIsDisplayed()
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellScreenTest*'`
Expected: compile error (`ShellScreen`, `ShellNavigation` unresolved).

- [ ] **Step 3: Write `ShellScreen.kt`**

```kotlin
package net.havenkeys.android.ui.shell

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.AddButton
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.ToastTone
import net.havenkeys.android.ui.kit.rememberToastState
import uniffi.havenkeys_mobile.ItemKind

/** Where the shell leads outside itself; a route carries at most a kind. */
class ShellNavigation(val onSearch: () -> Unit, val onNew: (ItemKind) -> Unit, val onGenerator: () -> Unit)

/**
 * The shell (spec §6.1): the fixed top bar, the current tab's content with
 * its own back stack, the bottom bar, and the add button on Home and Items.
 * The bars live outside the tabs' NavHost, so a tab change leaves them
 * still. The lock replaces this whole destination with Unlock.
 */
@Composable
fun ShellScreen(
    viewModel: ShellViewModel,
    screens: ShellScreens,
    navigation: ShellNavigation,
    modifier: Modifier = Modifier,
    searchPillModifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val tabs = rememberNavController()
    val entry by tabs.currentBackStackEntryAsState()
    val tab = entry?.destination?.tab() ?: Tab.HOME
    var adding by remember { mutableStateOf(false) }
    val toasts = rememberToastState()
    val actions = remember(viewModel, navigation) {
        TopBarActions(onSearch = navigation.onSearch, onSync = viewModel::sync, onLock = viewModel::lock)
    }
    val syncFailed = state.syncError?.let { stringResource(errorText(it)) }
    LaunchedEffect(syncFailed) {
        if (syncFailed != null) {
            toasts.show(syncFailed, ToastTone.Alert)
            viewModel.syncErrorShown()
        }
    }
    // The add button floats on Home and Items (category lists included), not on Settings.
    val addButton: (@Composable () -> Unit)? = if (tab == Tab.SETTINGS) null else {
        { AddButton(onClick = { adding = true }) }
    }
    HavenScaffold(
        modifier = modifier,
        topBar = { ShellTopBar(state.online, state.syncing, actions, pillModifier = searchPillModifier) },
        bottomBar = { BottomBar(tab, onSelect = tabs::selectTab) },
        floatingButton = addButton,
        toastState = toasts,
    ) { padding ->
        ShellNavHost(tabs, screens, padding)
    }
    if (adding) {
        AddSheet(
            state.addTiles,
            onPick = { tile ->
                adding = false
                val kind = tile.kind
                if (kind == null) navigation.onGenerator() else navigation.onNew(kind)
            },
            onDismiss = { adding = false },
        )
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellScreenTest*'`
Expected: PASS.

- [ ] **Step 5: Run the suite, detekt and lint**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/shell/ShellScreen.kt apps/android/app/src/test/kotlin/net/havenkeys/android/ui/shell/ShellScreenTest.kt
git commit -m "feat(android): the shell: fixed bars, per-tab content, add button and sheet, sync toast"
```

---

### Task 14: The app's NavHost hosts the shell and search

**Files:**
- Rewrite: `ANDROID/ui/nav/HavenNavHost.kt`
- Create: `ANDROID/ui/nav/ShellDestinations.kt`
- Modify: `ANDROID/ui/nav/Routes.kt` (drop `VAULT` and `SETTINGS`; `routeOf(Start.VAULT)` is `SHELL`)
- Test: `ANDROID_TEST/ui/nav/RoutesTest.kt`, `ANDROID_TEST/ui/nav/RootViewModelTest.kt`

**Interfaces:**
- Consumes: everything above. `ItemScreen`, `EditScreen`, `GeneratorScreen`, `DevicesScreen`, `AutofillSetupScreen`, `OnboardingScreen`, `UnlockScreen` and their navigation classes, unchanged.
- Produces: `internal fun shellScreens(container: AppContainer, activity: FragmentActivity, navController: NavHostController, open: OpenItem, sharedTitle: SharedTitle): ShellScreens`. The old `VaultScreen` is no longer referenced (Task 15 deletes it).

- [ ] **Step 1: Update the route tests (they fail to compile)**

In `ANDROID_TEST/ui/nav/RoutesTest.kt` and `ANDROID_TEST/ui/nav/RootViewModelTest.kt`, replace every `Routes.VAULT` with `Routes.SHELL`. In `RoutesTest.aRestoredVaultScreenGivesWayToUnlockWhenLocked`, replace `Routes.SETTINGS` with `Routes.SEARCH`, rename the test `aRestoredShellScreenGivesWayToUnlockWhenLocked`, and add:

```kotlin
    @Test
    fun aRestoredSearchGivesWayToUnlock() {
        assertEquals(Routes.UNLOCK, routeToForce(Routes.SEARCH, Start.UNLOCK))
        assertEquals(Routes.ONBOARDING, routeToForce(Routes.SEARCH, Start.ONBOARDING))
    }

    @Test
    fun theUnlockedVaultOpensTheShellAndSearchTakesNoArgument() {
        assertEquals(Routes.SHELL, routeOf(Start.VAULT))
        assertEquals("search", Routes.SEARCH)
        assertEquals("shell", Routes.SHELL)
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*RoutesTest*' --tests '*RootViewModelTest*'`
Expected: FAIL (`routeOf(Start.VAULT)` is still `"vault"`).

- [ ] **Step 3: Update `Routes.kt`**

Replace the object and `routeOf` with:

```kotlin
/**
 * The app's routes. An argument is never a secret: `item/{id}` and
 * `edit/{id}` carry only the item's UUID, `new/{kind}` only a kind;
 * `search` takes none (the query lives only in its ViewModel). The tabs
 * and category lists are routes of the shell's own NavHost (ShellRoutes).
 */
object Routes {
    const val ONBOARDING = "onboarding"
    const val UNLOCK = "unlock"

    /** The shell: top bar, the tabs, bottom bar (spec §6.1). */
    const val SHELL = "shell"

    /** Search takes no argument: the query lives only in its ViewModel. */
    const val SEARCH = "search"
    const val ITEM_ID = "id"
    const val ITEM = "item/{$ITEM_ID}"
    const val GENERATOR = "generator"
    const val DEVICES = "devices"
    const val AUTOFILL_SETUP = "autofill-setup"
    const val EDIT = "edit/{$ITEM_ID}"
    const val KIND = "kind"
    const val NEW = "new/{$KIND}"

    fun item(id: String) = "item/$id"
    fun edit(id: String) = "edit/$id"
    fun new(kind: ItemKind) = "new/${kindArg(kind)}"
}
```

and in `routeOf`, `Start.VAULT -> Routes.SHELL`. Leave `kindArg`, `creatableKind` and `routeToForce` as they are.

- [ ] **Step 4: Write `ShellDestinations.kt`**

```kotlin
package net.havenkeys.android.ui.nav

import androidx.compose.runtime.getValue
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavHostController
import net.havenkeys.android.AppContainer
import net.havenkeys.android.ui.home.HomeScreen
import net.havenkeys.android.ui.home.HomeViewModel
import net.havenkeys.android.ui.items.CategoryScreen
import net.havenkeys.android.ui.items.ItemListViewModel
import net.havenkeys.android.ui.items.ItemsScreen
import net.havenkeys.android.ui.settings.SettingsNavigation
import net.havenkeys.android.ui.settings.SettingsScreen
import net.havenkeys.android.ui.settings.SettingsViewModel
import net.havenkeys.android.ui.settings.rememberSettingsActions
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.ShellScreens

/**
 * The shell's tab screens. Each `viewModel { }` runs inside its tab's back
 * stack entry, so a ViewModel lives as long as that screen's place in its
 * tab, survives tab switches (saved state), and goes with the shell on lock.
 */
internal fun shellScreens(
    container: AppContainer,
    activity: FragmentActivity,
    navController: NavHostController,
    open: OpenItem,
    sharedTitle: SharedTitle,
): ShellScreens = ShellScreens(
    home = { padding ->
        HomeScreen(
            viewModel = viewModel {
                HomeViewModel(container.vaultRepository, container.accountRepository, container.events)
            },
            onOpen = open,
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    items = { padding, onCategory ->
        ItemsScreen(
            viewModel = viewModel {
                ItemListViewModel(container.vaultRepository, container.accountRepository, container.events)
            },
            onCategory = onCategory,
            contentPadding = padding,
        )
    },
    category = { padding, category, onBack ->
        CategoryScreen(
            viewModel = viewModel {
                ItemListViewModel(container.vaultRepository, container.accountRepository, container.events)
            },
            category = category,
            onOpen = open,
            onBack = onBack,
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    settings = { padding ->
        val online by container.events.online.collectAsStateWithLifecycle()
        SettingsScreen(
            viewModel = viewModel {
                SettingsViewModel(
                    container.settingsRepository,
                    container.accountRepository,
                    container.vaultRepository,
                    biometricEnrolled = container::hasBiometricUnlock,
                )
            },
            online = online,
            actions = rememberSettingsActions(container, activity),
            navigation = SettingsNavigation(
                onDevices = { navController.navigate(Routes.DEVICES) },
                onAutofillSetup = { navController.navigate(Routes.AUTOFILL_SETUP) },
            ),
            contentPadding = padding,
        )
    },
)
```

- [ ] **Step 5: Rewrite `HavenNavHost.kt`**

```kotlin
package net.havenkeys.android.ui.nav

import androidx.activity.compose.LocalActivity
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavGraphBuilder
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.ui.autofillsetup.AutofillSetupScreen
import net.havenkeys.android.ui.edit.EditNavigation
import net.havenkeys.android.ui.edit.EditScreen
import net.havenkeys.android.ui.edit.EditTarget
import net.havenkeys.android.ui.edit.EditViewModel
import net.havenkeys.android.ui.generator.GeneratorScreen
import net.havenkeys.android.ui.generator.GeneratorViewModel
import net.havenkeys.android.ui.item.ItemNavigation
import net.havenkeys.android.ui.item.ItemScreen
import net.havenkeys.android.ui.item.ItemViewModel
import net.havenkeys.android.ui.onboarding.OnboardingScreen
import net.havenkeys.android.ui.onboarding.OnboardingViewModel
import net.havenkeys.android.ui.search.SearchScreen
import net.havenkeys.android.ui.search.SearchViewModel
import net.havenkeys.android.ui.settings.DevicesScreen
import net.havenkeys.android.ui.settings.DevicesViewModel
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.ShellNavigation
import net.havenkeys.android.ui.shell.ShellScreen
import net.havenkeys.android.ui.shell.ShellViewModel
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel

/** What every destination of the app's graph needs. */
private class Nav(
    val container: AppContainer,
    val controller: NavHostController,
    val activity: FragmentActivity,
    val shared: SharedTransitionScope,
    val motion: HavenMotion,
    val travel: TitleTravel,
) {
    /** Opens an item over the shell; its title travels from the tapped row. */
    val open: OpenItem = { id, origin ->
        travel.tap(id, origin)
        controller.navigate(Routes.item(id))
    }

    val back: () -> Unit = { controller.popBackStack() }
    val lock: () -> Unit = container.vaultRepository::lock
}

/**
 * The app's navigation: onboarding and unlock outside the shell; the shell
 * and search; and the full-screen screens over the shell (spec §6.1). A lock
 * replaces the whole back stack with Unlock, at once.
 */
@Composable
fun HavenNavHost(container: AppContainer, modifier: Modifier = Modifier) {
    val activity = requireNotNull(LocalActivity.current as? FragmentActivity)
    val root = viewModel { RootViewModel(container.vaultRepository, container.events) }
    val start by root.start.collectAsStateWithLifecycle()
    val first = start
    if (first == null) {
        Box(modifier.fillMaxSize().background(HavenTheme.colors.pane))
        return
    }

    val navController = rememberNavController()
    val scope = rememberCoroutineScope()
    val motion = HavenTheme.motion
    val travel = remember { TitleTravel() }
    // The window's ground shows through a screen that fades under a push, which reads as dimmed.
    SharedTransitionLayout(modifier.background(HavenTheme.colors.pane)) {
        val nav = Nav(container, navController, activity, this, motion, travel)
        NavHost(
            navController = navController,
            startDestination = routeOf(first),
            enterTransition = {
                enterFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = false)
            },
            exitTransition = {
                exitFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = false)
            },
            popEnterTransition = {
                enterFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = true)
            },
            popExitTransition = {
                exitFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = true)
            },
        ) {
            entryScreens(nav, root, scope)
            shellAndSearch(nav)
            itemScreens(nav)
            toolScreens(nav)
        }
    }

    // A back stack restored from before a process kill must not outlive the
    // lock. Decided from Rust's state now, not `first`: after a rotation the
    // vault may have been unlocked or created since the start was read.
    LaunchedEffect(navController) {
        routeToForce(navController.currentDestination?.route, root.current())?.let(navController::replaceAll)
    }
    // The lock wipe: no screen that showed vault data stays in the back stack.
    LaunchedEffect(navController) {
        root.lockedSignal.collect { target ->
            val route = routeOf(target)
            if (navController.currentDestination?.route != route) navController.replaceAll(route)
        }
    }
}

private fun NavHostController.replaceAll(route: String) =
    navigate(route) { popUpTo(graph.id) { inclusive = true } }

/** Onboarding and unlock, outside the shell. */
private fun NavGraphBuilder.entryScreens(nav: Nav, root: RootViewModel, scope: CoroutineScope) {
    val container = nav.container
    composable(Routes.ONBOARDING) {
        OnboardingScreen(
            viewModel = viewModel { OnboardingViewModel(container.accountRepository) },
            onDone = { scope.launch { nav.controller.replaceAll(routeOf(root.current())) } },
        )
    }
    composable(Routes.UNLOCK) {
        UnlockScreen(
            viewModel = viewModel {
                UnlockViewModel(
                    container.vaultRepository,
                    biometricAvailable = container.biometricGate.available(nav.activity),
                    hasBundle = container::hasBiometricUnlock,
                    deleteBundle = container::forgetBiometricUnlock,
                )
            },
            activity = nav.activity,
            container = container,
            onUnlocked = { nav.controller.replaceAll(Routes.SHELL) },
        )
    }
}

/** The shell and search; the pill and the field are one shared element. */
private fun NavGraphBuilder.shellAndSearch(nav: Nav) {
    val container = nav.container
    composable(Routes.SHELL) {
        ShellScreen(
            viewModel = viewModel {
                ShellViewModel(container.vaultRepository, container.accountRepository, container.events)
            },
            screens = shellScreens(
                container,
                nav.activity,
                nav.controller,
                nav.open,
                nav.travel.from(nav.shared, this, nav.motion),
            ),
            navigation = ShellNavigation(
                onSearch = { nav.controller.navigate(Routes.SEARCH) },
                onNew = { kind -> nav.controller.navigate(Routes.new(kind)) },
                onGenerator = { nav.controller.navigate(Routes.GENERATOR) },
            ),
            searchPillModifier = Modifier.sharedIfMoving(nav.shared, SEARCH_KEY, this, nav.motion),
        )
    }
    composable(Routes.SEARCH) {
        SearchScreen(
            viewModel = viewModel { SearchViewModel(container.vaultRepository, container.events) },
            onOpen = nav.open,
            onCancel = nav.back,
            fieldModifier = Modifier.sharedIfMoving(
                nav.shared,
                SEARCH_KEY,
                this,
                nav.motion,
                SharedTransitionScope.ResizeMode.RemeasureToBounds,
            ),
            sharedTitle = nav.travel.from(nav.shared, this, nav.motion),
        )
    }
}

/** An item and its editors, over the shell. */
private fun NavGraphBuilder.itemScreens(nav: Nav) {
    val container = nav.container
    composable(Routes.ITEM, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
        val id = requireNotNull(it.arguments?.getString(Routes.ITEM_ID))
        val online by container.events.online.collectAsStateWithLifecycle()
        ItemScreen(
            viewModel = viewModel {
                ItemViewModel(container.vaultRepository, container.settingsRepository, container.events, id)
            },
            clipboard = container.clipboard,
            online = online,
            navigation = ItemNavigation(
                onBack = nav.back,
                onLock = nav.lock,
                onEdit = { nav.controller.navigate(Routes.edit(id)) },
                onDeleted = nav.back,
            ),
            titleModifier = Modifier.sharedIfMoving(nav.shared, titleKey(id), this, nav.motion),
        )
    }
    composable(Routes.EDIT, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
        val id = requireNotNull(it.arguments?.getString(Routes.ITEM_ID))
        EditRoute(nav, EditTarget.Existing(id))
    }
    composable(Routes.NEW, arguments = listOf(navArgument(Routes.KIND) { type = NavType.StringType })) {
        val kind = it.arguments?.getString(Routes.KIND)?.let(::creatableKind)
        if (kind == null) {
            LaunchedEffect(Unit) { nav.controller.popBackStack() }
        } else {
            EditRoute(nav, EditTarget.New(kind))
        }
    }
}

@Composable
private fun EditRoute(nav: Nav, target: EditTarget) {
    val container = nav.container
    val online by container.events.online.collectAsStateWithLifecycle()
    EditScreen(
        viewModel = viewModel {
            EditViewModel(container.vaultRepository, container.accountRepository, container.events, target)
        },
        isNew = target is EditTarget.New,
        online = online,
        navigation = EditNavigation(
            onDone = { id ->
                if (target is EditTarget.New) {
                    // The new item's screen replaces the editor, so Back goes to where the add began.
                    nav.controller.navigate(Routes.item(id)) { popUpTo(Routes.NEW) { inclusive = true } }
                } else {
                    nav.controller.popBackStack()
                }
            },
            onBack = nav.back,
            onLock = nav.lock,
        ),
    )
}

/** The generator, and the screens Settings leads to. No route carries an argument. */
private fun NavGraphBuilder.toolScreens(nav: Nav) {
    val container = nav.container
    composable(Routes.GENERATOR) {
        val online by container.events.online.collectAsStateWithLifecycle()
        GeneratorScreen(
            viewModel = viewModel { GeneratorViewModel(container.vaultRepository, container.settingsRepository) },
            clipboard = container.clipboard,
            online = online,
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.DEVICES) {
        val online by container.events.online.collectAsStateWithLifecycle()
        DevicesScreen(
            viewModel = viewModel { DevicesViewModel(container.accountRepository, container.events) },
            online = online,
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.AUTOFILL_SETUP) {
        val online by container.events.online.collectAsStateWithLifecycle()
        AutofillSetupScreen(online = online, onBack = nav.back, onLock = nav.lock)
    }
}
```

The `Routes.SETTINGS` destination from Task 12 and the `vaultScreens` / `sharedTitle` / `spec` functions are gone: Settings is a tab now, and every transition comes from `NavMotion.kt`.

- [ ] **Step 6: Run the route tests**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*RoutesTest*' --tests '*RootViewModelTest*' --tests '*NavMotionTest*'`
Expected: PASS.

- [ ] **Step 7: Run everything and build both variants**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease`
Expected: all pass. `grep -rn "VaultScreen\|Routes.VAULT\|Routes.SETTINGS" apps/android/app/src/main/kotlin --include=*.kt` lists only files under `ui/vault/` (deleted in Task 15).

- [ ] **Step 8: Commit**

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui/nav apps/android/app/src/test/kotlin/net/havenkeys/android/ui/nav
git commit -m "feat(android): the app opens on the shell; search and the full-screen screens over it"
```

---

### Task 15: Remove the old vault screen; keep Material out of the new screens

**Files:**
- Delete: `ANDROID/ui/vault/VaultScreen.kt`, `ANDROID/ui/vault/VaultViewModel.kt`, `ANDROID/ui/components/ItemRow.kt`, `ANDROID_TEST/ui/vault/VaultViewModelTest.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml` (drop the strings only the old screen used)
- Modify: `apps/android/app/build.gradle.kts` (`forbidMaterialInKit`)

**Interfaces:**
- Consumes: nothing new. Produces: the Gradle check covering `ui/shell`, `ui/home`, `ui/items`, `ui/search`, `ui/nav` and the Settings tab files.

- [ ] **Step 1: Show the check does not yet cover the new screens**

Temporarily add `import androidx.compose.material3.Text` as the last import of `ANDROID/ui/home/HomeScreen.kt`.
Run: `cd apps/android && ./gradlew forbidMaterialInKit`
Expected: BUILD SUCCESSFUL (the check misses it). Keep the line for Step 3.

- [ ] **Step 2: Widen the check**

In `apps/android/app/build.gradle.kts`, replace the `forbidMaterialInKit` registration's `description` and `include(...)` with:

```kotlin
    description = "Fails on a Material import in the kit, the theme (but MaterialBridge.kt), the catalogue, " +
        "or the screens built from the kit (shell, Home, Items, search, navigation, Settings tab)."
    val sources = fileTree("src") {
        include(
            "**/ui/kit/**/*.kt",
            "**/ui/theme/**/*.kt",
            "**/catalogue/**/*.kt",
            "**/ui/shell/**/*.kt",
            "**/ui/home/**/*.kt",
            "**/ui/items/**/*.kt",
            "**/ui/search/**/*.kt",
            "**/ui/nav/**/*.kt",
            "**/ui/settings/SettingsScreen.kt",
            "**/ui/settings/SettingsChoices.kt",
            "**/ui/settings/SettingsDialog.kt",
            "**/ui/settings/SettingsActions.kt",
        )
        exclude("**/ui/theme/MaterialBridge.kt")
    }
```

(Keep the rest of the task as it is; the message may say "Material is not used in screens built from the HavenKeys kit".)

- [ ] **Step 3: Verify it now catches the import, then remove it**

Run: `cd apps/android && ./gradlew forbidMaterialInKit`
Expected: FAIL naming `ui/home/HomeScreen.kt`. Remove the temporary import line and run it again: BUILD SUCCESSFUL.

- [ ] **Step 4: Delete the old screen**

```bash
git rm apps/android/app/src/main/kotlin/net/havenkeys/android/ui/vault/VaultScreen.kt \
  apps/android/app/src/main/kotlin/net/havenkeys/android/ui/vault/VaultViewModel.kt \
  apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components/ItemRow.kt \
  apps/android/app/src/test/kotlin/net/havenkeys/android/ui/vault/VaultViewModelTest.kt
```

- [ ] **Step 5: Drop the strings only the old screen used**

Remove these six entries from both `values/strings.xml` and `values-pt-rBR/strings.xml`: `vault_title`, `vault_search`, `vault_filter_identities`, `vault_offline_banner`, `vault_empty`, `vault_new_item`. Check that nothing else uses them and that every kept `vault_*` string is still used:

```bash
cd apps/android/app/src/main
for name in vault_title vault_search vault_filter_identities vault_offline_banner vault_empty vault_new_item; do
  grep -rn "R.string.$name\b" kotlin && echo "STILL USED: $name"
done
for name in $(grep -o 'name="vault_[a-z_]*"' res/values/strings.xml | cut -d'"' -f2); do
  grep -rqn "R.string.$name\b" kotlin || echo "UNUSED: $name"
done
```

Expected: no output.

- [ ] **Step 6: Run everything**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease`
Expected: all pass. `grep -rln "androidx.compose.material" apps/android/app/src/main/kotlin/net/havenkeys/android/ui/{shell,home,items,search,nav}` prints nothing.

- [ ] **Step 7: Commit**

```bash
git add -A apps/android/app/src/main/kotlin/net/havenkeys/android/ui/vault apps/android/app/src/main/kotlin/net/havenkeys/android/ui/components/ItemRow.kt apps/android/app/src/test/kotlin/net/havenkeys/android/ui/vault apps/android/app/build.gradle.kts apps/android/app/src/main/res/values/strings.xml apps/android/app/src/main/res/values-pt-rBR/strings.xml
git commit -m "refactor(android): remove the old vault screen and its menu; no Material in the new screens"
```

---

### Task 16: Screenshots of the new screens and the `/impeccable` review

The controller runs this task (it needs `/impeccable`). Fixes that come out of it are separate commits, each with a test where the fix has behaviour.

**Files:**
- Create: `apps/android/app/src/testDebug/kotlin/net/havenkeys/android/screens/ShellScreenshots.kt`
- Create: `apps/android/.impeccable/review/shell-*.png` (written by the test)
- Modify: new-screen files, as the review requires; `apps/android/DESIGN.md` (a "Review notes (stage 3)" section)

- [ ] **Step 1: Write the screenshot test**

It reuses stage 2's mechanism (`CatalogueScreenshots.kt`: Robolectric native graphics, the system property `havenkeys.screens.dir` that `-PscreensDir` sets, `decorView.draw` into a bitmap). If the property is wired only for `testDebug` sources, this file lives there, as below.

```kotlin
package net.havenkeys.android.screens

import android.graphics.Bitmap
import android.graphics.Canvas
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import java.io.File
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.home.HomeScreen
import net.havenkeys.android.ui.home.HomeViewModel
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.items.CategoryScreen
import net.havenkeys.android.ui.items.ItemListViewModel
import net.havenkeys.android.ui.items.ItemsScreen
import net.havenkeys.android.ui.kit.SheetSurface
import net.havenkeys.android.ui.search.SearchScreen
import net.havenkeys.android.ui.search.SearchViewModel
import net.havenkeys.android.ui.settings.SettingsActions
import net.havenkeys.android.ui.settings.SettingsNavigation
import net.havenkeys.android.ui.settings.SettingsScreen
import net.havenkeys.android.ui.settings.SettingsViewModel
import net.havenkeys.android.ui.shell.AddTiles
import net.havenkeys.android.ui.shell.ShellNavigation
import net.havenkeys.android.ui.shell.ShellScreen
import net.havenkeys.android.ui.shell.ShellScreens
import net.havenkeys.android.ui.shell.ShellViewModel
import net.havenkeys.android.ui.shell.tilesFor
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

/**
 * Renders the stage 3 screens with fake data for design review
 * (apps/android/.impeccable/review). Runs only when the build passes a
 * directory: `./gradlew testGithubDebugUnitTest --tests '*ShellScreenshots*' -PscreensDir=.impeccable/review`.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w411dp-h891dp-xxhdpi")
class ShellScreenshots {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val dir: File? = System.getProperty("havenkeys.screens.dir")?.takeIf { it.isNotBlank() }?.let(::File)

    private fun summary(id: String, kind: ItemKind, title: String, sub: String? = null, totp: Boolean = false, passkey: Boolean = false) =
        ItemSummary(id, kind, title, sub, null, totp, passkey, 0, 0)

    private val identity = summary("9", ItemKind.IDENTITY, "Sam Rocha")
    private val items = listOf(
        summary("1", ItemKind.LOGIN, "GitHub", "sam@example.com", totp = true, passkey = true),
        summary("2", ItemKind.LOGIN, "Banco do Brasil", "sam.rocha"),
        summary("3", ItemKind.SECURE_NOTE, "Wi-Fi at home"),
        summary("4", ItemKind.CARD, "Visa", "•••• 4242"),
        summary("5", ItemKind.LOGIN, "Amazon", "sam@example.com"),
        identity,
    )
    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply {
        this.items = Outcome.Ok(this@ShellScreenshots.items)
        recent = Outcome.Ok(this@ShellScreenshots.items.take(4))
        frequent = Outcome.Ok(listOf(this@ShellScreenshots.items[0], this@ShellScreenshots.items[4]))
        view = Outcome.Ok(
            ItemView(identity, listOf("first_name", "email", "city", "cpf").map { ViewField("identity.$it", it, FieldKind.TEXT, null) }),
        )
        searches += listOf("bank", "github")
    }

    private val home: @Composable (PaddingValues) -> Unit = { padding ->
        HomeScreen(remember { HomeViewModel(vault, accounts, events) }, onOpen = { _, _ -> }, contentPadding = padding)
    }

    private val screens: List<Pair<String, @Composable () -> Unit>> = listOf(
        "home" to {
            ShellScreen(
                remember { ShellViewModel(vault, accounts, events) },
                ShellScreens(
                    home = home,
                    items = { p, open -> ItemsScreen(remember { ItemListViewModel(vault, accounts, events) }, open, p) },
                    category = { p, c, back -> CategoryScreen(remember { ItemListViewModel(vault, accounts, events) }, c, { _, _ -> }, back, p) },
                    settings = { p ->
                        SettingsScreen(
                            remember { SettingsViewModel(FakeSettingsRepository(), accounts, vault, biometricEnrolled = { true }) },
                            online = false,
                            actions = SettingsActions(true, {}, { Outcome.Ok(Unit) }),
                            navigation = SettingsNavigation({}, {}),
                            contentPadding = p,
                        )
                    },
                ),
                ShellNavigation({}, {}, {}),
            )
        },
        "items" to { ItemsScreen(remember { ItemListViewModel(vault, accounts, events) }, {}, PaddingValues()) },
        "category" to {
            CategoryScreen(remember { ItemListViewModel(vault, accounts, events) }, Category.LOGINS, { _, _ -> }, {}, PaddingValues())
        },
        "search" to { SearchScreen(remember { SearchViewModel(vault, events) }, onOpen = { _, _ -> }, onCancel = {}) },
        "settings" to {
            SettingsScreen(
                remember { SettingsViewModel(FakeSettingsRepository(), accounts, vault, biometricEnrolled = { false }) },
                online = true,
                actions = SettingsActions(true, {}, { Outcome.Ok(Unit) }),
                navigation = SettingsNavigation({}, {}),
                contentPadding = PaddingValues(),
            )
        },
        "add-offline" to {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                SheetSurface("New item") { AddTiles(tilesFor(online = false), onPick = {}) }
            }
        },
    )

    private fun save(name: String) {
        rule.waitForIdle()
        val view = rule.activity.window.decorView
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        view.draw(Canvas(bitmap))
        val out = File(dir, "$name.png")
        out.parentFile?.mkdirs()
        out.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    @Test
    fun everyNewScreenInBothThemes() {
        assumeTrue(dir != null)
        var dark by mutableStateOf(false)
        var index by mutableIntStateOf(0)
        rule.setContent {
            HavenTheme(darkTheme = dark) {
                Box(Modifier.fillMaxSize().background(HavenTheme.colors.pane)) { screens[index].second() }
            }
        }
        for (theme in listOf(false, true)) {
            screens.forEachIndexed { i, (name, _) ->
                dark = theme
                index = i
                rule.mainClock.advanceTimeBy(2_000)
                save("shell-${if (theme) "dark" else "light"}-$name")
            }
        }
    }
}
```

(Long lines in this file follow the Global Constraints' wrapping rule. Each screen `remember`s its own ViewModel, so swapping `index` builds the next screen fresh.)

- [ ] **Step 2: Render**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ShellScreenshots*' -PscreensDir=.impeccable/review`
Expected: PASS, and `ls .impeccable/review/shell-*.png` lists 12 files. Without `-PscreensDir` the test is skipped (and `testGithubDebugUnitTest` stays green).

- [ ] **Step 3: `/impeccable` review**

Invoke `/impeccable` (skill `impeccable:impeccable`) in critique mode with this brief:

> Review the HavenKeys Android shell and new screens (stage 3 of the redesign). Artefacts: `apps/android/.impeccable/review/shell-*.png` (light and dark: the shell with Home, Items, a category list, search with recents, Settings, the add sheet offline) and the sources under `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/{shell,home,items,search,settings}`. Reference: `apps/android/DESIGN.md` (the kit as shipped), `apps/desktop/DESIGN.md`, spec `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §6–7, and 1Password's phone app as the navigation model. Judge: hierarchy and rhythm of Home (identity card, two groups), the top bar's density (pill, offline badge, sync, lock on a 360dp phone), the bottom bar's marker and labels, the add sheet's tiles, the category list's back chevron and large title, Settings' grouping, the One Fitting Rule (brass only on controls and thin lines; one primary per screen), and that nothing reads as Material. Return an ordered list of material fixes.

Apply the fixes the owner would agree with without asking (spacing, sizes, ink, copy inside these screens). Anything that changes a decision recorded in this plan (for example the identity card's summary, or Sync now in the top bar) goes into `apps/android/DESIGN.md` under "Review notes (stage 3)" as an open question instead. Re-render after fixes. Commit each fix:

```bash
git add apps/android/app/src/main/kotlin/net/havenkeys/android/ui apps/android/app/src/test
git commit -m "fix(android): <what the review changed, in the screen's terms>"
```

- [ ] **Step 4: Record and commit**

Add a "Review notes (stage 3)" section to `apps/android/DESIGN.md` (findings, what was fixed, what stays open) and a short "Shell" section describing the top bar, bottom bar, add sheet and lazy inset groups as they shipped.

```bash
git add apps/android/app/src/testDebug/kotlin/net/havenkeys/android/screens apps/android/.impeccable/review/shell-*.png apps/android/DESIGN.md
git commit -m "docs(android): stage 3 screens rendered and reviewed against the design system"
```

---

### Task 17: Documentation, the owner's device checks, closing the stage

**Files:**
- Modify: `docs/android.md` (status paragraph; a "Redesign stage 3" checklist)
- Modify: `docs/security-model.md` (§22.8 wording; §22.19 one paragraph)
- Modify: `docs/superpowers/plans/2026-10-03-android-redesign-index.md` (stage 3 row; stage 4 bullets)

- [ ] **Step 1: `docs/security-model.md`**

In §22.8, replace "typing in the vault search" with "typing in the search screen". At the end of §22.19, add:

```markdown
On Android the Home and search screens hold this data only in their
ViewModels' memory and drop it on lock, sign-out and removal. The query
being typed lives in the search screen's ViewModel and its field's
composition state; it is never put into a navigation route, saved instance
state, `SavedStateHandle` or a log, and it becomes a recent search only
when a result is opened.
```

- [ ] **Step 2: `docs/android.md`**

In the status paragraph, after the Android M4 sentence, add: "The redesign's shell (2026-10-03 spec §6) replaces the vault list: Home, Items and Settings tabs with their own back stacks, a search screen with recent searches, and an add sheet." Then add before "### Android M4" in "Manual checklist":

```markdown
### Redesign stage 3 (shell)

Run on an emulator or phone (Android 14+), in light and dark, once with
`adb shell settings put global animator_duration_scale 1` and once with `0`
(restore `1` afterwards).

- [ ] Unlock: Home appears on the slower reveal; the identity card and both groups settle in sequence; nothing settles again on a tab return. With animations removed, it is an instant cut.
- [ ] Tabs: Items → Logins → Home → Items shows Logins again; tapping Items again shows the Items root; Back from Items or Settings root goes to Home; the top bar does not move or flicker on a tab change; a light tick on a change, none on a reselect.
- [ ] Push and pop: a category list and an item slide in from the right while the old screen shifts left and dims; Back reverses; the predictive back gesture scrubs the item screen (Android 14+). A row tapped during a tab crossfade or a push opens at once.
- [ ] Shared title: an item in both Recently added and Frequently used: tapping either row moves that row's title into the item screen.
- [ ] Search: the pill grows into the focused field with the keyboard up; recent searches fade in after; Clear empties them; typing shows results and records nothing; opening a result records the query (it is at the top of recents next time); Cancel and Back shrink the field into the pill; Back from an opened result returns to the results.
- [ ] Search and the lock: with a query typed, lock (button, screen off, auto-lock): Unlock appears at once; after unlocking the query and results are gone. Kill the process with a query typed (`adb shell am kill net.havenkeys.android` after Home): the query never comes back.
- [ ] Add sheet: springs up, backdrop dims, tiles stagger; drag down and a backdrop tap close it; Login, Secure note and Card open their editors; Generate password opens the generator. Airplane mode: the item tiles are dimmed with "Adding needs a connection"; the generator still opens. The identity is never offered.
- [ ] Offline: the top bar shows "Offline"; Sync now shows a ring, then a toast that HavenKeys is offline.
- [ ] Pull to refresh on Home and a category list syncs; Sync now in the top bar does the same.
- [ ] Settings: every row works (auto-lock and clipboard sheets, the three switches, biometric enrolment dialog, autofill setup, devices, sign out, remove device with a wrong then right email).
- [ ] FLAG_SECURE: screenshots blocked and the recents thumbnail blank on Home, search, the add sheet and the Settings dialogs.
- [ ] TalkBack: tabs read "Home, tab, 1 of 3, selected"; the pill reads "Search HavenKeys, button"; Sync now and Lock now are named; the offline badge is read; the add sheet's dimmed tiles read as disabled; Home's headings are headings; the identity card reads its title and summary as one button; the category back chevron reads "Back"; Sync now is the reachable way to sync (the pull's custom action may not be).
- [ ] Stage 1's deferred checks: pick a direct-fill row in Chrome, open another form, then see that login under Frequently used; fill event history still delivers `TYPE_DATASET_SELECTED` on Android 14+ (`FillEventHistory` is deprecated in API 36 with no replacement); a pick after a null response is counted once; a confirmed login fill counts once.
```

- [ ] **Step 3: The plan index**

In `docs/superpowers/plans/2026-10-03-android-redesign-index.md`, set the stage 3 row's status to `Done (\`<first commit>..<last commit>\`)` with this stage's real commit range (`git log --oneline` shows it). Do not edit the stage 2 row. Under "### Stage 4", add:

```markdown
- Settings' dialogs are already kit (`HavenDialog` with content); Devices'
  revoke dialog and the item delete dialog follow the same pattern.
- The item screen's header becomes a kit `ItemTile` + title, and the
  monogram tile joins the shared title (`TitleTravel`, `titleKey`) in the
  row-to-detail move (spec §7).
- `ui/components/HavenTopBar.kt` goes once no full-screen screen uses it.
- Open questions from `apps/android/DESIGN.md` "Review notes (stage 3)".
```

- [ ] **Step 4: Final verification**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add docs/android.md docs/security-model.md docs/superpowers/plans/2026-10-03-android-redesign-index.md
git commit -m "docs: Android shell device checks, search query handling, stage 3 done"
```

- [ ] **Step 6: Hand the device checks to the owner**

Report the "Redesign stage 3 (shell)" checklist from `docs/android.md` to the owner as the remaining work of this stage: nothing in it has run on a device.

---

## Stage exit check

- [ ] `cd apps/android && ./gradlew testGithubDebugUnitTest detekt lintGithubDebug assembleGithubDebug assembleGithubRelease` passes.
- [ ] `grep -rln "androidx.compose.material" apps/android/app/src/main/kotlin/net/havenkeys/android/ui/{shell,home,items,search,nav}` prints nothing; `forbidMaterialInKit` covers those directories and the Settings tab files.
- [ ] `grep -rn "rememberSaveable\|rememberTextFieldState\|SavedStateHandle" apps/android/app/src/main/kotlin/net/havenkeys/android/ui/{search,home,items,shell}` prints nothing.
- [ ] `grep -rn "navigate(" apps/android/app/src/main/kotlin/net/havenkeys/android/ui` shows only `Routes.*`, `ShellRoutes.category(...)` and tab graph routes: no query in any route.
- [ ] `ui/vault/` and `ui/components/ItemRow.kt` no longer exist.
- [ ] ViewModel tests exist for Home, Items lists, search and the shell (add tiles), each with a lock-wipe case.
- [ ] The owner has the device checklist (Task 17, Step 6).
