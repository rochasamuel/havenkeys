# Android app redesign — Design

Status: proposed, 2026-10-03.
Amends `docs/superpowers/specs/2026-10-01-android-app-design.md` §9.1–9.2
(screen list, Material 3) and adds per-device activity data to the Rust core
(§4 below), which `CLAUDE.md` gets an amendment note for.

> This software has not undergone an independent security audit.

## 1. Goal

The Android app stops looking like a stock Material app and becomes
recognizably HavenKeys: the desktop app's visual system, adapted to touch,
built with Jetpack Compose's foundation layer only. Its navigation follows
the pattern of 1Password's phone app:

1. A bottom bar with **Home**, **Items** and **Settings**.
2. A search bar fixed at the top of every tab, opening a search screen with
   recent searches.
3. **Home**: the Identity pinned on top, the 6 most recently added items and
   the 6 most used.
4. A floating add button opening a sheet of item types, each leading to its
   form.
5. **Items**: All items or one category, each list on its own screen.
6. Calm, spring-based motion in the manner of Apple's apps.

Success: the author prefers opening HavenKeys on the phone to opening it on
the desktop for a quick lookup, nothing in the app reads as Material, and
every security rule of the Android spec still holds.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| How far "no Material" goes | Own component set on `androidx.compose.foundation`; `material3` and `material-icons-extended` removed | Restyle with Material underneath (its ripples, motion and sheet feel leak through); new screens only (app looks mixed) |
| What counts as using an item | An autofill or passkey fill of it, or copying one of its fields | Also opening the detail (browsing skews the list); autofill only (notes and cards would never appear) |
| Where usage is stored | Rust core, per device, never synced | Synced (a server write on every fill, impossible offline); Kotlin storage (vault-derived data stays in Rust, Android spec §9.3) |
| Recent searches | The last 10, sealed in Rust's per-device storage, kept across locks, clearable | Memory only until lock; none |
| Fonts | Bundle Hanken Grotesk, Source Serif 4, JetBrains Mono (OFL) | System font |
| Build approach | Five stages on the existing app, releasable after each | One rewrite; a parallel UI behind a switch |
| Activity API scope | In `havenkeys-core` on `VaultService`, so the desktop can adopt it later | Mobile crate only |

## 3. Scope

In scope: the Rust activity data (§4), the design system (§5), the new shell
and screens (§6), motion (§7), restyling every existing screen and the
autofill and passkey activities with the new components, and removing
Material (§8).

Out of scope: the desktop using the activity data (only prepared), tablet
layouts, new editor capabilities, iOS.

## 4. Activity data (Rust core)

### 4.1 Storage

A new module `crates/havenkeys-core/src/activity.rs` and a new slot
`LocalSlot::Activity` (name `activity`, its own `Purpose`) in `local.rs`. Like
`DeviceSettings`, it is sealed with the vault key, readable only while
unlocked, never sent to the server, and unreadable by another vault.

```rust
struct Activity {
    version: u32,                       // 1
    uses: BTreeMap<Uuid, Use>,          // per item
    searches: Vec<String>,              // newest first, at most 10
}
struct Use {
    score: f64,      // decayed count, as of `last_ms`
    last_ms: i64,    // Unix milliseconds of the last use
}
```

### 4.2 Rules

- **Decay.** A use adds 1 to the score after decaying the stored score by
  `0.5^((now - last_ms) / 30 days)`. Ranking compares scores decayed to
  "now" the same way.
- **Pruning.** On every write, entries for items that no longer exist and
  entries whose last use is over 365 days old are dropped.
- **Searches.** `record_search` trims the query; an empty one is ignored. A
  query already in the list moves to the top. Queries longer than the core's
  search-query limit are refused. The list keeps 10.
- **Version.** An unknown `version` reads as empty activity (it is a cache
  of convenience, not vault data). The vault is resettable during
  development, so no migration exists.

### 4.3 API on `VaultService`

| Call | Returns / effect |
|---|---|
| `record_use(item_id)` | Adds a use; `NotFound` for an unknown item |
| `frequently_used(n)` | Up to `n` `ItemOverview`s by decayed score, ties by `last_ms`, deleted items skipped |
| `recently_created(n)` | Up to `n` `ItemOverview`s by `created_at`, newest first (no new data) |
| `recent_searches()` | The list, newest first |
| `record_search(query)` | As in §4.2 |
| `clear_recent_searches()` | Empties the list |

All fail with `Locked` while locked. `ItemOverview` already has
`created_at`; the mobile `ItemSummary` gains `created_at: i64`.

### 4.4 Who records uses

