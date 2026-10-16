//! A login's or secure note's custom fields in the desktop app (specs
//! 2026-09-30-login-custom-fields §5.1, 2026-10-16-secure-note-fields §5.1).
//!
//! The core owns the fields and their rules. The renderer gets the view
//! (Password and OTP reduced to flags) when a login or note opens; a Password
//! value comes one at a time on an explicit reveal; copies and URL opens
//! happen here, from the vault, and never take a value from the renderer.

use crate::commands::{copy_from_vault, CopyResult};
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::custom_field::{self, AddressPart, SectionView};
use havenkeys_core::totp::TotpCode;
use havenkeys_core::SecretString;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn item_fields(state: State<'_, AppState>, id: Uuid) -> CmdResult<Vec<SectionView>> {
    // No `touch()`: loaded whenever a login or note is shown, including one the
    // browser extension opened (DT2, see `commands::reveal_secret`).
    Ok(custom_field::views(state.vault()?.item_sections(&id)?))
}

#[tauri::command]
pub fn reveal_item_field(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.item_field(&id, &field_id)?.concealed()?)
}

#[tauri::command]
pub fn item_field_totp(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
) -> CmdResult<TotpCode> {
    // No `touch()`: refreshed on a timer, like `get_totp_code`.
    Ok(state
        .vault()?
        .item_field(&id, &field_id)?
        .totp_code(AppState::unix_seconds())?)
}

#[tauri::command]
pub fn copy_item_field(
    state: State<'_, AppState>,
    id: Uuid,
    field_id: Uuid,
    part: Option<AddressPart>,
) -> CmdResult<CopyResult> {
    copy_from_vault(&state, |v| {
        v.item_field(&id, &field_id)?
            .copy_value(part, AppState::unix_seconds())
    })
}

/// Open a URL field in the default browser. The renderer names the field;
/// Rust opens only that field's saved address, normalised again (http(s)
/// only), like `open_website`.
#[tauri::command]
pub fn open_item_field_url(state: State<'_, AppState>, id: Uuid, field_id: Uuid) -> CmdResult<()> {
    state.touch();
    let target = state.vault()?.item_field(&id, &field_id)?.url()?;
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())
}
