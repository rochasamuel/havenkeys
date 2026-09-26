//! Request → core call. The authorization decisions live in the core
//! (`VaultService::{find_matches, fill_for_page, totp_for_page, check_login,
//! save_login, find_passkeys, passkey_assert, check_passkey_create,
//! stage_passkey_create, has_passkey_for_page}`); this layer
//! adds the integration switch and maps types, and never widens what the core
//! returns.

use havenkeys_core::generator::{generate, GeneratorOptions};
use havenkeys_core::origin::MatchStrength as CoreStrength;
use havenkeys_core::passkey::{encode_b64url, B64Url, CreateQuery, PasskeyCreate, Upgrade};
use havenkeys_core::vault::{
    SaveAction as CoreSaveAction, StagedSave, StagedWrite, VaultService, VaultState,
};
use havenkeys_core::{Error, SecretString};
use havenkeys_protocol::{
    ErrorCode, LockState, Match, MatchStrength, PasskeyCandidate, PasskeyMatch, Request,
    ResultBody, SaveAction, UpgradeHint, WireSecret, MAX_MATCHES,
};

fn code(e: Error) -> ErrorCode {
    match e {
        Error::Locked => ErrorCode::Locked,
        Error::Busy => ErrorCode::Busy,
        Error::NoVault => ErrorCode::NoVault,
        Error::NotFound => ErrorCode::NotFound,
        Error::Denied => ErrorCode::Denied,
        Error::InvalidInput(_) => ErrorCode::InvalidInput,
        Error::Decryption => ErrorCode::Decryption,
        Error::Corrupted => ErrorCode::Corrupted,
        Error::Offline => ErrorCode::Offline,
        _ => ErrorCode::Internal,
    }
}

/// For item-specific requests an unknown ID is reported exactly like an item
/// saved for a different site, so the extension cannot probe which IDs exist.
fn item_code(e: Error) -> ErrorCode {
    match e {
        Error::NotFound => ErrorCode::Denied,
        other => code(other),
    }
}

/// Refuse page requests while locked or when the user turned integration off.
fn require_enabled(v: &VaultService) -> Result<(), ErrorCode> {
    let settings = v.settings().map_err(code)?;
    if settings.browser_integration {
        Ok(())
    } else {
        Err(ErrorCode::IntegrationDisabled)
    }
}

fn lock_state(s: VaultState) -> LockState {
    match s {
        VaultState::Locked => LockState::Locked,
        VaultState::Unlocking => LockState::Unlocking,
        VaultState::Unlocked => LockState::Unlocked,
        VaultState::Locking => LockState::Locking,
    }
}

fn strength(s: CoreStrength) -> MatchStrength {
    match s {
        CoreStrength::ExactUrl => MatchStrength::ExactUrl,
        CoreStrength::SameHost => MatchStrength::SameHost,
        CoreStrength::SameSite => MatchStrength::SameSite,
    }
}

fn now_ms(unix_seconds: u64) -> i64 {
    i64::try_from(unix_seconds.saturating_mul(1000)).unwrap_or(i64::MAX)
}

fn bytes(s: &str) -> Result<Vec<u8>, ErrorCode> {
    B64Url::decode(s)
        .map(|b| b.0)
        .map_err(|_| ErrorCode::InvalidInput)
}

fn byte_list(v: &[String]) -> Result<Vec<Vec<u8>>, ErrorCode> {
    v.iter().map(|s| bytes(s)).collect()
}

/// What answering a request produced.
///
/// Saving a login cannot finish under the vault lock: the server has to
/// accept the write first, and holding the lock across a network request
/// would delay locking the vault. So the staged write comes back out and the
/// caller sends it.
pub enum Dispatched {
    Done(ResultBody),
    Save(StagedSave),
    /// A passkey sealed into its login. `result` is returned only after the
    /// server accepted `write`.
    CreatePasskey {
        write: StagedWrite,
        result: ResultBody,
    },
}

