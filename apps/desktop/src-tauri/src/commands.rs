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

use crate::item_input::ItemInputWire;
use crate::qr_scan;
use crate::scan_slot::ScannedTotp;
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::generator::{self, GeneratedPassword, GeneratorOptions};
use havenkeys_core::model::{ItemInput, ItemOverview, SecretField, Settings, TrashEntry};
use havenkeys_core::password_strength::{self, PasswordStrength};
use havenkeys_core::sso::SsoProvider;
use havenkeys_core::totp::TotpCode;
use havenkeys_core::vault::{ProviderLogin, SsoAccount, StagedWrite, VaultService, VaultStatus};
use havenkeys_core::SecretString;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const DEFAULT_CLIPBOARD_CLEAR_SECS: u32 = 30;

// ------------------------------------------------------------------ lifecycle

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> CmdResult<VaultStatus> {
    Ok(state.vault()?.status()?)
}

/// Unlock. Every vault is account-bound: the account comes from the local
/// store (never from the renderer), and unlocking needs the Secret Key too.
#[tauri::command]
pub async fn unlock_vault(
    app: AppHandle,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let client = app.state::<AppState>().client().clone();
    client.unlock(password, secret_key).await
}

/// Pull now, instead of waiting for the periodic sync.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> CmdResult<havenkeys_core::sync::SyncReport> {
    app.state::<AppState>().touch();
    let client = app.state::<AppState>().client().clone();
    client.sync_now().await
}

/// Download the whole vault again.
///
/// For the one case a normal sync cannot fix: an item the server once served
/// in a form this device could not open stays stale forever, because the
/// cursor moved past it. Resetting the cursor re-reads everything.
#[tauri::command]
pub async fn resync_vault(app: AppHandle) -> CmdResult<havenkeys_core::sync::SyncReport> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        state.vault()?.reset_sync_cursor(AppState::now_ms())?;
    }
    let client = app.state::<AppState>().client().clone();
    client.sync_now().await
}

#[tauri::command]
pub fn lock_vault(state: State<'_, AppState>) -> CmdResult<()> {
    state.lock("user");
    Ok(())
}

/// Change the master password: server first, local second.
#[tauri::command]
pub async fn change_master_password(
    app: AppHandle,
    current: SecretString,
    new: SecretString,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.change_master_password(current, new).await
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
    // No `touch()`: the UI also refreshes this list from `vault://synced` and
    // `vault://items-changed`, which are driven by the sync thread and by the
    // browser extension, not by the user. Counting those as activity would let
    // a server that changes one item every minute hold the vault unlocked
    // forever. Real interaction — including typing in the search box — reaches
    // the timer through `record_activity`.
    let v = state.vault()?;
    Ok(match query {
        Some(q) => v.search(&q)?,
        None => v.list_items()?,
    })
}

#[tauri::command]
pub fn get_item(state: State<'_, AppState>, id: Uuid) -> CmdResult<ItemOverview> {
    // No `touch()`, for the same reason as `list_items`: a refresh driven by
    // sync or by the extension must not count as user activity.
    Ok(state.vault()?.get_item(&id)?)
}

#[tauri::command]
pub fn sso_accounts(
    state: State<'_, AppState>,
    provider: SsoProvider,
) -> CmdResult<Vec<SsoAccount>> {
    // Read-only, no secrets; no `touch()`, like `get_item`.
    Ok(state.vault()?.sso_accounts(provider)?)
}

#[tauri::command]
pub fn provider_login(state: State<'_, AppState>, id: Uuid) -> CmdResult<ProviderLogin> {
    Ok(state.vault()?.provider_login(&id)?)
}

#[tauri::command]
pub fn reveal_secret(
    state: State<'_, AppState>,
    id: Uuid,
    field: SecretField,
) -> CmdResult<SecretString> {
    // No `touch()`: the editor loads secrets when it opens, and an opening
    // the browser extension asks for (`open_item`) is not the user at this
    // machine (DT2). A click on reveal already counts through
    // `record_activity`.
    Ok(state.vault()?.reveal(&id, field)?)
}

/// When each previous password of a login was replaced (Unix ms, newest first).
#[tauri::command]
pub fn password_history(state: State<'_, AppState>, id: Uuid) -> CmdResult<Vec<i64>> {
    state.touch();
    Ok(state.vault()?.password_history(&id)?)
}

/// Public details of a login's passkeys. Never the private key.
#[tauri::command]
pub fn list_passkeys(
    state: State<'_, AppState>,
    id: Uuid,
) -> CmdResult<Vec<havenkeys_core::passkey::PasskeyInfo>> {
    // No `touch()`: reading must not reset auto-lock (see `list_items`).
    Ok(state.vault()?.list_passkeys(&id)?)
}

