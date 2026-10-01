//! Biometric unlock (spec 2026-10-01-android-app §5.2). The client builds the
//! bundle the shell encrypts with its biometric Keystore key, and unlocks
//! from one the shell decrypted. The master password is never stored.

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::now_ms;
use havenkeys_core::unlock_bundle::UnlockBundle;
use havenkeys_core::vault::VaultStatus;
use havenkeys_core::{SecretBytes, SecretString};
use std::sync::Arc;
use tokio::task::spawn_blocking;

impl HavenClient {
    /// While unlocked, with the master password typed once more: the bytes
    /// the shell seals with its biometric key.
    pub async fn create_unlock_bundle(
        &self,
        password: SecretString,
        boot_count: i64,
    ) -> ClientResult<SecretBytes> {
        let account = self.vault_account()?;
        let ticket = self.vault()?.begin_bundle()?;
        let secret_key = self
            .device()?
            .secret_key(account.id)
            .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
        let enrolled_at = now_ms();
        let (ticket, bundle) = spawn_blocking(move || {
            let bundle = ticket.derive(&password, &secret_key, &account, enrolled_at, boot_count);
            (ticket, bundle)
        })
        .await
        .map_err(|_| ClientError::internal())?;
        let bundle = bundle?;
        // A lock while Argon2id ran wins: nothing is handed out for a vault
        // that closed in the meantime.
        let vault = self.vault()?;
        if !vault.is_unlocked() || vault.epoch() != ticket.epoch() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        Ok(bundle.encode())
    }

