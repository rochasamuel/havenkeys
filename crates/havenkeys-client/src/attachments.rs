//! Adding, removing and opening an item's attached files (spec
//! 2026-10-13-encrypted-attachments §5).

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::now_ms;
use havenkeys_core::attachment::{
    AttachmentChange, MAX_ATTACHMENTS_PER_ITEM, MAX_ATTACHMENT_BYTES,
};
use havenkeys_core::model::ItemOverview;
use havenkeys_core::Error;
use havenkeys_core::SecretBytes;
use havenkeys_sync_client::SyncError;
use std::sync::atomic::Ordering;
use uuid::Uuid;
use zeroize::Zeroizing;

impl HavenClient {
    /// Seal, upload and cache each file, then record them all on the item in
    /// one write. Uploads whose write is refused stay pending on the server
    /// until its sweep removes them.
    pub async fn add_attachments(
        &self,
        item_id: Uuid,
        files: Vec<(String, Zeroizing<Vec<u8>>)>,
    ) -> ClientResult<ItemOverview> {
        self.require_online()?;
        self.require_full()?;
        let (session, server) = (self.session()?, self.server()?);
        self.check_limits(item_id, &files)?;
        let now = now_ms();
        let mut metas = Vec::with_capacity(files.len());
        for (name, bytes) in &files {
            let sealed = self.vault()?.seal_attachment(&item_id, name, bytes, now)?;
            server
                .put_attachment(&session, sealed.meta.id, item_id, &sealed.blob)
                .await
                .map_err(|e| self.failed(e))?;
            self.vault()?
                .cache_attachment(&sealed.meta.id, &sealed.blob, now)?;
            metas.push(sealed.meta);
        }
        let staged =
            self.vault()?
                .stage_attachments(&item_id, AttachmentChange::Add(metas), now)?;
        self.push(staged).await?.ok_or_else(ClientError::internal)
    }

    /// Refuse a batch the core would refuse, before anything is uploaded.
    fn check_limits(
        &self,
        item_id: Uuid,
        files: &[(String, Zeroizing<Vec<u8>>)],
    ) -> ClientResult<()> {
        if files
            .iter()
            .any(|(_, bytes)| bytes.len() > MAX_ATTACHMENT_BYTES)
        {
            return Err(Error::InvalidInput("Files can be up to 5 MB").into());
        }
        // A trashed item is not listed; the core still checks it on staging.
        let held = self
            .vault()?
            .get_item(&item_id)
            .map_or(0, |o| o.attachments.len());
        if held + files.len() > MAX_ATTACHMENTS_PER_ITEM {
            return Err(Error::InvalidInput("An item holds up to 20 files").into());
        }
        Ok(())
    }

    pub async fn remove_attachment(
        &self,
        item_id: Uuid,
        attachment_id: Uuid,
    ) -> ClientResult<ItemOverview> {
        self.require_online()?;
        self.require_full()?;
        let staged = self.vault()?.stage_attachments(
            &item_id,
            AttachmentChange::Remove(attachment_id),
            now_ms(),
        )?;
        let updated = self.push(staged).await?.ok_or_else(ClientError::internal)?;
        self.prune_attachment_cache();
        Ok(updated)
    }

    /// The file's bytes: this device's sealed copy if it opens, else a
    /// download. Only a copy that opened is kept, so a bad one never sticks.
    pub async fn attachment_bytes(
        &self,
        item_id: Uuid,
        attachment_id: Uuid,
    ) -> ClientResult<SecretBytes> {
        let meta = self.vault()?.attachment_meta(&item_id, &attachment_id)?;
        let cached = self.vault()?.cached_attachment(&attachment_id)?;
        if let Some(blob) = cached {
            let opened = self.vault()?.open_attachment(&item_id, &meta, &blob);
            match opened {
                Ok(plain) => return Ok(plain),
                Err(_) => self.vault()?.evict_attachment(&attachment_id)?,
            }
        }
        let blob = self.download_attachment(item_id, attachment_id).await?;
        let plain = self
            .vault()?
            .open_attachment(&item_id, &meta, &blob)
            .map_err(|_| ClientError::attachment_damaged())?;
        self.vault()?
            .cache_attachment(&attachment_id, &blob, now_ms())?;
        Ok(plain)
    }

    async fn download_attachment(&self, item_id: Uuid, id: Uuid) -> ClientResult<Vec<u8>> {
        if !self.is_online() {
            return Err(ClientError::attachment_offline());
        }
        let (session, server) = (self.session()?, self.server()?);
        let (owner, blob) = match server.get_attachment(&session, id).await {
            Ok(found) => found,
            // The server withheld or deleted a file the item still lists.
            Err(SyncError::Refused("not found")) => return Err(ClientError::attachment_damaged()),
            Err(e) => return Err(self.failed(e)),
        };
        if owner != item_id {
            return Err(ClientError::attachment_damaged());
        }
        Ok(blob)
    }

    /// The most sealed copies this device keeps. Only a cache: files missing
    /// from it are downloaded again.
    pub fn set_attachment_cache_cap(&self, bytes: u64) {
        self.attachment_cache_cap.store(bytes, Ordering::Relaxed);
    }

    /// Forget copies nothing needs and shrink to the cap. Best effort: the
    /// cache never fails the operation it runs after.
    pub(crate) fn prune_attachment_cache(&self) {
        let cap = self.attachment_cache_cap.load(Ordering::Relaxed);
        if let Ok(vault) = self.vault() {
            let _ = vault.prune_attachment_cache(cap);
        }
    }
}
