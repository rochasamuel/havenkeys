//! One method per route, and the rule that the server is never trusted.
//!
//! Nothing here decides what an item means: blobs are carried, not opened.
//! What this layer does decide is whether an answer is shaped like the
//! protocol says, and it refuses rather than guesses.

use crate::error::{Conflict, Result, SyncError};
use crate::session::Session;
use crate::transport::{HttpRequest, HttpResponse, Method, Transport};
use crate::wire::{self, MAX_CHANGES, MAX_HEADER_BYTES};
use data_encoding::{BASE64, BASE64URL_NOPAD};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::crypto::keys::AuthKey;
use havenkeys_core::sync::RemoteChange;
use havenkeys_core::vault::StagedWrite;
use uuid::Uuid;

/// The key scheme this client speaks, and the only one it will accept.
pub const KEY_SCHEME: i16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activated {
    pub account_id: Uuid,
    pub vault_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AuthParams {
    pub account_id: Uuid,
    pub kdf: KdfParams,
}

/// A fetch answer: the items that fit, and the asked-for items that did not
/// (ask for those again; they are not deleted).
#[derive(Debug)]
pub struct Fetched {
    pub changes: Vec<RemoteChange>,
    pub unanswered: Vec<Uuid>,
}

pub struct Pulled {
    pub cursor: i64,
    pub has_more: bool,
    pub changes: Vec<RemoteChange>,
}

impl std::fmt::Debug for Pulled {
    /// Counts only. The changes carry ciphertext, and printing ciphertext is
    /// still printing vault contents.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pulled")
            .field("cursor", &self.cursor)
            .field("has_more", &self.has_more)
            .field("changes", &self.changes.len())
            .field(
                "deletions",
                &self.changes.iter().filter(|c| c.deleted).count(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteAck {
    pub cursor: i64,
    /// `(item_id, revision)` for each change the server accepted.
    pub applied: Vec<(Uuid, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: Uuid,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub current: bool,
    /// The device that approved this one's sign-in, when it came by a pairing.
    pub approved_by: Option<Uuid>,
}

pub struct CreatedPairing {
    pub pairing_id: String,
    pub expires_at: String,
}

#[derive(Clone, Debug)]
pub struct PairingDetails {
    pub device_name: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

pub enum Claim {
    Waiting,
    Denied,
    Approved { session: Session, envelope: Vec<u8> },
}

impl std::fmt::Debug for Claim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Claim::Waiting => "Claim::Waiting",
            Claim::Denied => "Claim::Denied",
            Claim::Approved { .. } => "Claim::Approved(<redacted>)",
        })
    }
}

/// `/v1/pairings/{id}{suffix}`, only for an id that cannot change the path.
fn pairing_path(pairing_id: &str, suffix: &str) -> Result<String> {
    if !havenkeys_core::pairing::valid_pairing_id(pairing_id) {
        return Err(SyncError::Refused("not found"));
    }
    Ok(format!("/v1/pairings/{pairing_id}{suffix}"))
}

/// What a device needs to activate an account.
pub struct Activation<'a> {
    pub email: &'a str,
    pub invite: &'a str,
    pub kdf: &'a KdfParams,
    pub auth_key: &'a AuthKey,
    pub vault_id: Uuid,
    /// The attested `header.json` from `VaultService::encode_account_header`.
    pub header: &'a [u8],
}

/// A master-password change: the login verifier, the KDF parameters and the
/// re-wrapped header move together in one request, so the server never sees
/// a device authenticated with a new key against an old header or vice
/// versa.
pub struct CredentialChange<'a> {
    pub current_auth_key: &'a AuthKey,
    pub kdf: &'a KdfParams,
    pub new_auth_key: &'a AuthKey,
    /// The attested `header.json`, re-wrapped under the new credentials.
    pub header: &'a [u8],
    pub base_header_revision: i64,
}

pub struct SyncClient<T: Transport> {
    transport: T,
}

