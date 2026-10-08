//! The Trash (spec 2026-10-08-trash §6.2): overviews only, never a value.

use crate::items::{parse_id, summary, ItemSummary};
use crate::vault::MobileVault;
use crate::MobileResult;

#[derive(uniffi::Record)]
pub struct TrashSummary {
    pub item: ItemSummary,
    pub trashed_at: i64,
    pub days_left: u32,
}

#[uniffi::export]
impl MobileVault {
    /// Delete moves the item to the Trash. `false`: its details did not
    /// open, so it was deleted for good. The identity cannot be (the core refuses).
    pub fn trash_item(&self, id: String) -> MobileResult<bool> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self
            .client
            .vault()?
            .stage_trash(&id, havenkeys_client::now_ms())?;
        self.send_returning(staged).map(|ov| ov.is_some())
    }

    pub fn restore_item(&self, id: String) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self.client.vault()?.stage_restore_trashed(&id)?;
        self.send(staged)
    }

    pub fn purge_item(&self, id: String) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self.client.vault()?.stage_purge(&id)?;
        self.send(staged)
    }

    pub fn empty_trash(&self) -> MobileResult<u32> {
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self.client.vault()?.stage_empty_trash()?;
        let client = self.client.clone();
        let n = self.block_on(client.push_batches(staged))?;
        havenkeys_client::ClientEvents::items_changed(&*self.events);
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    pub fn list_trash(&self) -> MobileResult<Vec<TrashSummary>> {
        self.unlocked()?;
        Ok(self
            .client
            .vault()?
            .list_trash(havenkeys_client::now_ms())?
            .iter()
            .map(|e| TrashSummary {
                item: summary(&e.overview),
                trashed_at: e.overview.trashed_at.unwrap_or(0),
                days_left: e.days_left,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::vault::tests::{code, overdue_refuses, unlocked};
    use havenkeys_core::model::{ItemInput, ItemType};

    #[test]
    fn a_trashed_item_leaves_the_list_and_waits_in_the_trash() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        assert!(v.list_trash().unwrap().is_empty());

        let id = {
            let mut vault = v.client.vault().unwrap();
            let input = ItemInput::blank_for_tests(ItemType::SecureNote, "Old wifi");
            let staged = vault.stage_create(input, 1).unwrap();
            let id = staged.item_id;
            vault.commit_write(staged, 1).unwrap();
            let staged = vault.stage_trash(&id, havenkeys_client::now_ms()).unwrap();
            vault.commit_write(staged, 2).unwrap();
            id
        };

        let trash = v.list_trash().unwrap();
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].item.id, id.to_string());
        assert_eq!(trash[0].item.title, "Old wifi");
        assert_eq!(trash[0].days_left, 30);
        assert!(trash[0].trashed_at > 0);
        assert!(v
            .list_items()
            .unwrap()
            .iter()
            .all(|i| i.id != id.to_string()));
    }

    #[test]
    fn a_locked_vault_lists_no_trash() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        overdue_refuses(&v, &seen, |v| v.list_trash());
        assert_eq!(code(v.list_trash().err().unwrap()), "locked");
    }
}
