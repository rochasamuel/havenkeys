//! Attachment commands (spec 2026-10-13-encrypted-attachments §5). The
//! renderer never supplies a path or bytes: Rust opens the native dialogs,
//! reads and writes the files, and hands the renderer only an item's updated
//! overview, a yes/no, or the bytes of an image it may show.

use crate::export::write_private;
use crate::state::{AppState, CmdError, CmdResult, ITEMS_CHANGED_EVENT};
use havenkeys_core::attachment::{
    is_previewable, sanitize_name, sniff, AttachmentMeta, MAX_ATTACHMENTS_PER_ITEM,
    MAX_ATTACHMENT_BYTES,
};
use havenkeys_core::model::ItemOverview;
use havenkeys_core::Error;
use std::io::Read;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use zeroize::Zeroizing;

/// The file at `path`, refusing anything over the attachment limit. Reads
/// stop one byte past it, so a file that grows cannot fill memory.
fn read_limited(path: &Path) -> CmdResult<Zeroizing<Vec<u8>>> {
    let file = std::fs::File::open(path).map_err(|_| CmdError::file())?;
    let mut buf = Zeroizing::new(Vec::new());
    file.take(MAX_ATTACHMENT_BYTES as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| CmdError::file())?;
    if buf.len() > MAX_ATTACHMENT_BYTES {
        return Err(Error::InvalidInput("Files can be up to 5 MB").into());
    }
    Ok(buf)
}

/// Refuse a pick the item has no room for, before any file is read.
fn ensure_room(held: usize, picked: usize) -> CmdResult<()> {
    if held + picked > MAX_ATTACHMENTS_PER_ITEM {
        return Err(Error::InvalidInput("An item holds up to 20 files").into());
    }
    Ok(())
}

/// The name a saved copy is offered under: sanitized again, since the record
/// came from the vault but the name ends up on disk.
fn save_name(meta: &AttachmentMeta) -> String {
    sanitize_name(&meta.name)
}

/// Only raster images are handed to the renderer, judged by their bytes.
fn ensure_previewable(bytes: &[u8]) -> CmdResult<()> {
    if is_previewable(sniff(bytes)) {
        Ok(())
    } else {
        Err(Error::InvalidInput("this file cannot be previewed").into())
    }
}

/// Let the user pick files and attach them to the item. `None` if the
/// picker was cancelled.
#[tauri::command]
pub async fn add_attachments(app: AppHandle, item_id: Uuid) -> CmdResult<Option<ItemOverview>> {
    let held = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
        // Checked before the picker opens, as for an import.
        state.require_online()?;
        state.require_full()?;
        // A trashed item is not listed; the client checks it again.
        let vault = state.client().vault()?;
        vault.get_item(&item_id).map_or(0, |o| o.attachments.len())
    };
    let handle = app.clone();
    let files = tauri::async_runtime::spawn_blocking(move || -> CmdResult<_> {
        let Some(picked) = handle
            .dialog()
            .file()
            .set_title("Attach files")
            .blocking_pick_files()
        else {
            return Ok(None);
        };
        ensure_room(held, picked.len())?;
        picked
            .into_iter()
            .map(|file| {
                let path = file.into_path().map_err(|_| CmdError::file())?;
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                Ok((name, read_limited(&path)?))
            })
            .collect::<CmdResult<Vec<_>>>()
            .map(Some)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    let Some(files) = files.filter(|f| !f.is_empty()) else {
        return Ok(None);
    };
    let item = app
        .state::<AppState>()
        .client()
        .clone()
        .add_attachments(item_id, files)
        .await?;
    let _ = app.emit(ITEMS_CHANGED_EVENT, ());
    Ok(Some(item))
}

/// Save a decrypted copy where the user chooses. `false` if the save dialog
/// was cancelled.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    item_id: Uuid,
    attachment_id: Uuid,
) -> CmdResult<bool> {
    let client = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
        state.client().clone()
    };
    let meta = client.vault()?.attachment_meta(&item_id, &attachment_id)?;
    // Fetched before the dialog, so being offline is reported first.
    let bytes = client.attachment_bytes(item_id, attachment_id).await?;
    let name = save_name(&meta);
    let handle = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .file()
            .set_title("Save attachment")
            .set_file_name(name)
            .blocking_save_file()
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    let path = picked.into_path().map_err(|_| CmdError::file())?;
    // A lock while the dialog was open drops the plaintext unwritten.
    app.state::<AppState>().require_unlocked()?;
    tauri::async_runtime::spawn_blocking(move || write_private(&path, bytes.expose()))
        .await
        .map_err(|_| CmdError::internal())??;
    Ok(true)
}

/// The bytes of an attached image, for the renderer to show. Anything that
/// is not a raster image is refused here, not left to the renderer.
#[tauri::command]
pub async fn preview_attachment(
    app: AppHandle,
    item_id: Uuid,
    attachment_id: Uuid,
) -> CmdResult<tauri::ipc::Response> {
    let client = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
        state.client().clone()
    };
    let bytes = client.attachment_bytes(item_id, attachment_id).await?;
    ensure_previewable(bytes.expose())?;
    Ok(tauri::ipc::Response::new(bytes.expose().to_vec()))
}

#[tauri::command]
pub async fn remove_attachment(
    app: AppHandle,
    item_id: Uuid,
    attachment_id: Uuid,
) -> CmdResult<ItemOverview> {
    app.state::<AppState>().touch();
    let item = app
        .state::<AppState>()
        .client()
        .clone()
        .remove_attachment(item_id, attachment_id)
        .await?;
    let _ = app.emit(ITEMS_CHANGED_EVENT, ());
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(name: &str) -> AttachmentMeta {
        AttachmentMeta {
            id: Uuid::new_v4(),
            name: name.into(),
            mime: "application/octet-stream".into(),
            size: 0,
            blob_sha256: [0; 32],
            created_at: 0,
            extra: Default::default(),
        }
    }

    #[test]
    fn read_limited_refuses_over_5_mb() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        std::fs::write(&path, vec![1u8; MAX_ATTACHMENT_BYTES]).unwrap();
        assert_eq!(read_limited(&path).unwrap().len(), MAX_ATTACHMENT_BYTES);
        std::fs::write(&path, vec![1u8; MAX_ATTACHMENT_BYTES + 1]).unwrap();
        assert!(read_limited(&path).is_err());
    }

    #[test]
    fn save_name_is_sanitized_again() {
        assert_eq!(save_name(&meta("../\u{202E}gpj.exe")), "gpj.exe");
    }

    #[test]
    fn a_pick_over_the_item_limit_is_refused_before_reading() {
        assert!(ensure_room(19, 1).is_ok());
        let err = ensure_room(19, 2).unwrap_err();
        assert!(err.message.contains("An item holds up to 20 files"));
        assert!(ensure_room(0, MAX_ATTACHMENTS_PER_ITEM + 1).is_err());
    }

    #[test]
    fn preview_refuses_non_images() {
        assert!(ensure_previewable(b"%PDF-1.7 rest").is_err());
        assert!(ensure_previewable(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_err());
        assert!(ensure_previewable(b"\x89PNG\r\n\x1a\nrest").is_ok());
    }
}
