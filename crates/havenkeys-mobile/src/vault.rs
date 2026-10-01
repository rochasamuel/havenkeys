use crate::error::{MobileError, MobileResult};
use crate::events::{LockClock, MobileEvents, VaultEvents};
use crate::key_file::{KeystoreCipher, KeystoreKeyStore};
use havenkeys_client::device::Device;
use havenkeys_client::kit::KitFields;
use havenkeys_client::{ClientConfig, ClientError, HavenClient};
use havenkeys_core::store::Store;
use havenkeys_core::vault::{VaultService, VaultState};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const AUTO_LOCK_TICK: Duration = Duration::from_secs(5);

#[derive(uniffi::Record)]
pub struct MobileConfig {
    /// The app's private files directory.
    pub data_dir: String,
    /// HavenKeys' own package: it is never a fill target.
    pub own_package: String,
}

#[derive(uniffi::Enum)]
pub enum LockState {
    Locked,
    Unlocking,
    Unlocked,
}

#[derive(uniffi::Record)]
pub struct Status {
    pub state: LockState,
    pub vault_exists: bool,
    pub online: bool,
    pub needs_secret_key: bool,
    pub email: Option<String>,
    pub server_url: Option<String>,
    pub last_synced_at: Option<i64>,
    pub unreadable_items: u32,
}

#[derive(uniffi::Object)]
pub struct MobileVault {
    pub(crate) client: Arc<HavenClient>,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) clock: Arc<LockClock>,
    /// A scanned Emergency Kit waiting for its master password.
    pub(crate) kit: Mutex<Option<KitFields>>,
    pub(crate) own_package: String,
}

impl MobileVault {
    pub(crate) fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.runtime.block_on(f)
    }

    fn auto_lock_tick(client: &HavenClient, clock: &LockClock) {
        let reason = {
            let Ok(vault) = client.vault() else { return };
            if !vault.is_unlocked() {
                return;
            }
            clock.due()
        };
        if let Some(reason) = reason {
            client.lock(reason.as_str());
        }
    }
}

#[uniffi::export]
impl MobileVault {
    #[uniffi::constructor]
    pub fn new(
        config: MobileConfig,
        events: Arc<dyn VaultEvents>,
        cipher: Arc<dyn KeystoreCipher>,
    ) -> MobileResult<Arc<Self>> {
        let dir = PathBuf::from(&config.data_dir);
        let path = dir.join("vault.sqlite3");
        let (store, storage_error) = match Store::open(&path) {
            Ok(s) => (s, None),
            Err(_) => (
                Store::open_in_memory()?,
                Some(ClientError::vault_unreadable(&dir)),
            ),
        };
        let vault = Arc::new(Mutex::new(VaultService::new(store)));
        let device = Device::load(&dir, Box::new(KeystoreKeyStore::new(dir.clone(), cipher)))
            .without_file_fallback();
        let clock = Arc::new(LockClock::new());
        let client = HavenClient::new(
            vault,
            device,
            storage_error,
            Arc::new(MobileEvents::new(events, clock.clone())),
            ClientConfig {
                device_name: "Android",
                vault_path: path,
            },
        );
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("havenkeys")
            .enable_all()
            .build()
            .map_err(|_| MobileError::internal())?;
        let (ticking, tick_clock) = (client.clone(), clock.clone());
        runtime.spawn(async move {
            let mut every = tokio::time::interval(AUTO_LOCK_TICK);
            loop {
                every.tick().await;
                Self::auto_lock_tick(&ticking, &tick_clock);
            }
        });
        Ok(Arc::new(Self {
            client,
            runtime,
            clock,
            kit: Mutex::new(None),
            own_package: config.own_package,
        }))
    }

    /// Safe while locked: no secrets.
    pub fn status(&self) -> MobileResult<Status> {
        let vs = self.client.vault()?.status()?;
        let device = self.client.device_status()?;
        let account = self.client.account_status()?;
        Ok(Status {
            state: match vs.state {
                VaultState::Unlocked => LockState::Unlocked,
                VaultState::Unlocking => LockState::Unlocking,
                VaultState::Locked | VaultState::Locking => LockState::Locked,
            },
            vault_exists: vs.vault_exists,
            online: device.online,
            needs_secret_key: device.needs_secret_key,
            email: account.as_ref().map(|a| a.email.clone()),
            server_url: account.as_ref().map(|a| a.server_url.clone()),
            last_synced_at: account.and_then(|a| a.last_synced_at),
            unreadable_items: u32::try_from(vs.unreadable_items).unwrap_or(u32::MAX),
        })
    }

    pub fn lock(&self) {
        self.client.lock("user");
        self.forget_kit();
    }

    pub fn screen_turned_off(&self) {
        if self.client.require_unlocked().is_ok() && self.device_settings().lock_on_screen_off {
            self.client.lock("screen_off");
        }
    }

    /// The user did something: reset the idle timer.
    pub fn touch(&self) {
        self.clock.touch();
    }

