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

pub(crate) type ServerClient = Arc<SyncClient<HttpTransport>>;

/// How often a device whose server stopped answering tries again.
pub const RETRY_INTERVAL: Duration = Duration::from_secs(15);

/// Whether this device has a server session. A locked vault is never online;
/// an unlocked one may be offline (spec 2026-09-20 §8.6).
enum Connectivity {
    Offline,
    /// The token lives here and nowhere else — never on disk — and is
    /// dropped (and zeroized) when the vault locks or the server refuses it.
    Online(Session),
    /// A session is held but the server did not answer the last request (no
    /// network, a phone waking from sleep, the server restarting). The shell
    /// shows it offline and writes are refused, but the token is kept, so the
    /// next pull that gets an answer brings the device back without an
    /// unlock. Dropping it here left a device offline until the next unlock.
    Unreachable(Session),
}

pub struct ClientConfig {
    /// The label this device reports, from [`device_label`]: the name the
    /// user gave the computer or phone, so the device list tells them apart.
    pub device_name: String,
    /// The vault database, which removing the device sets aside.
    pub vault_path: PathBuf,
}

/// Longest computer or phone name kept in a label, in characters. With the
/// platform it stays under the server's 64-character limit.
const MAX_LABEL_NAME_CHARS: usize = 40;

/// "DESKTOP-SAMS (Windows)": the name this device shows in the account's
/// device list. `name` comes from the OS and is cleaned here, because the
/// server refuses control characters and names over 64 characters; when
/// nothing is left, `kind` ("Desktop", "Android") stands in for it.
pub fn device_label(name: &str, kind: &str, platform: &str) -> String {
    let cleaned: String = name
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_LABEL_NAME_CHARS)
        .collect();
    let name = match cleaned.trim_end() {
        "" => kind,
        n => n,
    };
    format!("{name} ({platform})")
}

