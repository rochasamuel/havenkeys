//! A login's custom fields in the desktop app (spec
//! 2026-09-30-login-custom-fields §5.1).
//!
//! The core owns the fields and their rules. The renderer gets the view
//! (Password and OTP reduced to flags) when a login opens; a Password value
//! comes one at a time on an explicit reveal; copies and URL opens happen
//! here, from the vault, and never take a value from the renderer.

use crate::commands::{copy_from_vault, CopyResult};
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::custom_field::{self, AddressPart, SectionView};
use havenkeys_core::totp::TotpCode;
use havenkeys_core::SecretString;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn login_fields(state: State<'_, AppState>, id: Uuid) -> CmdResult<Vec<SectionView>> {
    // No `touch()`: loaded whenever a login is shown, including one the
    // browser extension opened (DT2, see `commands::reveal_secret`).
    Ok(custom_field::views(state.vault()?.login_sections(&id)?))
}

#[tauri::command]
pub fn reveal_login_field(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.login_field(&id, &field_id)?.concealed()?)
}

#[tauri::command]
pub fn login_field_totp(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
) -> CmdResult<TotpCode> {
    // No `touch()`: refreshed on a timer, like `get_totp_code`.
    Ok(state
        .vault()?
        .login_field(&id, &field_id)?
        .totp_code(AppState::unix_seconds())?)
}

#[tauri::command]
pub fn copy_login_field(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
    part: Option<AddressPart>,
) -> CmdResult<CopyResult> {
    copy_from_vault(&state, |v| {
        v.login_field(&id, &field_id)?
            .copy_value(part, AppState::unix_seconds())
    })
}

/// Open a URL field in the default browser. The renderer names the field;
/// Rust opens only that field's saved address, normalised again (http(s)
/// only), like `open_website`.
#[tauri::command]
pub fn open_login_field_url(state: State<'_, AppState>, id: Uuid, field_id: Uuid) -> CmdResult<()> {
    state.touch();
    let target = state.vault()?.login_field(&id, &field_id)?.url()?;
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())
}
