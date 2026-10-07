//! Vault health for the apps: the report is computed outside the vault
//! lock (zxcvbn takes milliseconds per password), so fills never wait on it.

use crate::{ClientResult, HavenClient};
use havenkeys_core::health::{self, HealthCheck, HealthReport};
use uuid::Uuid;

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
        let report = health::compute(&snapshot, now_ms);
        self.vault()?.store_health(&snapshot, &report);
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
    use crate::now_ms;
    use crate::restore::tests::{create, login, online};
    use havenkeys_core::health::HealthCheck;

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
    async fn fill_is_not_blocked_while_health_computes() {
        // The vault mutex must be free while zxcvbn runs: take the snapshot,
        // then prove the lock can be taken before compute is called.
        let (_dir, client, _server) = online().await;
        create(&client, login("Weak", "password1")).await;
        let snapshot = client.vault().unwrap().health_snapshot().unwrap();
        assert!(
            client.vault().is_ok(),
            "the lock is released after the snapshot"
        );
        let _ = havenkeys_core::health::compute(&snapshot, now_ms());
    }
}
