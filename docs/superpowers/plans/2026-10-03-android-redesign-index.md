# Android Redesign — Plan Index

**Spec:** `docs/superpowers/specs/2026-10-03-android-redesign-design.md`

The spec's five stages run in order and each leaves the app releasable. Each
stage gets its own plan, written when the previous stage is done: later
stages build on component APIs and review findings that only exist once the
earlier stage is built, so planning them now would mean guessing at names
and signatures.

| Stage | Plan | Status | Depends on |
|---|---|---|---|
| 1. Activity data | `2026-10-03-android-redesign-stage1-activity.md` | Done (`cb5c461..576f07c`) | — |
| 2. Design system | `2026-10-03-android-redesign-stage2-design-system.md` | Done (`6a4a2f8..83fbc18`) | Nothing in stage 1 (can start in parallel if wanted) |
| 3. Shell and new screens | `2026-10-03-android-redesign-stage3-shell.md` | Done (`5b72db6..a9def0d`) | Stages 1 and 2 |
| 4. Existing screens rebuilt | `2026-10-03-android-redesign-stage4-screens.md` | Done (`5744144..1a1ff47`) | Stage 2 (and 3 for navigation) |
| 5. Remove Material | `2026-10-03-android-redesign-stage5-remove-material.md` | Planned | Stage 4 |

## What each later plan must cover

### Stage 2: Design system (spec §5)

- `ui/theme` rewritten without Material types: `HavenColors` (all desktop
  roles, light and dark), `HavenTypography` with bundled Hanken Grotesk, Source
  Serif 4, JetBrains Mono under `res/font` (OFL notices in
  `THIRD-PARTY-NOTICES.md`), shapes, `HavenMotion` springs.
- `HavenIcon`: the desktop icon paths (`apps/desktop/src/components/Icon.tsx`)
  ported; new tab icons drawn to the same rules and added to the desktop set.
- Every `ui/kit` component in spec §5.2, each with light and dark previews,
  semantics tests (role, state, label) and a minimum 48dp touch target.
- A debug-only catalogue screen showing every component.
- `/impeccable` review of the catalogue against the desktop before closing;
  `apps/android/DESIGN.md` written from what shipped.
- Entry check: confirm the font licences and file sizes; confirm the
  Compose foundation APIs used for sheets and text fields on the current BOM.

### Stage 3: Shell and new screens (spec §6.1–6.8, §7)

- Builds on the kit as shipped: read `apps/android/DESIGN.md` and the `ui/kit`
  sources for component names and signatures; carry the open questions from
  DESIGN.md's review notes.
- Navigation: shell with per-tab back stacks (save and restore state),
  reselect pops to root, non-secret route arguments only.
- Fixed top bar, search screen (recents, Clear, record on result open only,
  wipe on lock), bottom bar, Home (Identity card, Recently added,
  Frequently used), add button and sheet (offline dimming), Items and
  category lists, Settings as grouped rows.
- The old vault screen and its overflow menu removed.
- Motion from spec §7, all through `HavenMotion`, instant under "Remove
  animations".
- ViewModel tests with fakes for Home, search, Items lists, add sheet.
- Emulator pass, including stage 1's deferred check: pick a direct-fill row
  in Chrome, open another form, then see that login under Frequently used; fill event history still
  delivers TYPE_DATASET_SELECTED on Android 14+ (FillEventHistory is
  deprecated in API 36 with no replacement); a pick after a null response is
  counted once; a confirmed login fill counts once.

### Stage 4: Existing screens (spec §6.9)

- Item detail, editors, generator, unlock, onboarding, devices, autofill
  setup, the autofill picker and "Search HavenKeys…", passkey sheets, all
  from `ui/kit`, behaviour unchanged (existing tests keep passing).
- The editor has no copy actions, so it records no use (stage 4 plan, decision R5).

### Stage 5: Remove Material (spec §8.5)

- Drop `material3` and `material-icons-extended` from
  `gradle/libs.versions.toml` and `app/build.gradle.kts`.
- A detekt rule (or the existing `forbidLogging`-style Gradle check)
  forbidding `androidx.compose.material` imports.
- Final `/impeccable` and TalkBack pass over the whole app.
- What is left: `ui/theme/MaterialBridge.kt` (the whole file: `MaterialBridge`,
  the colour schemes, `MaterialTypography`, `HavenType`, `MaterialShapes`), the
  `MaterialBridge(...)` call in `ui/theme/Theme.kt`, `compose-material3` and
  `compose-icons` in `gradle/libs.versions.toml` and `app/build.gradle.kts`,
  and the `MaterialBridge.kt` exclusion in `forbidMaterialInKit` (which then
  becomes the permanent rule). The platform theme's `android:Theme.Material`
  parent in `res/values*/themes.xml` is the window behind Compose, not
  Compose Material, and stays.
- Open questions from `apps/android/DESIGN.md` "Review notes (stage 4)".
