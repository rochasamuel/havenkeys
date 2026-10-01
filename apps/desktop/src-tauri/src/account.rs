//! The account: activation, signing in on a second device, the Emergency Kit
//! and the account screen.
//!
//! Every vault belongs to an account, and activation is the only way one is
//! created (design §5). The cryptography all happens here on the device; the
//! server records the result and never sees the master password, the Secret
//! Key, the KEK or the vault key.

use crate::commands::{copy_to_clipboard, CopyResult};
use crate::secret_store::Storage;
use crate::state::{AppState, CmdError, CmdResult};
use crate::sync;
use havenkeys_client::device::Device;
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::{AccountRecord, KeyScheme};
use havenkeys_core::sync::{encode_header_for, prepare_sign_in};
use havenkeys_core::vault::{
    self, prepare_new_account_vault, PreparedVault, VaultService, VaultStatus,
};
use havenkeys_core::SecretString;
use havenkeys_sync_client::{invite as invite_parser, Activation};
use serde::{Deserialize, Serialize};
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
pub(crate) async fn off_main_thread<T: Send + 'static>(
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
            let status = state.client().device()?.key_status(a.account_id);
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
    let client = state.client().server_for(&server_url)?;

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
        let mut device = state.client().device()?;
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
    let record = new_account_record(&account, server_url, 0);
    let minutes = create_vault(&state, made.prepared, &record)?;
    state.arm_auto_lock(minutes);
    state.notify_unlocked();
    let status = state.vault()?.status()?;

    // Sign in so the vault is immediately writable. A failure here leaves a
    // perfectly good offline vault, so it is not an activation failure.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let client = handle.state::<AppState>().client().clone();
        let _ = client.connect(made.auth_key).await;
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
    let client = state.client().server_for(&server_url)?;
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
            .client()
            .device()?
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
            "Desktop",
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

    let record = new_account_record(&account, server_url, header_revision);
    let minutes = create_vault(&state, prepared, &record)?;
    {
        let mut device = state.client().device()?;
        device
            .set_secret_key(account.id, &secret_key)
            .map_err(|_| CmdError::file())?;
    }
    state.arm_auto_lock(minutes);
    state.client().set_online(session);
    state.notify_unlocked();
    let status = state.vault()?.status()?;
    let _ = app.emit(sync::CONNECTIVITY_EVENT, true);

    // Catch up in the background: a vault with many items should not hold
    // the sign-in screen open.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let client = handle.state::<AppState>().client().clone();
        let _ = client.sync_now().await;
    });
    Ok(status)
}

/// This device's record of a newly joined account, before its first sync.
fn new_account_record(
    account: &AccountRef,
    server_url: String,
    max_header_rev: i64,
) -> AccountRecord {
    AccountRecord {
        account_id: account.id,
        email: account.email.as_str().to_string(),
        server_url,
        server_cursor: 0,
        max_header_rev,
        last_synced_at: None,
    }
}

/// Store the new vault, which opens unlocked, and return its auto-lock
/// minutes for the caller to arm.
fn create_vault(
    state: &AppState,
    prepared: PreparedVault,
    record: &AccountRecord,
) -> CmdResult<u32> {
    let mut vault = state.vault()?;
    vault.create_account_vault(prepared, record)?;
    Ok(vault.settings()?.auto_lock_minutes)
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
    let (session, client) = (state.session()?, state.client().server()?);
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
    let (session, client) = (state.session()?, state.client().server()?);
    client.revoke_device(&session, id).await?;
    if id == state.device_id()? {
        state.client().go_offline();
    }
    Ok(())
}

/// End this device's session and lock the vault. The vault stays on disk and
/// can be unlocked again; the server session is gone.
#[tauri::command]
pub async fn sign_out(app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if let (Ok(session), Ok(client)) = (state.session(), state.client().server()) {
        let _ = client.logout(&session).await;
    }
    state.lock("user");
    let _ = app.emit(sync::CONNECTIVITY_EVENT, false);
    Ok(())
}

// ------------------------------------------------------------------ Account item

/// The values of the HavenKeys Account item, pinned first in the vault list.
/// The item is virtual (spec 2026-09-29-account-item): it is built from the
/// account record and this device's Secret Key when shown, never stored, and
/// the browser extension cannot reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountField {
    Email,
    Server,
    AccountId,
    SecretKey,
}

/// The account behind the Account item. Only while unlocked.
fn unlocked_account(vault: &VaultService) -> havenkeys_core::Result<AccountRecord> {
    if !vault.is_unlocked() {
        return Err(havenkeys_core::Error::Locked);
    }
    vault.account()?.ok_or(havenkeys_core::Error::NoVault)
}

/// One value of the Account item. The Secret Key comes from this device's
/// keychain (or `device.json`); `NotFound` when this computer lacks it.
fn account_field(
    account: &AccountRecord,
    device: &mut Device,
    field: AccountField,
) -> havenkeys_core::Result<SecretString> {
    Ok(match field {
        AccountField::Email => SecretString::new(account.email.clone()),
        AccountField::Server => SecretString::new(account.server_url.clone()),
        AccountField::AccountId => SecretString::new(account.account_id.to_string()),
        AccountField::SecretKey => device
            .secret_key_text(account.account_id)
            .ok_or(havenkeys_core::Error::NotFound)?,
    })
}

