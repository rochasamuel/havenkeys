//! Local socket server.
//!
//! One thread accepts connections; each connection gets a reader thread
//! (parse → authorize → answer) and a writer thread fed by a bounded
//! channel. A request that may wait for the user (a repeat password change)
//! is answered on a thread of its own, so it never holds up the connection;
//! every reply still goes through the one writer, so frames never interleave.
//! Events such as "locked" are queued with `try_send`, so a peer
//! that stops reading can never block the code path that locks the vault.

use crate::dispatch::{dispatch_with, Dispatched, UpdateConsent};
use crate::ratelimit::{RateLimiter, RequestClass};
use havenkeys_core::vault::{ConfirmUpdate, StagedWrite, VaultService};
use havenkeys_protocol::endpoint::Endpoint;
use havenkeys_protocol::frame::{read_frame, write_frame, FrameError};
use havenkeys_protocol::{
    parse_request, ErrorCode, Event, Outgoing, Request, RequestEnvelope, Response, ResultBody,
    MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES,
};
use interprocess::local_socket::{prelude::*, Stream};
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Concurrent native-host connections (one per browser profile is typical).
pub const MAX_CONNECTIONS: usize = 8;
const OUTBOX: usize = 16;
/// A connection that sends nothing for this long is closed, so idle or
/// half-sent connections cannot hold slots forever. The extension closes its
/// port after 60 s idle, so real hosts never hit this.
///
/// Unix only, and the `cfg` says so rather than leaving a constant Windows
/// builds warn about: named pipes have no receive timeout, so there an idle
/// connection holds its slot until the host exits.
#[cfg(unix)]
const IDLE_TIMEOUT: Duration = Duration::from_secs(600);

type Frame = Zeroizing<Vec<u8>>;

/// Requests one connection may have answered on threads of their own at
/// once (see `may_wait_for_user`); the rest are answered `busy` without
/// starting a thread. One place is for the confirmation that may be waiting
/// for the user (only one waits at a time, across all connections). The
/// second lets a password change that needs no confirmation (another
/// login's first change in its interval) still go through meanwhile,
/// instead of being refused only because a confirmation is pending.
const MAX_BACKGROUND_PER_CONNECTION: usize = 2;

/// Hook run once `open_item` is allowed; see `Bridge::set_open_item_hook`.
type OpenItemHook = Arc<dyn Fn(Uuid) + Send + Sync>;

/// Hook run once `show_unlock` is allowed; see `Bridge::set_show_unlock_hook`.
type ShowUnlockHook = Arc<dyn Fn() + Send + Sync>;

/// How long the desktop has to answer a confirmation before it counts as a
/// Deny (spec 2026-10-13-confirm-repeat-password-change §4).
pub const CONFIRM_UPDATE_TIMEOUT: Duration = Duration::from_secs(60);

/// Asks the user to allow a repeat password change; see
/// `Bridge::set_confirm_update_hook`.
type ConfirmUpdateHook = Arc<dyn Fn(ConfirmUpdate, Duration) -> bool + Send + Sync>;

/// A password change's claim on its item's 10-minute interval.
#[derive(Clone, Copy)]
enum ItemBudget {
    /// Not an item update.
    None,
    /// Reserved at this instant; given back if the change does not happen.
    Reserved(Uuid, Instant),
    /// The interval is running: only a change the user allows on the
    /// desktop goes through, and it starts the interval again.
    CoolingDown(Uuid),
}

/// One of a connection's background-answer places. Freed on drop, however
/// the answer ends.
struct BackgroundSlot(Arc<AtomicUsize>);

impl BackgroundSlot {
    fn claim(count: &Arc<AtomicUsize>) -> Option<Self> {
        count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_BACKGROUND_PER_CONNECTION).then_some(n + 1)
            })
            .ok()
            .map(|_| Self(count.clone()))
    }
}

impl Drop for BackgroundSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The one confirmation slot. Freed on drop, however the wait ends.
struct PendingSlot<'a>(&'a AtomicBool);

impl<'a> PendingSlot<'a> {
    fn claim(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self(flag))
    }
}

