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
mod edit;
mod error;
mod events;
mod items;
mod key_file;
mod onboarding;
mod qr;
mod save;
mod settings;
mod unlock;
mod vault;

pub use account::DeviceInfo;
pub use autofill::{AutofillMatch, BoundFill, FillValues, TargetFacts, TargetKind};
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
