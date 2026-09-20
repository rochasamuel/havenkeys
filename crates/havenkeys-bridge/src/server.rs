//! Local socket server.
//!
//! One thread accepts connections; each connection gets a reader thread
//! (parse → authorize → answer) and a writer thread fed by a bounded
//! channel. Events such as "locked" are queued with `try_send`, so a peer
//! that stops reading can never block the code path that locks the vault.

use crate::dispatch::{dispatch, Dispatched};
use crate::ratelimit::{RateLimiter, RequestClass};
use havenkeys_core::vault::{StagedWrite, VaultService};
use havenkeys_protocol::endpoint::Endpoint;
use havenkeys_protocol::frame::{read_frame, write_frame, FrameError};
use havenkeys_protocol::{
    parse_request, ErrorCode, Event, Outgoing, Request, Response, ResultBody, MAX_REQUEST_BYTES,
    MAX_RESPONSE_BYTES,
};
use interprocess::local_socket::{prelude::*, Stream};
use std::io;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

/// Concurrent native-host connections (one per browser profile is typical).
pub const MAX_CONNECTIONS: usize = 8;
const OUTBOX: usize = 16;
/// A connection that sends nothing for this long is closed, so idle or
/// half-sent connections cannot hold slots forever. The extension closes its
/// port after 60 s idle, so real hosts never hit this. (Unix only: named pipes
/// have no receive timeout.)
const IDLE_TIMEOUT: Duration = Duration::from_secs(600);

type Frame = Zeroizing<Vec<u8>>;

struct Inner {
    vault: Arc<Mutex<VaultService>>,
    on_lock: Box<dyn Fn() + Send + Sync>,
    on_items_changed: Box<dyn Fn() + Send + Sync>,
    /// Sends a staged write to the account's server and records the result.
    /// Called without the vault lock held.
    save: Box<dyn Fn(StagedWrite) -> Result<(), ErrorCode> + Send + Sync>,
    limiter: Mutex<RateLimiter>,
    connections: Mutex<Vec<(u64, SyncSender<Frame>)>>,
    next_conn: Mutex<u64>,
}

