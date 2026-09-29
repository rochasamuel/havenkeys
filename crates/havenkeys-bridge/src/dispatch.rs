//! Request → core call. The authorization decisions live in the core
//! (`VaultService::{find_matches, fill_for_page, totp_for_page, check_login,
//! save_login, find_passkeys, passkey_assert, check_passkey_create,
//! stage_passkey_create, has_passkey_for_page, start_sso_for_page, check_sso,
//! stage_save_sso}`); this layer adds the integration switch and maps types,
//! and never widens what the core returns.

use havenkeys_core::generator::{generate, GeneratorOptions};
use havenkeys_core::identity::FillRole;
use havenkeys_core::origin::MatchStrength as CoreStrength;
use havenkeys_core::passkey::{encode_b64url, B64Url, CreateQuery, PasskeyCreate, Upgrade};
use havenkeys_core::sso::SsoProvider as CoreProvider;
use havenkeys_core::vault::{
    SaveAction as CoreSaveAction, SaveTarget, StagedSave, StagedWrite, VaultService, VaultState,
};
use havenkeys_core::{Error, SecretString};
use havenkeys_protocol::{
    ErrorCode, IdentityRole, IdentityValue, LockState, Match, MatchStrength, PasskeyCandidate,
    PasskeyMatch, Request, ResultBody, SaveAction, SsoProvider as WireProvider, UpgradeHint,
    WireSecret, MAX_MATCHES,
};
use uuid::Uuid;

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

fn wire_provider(p: CoreProvider) -> WireProvider {
    match p {
        CoreProvider::Google => WireProvider::Google,
        CoreProvider::Microsoft => WireProvider::Microsoft,
        CoreProvider::Github => WireProvider::Github,
        CoreProvider::Apple => WireProvider::Apple,
    }
}

fn core_role(r: IdentityRole) -> FillRole {
    match r {
        IdentityRole::FullName => FillRole::FullName,
        IdentityRole::FirstName => FillRole::FirstName,
        IdentityRole::MiddleName => FillRole::MiddleName,
        IdentityRole::LastName => FillRole::LastName,
        IdentityRole::Email => FillRole::Email,
        IdentityRole::Phone => FillRole::Phone,
        IdentityRole::BirthDate => FillRole::BirthDate,
        IdentityRole::BirthDay => FillRole::BirthDay,
        IdentityRole::BirthMonth => FillRole::BirthMonth,
        IdentityRole::BirthYear => FillRole::BirthYear,
        IdentityRole::Company => FillRole::Company,
        IdentityRole::Street => FillRole::Street,
        IdentityRole::Number => FillRole::Number,
        IdentityRole::Complement => FillRole::Complement,
        IdentityRole::AddressLine1 => FillRole::AddressLine1,
        IdentityRole::AddressLine2 => FillRole::AddressLine2,
        IdentityRole::Neighborhood => FillRole::Neighborhood,
        IdentityRole::City => FillRole::City,
        IdentityRole::State => FillRole::State,
        IdentityRole::PostalCode => FillRole::PostalCode,
        IdentityRole::Country => FillRole::Country,
        IdentityRole::Username => FillRole::Username,
        IdentityRole::Cpf => FillRole::Cpf,
        IdentityRole::Rg => FillRole::Rg,
        IdentityRole::Passport => FillRole::Passport,
        IdentityRole::DriversLicense => FillRole::DriversLicense,
    }
}

fn wire_role(r: FillRole) -> IdentityRole {
    match r {
        FillRole::FullName => IdentityRole::FullName,
        FillRole::FirstName => IdentityRole::FirstName,
        FillRole::MiddleName => IdentityRole::MiddleName,
        FillRole::LastName => IdentityRole::LastName,
        FillRole::Email => IdentityRole::Email,
        FillRole::Phone => IdentityRole::Phone,
        FillRole::BirthDate => IdentityRole::BirthDate,
        FillRole::BirthDay => IdentityRole::BirthDay,
        FillRole::BirthMonth => IdentityRole::BirthMonth,
        FillRole::BirthYear => IdentityRole::BirthYear,
        FillRole::Company => IdentityRole::Company,
        FillRole::Street => IdentityRole::Street,
        FillRole::Number => IdentityRole::Number,
        FillRole::Complement => IdentityRole::Complement,
        FillRole::AddressLine1 => IdentityRole::AddressLine1,
        FillRole::AddressLine2 => IdentityRole::AddressLine2,
        FillRole::Neighborhood => IdentityRole::Neighborhood,
        FillRole::City => IdentityRole::City,
        FillRole::State => IdentityRole::State,
        FillRole::PostalCode => IdentityRole::PostalCode,
        FillRole::Country => IdentityRole::Country,
        FillRole::Username => IdentityRole::Username,
        FillRole::Cpf => IdentityRole::Cpf,
        FillRole::Rg => IdentityRole::Rg,
        FillRole::Passport => IdentityRole::Passport,
        FillRole::DriversLicense => IdentityRole::DriversLicense,
    }
}