impl<T: Transport> SyncClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Is the server there? Used to decide online/offline, never for
    /// authorization.
    pub async fn health(&self) -> Result<()> {
        let response = self.get("/v1/health", None).await?;
        match response.status {
            200 => Ok(()),
            _ => Err(SyncError::Unavailable),
        }
    }

    pub async fn activate(&self, activation: Activation<'_>) -> Result<Activated> {
        check_header(activation.header)?;
        let body = wire::ActivateBody {
            email: activation.email,
            invite: activation.invite,
            kdf: activation.kdf.into(),
            auth_key: &activation.auth_key.to_base64(),
            vault_id: activation.vault_id,
            header: BASE64.encode(activation.header),
            key_scheme: KEY_SCHEME,
        };
        let response = self.post("/v1/accounts/activate", None, &body).await?;
        let dto: wire::ActivatedDto = expect_ok(response)?;
        Ok(Activated {
            account_id: dto.account_id,
            vault_id: dto.vault_id,
        })
    }

    /// The account id and KDF parameters a new device needs before it can
    /// derive anything. The answer is the same shape whether or not the
    /// account exists, so nothing here may be treated as proof that it does.
    pub async fn auth_params(&self, email: &str) -> Result<AuthParams> {
        let body = wire::ParamsBody { email };
        let response = self.post("/v1/auth/params", None, &body).await?;
        let dto: wire::ParamsDto = expect_ok(response)?;
        Ok(AuthParams {
            account_id: dto.account_id,
            kdf: dto.kdf.try_into()?,
        })
    }

    /// `account_id` is the one the caller already derived keys against —
    /// from the invite or from `auth_params`. It is carried into the session
    /// rather than read from the answer, because the server saying "you are
    /// account X" would be the server choosing which account a device
    /// believes it is signed in to.
    pub async fn login(
        &self,
        email: &str,
        auth_key: &AuthKey,
        account_id: Uuid,
        device_id: Uuid,
        device_name: &str,
    ) -> Result<Session> {
        let body = wire::LoginBody {
            email,
            auth_key: &auth_key.to_base64(),
            device_id,
            device_name,
        };
        let response = self.post("/v1/auth/login", None, &body).await?;
        let dto: wire::LoginDto = expect_ok(response)?;
        if dto.token.is_empty() || dto.token.len() > 128 {
            return Err(SyncError::Protocol("token"));
        }
        Ok(Session::new(
            dto.token,
            dto.expires_at,
            account_id,
            dto.vault_id,
        ))
    }

    pub async fn logout(&self, session: &Session) -> Result<()> {
        let response = self
            .send(Method::Post, "/v1/auth/logout", Some(session), None)
            .await?;
        match response.status {
            // A session that is already gone is a successful logout.
            204 | 200 | 401 => Ok(()),
            _ => Err(error_from(&response)),
        }
    }

    pub async fn header(&self, session: &Session) -> Result<wire::RemoteHeader> {
        let response = self.get("/v1/vault/header", Some(session)).await?;
        let dto: wire::HeaderDto = expect_ok(response)?;
        let header: wire::RemoteHeader = dto.try_into()?;
        if header.key_scheme < KEY_SCHEME {
            // A downgrade attempt. The core would refuse it too, but there is
            // no reason to carry it that far.
            return Err(SyncError::Protocol("key scheme"));
        }
        Ok(header)
    }

    /// Change the master password on the server: the header, the login
    /// verifier and the KDF parameters move together, and every other
    /// session on the account ends. Nothing local changes here; the caller
    /// commits the new wrap only once this returns.
    pub async fn change_credentials(
        &self,
        session: &Session,
        change: CredentialChange<'_>,
    ) -> Result<i64> {
        check_header(change.header)?;
        if change.base_header_revision < 0 {
            return Err(SyncError::Refused("header revision is not valid"));
        }
        // `to_base64` returns `Zeroizing<String>`; bound here so the copies
        // are wiped when this function returns rather than living for as
        // long as the caller's `CredentialChange`.
        let current = change.current_auth_key.to_base64();
        let new = change.new_auth_key.to_base64();
        let body = wire::CredentialsBody {
            current_auth_key: &current,
            kdf: change.kdf.into(),
            new_auth_key: &new,
            header: BASE64.encode(change.header),
            base_header_revision: change.base_header_revision,
        };
        let response = self
            .post("/v1/account/credentials", Some(session), &body)
            .await?;
        let dto: wire::CredentialsAckDto = expect_ok(response)?;
        // The server assigns base + 1 and nothing else. Anything different
        // would leave this device committing a revision the server never
        // stored.
        if dto.header_revision != change.base_header_revision + 1 {
            return Err(SyncError::Protocol("header revision"));
        }
        Ok(dto.header_revision)
    }

    /// Delete the account and everything the server holds for it. The
    /// caller erases the local copy only once this returns `Ok`.
    pub async fn delete_account(
        &self,
        session: &Session,
        current_auth_key: &AuthKey,
        email: &str,
    ) -> Result<()> {
        let current = current_auth_key.to_base64();
        let body = wire::AccountDeletionBody {
            current_auth_key: &current,
            email,
        };
        let response = self
            .post("/v1/account/delete", Some(session), &body)
            .await?;
        match response.status {
            204 | 200 => Ok(()),
            _ => Err(error_from(&response)),
        }
    }

    /// One page of changes after `since`. The caller keeps calling while
    /// `has_more` is true, passing the cursor it was given.
    pub async fn pull(&self, session: &Session, since: i64) -> Result<Pulled> {
        if since < 0 {
            return Err(SyncError::Refused("cursor is not valid"));
        }
        let response = self
            .get(&format!("/v1/sync?since={since}"), Some(session))
            .await?;
        let dto: wire::PullDto = expect_ok(response)?;
        if dto.cursor < 0 {
            return Err(SyncError::Protocol("cursor"));
        }
        if dto.changes.len() > MAX_CHANGES {
            return Err(SyncError::Protocol(
                "page is larger than the protocol allows",
            ));
        }
        let changes = dto
            .changes
            .into_iter()
            .map(RemoteChange::try_from)
            .collect::<Result<Vec<_>>>()?;
        Ok(Pulled {
            cursor: dto.cursor,
            has_more: dto.has_more,
            changes,
        })
    }

    /// Send staged writes. A 409 comes back as `SyncError::Conflict` naming
    /// the items: nothing was written, and the caller pulls rather than
    /// retrying blindly.
    pub async fn write(&self, session: &Session, staged: &[StagedWrite]) -> Result<WriteAck> {
        if staged.is_empty() {
            return Err(SyncError::Refused("nothing to write"));
        }
        if staged.len() > MAX_CHANGES {
            return Err(SyncError::Refused("too many changes in one batch"));
        }
        let changes = staged
            .iter()
            .map(wire::ChangeDto::try_from)
            .collect::<Result<Vec<_>>>()?;
        let response = self
            .post("/v1/items", Some(session), &wire::WriteBody { changes })
            .await?;
        if response.status == 409 {
            let dto: wire::ConflictsDto = wire::parse(&response.body)?;
            let conflicts: Vec<Conflict> = dto.conflicts.into_iter().map(Conflict::from).collect();
            if conflicts.is_empty() {
                return Err(SyncError::Protocol("a conflict naming no items"));
            }
            return Err(SyncError::Conflict(conflicts));
        }
        let dto: wire::WriteAckDto = expect_ok(response)?;
        if dto.applied.len() != staged.len() {
            return Err(SyncError::Protocol(
                "the server acknowledged a different batch",
            ));
        }
        Ok(WriteAck {
            cursor: dto.cursor,
            applied: dto
                .applied
                .into_iter()
                .map(|a| (a.item_id, a.revision))
                .collect(),
        })
    }

    /// The current version of specific items, for retrying ones that did not
    /// open. Every returned change must be for an ID that was asked for, at
    /// most once, and an unanswered ID must have been asked for and not also
    /// answered.
    pub async fn fetch_items(&self, session: &Session, ids: &[Uuid]) -> Result<Fetched> {
        if ids.is_empty() || ids.len() > MAX_CHANGES {
            return Err(SyncError::Refused("item list is not valid"));
        }
        let response = self
            .post(
                "/v1/items/fetch",
                Some(session),
                &wire::FetchBody { item_ids: ids },
            )
            .await?;
        let dto: wire::FetchDto = expect_ok(response)?;
        let asked: std::collections::HashSet<Uuid> = ids.iter().copied().collect();
        let mut seen = std::collections::HashSet::new();
        for id in dto
            .changes
            .iter()
            .map(|c| c.item_id)
            .chain(dto.unanswered.iter().copied())
        {
            if !asked.contains(&id) || !seen.insert(id) {
                return Err(SyncError::Protocol("an item that was not asked for"));
            }
        }
        Ok(Fetched {
            changes: dto
                .changes
                .into_iter()
                .map(RemoteChange::try_from)
                .collect::<Result<Vec<_>>>()?,
            unanswered: dto.unanswered,
        })
    }

    pub async fn devices(&self, session: &Session) -> Result<Vec<Device>> {
        let response = self.get("/v1/devices", Some(session)).await?;
        let dto: Vec<wire::DeviceDto> = expect_ok(response)?;
        Ok(dto
            .into_iter()
            .map(|d| Device {
                id: d.id,
                name: d.name,
                created_at: d.created_at,
                last_seen_at: d.last_seen_at,
                current: d.current,
                approved_by: d.approved_by,
            })
            .collect())
    }

    pub async fn revoke_device(&self, session: &Session, device_id: Uuid) -> Result<()> {
        let response = self
            .send(
                Method::Delete,
                &format!("/v1/devices/{device_id}"),
                Some(session),
                None,
            )
            .await?;
        match response.status {
            200 | 204 => Ok(()),
            _ => Err(error_from(&response)),
        }
    }

    /// Open a pairing request for this (new) device. No session. The device's
    /// public key is not sent: it travels only in the QR code, so the server
    /// cannot seal an envelope of its own to it.
    pub async fn create_pairing(
        &self,
        device_id: Uuid,
        device_name: &str,
        claim_hash: &[u8; 32],
    ) -> Result<CreatedPairing> {
        let body = wire::CreatePairingBody {
            device_id,
            device_name,
            claim_hash: BASE64URL_NOPAD.encode(claim_hash),
        };
        let dto: wire::CreatedPairingDto =
            expect_ok(self.post("/v1/pairings", None, &body).await?)?;
        if !havenkeys_core::pairing::valid_pairing_id(&dto.pairing_id) || dto.expires_at.len() > 64
        {
            return Err(SyncError::Protocol("pairing"));
        }
        Ok(CreatedPairing {
            pairing_id: dto.pairing_id,
            expires_at: dto.expires_at,
        })
    }

    /// What the server says about a pairing. Shown to the user, never trusted
    /// for anything else.
    pub async fn pairing_details(
        &self,
        session: &Session,
        pairing_id: &str,
    ) -> Result<PairingDetails> {
        let path = pairing_path(pairing_id, "")?;
        let dto: wire::PairingDetailsDto = expect_ok(self.get(&path, Some(session)).await?)?;
        let short = |s: &str| s.chars().count() <= 64;
        if !short(&dto.device_name)
            || !short(&dto.ip)
            || !dto.location.as_deref().is_none_or(short)
            || dto.created_at.len() > 64
            || dto.expires_at.len() > 64
        {
            return Err(SyncError::Protocol("pairing"));
        }
        Ok(PairingDetails {
            device_name: dto.device_name,
            ip: dto.ip,
            location: dto.location,
            created_at: dto.created_at,
            expires_at: dto.expires_at,
        })
    }

    pub async fn approve_pairing(
        &self,
        session: &Session,
        pairing_id: &str,
        envelope: &[u8],
    ) -> Result<()> {
        let path = pairing_path(pairing_id, "/approve")?;
        let body = wire::ApprovePairingBody {
            envelope: BASE64.encode(envelope),
        };
        let response = self.post(&path, Some(session), &body).await?;
        match response.status {
            200 | 204 => Ok(()),
            _ => Err(error_from(&response)),
        }
    }

    pub async fn deny_pairing(&self, session: &Session, pairing_id: &str) -> Result<()> {
        let path = pairing_path(pairing_id, "/deny")?;
        let response = self.send(Method::Post, &path, Some(session), None).await?;
        match response.status {
            200 | 204 => Ok(()),
            _ => Err(error_from(&response)),
        }
    }

    /// Ask whether the pairing was approved. The session comes from here,
    /// but the caller checks its account and vault against the envelope.
    pub async fn claim_pairing(&self, pairing_id: &str, claim_secret: &[u8; 32]) -> Result<Claim> {
        let path = pairing_path(pairing_id, "/claim")?;
        let body = wire::ClaimPairingBody {
            claim_secret: BASE64URL_NOPAD.encode(claim_secret),
        };
        let dto: wire::ClaimDto = expect_ok(self.post(&path, None, &body).await?)?;
        Ok(match dto {
            wire::ClaimDto::Waiting => Claim::Waiting,
            wire::ClaimDto::Denied => Claim::Denied,
            wire::ClaimDto::Approved {
                token,
                expires_at,
                account_id,
                vault_id,
                envelope,
            } => {
                if token.is_empty() || token.len() > 128 || expires_at.len() > 64 {
                    return Err(SyncError::Protocol("token"));
                }
                if envelope.len() > havenkeys_core::pairing::MAX_ENVELOPE_LEN / 3 * 4 + 4 {
                    return Err(SyncError::TooLarge);
                }
                let envelope = BASE64
                    .decode(envelope.as_bytes())
                    .map_err(|_| SyncError::Protocol("envelope"))?;
                Claim::Approved {
                    session: Session::new(token, expires_at, account_id, vault_id),
                    envelope,
                }
            }
        })
    }

    async fn get(&self, path: &str, session: Option<&Session>) -> Result<HttpResponse> {
        self.send(Method::Get, path, session, None).await
    }

    /// POST `body` as JSON.
    async fn post<B: serde::Serialize>(
        &self,
        path: &str,
        session: Option<&Session>,
        body: &B,
    ) -> Result<HttpResponse> {
        let body = serde_json::to_vec(body).map_err(|_| SyncError::Protocol("request encoding"))?;
        self.send(Method::Post, path, session, Some(body)).await
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        session: Option<&Session>,
        body: Option<Vec<u8>>,
    ) -> Result<HttpResponse> {
        self.transport
            .send(HttpRequest {
                method,
                path: path.to_string(),
                token: session.map(Session::token),
                body,
            })
            .await
    }
}