    /// Check the auto-lock now (the app calls this when it returns to the
    /// foreground, in case the process was frozen).
    pub fn tick(&self) {
        Self::auto_lock_tick(&self.client, &self.clock);
    }

    pub fn forget_kit(&self) {
        if let Ok(mut kit) = self.kit.lock() {
            *kit = None;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::events::VaultEvents;
    use crate::key_file::{CipherError, KeystoreCipher};
    use crate::settings::MobileSettings;
    use std::sync::Mutex;

    pub(crate) const PASSWORD: &str = "correct horse battery staple";

    #[derive(Default)]
    pub(crate) struct Seen(pub Mutex<Vec<String>>);
    impl Seen {
        pub fn has(&self, e: &str) -> bool {
            self.0.lock().unwrap().iter().any(|x| x == e)
        }
    }
    impl VaultEvents for Seen {
        fn locked(&self, reason: String) {
            self.0.lock().unwrap().push(format!("locked:{reason}"))
        }
        fn unlocked(&self) {
            self.0.lock().unwrap().push("unlocked".into())
        }
        fn connectivity(&self, online: bool) {
            self.0.lock().unwrap().push(format!("online:{online}"))
        }
        fn signed_out(&self) {
            self.0.lock().unwrap().push("signed_out".into())
        }
        fn items_changed(&self) {
            self.0.lock().unwrap().push("items_changed".into())
        }
        fn removed(&self) {
            self.0.lock().unwrap().push("removed".into())
        }
    }

    /// Stands in for the Keystore: XOR, so the file is not the plaintext.
    pub(crate) struct XorCipher(pub bool);
    impl KeystoreCipher for XorCipher {
        fn seal(&self, plaintext: Vec<u8>) -> Result<Vec<u8>, CipherError> {
            if !self.0 {
                return Err(CipherError::Failed);
            }
            Ok(plaintext.iter().map(|b| b ^ 0x5a).collect())
        }
        fn open(&self, sealed: Vec<u8>) -> Result<Vec<u8>, CipherError> {
            if !self.0 {
                return Err(CipherError::Failed);
            }
            Ok(sealed.iter().map(|b| b ^ 0x5a).collect())
        }
    }

    pub(crate) fn mobile(dir: &std::path::Path) -> (Arc<MobileVault>, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        let vault = MobileVault::new(
            MobileConfig {
                data_dir: dir.to_string_lossy().into_owned(),
                own_package: "net.havenkeys.android".into(),
            },
            seen.clone(),
            Arc::new(XorCipher(true)),
        )
        .unwrap();
        (vault, seen)
    }

    /// An unlocked vault with an account (offline).
    pub(crate) fn unlocked(dir: &std::path::Path) -> (Arc<MobileVault>, Arc<Seen>) {
        let (vault, seen) = mobile(dir);
        havenkeys_client::testing::seed_account_vault(&vault.client, PASSWORD);
        (vault, seen)
    }

    pub(crate) fn wait_for(what: impl Fn() -> bool) {
        for _ in 0..250 {
            if what() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("timed out");
    }

    #[test]
    fn a_new_install_has_no_vault_and_is_locked() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = mobile(dir.path());
        let s = vault.status().unwrap();
        assert!(matches!(s.state, LockState::Locked));
        assert!(!s.vault_exists);
        assert!(!s.online);
    }

    #[test]
    fn locking_tells_the_app_from_its_own_thread() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, seen) = unlocked(dir.path());
        vault.lock();
        wait_for(|| seen.has("locked:user"));
        assert!(matches!(vault.status().unwrap().state, LockState::Locked));
    }

    #[test]
    fn screen_off_locks_only_when_the_setting_is_on() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, seen) = unlocked(dir.path());
        let mut s = vault.settings().unwrap();
        assert!(s.lock_on_screen_off);
        s.lock_on_screen_off = false;
        vault.update_settings(s.clone()).unwrap();
        vault.screen_turned_off();
        assert!(matches!(vault.status().unwrap().state, LockState::Unlocked));
        s.lock_on_screen_off = true;
        vault.update_settings(s).unwrap();
        vault.screen_turned_off();
        wait_for(|| seen.has("locked:screen_off"));
    }

    #[test]
    fn settings_round_trip_and_bad_values_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = unlocked(dir.path());
        let defaults = vault.settings().unwrap();
        assert!(!defaults.confirm_before_filling);
        assert!(defaults.asset_links);
        let changed = MobileSettings {
            auto_lock_minutes: 5,
            confirm_before_filling: true,
            asset_links: false,
            ..defaults
        };
        vault.update_settings(changed.clone()).unwrap();
        let back = vault.settings().unwrap();
        assert_eq!(back.auto_lock_minutes, 5);
        assert!(back.confirm_before_filling);
        assert!(!back.asset_links);
        let bad = MobileSettings {
            auto_lock_minutes: 7,
            ..back
        };
        assert!(vault.update_settings(bad).is_err());
    }

    #[test]
    fn a_locked_vault_has_no_settings_to_read() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = unlocked(dir.path());
        vault.lock();
        assert!(vault.settings().is_err());
    }
}
