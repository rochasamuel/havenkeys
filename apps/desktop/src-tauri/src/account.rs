//! The account: activation, signing in on a second device, the Emergency Kit
//! and the account screen.
//!
//! Every vault belongs to an account, and activation is the only way one is
//! created (design §5). The cryptography all happens here on the device; the
//! server records the result and never sees the master password, the Secret
//! Key, the KEK or the vault key.

use crate::secret_store::Storage;
use crate::state::{AppState, CmdError, CmdResult};
use crate::sync::{self, DEVICE_NAME};
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::{AccountRecord, KeyScheme};
use havenkeys_core::sync::{encode_header_for, prepare_sign_in};
use havenkeys_core::vault::{self, prepare_new_account_vault, VaultStatus};
use havenkeys_core::SecretString;
use havenkeys_sync_client::{invite as invite_parser, Activation};
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
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
    /// Where the Secret Key is kept: "keychain", "file" (no keychain
    /// answered; Settings warns) or "none".
    secret_key_storage: Storage,
}

/// Run `f` on a blocking thread. For commands that touch the keychain
/// (through `Device`): a sync command runs on the main thread, where a
/// keychain waiting on D-Bus or a prompt would freeze the window.
async fn off_main_thread<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|_| CmdError::internal())?
}

/// Safe to call while locked: reveals no secrets.
#[tauri::command]
pub async fn device_status(app: AppHandle) -> CmdResult<DeviceStatus> {
    off_main_thread(app, device_status_blocking).await
}

fn device_status_blocking(state: &AppState) -> CmdResult<DeviceStatus> {
    // Vault before device, as everywhere.
    let (key_scheme, account) = {
        let v = state.vault()?;
        (v.key_scheme()?, v.account()?)
    };
    let uses_secret_key = key_scheme.is_some_and(KeyScheme::uses_secret_key);
    let (needs_secret_key, secret_key_storage) = match account {
        Some(a) => {
            let status = state
                .device
                .lock()
                .map_err(|_| CmdError::internal())?
                .key_status(a.account_id);
            (uses_secret_key && status.missing, status.storage)
        }
        None => (false, Storage::None),
    };
    Ok(DeviceStatus {
        key_scheme,
        needs_secret_key,
        online: state.is_online(),
        secret_key_storage,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    email: String,
    server_url: String,
    account_id: Uuid,
    online: bool,
    /// Unix ms of the last successful pull, or null if none yet.
    last_synced_at: Option<i64>,
}

/// The account this vault belongs to. No secrets; safe while locked.
#[tauri::command]
pub fn account_status(state: State<'_, AppState>) -> CmdResult<Option<AccountStatus>> {
    let Some(account) = state.vault()?.account()? else {
        return Ok(None);
    };
    Ok(Some(AccountStatus {
        email: account.email,
        server_url: account.server_url,
        account_id: account.account_id,
        online: state.is_online(),
        last_synced_at: account.last_synced_at,
    }))
}

// ------------------------------------------------------------------ activation

/// First run: the user pastes the invite and chooses a master password.
///
/// The order is deliberate. Keys are derived and the header is built in
/// memory, the server is asked to accept them, and only then is anything
/// written to disk — so a refused activation leaves this device with no
/// vault at all, rather than one bound to an account no server knows.
#[tauri::command]
pub async fn activate_account(
    app: AppHandle,
    invite: String,
    password: SecretString,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    if state.vault()?.key_scheme()?.is_some() {
        return Err(havenkeys_core::Error::VaultExists.into());
    }
    vault::check_new_master_password(&password)?;

    // The server decodes and burns the invite itself, so the original string
    // is what travels; the fields read here only tell this device which
    // account and server to derive against.
    let raw_invite = invite.trim().to_string();
    let invite = invite_parser::decode(&raw_invite)?;
    let email = NormalizedEmail::parse(&invite.email)?;
    let account = AccountRef::new(invite.account, email);
    let server_url = invite.server.clone();
    let client = sync::client_for(&state, &server_url)?;

    let kdf = KdfParams::generate()?;
    let account_for_derivation = account.clone();
    let made = tauri::async_runtime::spawn_blocking(move || {
        prepare_new_account_vault(&password, &account_for_derivation, kdf, AppState::now_ms())
    })
    .await
    .map_err(|_| CmdError::internal())??;

    let header = encode_header_for(&made.prepared)?;
    let vault_id = made.prepared.vault_id();
    let kdf = made.prepared.kdf().clone();

    // Written before the server is asked, not after. If activation succeeds
    // and anything below it fails — a full disk, a crash — the account
    // exists, its invite is spent, and the only copy of the Secret Key would
    // otherwise be gone with it, leaving the vault unopenable forever. A key
    // stored for an activation that never completed is harmless.
    {
        let mut device = state.device.lock().map_err(|_| CmdError::internal())?;
        device
            .set_secret_key(account.id, &made.secret_key)
            .map_err(|_| CmdError::file())?;
    }

    client
        .activate(Activation {
            email: account.email.as_str(),
            invite: &raw_invite,
            kdf: &kdf,
            auth_key: &made.auth_key,
            vault_id,
            header: &header,
        })
        .await?;

    // The server has the account; now this device gets its vault.
    let record = AccountRecord {
        account_id: account.id,
        email: account.email.as_str().to_string(),
        server_url,
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    };
    let minutes = {
        let mut vault = state.vault()?;
        vault.create_account_vault(made.prepared, &record)?;
        vault.settings()?.auto_lock_minutes
    };
    state.arm_auto_lock(minutes);
    state.notify_unlocked();
    let status = state.vault()?.status()?;

    // Sign in so the vault is immediately writable. A failure here leaves a
    // perfectly good offline vault, so it is not an activation failure.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sync::connect(&handle, made.auth_key).await;
    });
    Ok(status)
}

