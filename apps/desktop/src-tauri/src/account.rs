//! Secret Key and Emergency Kit.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::KeyScheme;
use havenkeys_core::SecretString;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

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
    /// "account_bound"; null without a vault.
    key_scheme: Option<KeyScheme>,
    /// The vault needs a Secret Key and this device does not have it.
    needs_secret_key: bool,
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
