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
use crate::sync;
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::generator::{self, GeneratedPassword, GeneratorOptions};
use havenkeys_core::model::{ItemInput, ItemOverview, SecretField, Settings};
use havenkeys_core::sync::prepare_sign_in;
use havenkeys_core::totp::TotpCode;
use havenkeys_core::vault::{self, VaultStatus};
use havenkeys_core::SecretString;
use havenkeys_sync_client::{CredentialChange, SyncError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const DEFAULT_CLIPBOARD_CLEAR_SECS: u32 = 30;
/// How long the unlock fallback waits for the server's KDF parameters.
const FALLBACK_PARAMS_TIMEOUT: Duration = Duration::from_secs(3);

// ------------------------------------------------------------------ lifecycle

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> CmdResult<VaultStatus> {
    Ok(state.vault()?.status()?)
}

/// Unlock. Every vault is account-bound: the account comes from the local
/// store (never from the renderer), and unlocking needs the Secret Key too —
/// either typed from the Emergency Kit (it is then saved here once the
/// unlock succeeds) or the one already saved on this device.
#[tauri::command]
pub async fn unlock_vault(
    app: AppHandle,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    let account = state
        .vault()?
        .account()?
        .ok_or(havenkeys_core::Error::NoVault)?
        .to_ref()?;
    let (stored, stored_text) = {
        let mut device = state.device.lock().map_err(|_| CmdError::internal())?;
        let text = device.secret_key_text(account.id);
        let key = text
            .as_ref()
            .and_then(|t| SecretKey::parse(t.expose()).ok());
        (key, text)
    };
    let typed = match secret_key {
        Some(t) if !t.is_empty() => Some(SecretKey::parse(t.expose())?),
        _ => None,
    };
    // `SecretKey` is deliberately not `Clone`, so both options move into the
    // blocking closure and are borrowed there, as the current code does.
    if typed.is_none() && stored.is_none() {
        let ticket = state.vault()?.begin_unlock()?;
        let r = state
            .vault()?
            .finish_unlock(ticket, Err(havenkeys_core::Error::SecretKeyRequired));
        return Err(r
            .err()
            .unwrap_or(havenkeys_core::Error::SecretKeyRequired)
            .into());
    }
    let account_id = account.id;
    let key_text = typed.as_ref().map(|k| k.to_text());
    // Kept for the fallback below: `SecretKey` is not `Clone`, so the text
    // is re-parsed there. The typed key wins, as it does here.
    let key_text_for_fallback = key_text.clone().or(stored_text);
    let password_for_fallback = password.clone();
    let ticket = state.vault()?.begin_unlock()?;
    let joined = tauri::async_runtime::spawn_blocking(move || {
        // One Argon2id run yields both the KEK and the auth key: the first
        // opens the vault, the second opens the server session.
        let derived = match typed.as_ref().or(stored.as_ref()) {
            Some(sk) => ticket.derive_session_for_account(&password, sk, &account),
            None => Err(havenkeys_core::Error::SecretKeyRequired),
        };
        (ticket, derived)
    })
    .await;
    let (ticket, derived) = match joined {
        Ok(v) => v,
        Err(_) => {
            // The KDF task died; make sure we do not stay in UNLOCKING.
            state.lock(&app, "error");
            return Err(CmdError::internal());
        }
    };

    let (key, auth_key) = match derived {
        Ok((key, auth_key)) => (Ok(key), Some(auth_key)),
        Err(e) => (Err(e), None),
    };
    // One guard from `finish_unlock` to `notify_unlocked`, so a concurrent
    // lock cannot fall between them. Scoped: it is released before the
    // fallback's requests, never held across an await.
    let unlocked = {
        let mut v = state.vault()?;
        match v.finish_unlock(ticket, key) {
            Ok(()) => {
                let minutes = v.settings()?.auto_lock_minutes;
                let status = v.status()?;
                // Arm before releasing the vault lock so the auto-lock thread
                // can never tick against the previous session's timestamps.
                state.arm_auto_lock(minutes);
                // Still under the vault lock, so a concurrent lock's `locked`
                // event can never be overtaken by this one. `notify` never
                // blocks.
                state.notify_unlocked();
                Ok(status)
            }
            // The epoch as of this failure, under the same guard: a lock
            // requested while the fallback runs advances it, and the
            // adoption is then refused.
            Err(err) => Err((err, v.epoch())),
        }
    };
    let status = match unlocked {
        Ok(status) => status,
        Err((err, epoch)) => {
            let Some(sk_text) = key_text_for_fallback.filter(|_| err.code() == "unlock_failed")
            else {
                return Err(err.into());
            };
            // The password may have been changed on another device: this
            // device's header still has the old salt. Ask the server. Any
            // failure below is reported as the original wrong password, so a
            // caller learns nothing about which step failed.
            return match unlock_from_server(&app, password_for_fallback, sk_text, epoch).await {
                Ok(status) => {
                    remember_typed_secret_key(&state, account_id, key_text);
                    Ok(status)
                }
                Err(_) => Err(err.into()),
            };
        }
    };
    remember_typed_secret_key(&state, account_id, key_text);
    // Open the server session in the background. The vault is already
    // usable: a device that cannot reach its server is offline and
    // read-only, not locked.
    if let Some(auth_key) = auth_key {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = sync::connect(&handle, auth_key).await;
        });
    }
    Ok(status)
}

