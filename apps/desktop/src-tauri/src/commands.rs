//! Tauri commands: the complete renderer → core interface.
//!
//! Every command here must also be listed in `build.rs` (which generates the
//! permission set) and in `capabilities/main.json`. Keep the three in sync.
//!
//! Rules:
//! * Commands never log arguments or results.
//! * Secrets are returned only by `reveal_secret` and `get_totp_code`, and
//!   only one field per call. Copying happens in Rust.
//! * All validation happens in the core; nothing here trusts the renderer.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::generator::{self, GeneratedPassword, GeneratorOptions};
use havenkeys_core::model::{ItemInput, ItemOverview, SecretField, Settings};
use havenkeys_core::totp::TotpCode;
use havenkeys_core::vault::{self, VaultStatus};
use havenkeys_core::SecretString;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

const DEFAULT_CLIPBOARD_CLEAR_SECS: u32 = 30;

// ------------------------------------------------------------------ lifecycle

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> CmdResult<VaultStatus> {
    Ok(state.vault()?.status()?)
}

#[tauri::command]
pub async fn create_vault(app: AppHandle, password: SecretString) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    if state.vault()?.status()?.vault_exists {
        return Err(havenkeys_core::Error::VaultExists.into());
    }
    vault::check_new_master_password(&password)?;
    // Argon2id runs off the async runtime and without holding the vault lock.
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        vault::prepare_new_vault(&password, KdfParams::generate()?, AppState::now_ms())
    })
    .await
    .map_err(|_| CmdError::internal())??;

    let mut v = state.vault()?;
    v.create_vault(prepared)?;
    let minutes = v.settings()?.auto_lock_minutes;
    let status = v.status()?;
    // Arm before releasing the vault lock so the auto-lock thread can never
    // tick against the previous session's timestamps.
    state.arm_auto_lock(minutes);
    drop(v);
    Ok(status)
}

#[tauri::command]
pub async fn unlock_vault(app: AppHandle, password: SecretString) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    let ticket = state.vault()?.begin_unlock()?;
    let joined = tauri::async_runtime::spawn_blocking(move || {
        let key = ticket.derive(&password);
        (ticket, key)
    })
    .await;
    let (ticket, key) = match joined {
        Ok(v) => v,
        Err(_) => {
            // The KDF task died; make sure we do not stay in UNLOCKING.
            state.lock(&app, "error");
            return Err(CmdError::internal());
        }
    };

    let mut v = state.vault()?;
    v.finish_unlock(ticket, key)?;
    let minutes = v.settings()?.auto_lock_minutes;
    let status = v.status()?;
    // Arm before releasing the vault lock so the auto-lock thread can never
    // tick against the previous session's timestamps.
    state.arm_auto_lock(minutes);
    drop(v);
    Ok(status)
}

#[tauri::command]
pub fn lock_vault(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.lock(&app, "user");
    Ok(())
}