/// Remove one passkey from a login. A server write, like any edit.
#[tauri::command]
pub async fn delete_passkey(
    app: AppHandle,
    id: Uuid,
    credential_id: String,
) -> CmdResult<ItemOverview> {
    let staged = stage_write(&app, |state| {
        Ok(state
            .vault()?
            .stage_remove_passkey(&id, &credential_id, AppState::now_ms())?)
    })?;
    let item = app
        .state::<AppState>()
        .client()
        .clone()
        .push(staged)
        .await?
        .ok_or_else(CmdError::internal)?;
    let _ = app.emit(crate::state::ITEMS_CHANGED_EVENT, ());
    Ok(item)
}

#[tauri::command]
pub fn reveal_previous_password(
    state: State<'_, AppState>,
    id: Uuid,
    index: usize,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.reveal_previous_password(&id, index)?)
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
    pub(crate) clear_after_seconds: u32,
}

#[tauri::command]
pub fn copy_secret(
    state: State<'_, AppState>,
    id: Uuid,
    field: CopyField,
) -> CmdResult<CopyResult> {
    copy_from_vault(&state, |v| {
        Ok(match field {
            CopyField::Username => SecretString::new(
                v.get_item(&id)?
                    .username
                    .clone()
                    .ok_or(havenkeys_core::Error::NotFound)?,
            ),
            CopyField::Password => v.reveal(&id, SecretField::Password)?,
            CopyField::Totp => v.totp_code(&id, AppState::unix_seconds())?.code,
        })
    })
}

/// Copy one vault value: it and the vault's clipboard delay are read under
/// one guard, released before the clipboard is touched. Counts as activity.
pub(crate) fn copy_from_vault(
    state: &AppState,
    read: impl FnOnce(&VaultService) -> havenkeys_core::Result<SecretString>,
) -> CmdResult<CopyResult> {
    state.touch();
    let (value, seconds) = {
        let v = state.vault()?;
        let seconds = v.settings()?.clipboard_clear_seconds;
        (read(&v)?, seconds)
    };
    copy_to_clipboard(state, &value, seconds)
}

/// Put `value` on the clipboard, to be cleared after `seconds` if it is
/// still there.
pub(crate) fn copy_to_clipboard(
    state: &AppState,
    value: &SecretString,
    seconds: u32,
) -> CmdResult<CopyResult> {
    state
        .clipboard
        .copy(value.expose(), Duration::from_secs(u64::from(seconds)))
        .map_err(|_| CmdError::clipboard())?;
    Ok(CopyResult {
        clear_after_seconds: seconds,
    })
}

/// Open one of an item's saved websites in the default browser. The renderer
/// names the website, but only an address already saved on that item opens,
/// and only after it passes the same http(s) check as saving; a compromised
/// renderer cannot hand the OS an arbitrary URL or scheme.
#[tauri::command]
pub fn open_website(state: State<'_, AppState>, id: Uuid, url: String) -> CmdResult<()> {
    state.touch();
    let target = {
        let v = state.vault()?;
        let item = v.get_item(&id)?;
        let saved = item
            .urls
            .iter()
            .find(|rule| rule.url == url)
            .ok_or(havenkeys_core::Error::NotFound)?;
        havenkeys_core::model::normalize_url(&saved.url)?
    };
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())
}

/// Look for a TOTP setup QR code: the clipboard image first, then every
/// monitor. Returns a token and labels per code; the URI stays in Rust
/// (scan_slot.rs) until an item is saved with the token.
#[tauri::command]
pub async fn scan_totp_qr(app: AppHandle) -> CmdResult<Vec<ScannedTotp>> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
    }
    // Capture and decoding take a moment, and a Wayland portal waits for
    // the user: keep them off the main thread.
    let codes = tauri::async_runtime::spawn_blocking(|| {
        qr_scan::scan(qr_scan::clipboard_frame, qr_scan::screen_frames)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    app.state::<AppState>().store_totp_scan(codes)
}

/// Create an item: seal it under the vault lock, let the server assign its
/// revision, then record it locally. Nothing is stored until the server has
/// accepted it, so the replica is never ahead of the authority.
#[tauri::command]
pub async fn create_item(app: AppHandle, input: ItemInputWire) -> CmdResult<ItemOverview> {
    save_item(&app, input, |v, input| {
        v.stage_create(input, AppState::now_ms())
    })
    .await
}

#[tauri::command]
pub async fn update_item(
    app: AppHandle,
    id: Uuid,
    input: ItemInputWire,
) -> CmdResult<ItemOverview> {
    save_item(&app, input, |v, input| {
        v.stage_update(&id, input, AppState::now_ms())
    })
    .await
}

/// Delete moves the item to the Trash (spec 2026-10-08-trash). `false`: its
/// details did not open, so it was deleted for good instead.
#[tauri::command]
pub async fn trash_item(app: AppHandle, id: Uuid) -> CmdResult<bool> {
    let staged = stage_write(&app, |state| {
        Ok(state.vault()?.stage_trash(&id, AppState::now_ms())?)
    })?;
    let client = app.state::<AppState>().client().clone();
    Ok(client.push(staged).await?.is_some())
}

#[tauri::command]
pub async fn restore_item(app: AppHandle, id: Uuid) -> CmdResult<ItemOverview> {
    let staged = stage_write(&app, |state| {
        Ok(state.vault()?.stage_restore_trashed(&id)?)
    })?;
    let client = app.state::<AppState>().client().clone();
    client.push(staged).await?.ok_or_else(CmdError::internal)
}

#[tauri::command]
pub async fn purge_item(app: AppHandle, id: Uuid) -> CmdResult<()> {
    let staged = stage_write(&app, |state| Ok(state.vault()?.stage_purge(&id)?))?;
    let client = app.state::<AppState>().client().clone();
    client.push(staged).await.map(|_| ())
}

#[tauri::command]
pub async fn empty_trash(app: AppHandle) -> CmdResult<usize> {
    let (staged, client) = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        state.require_full()?;
        let staged = state.vault()?.stage_empty_trash()?;
        (staged, state.client().clone())
    };
    client.push_batches(staged).await
}