pub struct HavenClient {
    vault: Arc<Mutex<VaultService>>,
    device: Mutex<Device>,
    connectivity: Mutex<Connectivity>,
    /// Kept because building one sets up a TLS stack. Keyed by URL so a
    /// re-pointed vault cannot keep talking to the old server.
    server: Mutex<Option<(String, ServerClient)>>,
    /// When a pull was last attempted, so the periodic one keeps its spacing
    /// whether or not the attempt worked. Real sync times live in the
    /// account record.
    last_sync_attempt: Mutex<Option<Duration>>,
    /// The desktop's half of a pairing in progress: memory only.
    pub(crate) pairing: Mutex<Option<crate::pairing::PendingPairing>>,
    /// Serialises `poll_pairing` calls.
    pub(crate) pairing_gate: tokio::sync::Mutex<()>,
    /// Set when the vault file could not be opened. The app still starts —
    /// it has to, or there is nowhere to show the reason — and every vault
    /// access refuses with this.
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
            pairing: Mutex::new(None),
            pairing_gate: tokio::sync::Mutex::new(()),
            storage_error,
            events,
            config,
            origin: Instant::now(),
        })
    }

    pub fn vault(&self) -> ClientResult<MutexGuard<'_, VaultService>> {
        // Checked here rather than in each command: this is the one door
        // every one of them goes through.
        if let Some(err) = &self.storage_error {
            return Err(err.clone());
        }
        self.vault.lock().map_err(|_| ClientError::internal())
    }

    /// Whether the vault mutex can be taken right now, without waiting.
    #[cfg(test)]
    pub(crate) fn vault_is_free(&self) -> bool {
        self.vault.try_lock().is_ok()
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

    /// The held session, also while the server is unreachable: a request
    /// made with it is how the device finds the server again.
    pub(crate) fn session(&self) -> ClientResult<Session> {
        match self.connectivity.lock().as_deref() {
            Ok(Connectivity::Online(session) | Connectivity::Unreachable(session)) => {
                Ok(session.clone())
            }
            _ => Err(havenkeys_core::Error::Offline.into()),
        }
    }

    /// The server did not answer: keep the session, tell the shell once.
    pub(crate) fn mark_unreachable(&self) {
        let changed = match self.connectivity.lock() {
            Ok(mut c) => match std::mem::replace(&mut *c, Connectivity::Offline) {
                Connectivity::Online(session) => {
                    *c = Connectivity::Unreachable(session);
                    true
                }
                other => {
                    *c = other;
                    false
                }
            },
            Err(_) => false,
        };
        if changed {
            self.events.connectivity(false);
        }
    }

    /// The server answered with the held session: online again, told once.
    pub(crate) fn mark_reachable(&self) {
        let changed = match self.connectivity.lock() {
            Ok(mut c) => match std::mem::replace(&mut *c, Connectivity::Offline) {
                Connectivity::Unreachable(session) => {
                    *c = Connectivity::Online(session);
                    true
                }
                other => {
                    *c = other;
                    false
                }
            },
            Err(_) => false,
        };
        if changed {
            self.events.connectivity(true);
        }
    }

    pub(crate) fn set_online(&self, session: Session) {
        if let Ok(mut c) = self.connectivity.lock() {
            *c = Connectivity::Online(session);
        }
    }

    /// Drop the session without telling the shell. Returns whether one was
    /// held, and whether the shell still showed it online.
    fn drop_session(&self) -> (bool, bool) {
        match self.connectivity.lock() {
            Ok(mut c) => match std::mem::replace(&mut *c, Connectivity::Offline) {
                Connectivity::Online(_) => (true, true),
                Connectivity::Unreachable(_) => (true, false),
                Connectivity::Offline => (false, false),
            },
            Err(_) => (false, false),
        }
    }

    /// Drop the session, and tell the shell when it still showed it online.
    /// Returns whether one was held.
    pub(crate) fn go_offline(&self) -> bool {
        let (held, shown_online) = self.drop_session();
        if shown_online {
            self.events.connectivity(false);
        }
        held
    }

    /// The HTTP client for this vault's server.
    pub(crate) fn server(&self) -> ClientResult<ServerClient> {
        let url = self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?
            .server_url;
        self.server_for(&url)
    }

    pub(crate) fn server_for(&self, url: &str) -> ClientResult<ServerClient> {
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
    pub(crate) fn forget_server(&self) {
        if let Ok(mut cached) = self.server.lock() {
            *cached = None;
        }
    }

    pub(crate) fn mark_sync_attempt(&self) {
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = Some(self.origin.elapsed());
        }
    }

    /// Is the periodic pull due? Only with a session; every [`PULL_INTERVAL`]
    /// while online, and every [`RETRY_INTERVAL`] while the server is
    /// unreachable, so the device comes back soon after the network does.
    ///
    /// [`PULL_INTERVAL`]: crate::PULL_INTERVAL
    pub fn pull_due(&self) -> bool {
        self.pull_interval()
            .is_some_and(|every| self.sync_due(every))
    }

    fn pull_interval(&self) -> Option<Duration> {
        match self.connectivity.lock().as_deref() {
            Ok(Connectivity::Online(_)) => Some(crate::sync::PULL_INTERVAL),
            Ok(Connectivity::Unreachable(_)) => Some(RETRY_INTERVAL),
            _ => None,
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

    /// Refuse a write while the account is frozen (spec 2026-10-07 §6.3).
    /// Decided from the stored entitlement, so it answers offline too.
    pub fn require_full(&self) -> ClientResult<()> {
        match self.vault()?.entitlement()? {
            havenkeys_core::store::Entitlement::Full => Ok(()),
            havenkeys_core::store::Entitlement::Frozen => Err(ClientError::account_frozen()),
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
        fn account_deleted(&self, keychain_cleared: bool) {
            self.push(format!("account_deleted:{keychain_cleared}"));
        }
    }

    pub(crate) fn client_in(dir: &std::path::Path) -> (Arc<HavenClient>, Arc<RecordingEvents>) {
        client_with_store(dir, Box::new(MemoryKeyStore::default()))
    }

    pub(crate) fn client_with_store(
        dir: &std::path::Path,
        store: Box<dyn crate::key_store::KeyStore>,
    ) -> (Arc<HavenClient>, Arc<RecordingEvents>) {
        let vault = Arc::new(Mutex::new(VaultService::new(
            Store::open_in_memory().unwrap(),
        )));
        let device = Device::load(dir, store);
        let events = Arc::new(RecordingEvents::default());
        let client = HavenClient::new(
            vault,
            device,
            None,
            events.clone(),
            ClientConfig {
                device_name: "Test".into(),
                vault_path: dir.join("vault.sqlite3"),
            },
        );
        (client, events)
    }

    #[test]
    fn a_device_label_names_the_computer_and_its_platform() {
        assert_eq!(
            device_label("DESKTOP-SAMS", "Desktop", "Windows"),
            "DESKTOP-SAMS (Windows)"
        );
        assert_eq!(
            device_label("  Sam's\tPixel\n 8 ", "Android", "Android"),
            "Sam's Pixel 8 (Android)"
        );
    }

    #[test]
    fn a_device_label_without_a_usable_name_falls_back_to_the_kind() {
        assert_eq!(device_label("", "Desktop", "Linux"), "Desktop (Linux)");
        assert_eq!(
            device_label(" \u{0}\u{7} ", "Desktop", "Linux"),
            "Desktop (Linux)"
        );
    }

    #[test]
    fn a_long_device_name_is_cut_to_fit_the_server_limit() {
        let label = device_label(&"é".repeat(200), "Desktop", "Windows");
        assert_eq!(label, format!("{} (Windows)", "é".repeat(40)));
        assert!(label.chars().count() <= 64);
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
                device_name: "Test".into(),
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

    fn a_session() -> Session {
        Session::new("token".into(), "never".into(), Uuid::nil(), Uuid::nil())
    }

    #[test]
    fn an_unanswered_request_keeps_the_session_and_the_next_answer_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        client.set_online(a_session());
        assert!(client.pull_due());
        client.mark_sync_attempt();
        assert!(!client.pull_due());

        client.failed(havenkeys_sync_client::SyncError::Unavailable);
        // A second unanswered request changes nothing.
        client.failed(havenkeys_sync_client::SyncError::Unavailable);
        // Shown offline, writes refused, but the token is still there to try with.
        assert!(!client.is_online());
        assert_eq!(client.require_online().unwrap_err().code, "offline");
        assert!(client.session().is_ok());

        client.mark_reachable();
        assert!(client.is_online());
        // Each change told once.
        assert_eq!(events.seen(), vec!["online:false", "online:true"]);
    }

    #[test]
    fn an_unreachable_device_retries_sooner_than_the_periodic_pull() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        assert_eq!(client.pull_interval(), None);
        client.set_online(a_session());
        assert_eq!(client.pull_interval(), Some(crate::PULL_INTERVAL));
        client.mark_unreachable();
        assert_eq!(client.pull_interval(), Some(RETRY_INTERVAL));
        assert!(RETRY_INTERVAL < crate::PULL_INTERVAL);
    }

    #[test]
    fn a_refused_session_is_dropped_and_signed_out_even_while_unreachable() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        client.set_online(a_session());
        client.mark_unreachable();
        client.failed(havenkeys_sync_client::SyncError::Unauthorized);
        assert!(client.session().is_err());
        assert!(!client.pull_due());
        // No second "offline": the shell already showed it.
        assert_eq!(events.seen(), vec!["online:false", "signed_out"]);
    }

    #[test]
    fn locking_drops_a_session_the_server_stopped_answering() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        client.set_online(a_session());
        client.mark_unreachable();
        client.lock("user");
        assert!(client.session().is_err());
        assert!(!client.pull_due());
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