- The Rust calls that hand out an item's values for a fill on the phone
  record a use themselves: Android direct fill and confirmed fill, and
  passkey assertions (in `havenkeys-mobile`, calling `record_use`). The
  desktop's bridge fills (browser extension) do not record yet; that comes
  with the desktop adopting this data.
- The app calls `record_use` after a copy (password, username, code, any
  revealed field), through `havenkeys-mobile`.
- Opening or viewing an item records nothing.

`havenkeys-mobile` exposes the six calls through UniFFI. The desktop's Tauri
layer gets nothing in this change.

## 5. Design system

Source of truth: `apps/desktop/DESIGN.md` and `packages/ui` tokens, adapted
to touch, reviewed with `/impeccable` against the desktop before any screen
uses it. Recorded in a new `apps/android/DESIGN.md`.

### 5.1 Theme (`ui/theme`, no Material types)

- **Colors:** one `HavenColors` holding the desktop's named roles (pane,
  list, group, group-line, field, raised, hover, line, line-strong, text,
  text-strong, muted, brass, brass-hi, brass-ink, brass-soft, on-brass, ok,
  danger, glass, digit, symbol, avatar), light and dark, following the
  system setting.
- **Type:** Hanken Grotesk (text), Source Serif 4 (monogram tiles),
  JetBrains Mono (secrets, codes), bundled under `res/font` with their OFL
  notices added to `THIRD-PARTY-NOTICES.md`. The desktop's weights and
  hierarchy, with the reading size raised for a phone: body about 15sp, row
  title 15–16sp, label 13sp, headline 28sp, code 22sp tabular.
- **Shapes:** the desktop radii 8, 10, 12, 14, 18 dp.
- **Motion:** the existing `HavenMotion` plus named springs (§7).
- **Icons:** the desktop icon set (`apps/desktop/src/components/Icon.tsx`
  paths: 24 grid, 1.6 stroke, round caps and joins) ported to a Compose
  `HavenIcon` from the same path data. It replaces
  `material-icons-extended`. Icons the phone needs and the set lacks
  (home, items, settings tabs) are drawn to the same rules and added to the
  desktop set too, so both stay one library.

### 5.2 Components (`ui/kit`)

Each in its own file, with previews (light and dark) and tests, and each
setting its TalkBack role, state and label explicitly, since Material no
longer does.

| Group | Components |
|---|---|
| Structure | `HavenScaffold` (top area, content, bottom bar, insets), `InsetGroup` and `GroupRow`, `ItemRow` (monogram or type icon, title, subtitle, passkey and code marks), `SectionHeader` |
| Inputs | `HavenTextField`, `SecretTextField` (masked, reveal, mono), `HavenSwitch`, `HavenSlider`, `SegmentedControl` |
| Actions | `HavenButton` (primary, secondary, quiet, danger), `HavenIconButton`, `CopyButton` (glyph to check), `Pill`, `AddButton` (the floating button) |
| Overlays | `HavenSheet` (spring, drag to dismiss), `HavenDialog`, `Toast` (glass pill), `HavenMenu` |
| Feedback | `ProgressRing`, `PullToRefresh` |

Rules kept from the desktop: one primary button per screen; brass marks
controls and thin lines, never large surfaces; no ripple anywhere (press is
a scale and a brass-soft tint). Touch targets are at least 48dp.

## 6. Shell and screens

### 6.1 Structure

- **Outside the shell:** onboarding and unlock. A lock replaces the whole
  back stack with unlock, as today.
- **The shell:** a fixed top bar, the current tab's content, the bottom
  bar. Tab roots and category lists live inside it.
- **Over the shell, full screen, bars hidden:** item detail, editors, the
  generator, devices, autofill setup.
- Each tab keeps its own back stack (Navigation Compose with saved and
  restored state per tab). Reselecting the current tab pops it to its
  root. Route arguments stay non-secret: an item UUID, a kind or a
  category, never a query.

### 6.2 Fixed top bar

A search pill ("Search HavenKeys"), the lock button and, when offline, the
offline badge. Identical on every tab and category list; it does not move
or recompose on a tab change.

### 6.3 Search

- Tapping the pill turns it into a full-screen search: the pill becomes
  the field, focused, keyboard up.
- Empty field: "Recent searches" (from §4) with **Clear**; tapping one runs
  it again.
- Typing: live results from Rust's existing `search`, as item rows.
- Opening a result calls `record_search` with the current query. Typing
  alone records nothing.
- Back or Cancel shrinks it back into the pill. Lock wipes the query.

### 6.4 Bottom bar

Home, Items, Settings, with `HavenIcon`s and a brass marker under the
selected tab. Light haptic tick on a change.

### 6.5 Home

1. **Identity card** on top: the identity's title and a one-line summary
   (name, or "Add your details" when empty); opens the identity.
