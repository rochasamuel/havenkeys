//! The server session, and every operation that needs one.
//!
//! The vault is a replica: reads come from SQLite and work offline, writes go
//! to the server first and are recorded locally only once it has accepted
//! them (spec 2026-09-20 §8.4). This module owns that round trip, the
//! periodic pull, and the connectivity state the UI shows.
//!
//! Nothing here holds the vault lock across an `await`: the guard is taken,
//! used and dropped before every network call, so a lock (button, auto-lock,
//! screen lock) is never delayed by a slow server.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::crypto::keys::AuthKey;
use havenkeys_core::model::ItemOverview;
use havenkeys_core::sync::SyncReport;
use havenkeys_core::vault::StagedWrite;
use havenkeys_sync_client::{HttpTransport, SyncClient, SyncError};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

/// A pull finished; the UI re-reads the item list.
pub const SYNCED_EVENT: &str = "vault://synced";
/// Online or offline changed.
pub const CONNECTIVITY_EVENT: &str = "vault://connectivity";

/// The label this device reports. Deliberately not the hostname, which is
/// metadata the server has no use for.
pub const DEVICE_NAME: &str = "Desktop";

/// How often an unlocked, online device pulls.
pub const PULL_INTERVAL_SECS: u64 = 60;

pub type Client = Arc<SyncClient<HttpTransport>>;

impl From<SyncError> for CmdError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Conflict(_) => havenkeys_core::Error::ItemChangedElsewhere.into(),
            SyncError::Unavailable => havenkeys_core::Error::Offline.into(),
            SyncError::Unauthorized => Self {
                code: "signed_out",
                message: "HavenKeys is signed out of this account. Unlock again to reconnect."
                    .into(),
            },
            SyncError::RateLimited => Self {
                code: "rate_limited",
                message: "Too many attempts. Try again in a few minutes.".into(),
            },
            SyncError::InvalidServerUrl => Self {
                code: "invalid_server_url",
                message: "That server address cannot be used. It must start with https://.".into(),
            },
            // The server's own words are never shown: they are text an
            // attacker could choose.
            SyncError::Refused(_) | SyncError::Protocol(_) | SyncError::TooLarge => Self {
                code: "sync_failed",
                message: "The server did not accept that request.".into(),
            },
        }
    }
}

/// Map a failure, and drop the session when the server says it is gone, so
/// the app falls back to read-only instead of retrying with a dead token.
fn failed(app: &AppHandle, err: SyncError) -> CmdError {
    if matches!(err, SyncError::Unauthorized | SyncError::Unavailable) {
        go_offline(app);
    }
    err.into()
}

pub fn go_offline(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if state.go_offline() {
            let _ = app.emit(CONNECTIVITY_EVENT, false);
        }
    }
}

/// The client for this vault's server. Cached, because building one sets up
/// a TLS stack.
pub fn client(state: &AppState) -> CmdResult<Client> {
    let url = state
        .vault()?
        .account()?
        .ok_or(havenkeys_core::Error::NoVault)?
        .server_url;
    client_for(state, &url)
}

pub fn client_for(state: &AppState, url: &str) -> CmdResult<Client> {
    state.sync_client(url, || {
        HttpTransport::new(url)
            .map(|t| Arc::new(SyncClient::new(t)))
            .map_err(CmdError::from)
    })
}

/// Sign in to the account with a freshly derived auth key, then catch up.
///
/// Called right after an unlock and after activation. A failure here is not
/// an unlock failure: the vault stays open and readable, offline.
pub async fn connect(app: &AppHandle, auth_key: AuthKey) -> CmdResult<()> {
    let (email, device_id, client) = {
        let state = app.state::<AppState>();
        let account = state
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?;
        let client = client_for(&state, &account.server_url)?;
        let device_id = state.device_id()?;
        (account.email, device_id, client)
    };

    let session = client
        .login(&email, &auth_key, device_id, DEVICE_NAME)
        .await
        .map_err(|e| failed(app, e))?;
    drop(auth_key);

    app.state::<AppState>().set_online(session);
    let _ = app.emit(CONNECTIVITY_EVENT, true);
    sync_now(app).await.map(|_| ())
}

/// Reconcile the header, then pull until the replica has caught up.
pub async fn sync_now(app: &AppHandle) -> CmdResult<SyncReport> {
    let (session, client) = {
        let state = app.state::<AppState>();
        // Recorded up front, so a failing server is retried on the same
        // spacing rather than on every tick.
        state.mark_sync_attempt();
        (state.session()?, client(&state)?)
    };
    let mut report = SyncReport::default();

    // The header first: a master password changed on another device must be
    // adopted before anything else, and a local change that has not reached
    // the server yet must be published. The core checks the attestation and
    // refuses a revision that goes backwards.
    let remote = client.header(&session).await.map_err(|e| failed(app, e))?;
    let local_revision = {
        let state = app.state::<AppState>();
        let revision = state.vault()?.header_revision()?.unwrap_or(0);
        if remote.revision as u64 > revision {
            report.header_adopted = state.vault()?.adopt_account_header(&remote.bytes)?;
            None
        } else if revision > remote.revision as u64 {
            Some((revision, state.vault()?.encode_account_header()?))
        } else {
            None
        }
    };
    if let Some((revision, header)) = local_revision {
        client
            .put_header(&session, &header, revision as i64)
            .await
            .map_err(|e| failed(app, e))?;
    }

    let mut cursor = {
        let state = app.state::<AppState>();
        let account = state.vault()?.account()?;
        account.map(|a| a.server_cursor).unwrap_or(0)
    };
    loop {
        let pulled = client
            .pull(&session, cursor)
            .await
            .map_err(|e| failed(app, e))?;
        let has_more = pulled.has_more;
        let next = pulled.cursor;
        let page = {
            let state = app.state::<AppState>();
            let mut vault = state.vault()?;
            vault.apply_remote_changes(next, pulled.changes, AppState::now_ms())?
        };
        report.added += page.added;
        report.updated += page.updated;
        report.deleted += page.deleted;
        report.skipped_items += page.skipped_items;

        // A server claiming there is more while handing out the same cursor
        // would spin this loop forever.
        if !has_more || next <= cursor {
            break;
        }
        cursor = next;
    }

    app.state::<AppState>().mark_sync_attempt();
    let _ = app.emit(SYNCED_EVENT, report);
    Ok(report)
}

/// Send one staged write, then record what the server accepted.
///
/// The order matters: nothing is written locally until the server has
/// assigned the revision, so the replica can never be ahead of the authority.
pub async fn push(app: &AppHandle, staged: StagedWrite) -> CmdResult<Option<ItemOverview>> {
    let (session, client) = {
        let state = app.state::<AppState>();
        (state.session()?, client(&state)?)
    };
    let item_id = staged.item_id;
    let ack = client
        .write(&session, std::slice::from_ref(&staged))
        .await
        .map_err(|e| failed(app, e))?;
    let revision = revision_for(&ack.applied, item_id)?;
    let state = app.state::<AppState>();
    let overview = state.vault()?.commit_write(staged, revision)?;
    Ok(overview)
}

fn revision_for(applied: &[(Uuid, i64)], item_id: Uuid) -> CmdResult<i64> {
    applied
        .iter()
        .find(|(id, _)| *id == item_id)
        .map(|(_, revision)| *revision)
        .ok_or_else(|| CmdError {
            code: "sync_failed",
            message: "The server did not acknowledge that item.".into(),
        })
}