/// An attested header this client is willing to send.
fn check_header(header: &[u8]) -> Result<()> {
    if header.is_empty() || header.len() > MAX_HEADER_BYTES {
        return Err(SyncError::Refused("header is not valid"));
    }
    Ok(())
}

/// A 200 answer's body, parsed; any other status as its error.
fn expect_ok<D: serde::de::DeserializeOwned>(response: HttpResponse) -> Result<D> {
    if response.status != 200 {
        return Err(error_from(&response));
    }
    wire::parse(&response.body)
}

/// An error answer as its error. A 410 means the account was deleted only
/// when the server itself says so: a proxy's or CDN's 410 in front of a live
/// server must never make a device erase its copy. Only the code is read;
/// the server's message is never used.
fn error_from(response: &HttpResponse) -> SyncError {
    #[derive(serde::Deserialize)]
    struct Body {
        error: Code,
    }
    #[derive(serde::Deserialize)]
    struct Code {
        code: String,
    }
    if response.status == 410 {
        let said = serde_json::from_slice::<Body>(&response.body).ok();
        if said.is_some_and(|b| b.error.code == "account_deleted") {
            return SyncError::AccountDeleted;
        }
    }
    error_for(response.status)
}

/// Map a status to an error. The server's own message is never used: it
/// is attacker-controlled text, and the client has its own words for
/// every case it can act on.
fn error_for(status: u16) -> SyncError {
    match status {
        401 | 403 => SyncError::Unauthorized,
        404 => SyncError::Refused("not found"),
        409 => SyncError::Conflict(Vec::new()),
        413 => SyncError::Refused("too large"),
        429 => SyncError::RateLimited,
        400..=499 => SyncError::Refused("the request was refused"),
        _ => SyncError::Unavailable,
    }
}

#[cfg(test)]
mod pairing_tests {
    use super::*;

    #[test]
    fn a_pairing_id_that_could_change_the_path_is_refused_before_sending() {
        assert!(pairing_path("AAAAAAAAAAAAAAAAAAAAAA", "claim").is_ok());
        for bad in [
            "",
            "../devices",
            "AAAAAAAAAAAAAAAAAAAAA/",
            "A".repeat(23).as_str(),
        ] {
            assert!(pairing_path(bad, "claim").is_err());
        }
    }
}

#[cfg(test)]
mod deletion_tests {
    use super::*;

    fn answer(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn only_the_servers_own_410_means_the_account_was_deleted() {
        let deleted = r#"{"error":{"code":"account_deleted","message":"x"}}"#;
        assert_eq!(error_from(&answer(410, deleted)), SyncError::AccountDeleted);
        // A proxy's or CDN's 410 must never erase a device.
        assert_ne!(
            error_from(&answer(410, "<html>Gone</html>")),
            SyncError::AccountDeleted
        );
        assert_ne!(
            error_from(&answer(410, r#"{"error":{"code":"gone"}}"#)),
            SyncError::AccountDeleted
        );
        assert_eq!(error_from(&answer(401, deleted)), SyncError::Unauthorized);
    }
}
