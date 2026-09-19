//! Secret Key, Emergency Kit, sync folder, and joining a vault from another
//! device. As with imports, the renderer never supplies a path: Rust opens
//! the native folder picker and remembers the choice.

use crate::state::{AppState, CmdError, CmdResult};
use crate::sync;
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::KeyScheme;
use havenkeys_core::sync::{folder, prepare_join, SyncReport};
use havenkeys_core::vault::VaultStatus;
use havenkeys_core::SecretString;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

fn folder_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

fn pick_folder(app: &AppHandle, title: &'static str) -> Option<PathBuf> {
    app.dialog()
        .file()
        .set_title(title)
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
}

async fn pick_folder_async(app: &AppHandle, title: &'static str) -> CmdResult<Option<PathBuf>> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || pick_folder(&handle, title))
        .await
        .map_err(|_| CmdError::internal())
}

fn device_error() -> CmdError {
    CmdError {
        code: "device",
        message: "Could not save this device's settings.".into(),
    }
}

// ------------------------------------------------------------------ status

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    /// "password_only" or "password_and_secret_key"; null without a vault.
    key_scheme: Option<KeyScheme>,
    /// The vault needs a Secret Key and this device does not have it: the
    /// unlock screen must ask for it.
    needs_secret_key: bool,
    /// Folder name only (no full path).
    sync_folder: Option<String>,
    last_sync: Option<sync::SyncStatus>,
}

/// Safe to call while locked: reveals no secrets.
#[tauri::command]
pub fn device_status(state: State<'_, AppState>) -> CmdResult<DeviceStatus> {
    let key_scheme = state.vault()?.key_scheme()?;
    let device = state.device.lock().map_err(|_| CmdError::internal())?;
    Ok(DeviceStatus {
        key_scheme,
        needs_secret_key: key_scheme == Some(KeyScheme::PasswordAndSecretKey)
            && device.secret_key().is_none(),
        sync_folder: device.sync_folder.as_deref().map(folder_name),
        last_sync: state.sync.last.lock().ok().and_then(|l| l.clone()),
    })
}

// ------------------------------------------------------------------ Emergency Kit

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyKit {
    secret_key: SecretString,
    vault_id: Uuid,
    created_at: i64,
    /// QR code for the mobile app: `size` × `size` modules, row-major, true = dark.
    qr_size: usize,
    qr_modules: Vec<bool>,
}

/// The Emergency Kit: the Secret Key plus a QR code of it. Only while
/// unlocked, and only on explicit request.
#[tauri::command]
pub fn get_emergency_kit(state: State<'_, AppState>) -> CmdResult<EmergencyKit> {
    state.touch();
    let (vault_id, created_at) = {
        let v = state.vault()?;
        if !v.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        (
            v.vault_id()?.ok_or(havenkeys_core::Error::NoVault)?,
            v.created_at()?.unwrap_or(0),
        )
    };
    let secret_key = state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .secret_key_text()
        .cloned()
        .ok_or(havenkeys_core::Error::NotFound)?;
    // The mobile app scans this to set itself up (docs/sync.md, Emergency Kit).
    let payload = SecretString::new(format!(
        "havenkeys://kit/v1?vault={vault_id}&key={}",
        secret_key.expose()
    ));
    let code = QrCode::with_error_correction_level(payload.expose().as_bytes(), EcLevel::M)
        .map_err(|_| CmdError::internal())?;
    let qr_modules = code
        .to_colors()
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    Ok(EmergencyKit {
        secret_key,
        vault_id,
        created_at,
        qr_size: code.width(),
        qr_modules,
    })
}

/// Protect an existing password-only vault with a Secret Key (key scheme
/// 1 → 2). Needs the master password. The new key is saved on this device;
/// the UI then shows the Emergency Kit.
#[tauri::command]
pub async fn setup_secret_key(app: AppHandle, password: SecretString) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let secret_key = SecretKey::generate()?;
    let kdf = KdfParams::generate()?;
    let ticket = state.vault()?.begin_rekey()?;
    let key_text = secret_key.to_text();
    let (ticket, rekeyed) = tauri::async_runtime::spawn_blocking(move || {
        let r = ticket.derive_secret_key_upgrade(&password, &secret_key, kdf);
        (ticket, r)
    })
    .await
    .map_err(|_| CmdError::internal())?;
    // Save the key on this device before committing, so a crash in between
    // can never leave a vault whose Secret Key exists nowhere.
    let parsed = SecretKey::parse(key_text.expose())?;
    state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .set_secret_key(&parsed)
        .map_err(|_| device_error())?;
    state.vault()?.commit_rekey(ticket, rekeyed)?;
    Ok(())
}

