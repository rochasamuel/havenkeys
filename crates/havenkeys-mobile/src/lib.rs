//! The API the HavenKeys Android app (and later iOS) calls, through UniFFI.
//!
//! Intent-level, like the desktop bridge: it never returns keys, blobs or
//! whole vault objects, and every call that returns a secret checks the lock
//! state, the item and the fill target here (spec 2026-10-01-android-app
//! §4.2). Calls block; the app runs them on `Dispatchers.IO`.

#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

uniffi::setup_scaffolding!();

mod account;
mod activity;
#[cfg(target_os = "android")]
mod android_tls;
mod asset_links_fetch;
mod autofill;
mod cards;
mod credentials;
mod edit;
mod error;
mod events;
mod health;
mod identity_fill;
mod items;
mod key_file;
mod onboarding;
mod pairing;
mod passkey_json;
mod qr;
mod save;
mod settings;
mod trash;
mod unlock;
mod vault;

pub use account::DeviceInfo;
pub use autofill::{AutofillMatch, BoundFill, FillValues, FrameFacts, TargetFacts, TargetKind};
pub use cards::{
    CardChoice, CardChoices, CardFrameRoles, CardRole, CardValue, SaveCard, MAX_CARD_FRAMES,
};
pub use credentials::{
    privileged_browsers_json, CredentialCaller, PasskeyCreatePlan, PasskeyOffer,
};
pub use edit::{Change, EditField, FieldChange, ItemDraft, ItemEdit, MatchKind, Website};
pub use error::{MobileError, MobileResult};
pub use events::VaultEvents;
pub use health::{HealthCountsView, HealthIssueView, HealthKind, HealthView};
pub use identity_fill::{IdentityChoice, IdentityRole, IdentityValue, MAX_IDENTITY_ROLES};
pub use items::{
    FieldKind, Generated, GeneratorOptions, ItemKind, ItemSummary, ItemView, TotpNow, ViewField,
};
pub use key_file::{CipherError, KeystoreCipher};
pub use onboarding::{KitPreview, LumaFrame};
pub use pairing::PairingRequestView;
pub use save::{SaveLogin, SaveResult};
pub use settings::MobileSettings;
pub use vault::{LockState, MobileConfig, MobileVault, Status};

/// Test support for this crate's integration tests. Not part of the API
/// (not exported through UniFFI).
#[cfg(any(test, feature = "testing"))]
#[doc(hidden)]
pub mod testing {
    use crate::MobileVault;
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use havenkeys_core::SecretString;

    pub fn seed_with_github_login(v: &MobileVault) {
        havenkeys_client::testing::seed_account_vault(&v.client, "correct horse battery staple");
        seed_with_github_login_unlocked(v);
    }

    /// A card in an unlocked vault: (item ID). Expiry 04/2033, CVV 123.
    pub fn seed_card(v: &MobileVault, title: &str, number: &str) -> String {
        use havenkeys_core::card::{CardExpiry, CardInput};
        let input = ItemInput {
            card: Some(CardInput {
                cardholder_name: Some(SecretString::from("Samuel Rocha")),
                brand: None,
                number: SecretUpdate::Set(SecretString::from(number)),
                verification_number: SecretUpdate::Set(SecretString::from("123")),
                expiry: Some(CardExpiry {
                    year: 2033,
                    month: 4,
                }),
                notes: None,
            }),
            ..blank(ItemType::Card, title)
        };
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, 1).unwrap();
        let id = staged.item_id;
        vault.commit_write(staged, 1).unwrap();
        id.to_string()
    }

    /// The account's identity: Samuel Rocha, user@example.com, a mobile
    /// phone, a postal code and a CPF. The vault must be unlocked.
    pub fn seed_identity(v: &MobileVault) {
        use havenkeys_core::identity::IdentityFields;
        let mut vault = v.client.vault().unwrap();
        let id = match vault
            .stage_identity_if_missing("user@example.com", 1)
            .unwrap()
        {
            Some(staged) => {
                let id = staged.item_id;
                vault.commit_write(staged, 1).unwrap();
                id
            }
            None => vault.identity_item_id().unwrap(),
        };
        let some = |s: &str| Some(SecretString::from(s));
        let input = ItemInput {
            identity: Some(IdentityFields {
                first_name: some("Samuel"),
                last_name: some("Rocha"),
                email: some("user@example.com"),
                mobile_phone: some("+55 61 99999-0000"),
                postal_code: some("71266-105"),
                cpf: some("123.456.789-00"),
                ..Default::default()
            }),
            ..blank(ItemType::Identity, "")
        };
        let staged = vault.stage_update(&id, input, 2).unwrap();
        vault.commit_write(staged, 2).unwrap();
    }

    /// The GitHub login of `seed_with_github_login`, in a vault that is
    /// already unlocked.
    pub fn seed_with_github_login_unlocked(v: &MobileVault) {
        let input = ItemInput {
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: "https://github.com".into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
            ..blank(ItemType::Login, "GitHub")
        };
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, 1).unwrap();
        vault.commit_write(staged, 1).unwrap();
    }

    /// An item input with nothing set but its type and title.
    fn blank(item_type: ItemType, title: &str) -> ItemInput {
        ItemInput {
            tags: None,
            item_type,
            title: title.into(),
            username: None,
            urls: vec![],
            password: SecretUpdate::Keep,
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: None,
            sections: None,
        }
    }

    /// A passkey for github.com, as the extension would have saved it:
    /// (item ID, credential ID, SubjectPublicKeyInfo).
    pub fn seed_github_passkey(v: &MobileVault) -> (String, Vec<u8>, Vec<u8>) {
        use havenkeys_core::passkey::PasskeyCreate;
        let mut vault = v.client.vault().unwrap();
        let staged = vault
            .stage_passkey_create(
                PasskeyCreate {
                    rp_id: "github.com",
                    page_url: "https://github.com/",
                    top_url: None,
                    challenge: &[7; 32],
                    user_handle: &[1, 2, 3],
                    user_name: "octo",
                    display_name: None,
                    item_id: None,
                    conditional: false,
                },
                1,
            )
            .unwrap();
        let out = (
            staged.item_id.to_string(),
            staged.registration.credential_id.clone(),
            staged.registration.public_key.clone(),
        );
        vault.commit_write(staged.write, 100).unwrap();
        out
    }

    /// Records a fresh Digital Asset Links answer for `host` vouching for
    /// `package` signed with `cert` (32 bytes).
    pub fn vouch(v: &MobileVault, host: &str, package: &str, cert: &[u8], login_creds: bool) {
        use havenkeys_core::asset_links::{AppStatement, AssetLinksCache};
        use havenkeys_core::local::LocalSlot;
        let vault = v.client.vault().unwrap();
        let mut cache: AssetLinksCache = vault
            .read_local(LocalSlot::AssetLinks)
            .unwrap()
            .unwrap_or_default();
        cache.record(
            host,
            havenkeys_client::now_ms(),
            Some(vec![AppStatement {
                package: package.into(),
                certs: vec![data_encoding::HEXLOWER.encode(cert)],
                login_creds,
            }]),
        );
        vault.write_local(LocalSlot::AssetLinks, &cache).unwrap();
    }

    /// The Secret Key a second test device signs in with.
    pub fn secret_key_text(v: &MobileVault) -> String {
        let account = v
            .client
            .vault()
            .unwrap()
            .account()
            .unwrap()
            .unwrap()
            .account_id;
        v.client
            .device()
            .unwrap()
            .secret_key_text(account)
            .unwrap()
            .expose()
            .to_owned()
    }
}
