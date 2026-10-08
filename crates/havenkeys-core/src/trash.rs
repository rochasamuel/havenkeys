//! The Trash (spec 2026-10-08-trash). A trashed item keeps its sealed
//! details; its overview carries `trashed_at` and lives in `Session.trash`,
//! which only these functions read.

use uuid::Uuid;

use crate::error::{Error, Result};
use crate::model::TrashEntry;
use crate::vault::{StagedWrite, VaultService};

/// How long an item stays in the Trash.
pub const TRASH_RETENTION_MS: i64 = 30 * 86_400_000;
const DAY_MS: i64 = 86_400_000;

impl VaultService {
    /// Move a live item to the Trash. Its details are re-sealed unchanged
    /// (they are bound to the overview blob). An item whose details no
    /// longer open could never be restored, so it is deleted instead: the
    /// commit then returns `None`.
    pub fn stage_trash(&self, id: &Uuid, now_ms: i64) -> Result<StagedWrite> {
        let session = self.session()?;
        if *id == session.identity_id {
            return Err(Error::Denied);
        }
        let mut overview = session.overviews.get(id).cloned().ok_or(Error::NotFound)?;
        let base = self.item_revision(id)?;
        let details = match self.load_details(id) {
            Ok(d) => d,
            Err(Error::Decryption | Error::Corrupted) => return self.tombstone(id),
            Err(e) => return Err(e),
        };
        overview.trashed_at = Some(now_ms);
        self.seal_staged(overview, Some(&details), base)
    }

    /// Put a trashed item back, exactly as it was.
    pub fn stage_restore_trashed(&self, id: &Uuid) -> Result<StagedWrite> {
        let session = self.session()?;
        let mut overview = session.trash.get(id).cloned().ok_or(Error::NotFound)?;
        let details = self.load_trashed_details(id)?;
        overview.trashed_at = None;
        let base = self.item_revision(id)?;
        // `stage`, not `seal_staged`: its tags take the vault's spellings.
        self.stage(overview, Some(&details), base)
    }

    /// Delete one trashed item for good.
    pub fn stage_purge(&self, id: &Uuid) -> Result<StagedWrite> {
        if !self.session()?.trash.contains_key(id) {
            return Err(Error::NotFound);
        }
        self.tombstone(id)
    }

    /// Delete every trashed item for good.
    pub fn stage_empty_trash(&self) -> Result<Vec<StagedWrite>> {
        let mut ids: Vec<Uuid> = self.session()?.trash.keys().copied().collect();
        ids.sort_unstable();
        ids.iter().map(|id| self.tombstone(id)).collect()
    }

    /// Tombstones for items trashed at least `TRASH_RETENTION_MS` ago. A
    /// `trashed_at` in the future (another device's clock) is not due.
    pub fn stage_expired_trash(&self, now_ms: i64) -> Result<Vec<StagedWrite>> {
        let mut ids: Vec<Uuid> = self
            .session()?
            .trash
            .values()
            .filter(|o| {
                o.trashed_at
                    .is_some_and(|t| now_ms.saturating_sub(t) >= TRASH_RETENTION_MS)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_unstable();
        ids.iter().map(|id| self.tombstone(id)).collect()
    }

    /// The Trash, newest first.
    pub fn list_trash(&self, now_ms: i64) -> Result<Vec<TrashEntry>> {
        let mut entries: Vec<TrashEntry> = self
            .session()?
            .trash
            .values()
            .map(|o| TrashEntry {
                days_left: days_left(o.trashed_at.unwrap_or(now_ms), now_ms),
                overview: o.clone(),
            })
            .collect();
        entries.sort_by_key(|e| (std::cmp::Reverse(e.overview.trashed_at), e.overview.id));
        Ok(entries)
    }

    pub fn trash_count(&self) -> Result<usize> {
        Ok(self.session()?.trash.len())
    }
}

/// Whole days, rounded up, until removal; 0 to 30.
fn days_left(trashed_at: i64, now_ms: i64) -> u32 {
    let remaining = trashed_at
        .saturating_add(TRASH_RETENTION_MS)
        .saturating_sub(now_ms);
    let days = remaining.saturating_add(DAY_MS - 1).div_euclid(DAY_MS);
    days.clamp(0, TRASH_RETENTION_MS / DAY_MS) as u32
}