    /// Unlock from a bundle the shell decrypted. The server session opens in
    /// the background, as after a password unlock. If the server refuses the
    /// bundle's auth key (the master password changed on another device, or
    /// this device was revoked) the bundle is stale: the vault locks with
    /// reason "bundle_refused" and the shell asks for the password.
    pub async fn unlock_with_bundle(
        self: &Arc<Self>,
        bundle: SecretBytes,
        boot_count: i64,
    ) -> ClientResult<VaultStatus> {
        let bundle = UnlockBundle::decode(bundle.expose())?;
        bundle.check_fresh(now_ms(), boot_count)?;
        let (vault_key, auth_key) = bundle.into_keys();
        let (status, epoch) = {
            let mut v = self.vault()?;
            v.unlock_with_vault_key(&vault_key)?;
            let minutes = v.settings()?.auto_lock_minutes;
            let status = v.status()?;
            self.events.unlocked(minutes);
            (status, v.epoch())
        };
        drop(vault_key);
        let client = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = client.connect(auth_key).await {
                let same_session = client.vault().is_ok_and(|v| v.epoch() == epoch);
                if e.code == "signed_out" && same_session {
                    client.lock("bundle_refused");
                }
            }
        });
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use crate::client::tests::{client_in, RecordingEvents};
    use crate::client::HavenClient;
    use crate::stub_server::{StubAccount, StubServer};
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::sync::encode_header_for;
    use havenkeys_core::vault::{prepare_new_account_vault, VaultState};
    use havenkeys_core::SecretString;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    const PASSWORD: &str = "correct horse battery staple";

    async fn signed_in() -> (
        Arc<HavenClient>,
        Arc<RecordingEvents>,
        StubServer,
        tempfile::TempDir,
    ) {
        let account = AccountRef::new(
            Uuid::from_u128(1),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
        let made = prepare_new_account_vault(
            &SecretString::from(PASSWORD),
            &account,
            kdf,
            1_700_000_000_000,
        )
        .unwrap();
        let server = StubServer::start(StubAccount {
            account_id: account.id,
            vault_id: made.prepared.vault_id(),
            kdf: made.prepared.kdf().clone(),
            header: encode_header_for(&made.prepared).unwrap(),
            header_revision: made.prepared.header_revision() as i64,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        server.release_login.notify_one();
        client
            .sign_in(
                server.url.clone(),
                "user@example.com".into(),
                SecretString::from(PASSWORD),
                Some(made.secret_key.to_text()),
            )
            .await
            .unwrap();
        (client, events, server, dir)
    }

    async fn until(what: impl Fn() -> bool) {
        for _ in 0..200 {
            if what() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("timed out");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bundle_unlocks_a_locked_vault() {
        let (client, _events, server, _dir) = signed_in().await;
        let bundle = client
            .create_unlock_bundle(SecretString::from(PASSWORD), 7)
            .await
            .unwrap();
        client.lock("user");
        server.release_login.notify_one();
        let status = client.unlock_with_bundle(bundle, 7).await.unwrap();
        assert_eq!(status.state, VaultState::Unlocked);
        until(|| client.is_online()).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bundle_from_another_boot_is_refused() {
        let (client, _events, _server, _dir) = signed_in().await;
        let bundle = client
            .create_unlock_bundle(SecretString::from(PASSWORD), 7)
            .await
            .unwrap();
        client.lock("user");
        let err = client.unlock_with_bundle(bundle, 8).await.unwrap_err();
        assert_eq!(err.code, "bundle_refused");
        assert!(!client.vault().unwrap().is_unlocked());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unknown_boot_count_enrolls_nothing_and_unlocks_nothing() {
        let (client, _events, server, _dir) = signed_in().await;
        let err = client
            .create_unlock_bundle(SecretString::from(PASSWORD), -1)
            .await
            .unwrap_err();
        assert_eq!(err.code, "bundle_refused");
        let bundle = client
            .create_unlock_bundle(SecretString::from(PASSWORD), 0)
            .await
            .unwrap();
        client.lock("user");
        let err = client
            .unlock_with_bundle(bundle.clone(), -1)
            .await
            .unwrap_err();
        assert_eq!(err.code, "bundle_refused");
        server.release_login.notify_one();
        let status = client.unlock_with_bundle(bundle, 0).await.unwrap();
        assert_eq!(status.state, VaultState::Unlocked);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_wrong_password_enrolls_nothing() {
        let (client, _events, _server, _dir) = signed_in().await;
        let err = client
            .create_unlock_bundle(SecretString::from("not the password"), 7)
            .await
            .unwrap_err();
        assert_eq!(err.code, "unlock_failed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_lock_during_enrollment_hands_out_nothing() {
        let (client, _events, _server, _dir) = signed_in().await;
        let enrolling = {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .create_unlock_bundle(SecretString::from(PASSWORD), 7)
                    .await
            })
        };
        // Argon2id takes far longer than this.
        tokio::time::sleep(Duration::from_millis(5)).await;
        client.lock("auto");
        assert_eq!(enrolling.await.unwrap().unwrap_err().code, "locked");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_refused_auth_key_locks_the_vault() {
        let (client, events, server, _dir) = signed_in().await;
        let bundle = client
            .create_unlock_bundle(SecretString::from(PASSWORD), 7)
            .await
            .unwrap();
        client.lock("user");
        server.refuse_login.store(true, Ordering::SeqCst);
        let status = client.unlock_with_bundle(bundle, 7).await.unwrap();
        assert_eq!(status.state, VaultState::Unlocked);
        until(|| {
            events
                .seen()
                .iter()
                .any(|e| e == "locked:bundle_refused:true")
        })
        .await;
        assert!(!client.vault().unwrap().is_unlocked());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_refusal_for_an_older_session_does_not_lock_a_newer_one() {
        let (client, events, server, _dir) = signed_in().await;
        let bundle = client
            .create_unlock_bundle(SecretString::from(PASSWORD), 7)
            .await
            .unwrap();
        client.lock("user");
        server.refuse_login.store(true, Ordering::SeqCst);
        // The vault is locked and reopened (password unlock) before the
        // bundle session's refusal is handled: only the epoch it unlocked
        // may be closed.
        client.unlock_with_bundle(bundle, 7).await.unwrap();
        client.lock("user");
        server.refuse_login.store(false, Ordering::SeqCst);
        server.release_login.notify_one();
        client
            .unlock(SecretString::from(PASSWORD), None)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(client.vault().unwrap().is_unlocked());
        assert!(!events
            .seen()
            .iter()
            .any(|e| e == "locked:bundle_refused:true"));
    }
}