/// A Secret Key typed from the Emergency Kit proved correct: remember it.
fn remember_typed_secret_key(state: &AppState, account: Uuid, key_text: Option<SecretString>) {
    if let Some(text) = key_text {
        if let Ok(k) = SecretKey::parse(text.expose()) {
            if let Ok(mut d) = state.device.lock() {
                let _ = d.set_secret_key(account, &k);
            }
        }
    }
}

/// Unlock with a header the server serves, when the local one no longer
/// matches the password (changed on another device). The header is verified
/// exactly as a sign-in verifies it, and adopted only if it is newer than
/// everything this device has seen (`adopt_and_unlock`).
///
/// Only reached from `unlock_vault` after the local unlock failed with
/// `unlock_failed`, so the vault is LOCKED on entry; it stays LOCKED unless
/// the final adoption succeeds. The caller hides every error from here.
async fn unlock_from_server(
    app: &AppHandle,
    password: SecretString,
    secret_key_text: SecretString,
    epoch: u64,
) -> CmdResult<VaultStatus> {
    let state = app.state::<AppState>();
    let account = state
        .vault()?
        .account()?
        .ok_or(havenkeys_core::Error::NoVault)?
        .to_ref()?;
    let local_kdf = state.vault()?.kdf()?;
    let client = sync::client(&state)?;
    // Short, because this runs on every wrong password: an unreachable server
    // must not hold the unlock screen for the transport's full timeout. The
    // later requests keep the normal ones; the server has answered by then.
    let params = tokio::time::timeout(
        FALLBACK_PARAMS_TIMEOUT,
        client.auth_params(account.email.as_str()),
    )
    .await
    .map_err(|_| havenkeys_core::Error::UnlockFailed)??;
    // Same parameters as the local header means the password really is
    // wrong: no change was made elsewhere, so there is nothing to fetch.
    // Another account ID means the server is not the one this vault knows.
    if params.account_id != account.id || local_kdf.as_ref() == Some(&params.kdf) {
        return Err(havenkeys_core::Error::UnlockFailed.into());
    }
    let (pw, sk_text, acct) = (password.clone(), secret_key_text.clone(), account.clone());
    let kdf = params.kdf;
    let auth_key = tauri::async_runtime::spawn_blocking(move || {
        let sk = SecretKey::parse(sk_text.expose())?;
        vault::derive_auth_key(&pw, &sk, &kdf, &acct)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    let session = client
        .login(
            account.email.as_str(),
            &auth_key,
            account.id,
            state.device_id()?,
            sync::DEVICE_NAME,
        )
        .await?;
    drop(auth_key);
    let header = client.header(&session).await?;
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let sk = SecretKey::parse(secret_key_text.expose())?;
        prepare_sign_in(&header.bytes, &password, &sk, &account).map(|(p, _)| p)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    let status = {
        let mut v = state.vault()?;
        v.adopt_and_unlock(prepared, epoch)?;
        let opened = v
            .settings()
            .and_then(|s| Ok((s.auto_lock_minutes, v.status()?)));
        let (minutes, status) = match opened {
            Ok(opened) => opened,
            Err(e) => {
                // Never report a failure while leaving the vault open.
                v.lock();
                return Err(e.into());
            }
        };
        // As in `unlock_vault`: armed and announced under the vault lock,
        // and the session set there too, so a concurrent lock (which takes
        // the vault lock first) always drops it afterwards.
        state.arm_auto_lock(minutes);
        state.notify_unlocked();
        state.set_online(session);
        status
    };
    let _ = app.emit(sync::CONNECTIVITY_EVENT, true);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sync::sync_now(&handle).await;
    });
    Ok(status)
}

/// Pull now, instead of waiting for the periodic sync.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> CmdResult<havenkeys_core::sync::SyncReport> {
    app.state::<AppState>().touch();
    sync::sync_now(&app).await
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
    sync::sync_now(&app).await
}

#[tauri::command]
pub fn lock_vault(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.lock(&app, "user");
    Ok(())
}

