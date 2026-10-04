//! Signing in a new device from an approving one
//! (spec 2026-10-03-phone-approved-sign-in). The desktop side starts a
//! pairing, polls for the answer and builds its vault from the envelope; the
//! phone side reads a scanned code, and approves or denies it.

use crate::account::new_account_record;
use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use havenkeys_core::pairing::{ClaimSecret, PairingKeys, PairingLink};
use havenkeys_core::sync::prepare_paired_sign_in;
use havenkeys_core::vault::VaultStatus;
use havenkeys_sync_client::client::Claim;
use havenkeys_sync_client::SyncError;
use std::sync::Arc;

/// What the desktop shows: the QR code's text and when it stops working.
#[derive(Debug)]
pub struct PairingStart {
    pub link: String,
    pub expires_at: String,
}

#[derive(Debug)]
pub enum PairingPoll {
    Waiting,
    Denied,
    Expired,
    Approved(VaultStatus),
}

/// What the phone shows before Allow. From the server: shown, not trusted.
#[derive(Debug)]
pub struct PairingRequest {
    pub link: String,
    pub device_name: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: String,
}

/// The desktop's half of one pairing, in memory only.
pub(crate) struct PendingPairing {
    keys: PairingKeys,
    claim: ClaimSecret,
    server_url: String,
    pairing_id: String,
}

fn normalize(server_url: &str) -> String {
    server_url.trim().trim_end_matches('/').to_string()
}

impl HavenClient {
    pub async fn start_pairing(
        &self,
        server_url: String,
        device_name: &str,
    ) -> ClientResult<PairingStart> {
        if self.vault()?.key_scheme()?.is_some() {
            return Err(havenkeys_core::Error::VaultExists.into());
        }
        let server_url = normalize(&server_url);
        let server = self.server_for(&server_url)?;
        let keys = PairingKeys::generate();
        let claim = ClaimSecret::generate()?;
        let created = server
            .create_pairing(self.device_id()?, device_name, &claim.hash())
            .await?;
        // The public key goes only into the QR code: whoever seals to it has
        // seen this screen.
        let link = PairingLink {
            server_url: server_url.clone(),
            pairing_id: created.pairing_id.clone(),
            public_key: keys.public_key(),
        }
        .to_text();
        *self.pairing.lock().map_err(|_| ClientError::internal())? = Some(PendingPairing {
            keys,
            claim,
            server_url,
            pairing_id: created.pairing_id,
        });
        Ok(PairingStart {
            link,
            expires_at: created.expires_at,
        })
    }

    pub fn cancel_pairing(&self) {
        if let Ok(mut p) = self.pairing.lock() {
            *p = None;
        }
    }

    /// One claim attempt. Any answer but `Waiting` ends the pairing.
    pub async fn poll_pairing(self: &Arc<Self>) -> ClientResult<PairingPoll> {
        // One poll at a time: a second caller waits for the first, so an
        // approval consumed by one poll cannot be cancelled by another.
        let _gate = self.pairing_gate.lock().await;
        let (server_url, pairing_id, secret) = {
            let guard = self.pairing.lock().map_err(|_| ClientError::internal())?;
            let p = guard.as_ref().ok_or_else(ClientError::pairing_gone)?;
            (
                p.server_url.clone(),
                p.pairing_id.clone(),
                zeroize::Zeroizing::new(*p.claim.as_bytes()),
            )
        };
        let server = self.server_for(&server_url)?;
        let answer = server.claim_pairing(&pairing_id, &secret).await;
        let (session, envelope) = match answer {
            Ok(Claim::Waiting) => return Ok(PairingPoll::Waiting),
            Ok(Claim::Denied) => {
                self.cancel_pairing_if(&pairing_id);
                return Ok(PairingPoll::Denied);
            }
            Err(SyncError::Refused(_)) => {
                self.cancel_pairing_if(&pairing_id);
                return Ok(PairingPoll::Expired);
            }
            Err(e) => return Err(e.into()),
            Ok(Claim::Approved { session, envelope }) => (session, envelope),
        };
        // Only the pairing this poll claimed; a newer one is left alone.
        let pending = {
            let mut guard = self.pairing.lock().map_err(|_| ClientError::internal())?;
            if guard.as_ref().is_some_and(|p| p.pairing_id == pairing_id) {
                guard.take()
            } else {
                None
            }
        }
        .ok_or_else(ClientError::pairing_gone)?;
        let status = self.finish_pairing(pending, session, envelope).await?;
        Ok(PairingPoll::Approved(status))
    }

