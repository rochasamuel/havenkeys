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
        Ok(self
            .client
            .vault()?
            .record_use(&id, havenkeys_client::now_ms())?)
    }

    pub fn frequently_used(&self, n: u32) -> MobileResult<Vec<ItemSummary>> {
        self.unlocked()?;
        let found = self
            .client
            .vault()?
            .frequently_used(n as usize, havenkeys_client::now_ms())?;
        Ok(found.iter().map(summary).collect())
    }

    pub fn recently_created(&self, n: u32) -> MobileResult<Vec<ItemSummary>> {
        self.unlocked()?;
        Ok(self
            .client
            .vault()?
            .recently_created(n as usize)?
            .iter()
            .map(summary)
            .collect())
    }

    pub fn recent_searches(&self) -> MobileResult<Vec<String>> {
        self.unlocked()?;
        Ok(self.client.vault()?.recent_searches()?)
    }

    pub fn record_search(&self, query: String) -> MobileResult<()> {
        self.unlocked()?;
        Ok(self
            .client
            .vault()?
            .record_search(&query, havenkeys_client::now_ms())?)
    }

    pub fn clear_recent_searches(&self) -> MobileResult<()> {
        self.unlocked()?;
        Ok(self
            .client
            .vault()?
            .clear_recent_searches(havenkeys_client::now_ms())?)
    }
}

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
        assert_eq!(
            recent.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
            ["B", "A"]
        );
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
