//! Background folder sync (docs/sync.md).
//!
//! A worker thread syncs every minute while the vault is unlocked and a sync
//! folder is set, and soon after local changes. Each round:
//!
//! 1. reads the folder with no lock held (a cloud folder may be slow, or
//!    still downloading);
//! 2. merges under the vault lock (fast, in memory and SQLite);
//! 3. writes this device's snapshot back, again with no lock held.
//!
//! Locking the vault is never delayed by sync.

use crate::state::{AppState, CmdError, CmdResult, ITEMS_CHANGED_EVENT};
use havenkeys_core::sync::{folder, SyncReport};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const INTERVAL: Duration = Duration::from_secs(60);
/// After a local change, wait this long for more changes before syncing.
const SETTLE: Duration = Duration::from_millis(1500);

/// Result of the last sync round, for the Settings screen. Counts only.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub at: i64,
    pub ok: bool,
    pub report: Option<SyncReport>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct SyncState {
    running: AtomicBool,
    trigger: Mutex<Option<Sender<()>>>,
    pub last: Mutex<Option<SyncStatus>>,
}

impl SyncState {
    /// Ask the worker to sync soon (after local changes). Never blocks.
    pub fn request(&self) {
        if let Ok(t) = self.trigger.lock() {
            if let Some(tx) = t.as_ref() {
                let _ = tx.send(());
            }
        }
    }
}

pub fn folder_error() -> CmdError {
    CmdError {
        code: "sync_folder",
        message: "Could not read or write the sync folder.".into(),
    }
}

/// Start the worker. Call once at startup.
pub fn spawn(app: &AppHandle) -> std::io::Result<()> {
    let (tx, rx) = channel::<()>();
    if let Ok(mut t) = app.state::<AppState>().sync.trigger.lock() {
        *t = Some(tx);
    }
    let handle = app.clone();
    std::thread::Builder::new()
        .name("sync".into())
        .spawn(move || loop {
            match rx.recv_timeout(INTERVAL) {
                Ok(()) => {
                    std::thread::sleep(SETTLE);
                    while rx.try_recv().is_ok() {}
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let _ = run(&handle);
        })?;
    Ok(())
}

/// One sync round. Returns `Ok(None)` when there is nothing to do (locked,
/// no folder set, or a round already running).
pub fn run(app: &AppHandle) -> CmdResult<Option<SyncReport>> {
    let state = app.state::<AppState>();
    if state.sync.running.swap(true, Ordering::SeqCst) {
        return Ok(None);
    }
    let result = round(&state);
    state.sync.running.store(false, Ordering::SeqCst);
    let status = match &result {
        Ok(None) => return Ok(None),
        Ok(Some(report)) => SyncStatus {
            at: AppState::now_ms(),
            ok: true,
            report: Some(*report),
            error: None,
        },
        Err(e) => SyncStatus {
            at: AppState::now_ms(),
            ok: false,
            report: None,
            error: Some(e.message.clone()),
        },
    };
    if let Ok(mut last) = state.sync.last.lock() {
        *last = Some(status);
    }
    if let Ok(Some(r)) = &result {
        if r.added + r.updated + r.deleted > 0 {
            let _ = app.emit(ITEMS_CHANGED_EVENT, ());
        }
    }
    result
}

fn round(state: &AppState) -> CmdResult<Option<SyncReport>> {
    let (root, me) = {
        let device = state.device.lock().map_err(|_| CmdError::internal())?;
        match &device.sync_folder {
            Some(f) => (f.clone(), device.id),
            None => return Ok(None),
        }
    };
    let vault_id = {
        let v = state.vault()?;
        if !v.is_unlocked() {
            return Ok(None);
        }
        v.vault_id()?.ok_or(havenkeys_core::Error::NoVault)?
    };
    let dir = folder::vault_dir(&root, vault_id);

    let input = folder::read(&dir, me).map_err(|_| folder_error())?;
    let out = {
        let mut v = state.vault()?;
        if !v.is_unlocked() {
            return Ok(None);
        }
        v.sync(me, input, AppState::now_ms())?
    };
    folder::write(&dir, me, out.header.as_deref(), &out.snapshot).map_err(|_| folder_error())?;
    Ok(Some(out.report))
}
