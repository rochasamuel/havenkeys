//! The client's state: the vault handle, the device record, the server
//! session and the HTTP client.

use crate::device::Device;
use crate::error::{ClientError, ClientResult};
use crate::events::ClientEvents;
use havenkeys_core::vault::VaultService;
use havenkeys_sync_client::{HttpTransport, Session, SyncClient};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub type ServerClient = Arc<SyncClient<HttpTransport>>;

/// Whether this device has a server session. A locked vault is never online;
/// an unlocked one may be offline (spec 2026-09-20 §8.6).
enum Connectivity {
    Offline,
    /// The token lives here and nowhere else — never on disk — and is
    /// dropped (and zeroized) when the vault locks or the device goes
    /// offline.
    Online(Session),
}

pub struct ClientConfig {
    /// The label this device reports. Deliberately not the hostname, which
    /// is metadata the server has no use for.
    pub device_name: &'static str,
    /// The vault database, which removing the device sets aside.
    pub vault_path: PathBuf,
}

pub struct HavenClient {
    vault: Arc<Mutex<VaultService>>,
    device: Mutex<Device>,
    connectivity: Mutex<Connectivity>,
    /// Kept because building one sets up a TLS stack. Keyed by URL so a
    /// re-pointed vault cannot keep talking to the old server.
    server: Mutex<Option<(String, ServerClient)>>,
    /// When a pull was last attempted, so the periodic one keeps its spacing
    /// whether or not the attempt worked.
    last_sync_attempt: Mutex<Option<Duration>>,
    /// Set when the vault file could not be opened: every vault access
    /// refuses with it.
    storage_error: Option<ClientError>,
    pub(crate) events: Arc<dyn ClientEvents>,
    pub(crate) config: ClientConfig,
    origin: Instant,
}

impl HavenClient {
    pub fn new(
        vault: Arc<Mutex<VaultService>>,
        device: Device,
        storage_error: Option<ClientError>,
        events: Arc<dyn ClientEvents>,
        config: ClientConfig,
    ) -> Arc<Self> {
        Arc::new(Self {
            vault,
            device: Mutex::new(device),
            connectivity: Mutex::new(Connectivity::Offline),
            server: Mutex::new(None),
            last_sync_attempt: Mutex::new(None),
            storage_error,
            events,
            config,
            origin: Instant::now(),
        })
    }