/// Answer a request. `lock` is handled by the caller, which must not hold
/// the vault while locking.
pub fn dispatch(
    v: &mut VaultService,
    req: &Request,
    unix_seconds: u64,
) -> Result<Dispatched, ErrorCode> {
    match req {
        Request::Status {} => {
            let s = v.status().map_err(code)?;
            Ok(Dispatched::Done(ResultBody::Status {
                state: lock_state(s.state),
                vault_exists: s.vault_exists,
            }))
        }
        Request::Lock {} => Err(ErrorCode::Internal),
        Request::FindMatches { url, top_url } => {
            require_enabled(v)?;
            let matches = v
                .find_matches(url, top_url.as_deref())
                .map_err(code)?
                .into_iter()
                .take(MAX_MATCHES)
                .map(|s| Match {
                    id: s.id,
                    title: s.title,
                    username: s.username,
                    has_totp: s.has_totp,
                    strength: strength(s.strength),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FindMatches { matches }))
        }
        Request::FillItem {
            item_id,
            url,
            top_url,
        } => {
            require_enabled(v)?;
            let creds = v
                .fill_for_page(item_id, url, top_url.as_deref(), now_ms(unix_seconds))
                .map_err(item_code)?;
            let auto_submit = v.auto_sign_in_for(item_id).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::FillItem {
                username: creds.username.clone(),
                password: creds
                    .password
                    .as_ref()
                    .map(|p| WireSecret::new(p.expose().to_owned())),
                auto_submit,
            }))
        }
        Request::GetTotp {
            item_id,
            url,
            top_url,
        } => {
            require_enabled(v)?;
            let totp = v
                .totp_for_page(item_id, url, top_url.as_deref(), unix_seconds)
                .map_err(item_code)?;
            let auto_submit = v.auto_sign_in_for(item_id).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::GetTotp {
                code: WireSecret::new(totp.code.expose().to_owned()),
                period: totp.period,
                seconds_remaining: totp.seconds_remaining,
                auto_submit,
            }))
        }
        Request::GeneratePassword {} => {
            require_enabled(v)?;
            let generated = generate(&GeneratorOptions::default()).map_err(code)?;
            Ok(Dispatched::Done(ResultBody::GeneratePassword {
                password: WireSecret::new(generated.password.expose().to_owned()),
            }))
        }
        Request::CheckLogin {
            url,
            top_url,
            username,
            password,
        } => {
            require_enabled(v)?;
            let secret = SecretString::new(password.expose().to_owned());
            let (action, item_id) = match v
                .check_login(url, top_url.as_deref(), username.as_deref(), &secret)
                .map_err(code)?
            {
                CoreSaveAction::Add => (SaveAction::Add, None),
                CoreSaveAction::Update(id) => (SaveAction::Update, Some(id)),
                CoreSaveAction::Unchanged => (SaveAction::Unchanged, None),
            };
            Ok(Dispatched::Done(ResultBody::CheckLogin { action, item_id }))
        }
        Request::SaveLogin {
            url,
            top_url,
            username,
            password,
            item_id,
        } => {
            require_enabled(v)?;
            let secret = SecretString::new(password.expose().to_owned());
            let staged = v
                .stage_save_login(
                    url,
                    top_url.as_deref(),
                    username.as_deref(),
                    secret,
                    item_id.as_ref(),
                    now_ms(unix_seconds),
                )
                .map_err(item_code)?;
            Ok(Dispatched::Save(staged))
        }
        Request::FindPasskeys {
            url,
            top_url,
            rp_id,
            allow_credentials,
        } => {
            require_enabled(v)?;
            let allow = byte_list(allow_credentials)?;
            let passkeys = v
                .find_passkeys(rp_id, url, top_url.as_deref(), &allow)
                .map_err(code)?
                .into_iter()
                .take(MAX_MATCHES)
                .map(|m| PasskeyMatch {
                    item_id: m.item_id,
                    credential_id: encode_b64url(&m.credential_id),
                    title: m.title,
                    user_name: m.user_name,
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FindPasskeys { passkeys }))
        }
        Request::PasskeyGet {
            item_id,
            credential_id,
            url,
            top_url,
            rp_id,
            challenge,
        } => {
            require_enabled(v)?;
            let a = v
                .passkey_assert(
                    item_id,
                    &bytes(credential_id)?,
                    rp_id,
                    url,
                    top_url.as_deref(),
                    &bytes(challenge)?,
                )
                .map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::PasskeyGet {
                credential_id: encode_b64url(&a.credential_id),
                authenticator_data: encode_b64url(&a.authenticator_data),
                client_data_json: encode_b64url(&a.client_data_json),
                signature: encode_b64url(&a.signature),
                user_handle: encode_b64url(&a.user_handle),
            }))
        }
        Request::CheckPasskeyCreate {
            url,
            top_url,
            rp_id,
            user_name,
            exclude_credentials,
            conditional,
        } => {
            require_enabled(v)?;
            let exclude = byte_list(exclude_credentials)?;
            let check = v
                .check_passkey_create(
                    &CreateQuery {
                        rp_id,
                        page_url: url,
                        top_url: top_url.as_deref(),
                        user_name,
                        exclude: &exclude,
                        conditional: *conditional,
                    },
                    now_ms(unix_seconds),
                )
                .map_err(code)?;
            let candidates = if check.excluded {
                Vec::new()
            } else {
                check
                    .candidates
                    .into_iter()
                    .take(MAX_MATCHES)
                    .map(|s| PasskeyCandidate {
                        item_id: s.id,
                        title: s.title,
                        username: s.username,
                    })
                    .collect()
            };
            let upgrade = match check.upgrade {
                Upgrade::None => UpgradeHint::None {},
                Upgrade::Ask(item_id) => UpgradeHint::Ask { item_id },
                Upgrade::Auto(item_id) => UpgradeHint::Auto { item_id },
            };
            Ok(Dispatched::Done(ResultBody::CheckPasskeyCreate {
                excluded: check.excluded,
                candidates,
                upgrade,
            }))
        }
        Request::PasskeyCreate {
            url,
            top_url,
            rp_id,
            challenge,
            user_handle,
            user_name,
            display_name,
            item_id,
            conditional,
        } => {
            require_enabled(v)?;
            let challenge = bytes(challenge)?;
            let user_handle = bytes(user_handle)?;
            let staged = v
                .stage_passkey_create(
                    PasskeyCreate {
                        rp_id,
                        page_url: url,
                        top_url: top_url.as_deref(),
                        challenge: &challenge,
                        user_handle: &user_handle,
                        user_name,
                        display_name: display_name.as_deref(),
                        item_id: *item_id,
                        conditional: *conditional,
                    },
                    now_ms(unix_seconds),
                )
                .map_err(item_code)?;
            let r = &staged.registration;
            let result = ResultBody::PasskeyCreate {
                credential_id: encode_b64url(&r.credential_id),
                attestation_object: encode_b64url(&r.attestation_object),
                client_data_json: encode_b64url(&r.client_data_json),
                authenticator_data: encode_b64url(&r.authenticator_data),
                public_key: encode_b64url(&r.public_key),
                public_key_algorithm: havenkeys_protocol::COSE_ES256,
            };
            Ok(Dispatched::CreatePasskey {
                write: staged.write,
                result,
            })
        }
        Request::PasskeyStatus { url, top_url } => {
            require_enabled(v)?;
            let has_passkey = v
                .has_passkey_for_page(url, top_url.as_deref())
                .map_err(code)?;
            Ok(Dispatched::Done(ResultBody::PasskeyStatus { has_passkey }))
        }
    }
}