#[tauri::command]
pub async fn change_master_password(
    app: AppHandle,
    current: SecretString,
    new: SecretString,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    vault::check_new_master_password(&new)?;
    let kdf = KdfParams::generate()?;
    let ticket = state.vault()?.begin_rekey()?;
    // Both Argon2id runs happen without the vault lock, so locking (button,
    // auto-lock, window close) is never delayed; the commit re-checks the
    // lock epoch and the header.
    let (ticket, rekeyed) = tauri::async_runtime::spawn_blocking(move || {
        let rekeyed = ticket.derive(&current, &new, kdf);
        (ticket, rekeyed)
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let mut v = state.vault()?;
    v.commit_rekey(ticket, rekeyed)?;
    Ok(())
}

#[tauri::command]
pub fn record_activity(state: State<'_, AppState>) {
    state.touch();
}

// ------------------------------------------------------------------ items

#[tauri::command]
pub fn list_items(
    state: State<'_, AppState>,
    query: Option<String>,
) -> CmdResult<Vec<ItemOverview>> {
    state.touch();
    let v = state.vault()?;
    Ok(match query {
        Some(q) => v.search(&q)?,
        None => v.list_items()?,
    })
}

#[tauri::command]
pub fn get_item(state: State<'_, AppState>, id: Uuid) -> CmdResult<ItemOverview> {
    state.touch();
    Ok(state.vault()?.get_item(&id)?)
}

#[tauri::command]
pub fn reveal_secret(
    state: State<'_, AppState>,
    id: Uuid,
    field: SecretField,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.reveal(&id, field)?)
}

#[tauri::command]
pub fn get_totp_code(state: State<'_, AppState>, id: Uuid) -> CmdResult<TotpCode> {
    // No `touch()`: the UI refreshes codes on a timer, which is not user
    // activity and must not keep the vault from auto-locking.
    Ok(state.vault()?.totp_code(&id, AppState::unix_seconds())?)
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyField {
    Username,
    Password,
    Totp,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    clear_after_seconds: u32,
}

#[tauri::command]
pub fn copy_secret(
    state: State<'_, AppState>,
    id: Uuid,
    field: CopyField,
) -> CmdResult<CopyResult> {
    state.touch();
    let (value, seconds) = {
        let v = state.vault()?;
        let seconds = v.settings()?.clipboard_clear_seconds;
        let value = match field {
            CopyField::Username => SecretString::new(
                v.get_item(&id)?
                    .username
                    .clone()
                    .ok_or(havenkeys_core::Error::NotFound)?,
            ),
            CopyField::Password => v.reveal(&id, SecretField::Password)?,
            CopyField::Totp => v.totp_code(&id, AppState::unix_seconds())?.code,
        };
        (value, seconds)
    };
    state
        .clipboard
        .copy(value.expose(), Duration::from_secs(u64::from(seconds)))
        .map_err(|_| CmdError::clipboard())?;
    Ok(CopyResult {
        clear_after_seconds: seconds,
    })
}

#[tauri::command]
pub fn create_item(state: State<'_, AppState>, input: ItemInput) -> CmdResult<ItemOverview> {
    state.touch();
    Ok(state.vault()?.create_item(input, AppState::now_ms())?)
}

#[tauri::command]
pub fn update_item(
    state: State<'_, AppState>,
    id: Uuid,
    input: ItemInput,
) -> CmdResult<ItemOverview> {
    state.touch();
    Ok(state.vault()?.update_item(&id, input, AppState::now_ms())?)
}

#[tauri::command]
pub fn delete_item(state: State<'_, AppState>, id: Uuid) -> CmdResult<()> {
    state.touch();
    Ok(state.vault()?.delete_item(&id)?)
}

// ------------------------------------------------------------------ generator

#[tauri::command]
pub fn generate_password(
    state: State<'_, AppState>,
    options: GeneratorOptions,
) -> CmdResult<GeneratedPassword> {
    state.touch();
    Ok(generator::generate(&options)?)
}

/// Copy a value the renderer already holds (a freshly generated password)
/// with the same auto-clear behaviour as vault secrets.
#[tauri::command]
pub fn copy_generated_password(
    state: State<'_, AppState>,
    value: SecretString,
) -> CmdResult<CopyResult> {
    state.touch();
    if value.is_empty() || value.char_len() > generator::MAX_LENGTH {
        return Err(havenkeys_core::Error::InvalidInput("nothing to copy").into());
    }
    let seconds = state
        .vault()?
        .settings()
        .map(|s| s.clipboard_clear_seconds)
        .unwrap_or(DEFAULT_CLIPBOARD_CLEAR_SECS);
    state
        .clipboard
        .copy(value.expose(), Duration::from_secs(u64::from(seconds)))
        .map_err(|_| CmdError::clipboard())?;
    Ok(CopyResult {
        clear_after_seconds: seconds,
    })
}

// ------------------------------------------------------------------ settings

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    state.touch();
    Ok(state.vault()?.settings()?)
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<Settings> {
    state.touch();
    let mut v = state.vault()?;
    v.update_settings(settings)?;
    drop(v);
    state.arm_auto_lock(settings.auto_lock_minutes);
    Ok(settings)
}