// ------------------------------------------------------------------ sync folder

/// Choose the folder to sync through (a folder OneDrive, Dropbox, Google
/// Drive or Syncthing keeps in step). Returns the folder name, or null if the
/// picker was cancelled.
#[tauri::command]
pub async fn choose_sync_folder(app: AppHandle) -> CmdResult<Option<String>> {
    let state = app.state::<AppState>();
    state.touch();
    {
        let v = state.vault()?;
        if !v.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        if v.key_scheme()? != Some(KeyScheme::PasswordAndSecretKey) {
            return Err(
                havenkeys_core::Error::InvalidInput("set up a Secret Key before syncing").into(),
            );
        }
    }
    let Some(path) = pick_folder_async(&app, "Choose a folder to sync through").await? else {
        return Ok(None);
    };
    let name = folder_name(&path);
    state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .set_sync_folder(Some(path))
        .map_err(|_| device_error())?;
    Ok(Some(name))
}

#[tauri::command]
pub async fn sync_now(app: AppHandle) -> CmdResult<Option<SyncReport>> {
    app.state::<AppState>().touch();
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || sync::run(&handle))
        .await
        .map_err(|_| CmdError::internal())?
}

/// Stop syncing on this device. The folder's files are left alone.
#[tauri::command]
pub fn stop_sync(state: State<'_, AppState>) -> CmdResult<()> {
    state.touch();
    state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .set_sync_folder(None)
        .map_err(|_| device_error())
}

// ------------------------------------------------------------------ joining

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinFolder {
    folder: String,
}

/// New device: pick the sync folder that holds the vault. Only possible
/// while there is no vault on this computer.
#[tauri::command]
pub async fn pick_join_folder(app: AppHandle) -> CmdResult<Option<JoinFolder>> {
    let state = app.state::<AppState>();
    if state.vault()?.status()?.vault_exists {
        return Err(havenkeys_core::Error::VaultExists.into());
    }
    let Some(root) = pick_folder_async(&app, "Choose your HavenKeys sync folder").await? else {
        return Ok(None);
    };
    let vaults = folder::list_vaults(&root);
    let [vault_id] = vaults.as_slice() else {
        return Err(CmdError {
            code: "no_synced_vault",
            message: if vaults.is_empty() {
                "No HavenKeys vault was found in that folder.".into()
            } else {
                "That folder holds more than one vault.".into()
            },
        });
    };
    let name = folder_name(&root);
    *state
        .pending_join
        .lock()
        .map_err(|_| CmdError::internal())? = Some((root, *vault_id));
    Ok(Some(JoinFolder { folder: name }))
}

/// New device: unlock the synced vault with the master password and the
/// Secret Key from the Emergency Kit, then copy it here and keep it in sync.
#[tauri::command]
pub async fn join_synced_vault(
    app: AppHandle,
    password: SecretString,
    secret_key: SecretString,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    if state.vault()?.status()?.vault_exists {
        return Err(havenkeys_core::Error::VaultExists.into());
    }
    let (root, vault_id) = state
        .pending_join
        .lock()
        .map_err(|_| CmdError::internal())?
        .clone()
        .ok_or(havenkeys_core::Error::NotFound)?;
    let secret_key = SecretKey::parse(secret_key.expose())?;
    let key_text = secret_key.to_text();
    let dir = folder::vault_dir(&root, vault_id);
    let prepared = tauri::async_runtime::spawn_blocking(move || -> CmdResult<_> {
        let header = folder::read_header(&dir).map_err(|_| sync::folder_error())?;
        Ok(prepare_join(&header, &password, &secret_key)?)
    })
    .await
    .map_err(|_| CmdError::internal())??;

    {
        let mut device = state.device.lock().map_err(|_| CmdError::internal())?;
        device
            .set_secret_key(&SecretKey::parse(key_text.expose())?)
            .map_err(|_| device_error())?;
        device
            .set_sync_folder(Some(root))
            .map_err(|_| device_error())?;
    }
    let status = {
        let mut v = state.vault()?;
        v.create_vault(prepared)?;
        let minutes = v.settings()?.auto_lock_minutes;
        state.arm_auto_lock(minutes);
        state.notify_unlocked();
        v.status()?
    };
    *state
        .pending_join
        .lock()
        .map_err(|_| CmdError::internal())? = None;
    // Pull the items now rather than in a minute.
    let handle = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || sync::run(&handle)).await;
    Ok(status)
}