    pub fn vault(&self) -> ClientResult<MutexGuard<'_, VaultService>> {
        if let Some(err) = &self.storage_error {
            return Err(err.clone());
        }
        self.vault.lock().map_err(|_| ClientError::internal())
    }

    pub fn device(&self) -> ClientResult<MutexGuard<'_, Device>> {
        self.device.lock().map_err(|_| ClientError::internal())
    }

    pub fn device_id(&self) -> ClientResult<Uuid> {
        Ok(self.device()?.id)
    }

    pub fn is_online(&self) -> bool {
        matches!(
            self.connectivity.lock().as_deref(),
            Ok(Connectivity::Online(_))
        )
    }

    pub fn session(&self) -> ClientResult<Session> {
        match self.connectivity.lock().as_deref() {
            Ok(Connectivity::Online(session)) => Ok(session.clone()),
            _ => Err(havenkeys_core::Error::Offline.into()),
        }
    }

    pub(crate) fn set_online(&self, session: Session) {
        if let Ok(mut c) = self.connectivity.lock() {
            *c = Connectivity::Online(session);
        }
    }

    /// Drop the session without telling the shell. Returns whether one was held.
    fn drop_session(&self) -> bool {
        match self.connectivity.lock() {
            Ok(mut c) => {
                let was_online = matches!(*c, Connectivity::Online(_));
                *c = Connectivity::Offline;
                was_online
            }
            Err(_) => false,
        }
    }

    /// Drop the session and tell the shell, when one was held.
    pub fn go_offline(&self) -> bool {
        let dropped = self.drop_session();
        if dropped {
            self.events.connectivity(false);
        }
        dropped
    }

    /// The HTTP client for this vault's server.
    pub fn server(&self) -> ClientResult<ServerClient> {
        let url = self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?
            .server_url;
        self.server_for(&url)
    }

    pub fn server_for(&self, url: &str) -> ClientResult<ServerClient> {
        let mut cached = self.server.lock().map_err(|_| ClientError::internal())?;
        if let Some((cached_url, client)) = cached.as_ref() {
            if cached_url == url {
                return Ok(client.clone());
            }
        }
        let client = Arc::new(SyncClient::new(HttpTransport::new(url)?));
        *cached = Some((url.to_string(), client.clone()));
        Ok(client)
    }

    /// So a stale client for an old server is never reused after this
    /// device leaves its account.
    pub fn forget_server(&self) {
        if let Ok(mut cached) = self.server.lock() {
            *cached = None;
        }
    }

    pub(crate) fn mark_sync_attempt(&self) {
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = Some(self.origin.elapsed());
        }
    }

    /// Is a periodic pull due? Also true when none has been attempted yet.
    pub fn sync_due(&self, interval: Duration) -> bool {
        match self.last_sync_attempt.lock() {
            Ok(last) => last.is_none_or(|at| self.origin.elapsed().saturating_sub(at) >= interval),
            Err(_) => false,
        }
    }

    pub fn require_unlocked(&self) -> ClientResult<()> {
        if self.vault()?.is_unlocked() {
            Ok(())
        } else {
            Err(havenkeys_core::Error::Locked.into())
        }
    }

    pub fn require_online(&self) -> ClientResult<()> {
        if self.is_online() {
            Ok(())
        } else {
            Err(havenkeys_core::Error::Offline.into())
        }
    }

    /// Lock the vault and drop the session: a locked vault cannot reach the
    /// server at all (spec 2026-09-20 §6).
    pub fn lock(&self, reason: &'static str) {
        let was_open = match self.vault.lock() {
            Ok(mut v) => v.lock(),
            // A poisoned mutex means a panic mid-operation; the session is
            // still dropped.
            Err(poisoned) => poisoned.into_inner().lock(),
        };
        self.drop_session();
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = None;
        }
        self.events.locked(reason, was_open);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::key_store::MemoryKeyStore;
    use havenkeys_core::store::Store;
    use havenkeys_core::sync::SyncReport;

    #[derive(Default)]
    pub(crate) struct RecordingEvents(pub Mutex<Vec<String>>);

    impl RecordingEvents {
        pub fn seen(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
        fn push(&self, s: String) {
            self.0.lock().unwrap().push(s);
        }
    }

    impl ClientEvents for RecordingEvents {
        fn unlocked(&self, minutes: u32) {
            self.push(format!("unlocked:{minutes}"));
        }
        fn locked(&self, reason: &'static str, was_open: bool) {
            self.push(format!("locked:{reason}:{was_open}"));
        }
        fn connectivity(&self, online: bool) {
            self.push(format!("online:{online}"));
        }
        fn signed_out(&self) {
            self.push("signed_out".into());
        }
        fn synced(&self, _: SyncReport) {
            self.push("synced".into());
        }
        fn items_changed(&self) {
            self.push("items_changed".into());
        }
        fn removed(&self, keychain_cleared: bool) {
            self.push(format!("removed:{keychain_cleared}"));
        }
    }

    pub(crate) fn client_in(dir: &std::path::Path) -> (Arc<HavenClient>, Arc<RecordingEvents>) {
        let vault = Arc::new(Mutex::new(VaultService::new(
            Store::open_in_memory().unwrap(),
        )));
        let device = Device::load(dir, Box::new(MemoryKeyStore::default()));
        let events = Arc::new(RecordingEvents::default());
        let client = HavenClient::new(
            vault,
            device,
            None,
            events.clone(),
            ClientConfig {
                device_name: "Test",
                vault_path: dir.join("vault.sqlite3"),
            },
        );
        (client, events)
    }

    #[test]
    fn a_new_client_is_offline_and_refuses_writes() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        assert!(!client.is_online());
        assert_eq!(client.require_online().unwrap_err().code, "offline");
        assert_eq!(client.session().unwrap_err().code, "offline");
    }

    #[test]
    fn a_storage_error_is_returned_by_every_vault_access() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Mutex::new(VaultService::new(
            Store::open_in_memory().unwrap(),
        )));
        let client = HavenClient::new(
            vault,
            Device::load(dir.path(), Box::new(MemoryKeyStore::default())),
            Some(ClientError::vault_unreadable(dir.path())),
            Arc::new(RecordingEvents::default()),
            ClientConfig {
                device_name: "Test",
                vault_path: dir.path().join("vault.sqlite3"),
            },
        );
        assert_eq!(client.vault().err().unwrap().code, "vault_unreadable");
    }

    #[test]
    fn locking_a_locked_vault_tells_the_shell_it_was_not_open() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        client.lock("user");
        assert_eq!(events.seen(), vec!["locked:user:false"]);
    }

    #[test]
    fn going_offline_is_announced_only_when_a_session_was_held() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        assert!(!client.go_offline());
        assert!(events.seen().is_empty());
    }

    #[test]
    fn the_server_client_is_rebuilt_for_another_url() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        let a = client.server_for("https://a.example.com").unwrap();
        let again = client.server_for("https://a.example.com").unwrap();
        let b = client.server_for("https://b.example.com").unwrap();
        assert!(Arc::ptr_eq(&a, &again));
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn a_pull_is_due_until_one_is_attempted() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        assert!(client.sync_due(Duration::from_secs(60)));
        client.mark_sync_attempt();
        assert!(!client.sync_due(Duration::from_secs(60)));
        client.lock("user");
        assert!(client.sync_due(Duration::from_secs(60)));
    }
}