/// Cheap to clone; all clones share state.
#[derive(Clone)]
pub struct Bridge {
    inner: Arc<Inner>,
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned mutex means a panic elsewhere; the data here (limiter,
    // connection list) stays usable, and refusing to lock would be worse.
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Bridge {
    /// `on_lock` must lock the vault (and do whatever else the app does on
    /// lock). It is called without any bridge or vault mutex held.
    pub fn new(
        vault: Arc<Mutex<VaultService>>,
        on_lock: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_change_hook(vault, on_lock, || {})
    }

    /// Like [`new`](Self::new); `on_items_changed` runs (without any mutex
    /// held) after the extension saved a login, so the UI can refresh.
    ///
    /// Without a `save` hook a login cannot be stored at all: writes go to
    /// the account's server, which this crate knows nothing about. That is
    /// what `Offline` means here, and it is the honest answer for a bridge
    /// wired up without one.
    pub fn with_change_hook(
        vault: Arc<Mutex<VaultService>>,
        on_lock: impl Fn() + Send + Sync + 'static,
        on_items_changed: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_writer(vault, on_lock, on_items_changed, |_| {
            Err(ErrorCode::Offline)
        })
    }

    /// The full wiring: `save` sends a staged write to the account's server
    /// and records it locally, exactly as a write from the desktop UI does.
    /// It is called without the vault lock held.
    pub fn with_writer(
        vault: Arc<Mutex<VaultService>>,
        on_lock: impl Fn() + Send + Sync + 'static,
        on_items_changed: impl Fn() + Send + Sync + 'static,
        save: impl Fn(StagedWrite) -> Result<(), ErrorCode> + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                vault,
                on_lock: Box::new(on_lock),
                on_items_changed: Box::new(on_items_changed),
                save: Box::new(save),
                limiter: Mutex::new(RateLimiter::default()),
                connections: Mutex::new(Vec::new()),
                next_conn: Mutex::new(0),
            }),
        }
    }

    /// Handle one raw request frame and produce the reply. Never panics on
    /// any input.
    pub fn handle_frame(&self, bytes: &[u8]) -> Outgoing {
        let env = match parse_request(bytes) {
            Ok(env) => env,
            Err(rejection) => return rejection.response().into(),
        };
        match self.handle(&env.request) {
            Ok(result) => Response::ok(env.id, result).into(),
            Err(code) => Response::err(Some(env.id), code).into(),
        }
    }

    fn handle(&self, req: &Request) -> Result<ResultBody, ErrorCode> {
        let class = match req {
            Request::Status {} => None,
            Request::Lock {} => {
                (self.inner.on_lock)();
                return Ok(ResultBody::Lock {});
            }
            Request::FindMatches { .. } | Request::GeneratePassword {} => {
                Some(RequestClass::Lookup)
            }
            Request::FillItem { .. }
            | Request::GetTotp { .. }
            | Request::CheckLogin { .. }
            | Request::SaveLogin { .. } => Some(RequestClass::Secret),
        };
        if let Some(class) = class {
            if !guard(&self.inner.limiter).allow(class, Instant::now()) {
                return Err(ErrorCode::RateLimited);
            }
        }
        // Password changes from the browser: one per item per interval.
        // Checked before the core, so probing unknown IDs also spends it.
        if let Request::SaveLogin {
            item_id: Some(id), ..
        } = req
        {
            if !guard(&self.inner.limiter).allow_item_update(*id, Instant::now()) {
                return Err(ErrorCode::RateLimited);
            }
        }
        let dispatched = {
            let mut vault = self.inner.vault.lock().map_err(|_| ErrorCode::Internal)?;
            dispatch(&mut vault, req, unix_seconds())
        };
        // The vault lock is released above, before the save below reaches the
        // network: a slow or hostile server must never delay locking.
        let result = match dispatched {
            Ok(Dispatched::Done(body)) => Ok(body),
            Ok(Dispatched::Save(staged)) => {
                let item_id = staged.item_id;
                (self.inner.save)(staged.write).map(|()| ResultBody::SaveLogin { item_id })
            }
            Err(e) => Err(e),
        };
        if matches!(req, Request::SaveLogin { .. }) && result.is_ok() {
            (self.inner.on_items_changed)();
        }
        result
    }

    /// Push an event to every connected native host. Never blocks.
    pub fn notify(&self, event: Event) {
        let Some(bytes) = Outgoing::from(event).to_bytes() else {
            return;
        };
        guard(&self.inner.connections).retain(|(_, tx)| match tx.try_send(bytes.clone()) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    pub fn connection_count(&self) -> usize {
        guard(&self.inner.connections).len()
    }

    /// Start listening on `endpoint` in a background thread.
    ///
    /// Fails with `AddrInUse` if another HavenKeys instance is already
    /// serving there, and with `PermissionDenied` if the socket directory is
    /// not private to this user.
    pub fn serve(&self, endpoint: &Endpoint) -> io::Result<()> {
        prepare_endpoint(endpoint)?;
        let listener = endpoint.listener_options()?.create_sync()?;
        let bridge = self.clone();
        std::thread::Builder::new()
            .name("bridge-accept".into())
            .spawn(move || {
                for conn in listener.incoming() {
                    match conn {
                        Ok(stream) => bridge.accept(stream),
                        // Back off instead of spinning on a persistent error.
                        Err(_) => std::thread::sleep(Duration::from_millis(100)),
                    }
                }
            })?;
        Ok(())
    }

    fn accept(&self, stream: Stream) {
        if !peer_is_same_user(&stream) {
            return;
        }
        #[cfg(unix)]
        if stream.set_recv_timeout(Some(IDLE_TIMEOUT)).is_err() {
            return;
        }
        let (tx, rx) = sync_channel::<Frame>(OUTBOX);
        let id = {
            let mut conns = guard(&self.inner.connections);
            if conns.len() >= MAX_CONNECTIONS {
                return;
            }
            let mut next = guard(&self.inner.next_conn);
            *next += 1;
            conns.push((*next, tx.clone()));
            *next
        };
        let (mut recv, mut send) = stream.split();

        let writer = std::thread::Builder::new()
            .name("bridge-write".into())
            .spawn(move || {
                for frame in rx {
                    if write_frame(&mut send, &frame, MAX_RESPONSE_BYTES).is_err() {
                        break;
                    }
                }
            });
        if writer.is_err() {
            self.forget(id);
            return;
        }

        let bridge = self.clone();
        let reader = std::thread::Builder::new()
            .name("bridge-read".into())
            .spawn(move || {
                bridge.read_loop(&mut recv, &tx);
                bridge.forget(id);
                // Dropping `tx` (and the registry's clone) ends the writer.
            });
        if reader.is_err() {
            self.forget(id);
        }
    }

    fn read_loop<R: io::Read>(&self, recv: &mut R, tx: &SyncSender<Frame>) {
        loop {
            let reply = match read_frame(recv, MAX_REQUEST_BYTES) {
                Ok(Some(bytes)) => self.handle_frame(&bytes),
                Ok(None) => return,
                // A6: an oversized frame is answered and the connection
                // closed. Its payload is never read.
                Err(FrameError::TooLarge(_)) => {
                    if let Some(b) =
                        Outgoing::from(Response::err(None, ErrorCode::TooLarge)).to_bytes()
                    {
                        let _ = tx.send(b);
                    }
                    return;
                }
                Err(FrameError::Io) => return,
            };
            let Some(bytes) = reply.to_bytes() else {
                return;
            };
            if tx.send(bytes).is_err() {
                return;
            }
        }
    }

    fn forget(&self, id: u64) {
        guard(&self.inner.connections).retain(|(c, _)| *c != id);
    }
}

#[cfg(unix)]
fn peer_is_same_user(stream: &Stream) -> bool {
    stream
        .peer_creds()
        .ok()
        .and_then(|c| c.euid())
        .is_some_and(|uid| uid == havenkeys_protocol::endpoint::current_euid())
}

// Named pipes: the pipe's DACL admits only its owner (see endpoint.rs).
#[cfg(windows)]
fn peer_is_same_user(_stream: &Stream) -> bool {
    true
}

/// Create the private socket directory and clear a stale socket file.
#[cfg(unix)]
fn prepare_endpoint(endpoint: &Endpoint) -> io::Result<()> {
    use havenkeys_protocol::endpoint::check_private_dir;
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt};

    let path = endpoint.path();
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent"))?;
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    check_private_dir(dir)?;

    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => {
            if endpoint.connect().is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another instance is running",
                ));
            }
            std::fs::remove_file(path)?;
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "socket path is occupied",
            ))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(())
}

#[cfg(windows)]
fn prepare_endpoint(_endpoint: &Endpoint) -> io::Result<()> {
    Ok(())
}