/// The value and the vault's clipboard delay. The vault is released before
/// the device store is taken, as in `emergency_kit`.
fn account_value(state: &AppState, field: AccountField) -> CmdResult<(SecretString, u32)> {
    state.touch();
    let (account, seconds) = {
        let v = state.vault()?;
        (unlocked_account(&v)?, v.settings()?.clipboard_clear_seconds)
    };
    let mut device = state.client().device()?;
    Ok((account_field(&account, &mut device, field)?, seconds))
}

/// The Secret Key, on an explicit reveal from the Account item.
#[tauri::command]
pub async fn reveal_account_secret_key(app: AppHandle) -> CmdResult<SecretString> {
    off_main_thread(app, |state| {
        account_value(state, AccountField::SecretKey).map(|(value, _)| value)
    })
    .await
}

/// Copy one of the Account item's values. It goes from here to the
/// clipboard, cleared after the usual delay; the renderer never holds it.
#[tauri::command]
pub async fn copy_account_field(app: AppHandle, field: AccountField) -> CmdResult<CopyResult> {
    off_main_thread(app, move |state| {
        let (value, seconds) = account_value(state, field)?;
        copy_to_clipboard(state, &value, seconds)
    })
    .await
}

#[cfg(test)]
mod tests {
    mod account_item {
        use super::super::{account_field, unlocked_account, AccountField};
        use havenkeys_client::device::Device;
        use havenkeys_client::key_store::MemoryKeyStore;
        use havenkeys_core::account::{AccountRef, NormalizedEmail};
        use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
        use havenkeys_core::crypto::secret_key::SecretKey;
        use havenkeys_core::store::{AccountRecord, Store};
        use havenkeys_core::vault::{prepare_new_account_vault, VaultService};
        use havenkeys_core::{Error, SecretString};
        use uuid::Uuid;

        const ACCOUNT: Uuid = Uuid::from_u128(0x5eed);

        fn record() -> AccountRecord {
            AccountRecord {
                account_id: ACCOUNT,
                email: "user@example.com".into(),
                server_url: "https://vault.example.com".into(),
                server_cursor: 0,
                max_header_rev: 0,
                last_synced_at: None,
            }
        }

        /// An activated account vault, unlocked, and its Secret Key.
        fn activated() -> (VaultService, SecretKey) {
            let account =
                AccountRef::new(ACCOUNT, NormalizedEmail::parse("user@example.com").unwrap());
            let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
            let made = prepare_new_account_vault(
                &SecretString::from("correct horse battery staple"),
                &account,
                kdf,
                1_700_000_000_000,
            )
            .unwrap();
            let mut vault = VaultService::new(Store::open_in_memory().unwrap());
            vault
                .create_account_vault(made.prepared, &record())
                .unwrap();
            (vault, made.secret_key)
        }

        fn device_with(key: Option<&SecretKey>) -> (tempfile::TempDir, Device) {
            let dir = tempfile::tempdir().unwrap();
            let mut device = Device::load(dir.path(), Box::new(MemoryKeyStore::default()));
            if let Some(key) = key {
                device.set_secret_key(ACCOUNT, key).unwrap();
            }
            (dir, device)
        }

        #[test]
        fn a_locked_vault_gives_no_account_values() {
            let (mut vault, _) = activated();
            vault.lock();
            assert!(matches!(unlocked_account(&vault), Err(Error::Locked)));
        }

        #[test]
        fn every_field_reads_from_the_account_and_the_device() {
            let (vault, key) = activated();
            let (_dir, mut device) = device_with(Some(&key));
            let account = unlocked_account(&vault).unwrap();
            let value = |device: &mut Device, field| {
                account_field(&account, device, field)
                    .unwrap()
                    .expose()
                    .to_owned()
            };
            assert_eq!(value(&mut device, AccountField::Email), "user@example.com");
            assert_eq!(
                value(&mut device, AccountField::Server),
                "https://vault.example.com"
            );
            assert_eq!(
                value(&mut device, AccountField::AccountId),
                ACCOUNT.to_string()
            );
            assert_eq!(
                value(&mut device, AccountField::SecretKey),
                key.to_text().expose()
            );
        }

        #[test]
        fn a_missing_secret_key_is_not_found_but_the_rest_still_reads() {
            let (vault, _) = activated();
            let (_dir, mut device) = device_with(None);
            let account = unlocked_account(&vault).unwrap();
            assert!(matches!(
                account_field(&account, &mut device, AccountField::SecretKey),
                Err(Error::NotFound)
            ));
            assert!(account_field(&account, &mut device, AccountField::Email).is_ok());
        }

        #[test]
        fn fields_parse_from_the_wire_and_unknown_ones_are_refused() {
            for (wire, field) in [
                ("\"email\"", AccountField::Email),
                ("\"server\"", AccountField::Server),
                ("\"account_id\"", AccountField::AccountId),
                ("\"secret_key\"", AccountField::SecretKey),
            ] {
                assert_eq!(serde_json::from_str::<AccountField>(wire).unwrap(), field);
            }
            assert!(serde_json::from_str::<AccountField>("\"password\"").is_err());
        }

        #[test]
        fn a_revealed_key_does_not_debug_print() {
            let (vault, key) = activated();
            let (_dir, mut device) = device_with(Some(&key));
            let account = unlocked_account(&vault).unwrap();
            let value = account_field(&account, &mut device, AccountField::SecretKey).unwrap();
            assert!(!format!("{value:?}").contains(key.to_text().expose()));
        }
    }
}