fn core_provider(p: WireProvider) -> CoreProvider {
    match p {
        WireProvider::Google => CoreProvider::Google,
        WireProvider::Microsoft => CoreProvider::Microsoft,
        WireProvider::Github => CoreProvider::Github,
        WireProvider::Apple => CoreProvider::Apple,
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
    /// A "Sign in with" login created or updated by `save_sso`. Same reason
    /// as `Save`: the write reaches the server before the item ID is
    /// returned.
    SaveSso(StagedSave),
    /// A passkey sealed into its login. `result` is returned only after the
    /// server accepted `write`.
    CreatePasskey {
        write: StagedWrite,
        result: ResultBody,
    },
    /// The item may be opened in the desktop editor; the caller runs the
    /// hook once the vault lock is released.
    OpenItem(Uuid),
    /// The identity may be opened in the desktop; the caller runs the
    /// open-item hook once the vault lock is released.
    OpenIdentity(Uuid),
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
                    // A sign-in-with login shows its account where a username would go.
                    username: s.username.or(s.account),
                    has_totp: s.has_totp,
                    strength: strength(s.strength),
                    provider: s.provider.map(wire_provider),
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
            current_password,
        } => {
            require_enabled(v)?;
            let secret = SecretString::new(password.expose().to_owned());
            let current = current_password
                .as_ref()
                .map(|c| SecretString::new(c.expose().to_owned()));
            let (action, item_id) = match v
                .check_login(
                    url,
                    top_url.as_deref(),
                    username.as_deref(),
                    &secret,
                    current.as_ref(),
                )
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
            title,
        } => {
            require_enabled(v)?;
            let secret = SecretString::new(password.expose().to_owned());
            let staged = v
                .stage_save_login(
                    url,
                    top_url.as_deref(),
                    username.as_deref(),
                    secret,
                    match item_id {
                        Some(id) => SaveTarget::Update(id),
                        None => SaveTarget::New {
                            title: title.as_deref(),
                        },
                    },
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
        Request::OpenItem {
            item_id,
            url,
            top_url,
        } => {
            require_enabled(v)?;
            // The same origin binding as fill_item, without decrypting a
            // secret: the item must be one of this page's matches. An unknown
            // ID answers exactly like another site's item.
            let saved_here = v
                .find_matches(url, top_url.as_deref())
                .map_err(code)?
                .iter()
                .any(|s| s.id == *item_id);
            if saved_here {
                Ok(Dispatched::OpenItem(*item_id))
            } else {
                Err(ErrorCode::Denied)
            }
        }
        Request::FindIdentity { url, top_url } => {
            require_enabled(v)?;
            let s = v
                .identity_summary_for_page(url, top_url.as_deref())
                .map_err(code)?;
            Ok(Dispatched::Done(ResultBody::FindIdentity {
                title: s.title,
                email: s.email,
                roles: s.roles.into_iter().map(wire_role).collect(),
            }))
        }
        Request::FillIdentity {
            url,
            top_url,
            roles,
            documents,
        } => {
            require_enabled(v)?;
            let roles: Vec<FillRole> = roles.iter().copied().map(core_role).collect();
            let values = v
                .identity_values_for_page(url, top_url.as_deref(), &roles, *documents)
                .map_err(code)?
                .into_iter()
                .map(|(role, value)| IdentityValue {
                    role: wire_role(role),
                    value: WireSecret::new(value.expose().to_owned()),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FillIdentity { values }))
        }
        Request::OpenIdentity { url, top_url } => {
            require_enabled(v)?;
            let id = v
                .identity_id_for_page(url, top_url.as_deref())
                .map_err(code)?;
            Ok(Dispatched::OpenIdentity(id))
        }
        Request::StartSso {
            item_id,
            url,
            top_url,
        } => {
            require_enabled(v)?;
            let s = v
                .start_sso_for_page(item_id, url, top_url.as_deref())
                .map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::StartSso {
                provider: wire_provider(s.provider),
                account: s.account.clone(),
                provider_origins: s
                    .provider
                    .origins()
                    .iter()
                    .map(|o| (*o).to_owned())
                    .collect(),
                auto_choose: s.auto_choose,
            }))
        }
        Request::CheckSso {
            url,
            top_url,
            provider,
            account,
        } => {
            require_enabled(v)?;
            let (action, item_id) = match v
                .check_sso(
                    url,
                    top_url.as_deref(),
                    core_provider(*provider),
                    account.as_deref(),
                )
                .map_err(code)?
            {
                CoreSaveAction::Add => (SaveAction::Add, None),
                CoreSaveAction::Update(id) => (SaveAction::Update, Some(id)),
                CoreSaveAction::Unchanged => (SaveAction::Unchanged, None),
            };
            // Only a prompt that will be shown is offered the vault's accounts.
            let accounts = if action == SaveAction::Unchanged {
                Vec::new()
            } else {
                v.provider_accounts(core_provider(*provider))
                    .map_err(code)?
            };
            Ok(Dispatched::Done(ResultBody::CheckSso {
                action,
                item_id,
                accounts,
            }))
        }
        Request::SaveSso {
            url,
            top_url,
            provider,
            account,
            item_id,
            title,
        } => {
            require_enabled(v)?;
            let staged = v
                .stage_save_sso(
                    url,
                    top_url.as_deref(),
                    core_provider(*provider),
                    account.as_deref(),
                    match item_id {
                        Some(id) => SaveTarget::Update(id),
                        None => SaveTarget::New {
                            title: title.as_deref(),
                        },
                    },
                    now_ms(unix_seconds),
                )
                .map_err(item_code)?;
            Ok(Dispatched::SaveSso(staged))
        }
    }
}
