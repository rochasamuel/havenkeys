//! Request → core call. The authorization decisions live in the core
//! (`VaultService::{find_matches, fill_for_page, totp_for_page, check_login,
//! save_login}`); this layer
//! adds the integration switch and maps types, and never widens what the core
//! returns.

use havenkeys_core::generator::{generate, GeneratorOptions};
use havenkeys_core::origin::MatchStrength as CoreStrength;
use havenkeys_core::vault::{SaveAction as CoreSaveAction, StagedSave, VaultService, VaultState};
use havenkeys_core::{Error, SecretString};
use havenkeys_protocol::{
    ErrorCode, LockState, Match, MatchStrength, Request, ResultBody, SaveAction, WireSecret,
    MAX_MATCHES,
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

/// What answering a request produced.
///
/// Saving a login cannot finish under the vault lock: the server has to
/// accept the write first, and holding the lock across a network request
/// would delay locking the vault. So the staged write comes back out and the
/// caller sends it.
pub enum Dispatched {
    Done(ResultBody),
    Save(StagedSave),
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
                .fill_for_page(item_id, url, top_url.as_deref())
                .map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::FillItem {
                username: creds.username.clone(),
                password: creds
                    .password
                    .as_ref()
                    .map(|p| WireSecret::new(p.expose().to_owned())),
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
            Ok(Dispatched::Done(ResultBody::GetTotp {
                code: WireSecret::new(totp.code.expose().to_owned()),
                period: totp.period,
                seconds_remaining: totp.seconds_remaining,
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
    }
}
