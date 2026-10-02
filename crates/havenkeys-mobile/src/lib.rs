//! The API the HavenKeys Android app (and later iOS) calls, through UniFFI.
//!
//! Intent-level, like the desktop bridge: it never returns keys, blobs or
//! whole vault objects, and every call that returns a secret checks the lock
//! state, the item and the fill target here (spec 2026-10-01-android-app
//! §4.2). Calls block; the app runs them on `Dispatchers.IO`.

#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

uniffi::setup_scaffolding!();

mod account;
#[cfg(target_os = "android")]
mod android_tls;
mod asset_links_fetch;
mod autofill;
mod credentials;
mod edit;
mod error;
mod events;
mod items;
mod key_file;
mod onboarding;
mod passkey_json;
mod qr;
mod save;
mod settings;
mod unlock;
mod vault;

pub use account::DeviceInfo;
pub use autofill::{AutofillMatch, BoundFill, FillValues, TargetFacts, TargetKind};
pub use credentials::{
    privileged_browsers_json, CredentialCaller, PasskeyCreatePlan, PasskeyOffer,
};
pub use edit::{Change, EditField, FieldChange, ItemDraft, ItemEdit, MatchKind, Website};
pub use error::{MobileError, MobileResult};
pub use events::VaultEvents;
pub use items::{
    FieldKind, Generated, GeneratorOptions, ItemKind, ItemSummary, ItemView, TotpNow, ViewField,
};
pub use key_file::{CipherError, KeystoreCipher};
pub use onboarding::{KitPreview, LumaFrame};
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
        let input = ItemInput {
            item_type: ItemType::Login,
            title: "GitHub".into(),
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: "https://github.com".into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: None,
            sections: None,
        };
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, 1).unwrap();
        vault.commit_write(staged, 1).unwrap();
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
