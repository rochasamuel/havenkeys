//! The desktop's answer to a repeat password change from the browser (spec
//! docs/superpowers/specs/2026-10-13-confirm-repeat-password-change-design.md
//! §5).
//!
//! The bridge calls [`ask`] on one of its threads, without the vault lock.
//! It raises the main window, tells the UI ([`CONFIRM_UPDATE_EVENT`]) and
//! waits for [`answer_update_confirmation`], a lock, the window closing or
//! the timeout, whichever comes first. Anything but an Allow is a Deny.
//! The summary carries no secret, and nothing here logs.
//!
//! The events are addressed to the main window, but `emit_to` is not an
//! isolation boundary: any webview allowed to listen to events globally
//! (the quick window has `core:event:allow-listen`) receives them too. What
//! keeps the answer to the main window is the command: it is granted in
//! capabilities/main.json only.

use crate::state::{AppState, CmdError, CmdResult};
use crate::tray;
use havenkeys_core::vault::ConfirmUpdate;
use serde::Serialize;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// A repeat password change waits for the user. Payload: [`ConfirmUpdatePayload`].
pub const CONFIRM_UPDATE_EVENT: &str = "bridge://confirm-update";
/// That wait ended, however it ended. Payload: the confirmation's ID.
pub const CONFIRM_UPDATE_CLOSED_EVENT: &str = "bridge://confirm-update-closed";

struct Waiting {
    id: Uuid,
    answer: SyncSender<bool>,
}

/// The one confirmation the user may be looking at.
#[derive(Default)]
pub struct PendingConfirmation {
    slot: Mutex<Option<Waiting>>,
}

/// What the dialog shows. No secret; never logged.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmUpdatePayload {
    id: Uuid,
    title: String,
    username: Option<String>,
    site: String,
    timeout_ms: u64,
}

fn slot(p: &PendingConfirmation) -> std::sync::MutexGuard<'_, Option<Waiting>> {
    // Poisoned means a panic elsewhere; the slot stays usable, and refusing
    // to lock would leave a wait nobody can end.
    p.slot.lock().unwrap_or_else(|e| e.into_inner())
}

impl PendingConfirmation {
    /// Start a confirmation, or `None` while another one is pending.
    pub fn begin(&self) -> Option<(Uuid, Receiver<bool>)> {
        let mut slot = slot(self);
        if slot.is_some() {
            return None;
        }
        let (answer, rx) = sync_channel(1);
        let id = Uuid::new_v4();
        *slot = Some(Waiting { id, answer });
        Some((id, rx))
    }

    /// The user's answer. Only the pending confirmation's ID is accepted,
    /// and only once.
    pub fn answer(&self, id: Uuid, allow: bool) -> bool {
        let mut slot = slot(self);
        if !slot.as_ref().is_some_and(|w| w.id == id) {
            return false;
        }
        if let Some(w) = slot.take() {
            let _ = w.answer.try_send(allow);
        }
        true
    }

    /// Deny whatever is pending: the vault locked, or the window closed.
    pub fn cancel(&self) {
        if let Some(w) = slot(self).take() {
            let _ = w.answer.try_send(false);
        }
    }

    /// Forget `id` once its wait ended. A newer confirmation is left alone.
    pub fn finish(&self, id: Uuid) {
        let mut slot = slot(self);
        if slot.as_ref().is_some_and(|w| w.id == id) {
            *slot = None;
        }
    }
}

/// The user's answer, or a Deny after `timeout` or when the answer can no
/// longer come.
pub fn wait(answer: &Receiver<bool>, timeout: Duration) -> bool {
    answer.recv_timeout(timeout).unwrap_or(false)
}

/// One confirmation from start to end: `show` raises the dialog (false when
/// it could not be shown), then the wait runs until the answer, a cancel or
/// `timeout`, whichever comes first. The deadline is enforced here, not by
/// the bridge, so the wait always ends. `closed` runs whenever a dialog was
/// started, however it ended.
///
/// `unlocked` is asked once the slot is taken: a lock that came before it
/// found nothing to cancel, so a vault already locked is a Deny at once,
/// with no dialog. A lock after this check cancels the slot as usual.
fn run(
    pending: &PendingConfirmation,
    timeout: Duration,
    unlocked: impl FnOnce() -> bool,
    show: impl FnOnce(Uuid) -> bool,
    closed: impl FnOnce(Uuid),
) -> bool {
    let Some((id, answer)) = pending.begin() else {
        return false;
    };
    if !unlocked() {
        pending.finish(id);
        return false;
    }
    let allowed = show(id) && wait(&answer, timeout);
    pending.finish(id);
    closed(id);
    allowed
}

/// The bridge's confirmation hook. No app state, a confirmation already
/// showing, a vault that locked since the request was dispatched, no main
/// window or a failed emit is a Deny at once. An emit
/// nobody listens to still succeeds (Tauri cannot tell), so a UI that never
/// shows the dialog is a Deny only at the deadline.
pub fn ask(app: &AppHandle, summary: ConfirmUpdate, timeout: Duration) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    run(
        &state.confirm,
        timeout,
        || state.require_unlocked().is_ok(),
        |id| {
            if app.get_webview_window(tray::MAIN_WINDOW).is_none() {
                return false;
            }
            tray::show_main_window(app);
            let payload = ConfirmUpdatePayload {
                id,
                title: summary.title,
                username: summary.username,
                site: summary.site,
                timeout_ms,
            };
            app.emit_to(tray::MAIN_WINDOW, CONFIRM_UPDATE_EVENT, payload)
                .is_ok()
        },
        |id| {
            let _ = app.emit_to(tray::MAIN_WINDOW, CONFIRM_UPDATE_CLOSED_EVENT, id);
        },
    )
}