    /// Ends the pending pairing only if it is still `pairing_id`.
    fn cancel_pairing_if(&self, pairing_id: &str) {
        if let Ok(mut p) = self.pairing.lock() {
            if p.as_ref().is_some_and(|x| x.pairing_id == pairing_id) {
                *p = None;
            }
        }
    }

    async fn finish_pairing(
        self: &Arc<Self>,
        pending: PendingPairing,
        session: havenkeys_sync_client::Session,
        envelope: Vec<u8>,
    ) -> ClientResult<VaultStatus> {
        let payload = pending
            .keys
            .open(&pending.server_url, &pending.pairing_id, &envelope)
            .map_err(|_| ClientError::pairing_failed())?;
        // The server named the account and vault; the envelope proves them.
        if payload.account_id != session.account_id || payload.vault_id != session.vault_id {
            return Err(ClientError::pairing_failed());
        }
        let server = self.server_for(&pending.server_url)?;
        let header = server
            .header(&session)
            .await
            .map_err(|_| ClientError::pairing_failed())?;
        let paired = prepare_paired_sign_in(&header.bytes, payload)
            .map_err(|_| ClientError::pairing_failed())?;
        let revision = paired.prepared.header_revision() as i64;
        let record = new_account_record(&paired.account, pending.server_url.clone(), revision);
        self.create_vault(paired.prepared, &record)?;
        self.device()?
            .set_secret_key(paired.account.id, &paired.secret_key)
            .map_err(|_| ClientError::file())?;
        self.go_online_after_sign_in(session)
    }

    /// The phone: parse a scanned code and ask the server about it.
    pub async fn pairing_request(&self, link: &str) -> ClientResult<PairingRequest> {
        let link = self.pairing_link_for_this_account(link)?;
        let (session, server) = (self.session()?, self.server()?);
        let details = server
            .pairing_details(&session, &link.pairing_id)
            .await
            .map_err(|e| self.pairing_error(e))?;
        Ok(PairingRequest {
            link: link.to_text(),
            device_name: details.device_name,
            ip: details.ip,
            location: details.location,
            created_at: details.created_at,
        })
    }

    /// The phone: seal this vault's keys to the code's device and approve.
    pub async fn approve_pairing(&self, link: &str) -> ClientResult<()> {
        let link = self.pairing_link_for_this_account(link)?;
        let envelope = {
            let vault = self.vault()?;
            let account = vault.account()?.ok_or(havenkeys_core::Error::NoVault)?;
            let secret_key = self
                .device()?
                .secret_key(account.account_id)
                .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
            vault.seal_pairing(&link, &account, secret_key)?
        };
        let (session, server) = (self.session()?, self.server()?);
        server
            .approve_pairing(&session, &link.pairing_id, &envelope)
            .await
            .map_err(|e| self.pairing_error(e))
    }

    pub async fn deny_pairing(&self, link: &str) -> ClientResult<()> {
        let link = self.pairing_link_for_this_account(link)?;
        let (session, server) = (self.session()?, self.server()?);
        server
            .deny_pairing(&session, &link.pairing_id)
            .await
            .map_err(|e| self.pairing_error(e))
    }

    /// Unlocked, online, and a code for this account's server.
    fn pairing_link_for_this_account(&self, link: &str) -> ClientResult<PairingLink> {
        self.require_unlocked()?;
        let link = PairingLink::parse(link)?;
        let ours = self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?
            .server_url;
        if normalize(&link.server_url) != normalize(&ours) {
            return Err(ClientError::pairing_other_server());
        }
        self.require_online()?;
        Ok(link)
    }

    /// A gone pairing (404) is `pairing_gone`; any other refusal of the code
    /// (a device that cannot be approved, a bad envelope) is
    /// `pairing_failed`. Only a refused session or no answer go through the
    /// usual handling, which may sign this device out.
    fn pairing_error(&self, e: SyncError) -> ClientError {
        match e {
            SyncError::Refused("not found") => ClientError::pairing_gone(),
            SyncError::Refused(_) => ClientError::pairing_failed(),
            other => self.failed(other),
        }
    }
}