/// A second device: the user enters the server, their email, the master
/// password and the Secret Key from the Emergency Kit.
///
/// Nothing is written to disk until the header the server serves has been
/// opened with the keys derived here — which is what proves all four inputs
/// are right.
#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    server_url: String,
    email: String,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    if state.vault()?.key_scheme()?.is_some() {
        return Err(havenkeys_core::Error::VaultExists.into());
    }
    let email = NormalizedEmail::parse(&email)?;
    let typed = match secret_key {
        Some(typed) if !typed.is_empty() => Some(SecretKey::parse(typed.expose())?),
        _ => None,
    };
    let server_url = server_url.trim().trim_end_matches('/').to_string();
    let client = sync::client_for(&state, &server_url)?;
    let device_id = state.device_id()?;

    // The parameters are public by design: a device needs them before it can
    // derive anything, and the server answers the same way for an address it
    // has never seen.
    let params = client.auth_params(email.as_str()).await?;
    let account = AccountRef::new(params.account_id, email);

    // A key already on this computer for this account is used when none is
    // typed. That is what makes an activation interrupted after the server
    // accepted it recoverable: the Secret Key was written here before the
    // server was asked, so signing in finishes what activation started.
    let secret_key = match typed {
        Some(k) => k,
        None => state
            .device
            .lock()
            .map_err(|_| CmdError::internal())?
            .secret_key(account.id)
            .ok_or(havenkeys_core::Error::SecretKeyRequired)?,
    };

    let (account_for_auth, secret_key) = (account.clone(), secret_key);
    let password_for_auth = password.clone();
    let kdf = params.kdf.clone();
    let (auth_key, secret_key) = tauri::async_runtime::spawn_blocking(move || {
        let derived =
            vault::derive_auth_key(&password_for_auth, &secret_key, &kdf, &account_for_auth);
        (derived, secret_key)
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let auth_key = auth_key?;

    let session = client
        .login(
            account.email.as_str(),
            &auth_key,
            account.id,
            device_id,
            DEVICE_NAME,
        )
        .await
        .map_err(|_| CmdError::sign_in_failed())?;
    let header = client
        .header(&session)
        .await
        .map_err(|_| CmdError::sign_in_failed())?;

    let account_for_unwrap = account.clone();
    let (prepared, secret_key) = tauri::async_runtime::spawn_blocking(move || {
        let prepared = prepare_sign_in(&header.bytes, &password, &secret_key, &account_for_unwrap);
        (prepared, secret_key)
    })
    .await
    .map_err(|_| CmdError::internal())?;
    // Any failure here — wrong password, wrong Secret Key, a header that
    // does not open — is one message. The device never says which.
    let (prepared, _) = prepared.map_err(|_| CmdError::sign_in_failed())?;
    let header_revision = prepared.header_revision() as i64;

    let record = AccountRecord {
        account_id: account.id,
        email: account.email.as_str().to_string(),
        server_url,
        server_cursor: 0,
        max_header_rev: header_revision,
        last_synced_at: None,
    };
    let minutes = {
        let mut vault = state.vault()?;
        vault.create_account_vault(prepared, &record)?;
        vault.settings()?.auto_lock_minutes
    };
    {
        let mut device = state.device.lock().map_err(|_| CmdError::internal())?;
        device
            .set_secret_key(account.id, &secret_key)
            .map_err(|_| CmdError::file())?;
    }
    state.arm_auto_lock(minutes);
    state.set_online(session);
    state.notify_unlocked();
    let status = state.vault()?.status()?;
    let _ = app.emit(sync::CONNECTIVITY_EVENT, true);

    // Catch up in the background: a vault with many items should not hold
    // the sign-in screen open.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sync::sync_now(&handle).await;
    });
    Ok(status)
}

// ------------------------------------------------------------------ devices

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEntry {
    id: Uuid,
    name: String,
    created_at: String,
    last_seen_at: Option<String>,
    current: bool,
}

#[tauri::command]
pub async fn list_devices(app: AppHandle) -> CmdResult<Vec<DeviceEntry>> {
    let state = app.state::<AppState>();
    state.touch();
    let (session, client) = (state.session()?, sync::client(&state)?);
    let devices = client.devices(&session).await?;
    Ok(devices
        .into_iter()
        .map(|d| DeviceEntry {
            id: d.id,
            name: d.name,
            created_at: d.created_at,
            last_seen_at: d.last_seen_at,
            current: d.current,
        })
        .collect())
}

/// Cut a device off. Revoking this one signs it out immediately.
#[tauri::command]
pub async fn revoke_device(app: AppHandle, id: Uuid) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let (session, client) = (state.session()?, sync::client(&state)?);
    client.revoke_device(&session, id).await?;
    if id == state.device_id()? {
        sync::go_offline(&app);
    }
    Ok(())
}

/// End this device's session and lock the vault. The vault stays on disk and
/// can be unlocked again; the server session is gone.
#[tauri::command]
pub async fn sign_out(app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if let (Ok(session), Ok(client)) = (state.session(), sync::client(&state)) {
        let _ = client.logout(&session).await;
    }
    state.lock(&app, "user");
    let _ = app.emit(sync::CONNECTIVITY_EVENT, false);
    Ok(())
}

// ------------------------------------------------------------------ Emergency Kit

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

/// The Emergency Kit: everything a new device needs, and nothing a thief can
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
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
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