/// The user's Allow or Deny in the dialog. Only the pending confirmation's
/// ID is accepted: an old dialog or a guessed ID is refused.
#[tauri::command]
pub fn answer_update_confirmation(
    state: State<'_, AppState>,
    id: Uuid,
    allow: bool,
) -> CmdResult<()> {
    state.touch();
    if state.confirm.answer(id, allow) {
        Ok(())
    } else {
        Err(CmdError {
            code: "no_confirmation",
            message: "Nothing is waiting for this answer.".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn only_the_pending_id_is_answered() {
        let p = PendingConfirmation::default();
        let (id, rx) = p.begin().unwrap();
        assert!(!p.answer(Uuid::new_v4(), true), "a guessed ID");
        assert!(p.answer(id, true));
        assert!(wait(&rx, Duration::from_secs(1)));
        assert!(!p.answer(id, false), "answered once");
    }

    #[test]
    fn one_at_a_time() {
        let p = PendingConfirmation::default();
        let (id, _rx) = p.begin().unwrap();
        assert!(p.begin().is_none());
        p.finish(id);
        assert!(p.begin().is_some());
    }

    #[test]
    fn cancel_denies_at_once() {
        let p = PendingConfirmation::default();
        let (_id, rx) = p.begin().unwrap();
        p.cancel();
        let started = Instant::now();
        assert!(!wait(&rx, Duration::from_secs(5)));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(p.begin().is_some(), "the slot is free again");
    }

    #[test]
    fn the_timeout_denies() {
        let p = PendingConfirmation::default();
        let (_id, rx) = p.begin().unwrap();
        assert!(!wait(&rx, Duration::from_millis(20)));
    }

    #[test]
    fn an_answer_after_the_wait_ended_is_refused() {
        let p = PendingConfirmation::default();
        let (old, _rx) = p.begin().unwrap();
        p.finish(old);
        assert!(!p.answer(old, true));
        let (new, _rx) = p.begin().unwrap();
        p.finish(old); // a stale finish leaves the newer one alone
        assert!(p.answer(new, true));
    }

    #[test]
    fn the_payload_carries_exactly_the_summary() {
        let payload = ConfirmUpdatePayload {
            id: Uuid::nil(),
            title: "GitHub".into(),
            username: Some("octo".into()),
            site: "github.com".into(),
            timeout_ms: 60_000,
        };
        let json = serde_json::to_value(&payload).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["id", "site", "timeoutMs", "title", "username"]);
    }

    // The hook enforces the deadline itself: the bridge only judges lateness
    // after the hook returns, so a wait nobody ends must still end.
    #[test]
    fn the_hook_gives_up_at_its_deadline_and_frees_the_slot() {
        let p = PendingConfirmation::default();
        let mut closed = None;
        let started = Instant::now();
        let allowed = run(
            &p,
            Duration::from_millis(50),
            || true,
            |_| true,
            |id| closed = Some(id),
        );
        assert!(!allowed);
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(started.elapsed() < Duration::from_secs(2));
        let closed = closed.expect("the dialog is told to close");
        assert!(!p.answer(closed, true), "a late answer is refused");
        assert!(p.begin().is_some(), "the slot is free again");
    }

    #[test]
    fn the_hook_returns_the_users_allow() {
        let p = Arc::new(PendingConfirmation::default());
        let answering = Arc::clone(&p);
        let mut closed = None;
        let allowed = run(
            &p,
            Duration::from_secs(5),
            || true,
            move |id| {
                std::thread::spawn(move || assert!(answering.answer(id, true)));
                true
            },
            |id| closed = Some(id),
        );
        assert!(allowed);
        assert!(closed.is_some());
        assert!(p.begin().is_some());
    }

    // `show` returns false when there is no main window or the emit failed.
    #[test]
    fn the_hook_denies_when_the_dialog_cannot_be_shown() {
        let p = PendingConfirmation::default();
        let started = Instant::now();
        let mut closed = false;
        assert!(!run(
            &p,
            Duration::from_secs(5),
            || true,
            |_| false,
            |_| closed = true
        ));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "no wait for an unseen dialog"
        );
        assert!(closed);
        assert!(p.begin().is_some());
    }

    #[test]
    fn the_hook_denies_while_another_confirmation_is_showing() {
        let p = PendingConfirmation::default();
        let (_id, _rx) = p.begin().unwrap();
        let mut shown = false;
        let mut closed = false;
        assert!(!run(
            &p,
            Duration::from_secs(5),
            || true,
            |_| {
                shown = true;
                true
            },
            |_| closed = true
        ));
        assert!(!shown && !closed, "the showing one is left alone");
    }

    // A lock between the bridge's dispatch and `begin` found no slot to
    // cancel: the hook must not wait out the deadline for a locked vault.
    #[test]
    fn the_hook_denies_at_once_when_the_vault_is_already_locked() {
        let p = PendingConfirmation::default();
        let started = Instant::now();
        let mut shown = false;
        let mut closed = false;
        assert!(!run(
            &p,
            Duration::from_secs(5),
            || false,
            |_| {
                shown = true;
                true
            },
            |_| closed = true
        ));
        assert!(started.elapsed() < Duration::from_secs(1), "no wait");
        assert!(!shown && !closed, "no dialog is raised");
        assert!(p.begin().is_some(), "the slot is free again");
    }

    #[test]
    fn a_lock_ends_the_hooks_wait_with_deny() {
        let p = Arc::new(PendingConfirmation::default());
        let locking = Arc::clone(&p);
        let started = Instant::now();
        let allowed = run(
            &p,
            Duration::from_secs(5),
            || true,
            move |_| {
                std::thread::spawn(move || locking.cancel());
                true
            },
            |_| {},
        );
        assert!(!allowed);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