/// Change the master password: server first, local second.
///
/// The re-wrapped header goes to the server together with the current and
/// the new login keys, in one request (`change_credentials`); the server
/// applies it atomically at base + 1 and signs out every other device. Only
/// once it has answered 2xx is the new wrap committed here. The other order
/// would leave this device holding a header, and a password, the server has
/// never heard of: every later sign-in with the new password would fail.
#[tauri::command]
pub async fn change_master_password(
    app: AppHandle,
    current: SecretString,
    new: SecretString,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    state.require_online()?;
    vault::check_new_master_password(&new)?;
    // Adopt a change made elsewhere first, so the base revision is current.
    sync::sync_now(&app).await?;
    let kdf = KdfParams::generate()?;
    let account = state
        .vault()?
        .account()?
        .ok_or(havenkeys_core::Error::NoVault)?
        .to_ref()?;
    let secret_key = state
        .device
        .lock()
        .map_err(|_| CmdError::internal())?
        .secret_key(account.id)
        .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
    let ticket = state.vault()?.begin_rekey()?;
    // Both Argon2id runs happen without the vault lock, so locking (button,
    // auto-lock, window close) is never delayed; the commit re-checks the
    // lock epoch and the header.
    let (ticket, rekeyed) = tauri::async_runtime::spawn_blocking(move || {
        let rekeyed = ticket.derive_for_account(&current, &new, kdf, &secret_key, &account);
        (ticket, rekeyed)
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let rekeyed = rekeyed?;
    // Each `state.vault()?` guard below is a temporary, dropped at the end
    // of its statement: none is held across the request.
    let header = state.vault()?.encode_rekeyed_header(&ticket, &rekeyed)?;
    let (session, client) = (state.session()?, sync::client(&state)?);
    let revision = client
        .change_credentials(
            &session,
            CredentialChange {
                current_auth_key: rekeyed.current_auth_key(),
                kdf: rekeyed.kdf(),
                new_auth_key: rekeyed.new_auth_key(),
                header: &header,
                base_header_revision: ticket.base_revision() as i64,
            },
        )
        .await
        .map_err(|e| credential_change_conflict(&e).unwrap_or_else(|| sync::failed(&app, e)))?;
    // The server has it; from here on the change has happened.
    let revision = u64::try_from(revision).map_err(|_| CmdError::internal())?;
    let committed = state.vault()?.commit_rekey(ticket, Ok(rekeyed), revision);
    match committed {
        // `Busy`: a background sync already adopted this very header from
        // the server. `Locked`: the vault was locked meanwhile; the next
        // unlock (via the server fallback) or sync adopts it. Either way the
        // change succeeded, and saying otherwise would send the user back to
        // a password that no longer works.
        Ok(()) | Err(havenkeys_core::Error::Busy | havenkeys_core::Error::Locked) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// A 409 on a credential change: the header moved on the server since this
/// device read it, which only another device's password change does. The
/// password this device knows is no longer the account's.
fn credential_change_conflict(err: &SyncError) -> Option<CmdError> {
    matches!(err, SyncError::Conflict(_)).then(|| CmdError {
        code: "password_changed_elsewhere",
        message: "Your master password was changed on another device. Lock and unlock with the new password.".into(),
    })
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
pub fn reveal_secret(
    state: State<'_, AppState>,
    id: Uuid,
    field: SecretField,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.reveal(&id, field)?)
}

/// When each previous password of a login was replaced (Unix ms, newest first).
#[tauri::command]
pub fn password_history(state: State<'_, AppState>, id: Uuid) -> CmdResult<Vec<i64>> {
    state.touch();
    Ok(state.vault()?.password_history(&id)?)
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

/// Create an item: seal it under the vault lock, let the server assign its
/// revision, then record it locally. Nothing is stored until the server has
/// accepted it, so the replica is never ahead of the authority.
#[tauri::command]
pub async fn create_item(app: AppHandle, input: ItemInput) -> CmdResult<ItemOverview> {
    let staged = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        let staged = state.vault()?.stage_create(input, AppState::now_ms())?;
        staged
    };
    sync::push(&app, staged)
        .await?
        .ok_or_else(CmdError::internal)
}

#[tauri::command]
pub async fn update_item(app: AppHandle, id: Uuid, input: ItemInput) -> CmdResult<ItemOverview> {
    let staged = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        let staged = state
            .vault()?
            .stage_update(&id, input, AppState::now_ms())?;
        staged
    };
    sync::push(&app, staged)
        .await?
        .ok_or_else(CmdError::internal)
}

#[tauri::command]
pub async fn delete_item(app: AppHandle, id: Uuid) -> CmdResult<()> {
    let staged = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        let staged = state.vault()?.stage_delete(&id)?;
        staged
    };
    sync::push(&app, staged).await.map(|_| ())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conflict_on_a_credential_change_means_changed_elsewhere() {
        let err = credential_change_conflict(&SyncError::Conflict(vec![])).unwrap();
        assert_eq!(err.code, "password_changed_elsewhere");
        assert_eq!(
            err.message,
            "Your master password was changed on another device. Lock and unlock with the new password."
        );
    }

    #[test]
    fn other_credential_change_failures_keep_their_usual_mapping() {
        for e in [
            SyncError::Unauthorized,
            SyncError::Unavailable,
            SyncError::RateLimited,
            SyncError::TooLarge,
        ] {
            assert!(credential_change_conflict(&e).is_none());
        }
    }
}
