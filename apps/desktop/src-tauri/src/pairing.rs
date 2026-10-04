//! Signing this computer in from the phone
//! (spec 2026-10-03-phone-approved-sign-in §3.1, §3.4). The pairing's keys
//! stay in `havenkeys_client`; the renderer gets the QR code's modules, the
//! time left and the outcome.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_client::PairingPoll;
use havenkeys_core::vault::VaultStatus;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCode {
    qr_size: usize,
    /// `qr_size` × `qr_size` modules, row-major, true = dark.
    qr_modules: Vec<bool>,
    expires_at: String,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum PairingState {
    Waiting,
    Denied,
    Expired,
    Approved { status: VaultStatus },
}

/// "Desktop · Linux": the name the phone shows before Allow.
fn device_label() -> String {
    let os = match std::env::consts::OS {
        "linux" => "Linux",
        "windows" => "Windows",
        "macos" => "macOS",
        other => other,
    };
    format!("Desktop · {os}")
}

#[tauri::command]
pub async fn pairing_start(app: AppHandle, server_url: String) -> CmdResult<PairingCode> {
    let client = app.state::<AppState>().client().clone();
    let start = client.start_pairing(server_url, &device_label()).await?;
    let code = QrCode::with_error_correction_level(start.link.as_bytes(), EcLevel::M)
        .map_err(|_| CmdError::internal())?;
    Ok(PairingCode {
        qr_size: code.width(),
        qr_modules: code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect(),
        expires_at: start.expires_at,
    })
}

#[tauri::command]
pub async fn pairing_poll(app: AppHandle) -> CmdResult<PairingState> {
    let client = app.state::<AppState>().client().clone();
    Ok(match client.poll_pairing().await? {
        PairingPoll::Waiting => PairingState::Waiting,
        PairingPoll::Denied => PairingState::Denied,
        PairingPoll::Expired => PairingState::Expired,
        PairingPoll::Approved(status) => PairingState::Approved { status },
    })
}

#[tauri::command]
pub fn pairing_cancel(app: AppHandle) {
    app.state::<AppState>().client().cancel_pairing();
}
