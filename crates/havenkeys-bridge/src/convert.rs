//! One-to-one mappings between the core's types and the wire protocol's.
//!
//! The bridge owns neither side, so these are functions rather than `From`
//! impls. Every `match` is exhaustive: a variant added on either side fails
//! to compile here until it is mapped.

use havenkeys_core::card::CardBrand;
use havenkeys_core::card_page::CardRole as CoreCardRole;
use havenkeys_core::identity::FillRole;
use havenkeys_core::origin::MatchStrength as CoreStrength;
use havenkeys_core::sso::SsoProvider as CoreProvider;
use havenkeys_core::vault::{SaveAction as CoreSaveAction, VaultState};
use havenkeys_protocol::{
    CardBrandId, CardRole, IdentityRole, LockState, MatchStrength, SaveAction,
    SsoProvider as WireProvider,
};
use uuid::Uuid;

pub(crate) fn lock_state(s: VaultState) -> LockState {
    match s {
        VaultState::Locked => LockState::Locked,
        VaultState::Unlocking => LockState::Unlocking,
        VaultState::Unlocked => LockState::Unlocked,
        VaultState::Locking => LockState::Locking,
    }
}

pub(crate) fn strength(s: CoreStrength) -> MatchStrength {
    match s {
        CoreStrength::ExactUrl => MatchStrength::ExactUrl,
        CoreStrength::SameHost => MatchStrength::SameHost,
        CoreStrength::SameSite => MatchStrength::SameSite,
    }
}

pub(crate) fn wire_provider(p: CoreProvider) -> WireProvider {
    match p {
        CoreProvider::Google => WireProvider::Google,
        CoreProvider::Microsoft => WireProvider::Microsoft,
        CoreProvider::Github => WireProvider::Github,
        CoreProvider::Apple => WireProvider::Apple,
    }
}

pub(crate) fn core_role(r: IdentityRole) -> FillRole {
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

pub(crate) fn wire_role(r: FillRole) -> IdentityRole {
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

pub(crate) fn core_provider(p: WireProvider) -> CoreProvider {
    match p {
        WireProvider::Google => CoreProvider::Google,
        WireProvider::Microsoft => CoreProvider::Microsoft,
        WireProvider::Github => CoreProvider::Github,
        WireProvider::Apple => CoreProvider::Apple,
    }
}

pub(crate) fn core_card_role(r: CardRole) -> CoreCardRole {
    match r {
        CardRole::CardholderName => CoreCardRole::CardholderName,
        CardRole::CardholderGivenName => CoreCardRole::CardholderGivenName,
        CardRole::CardholderFamilyName => CoreCardRole::CardholderFamilyName,
        CardRole::Number => CoreCardRole::Number,
        CardRole::VerificationNumber => CoreCardRole::VerificationNumber,
        CardRole::ExpiryMonth => CoreCardRole::ExpiryMonth,
        CardRole::ExpiryYear => CoreCardRole::ExpiryYear,
        CardRole::Brand => CoreCardRole::Brand,
    }
}

pub(crate) fn wire_card_role(r: CoreCardRole) -> CardRole {
    match r {
        CoreCardRole::CardholderName => CardRole::CardholderName,
        CoreCardRole::CardholderGivenName => CardRole::CardholderGivenName,
        CoreCardRole::CardholderFamilyName => CardRole::CardholderFamilyName,
        CoreCardRole::Number => CardRole::Number,
        CoreCardRole::VerificationNumber => CardRole::VerificationNumber,
        CoreCardRole::ExpiryMonth => CardRole::ExpiryMonth,
        CoreCardRole::ExpiryYear => CardRole::ExpiryYear,
        CoreCardRole::Brand => CardRole::Brand,
    }
}

pub(crate) fn wire_brand(b: CardBrand) -> CardBrandId {
    match b {
        CardBrand::Visa => CardBrandId::Visa,
        CardBrand::Mastercard => CardBrandId::Mastercard,
        CardBrand::Amex => CardBrandId::Amex,
        CardBrand::Elo => CardBrandId::Elo,
        CardBrand::Hipercard => CardBrandId::Hipercard,
        CardBrand::Diners => CardBrandId::Diners,
        CardBrand::Discover => CardBrandId::Discover,
        CardBrand::Jcb => CardBrandId::Jcb,
        CardBrand::Unionpay => CardBrandId::Unionpay,
        CardBrand::Maestro => CardBrandId::Maestro,
        CardBrand::Other => CardBrandId::Other,
    }
}

/// What a save prompt should offer, and the login it would update.
pub(crate) fn save_action(a: CoreSaveAction) -> (SaveAction, Option<Uuid>) {
    match a {
        CoreSaveAction::Add => (SaveAction::Add, None),
        CoreSaveAction::Update(id) => (SaveAction::Update, Some(id)),
        CoreSaveAction::Unchanged => (SaveAction::Unchanged, None),
    }
}
