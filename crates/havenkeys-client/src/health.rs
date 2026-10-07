//! Vault health for the apps: the report is computed outside the vault
//! lock (zxcvbn takes milliseconds per password), so fills never wait on it.

use crate::{ClientResult, HavenClient};
use havenkeys_core::health::{self, HealthCheck, HealthReport};
use uuid::Uuid;

#[cfg(test)]
thread_local! {
    /// Test seam: runs on the calling thread between the snapshot and the
    /// computation, where the vault lock must already be free.
    static BEFORE_COMPUTE: std::cell::RefCell<Option<Box<dyn Fn()>>> =
        const { std::cell::RefCell::new(None) };
}

impl HavenClient {
    /// Blocking: call from a worker thread (Tauri `spawn_blocking`, Android IO).
    pub fn health_report(&self, now_ms: i64) -> ClientResult<HealthReport> {
        let snapshot = {
            let vault = self.vault()?;
            if let Some(cached) = vault.cached_health(now_ms)? {
                return Ok(cached);
            }
            vault.health_snapshot()?
        };
        // The vault lock is free here: fills and other commands go ahead.
        #[cfg(test)]
        BEFORE_COMPUTE.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook();
            }
        });
        let report = health::compute(&snapshot, now_ms);
        let mut vault = self.vault()?;
        // Locked (and maybe unlocked again) while computing: the report
        // belongs to a session that is gone, so it is neither cached nor
        // returned.
        if !vault.is_unlocked() || vault.epoch() != snapshot.epoch() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        vault.store_health(&snapshot, &report);
        Ok(report)
    }

    /// Replace the checks dismissed for one item. Server first, like every
    /// other edit: nothing changes locally until the server accepts it.
    pub async fn set_health_ignored(&self, id: Uuid, checks: Vec<HealthCheck>) -> ClientResult<()> {
        self.require_online()?;
        let staged = self.vault()?.stage_health_ignored(&id, checks)?;
        self.push(staged).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BEFORE_COMPUTE;
    use crate::now_ms;
    use crate::restore::tests::{create, login, online};
    use havenkeys_core::health::HealthCheck;
    use std::cell::Cell;
    use std::rc::Rc;

    #[tokio::test]
    async fn the_report_and_a_dismissal_round_trip() {
        let (_dir, client, _server) = online().await;
        let id = create(&client, login("Weak", "password1")).await;
        let r = client.health_report(now_ms()).unwrap();
        assert_eq!(r.counts.weak, 1);
        client
            .set_health_ignored(id, vec![HealthCheck::Weak])
            .await
            .unwrap();
        let r = client.health_report(now_ms()).unwrap();
        assert_eq!(r.counts.weak, 0);
        assert_eq!(r.dismissed.len(), 1);
    }

    #[tokio::test]
    async fn a_report_computed_across_a_lock_is_not_returned_or_cached() {
        let (_dir, client, _server) = online().await;
        create(&client, login("Weak", "password1")).await;
        let probe = client.clone();
        BEFORE_COMPUTE.with(|h| *h.borrow_mut() = Some(Box::new(move || probe.lock("user"))));
        let report = client.health_report(now_ms());
        BEFORE_COMPUTE.with(|h| *h.borrow_mut() = None);
        assert_eq!(report.unwrap_err().code, "locked");
        let vault = client.vault().unwrap();
        assert!(!vault.is_unlocked());
        assert_eq!(
            vault.cached_health(now_ms()).err(),
            Some(havenkeys_core::Error::Locked)
        );
    }

    #[tokio::test]
    async fn fill_is_not_blocked_while_health_computes() {
        // health_report must not hold the vault mutex while it computes: the
        // hook runs exactly where compute is about to start and checks the lock.
        let (_dir, client, _server) = online().await;
        create(&client, login("Weak", "password1")).await;
        let seen = Rc::new(Cell::new(None));
        let (probe, out) = (client.clone(), seen.clone());
        BEFORE_COMPUTE.with(|h| {
            *h.borrow_mut() = Some(Box::new(move || out.set(Some(probe.vault_is_free()))));
        });
        let report = client.health_report(now_ms());
        BEFORE_COMPUTE.with(|h| *h.borrow_mut() = None);
        report.unwrap();
        assert_eq!(
            seen.get(),
            Some(true),
            "the hook must run, with the vault lock free"
        );
    }
}