2. **Recently added**: `recently_created(6)`.
3. **Frequently used**: `frequently_used(6)`; empty state "Items you fill or
   copy will show here."

Each list is an `InsetGroup` of `ItemRow`s. Pull to refresh syncs.

### 6.6 Add

`AddButton` floats above the bottom bar on Home and Items. It opens a
`HavenSheet` of tiles: **Login, Secure note, Card, Generate password**.
An item tile opens `new/{kind}`; the generator tile opens the generator.
Offline, the item tiles are dimmed with "Adding needs a connection"; the
generator stays available. The identity is never creatable (one per
account).

### 6.7 Items

An `InsetGroup` with counts: **All items, Logins, Passkeys** (logins with a
passkey), **Secure notes, Cards**. Each opens its own list screen (large
title, A–Z, pull to refresh) inside the shell, top bar kept.

### 6.8 Settings

Today's settings as grouped rows: auto-lock, lock on screen off,
biometrics, Confirm before filling, autofill setup, Digital Asset Links,
devices, account, updates, sign out, remove device.

### 6.9 Existing screens

Item detail, editors, generator, unlock, onboarding, devices, autofill
setup, the autofill picker and "Search HavenKeys…", and the passkey sheets
keep their behaviour and are rebuilt from `ui/kit`. The editor's copy
actions call `record_use` like the detail's.

## 7. Motion

All values live in `HavenMotion`; with "Remove animations" on, every
transition is an instant cut and haptics remain.

| Moment | Motion |
|---|---|
| Push (category list, detail) | New screen slides in from the right, the old one shifts about 30% left and dims; back reverses; predictive back scrubs it |
| Row to detail | Shared title (existing) plus the monogram tile into the detail header |
| Tab change | Short crossfade with a few dp of vertical shift; top bar still |
| Search | Pill grows into the search screen (shared bounds); recents fade in after; Cancel reverses |
| Add sheet | Springs up, backdrop dims, tiles stagger about 30ms; tile press scales; drag down or tap outside closes |
| Unlock to Home | Existing slower reveal; Home groups settle in sequence |
| Press | Scale to about 0.97 with a brass-soft tint, no ripple |
| Copy | Glyph to check, glass toast "Copied · clears in 30 s", light haptic |
| Lock | Instant wipe, no exit animation |

Nothing blocks input while animating; a tap mid-transition goes straight to
its destination.

## 8. Build stages

Each stage leaves the app usable and releasable.

1. **Activity data** (§4): core module, slot, API, mobile exposure,
   `created_at` on `ItemSummary`.
2. **Design system** (§5): theme, fonts, icons, `ui/kit`, a debug-only
   catalogue screen of every component in both themes. No screen uses it
   yet.
3. **Shell and new screens** (§6.1–6.8) with their motion (§7). The old
   vault screen and its overflow menu are removed.
4. **Existing screens** (§6.9) rebuilt from `ui/kit`.
5. **Remove Material:** drop `material3` and `material-icons-extended`; a
   detekt rule forbids `androidx.compose.material` imports.

## 9. Security

- Activity data is sealed in Rust, per device, readable only unlocked,
  never synced, never stored in Kotlin (no DataStore, SharedPreferences or
  Room), and dropped from ViewModels on lock.
- Search queries never go into routes, `SavedStateHandle` or logs.
- Revealed values keep the Android spec's rules (composable state only,
  cleared on leave, after 30 s and on lock).
- `FLAG_SECURE`, autofill exclusion and touch filtering on every activity
  are unchanged.
- What the activity data reveals to someone who can unlock the vault
  (which items are used most, recent queries) is no more than the vault
  itself; to someone who cannot, it is ciphertext.

## 10. Testing

- **Rust:** decay and ranking (including ties), pruning of deleted and
  stale items, the search list (limit, dedup to top, blank ignored, clear),
  `Locked` while locked, another vault's key cannot read the slot,
  `recently_created` order, fill paths recording a use and views not.
- **Kotlin:** ViewModel tests with fake repositories for Home, Items lists,
  search (recents, record on open only, wipe on lock) and the add sheet
  (offline dimming); component tests for `ui/kit` semantics (roles,
  states, labels).
- **Per stage:** unit tests, detekt, Rust tests; an emulator run in light
  and dark with "Remove animations" on and off; a TalkBack pass over new
  components; `/impeccable` on each new screen.

## 11. Documentation

- An amendment note in the Android spec (§9.1–9.2) pointing here.
- An amendment note in `CLAUDE.md` for per-device activity data.
- `apps/android/DESIGN.md` for the phone's design system.
- `docs/security-model.md`: the `Activity` slot in the list of sealed
  local data.
