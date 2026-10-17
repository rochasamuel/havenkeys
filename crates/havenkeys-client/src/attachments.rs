//! Saving, removing and opening an item's attached files (spec
//! 2026-10-13-encrypted-attachments §5).

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::now_ms;
use havenkeys_core::attachment::{
    AttachmentChange, AttachmentMeta, FileEdits, MAX_ATTACHMENTS_PER_ITEM, MAX_ATTACHMENT_BYTES,
};
use havenkeys_core::model::ItemOverview;
use havenkeys_core::vault::{StagedWrite, VaultService};
use havenkeys_core::Error;
use havenkeys_core::SecretBytes;
use havenkeys_sync_client::SyncError;
use std::sync::atomic::Ordering;
use uuid::Uuid;
use zeroize::Zeroizing;

/// A picked file waiting to be sealed with the item it is saved on.
pub struct NewFile {
    pub name: String,
    pub bytes: Zeroizing<Vec<u8>>,
}

impl HavenClient {
    /// Save an item together with its file edits in one write. Each file is
    /// sealed for `item_id`, uploaded and cached first; `stage` then builds
    /// the write from the edits. Nothing is sealed or sent when a file is too
    /// large or the item would end up with too many. An upload whose write is
    /// refused stays pending on the server until its sweep removes it.
    pub async fn save_with_files(
        &self,
        item_id: Uuid,
        added: Vec<NewFile>,
        removed: Vec<Uuid>,
        stage: impl FnOnce(&VaultService, FileEdits) -> havenkeys_core::Result<StagedWrite>,
    ) -> ClientResult<ItemOverview> {
        self.require_full()?;
        self.check_limits(item_id, &added, removed.len())?;
        let metas = self.upload_files(item_id, &added).await?;
        let files = FileEdits {
            added: metas,
            removed,
        };
        let staged = stage(&*self.vault()?, files)?;
        let files_changed = staged.attachment_ids.is_some();
        let saved = self.push(staged).await?.ok_or_else(ClientError::internal)?;
        if files_changed {
            self.prune_attachment_cache();
        }
        Ok(saved)
    }

    /// Refuse an edit the core would refuse, before anything is uploaded. The
    /// core checks again once the edit is staged.
    fn check_limits(&self, item_id: Uuid, added: &[NewFile], removed: usize) -> ClientResult<()> {
        if added.iter().any(|f| f.bytes.len() > MAX_ATTACHMENT_BYTES) {
            return Err(Error::InvalidInput("Files can be up to 5 MB").into());
        }
        // An item not yet created, or trashed (not listed), holds none here.
        let held = self
            .vault()?
            .get_item(&item_id)
            .map_or(0, |o| o.attachments.len());
        if (held + added.len()).saturating_sub(removed) > MAX_ATTACHMENTS_PER_ITEM {
            return Err(Error::InvalidInput("An item holds up to 20 files").into());
        }
        Ok(())
    }

    /// Seal, upload and cache each file.
    async fn upload_files(
        &self,
        item_id: Uuid,
        files: &[NewFile],
    ) -> ClientResult<Vec<AttachmentMeta>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        self.require_online()?;
        let (session, server) = (self.session()?, self.server()?);
        let now = now_ms();
        let mut metas = Vec::with_capacity(files.len());
        for file in files {
            let sealed = self
                .vault()?
                .seal_attachment(&item_id, &file.name, &file.bytes, now)?;
            server
                .put_attachment(&session, sealed.meta.id, item_id, &sealed.blob)
                .await
                .map_err(|e| self.failed(e))?;
            self.vault()?
                .cache_attachment(&sealed.meta.id, &sealed.blob, now)?;
            metas.push(sealed.meta);
        }
        Ok(metas)
    }

    /// Add files to an existing item, alone.
    pub async fn add_attachments(
        &self,
        item_id: Uuid,
        files: Vec<(String, Zeroizing<Vec<u8>>)>,
    ) -> ClientResult<ItemOverview> {
        let added = files
            .into_iter()
            .map(|(name, bytes)| NewFile { name, bytes })
            .collect();
        self.save_with_files(item_id, added, Vec::new(), |vault, files| {
            vault.stage_attachments(&item_id, AttachmentChange::Add(files.added), now_ms())
        })
        .await
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
                Err(e) if is_damaged(&e) => self.vault()?.evict_attachment(&attachment_id)?,
                Err(e) => return Err(e.into()),
            }
        }
        let blob = self.download_attachment(item_id, attachment_id).await?;
        let plain = self
            .vault()?
            .open_attachment(&item_id, &meta, &blob)
            .map_err(|e| {
                if is_damaged(&e) {
                    ClientError::attachment_damaged()
                } else {
                    e.into()
                }
            })?;
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

/// The bytes did not open: the file is damaged, not the vault locked.
fn is_damaged(e: &Error) -> bool {
    matches!(
        e,
        Error::Decryption | Error::Corrupted | Error::UnsupportedVersion
    )
}

#[cfg(test)]
mod tests {
    use super::is_damaged;
    use havenkeys_core::Error;

    /// Only the bytes failing to open mean a damaged file: a lock or a
    /// storage error mid-read must neither evict the copy nor call it
    /// damaged.
    #[test]
    fn a_lock_does_not_evict_or_report_damaged() {
        for e in [
            Error::Decryption,
            Error::Corrupted,
            Error::UnsupportedVersion,
        ] {
            assert!(is_damaged(&e), "{e:?}");
        }
        for e in [Error::Locked, Error::Storage, Error::NotFound] {
            assert!(!is_damaged(&e), "{e:?}");
        }
    }
}