/// A read like `list_items`: no `touch()`.
#[tauri::command]
pub fn list_trash(state: State<'_, AppState>) -> CmdResult<Vec<TrashEntry>> {
    Ok(state.vault()?.list_trash(AppState::now_ms())?)
}

/// Stage a change for the server. Counts as activity and needs a session;
/// the vault guard ends with `stage`, before anything is sent.
fn stage_write(
    app: &AppHandle,
    stage: impl FnOnce(&AppState) -> CmdResult<StagedWrite>,
) -> CmdResult<StagedWrite> {
    let state = app.state::<AppState>();
    state.touch();
    state.require_online()?;
    state.require_full()?;
    stage(&state)
}

/// Create or update an item: seal it with `stage`, let the server assign
/// its revision, then record it. A scanned TOTP code it used is let go once
/// the save has gone through.
async fn save_item(
    app: &AppHandle,
    input: ItemInputWire,
    stage: impl FnOnce(&mut VaultService, ItemInput) -> havenkeys_core::Result<StagedWrite>,
) -> CmdResult<ItemOverview> {
    let uses_scan = input.uses_scan();
    let staged = stage_write(app, |state| {
        let input = state.resolve_item_input(input)?;
        let staged = stage(&mut *state.vault()?, input)?;
        Ok(staged)
    })?;
    let client = app.state::<AppState>().client().clone();
    let saved = client.push(staged).await?.ok_or_else(CmdError::internal)?;
    if uses_scan {
        app.state::<AppState>().clear_totp_scan();
    }
    Ok(saved)
}

// ------------------------------------------------------------------ generator

/// Save the generator tab's policy. The browser extension's "Generate strong
/// password" uses it too.
#[tauri::command]
pub fn set_generator_options(
    state: State<'_, AppState>,
    options: GeneratorOptions,
) -> CmdResult<GeneratorOptions> {
    state.touch();
    let mut v = state.vault()?;
    let mut settings = v.settings()?;
    settings.generator = options;
    v.update_settings(settings)?;
    Ok(options)
}

#[tauri::command]
pub fn generate_password(
    state: State<'_, AppState>,
    options: GeneratorOptions,
) -> CmdResult<GeneratedPassword> {
    state.touch();
    Ok(generator::generate(&options)?)
}

/// Strength of a master password being chosen, scored by zxcvbn in the
/// core. The renderer sends the draft after each pause in typing; the core
/// scores it and drops it. `user_inputs` (the account's email) count as
/// easy guesses. No `touch()`: there is no vault yet.
#[tauri::command]
pub fn estimate_master_password(
    password: SecretString,
    user_inputs: Vec<String>,
) -> CmdResult<PasswordStrength> {
    if user_inputs.len() > 4 || user_inputs.iter().any(|s| s.len() > 254) {
        return Err(havenkeys_core::Error::InvalidInput("too many user inputs").into());
    }
    let inputs: Vec<&str> = user_inputs.iter().map(String::as_str).collect();
    Ok(password_strength::estimate(password.expose(), &inputs))
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
    copy_to_clipboard(&state, &value, seconds)
}

// ------------------------------------------------------------------ settings

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    // No `touch()`: read whenever a view opens (DT2, see `reveal_secret`).
    Ok(state.vault()?.settings()?)
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, mut settings: Settings) -> CmdResult<Settings> {
    state.touch();
    let mut v = state.vault()?;
    // The generator tab saves its own policy (set_generator_options); a
    // Settings screen holding an older copy must not undo it.
    settings.generator = v.settings()?.generator;
    v.update_settings(settings)?;
    drop(v);
    state.arm_auto_lock(settings.auto_lock_minutes);
    Ok(settings)
}
