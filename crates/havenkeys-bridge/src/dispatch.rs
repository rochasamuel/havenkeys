//! Request → core call. The authorization decisions live in the core
//! (`VaultService::{find_matches, fill_for_page, totp_for_page, check_login,
//! save_login, find_passkeys, passkey_assert, check_passkey_create,
//! stage_passkey_create, has_passkey_for_page, start_sso_for_page, check_sso,
//! stage_save_sso, cards_for_page, card_values_for_page,
//! stage_save_card}`); this layer adds the integration switch and maps types,
//! and never widens what the core returns.

use crate::convert::{
    core_card_role, core_provider, core_role, lock_state, save_action, strength, wire_brand,
    wire_card_role, wire_provider, wire_role,
};
use havenkeys_core::card::CardExpiry;
use havenkeys_core::card_page::{CardFrame as CoreCardFrame, CardRole as CoreCardRole, NewCard};
use havenkeys_core::generator::{generate, GeneratorOptions};
use havenkeys_core::identity::FillRole;
use havenkeys_core::passkey::{encode_b64url, B64Url, CreateQuery, PasskeyCreate, Upgrade};
use havenkeys_core::vault::{SaveTarget, StagedWrite, VaultService};
use havenkeys_core::{Error, SecretString};
use havenkeys_protocol::{
    CardFrameValues, CardMatch, CardValue, ErrorCode, IdentityValue, Match, PasskeyCandidate,
    PasskeyMatch, PasswordOptions, Request, ResultBody, SaveAction, UpgradeHint, WireSecret,
    MAX_MATCHES,
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

fn wire_secret(s: &SecretString) -> WireSecret {
    WireSecret::new(s.expose().to_owned())
}

fn core_secret(s: &WireSecret) -> SecretString {
    SecretString::new(s.expose().to_owned())
}

/// Update the login the user picked, or add one titled `title`.
fn save_target<'a>(item_id: &'a Option<Uuid>, title: &'a Option<String>) -> SaveTarget<'a> {
    match item_id {
        Some(id) => SaveTarget::Update(id),
        None => SaveTarget::New {
            title: title.as_deref(),
        },
    }
}

/// What answering a request produced.
// One value per request, matched at once: boxing `Write` would only add an
// allocation. (The enum is the same size it was with one variant per save.)
#[allow(clippy::large_enum_variant)]
pub enum Dispatched {
    /// The answer.
    Done(ResultBody),
    /// A write the account's server must accept before `result` is returned:
    /// a saved login, card or "Sign in with" login, or a new passkey. It
    /// cannot finish under the vault lock, because holding the lock across a
    /// network request would delay locking the vault, so the staged write
    /// comes back out and the caller sends it.
    Write {
        write: StagedWrite,
        result: ResultBody,
    },
    /// The item may be opened in the desktop; the caller runs the open-item
    /// hook once the vault lock is released, then returns `result`.
    Open { item: Uuid, result: ResultBody },
}

/// Answer a request. `lock` and `show_unlock` are handled by the caller, which must not hold
/// the vault while locking.
pub fn dispatch(
    v: &mut VaultService,
    req: &Request,
    unix_seconds: u64,
) -> Result<Dispatched, ErrorCode> {
    // Everything but status and lock comes from a web page.
    if !matches!(
        req,
        Request::Status {} | Request::Lock {} | Request::ShowUnlock {}
    ) {
        require_enabled(v)?;
    }
    match req {
        Request::Status {} => {
            let s = v.status().map_err(code)?;
            Ok(Dispatched::Done(ResultBody::Status {
                state: lock_state(s.state),
                vault_exists: s.vault_exists,
            }))
        }
        // Handled by the caller, without the vault.
        Request::Lock {} | Request::ShowUnlock {} => Err(ErrorCode::Internal),
        Request::FindMatches { url, top_url } => {
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
            let creds = v
                .fill_for_page(item_id, url, top_url.as_deref(), now_ms(unix_seconds))
                .map_err(item_code)?;
            let auto_submit = v.auto_sign_in_for(item_id).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::FillItem {
                username: creds.username.clone(),
                password: creds.password.as_ref().map(wire_secret),
                auto_submit,
            }))
        }
        Request::GetTotp {
            item_id,
            url,
            top_url,
        } => {
            let totp = v
                .totp_for_page(item_id, url, top_url.as_deref(), unix_seconds)
                .map_err(item_code)?;
            let auto_submit = v.auto_sign_in_for(item_id).map_err(item_code)?;
            Ok(Dispatched::Done(ResultBody::GetTotp {
                code: wire_secret(&totp.code),
                period: totp.period,
                seconds_remaining: totp.seconds_remaining,
                auto_submit,
            }))
        }
        Request::GeneratorOptions {} => {
            let saved = v.settings().map_err(code)?.generator;
            Ok(Dispatched::Done(ResultBody::GeneratorOptions {
                options: PasswordOptions {
                    length: u32::try_from(saved.length).map_err(|_| ErrorCode::Internal)?,
                    uppercase: saved.uppercase,
                    lowercase: saved.lowercase,
                    digits: saved.digits,
                    symbols: saved.symbols,
                    avoid_ambiguous: saved.avoid_ambiguous,
                },
            }))
        }
        Request::GeneratePassword { options } => {
            // Without options: the policy saved in the desktop's generator tab.
            let policy = match options {
                Some(o) => GeneratorOptions {
                    length: o.length as usize,
                    uppercase: o.uppercase,
                    lowercase: o.lowercase,
                    digits: o.digits,
                    symbols: o.symbols,
                    avoid_ambiguous: o.avoid_ambiguous,
                },
                None => v.settings().map_err(code)?.generator,
            };
            let generated = generate(&policy).map_err(code)?;
            Ok(Dispatched::Done(ResultBody::GeneratePassword {
                password: wire_secret(&generated.password),
            }))
        }
        Request::CheckLogin {
            url,
            top_url,
            username,
            password,
            current_password,
        } => {
            let secret = core_secret(password);
            let current = current_password.as_ref().map(core_secret);
            let (action, item_id) = save_action(
                v.check_login(
                    url,
                    top_url.as_deref(),
                    username.as_deref(),
                    &secret,
                    current.as_ref(),
                )
                .map_err(code)?,
            );
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
            let staged = v
                .stage_save_login(
                    url,
                    top_url.as_deref(),
                    username.as_deref(),
                    core_secret(password),
                    save_target(item_id, title),
                    now_ms(unix_seconds),
                )
                .map_err(item_code)?;
            Ok(Dispatched::Write {
                write: staged.write,
                result: ResultBody::SaveLogin {
                    item_id: staged.item_id,
                },
            })
        }
        Request::FindPasskeys {
            url,
            top_url,
            rp_id,
            allow_credentials,
        } => {
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
            Ok(Dispatched::Write {
                write: staged.write,
                result,
            })
        }
        Request::PasskeyStatus { url, top_url } => {
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
            // The same origin binding as fill_item, without decrypting a
            // secret: the item must be one of this page's matches. An unknown
            // ID answers exactly like another site's item.
            let saved_here = v
                .find_matches(url, top_url.as_deref())
                .map_err(code)?
                .iter()
                .any(|s| s.id == *item_id);
            if !saved_here {
                return Err(ErrorCode::Denied);
            }
            Ok(Dispatched::Open {
                item: *item_id,
                result: ResultBody::OpenItem {},
            })
        }
        Request::FindIdentity { url, top_url } => {
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
            let roles: Vec<FillRole> = roles.iter().copied().map(core_role).collect();
            let values = v
                .identity_values_for_page(url, top_url.as_deref(), &roles, *documents)
                .map_err(code)?
                .into_iter()
                .map(|(role, value)| IdentityValue {
                    role: wire_role(role),
                    value: wire_secret(&value),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FillIdentity { values }))
        }
        Request::OpenIdentity { url, top_url } => {
            let id = v
                .identity_id_for_page(url, top_url.as_deref())
                .map_err(code)?;
            Ok(Dispatched::Open {
                item: id,
                result: ResultBody::OpenIdentity {},
            })
        }
        Request::FindCards { url, top_url } => {
            let list = v.cards_for_page(url, top_url.as_deref()).map_err(code)?;
            let cards = list
                .cards
                .into_iter()
                .take(MAX_MATCHES)
                .map(|c| CardMatch {
                    id: c.id,
                    title: c.title,
                    brand: c.brand.map(wire_brand),
                    last4: c.last4,
                    expiry: c.expiry.map(CardExpiry::to_wire),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FindCards {
                insecure: list.insecure,
                cards,
            }))
        }
        Request::FillCard {
            item_id,
            top_url,
            frames,
        } => {
            let roles: Vec<Vec<CoreCardRole>> = frames
                .iter()
                .map(|f| f.roles.iter().copied().map(core_card_role).collect())
                .collect();
            let core_frames: Vec<CoreCardFrame<'_>> = frames
                .iter()
                .zip(&roles)
                .map(|(f, r)| CoreCardFrame {
                    url: &f.url,
                    roles: r,
                })
                .collect();
            let per_frame = v
                .card_values_for_page(item_id, top_url, &core_frames)
                .map_err(code)?;
            // One entry per requested frame, in order; the extension pairs
            // them by position.
            if per_frame.len() != frames.len() {
                return Err(ErrorCode::Internal);
            }
            let values = per_frame
                .into_iter()
                .map(|frame| CardFrameValues {
                    values: frame
                        .into_iter()
                        .map(|(role, value)| CardValue {
                            role: wire_card_role(role),
                            value: wire_secret(&value),
                        })
                        .collect(),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FillCard { frames: values }))
        }
        Request::SaveCard {
            url,
            top_url,
            title,
            cardholder_name,
            number,
            verification_number,
            expiry,
        } => {
            let staged = v
                .stage_save_card(
                    url,
                    top_url.as_deref(),
                    NewCard {
                        title: title.as_deref(),
                        cardholder_name: cardholder_name.as_deref(),
                        number: core_secret(number),
                        verification_number: verification_number.as_ref().map(core_secret),
                        expiry: expiry.as_deref(),
                    },
                    now_ms(unix_seconds),
                )
                .map_err(code)?;
            Ok(Dispatched::Write {
                write: staged.write,
                result: ResultBody::SaveCard {
                    item_id: staged.item_id,
                },
            })
        }
        Request::StartSso {
            item_id,
            url,
            top_url,
        } => {
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
            let (action, item_id) = save_action(
                v.check_sso(
                    url,
                    top_url.as_deref(),
                    core_provider(*provider),
                    account.as_deref(),
                )
                .map_err(code)?,
            );
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
            let staged = v
                .stage_save_sso(
                    url,
                    top_url.as_deref(),
                    core_provider(*provider),
                    account.as_deref(),
                    save_target(item_id, title),
                    now_ms(unix_seconds),
                )
                .map_err(item_code)?;
            Ok(Dispatched::Write {
                write: staged.write,
                result: ResultBody::SaveSso {
                    item_id: staged.item_id,
                },
            })
        }
    }
}
