//! The account: activation, signing in on a second device, unlocking, the
//! master password change, devices and the Account item.
//!
//! Every vault belongs to an account, and activation is the only way one is
//! created (spec 2026-09-20 §5). The cryptography all happens here on the
//! device; the server never sees the master password, the Secret Key, the
//! KEK or the vault key.

use crate::client::HavenClient;
use crate::device::Device;
use crate::error::{ClientError, ClientResult};
use crate::key_store::Storage;
use crate::now_ms;
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::store::{AccountRecord, KeyScheme};
use havenkeys_core::sync::{encode_header_for, prepare_sign_in};
use havenkeys_core::vault::{
    self, prepare_new_account_vault, PreparedVault, VaultService, VaultStatus,
};
use havenkeys_core::SecretString;
use havenkeys_sync_client::{
    invite as invite_parser, Activation, CredentialChange, Session, SyncError,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;
use uuid::Uuid;

/// How long the unlock fallback waits for the server's KDF parameters.
const FALLBACK_PARAMS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    /// "account_bound"; null without a vault.
    pub key_scheme: Option<KeyScheme>,
    /// The vault needs a Secret Key and this device does not have it.
    pub needs_secret_key: bool,
    /// Whether this device currently has a server session (spec 2026-09-20
    /// §8.6). Independent of the lock state.
    pub online: bool,
    /// Where the Secret Key is kept: "keychain", "file" (no keychain
    /// answered; Settings warns) or "none".
    pub secret_key_storage: Storage,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub email: String,
    pub server_url: String,
    pub account_id: Uuid,
    pub online: bool,
    /// Unix ms of the last successful pull, or null if none yet.
    pub last_synced_at: Option<i64>,
    /// The server's plan status (`trialing`, `active`, ...), once known.
    /// May lag `entitlement` after a refused write, until the next sync.
    pub plan_status: Option<String>,
    /// `"full"` or `"frozen"`; a frozen account is read-only.
    pub entitlement: havenkeys_core::store::Entitlement,
    pub trial_ends_at: Option<String>,
    pub period_end: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    pub email: String,
    pub server_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEntry {
    pub id: Uuid,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub current: bool,
    pub approved_by: Option<Uuid>,
}

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

impl HavenClient {
    /// Safe while locked: reveals no secrets. May wait on the platform
    /// store, so shells call it off their UI thread.
    pub fn device_status(&self) -> ClientResult<DeviceStatus> {
        // Vault before device, as everywhere.
        let (key_scheme, account) = {
            let v = self.vault()?;
            (v.key_scheme()?, v.account()?)
        };
        let uses_secret_key = key_scheme.is_some_and(KeyScheme::uses_secret_key);
        let (needs_secret_key, secret_key_storage) = match account {
            Some(a) => {
                let status = self.device()?.key_status(a.account_id);
                (uses_secret_key && status.missing, status.storage)
            }
            None => (false, Storage::None),
        };
        Ok(DeviceStatus {
            key_scheme,
            needs_secret_key,
            online: self.is_online(),
            secret_key_storage,
        })
    }

    /// The account this vault belongs to. No secrets; safe while locked.
    pub fn account_status(&self) -> ClientResult<Option<AccountStatus>> {
        let (account, plan) = {
            let vault = self.vault()?;
            (vault.account()?, vault.plan()?)
        };
        let Some(account) = account else {
            return Ok(None);
        };
        Ok(Some(AccountStatus {
            email: account.email,
            server_url: account.server_url,
            account_id: account.account_id,
            online: self.is_online(),
            last_synced_at: account.last_synced_at,
            plan_status: plan.status,
            entitlement: plan.entitlement,
            trial_ends_at: plan.trial_ends_at,
            period_end: plan.period_end,
        }))
    }

    /// What an invite says, before anything is typed or sent: the account's
    /// email and the server it points at. Pure decoding, no network.
    pub fn preview_invite(&self, invite: &str) -> ClientResult<InvitePreview> {
        let invite = invite_parser::decode(invite.trim())
            .map_err(|_| havenkeys_core::Error::InvalidInput("that invite is not valid"))?;
        let email = NormalizedEmail::parse(&invite.email)?;
        Ok(InvitePreview {
            email: email.as_str().to_string(),
            server_url: invite.server.clone(),
        })
    }

    /// First run: the invite and a new master password.
    ///
    /// The order is deliberate. Keys are derived and the header is built in
    /// memory, the server is asked to accept them, and only then is anything
    /// written to disk — so a refused activation leaves this device with no
    /// vault at all, rather than one bound to an account no server knows.
    pub async fn activate(
        self: &Arc<Self>,
        invite: String,
        password: SecretString,
    ) -> ClientResult<VaultStatus> {
        if self.vault()?.key_scheme()?.is_some() {
            return Err(havenkeys_core::Error::VaultExists.into());
        }
        vault::check_new_master_password(&password)?;

        // The server decodes and burns the invite itself, so the original
        // string is what travels; the fields read here only tell this device
        // which account and server to derive against.
        let raw_invite = invite.trim().to_string();
        let invite = invite_parser::decode(&raw_invite)?;
        let email = NormalizedEmail::parse(&invite.email)?;
        let account = AccountRef::new(invite.account, email);
        let server_url = invite.server.clone();
        let server = self.server_for(&server_url)?;

        let kdf = KdfParams::generate()?;
        let account_for_derivation = account.clone();
        let made = spawn_blocking(move || {
            prepare_new_account_vault(&password, &account_for_derivation, kdf, now_ms())
        })
        .await
        .map_err(|_| ClientError::internal())??;

        let header = encode_header_for(&made.prepared)?;
        let vault_id = made.prepared.vault_id();
        let kdf = made.prepared.kdf().clone();

        // Written before the server is asked, not after. If activation
        // succeeds and anything below it fails — a full disk, a crash — the
        // account exists, its invite is spent, and the only copy of the
        // Secret Key would otherwise be gone with it, leaving the vault
        // unopenable forever. A key stored for an activation that never
        // completed is harmless.
        self.device()?
            .set_secret_key(account.id, &made.secret_key)
            .map_err(|_| ClientError::file())?;

        server
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
        self.create_vault(made.prepared, &record)?;
        let status = self.vault()?.status()?;

        // Sign in so the vault is immediately writable. A failure here leaves
        // a perfectly good offline vault, so it is not an activation failure.
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let _ = client.connect(made.auth_key).await;
        });
        Ok(status)
    }

    /// A second device: server, email, master password and the Secret Key
    /// from the Recovery Sheet.
    ///
    /// Nothing is written to disk until the header the server serves has
    /// been opened with the keys derived here — which is what proves all
    /// four inputs are right.
    pub async fn sign_in(
        self: &Arc<Self>,
        server_url: String,
        email: String,
        password: SecretString,
        secret_key: Option<SecretString>,
    ) -> ClientResult<VaultStatus> {
        if self.vault()?.key_scheme()?.is_some() {
            return Err(havenkeys_core::Error::VaultExists.into());
        }
        let email = NormalizedEmail::parse(&email)?;
        let typed = match secret_key {
            Some(typed) if !typed.is_empty() => Some(SecretKey::parse(typed.expose())?),
            _ => None,
        };
        let server_url = server_url.trim().trim_end_matches('/').to_string();
        let server = self.server_for(&server_url)?;
        let device_id = self.device_id()?;

        // The parameters are public by design: a device needs them before it
        // can derive anything, and the server answers the same way for an
        // address it has never seen.
        let params = server.auth_params(email.as_str()).await?;
        let account = AccountRef::new(params.account_id, email);

        // A key already on this device for this account is used when none is
        // typed. That is what makes an activation interrupted after the
        // server accepted it recoverable: the Secret Key was written here
        // before the server was asked, so signing in finishes what
        // activation started.
        let secret_key = match typed {
            Some(k) => k,
            None => self
                .device()?
                .secret_key(account.id)
                .ok_or(havenkeys_core::Error::SecretKeyRequired)?,
        };

        let account_for_auth = account.clone();
        let password_for_auth = password.clone();
        let kdf = params.kdf.clone();
        let (auth_key, secret_key) = spawn_blocking(move || {
            let derived =
                vault::derive_auth_key(&password_for_auth, &secret_key, &kdf, &account_for_auth);
            (derived, secret_key)
        })
        .await
        .map_err(|_| ClientError::internal())?;
        let auth_key = auth_key?;

        let session = server
            .login(
                account.email.as_str(),
                &auth_key,
                account.id,
                device_id,
                &self.config.device_name,
            )
            .await
            .map_err(|_| ClientError::sign_in_failed())?;
        let header = server
            .header(&session)
            .await
            .map_err(|_| ClientError::sign_in_failed())?;

        let account_for_unwrap = account.clone();
        let (prepared, secret_key) = spawn_blocking(move || {
            let prepared =
                prepare_sign_in(&header.bytes, &password, &secret_key, &account_for_unwrap);
            (prepared, secret_key)
        })
        .await
        .map_err(|_| ClientError::internal())?;
        // Any failure here — wrong password, wrong Secret Key, a header that
        // does not open — is one message. The device never says which.
        let (prepared, _) = prepared.map_err(|_| ClientError::sign_in_failed())?;
        let header_revision = prepared.header_revision() as i64;

        let record = new_account_record(&account, server_url, header_revision);
        let epoch = self.create_vault(prepared, &record)?;
        self.device()?
            .set_secret_key(account.id, &secret_key)
            .map_err(|_| ClientError::file())?;
        self.go_online_after_sign_in(session, epoch)
    }

    /// The new vault exists and is open: go online with `session` and catch
    /// up in the background. The keychain can hold the caller up for seconds
    /// (a prompt), long enough for the new vault to auto-lock. As in
    /// `connect`, a lock wins: the session is dropped unused and the locked
    /// status is reported.
    ///
    /// `epoch` is the one `create_vault` returned: a lock in between wins
    /// even if the vault was unlocked again since (DT1).
    pub(crate) fn go_online_after_sign_in(
        self: &Arc<Self>,
        session: Session,
        epoch: u64,
    ) -> ClientResult<VaultStatus> {
        let (status, online) = {
            let vault = self.vault()?;
            let online = vault.is_unlocked() && vault.epoch() == epoch;
            if online {
                self.set_online(session);
            }
            (vault.status()?, online)
        };
        if !online {
            return Ok(status);
        }
        self.events.connectivity(true);

        // Catch up in the background: a vault with many items should not
        // hold the sign-in screen open.
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let _ = client.sync_now().await;
        });
        Ok(status)
    }

    /// Store the new vault, which opens unlocked, and start its auto-lock.
    /// Returns the vault's epoch once open, for `go_online_after_sign_in`.
    pub(crate) fn create_vault(
        &self,
        prepared: PreparedVault,
        record: &AccountRecord,
    ) -> ClientResult<u64> {
        let mut vault = self.vault()?;
        vault.create_account_vault(prepared, record)?;
        let minutes = vault.settings()?.auto_lock_minutes;
        self.events.unlocked(minutes);
        Ok(vault.epoch())
    }

    /// The account this vault belongs to, from the local store (never from
    /// the caller).
    pub(crate) fn vault_account(&self) -> ClientResult<AccountRef> {
        Ok(self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?
            .to_ref()?)
    }

    /// Unlock. Every vault is account-bound: the account comes from the local
    /// store (never from the caller), and unlocking needs the Secret Key too
    /// — either typed from the Recovery Sheet (it is then saved here once the
    /// unlock succeeds) or the one already saved on this device.
    pub async fn unlock(
        self: &Arc<Self>,
        password: SecretString,
        secret_key: Option<SecretString>,
    ) -> ClientResult<VaultStatus> {
        let account = self.vault_account()?;
        let (stored, stored_text, definite) = {
            let mut device = self.device()?;
            let (text, definite) = device.secret_key_lookup(account.id);
            let key = text
                .as_ref()
                .and_then(|t| SecretKey::parse(t.expose()).ok());
            (key, text, definite)
        };
        let typed = match secret_key {
            Some(t) if !t.is_empty() => Some(SecretKey::parse(t.expose())?),
            _ => None,
        };
        // A keychain that failed or did not answer in time may hold the key.
        // Asking for the Recovery Sheet now would be wrong, and the typed key
        // would land in device.json because that keychain would not take it
        // either. The vault never left LOCKED, so nothing needs resetting.
        if typed.is_none() && stored.is_none() && !definite {
            return Err(ClientError::keychain_unavailable());
        }
        // `SecretKey` is deliberately not `Clone`, so both options move into
        // the blocking closure and are borrowed there, as the current code
        // does.
        if typed.is_none() && stored.is_none() {
            let ticket = self.vault()?.begin_unlock()?;
            let r = self
                .vault()?
                .finish_unlock(ticket, Err(havenkeys_core::Error::SecretKeyRequired));
            return Err(r
                .err()
                .unwrap_or(havenkeys_core::Error::SecretKeyRequired)
                .into());
        }
        let account_id = account.id;
        let key_text = typed.as_ref().map(|k| k.to_text());
        // Kept for the fallback below: `SecretKey` is not `Clone`, so the
        // text is re-parsed there. The typed key wins, as it does here.
        let key_text_for_fallback = key_text.clone().or(stored_text);
        let password_for_fallback = password.clone();
        let ticket = self.vault()?.begin_unlock()?;
        let joined = spawn_blocking(move || {
            // One Argon2id run yields both the KEK and the auth key: the
            // first opens the vault, the second opens the server session.
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
                self.lock("error");
                return Err(ClientError::internal());
            }
        };

        let (key, auth_key) = match derived {
            Ok((key, auth_key)) => (Ok(key), Some(auth_key)),
            Err(e) => (Err(e), None),
        };
        // One guard from `finish_unlock` to the `unlocked` event, so a
        // concurrent lock cannot fall between them. Scoped: it is released
        // before the fallback's requests, never held across an await.
        let unlocked = {
            let mut v = self.vault()?;
            match v.finish_unlock(ticket, key) {
                Ok(()) => {
                    let minutes = v.settings()?.auto_lock_minutes;
                    let status = v.status()?;
                    // Announced before releasing the vault lock, so the
                    // auto-lock clock is armed before it can tick against the
                    // previous session's timestamps, and a concurrent lock's
                    // `locked` event can never be overtaken by this one.
                    self.events.unlocked(minutes);
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
                // failure below is reported as the original wrong password,
                // so a caller learns nothing about which step failed.
                return match self
                    .unlock_from_server(password_for_fallback, sk_text, epoch)
                    .await
                {
                    Ok(status) => {
                        self.remember_typed_secret_key(account_id, key_text);
                        Ok(status)
                    }
                    Err(_) => Err(err.into()),
                };
            }
        };
        self.remember_typed_secret_key(account_id, key_text);
        // Open the server session in the background. The vault is already
        // usable: a device that cannot reach its server is offline and
        // read-only, not locked.
        if let Some(auth_key) = auth_key {
            let client = Arc::clone(self);
            tokio::spawn(async move {
                let _ = client.connect(auth_key).await;
            });
        }
        Ok(status)
    }

    /// A Secret Key typed from the Recovery Sheet proved correct: remember it.
    fn remember_typed_secret_key(&self, account: Uuid, key_text: Option<SecretString>) {
        if let Some(text) = key_text {
            if let Ok(k) = SecretKey::parse(text.expose()) {
                if let Ok(mut d) = self.device() {
                    let _ = d.set_secret_key(account, &k);
                }
            }
        }
    }

    /// Unlock with a header the server serves, when the local one no longer
    /// matches the password (changed on another device). The header is
    /// verified exactly as a sign-in verifies it, and adopted only if it is
    /// newer than everything this device has seen (`adopt_and_unlock`).
    ///
    /// Only reached from `unlock` after the local unlock failed with
    /// `unlock_failed`, so the vault is LOCKED on entry; it stays LOCKED
    /// unless the final adoption succeeds. The caller hides every error from
    /// here.
    async fn unlock_from_server(
        self: &Arc<Self>,
        password: SecretString,
        secret_key_text: SecretString,
        epoch: u64,
    ) -> ClientResult<VaultStatus> {
        let account = self.vault_account()?;
        let local_kdf = self.vault()?.kdf()?;
        let server = self.server()?;
        // Short, because this runs on every wrong password: an unreachable
        // server must not hold the unlock screen for the transport's full
        // timeout. The later requests keep the normal ones; the server has
        // answered by then.
        let params = tokio::time::timeout(
            FALLBACK_PARAMS_TIMEOUT,
            server.auth_params(account.email.as_str()),
        )
        .await
        .map_err(|_| havenkeys_core::Error::UnlockFailed)??;
        // Same parameters as the local header means the password really is
        // wrong: no change was made elsewhere, so there is nothing to fetch.
        // Another account ID means the server is not the one this vault
        // knows.
        if params.account_id != account.id || local_kdf.as_ref() == Some(&params.kdf) {
            return Err(havenkeys_core::Error::UnlockFailed.into());
        }
        let (pw, sk_text, acct) = (password.clone(), secret_key_text.clone(), account.clone());
        let kdf = params.kdf;
        let auth_key = spawn_blocking(move || {
            let sk = SecretKey::parse(sk_text.expose())?;
            vault::derive_auth_key(&pw, &sk, &kdf, &acct)
        })
        .await
        .map_err(|_| ClientError::internal())??;
        let session = server
            .login(
                account.email.as_str(),
                &auth_key,
                account.id,
                self.device_id()?,
                &self.config.device_name,
            )
            .await?;
        drop(auth_key);
        let header = server.header(&session).await?;
        let prepared = spawn_blocking(move || {
            let sk = SecretKey::parse(secret_key_text.expose())?;
            prepare_sign_in(&header.bytes, &password, &sk, &account).map(|(p, _)| p)
        })
        .await
        .map_err(|_| ClientError::internal())??;
        let status = {
            let mut v = self.vault()?;
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
            // As in `unlock`: announced under the vault lock, and the session
            // set there too, so a concurrent lock (which takes the vault lock
            // first) always drops it afterwards.
            self.events.unlocked(minutes);
            self.set_online(session);
            status
        };
        self.events.connectivity(true);
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let _ = client.sync_now().await;
        });
        Ok(status)
    }

    /// Re-check the master password before a sensitive action (an export).
    /// Argon2id runs without the vault lock. `UnlockFailed` if wrong.
    pub async fn verify_master_password(&self, password: SecretString) -> ClientResult<()> {
        let account = self.vault_account()?;
        let secret_key = self
            .device()?
            .secret_key(account.id)
            .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
        let check = self.vault()?.begin_password_check()?;
        spawn_blocking(move || check.verify(&password, &secret_key))
            .await
            .map_err(|_| ClientError::internal())??;
        Ok(())
    }

    /// Change the master password: server first, local second.
    ///
    /// The re-wrapped header goes to the server together with the current
    /// and the new login keys, in one request (`change_credentials`); the
    /// server applies it atomically at base + 1 and signs out every other
    /// device. Only once it has answered 2xx is the new wrap committed here.
    /// The other order would leave this device holding a header, and a
    /// password, the server has never heard of: every later sign-in with the
    /// new password would fail.
    pub async fn change_master_password(
        &self,
        current: SecretString,
        new: SecretString,
    ) -> ClientResult<()> {
        self.require_online()?;
        self.require_full()?;
        vault::check_new_master_password(&new)?;
        // Adopt a change made elsewhere first, so the base revision is
        // current.
        self.sync_now().await?;
        let kdf = KdfParams::generate()?;
        let account = self.vault_account()?;
        let secret_key = self
            .device()?
            .secret_key(account.id)
            .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
        // The account's address, for the re-check below: `account` moves
        // into the KDF task.
        let email = account.email.as_str().to_owned();
        let account_id = account.id;
        // The current KDF parameters, read under the same guard as the
        // ticket.
        let (ticket, old_kdf) = {
            let v = self.vault()?;
            let ticket = v.begin_rekey()?;
            (ticket, v.kdf()?.ok_or(havenkeys_core::Error::NoVault)?)
        };
        // Both Argon2id runs happen without the vault lock, so locking
        // (button, auto-lock, window close) is never delayed; the commit
        // re-checks the lock epoch and the header.
        let (ticket, rekeyed) = spawn_blocking(move || {
            let rekeyed = ticket.derive_for_account(&current, &new, kdf, &secret_key, &account);
            (ticket, rekeyed)
        })
        .await
        .map_err(|_| ClientError::internal())?;
        let rekeyed = rekeyed?;
        // Each `self.vault()?` guard below is a temporary, dropped at the end
        // of its statement: none is held across the request.
        let header = self.vault()?.encode_rekeyed_header(&ticket, &rekeyed)?;
        let (session, server) = (self.session()?, self.server()?);
        let sent = server
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
            .await;
        let revision = match sent {
            Ok(revision) => u64::try_from(revision).map_err(|_| ClientError::internal())?,
            // The request may have reached the server and been applied, with
            // only the answer lost. Saying "failed" then would send the user
            // back to a password that no longer works, and going offline
            // would stop the sync that would adopt the new header. So ask the
            // server which KDF parameters the account has now; the session
            // is kept.
            Err(e @ (SyncError::Unavailable | SyncError::Protocol(_))) => {
                let now = server
                    .auth_params(&email)
                    .await
                    .ok()
                    .filter(|p| p.account_id == account_id)
                    .map(|p| p.kdf);
                match change_outcome(now.as_ref(), &old_kdf, rekeyed.kdf()) {
                    // The server applies the change at base + 1.
                    ChangeOutcome::Applied => ticket.base_revision().saturating_add(1),
                    ChangeOutcome::NotApplied => return Err(e.into()),
                    ChangeOutcome::Unknown => return Err(ClientError::password_change_unknown()),
                }
            }
            Err(e) => return Err(credential_change_conflict(&e).unwrap_or_else(|| self.failed(e))),
        };
        // The server has it; from here on the change has happened.
        let committed = self.vault()?.commit_rekey(ticket, Ok(rekeyed), revision);
        match committed {
            // `Busy`: a background sync already adopted this very header
            // from the server. `Locked`: the vault was locked meanwhile; the
            // next unlock (via the server fallback) or sync adopts it. Either
            // way the change succeeded, and saying otherwise would send the
            // user back to a password that no longer works.
            Ok(()) | Err(havenkeys_core::Error::Busy | havenkeys_core::Error::Locked) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn list_devices(&self) -> ClientResult<Vec<DeviceEntry>> {
        // A locked vault has no session to use (DT1), even if one is held.
        self.require_unlocked()?;
        let (session, server) = (self.session()?, self.server()?);
        let devices = server.devices(&session).await?;
        Ok(devices
            .into_iter()
            .map(|d| DeviceEntry {
                id: d.id,
                name: d.name,
                created_at: d.created_at,
                last_seen_at: d.last_seen_at,
                current: d.current,
                approved_by: d.approved_by,
            })
            .collect())
    }

    /// Cut a device off. Revoking this one signs it out immediately.
    pub async fn revoke_device(&self, id: Uuid) -> ClientResult<()> {
        self.require_unlocked()?;
        let (session, server) = (self.session()?, self.server()?);
        server.revoke_device(&session, id).await?;
        if id == self.device_id()? {
            self.go_offline();
        }
        Ok(())
    }

    /// End this device's session and lock the vault. The vault stays on disk
    /// and can be unlocked again; the server session is gone.
    pub async fn sign_out(&self) -> ClientResult<()> {
        if let (Ok(session), Ok(server)) = (self.session(), self.server()) {
            let _ = server.logout(&session).await;
        }
        self.lock("user");
        self.events.connectivity(false);
        Ok(())
    }

    /// One value of the Account item and the vault's clipboard delay. The
    /// vault is released before the device store is taken. May wait on the
    /// platform store.
    pub fn account_value(&self, field: AccountField) -> ClientResult<(SecretString, u32)> {
        let (account, seconds) = {
            let v = self.vault()?;
            (unlocked_account(&v)?, v.settings()?.clipboard_clear_seconds)
        };
        let mut device = self.device()?;
        Ok((account_field(&account, &mut device, field)?, seconds))
    }
}

/// This device's record of a newly joined account, before its first sync.
pub(crate) fn new_account_record(
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

/// The account behind the Account item. Only while unlocked.
fn unlocked_account(vault: &VaultService) -> havenkeys_core::Result<AccountRecord> {
    if !vault.is_unlocked() {
        return Err(havenkeys_core::Error::Locked);
    }
    vault.account()?.ok_or(havenkeys_core::Error::NoVault)
}

/// One value of the Account item. The Secret Key comes from this device's
/// platform store (or `device.json`); `NotFound` when this device lacks it.
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

/// What a credential change whose answer was lost did, judged by the KDF
/// parameters the server serves for the account afterwards. The new
/// parameters carry a fresh random salt, so they cannot be confused with
/// the old ones.
#[derive(Debug, PartialEq, Eq)]
enum ChangeOutcome {
    /// The server serves the new parameters: it applied the change.
    Applied,
    /// The server still serves the old ones: nothing changed.
    NotApplied,
    /// No answer, or parameters that are neither.
    Unknown,
}

fn change_outcome(server: Option<&KdfParams>, old: &KdfParams, new: &KdfParams) -> ChangeOutcome {
    match server {
        Some(k) if k == new => ChangeOutcome::Applied,
        Some(k) if k == old => ChangeOutcome::NotApplied,
        _ => ChangeOutcome::Unknown,
    }
}

/// A 409 on a credential change: the header moved on the server since this
/// device read it, which only another device's password change does. The
/// password this device knows is no longer the account's.
fn credential_change_conflict(err: &SyncError) -> Option<ClientError> {
    matches!(err, SyncError::Conflict(_)).then(ClientError::password_changed_elsewhere)
}

#[cfg(test)]
mod tests {
    mod sign_in {
        use crate::client::tests::client_with_store;
        use crate::client::HavenClient;
        use crate::key_store::{KeyStore, MemoryKeyStore, StoreError};
        use crate::stub_server::{StubAccount, StubServer};
        use havenkeys_core::account::{AccountRef, NormalizedEmail};
        use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
        use havenkeys_core::sync::encode_header_for;
        use havenkeys_core::vault::{prepare_new_account_vault, VaultState};
        use havenkeys_core::SecretString;
        use std::sync::{Arc, OnceLock, Weak};
        use uuid::Uuid;

        /// A keychain whose save is where the vault locks: the auto-lock
        /// firing while a keychain prompt waits for the user.
        struct LocksOnSave {
            client: Arc<OnceLock<Weak<HavenClient>>>,
            inner: MemoryKeyStore,
        }

        impl KeyStore for LocksOnSave {
            fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
                self.inner.get(account)
            }

            fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
                if let Some(client) = self.client.get().and_then(Weak::upgrade) {
                    client.lock("auto");
                }
                self.inner.set(account, value)
            }

            fn delete(&self, account: Uuid) -> Result<(), StoreError> {
                self.inner.delete(account)
            }
        }

        #[tokio::test]
        async fn a_lock_while_saving_the_secret_key_leaves_the_device_offline() {
            let password = "correct horse battery staple";
            let account = AccountRef::new(
                Uuid::from_u128(1),
                NormalizedEmail::parse("user@example.com").unwrap(),
            );
            let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
            let made = prepare_new_account_vault(
                &SecretString::from(password),
                &account,
                kdf,
                1_700_000_000_000,
            )
            .unwrap();
            let server = StubServer::start(StubAccount {
                account_id: account.id,
                vault_id: made.prepared.vault_id(),
                kdf: made.prepared.kdf().clone(),
                header: encode_header_for(&made.prepared).unwrap(),
                header_revision: made.prepared.header_revision() as i64,
            })
            .await;
            server.release_login.notify_one();

            let dir = tempfile::tempdir().unwrap();
            let slot = Arc::new(OnceLock::new());
            let store = LocksOnSave {
                client: slot.clone(),
                inner: MemoryKeyStore::default(),
            };
            let (client, events) = client_with_store(dir.path(), Box::new(store));
            slot.set(Arc::downgrade(&client)).unwrap();

            let status = client
                .sign_in(
                    server.url.clone(),
                    "user@example.com".into(),
                    SecretString::from(password),
                    Some(made.secret_key.to_text()),
                )
                .await
                .unwrap();
            assert!(events.seen().iter().any(|e| e == "locked:auto:true"));
            assert_eq!(status.state, VaultState::Locked);
            assert!(status.vault_exists);
            assert!(!client.is_online());
            assert!(!events.seen().iter().any(|e| e == "online:true"));
        }
    }

    mod account_item {
        use super::super::{account_field, unlocked_account, AccountField};
        use crate::device::Device;
        use crate::key_store::MemoryKeyStore;
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

    mod credential_change {
        use super::super::*;

        #[test]
        fn a_lost_credential_change_is_judged_by_the_servers_kdf() {
            let old = KdfParams::generate().unwrap();
            let new = KdfParams::generate().unwrap();
            assert_ne!(old, new, "a fresh salt every time");
            assert_eq!(
                change_outcome(Some(&new), &old, &new),
                ChangeOutcome::Applied
            );
            assert_eq!(
                change_outcome(Some(&old), &old, &new),
                ChangeOutcome::NotApplied
            );
            let other = KdfParams::generate().unwrap();
            assert_eq!(
                change_outcome(Some(&other), &old, &new),
                ChangeOutcome::Unknown
            );
            assert_eq!(change_outcome(None, &old, &new), ChangeOutcome::Unknown);
        }

        #[test]
        fn an_unconfirmed_password_change_says_so() {
            let err = ClientError::password_change_unknown();
            assert_eq!(err.code, "password_change_unknown");
            assert!(err.message.contains("use the new one"));
        }

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

    mod invite_preview {
        #[test]
        fn preview_invite_decodes_without_touching_the_network() {
            let json = br#"{"server":"https://vault.example.com","email":"Me@Example.com","account":"00000000-0000-0000-0000-000000000001","secret":"AAAAAAAAAAAAAAAAAAAAAA"}"#;
            let raw = format!("HKINV1-{}", data_encoding::BASE64URL_NOPAD.encode(json));
            let dir = tempfile::tempdir().unwrap();
            let (client, _) = crate::client::tests::client_in(dir.path());
            let preview = client.preview_invite(&raw).unwrap();
            assert_eq!(preview.email, "me@example.com");
            assert_eq!(preview.server_url, "https://vault.example.com");
            assert_eq!(
                client.preview_invite("HKINV1-nope").err().unwrap().code,
                "invalid_input"
            );
        }
    }
}
