//! The Recovery Sheet: what a new device needs to join the account.

use crate::account::off_main_thread;
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::SecretString;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyKit {
    secret_key: SecretString,
    vault_id: Uuid,
    account_id: Uuid,
    email: String,
    server_url: String,
    created_at: i64,
    /// QR code for another device: `size` × `size` modules, row-major, true = dark.
    qr_size: usize,
    qr_modules: Vec<bool>,
}

/// The Recovery Sheet: everything a new device needs, and nothing a thief can
/// use without the master password. Only while unlocked, and only on explicit
/// request.
#[tauri::command]
pub async fn get_emergency_kit(app: AppHandle) -> CmdResult<EmergencyKit> {
    off_main_thread(app, emergency_kit).await
}

fn emergency_kit(state: &AppState) -> CmdResult<EmergencyKit> {
    state.touch();
    let (vault_id, created_at, account) = {
        let v = state.vault()?;
        if !v.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        (
            v.vault_id()?.ok_or(havenkeys_core::Error::NoVault)?,
            v.created_at()?.unwrap_or(0),
            v.account()?.ok_or(havenkeys_core::Error::NoVault)?,
        )
    };
    let secret_key = state
        .client()
        .device()?
        .secret_key_text(account.account_id)
        .ok_or(havenkeys_core::Error::NotFound)?;
    // v2 carries the account, the address and the server, because a new
    // device needs all four (design §5).
    let payload = SecretString::new(format!(
        "havenkeys://kit/v2?account={}&email={}&key={}&server={}",
        account.account_id,
        percent_encode(&account.email),
        secret_key.expose(),
        percent_encode(&account.server_url),
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
        account_id: account.account_id,
        email: account.email,
        server_url: account.server_url,
        created_at,
        qr_size: code.width(),
        qr_modules,
    })
}

/// Percent-encode everything but the unreserved set (RFC 3986 §2.3), so an
/// address with a `+` or a server URL with a port survives the round trip.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::percent_encode;

    #[test]
    fn reserved_characters_are_escaped() {
        assert_eq!(
            percent_encode("user+tag@example.com"),
            "user%2Btag%40example.com"
        );
        assert_eq!(
            percent_encode("https://vault.example.com:8443"),
            "https%3A%2F%2Fvault.example.com%3A8443"
        );
        assert_eq!(percent_encode("plain-word_1.0~"), "plain-word_1.0~");
    }
}
