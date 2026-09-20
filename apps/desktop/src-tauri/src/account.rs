//! Secret Key and Emergency Kit.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::store::KeyScheme;
use havenkeys_core::SecretString;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

// ------------------------------------------------------------------ status

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    /// "account_bound"; null without a vault.
    key_scheme: Option<KeyScheme>,
    /// The vault needs a Secret Key and this device does not have it.
    needs_secret_key: bool,
    /// Whether this device currently has a server session (spec 2026-09-20
    /// §8.6). Independent of the lock state.
    online: bool,
}

/// Safe to call while locked: reveals no secrets.
#[tauri::command]
pub fn device_status(state: State<'_, AppState>) -> CmdResult<DeviceStatus> {
    let key_scheme = state.vault()?.key_scheme()?;
    let device = state.device.lock().map_err(|_| CmdError::internal())?;
    let uses_secret_key = key_scheme.is_some_and(KeyScheme::uses_secret_key);
    Ok(DeviceStatus {
        key_scheme,
        needs_secret_key: uses_secret_key && device.secret_key().is_none(),
        online: state.is_online(),
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
    // The mobile app scans this to set itself up (docs/server-sync.md, Emergency Kit).
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
