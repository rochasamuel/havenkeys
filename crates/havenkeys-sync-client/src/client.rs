//! One method per route, and the rule that the server is never trusted.
//!
//! Nothing here decides what an item means: blobs are carried, not opened.
//! What this layer does decide is whether an answer is shaped like the
//! protocol says, and it refuses rather than guesses.

use crate::error::{Conflict, Result, SyncError};
use crate::session::Session;
use crate::transport::{HttpRequest, HttpResponse, Method, Transport};
use crate::wire::{self, MAX_CHANGES, MAX_HEADER_BYTES};
use data_encoding::BASE64;
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
        let response = self.send(Method::Get, "/v1/health", None, None).await?;
        match response.status {
            200 => Ok(()),
            _ => Err(SyncError::Unavailable),
        }
    }

    pub async fn activate(&self, activation: Activation<'_>) -> Result<Activated> {
        if activation.header.is_empty() || activation.header.len() > MAX_HEADER_BYTES {
            return Err(SyncError::Refused("header is not valid"));
        }
        let body = wire::ActivateBody {
            email: activation.email,
            invite: activation.invite,
            kdf: activation.kdf.into(),
            auth_key: &activation.auth_key.to_base64(),
            vault_id: activation.vault_id,
            header: BASE64.encode(activation.header),
            key_scheme: KEY_SCHEME,
        };
        let response = self
            .send(
                Method::Post,
                "/v1/accounts/activate",
                None,
                Some(json(&body)?),
            )
            .await?;
        let dto: wire::ActivatedDto = self.expect_ok(response)?;
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
        let response = self
            .send(Method::Post, "/v1/auth/params", None, Some(json(&body)?))
            .await?;
        let dto: wire::ParamsDto = self.expect_ok(response)?;
        Ok(AuthParams {
            account_id: dto.account_id,
            kdf: dto.kdf.try_into()?,
        })
    }

    pub async fn login(
        &self,
        email: &str,
        auth_key: &AuthKey,
        device_id: Uuid,
        device_name: &str,
    ) -> Result<Session> {
        let body = wire::LoginBody {
            email,
            auth_key: &auth_key.to_base64(),
            device_id,
            device_name,
        };
        let response = self
            .send(Method::Post, "/v1/auth/login", None, Some(json(&body)?))
            .await?;
        let dto: wire::LoginDto = self.expect_ok(response)?;
        if dto.token.is_empty() || dto.token.len() > 128 {
            return Err(SyncError::Protocol("token"));
        }
        Ok(Session::new(
            dto.token,
            dto.expires_at,
            // The account id is not in the login answer; the caller already
            // knows it from `auth_params` or from the invite, and taking the
            // server's word for it here would add nothing.
            Uuid::nil(),
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
            _ => Err(self.error_for(response)),
        }
    }

    pub async fn header(&self, session: &Session) -> Result<wire::RemoteHeader> {
        let response = self
            .send(Method::Get, "/v1/vault/header", Some(session), None)
            .await?;
        let dto: wire::HeaderDto = self.expect_ok(response)?;
        let header: wire::RemoteHeader = dto.try_into()?;
        if header.key_scheme < KEY_SCHEME {
            // A downgrade attempt. The core would refuse it too, but there is
            // no reason to carry it that far.
            return Err(SyncError::Protocol("key scheme"));
        }
        Ok(header)
    }

    pub async fn put_header(&self, session: &Session, header: &[u8], revision: i64) -> Result<()> {
        if header.is_empty() || header.len() > MAX_HEADER_BYTES {
            return Err(SyncError::Refused("header is not valid"));
        }
        let body = wire::HeaderBody {
            header: BASE64.encode(header),
            header_revision: revision,
            key_scheme: KEY_SCHEME,
        };
        let response = self
            .send(
                Method::Put,
                "/v1/vault/header",
                Some(session),
                Some(json(&body)?),
            )
            .await?;
        match response.status {
            200 | 204 => Ok(()),
            _ => Err(self.error_for(response)),
        }
    }

    /// One page of changes after `since`. The caller keeps calling while
    /// `has_more` is true, passing the cursor it was given.
    pub async fn pull(&self, session: &Session, since: i64) -> Result<Pulled> {
        if since < 0 {
            return Err(SyncError::Refused("cursor is not valid"));
        }
        let response = self
            .send(
                Method::Get,
                &format!("/v1/sync?since={since}"),
                Some(session),
                None,
            )
            .await?;
        let dto: wire::PullDto = self.expect_ok(response)?;
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
            .send(
                Method::Post,
                "/v1/items",
                Some(session),
                Some(json(&wire::WriteBody { changes })?),
            )
            .await?;
        if response.status == 409 {
            let dto: wire::ConflictsDto = wire::parse(&response.body)?;
            let conflicts: Vec<Conflict> = dto.conflicts.into_iter().map(Conflict::from).collect();
            if conflicts.is_empty() {
                return Err(SyncError::Protocol("a conflict naming no items"));
            }
            return Err(SyncError::Conflict(conflicts));
        }
        let dto: wire::WriteAckDto = self.expect_ok(response)?;
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

    pub async fn devices(&self, session: &Session) -> Result<Vec<Device>> {
        let response = self
            .send(Method::Get, "/v1/devices", Some(session), None)
            .await?;
        let dto: Vec<wire::DeviceDto> = self.expect_ok(response)?;
        Ok(dto
            .into_iter()
            .map(|d| Device {
                id: d.id,
                name: d.name,
                created_at: d.created_at,
                last_seen_at: d.last_seen_at,
                current: d.current,
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
            _ => Err(self.error_for(response)),
        }
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

    fn expect_ok<D: serde::de::DeserializeOwned>(&self, response: HttpResponse) -> Result<D> {
        if response.status != 200 {
            return Err(self.error_for(response));
        }
        wire::parse(&response.body)
    }

    /// Map a status to an error. The server's own message is never used: it
    /// is attacker-controlled text, and the client has its own words for
    /// every case it can act on.
    fn error_for(&self, response: HttpResponse) -> SyncError {
        match response.status {
            401 | 403 => SyncError::Unauthorized,
            404 => SyncError::Refused("not found"),
            409 => SyncError::Conflict(Vec::new()),
            413 => SyncError::Refused("too large"),
            429 => SyncError::RateLimited,
            400..=499 => SyncError::Refused("the request was refused"),
            _ => SyncError::Unavailable,
        }
    }
}

fn json<B: serde::Serialize>(body: &B) -> Result<Vec<u8>> {
    serde_json::to_vec(body).map_err(|_| SyncError::Protocol("request encoding"))
}
