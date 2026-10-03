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

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Activity {
    version: u32,
    pub(crate) uses: BTreeMap<Uuid, Use>,
    /// Newest first. Search queries are the user's own words: this type has no
    /// `Debug`, so they cannot be printed by accident.
    searches: Vec<String>,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            version: VERSION,
            uses: BTreeMap::new(),
            searches: Vec::new(),
        }
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
            Some(u) => Use {
                score: decayed(u.score, u.last_ms, now_ms) + 1.0,
                last_ms: now_ms.max(u.last_ms),
            },
            None => Use {
                score: 1.0,
                last_ms: now_ms,
            },
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
                session
                    .overviews
                    .get(id)
                    .map(|o| (decayed(u.score, u.last_ms, now_ms), u.last_ms, o))
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(b.1.cmp(&a.1))
                .then(a.2.id.cmp(&b.2.id))
        });
        Ok(ranked
            .into_iter()
            .take(n)
            .map(|(_, _, o)| o.clone())
            .collect())
    }

    /// Up to `n` items, newest first, the account's Identity left out.
    pub fn recently_created(&self, n: usize) -> Result<Vec<ItemOverview>> {
        let session = self.session()?;
        let identity = self.identity_item_id().ok();
        let mut items: Vec<&ItemOverview> = session
            .overviews
            .values()
            .filter(|o| Some(o.id) != identity)
            .collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
        Ok(items.into_iter().take(n).cloned().collect())
    }
}

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
        assert_eq!(
            titles(&v.frequently_used(10, T0 + 1).unwrap()),
            ["B", "C", "A"]
        );
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
        assert_eq!(
            titles(&v.frequently_used(10, T0 + 90 * DAY).unwrap()),
            ["New", "Old"]
        );
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

    #[test]
    fn recently_created_is_newest_first_without_the_identity() {
        let mut v = unlocked_vault();
        add(&mut v, "First", T0);
        add(&mut v, "Second", T0 + 1);
        add(&mut v, "Third", T0 + 2);
        let listed = v.recently_created(2).unwrap();
        assert_eq!(titles(&listed), ["Third", "Second"]);
        let identity = v.identity_item_id().ok();
        assert!(v
            .recently_created(100)
            .unwrap()
            .iter()
            .all(|o| Some(o.id) != identity));
        v.lock();
        assert_eq!(v.recently_created(6).unwrap_err(), Error::Locked);
    }
}