impl Drop for PendingSlot<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

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
    /// What `open_item` does once allowed; see `set_open_item_hook`.
    on_open_item: Mutex<Option<OpenItemHook>>,
    /// What `show_unlock` does once allowed; see `set_show_unlock_hook`.
    on_show_unlock: Mutex<Option<ShowUnlockHook>>,
    /// Asks the user about a repeat password change; see
    /// `set_confirm_update_hook`.
    on_confirm_update: Mutex<Option<ConfirmUpdateHook>>,
    /// Set while a confirmation is with the user: only one at a time.
    confirm_pending: AtomicBool,
    confirm_timeout: Mutex<Duration>,
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
                on_open_item: Mutex::new(None),
                on_show_unlock: Mutex::new(None),
                on_confirm_update: Mutex::new(None),
                confirm_pending: AtomicBool::new(false),
                confirm_timeout: Mutex::new(CONFIRM_UPDATE_TIMEOUT),
            }),
        }
    }

    /// What `open_item` does once the bridge has allowed it: the desktop
    /// shows its window on that item's editor. Called on the bridge's
    /// thread, without the vault lock held. Without a hook, `open_item`
    /// answers `internal`: a bridge with no UI has nothing to open.
    pub fn set_open_item_hook(&self, hook: impl Fn(Uuid) + Send + Sync + 'static) {
        *guard(&self.inner.on_open_item) = Some(Arc::new(hook));
    }

    /// What `show_unlock` does once the bridge has allowed it: the desktop
    /// brings its window forward (on the unlock screen while locked). Called
    /// on the bridge's thread, without the vault lock held. Without a hook,
    /// `show_unlock` answers `internal`, as `open_item` does.
    pub fn set_show_unlock_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *guard(&self.inner.on_show_unlock) = Some(Arc::new(hook));
    }

    /// How a repeat password change from the browser is put to the user: the
    /// desktop raises its window and asks, returning `true` for Allow. Gets
    /// the change's summary (no secret) and how long it has. Called on a
    /// bridge thread without the vault lock or any bridge mutex held; it
    /// must return within that time and answer `false` on any failure.
    /// Without a hook every such change is refused (`denied`).
    pub fn set_confirm_update_hook(
        &self,
        hook: impl Fn(ConfirmUpdate, Duration) -> bool + Send + Sync + 'static,
    ) {
        *guard(&self.inner.on_confirm_update) = Some(Arc::new(hook));
    }

    /// Shortens the confirmation timeout. For tests: the desktop keeps
    /// [`CONFIRM_UPDATE_TIMEOUT`].
    pub fn set_confirm_timeout(&self, timeout: Duration) {
        *guard(&self.inner.confirm_timeout) = timeout;
    }

    /// Handle one raw request frame and produce the reply. Never panics on
    /// any input.
    pub fn handle_frame(&self, bytes: &[u8]) -> Outgoing {
        match parse_request(bytes) {
            Ok(env) => self.answer(&env),
            Err(rejection) => rejection.response().into(),
        }
    }

    fn answer(&self, env: &RequestEnvelope) -> Outgoing {
        match self.handle(&env.request) {
            Ok(result) => fit_frame(Response::ok(env.id, result)).into(),
            Err(code) => Response::err(Some(env.id), code).into(),
        }
    }

    /// Answer `env` on its own thread and queue the reply for the
    /// connection's writer when it is ready. Replies carry the request's ID,
    /// so they may arrive out of order. With no place left on this
    /// connection (or no thread to be had) it is answered `busy` at once.
    /// The thread can outlive its connection (its reply is then dropped,
    /// but the request still completes), yet such threads stay bounded:
    /// only one at a time waits on the user, and the others are answered
    /// `busy` or rate-limited.
    fn answer_in_background(
        &self,
        env: RequestEnvelope,
        tx: &SyncSender<Frame>,
        in_flight: &Arc<AtomicUsize>,
    ) {
        let id = env.id;
        let busy = || {
            if let Some(bytes) = Outgoing::from(Response::err(Some(id), ErrorCode::Busy)).to_bytes()
            {
                let _ = tx.send(bytes);
            }
        };
        let Some(slot) = BackgroundSlot::claim(in_flight) else {
            return busy();
        };
        let bridge = self.clone();
        let reply_tx = tx.clone();
        let spawned = std::thread::Builder::new()
            .name("bridge-confirm".into())
            .spawn(move || {
                let _slot = slot;
                if let Some(bytes) = bridge.answer(&env).to_bytes() {
                    let _ = reply_tx.send(bytes);
                }
            });
        // The closure (and its slot) is dropped with the error.
        if spawned.is_err() {
            busy();
        }
    }

    fn handle(&self, req: &Request) -> Result<ResultBody, ErrorCode> {
        if let Request::Lock {} = req {
            (self.inner.on_lock)();
            return Ok(ResultBody::Lock {});
        }
        let budget = self.check_rate_limits(req)?;
        let consent = match budget {
            ItemBudget::CoolingDown(_) => UpdateConsent::AskAlways,
            ItemBudget::None | ItemBudget::Reserved(..) => UpdateConsent::AskIfRecent,
        };
        let result = self.handle_limited(req, consent);
        match (&result, budget) {
            // BR-2: a password change that was denied or failed does not
            // spend the item's budget, so a retry after an outage still goes
            // through.
            (Err(_), ItemBudget::Reserved(item, at)) => {
                guard(&self.inner.limiter).release_item_update(item, at)
            }
            // A change the user allowed inside the interval starts it again.
            (Ok(_), ItemBudget::CoolingDown(item)) => {
                guard(&self.inner.limiter).record_item_update(item, Instant::now())
            }
            _ => {}
        }
        result
    }

    fn handle_limited(
        &self,
        req: &Request,
        consent: UpdateConsent,
    ) -> Result<ResultBody, ErrorCode> {
        // Answered in any lock state and without the vault: it carries no
        // secret and returns none. The browser-integration switch is sealed
        // in the vault, so it cannot be read while locked; the rate limit
        // above bounds how often the window can be raised.
        if let Request::ShowUnlock {} = req {
            return self.show_unlock().map(|()| ResultBody::ShowUnlock {});
        }
        let dispatched = {
            let mut vault = self.inner.vault.lock().map_err(|_| ErrorCode::Internal)?;
            dispatch_with(&mut vault, req, unix_seconds(), consent)
        };
        // A repeat password change: the vault lock is already released above,
        // so the user can take their time and the vault can still lock.
        let dispatched = match dispatched? {
            Dispatched::Confirm { summary, epoch } => {
                self.confirm_then_stage(req, summary, epoch)?
            }
            other => other,
        };
        // The vault lock is released above, before the save below reaches the
        // network: a slow or hostile server must never delay locking.
        let result = match dispatched {
            Dispatched::Done(body) => Ok(body),
            Dispatched::Write { write, result } => (self.inner.save)(write).map(|()| result),
            Dispatched::Tidy { write, result } => {
                if (self.inner.save)(write).is_ok() {
                    (self.inner.on_items_changed)();
                }
                Ok(result)
            }
            Dispatched::Open { item, result } => self.open_in_desktop(item).map(|()| result),
            // `confirm_then_stage` never asks twice.
            Dispatched::Confirm { .. } => Err(ErrorCode::Internal),
        };
        if result.is_ok() && changes_items(req) {
            (self.inner.on_items_changed)();
        }
        result
    }

    /// Spend `req`'s rate-limit tokens, or refuse it. A password change also
    /// reserves its item's budget, returned so a failure can give it back,
    /// or learns that the item's interval is running.
    fn check_rate_limits(&self, req: &Request) -> Result<ItemBudget, ErrorCode> {
        if let Some(class) = request_class(req) {
            if !guard(&self.inner.limiter).allow(class, Instant::now()) {
                return Err(ErrorCode::RateLimited);
            }
        }
        // Password changes from the browser: one per item per interval,
        // reserved before the core runs (so concurrent saves cannot both
        // pass) and given back by `handle` if the change does not happen.
        // Inside the interval a `save_login` update is asked on the desktop
        // instead of refused; `save_sso` is refused as before.
        let now = Instant::now();
        match req {
            Request::SaveLogin {
                item_id: Some(id), ..
            } => Ok(if guard(&self.inner.limiter).allow_item_update(*id, now) {
                ItemBudget::Reserved(*id, now)
            } else {
                ItemBudget::CoolingDown(*id)
            }),
            Request::SaveSso {
                item_id: Some(id), ..
            } => {
                if !guard(&self.inner.limiter).allow_item_update(*id, now) {
                    return Err(ErrorCode::RateLimited);
                }
                Ok(ItemBudget::Reserved(*id, now))
            }
            _ => Ok(ItemBudget::None),
        }
    }

    /// Ask the user, then stage the save again from scratch under the vault
    /// lock: the item, the page's rules, the integration switch and the plan
    /// are all checked again. A lock in between (even one followed by an
    /// unlock) refuses it.
    fn confirm_then_stage(
        &self,
        req: &Request,
        summary: ConfirmUpdate,
        epoch: u64,
    ) -> Result<Dispatched, ErrorCode> {
        self.ask_user(summary)?;
        let mut vault = self.inner.vault.lock().map_err(|_| ErrorCode::Internal)?;
        if vault.epoch() != epoch {
            return Err(ErrorCode::Denied);
        }
        dispatch_with(&mut vault, req, unix_seconds(), UpdateConsent::Allowed)
    }

    /// Run the confirmation hook with no vault or bridge mutex held. Only an
    /// Allow given within the timeout counts. Nothing about the change is
    /// logged here or anywhere in this crate.
    fn ask_user(&self, summary: ConfirmUpdate) -> Result<(), ErrorCode> {
        let hook = guard(&self.inner.on_confirm_update).clone();
        let hook = hook.ok_or(ErrorCode::Denied)?;
        let _slot = PendingSlot::claim(&self.inner.confirm_pending).ok_or(ErrorCode::Busy)?;
        let timeout = *guard(&self.inner.confirm_timeout);
        let started = Instant::now();
        let allowed = hook(summary, timeout);
        if allowed && started.elapsed() <= timeout {
            Ok(())
        } else {
            Err(ErrorCode::Denied)
        }
    }

    /// Run the open-item hook, without holding its mutex while it runs.
    fn open_in_desktop(&self, item: Uuid) -> Result<(), ErrorCode> {
        let hook = guard(&self.inner.on_open_item).clone();
        let hook = hook.ok_or(ErrorCode::Internal)?;
        hook(item);
        Ok(())
    }

    /// Run the show-unlock hook, without holding its mutex while it runs.
    fn show_unlock(&self) -> Result<(), ErrorCode> {
        let hook = guard(&self.inner.on_show_unlock).clone();
        let hook = hook.ok_or(ErrorCode::Internal)?;
        hook();
        Ok(())
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
        let in_flight = Arc::new(AtomicUsize::new(0));
        loop {
            let reply = match read_frame(recv, MAX_REQUEST_BYTES) {
                Ok(Some(bytes)) => match parse_request(&bytes) {
                    Err(rejection) => rejection.response().into(),
                    // May wait up to a minute for the user's answer in the
                    // desktop: answered on its own thread, so the browser's
                    // other requests on this connection are not held up.
                    Ok(env) if may_wait_for_user(&env.request) => {
                        self.answer_in_background(env, tx, &in_flight);
                        continue;
                    }
                    Ok(env) => self.answer(&env),
                },
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

/// Bytes `response` takes on the wire, counted without keeping a copy.
fn wire_len(response: &Response) -> usize {
    struct Count(usize);
    impl io::Write for Count {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    match serde_json::to_writer(&mut count, response) {
        Ok(()) => count.0,
        Err(_) => usize::MAX,
    }
}

/// A `find_matches` answer too big for one frame (50 logins with the
/// longest titles, usernames and 20 tags each) loses tags, last match
/// first, until it fits: tags are only labels, and a frame over
/// `MAX_RESPONSE_BYTES` would cost the extension its connection.
fn fit_frame(mut response: Response) -> Response {
    if !matches!(response.result, Some(ResultBody::FindMatches { .. })) {
        return response;
    }
    while wire_len(&response) > MAX_RESPONSE_BYTES {
        let Some(ResultBody::FindMatches { matches, .. }) = &mut response.result else {
            break;
        };
        match matches.iter_mut().rev().find(|m| !m.tags.is_empty()) {
            Some(m) => m.tags.clear(),
            None => break,
        }
    }
    response
}

/// Which rate-limit bucket a request spends from. `status` spends nothing;
/// `lock` never gets here.
fn request_class(req: &Request) -> Option<RequestClass> {
    match req {
        Request::Status {} | Request::Lock {} => None,
        Request::FindMatches { .. }
        | Request::CheckLookalike { .. }
        | Request::GeneratorOptions {}
        | Request::GeneratePassword { .. }
        | Request::FindPasskeys { .. }
        | Request::CheckPasskeyCreate { .. }
        | Request::PasskeyStatus { .. }
        | Request::FindIdentity { .. }
        | Request::FindCards { .. } => Some(RequestClass::Lookup),
        Request::FillItem { .. }
        | Request::GetTotp { .. }
        | Request::CheckLogin { .. }
        | Request::SaveLogin { .. }
        | Request::PasskeyGet { .. }
        | Request::PasskeyCreate { .. }
        | Request::OpenItem { .. }
        // `show_unlock` raises the desktop window: a visible effect.
        | Request::ShowUnlock {}
        | Request::FillIdentity { .. }
        | Request::OpenIdentity { .. }
        // Each follows a user action; `start_sso` has a visible effect
        // (the provider's chooser page), and `check_sso` reveals which
        // sites have logins.
        | Request::StartSso { .. }
        | Request::CheckSso { .. }
        | Request::SaveSso { .. }
        // `fill_card` returns a card; `save_card` writes one.
        | Request::FillCard { .. }
        | Request::SaveCard { .. } => Some(RequestClass::Secret),
    }
}

/// Requests that may wait for the user's answer in the desktop: a browser
/// password change of an existing login (spec 2026-10-13 §4). At most one
/// waits at a time; the others answer `busy` at once, and
/// `MAX_BACKGROUND_PER_CONNECTION` bounds the threads this starts.
fn may_wait_for_user(req: &Request) -> bool {
    matches!(
        req,
        Request::SaveLogin {
            item_id: Some(_),
            ..
        }
    )
}

/// Requests that add or change an item, so the desktop list must refresh.
fn changes_items(req: &Request) -> bool {
    matches!(
        req,
        Request::SaveLogin { .. }
            | Request::PasskeyCreate { .. }
            | Request::SaveSso { .. }
            | Request::SaveCard { .. }
    )
}

#[cfg(unix)]
fn peer_is_same_user(stream: &Stream) -> bool {
    stream
        .peer_creds()
        .ok()
        .and_then(|c| c.euid())
        .is_some_and(|uid| uid == havenkeys_protocol::endpoint::current_euid())
}

// Named pipes: the DACL admits only the owner; the client's user is checked
// too, in case the pipe is not the one HavenKeys created.
#[cfg(windows)]
fn peer_is_same_user(stream: &Stream) -> bool {
    stream
        .peer_creds()
        .ok()
        .and_then(|c| c.pid())
        .is_some_and(|pid| {
            havenkeys_protocol::win_identity::process_is_current_user(pid).unwrap_or(false)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_slots_are_capped_per_connection_and_freed_on_drop() {
        let count = Arc::new(AtomicUsize::new(0));
        let first = BackgroundSlot::claim(&count).expect("first place");
        let _second = BackgroundSlot::claim(&count).expect("second place");
        assert!(BackgroundSlot::claim(&count).is_none(), "third is refused");
        drop(first);
        assert!(
            BackgroundSlot::claim(&count).is_some(),
            "freed place reused"
        );
        // The temporary above was dropped again: one place still held.
        assert_eq!(count.load(Ordering::Acquire), 1);
    }
}
