//! Request → core call. The authorization decisions live in the core
//! (`VaultService::{find_matches, fill_for_page, totp_for_page}`); this layer
//! adds the integration switch and maps types, and never widens what the core
//! returns.

use havenkeys_core::origin::MatchStrength as CoreStrength;
use havenkeys_core::vault::{VaultService, VaultState};
use havenkeys_core::Error;
use havenkeys_protocol::{
    ErrorCode, LockState, Match, MatchStrength, Request, ResultBody, WireSecret, MAX_MATCHES,
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

/// Answer a request that only reads the vault. `lock` is handled by the
/// caller, which must not hold the vault while locking.
pub fn dispatch(v: &VaultService, req: &Request, unix_seconds: u64) -> Result<ResultBody, ErrorCode> {
    match req {
        Request::Status {} => {
            let s = v.status().map_err(code)?;
            Ok(ResultBody::Status {
                state: lock_state(s.state),
                vault_exists: s.vault_exists,
            })
        }
        Request::Lock {} => Err(ErrorCode::Internal),
        Request::FindMatches { url } => {
            require_enabled(v)?;
            let matches = v
                .find_matches(url)
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
            Ok(ResultBody::FindMatches { matches })
        }
        Request::FillItem { item_id, url } => {
            require_enabled(v)?;
            let creds = v.fill_for_page(item_id, url).map_err(item_code)?;
            Ok(ResultBody::FillItem {
                username: creds.username.clone(),
                password: creds
                    .password
                    .as_ref()
                    .map(|p| WireSecret::new(p.expose().to_owned())),
            })
        }
        Request::GetTotp { item_id, url } => {
            require_enabled(v)?;
            let totp = v
                .totp_for_page(item_id, url, unix_seconds)
                .map_err(item_code)?;
            Ok(ResultBody::GetTotp {
                code: WireSecret::new(totp.code.expose().to_owned()),
                period: totp.period,
                seconds_remaining: totp.seconds_remaining,
            })
        }
    }
}
