//! Signing in during the move to OPAQUE (spec 2026-10-13-pake-login §6).

use havenkeys_core::crypto::keys::AuthKey;
use havenkeys_sync_client::{Session, SyncClient, SyncError, Transport};
use uuid::Uuid;

/// Only these mean "this server or account may not have OPAQUE yet". Any
/// other failure (offline, rate limited, frozen, deleted) is reported as is.
fn may_fall_back(e: &SyncError) -> bool {
    matches!(e, SyncError::Unauthorized | SyncError::Refused("not found"))
}

/// OPAQUE first. A device that has never signed in to this account with
/// OPAQUE (`confirmed` false) may then try the legacy login, and if that
/// works enrols the account at once. A confirmed device never sends the
/// auth key: someone who makes OPAQUE fail cannot make it fall back.
///
/// The bool is whether OPAQUE now works for this account.
pub(crate) async fn sign_in_with_key<T: Transport>(
    server: &SyncClient<T>,
    email: &str,
    auth_key: &AuthKey,
    account_id: Uuid,
    device_id: Uuid,
    device_name: &str,
    confirmed: bool,
) -> Result<(Session, bool), SyncError> {
    let first = match server
        .login_pake(email, auth_key, account_id, device_id, device_name)
        .await
    {
        Ok(session) => return Ok((session, true)),
        Err(e) if confirmed || !may_fall_back(&e) => return Err(e),
        Err(e) => e,
    };
    let session = match server
        .login_legacy(email, auth_key, account_id, device_id, device_name)
        .await
    {
        Ok(session) => session,
        // After the cutoff the legacy answer says nothing useful about why
        // OPAQUE failed; the first error does.
        Err(SyncError::Refused("upgrade required")) => return Err(first),
        Err(e) => return Err(e),
    };
    // Not fatal: the next sign-in tries again.
    let enrolled = server.enroll(&session, auth_key).await.is_ok();
    Ok((session, enrolled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stub_server::{StubAccount, StubServer};
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::vault::prepare_new_account_vault;
    use havenkeys_core::SecretString;
    use havenkeys_sync_client::HttpTransport;
    use std::sync::atomic::Ordering::SeqCst;

    const ACCOUNT: Uuid = Uuid::from_u128(1);

    async fn stub() -> StubServer {
        StubServer::start(StubAccount {
            account_id: ACCOUNT,
            vault_id: Uuid::from_u128(2),
            kdf: KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap(),
            header: Vec::new(),
            header_revision: 0,
        })
        .await
    }

    fn auth_key() -> AuthKey {
        let account = AccountRef::new(ACCOUNT, NormalizedEmail::parse("user@example.com").unwrap());
        let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
        prepare_new_account_vault(
            &SecretString::from("correct horse battery staple"),
            &account,
            kdf,
            1_700_000_000_000,
        )
        .unwrap()
        .auth_key
    }

    async fn attempt(stub: &StubServer, confirmed: bool) -> Result<(Session, bool), SyncError> {
        let client = SyncClient::new(HttpTransport::new(&stub.url).unwrap());
        sign_in_with_key(
            &client,
            "user@example.com",
            &auth_key(),
            ACCOUNT,
            Uuid::new_v4(),
            "Test",
            confirmed,
        )
        .await
    }

    #[tokio::test]
    async fn an_unconfirmed_device_falls_back_when_the_server_has_no_opaque() {
        let stub = stub().await; // pake/start → 404
        stub.release_login.notify_one();
        let (_, pake_ok) = attempt(&stub, false).await.unwrap();
        assert!(!pake_ok, "enrolment could not run on this server");
        assert_eq!(stub.legacy_logins.load(SeqCst), 1);
        assert_eq!(stub.enrolls.load(SeqCst), 0);
    }

    #[tokio::test]
    async fn an_unconfirmed_device_falls_back_on_401() {
        let stub = stub().await;
        stub.pake_start_status.store(401, SeqCst);
        stub.release_login.notify_one();
        assert!(attempt(&stub, false).await.is_ok());
        assert_eq!(stub.legacy_logins.load(SeqCst), 1);
    }

    #[tokio::test]
    async fn a_confirmed_device_never_falls_back_to_the_legacy_login() {
        for status in [401u16, 404] {
            let stub = stub().await;
            stub.pake_start_status.store(status, SeqCst);
            let err = attempt(&stub, true).await.unwrap_err();
            assert!(
                matches!(err, SyncError::Unauthorized | SyncError::Refused(_)),
                "{err:?}"
            );
            assert_eq!(
                stub.legacy_logins.load(SeqCst),
                0,
                "status {status}: the auth key must not be sent"
            );
        }
    }

    #[tokio::test]
    async fn other_failures_do_not_fall_back() {
        let stub = stub().await;
        stub.pake_start_status.store(503, SeqCst);
        assert_eq!(
            attempt(&stub, false).await.unwrap_err(),
            SyncError::Unavailable
        );
        assert_eq!(stub.legacy_logins.load(SeqCst), 0);
    }

    #[tokio::test]
    async fn after_the_cutoff_a_failed_fallback_reports_the_first_error() {
        let stub = stub().await;
        stub.pake_start_status.store(401, SeqCst);
        stub.legacy_status.store(426, SeqCst);
        assert_eq!(
            attempt(&stub, false).await.unwrap_err(),
            SyncError::Unauthorized
        );
        assert_eq!(stub.legacy_logins.load(SeqCst), 1);
    }
}
