# Android Redesign, Stage 1: Activity Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record, per device and sealed in Rust, which items the user fills or copies and what they search for, and expose Home's lists ("Recently added", "Frequently used") and recent searches to the Android app, ready for the desktop to adopt later.

**Architecture:** A new `activity` module in `havenkeys-core` keeps one JSON document in a new sealed local slot (`LocalSlot::Activity`), read and written through the existing `read_local` / `write_local`. `havenkeys-mobile` exposes six UniFFI calls and records uses in the three Rust calls that only run after a user pick. Kotlin records the other picks (copies, confirmed autofill, direct-fill rows picked from Android's fill event history). No UI changes in this stage.

**Tech Stack:** Rust (havenkeys-core, havenkeys-mobile, UniFFI 0.x proc-macros, serde_json), Kotlin (Android app, coroutines, JUnit 4), Gradle, cargo-ndk.

**Spec:** `docs/superpowers/specs/2026-10-03-android-redesign-design.md` §4, §9, §10, §11.

## Global Constraints

- Activity data is per device, sealed with the vault key, never synced, never stored in Kotlin (no DataStore, SharedPreferences, Room).
- All activity calls fail with `Locked` while locked.
- Decay half-life 30 days; entries unused for over 365 days are dropped; at most 10 recent searches; search query limit is the core's `MAX_SEARCH_QUERY_CHARS` (256).
- Unknown `version` or an unreadable blob reads as empty activity (a cache, not vault data); no migration code.
- Both lists leave out the account's Identity.
- A use is recorded only where the user picked that item, never where values are fetched ahead of a pick (`autofill_fill`, `autofill_totp`, `autofill_card_values`, `autofill_identity_values` record nothing).
- Recording a use never makes a fill or copy fail: errors from recording are ignored where the user's action is the point.
- `client.vault()` is a `std::sync::MutexGuard` (not re-entrant): never call `record_use` while a guard from the same call is alive.
- Never log, `Debug`-print or put in errors any search query or item title. No `Log`/`println` in Kotlin (detekt).
- Commits without Co-Authored-By lines (owner's rule).

## Review Focus

1. A search query with only spaces, or differing only in letter case from a saved one: expected not stored, and moved to the top without a duplicate respectively. Pinned in Task 4.
2. An item deleted after it was used or searched for: expected gone from "Frequently used" immediately (not only after the next write). Pinned in Task 2 (`frequently_used` filters by live overviews).
3. A clock that jumps backwards (a use "in the future"): expected no panic, no negative decay boost above the stored score. Pinned in Task 2 (`decayed` clamps elapsed at 0).
4. Direct-fill rows prefetching values: expected opening the autofill menu counts nothing until a row is picked. Pinned in Task 6 (Rust fetch calls record nothing) and Task 8 (only `TYPE_DATASET_SELECTED` with an `item:` id counts).
5. A confirmed (authenticated) autofill row: expected counted once, not twice (auth activity plus event history). Pinned in Task 8 (ids only on rows without authentication).

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-core/src/crypto/blob.rs` | Add `Purpose::Activity` (label `activity`) |
| `crates/havenkeys-core/src/local.rs` | Add `LocalSlot::Activity` |
| `crates/havenkeys-core/src/activity.rs` (new) | The `Activity` document, decay, pruning, and the six `VaultService` methods |
| `crates/havenkeys-core/src/lib.rs` | `pub mod activity;` |
| `crates/havenkeys-mobile/src/items.rs` | `ItemSummary.created_at`; `summary` becomes `pub(crate)` |
| `crates/havenkeys-mobile/src/activity.rs` (new) | UniFFI exports and `note_use` |
| `crates/havenkeys-mobile/src/lib.rs` | `mod activity;` |
| `crates/havenkeys-mobile/src/credentials.rs`, `autofill.rs` | `note_use` after a pick |
| `apps/android/.../uniffi/havenkeys_mobile/havenkeys_mobile.kt` | Regenerated bindings |
| `apps/android/.../data/VaultRepository.kt`, `AutofillRepository.kt` | Repository calls |
| `apps/android/.../ui/item/ItemViewModel.kt`, `ItemScreen.kt` | Record a use after a copy |
| `apps/android/.../autofill/DatasetIds.kt` (new) | `item:<uuid>` dataset ids, parsing |
| `apps/android/.../autofill/AutofillRows.kt`, `DatasetFactory.kt`, `WalletDatasets.kt`, `HavenAutofillService.kt`, `AutofillAuthActivity.kt` | Ids on direct rows, event history, confirmed fills |
| Docs | `CLAUDE.md`, `docs/security-model.md`, Android spec amendment note |

Android paths below abbreviate `apps/android/app/src/main/kotlin/net/havenkeys/android` as `ANDROID/` and the test root `apps/android/app/src/test/kotlin/net/havenkeys/android` as `ANDROID_TEST/`.

---

### Task 1: The Activity slot

**Files:**
- Modify: `crates/havenkeys-core/src/crypto/blob.rs:36-60`
- Modify: `crates/havenkeys-core/src/local.rs:1-31` and its tests

**Interfaces:**
- Produces: `LocalSlot::Activity` (store name `"activity"`), `Purpose::Activity` (label `b"activity"`).

- [ ] **Step 1: Write the failing test** (append to `mod tests` in `local.rs`)

```rust
    #[test]
    fn the_activity_slot_is_its_own() {
        let vault = unlocked_vault();
        vault
            .write_local(LocalSlot::Activity, &Prefs { flag: true })
            .unwrap();
        assert_eq!(
            vault.read_local::<Prefs>(LocalSlot::Activity).unwrap(),
            Some(Prefs { flag: true })
        );
        assert_eq!(
            vault.read_local::<Prefs>(LocalSlot::DeviceSettings).unwrap(),
            None
        );
        // Sealed under its own purpose: another slot's blob does not open here.
        let blob = vault.store.local_blob("activity").unwrap().unwrap();
        vault.store.set_local_blob("device_settings", &blob).unwrap();
        assert!(vault.read_local::<Prefs>(LocalSlot::DeviceSettings).is_err());
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p havenkeys-core local::tests::the_activity_slot_is_its_own`
Expected: compile error, `no variant named Activity`.

- [ ] **Step 3: Implement**

In `blob.rs`, add to `enum Purpose` after `AssetLinks`:

```rust
    /// This device's item uses and recent searches (never synced).
    Activity,
```

and to `label()`:

```rust
            Purpose::Activity => b"activity",
```

In `local.rs`, update the module comment's first sentence to "Values that belong to this device and never sync: the Android app's own settings, its Digital Asset Links cache, and item activity (uses, recent searches)." Add the variant and both match arms:

```rust
pub enum LocalSlot {
    DeviceSettings,
    AssetLinks,
    Activity,
}
```

```rust
            Self::Activity => "activity",
```

```rust
            Self::Activity => Purpose::Activity,
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-core local::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/crypto/blob.rs crates/havenkeys-core/src/local.rs
git commit -m "feat(core): a sealed per-device slot for item activity"
```

---

### Task 2: Uses and "Frequently used"

**Files:**
- Create: `crates/havenkeys-core/src/activity.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `pub mod activity;` after `pub mod account;` in alphabetical order)

**Interfaces:**
- Consumes: `LocalSlot::Activity`; `VaultService::{read_local, write_local, session}`; `Session.overviews: HashMap<Uuid, ItemOverview>`; `VaultService::identity_item_id() -> Result<Uuid>`.
- Produces:
  - `pub fn record_use(&self, id: &Uuid, now_ms: i64) -> Result<()>`
  - `pub fn frequently_used(&self, n: usize, now_ms: i64) -> Result<Vec<ItemOverview>>`
  - constants `HALF_LIFE_MS`, `FORGET_AFTER_MS`, `MAX_RECENT_SEARCHES`.

- [ ] **Step 1: Write the failing tests** (create `activity.rs` with only the test module first)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::local::tests::unlocked_vault;
    use crate::model::{ItemInput, ItemType};
    use crate::Error;

    const T0: i64 = 1_800_000_000_000;
    const DAY: i64 = 86_400_000;

    fn add(vault: &mut VaultService, title: &str, at: i64) -> Uuid {
        let staged = vault
            .stage_create(ItemInput::blank(ItemType::SecureNote, title.into()), at)
            .unwrap();
        let id = staged.item_id;
        vault.commit_write(staged, 1).unwrap();
        id
    }

    fn titles(items: &[ItemOverview]) -> Vec<&str> {
        items.iter().map(|o| o.title.as_str()).collect()
    }

    #[test]
    fn more_uses_rank_higher_and_ties_go_to_the_latest() {
        let mut v = unlocked_vault();
        let a = add(&mut v, "A", T0);
        let b = add(&mut v, "B", T0);
        let c = add(&mut v, "C", T0);
        v.record_use(&a, T0).unwrap();
        v.record_use(&b, T0).unwrap();
        v.record_use(&b, T0).unwrap();
        v.record_use(&c, T0 + 1).unwrap();
        assert_eq!(titles(&v.frequently_used(10, T0 + 1).unwrap()), ["B", "C", "A"]);
        assert_eq!(titles(&v.frequently_used(1, T0 + 1).unwrap()), ["B"]);
    }

    #[test]
    fn old_uses_fade() {
        let mut v = unlocked_vault();
        let old = add(&mut v, "Old", T0);
        let new = add(&mut v, "New", T0);
        for _ in 0..3 {
            v.record_use(&old, T0).unwrap();
        }
        // 90 days later three old uses weigh 3 * 0.125 = 0.375 < 1.
        v.record_use(&new, T0 + 90 * DAY).unwrap();
        assert_eq!(titles(&v.frequently_used(10, T0 + 90 * DAY).unwrap()), ["New", "Old"]);
    }

    #[test]
    fn a_use_from_the_future_does_not_inflate() {
        assert_eq!(decayed(2.0, T0, T0 - DAY), 2.0);
    }

    #[test]
    fn deleted_items_and_the_identity_never_show() {
        let mut v = unlocked_vault();
        let gone = add(&mut v, "Gone", T0);
        let kept = add(&mut v, "Kept", T0);
        let identity = v.identity_item_id().unwrap();
        v.record_use(&gone, T0).unwrap();
        v.record_use(&kept, T0).unwrap();
        let staged = v.stage_delete(&gone).unwrap();
        v.commit_write(staged, 2).unwrap();
        // The identity is never listed, even if something recorded it.
        if v.session().unwrap().overviews.contains_key(&identity) {
            v.record_use(&identity, T0).unwrap();
        }
        assert_eq!(titles(&v.frequently_used(10, T0).unwrap()), ["Kept"]);
    }

    #[test]
    fn writes_prune_deleted_and_year_old_entries() {
        let mut v = unlocked_vault();
        let stale = add(&mut v, "Stale", T0);
        let gone = add(&mut v, "Gone", T0);
        let fresh = add(&mut v, "Fresh", T0);
        v.record_use(&stale, T0).unwrap();
        v.record_use(&gone, T0).unwrap();
        let staged = v.stage_delete(&gone).unwrap();
        v.commit_write(staged, 2).unwrap();
        v.record_use(&fresh, T0 + 366 * DAY).unwrap();
        let stored = v.activity().unwrap();
        assert_eq!(stored.uses.keys().collect::<Vec<_>>(), [&fresh]);
    }

    #[test]
    fn unknown_items_and_a_locked_vault_are_refused() {
        let mut v = unlocked_vault();
        assert_eq!(v.record_use(&Uuid::nil(), T0).unwrap_err(), Error::NotFound);
        v.lock();
        assert_eq!(v.frequently_used(6, T0).unwrap_err(), Error::Locked);
        assert_eq!(v.record_use(&Uuid::nil(), T0).unwrap_err(), Error::Locked);
    }

    #[test]
    fn unreadable_or_newer_activity_reads_as_empty() {
        let mut v = unlocked_vault();
        let a = add(&mut v, "A", T0);
        v.write_local(
            LocalSlot::Activity,
            &serde_json::json!({"version": 99, "whatever": true}),
        )
        .unwrap();
        assert!(v.frequently_used(6, T0).unwrap().is_empty());
        v.record_use(&a, T0).unwrap();
        assert_eq!(titles(&v.frequently_used(6, T0).unwrap()), ["A"]);
    }
}
```

Note: `local::tests` is `pub(crate) mod tests` with `pub(crate) fn unlocked_vault()`; it is usable from other modules' tests already.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-core activity::`
Expected: compile errors (`record_use`, `frequently_used`, `decayed`, `activity` not found).

- [ ] **Step 3: Implement** (above the test module in `activity.rs`)

```rust
//! What this device did with the vault: which items the user filled or
//! copied, and what they searched for (spec 2026-10-03-android-redesign §4).
//! One sealed document in the `Activity` local slot: per device, never
//! synced, readable only while unlocked. It is a convenience cache: an
//! unreadable or newer document reads as empty.

use crate::error::{Error, Result};
use crate::local::LocalSlot;
use crate::model::ItemOverview;
use crate::vault::VaultService;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

const DAY_MS: i64 = 86_400_000;
/// A use counts half as much after this long.
pub const HALF_LIFE_MS: i64 = 30 * DAY_MS;
/// A use older than this is forgotten.
pub const FORGET_AFTER_MS: i64 = 365 * DAY_MS;
pub const MAX_RECENT_SEARCHES: usize = 10;
const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Activity {
    version: u32,
    pub(crate) uses: BTreeMap<Uuid, Use>,
    /// Newest first. No `Debug` concern: tests only print counts.
    searches: Vec<String>,
}

impl Default for Activity {
    fn default() -> Self {
        Self { version: VERSION, uses: BTreeMap::new(), searches: Vec::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Use {
    /// Decayed use count as of `last_ms`.
    score: f64,
    /// Unix milliseconds of the last use.
    last_ms: i64,
}

/// `score` as of `to_ms`. A clock that went backwards never adds weight.
fn decayed(score: f64, from_ms: i64, to_ms: i64) -> f64 {
    let elapsed = to_ms.saturating_sub(from_ms).max(0) as f64;
    score * 0.5_f64.powf(elapsed / HALF_LIFE_MS as f64)
}

impl VaultService {
    pub(crate) fn activity(&self) -> Result<Activity> {
        match self.read_local::<Activity>(LocalSlot::Activity) {
            Ok(Some(a)) if a.version == VERSION => Ok(a),
            Ok(_) | Err(Error::Corrupted | Error::Decryption) => Ok(Activity::default()),
            Err(e) => Err(e),
        }
    }

    /// Drops uses of deleted items and uses older than a year, then seals.
    fn save_activity(&self, mut activity: Activity, now_ms: i64) -> Result<()> {
        let overviews = &self.session()?.overviews;
        activity.uses.retain(|id, u| {
            overviews.contains_key(id) && now_ms.saturating_sub(u.last_ms) <= FORGET_AFTER_MS
        });
        self.write_local(LocalSlot::Activity, &activity)
    }

    /// The user filled or copied `id`.
    pub fn record_use(&self, id: &Uuid, now_ms: i64) -> Result<()> {
        if !self.session()?.overviews.contains_key(id) {
            return Err(Error::NotFound);
        }
        let mut activity = self.activity()?;
        let next = match activity.uses.get(id) {
            Some(u) => Use { score: decayed(u.score, u.last_ms, now_ms) + 1.0, last_ms: now_ms.max(u.last_ms) },
            None => Use { score: 1.0, last_ms: now_ms },
        };
        activity.uses.insert(*id, next);
        self.save_activity(activity, now_ms)
    }

    /// Up to `n` items by decayed use, ties by the latest use. Deleted items
    /// and the account's Identity are left out.
    pub fn frequently_used(&self, n: usize, now_ms: i64) -> Result<Vec<ItemOverview>> {
        let session = self.session()?;
        let identity = self.identity_item_id().ok();
        let activity = self.activity()?;
        let mut ranked: Vec<(f64, i64, &ItemOverview)> = activity
            .uses
            .iter()
            .filter(|(id, _)| Some(**id) != identity)
            .filter_map(|(id, u)| {
                session.overviews.get(id).map(|o| (decayed(u.score, u.last_ms, now_ms), u.last_ms, o))
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.id.cmp(&b.2.id)));
        Ok(ranked.into_iter().take(n).map(|(_, _, o)| o.clone()).collect())
    }
}
```

Check `identity_item_id` at `vault.rs:1676`: if it returns `Err(Locked)` while locked, the `session()?` above already returned `Locked`; `.ok()` only covers vaults without an identity.

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-core activity:: && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/activity.rs crates/havenkeys-core/src/lib.rs
git commit -m "feat(core): record item uses and rank frequently used items"
```

---

### Task 3: "Recently added"

**Files:**
- Modify: `crates/havenkeys-core/src/activity.rs`

**Interfaces:**
- Produces: `pub fn recently_created(&self, n: usize) -> Result<Vec<ItemOverview>>`

- [ ] **Step 1: Write the failing test** (in `activity.rs` tests)

```rust
    #[test]
    fn recently_created_is_newest_first_without_the_identity() {
        let mut v = unlocked_vault();
        add(&mut v, "First", T0);
        add(&mut v, "Second", T0 + 1);
        add(&mut v, "Third", T0 + 2);
        let listed = v.recently_created(2).unwrap();
        assert_eq!(titles(&listed), ["Third", "Second"]);
        let identity = v.identity_item_id().ok();
        assert!(v.recently_created(100).unwrap().iter().all(|o| Some(o.id) != identity));
        v.lock();
        assert_eq!(v.recently_created(6).unwrap_err(), Error::Locked);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p havenkeys-core activity::tests::recently_created`
Expected: compile error, no method `recently_created`.

- [ ] **Step 3: Implement** (inside the `impl VaultService` block)

```rust
    /// Up to `n` items, newest first, the account's Identity left out.
    pub fn recently_created(&self, n: usize) -> Result<Vec<ItemOverview>> {
        let session = self.session()?;
        let identity = self.identity_item_id().ok();
        let mut items: Vec<&ItemOverview> =
            session.overviews.values().filter(|o| Some(o.id) != identity).collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
        Ok(items.into_iter().take(n).cloned().collect())
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-core activity::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/activity.rs
git commit -m "feat(core): list recently created items"
```

---

### Task 4: Recent searches

**Files:**
- Modify: `crates/havenkeys-core/src/activity.rs`

**Interfaces:**
- Consumes: `crate::vault::MAX_SEARCH_QUERY_CHARS` (256).
- Produces: `recent_searches(&self) -> Result<Vec<String>>`, `record_search(&self, query: &str, now_ms: i64) -> Result<()>`, `clear_recent_searches(&self, now_ms: i64) -> Result<()>`.

`now_ms` is passed to the two writers only because every write prunes uses (`save_activity`).

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn recent_searches_keep_ten_newest_first_without_duplicates() {
        let v = unlocked_vault();
        for i in 0..12 {
            v.record_search(&format!("q{i}"), T0).unwrap();
        }
        let recent = v.recent_searches().unwrap();
        assert_eq!(recent.len(), MAX_RECENT_SEARCHES);
        assert_eq!(recent[0], "q11");
        assert_eq!(recent[9], "q2");
        // Same words in another case move to the top, keeping the new spelling.
        v.record_search("Q5", T0).unwrap();
        let recent = v.recent_searches().unwrap();
        assert_eq!(recent[0], "Q5");
        assert_eq!(recent.iter().filter(|q| q.eq_ignore_ascii_case("q5")).count(), 1);
    }

    #[test]
    fn blank_and_overlong_searches_are_not_kept() {
        let v = unlocked_vault();
        v.record_search("   ", T0).unwrap();
        v.record_search("  github  ", T0).unwrap();
        assert_eq!(v.recent_searches().unwrap(), ["github"]);
        let long = "x".repeat(crate::vault::MAX_SEARCH_QUERY_CHARS + 1);
        assert!(matches!(v.record_search(&long, T0), Err(Error::InvalidInput(_))));
        assert_eq!(v.recent_searches().unwrap(), ["github"]);
    }

    #[test]
    fn clearing_searches_keeps_uses() {
        let mut v = unlocked_vault();
        let a = add(&mut v, "A", T0);
        v.record_use(&a, T0).unwrap();
        v.record_search("a", T0).unwrap();
        v.clear_recent_searches(T0).unwrap();
        assert!(v.recent_searches().unwrap().is_empty());
        assert_eq!(titles(&v.frequently_used(6, T0).unwrap()), ["A"]);
        v.lock();
        assert_eq!(v.recent_searches().unwrap_err(), Error::Locked);
        assert_eq!(v.record_search("a", T0).unwrap_err(), Error::Locked);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-core activity::`
Expected: compile errors for the three new methods.

- [ ] **Step 3: Implement**

Add `use crate::vault::MAX_SEARCH_QUERY_CHARS;` to the imports, then inside the `impl VaultService` block:

```rust
    /// Newest first.
    pub fn recent_searches(&self) -> Result<Vec<String>> {
        self.session()?;
        Ok(self.activity()?.searches)
    }

    /// Keeps `query` (trimmed) on top; a blank one is ignored, and the same
    /// words in another case replace the older spelling.
    pub fn record_search(&self, query: &str, now_ms: i64) -> Result<()> {
        self.session()?;
        let query = query.trim();
        if query.is_empty() {
            return Ok(());
        }
        if query.chars().count() > MAX_SEARCH_QUERY_CHARS {
            return Err(Error::InvalidInput("search query too long"));
        }
        let mut activity = self.activity()?;
        let lower = query.to_lowercase();
        activity.searches.retain(|q| q.to_lowercase() != lower);
        activity.searches.insert(0, query.to_owned());
        activity.searches.truncate(MAX_RECENT_SEARCHES);
        self.save_activity(activity, now_ms)
    }

    pub fn clear_recent_searches(&self, now_ms: i64) -> Result<()> {
        self.session()?;
        let mut activity = self.activity()?;
        activity.searches.clear();
        self.save_activity(activity, now_ms)
    }
```

- [ ] **Step 4: Run tests and lint**

Run: `cargo test -p havenkeys-core && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/activity.rs
git commit -m "feat(core): keep this device's recent searches"
```

---

### Task 5: Mobile exports and `created_at`

**Files:**
- Modify: `crates/havenkeys-mobile/src/items.rs:25-35` (`ItemSummary`), `:109` (`summary` to `pub(crate)`), `:128` (set `created_at`)
- Create: `crates/havenkeys-mobile/src/activity.rs`
- Modify: `crates/havenkeys-mobile/src/lib.rs` (add `mod activity;` after `mod account;`)

**Interfaces:**
- Consumes: Tasks 2–4 core methods; `items::{parse_id, summary}`; `MobileVault::unlocked()`; `havenkeys_client::now_ms()`.
- Produces (UniFFI, Kotlin names in brackets):
  - `record_use(id: String) -> MobileResult<()>` [`recordUse(id)`]
  - `frequently_used(n: u32) -> MobileResult<Vec<ItemSummary>>` [`frequentlyUsed(n: UInt)`]
  - `recently_created(n: u32) -> MobileResult<Vec<ItemSummary>>` [`recentlyCreated(n: UInt)`]
  - `recent_searches() -> MobileResult<Vec<String>>` [`recentSearches()`]
  - `record_search(query: String) -> MobileResult<()>` [`recordSearch(query)`]
  - `clear_recent_searches() -> MobileResult<()>` [`clearRecentSearches()`]
  - `ItemSummary.created_at: i64` (last field) [`createdAt: Long`]
  - `pub(crate) fn note_use(&self, id: &Uuid)` for Task 6.

- [ ] **Step 1: Write the failing tests** (in the new `activity.rs`)

```rust
#[cfg(test)]
mod tests {
    use crate::vault::tests::unlocked;
    use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
    use havenkeys_core::SecretString;

    fn add_note(v: &crate::MobileVault, title: &str, at: i64) -> String {
        let mut input = ItemInput::blank_for_tests(ItemType::SecureNote, title);
        input.content = SecretUpdate::Set(SecretString::from("body"));
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, at).unwrap();
        let id = staged.item_id;
        vault.commit_write(staged, 1).unwrap();
        id.to_string()
    }

    #[test]
    fn home_lists_and_searches_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let a = add_note(&v, "A", 1);
        let b = add_note(&v, "B", 2);
        let recent = v.recently_created(6).unwrap();
        assert_eq!(recent.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["B", "A"]);
        assert_eq!(recent[0].created_at, 2);
        v.record_use(a.clone()).unwrap();
        assert_eq!(v.frequently_used(6).unwrap()[0].id, a);
        v.record_search("note".into()).unwrap();
        assert_eq!(v.recent_searches().unwrap(), ["note"]);
        v.clear_recent_searches().unwrap();
        assert!(v.recent_searches().unwrap().is_empty());
        assert!(v.record_use("not-a-uuid".into()).is_err());
        let _ = b;
    }

    #[test]
    fn nothing_is_read_or_written_locked() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        v.lock();
        assert!(v.frequently_used(6).is_err());
        assert!(v.recent_searches().is_err());
        assert!(v.record_search("x".into()).is_err());
    }
}
```

`ItemInput::blank` is `pub(crate)` in core, so add to `crates/havenkeys-core/src/model.rs`, next to `blank`:

```rust
    /// `blank` for other crates' tests.
    #[cfg(any(test, feature = "testing"))]
    #[doc(hidden)]
    pub fn blank_for_tests(item_type: ItemType, title: &str) -> ItemInput {
        Self::blank(item_type, title.to_owned())
    }
```

Check `crates/havenkeys-core/Cargo.toml` for a `testing` feature; if absent, add `[features] testing = []` and in `crates/havenkeys-mobile/Cargo.toml` `[dev-dependencies]` enable it: `havenkeys-core = { path = "../havenkeys-core", features = ["testing"] }` (match the existing dependency line's other settings).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-mobile activity::`
Expected: compile errors (`recently_created` etc. not found on `MobileVault`, no `created_at`).

- [ ] **Step 3: Implement**

`items.rs`, `ItemSummary` gains a last field (last, so positional Kotlin calls only append one argument):

```rust
    pub updated_at: i64,
    pub created_at: i64,
}
```

`fn summary` becomes `pub(crate) fn summary`, and its struct literal gains `created_at: o.created_at,` after `updated_at`.

`activity.rs` (above the tests):

```rust
//! Home's lists and the search screen's recent searches (spec
//! 2026-10-03-android-redesign §4). Per device, sealed, never synced.

use crate::error::MobileResult;
use crate::items::{parse_id, summary, ItemSummary};
use crate::vault::MobileVault;
use uuid::Uuid;

impl MobileVault {
    /// A pick the user made in a Rust call (passkey, Credential Manager
    /// password, confirmed binding). Never fails the pick: the activity is a
    /// convenience, the fill is the point. Call it only after every
    /// `client.vault()` guard of the caller is dropped (not re-entrant).
    pub(crate) fn note_use(&self, id: &Uuid) {
        if let Ok(vault) = self.client.vault() {
            let _ = vault.record_use(id, havenkeys_client::now_ms());
        }
    }
}

#[uniffi::export]
impl MobileVault {
    /// The app copied a field of `id`, or Android filled a row of it.
    pub fn record_use(&self, id: String) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        Ok(self.client.vault()?.record_use(&id, havenkeys_client::now_ms())?)
    }

    pub fn frequently_used(&self, n: u32) -> MobileResult<Vec<ItemSummary>> {
        self.unlocked()?;
        let found = self.client.vault()?.frequently_used(n as usize, havenkeys_client::now_ms())?;
        Ok(found.iter().map(summary).collect())
    }

    pub fn recently_created(&self, n: u32) -> MobileResult<Vec<ItemSummary>> {
        self.unlocked()?;
        Ok(self.client.vault()?.recently_created(n as usize)?.iter().map(summary).collect())
    }

    pub fn recent_searches(&self) -> MobileResult<Vec<String>> {
        self.unlocked()?;
        Ok(self.client.vault()?.recent_searches()?)
    }

    pub fn record_search(&self, query: String) -> MobileResult<()> {
        self.unlocked()?;
        Ok(self.client.vault()?.record_search(&query, havenkeys_client::now_ms())?)
    }

    pub fn clear_recent_searches(&self) -> MobileResult<()> {
        self.unlocked()?;
        Ok(self.client.vault()?.clear_recent_searches(havenkeys_client::now_ms())?)
    }
}
```

No `touch()` here, following the rule in `items.rs` `list_items`: reads are not activity; the app touches on real interaction.

- [ ] **Step 4: Run tests and lint**

Run: `cargo test -p havenkeys-mobile && cargo clippy -p havenkeys-mobile --all-targets -- -D warnings`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/model.rs crates/havenkeys-core/Cargo.toml crates/havenkeys-mobile
git commit -m "feat(mobile): expose Home's lists, recent searches and created_at"
```

---

### Task 6: Rust picks record a use

**Files:**
- Modify: `crates/havenkeys-mobile/src/credentials.rs` (`passkey_sign_in` at :216, `credential_password` at :371)
- Modify: `crates/havenkeys-mobile/src/autofill.rs` (`autofill_bind_and_fill` at :260)

**Interfaces:**
- Consumes: `note_use` (Task 5), `frequently_used` (Task 5).

- [ ] **Step 1: Write the failing tests**

In `autofill.rs` tests, using that module's existing login helper and target builders (read the top of its `mod tests` for their names; the module already tests `autofill_fill` and `autofill_bind_and_fill`):

```rust
    #[test]
    fn prefetching_values_records_nothing_but_a_confirmed_binding_does() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v); // the module's helper: a GitHub login
        let _ = v.autofill_fill(id.clone(), browser_target("https://github.com/login"));
        let _ = v.autofill_totp(id.clone(), browser_target("https://github.com/login"));
        assert!(v.frequently_used(6).unwrap().is_empty());
        let _ = v.autofill_bind_and_fill(id.clone(), app_target("com.example.app"));
        assert_eq!(v.frequently_used(6).unwrap()[0].id, id);
    }
```

If the helpers have other names (`target_for_page`, `app_facts`…), use those; the assertion lines are the test.

In `credentials.rs` tests, next to the existing `credential_password` test (it uses `crate::testing::vouch` for the caller):

```rust
    #[test]
    fn a_credential_manager_password_pick_records_a_use() {
        // Arrange exactly as the existing credential_password test does,
        // then after the successful call:
        assert_eq!(v.frequently_used(6).unwrap()[0].id, id);
    }
```

Copy the existing test's arrange-and-act lines verbatim above that assertion. Do the same for the existing successful `passkey_sign_in` test (`seed_github_passkey`), asserting the passkey's item id is first in `frequently_used(6)`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p havenkeys-mobile`
Expected: the three new tests fail on the `frequently_used` assertions (empty).

- [ ] **Step 3: Implement**

`autofill_bind_and_fill`: the guard lives inside the `let creds = { ... }` block, so after it:

```rust
        self.note_use(&item);
        Ok(BoundFill {
```

`credential_password`: the guards are temporaries inside the `match`, dropped at its end, so before the `Ok(FillValues { .. })`:

```rust
        self.note_use(&id);
```

`passkey_sign_in`: the `let vault` guard is scoped to the `Caller::Browser` arm; after the `let assertion = match ... ;` statement and before building the JSON response:

```rust
        self.note_use(&item);
```

Do not add `note_use` to `autofill_fill`, `autofill_totp`, `autofill_card_values` or `autofill_identity_values`: direct fill calls them for every offered row before the user picks.

- [ ] **Step 4: Run tests**

Run: `cargo test -p havenkeys-mobile`
Expected: all pass (a deadlock here would hang: it means a guard was still alive; move `note_use` later).

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-mobile/src/autofill.rs crates/havenkeys-mobile/src/credentials.rs
git commit -m "feat(mobile): a passkey, password or binding pick counts as a use"
```

---

### Task 7: Kotlin bindings, repositories and copies

**Files:**
- Regenerate: `apps/android/app/src/main/kotlin/uniffi/havenkeys_mobile/havenkeys_mobile.kt`
- Modify: `ANDROID/data/VaultRepository.kt:14-80`, `ANDROID/data/AutofillRepository.kt:20-45`
- Modify: `ANDROID/ui/item/ItemViewModel.kt`, `ANDROID/ui/item/ItemScreen.kt:238-242`
- Modify: `ANDROID/ui/components/ItemRow.kt:111` (preview `ItemSummary(` call)
- Test: `ANDROID_TEST/fakes/FakeRepositories.kt`, `ANDROID_TEST/ui/item/ItemViewModelTest.kt`, `ANDROID_TEST/ui/vault/VaultViewModelTest.kt:156`

**Interfaces:**
- Consumes: Task 5 UniFFI calls.
- Produces (`VaultRepository`):
  - `suspend fun recordUse(id: String): Outcome<Unit>`
  - `suspend fun frequentlyUsed(n: Int): Outcome<List<ItemSummary>>`
  - `suspend fun recentlyCreated(n: Int): Outcome<List<ItemSummary>>`
  - `suspend fun recentSearches(): Outcome<List<String>>`
  - `suspend fun recordSearch(query: String): Outcome<Unit>`
  - `suspend fun clearRecentSearches(): Outcome<Unit>`
- Produces (`AutofillRepository`): `suspend fun recordUse(id: String): Outcome<Unit>`
- Produces (`ItemViewModel`): `suspend fun copied()`

- [ ] **Step 1: Regenerate bindings**

Run: `ANDROID_NDK_HOME=<your NDK> scripts/build-android.sh`
Expected: `havenkeys_mobile.kt` now has `recordUse`, `frequentlyUsed`, `recentlyCreated`, `recentSearches`, `recordSearch`, `clearRecentSearches`, and `ItemSummary(..., updatedAt: Long, createdAt: Long)`.

- [ ] **Step 2: Fix the positional `ItemSummary(` calls**

Append a `createdAt` argument (`0`) to each of the three calls: `ItemRow.kt:111` preview, `ItemViewModelTest.kt` `summary()`, `VaultViewModelTest.kt:156`. Run `grep -rn "ItemSummary(" apps/android/app/src --include=*.kt | grep -v uniffi` to confirm none is missed.

- [ ] **Step 3: Write the failing test** (`ItemViewModelTest`)

```kotlin
    @Test
    fun aCopyCountsAsAUseOfThisItem() = runTest {
        val vault = FakeVaultRepository()
        val vm = vm(vault)
        vm.copied()
        assertEquals(listOf("id"), vault.usesRecorded)
    }
```

In `FakeVaultRepository` add:

```kotlin
    val usesRecorded = mutableListOf<String>()
    var frequent: Outcome<List<ItemSummary>> = Outcome.Ok(emptyList())
    var recent: Outcome<List<ItemSummary>> = Outcome.Ok(emptyList())
    val searches = mutableListOf<String>()

    override suspend fun recordUse(id: String): Outcome<Unit> {
        usesRecorded += id
        return Outcome.Ok(Unit)
    }

    override suspend fun frequentlyUsed(n: Int) = frequent

    override suspend fun recentlyCreated(n: Int) = recent

    override suspend fun recentSearches(): Outcome<List<String>> = Outcome.Ok(searches.toList())

    override suspend fun recordSearch(query: String): Outcome<Unit> {
        searches.removeAll { it.equals(query.trim(), ignoreCase = true) }
        if (query.isNotBlank()) searches.add(0, query.trim())
        return Outcome.Ok(Unit)
    }

    override suspend fun clearRecentSearches(): Outcome<Unit> {
        searches.clear()
        return Outcome.Ok(Unit)
    }
```

and in the fake `AutofillRepository` (same file) `val usesRecorded = mutableListOf<String>()` with `override suspend fun recordUse(id: String): Outcome<Unit> { usesRecorded += id; return Outcome.Ok(Unit) }`.

- [ ] **Step 4: Run to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*ItemViewModelTest*'`
Expected: compile error, `copied` / repository members missing.

- [ ] **Step 5: Implement**

`VaultRepository` interface, after `delete`:

```kotlin
    suspend fun recordUse(id: String): Outcome<Unit>
    suspend fun frequentlyUsed(n: Int): Outcome<List<ItemSummary>>
    suspend fun recentlyCreated(n: Int): Outcome<List<ItemSummary>>
    suspend fun recentSearches(): Outcome<List<String>>
    suspend fun recordSearch(query: String): Outcome<Unit>
    suspend fun clearRecentSearches(): Outcome<Unit>
```

`RustVaultRepository`, after `delete`:

```kotlin
    override suspend fun recordUse(id: String) = rust { vault.recordUse(id) }
    override suspend fun frequentlyUsed(n: Int) = rust { vault.frequentlyUsed(n.toUInt()) }
    override suspend fun recentlyCreated(n: Int) = rust { vault.recentlyCreated(n.toUInt()) }
    override suspend fun recentSearches() = rust { vault.recentSearches() }
    override suspend fun recordSearch(query: String) = rust { vault.recordSearch(query) }
    override suspend fun clearRecentSearches() = rust { vault.clearRecentSearches() }
```

`AutofillRepository` interface gains `suspend fun recordUse(id: String): Outcome<Unit>`; `RustAutofillRepository` gains `override suspend fun recordUse(id: String) = rust { vault.recordUse(id) }` (use the file's existing wrapper helper name if it is not `rust`).

`ItemViewModel`, after `reveal`:

```kotlin
    /** The screen copied one of this item's fields: Home's "Frequently used" counts it. */
    suspend fun copied() {
        vault.recordUse(id)
    }
```

`ItemScreen.kt` `copy`:

```kotlin
    private suspend fun copy(label: String, value: String) {
        val seconds = viewModel.clipboardClearSeconds()
        clipboard.copy(label, value, seconds)
        viewModel.copied()
        snackbar.showSnackbar(resources.getString(R.string.copied, label, seconds))
    }
```

- [ ] **Step 6: Run tests and detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add apps/android
git commit -m "feat(android): repositories for activity data; a copy counts as a use"
```

---

### Task 8: Autofill picks count

**Files:**
- Create: `ANDROID/autofill/DatasetIds.kt`
- Modify: `ANDROID/autofill/AutofillRows.kt:85-118` (`dataset`, `legacyDataset`)
- Modify: `ANDROID/autofill/DatasetFactory.kt:76-87` (`datasetOf`)
- Modify: `ANDROID/autofill/WalletDatasets.kt:49-62` (`cards`)
- Modify: `ANDROID/autofill/HavenAutofillService.kt:40-70` (`onFillRequest`)
- Modify: `ANDROID/autofill/AutofillAuthActivity.kt:60-90` (login and card answers)
- Test: `ANDROID_TEST/autofill/DatasetIdsTest.kt`

**Interfaces:**
- Consumes: `AutofillRepository.recordUse` (Task 7).
- Produces: `object DatasetIds { fun of(itemId: String): String; fun itemOf(datasetId: String?): String? }`; `AutofillRows.dataset(..., datasetId: String? = null)`.

- [ ] **Step 1: Write the failing test**

```kotlin
package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DatasetIdsTest {
    private val uuid = "7c9e6679-7425-40de-944b-e07fc1f90ae7"

    @Test
    fun anItemRowRoundTrips() = assertEquals(uuid, DatasetIds.itemOf(DatasetIds.of(uuid)))

    @Test
    fun otherRowsAndJunkAreNotItems() {
        assertNull(DatasetIds.itemOf(null))
        assertNull(DatasetIds.itemOf("search"))
        assertNull(DatasetIds.itemOf("item:"))
        assertNull(DatasetIds.itemOf("item:not-a-uuid"))
        assertNull(DatasetIds.itemOf("item:$uuid:extra"))
    }

    @Test
    fun onlyPickedDirectRowsCount() {
        val events = listOf(
            DatasetIds.Picked(DatasetIds.Kind.SELECTED, DatasetIds.of(uuid)),
            DatasetIds.Picked(DatasetIds.Kind.AUTHENTICATION_SELECTED, DatasetIds.of(uuid)),
            DatasetIds.Picked(DatasetIds.Kind.SELECTED, "search"),
            DatasetIds.Picked(DatasetIds.Kind.OTHER, DatasetIds.of(uuid)),
        )
        assertEquals(listOf(uuid), DatasetIds.usedItems(events))
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*DatasetIdsTest*'`
Expected: compile error, `DatasetIds` missing.

- [ ] **Step 3: Implement `DatasetIds.kt`**

```kotlin
package net.havenkeys.android.autofill

import java.util.UUID

/**
 * Ids on direct-fill rows, so a picked row can count as a use of its item
 * (spec 2026-10-03-android-redesign §4.4). Only rows without authentication
 * carry one: a confirmed row counts in [AutofillAuthActivity] instead. An
 * item UUID is not a secret (routes carry it too).
 */
object DatasetIds {
    private const val PREFIX = "item:"

    fun of(itemId: String): String = PREFIX + itemId

    fun itemOf(datasetId: String?): String? {
        val rest = datasetId?.removePrefix(PREFIX)?.takeIf { it != datasetId } ?: return null
        return runCatching { UUID.fromString(rest) }.getOrNull()?.takeIf { it.toString() == rest.lowercase() }?.let { rest }
    }

    enum class Kind { SELECTED, AUTHENTICATION_SELECTED, OTHER }

    /** One event of Android's fill event history, reduced to what counts. */
    data class Picked(val kind: Kind, val datasetId: String?)

    fun usedItems(events: List<Picked>): List<String> =
        events.filter { it.kind == Kind.SELECTED }.mapNotNull { itemOf(it.datasetId) }
}
```

- [ ] **Step 4: Run the test**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*DatasetIdsTest*'`
Expected: PASS.

- [ ] **Step 5: Put ids on direct rows**

`AutofillRows.dataset` gains a last parameter `datasetId: String? = null`. In the Tiramisu branch add `datasetId?.let(builder::setId)` after the fields loop; pass it to `legacyDataset` and there add `datasetId?.let(builder::setId)` (`Dataset.Builder.setId` exists since API 28, the app's minSdk).

`DatasetFactory.datasetOf`, last line:

```kotlin
        return rows.dataset(fields, plan.match.title, subtitle, auth, datasetId = if (auth == null) DatasetIds.of(plan.match.id) else null)
```

`WalletDatasets.cards`, the direct branch (`values != null`):

```kotlin
                rows.dataset(entries, cardTitle(row.card), subtitle, null, datasetId = DatasetIds.of(row.card.id))
```

Identity rows get no id (the Identity is not in Home's lists).

- [ ] **Step 6: Read the fill event history**

Before writing this step, check the current Android reference for `AutofillService.getFillEventHistory()` and `FillEventHistory.Event` on the app's targetSdk 36: if it is deprecated there, use the replacement it names; the mapping into `DatasetIds.Picked` stays the same.

In `HavenAutofillService.onFillRequest`, first thing inside `scope.launch { ... }` of `work` (before `respond`):

```kotlin
            recordPickedRows()
```

and add to the class:

```kotlin
    /**
     * Rows picked since the last request: Android reports them here and
     * nowhere else. Direct rows only (they carry an item id); confirmed rows
     * counted in AutofillAuthActivity.
     */
    private suspend fun recordPickedRows() {
        val events = fillEventHistory?.events.orEmpty().map { event ->
            val kind = when (event.type) {
                FillEventHistory.Event.TYPE_DATASET_SELECTED -> DatasetIds.Kind.SELECTED
                FillEventHistory.Event.TYPE_DATASET_AUTHENTICATION_SELECTED -> DatasetIds.Kind.AUTHENTICATION_SELECTED
                else -> DatasetIds.Kind.OTHER
            }
            DatasetIds.Picked(kind, event.datasetId)
        }
        if (!container.events.unlocked.value) return
        DatasetIds.usedItems(events).forEach { container.autofillRepository.recordUse(it) }
    }
```

(import `android.service.autofill.FillEventHistory`). Locked: the history is dropped, not queued; a fill while locked went through unlock and `AutofillAuthActivity` anyway.

- [ ] **Step 7: Count confirmed fills**

In `AutofillAuthActivity.answerLogin`, where a login dataset is produced from `repo.fill(itemId, ...)` (line ~76) and in `answerCard` where the card dataset is produced, record after a successful result and before `finishWith(...)`:

```kotlin
        if (dataset != null && itemId != null) repo.recordUse(itemId)
```

using the local names those functions already use for the resulting dataset and the repository (read lines 60–120 first; `repo` is the `AutofillRepository` there). The `MODE_COPY_TOTP` branch copies the code to the clipboard, which is a copy, so it counts too: call `repo.recordUse(itemId)` there after `copyCode(...)` succeeds.

- [ ] **Step 8: Build, test, detekt**

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug`
Expected: all pass, APK builds.

- [ ] **Step 9: Manual check deferred**

Nothing in the app shows activity until stage 3's Home. The mapping is covered by `DatasetIdsTest`; that Android really delivers `TYPE_DATASET_SELECTED` with our id is checked in stage 3's emulator pass ("pick a direct-fill row in Chrome, open another form, then see the login under Frequently used"). Record that check in the stage 3 plan.

- [ ] **Step 10: Commit**

```bash
git add apps/android
git commit -m "feat(android): a picked autofill row counts as a use of its item"
```

---

### Task 9: Docs

**Files:**
- Modify: `CLAUDE.md` (amendment block after the 2026-10-02 one)
- Modify: `docs/security-model.md` (the list of sealed local data)
- Modify: `docs/superpowers/specs/2026-10-01-android-app-design.md` §9.1 (amendment note)

- [ ] **Step 1: `CLAUDE.md`**

After the "Amended on 2026-10-02" block, add:

```markdown
> Amended on 2026-10-03 by
> `docs/superpowers/specs/2026-10-03-android-redesign-design.md`: each
> device keeps a sealed, never-synced activity record (which items the user
> filled or copied, and their last 10 searches) in Rust, for the phone's
> Home and search screens; the desktop may adopt it later.
```

- [ ] **Step 2: `docs/security-model.md`**

Find where `DeviceSettings` / `AssetLinks` local slots are described (`grep -n "AssetLinks\|asset_links\|local slot" docs/security-model.md`) and add the `activity` slot beside them: sealed with the data key under purpose `activity`, per device, never sent to the server, holds item ids with a decayed use score and last-use time, and up to 10 search queries; readable only while unlocked; reveals no more than the vault itself to someone who can unlock it.

- [ ] **Step 3: Android spec note**

Under the heading of §9.1 in `2026-10-01-android-app-design.md`:

```markdown
> Amended on 2026-10-03 by
> `docs/superpowers/specs/2026-10-03-android-redesign-design.md`: own
> component set instead of Material 3, Home/Items/Settings navigation,
> per-device activity data.
```

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs/security-model.md docs/superpowers/specs/2026-10-01-android-app-design.md
git commit -m "docs: per-device activity data"
```

---

## Stage exit check

- [ ] `cargo test -p havenkeys-core -p havenkeys-mobile` and `cargo clippy --all-targets -- -D warnings` for both crates pass.
- [ ] `cd apps/android && ./gradlew testGithubDebugUnitTest detekt assembleGithubDebug` passes.
- [ ] `grep -rn "record_use\|recordUse" crates apps/android/app/src/main` shows calls only in: core activity, mobile activity (`note_use`, export), `credentials.rs` (2), `autofill.rs` (`autofill_bind_and_fill`), `ItemViewModel.copied`, `HavenAutofillService.recordPickedRows`, `AutofillAuthActivity`.
- [ ] No search query or title appears in any `Debug` impl, error message or log.
